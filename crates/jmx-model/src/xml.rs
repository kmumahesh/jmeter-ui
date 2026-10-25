//! A minimal XML tree that keeps the original text of everything it does not change.
//!
//! JMeter saves plans as XML with its own spacing and escaping. To avoid noisy diffs and lost data,
//! every node keeps its raw source text; an element's start tag is only regenerated after an
//! attribute is edited.

use anyhow::{Context, Result, bail};
use quick_xml::{Reader, events::Event};

#[derive(Debug, Clone)]
pub enum Node {
    Element(Element),
    /// Raw text between tags, entities left as written.
    Text(String),
    /// Comment, processing instruction, CDATA or doctype, as written.
    Other(String),
}

#[derive(Debug, Clone)]
pub struct Element {
    pub name: String,
    /// The start tag as written, for example `<stringProp name="a">` or `<hashTree/>`.
    start_raw: String,
    attrs: Vec<(String, String)>,
    self_closing: bool,
    end_raw: String,
    pub children: Vec<Node>,
    /// Identity of a plan element for as long as the document is open; 0 when not assigned. Set by
    /// `Document::index_plan` and never reused, so selections and undo records survive edits.
    pub uid: u32,
}

/// An element's start tag, to put back after attributes were changed.
#[derive(Debug, Clone)]
pub struct Head {
    start_raw: String,
    attrs: Vec<(String, String)>,
}

#[derive(Debug, Clone)]
pub struct Document {
    /// Everything before the root element: declaration, comments, whitespace.
    pub prolog: String,
    pub root: Element,
    /// Everything after the root's end tag.
    pub epilog: String,
    /// The next element uid to hand out (see `Element::uid`).
    pub next_uid: u32,
}

impl Element {
    pub fn new(name: &str) -> Self {
        let mut e = Element {
            name: name.to_string(),
            start_raw: String::new(),
            attrs: Vec::new(),
            self_closing: false,
            end_raw: format!("</{name}>"),
            children: Vec::new(),
            uid: 0,
        };
        e.rebuild_start();
        e
    }

    /// Attribute value with entities decoded.
    pub fn attr(&self, name: &str) -> Option<String> {
        self.attrs.iter().find(|(k, _)| k == name).map(|(_, v)| unescape(v))
    }

    pub fn set_attr(&mut self, name: &str, value: &str) {
        let escaped = escape(value, true);
        match self.attrs.iter_mut().find(|(k, _)| k == name) {
            Some((_, v)) => *v = escaped,
            None => self.attrs.push((name.to_string(), escaped)),
        }
        self.rebuild_start();
    }

    fn rebuild_start(&mut self) {
        let mut s = format!("<{}", self.name);
        for (k, v) in &self.attrs {
            s.push_str(&format!(" {k}=\"{v}\""));
        }
        s.push_str(if self.self_closing && self.children.is_empty() { "/>" } else { ">" });
        self.start_raw = s;
    }

    /// Change the tag name, for example `intProp` to `stringProp`, keeping the rest of the tags as written.
    pub fn rename(&mut self, new_name: &str) {
        if self.name == new_name {
            return;
        }
        self.start_raw = self.start_raw.replacen(&format!("<{}", self.name), &format!("<{new_name}"), 1);
        if !self.end_raw.is_empty() {
            self.end_raw = self.end_raw.replacen(&format!("</{}", self.name), &format!("</{new_name}"), 1);
        }
        self.name = new_name.to_string();
    }

    pub fn head(&self) -> Head {
        Head { start_raw: self.start_raw.clone(), attrs: self.attrs.clone() }
    }

    pub fn set_head(&mut self, h: Head) {
        self.start_raw = h.start_raw;
        self.attrs = h.attrs;
    }

    /// Write an element with no children as `<name/>`.
    pub fn collapse(&mut self) {
        if self.children.is_empty() {
            self.self_closing = true;
            self.end_raw = String::new();
            self.rebuild_start();
        }
    }

    /// Undo `collapse`, so children can be added.
    pub fn expand(&mut self) {
        if self.self_closing {
            self.self_closing = false;
            self.end_raw = format!("</{}>", self.name);
            self.rebuild_start();
        }
    }

    /// Child elements only (skips text and comments).
    pub fn elements(&self) -> impl Iterator<Item = &Element> {
        self.children.iter().filter_map(|n| match n {
            Node::Element(e) => Some(e),
            _ => None,
        })
    }

    /// Text content with entities decoded, from direct text children.
    pub fn text(&self) -> String {
        self.children
            .iter()
            .filter_map(|n| match n {
                Node::Text(t) => Some(unescape(t)),
                _ => None,
            })
            .collect()
    }

    /// Replace all children with one escaped text node.
    pub fn set_text(&mut self, value: &str) {
        self.children = vec![Node::Text(escape(value, false))];
        if self.self_closing {
            self.self_closing = false;
            self.end_raw = format!("</{}>", self.name);
            self.rebuild_start();
        }
    }

    pub fn push(&mut self, child: Element) {
        if self.self_closing {
            self.self_closing = false;
            self.end_raw = format!("</{}>", self.name);
            self.rebuild_start();
        }
        self.children.push(Node::Element(child));
    }

    /// Visit this element and all descendants, depth first.
    pub fn walk_mut(&mut self, f: &mut impl FnMut(&mut Element)) {
        f(self);
        for c in &mut self.children {
            if let Node::Element(e) = c {
                e.walk_mut(f);
            }
        }
    }

    fn write(&self, out: &mut String) {
        out.push_str(&self.start_raw);
        if self.self_closing && self.children.is_empty() {
            return;
        }
        for c in &self.children {
            match c {
                Node::Element(e) => e.write(out),
                Node::Text(t) | Node::Other(t) => out.push_str(t),
            }
        }
        out.push_str(&self.end_raw);
    }
}

/// How deep elements may be nested. Real plans stay well under 100 levels; the limit keeps a hostile file
/// from overflowing the stack when the tree is walked, written or dropped.
pub const MAX_DEPTH: usize = 500;

impl Document {
    pub fn parse(src: &str) -> Result<Document> {
        // A byte order mark in front (a file saved by an editor that adds one) is not XML text: the reader skips it
        // and loses count of where it is, so it is taken off here and put back in front when the file is written.
        let body = src.trim_start_matches('\u{feff}');
        let (bom, src) = (&src[..src.len() - body.len()], body);
        let mut reader = Reader::from_str(src);
        let mut stack: Vec<Element> = Vec::new();
        let mut prolog = String::from(bom);
        let mut root: Option<Element> = None;
        let mut epilog = String::new();
        let mut last = 0usize;
        loop {
            let ev = reader.read_event().with_context(|| format!("XML error near byte {}", reader.buffer_position()))?;
            let end = reader.buffer_position() as usize;
            let Some(raw) = src.get(last..end) else {
                bail!("the reader lost its place in the text near byte {end}");
            };
            last = end;
            match ev {
                Event::Eof => break,
                Event::Start(e) => {
                    if stack.len() >= MAX_DEPTH {
                        bail!("elements are nested more than {MAX_DEPTH} levels deep near byte {}", reader.buffer_position());
                    }
                    stack.push(element_from_start(&e, raw, false)?);
                }
                Event::Empty(e) => {
                    let el = element_from_start(&e, raw, true)?;
                    attach(&mut stack, &mut root, el)?;
                }
                Event::End(_) => {
                    let mut el = stack.pop().context("unbalanced end tag")?;
                    el.end_raw = raw.to_string();
                    attach(&mut stack, &mut root, el)?;
                }
                Event::Text(_) | Event::CData(_) | Event::Comment(_) | Event::PI(_) | Event::Decl(_) | Event::DocType(_) | Event::GeneralRef(_) => {
                    let is_text = matches!(ev, Event::Text(_) | Event::GeneralRef(_));
                    if let Some(top) = stack.last_mut() {
                        match (is_text, top.children.last_mut()) {
                            (true, Some(Node::Text(prev))) => prev.push_str(raw),
                            (true, _) => top.children.push(Node::Text(raw.to_string())),
                            (false, _) => top.children.push(Node::Other(raw.to_string())),
                        }
                    } else if root.is_none() {
                        prolog.push_str(raw);
                    } else {
                        epilog.push_str(raw);
                    }
                }
            }
        }
        if !stack.is_empty() {
            bail!("unclosed element <{}>", stack.last().unwrap().name);
        }
        Ok(Document { prolog, root: root.context("no root element")?, epilog, next_uid: 1 })
    }

    pub fn write(&self) -> String {
        let mut out = String::with_capacity(self.prolog.len() + self.epilog.len() + 4096);
        out.push_str(&self.prolog);
        self.root.write(&mut out);
        out.push_str(&self.epilog);
        out
    }
}

/// Parse a fragment (one element) for use as a template.
pub fn parse_fragment(src: &str) -> Result<Element> {
    Ok(Document::parse(src)?.root)
}

fn attach(stack: &mut [Element], root: &mut Option<Element>, el: Element) -> Result<()> {
    match stack.last_mut() {
        Some(parent) => parent.children.push(Node::Element(el)),
        None => {
            if root.is_some() {
                bail!("more than one root element");
            }
            *root = Some(el);
        }
    }
    Ok(())
}

fn element_from_start(e: &quick_xml::events::BytesStart<'_>, raw: &str, self_closing: bool) -> Result<Element> {
    let name = e.name().as_ref().to_string();
    let mut attrs = Vec::new();
    for a in e.attributes().with_checks(false) {
        let a = a?;
        attrs.push((a.key.as_ref().to_string(), a.value.to_string()));
    }
    Ok(Element {
        name,
        start_raw: raw.to_string(),
        attrs,
        self_closing,
        end_raw: String::new(),
        children: Vec::new(),
        uid: 0,
    })
}

pub fn unescape(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        rest = &rest[i..];
        if let Some(j) = rest.find(';') {
            let ent = &rest[1..j];
            let rep = match ent {
                "amp" => Some('&'),
                "lt" => Some('<'),
                "gt" => Some('>'),
                "quot" => Some('"'),
                "apos" => Some('\''),
                _ if ent.starts_with("#x") => u32::from_str_radix(&ent[2..], 16).ok().and_then(char::from_u32),
                _ if ent.starts_with('#') => ent[1..].parse::<u32>().ok().and_then(char::from_u32),
                _ => None,
            };
            if let Some(c) = rep {
                out.push(c);
                rest = &rest[j + 1..];
                continue;
            }
        }
        out.push('&');
        rest = &rest[1..];
    }
    out.push_str(rest);
    out
}

pub fn escape(s: &str, attr: bool) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' if attr => out.push_str("&quot;"),
            // Line ends are kept as character references, as JMeter writes them; raw ones would be
            // normalised away when the file is read again.
            '\r' => out.push_str("&#xd;"),
            '\n' if attr => out.push_str("&#xa;"),
            '\t' if attr => out.push_str("&#x9;"),
            // XML 1.0 cannot hold these at all (not even as references), and a file with one in it cannot be
            // read by JMeter, so they are never written.
            c if !is_xml_char(c) => {}
            _ => out.push(c),
        }
    }
    out
}

/// Whether XML 1.0 allows the character.
pub fn is_xml_char(c: char) -> bool {
    matches!(c, '\u{9}' | '\u{A}' | '\u{D}' | '\u{20}'..='\u{D7FF}' | '\u{E000}'..='\u{FFFD}' | '\u{10000}'..='\u{10FFFF}')
}

#[cfg(test)]
mod bom_tests {
    use super::*;

    #[test]
    fn a_byte_order_mark_is_kept_and_does_not_confuse_the_reader() {
        for text in ["\u{feff}<a/>", "\u{feff}<?xml version=\"1.0\"?>\n<a b=\"c\">text &amp; more</a>\n", "\u{feff}\u{feff}<a/>"] {
            match Document::parse(text) {
                Ok(d) => assert_eq!(d.write(), text, "{text:?} is written back as it came"),
                Err(_) => {} // two marks in a row are not a document, and that is fine
            }
        }
        assert!(Document::parse("\u{feff}<a>\u{feff}</a>").is_ok(), "a mark inside the text is just a character");
        assert_eq!(Document::parse("\u{feff}<a>\u{feff}</a>").unwrap().write(), "\u{feff}<a>\u{feff}</a>");
    }

    #[test]
    fn what_the_fuzzer_found_does_not_panic() {
        // A mark, text that is not XML, and broken UTF-8 turned into replacement characters: the reader's position
        // used to land inside a character and the text was sliced there.
        let bytes: &[u8] = b"\xef\xbb\xbfw\x19<jmeteest\xff\xdfan vbrssin=0<j\xef\xbb\xbfw\xf80<jao\rd\r\xff\x00\xff?>";
        let text = String::from_utf8_lossy(bytes).into_owned();
        let _ = Document::parse(&text);
        let _ = parse_fragment(&text);
        assert_eq!(unescape(&escape(&text, false)), text.chars().filter(|&c| is_xml_char(c)).collect::<String>());
    }

    #[test]
    fn a_slice_that_would_split_a_character_is_an_error_not_a_panic() {
        // Whatever the reader says its position is, a bad one gives an error. Every prefix and suffix of text with
        // multi-byte characters and marks must be read or refused, never crash.
        let text = "\u{feff}<a x=\"\u{e9}\u{1F600}\">\u{feff}\u{e9}</a>\u{feff}";
        for i in 0..=text.len() {
            if text.is_char_boundary(i) {
                let _ = Document::parse(&text[..i]);
                let _ = Document::parse(&text[i..]);
            }
        }
    }
}
