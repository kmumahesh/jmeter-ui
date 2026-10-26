//! Changing a parsed plan in place while leaving every untouched byte as it was.
//!
//! A plan element and the `<hashTree>` that follows it are moved around as a `Pair`. Elements are found
//! by `uid` (see `Element::uid`), which stays the same through edits.

use crate::xml::{Document, Element, Head, Node, escape};
use anyhow::{Context as _, Result, bail};

/// One plan element with its children, as stored in the file: the element and the `<hashTree>` after it.
#[derive(Debug, Clone)]
pub struct Pair {
    pub element: Element,
    pub hash_tree: Element,
    /// The white space that stood before the element, kept so a removed pair goes back byte for byte.
    pub lead: Option<String>,
    /// The white space between the element and its `<hashTree>`.
    pub mid: Option<String>,
    /// The indentation the pair's own line breaks are written with. `None` for a new element written
    /// with indentation relative to itself.
    pub base: Option<String>,
    /// A new element (not taken from this document): its line breaks are always rewritten to match.
    pub template: bool,
}

impl Pair {
    /// A new element with no children, its text indented relative to its own first line.
    pub fn new(element: Element) -> Pair {
        Pair::with_base(element, None)
    }

    /// A new element whose text is indented as if its first line were indented with `base`.
    pub fn with_base(element: Element, base: Option<String>) -> Pair {
        let mut hash_tree = Element::new("hashTree");
        hash_tree.collapse();
        Pair { element, hash_tree, lead: None, mid: None, base, template: true }
    }

    pub fn uid(&self) -> u32 {
        self.element.uid
    }
}

/// What `Document::remove_pair` took out and where it was, enough to put it back.
#[derive(Debug, Clone)]
pub struct Removed {
    pub parent: Option<u32>,
    /// Position among the parent's plan children.
    pub index: usize,
    pub pair: Pair,
    /// Set when removing the last child emptied the parent's `<hashTree>`, which was then written as
    /// `<hashTree/>`: the closing white space that was there.
    pub closing: Option<String>,
}

/// What `Document::set_prop` replaced, enough for `restore_prop`.
#[derive(Debug, Clone)]
pub enum PropOld {
    /// The property was there: its old element, as written.
    Existing(Element),
    /// The property was added; `collapse` is set if its element had no content, and was written `<X/>`.
    Added { collapse: bool },
}

fn is_prop(n: &Node, name: &str) -> bool {
    matches!(n, Node::Element(c) if c.name.ends_with("Prop") && c.attr("name").as_deref() == Some(name))
}

/// Split `a/b/c` into the names of the nested elements (`a`, `b`) and the property (`c`). Some settings are
/// kept inside an element property, such as the loop count of a thread group.
fn split_prop_path(name: &str) -> (Vec<&str>, &str) {
    let mut parts: Vec<&str> = name.split('/').collect();
    let last = parts.pop().unwrap_or("");
    (parts, last)
}

fn is_container(c: &Element, name: &str) -> bool {
    (c.name == "elementProp" || c.name == "collectionProp") && c.attr("name").as_deref() == Some(name)
}

/// A path segment `#n` picks the n-th row of a table.
fn row_index(seg: &str) -> Option<usize> {
    seg.strip_prefix('#').and_then(|n| n.parse().ok())
}

fn nested<'a>(mut e: &'a Element, segs: &[&str]) -> Option<&'a Element> {
    for seg in segs {
        e = match row_index(seg) {
            Some(n) => e.elements().nth(n)?,
            None => e.elements().find(|c| is_container(c, seg))?,
        };
    }
    Some(e)
}

fn nested_mut<'a>(mut e: &'a mut Element, segs: &[&str]) -> Option<&'a mut Element> {
    for seg in segs {
        let row = row_index(seg);
        e = e
            .children
            .iter_mut()
            .filter_map(|n| match n {
                Node::Element(c) => Some(c),
                _ => None,
            })
            .filter(|c| row.is_some() || is_container(c, seg))
            .nth(row.unwrap_or(0))?;
    }
    Some(e)
}

/// A table row taken out of a plan element, enough to put it back exactly.
#[derive(Debug, Clone)]
pub struct RemovedRow {
    pub uid: u32,
    pub table: String,
    pub index: usize,
    lead: String,
    row: Element,
    /// Set when this was the last row and the table is now written `<collectionProp .../>`.
    closing: Option<String>,
}

type Path = Vec<usize>;

/// Every simple setting of an element with its text: a setting inside another is `outer/inner`, a table row is `#n`.
fn settings_of(element: &Element) -> Vec<(String, String)> {
    fn walk(e: &Element, prefix: &str, out: &mut Vec<(String, String)>) {
        let in_table = e.name == "collectionProp";
        for (i, c) in e.elements().enumerate() {
            if !c.name.ends_with("Prop") {
                continue;
            }
            let seg = if in_table { format!("#{i}") } else { c.attr("name").unwrap_or_default() };
            let path = if prefix.is_empty() { seg } else { format!("{prefix}/{seg}") };
            if c.name == "elementProp" || c.name == "collectionProp" {
                walk(c, &path, out);
            } else {
                out.push((path, c.text()));
            }
        }
    }
    let mut out = Vec::new();
    walk(element, "", &mut out);
    out
}

const INDENT: &str = "  ";

fn is_blank(t: &str) -> bool {
    t.trim().is_empty()
}

/// Positions in `level.children` of each (element, hashTree) pair: `(element_index, hash_tree_index)`.
fn pairs(level: &Element) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut prev = None;
    for (i, n) in level.children.iter().enumerate() {
        if let Node::Element(e) = n {
            if e.name == "hashTree" {
                if let Some(p) = prev.take() {
                    out.push((p, i));
                }
            } else {
                prev = Some(i);
            }
        }
    }
    out
}

fn el(level: &Element, i: usize) -> &Element {
    match &level.children[i] {
        Node::Element(e) => e,
        _ => unreachable!("index of an element"),
    }
}

fn el_mut(level: &mut Element, i: usize) -> &mut Element {
    match &mut level.children[i] {
        Node::Element(e) => e,
        _ => unreachable!("index of an element"),
    }
}

/// The top-level `<hashTree>` of a plan: its index among the root's children.
fn top_index(doc: &Document) -> Option<usize> {
    doc.root.children.iter().position(|n| matches!(n, Node::Element(e) if e.name == "hashTree"))
}

/// Path (indices from the root) of the `<hashTree>` holding the children of `parent`; `None` is the top.
fn level_path(doc: &Document, parent: Option<u32>) -> Option<Path> {
    let top = top_index(doc)?;
    let mut path = vec![top];
    match parent {
        None => Some(path),
        Some(uid) => {
            find_level(el(&doc.root, top), uid, &mut path).then_some(path)
        }
    }
}

fn find_level(level: &Element, uid: u32, path: &mut Path) -> bool {
    for (e, h) in pairs(level) {
        if el(level, e).uid == uid {
            path.push(h);
            return true;
        }
        path.push(h);
        if find_level(el(level, h), uid, path) {
            return true;
        }
        path.pop();
    }
    false
}

fn follow<'a>(root: &'a Element, path: &[usize]) -> &'a Element {
    path.iter().fold(root, |e, &i| el(e, i))
}

fn follow_mut<'a>(root: &'a mut Element, path: &[usize]) -> &'a mut Element {
    path.iter().fold(root, |e, &i| el_mut(e, i))
}

/// The white space in front of `children[i]`, if it is a blank text node.
fn lead_text(level: &Element, i: usize) -> Option<&str> {
    match i.checked_sub(1).and_then(|j| level.children.get(j)) {
        Some(Node::Text(t)) if is_blank(t) => Some(t),
        _ => None,
    }
}

/// The indentation (text after the last line break) of a white space string.
fn indent_of(ws: &str) -> &str {
    ws.rsplit('\n').next().unwrap_or("")
}

impl Document {
    /// Give every plan element without a `uid` one. Safe to call again after edits.
    pub fn index_plan(&mut self) {
        let Some(top) = top_index(self) else { return };
        let mut next = self.next_uid;
        fn walk(level: &mut Element, next: &mut u32) {
            for (e, h) in pairs(level) {
                let el = el_mut(level, e);
                if el.uid == 0 {
                    el.uid = *next;
                    *next += 1;
                }
                walk(el_mut(level, h), next);
            }
        }
        walk(el_mut(&mut self.root, top), &mut next);
        self.next_uid = next;
    }

    /// Hand out a fresh uid for a new element.
    pub fn new_uid(&mut self) -> u32 {
        let u = self.next_uid;
        self.next_uid += 1;
        u
    }

    /// Give every plan element inside `pair` a fresh uid (for copies and new elements).
    pub fn assign_uids(&mut self, pair: &mut Pair) {
        fn walk(level: &mut Element, next: &mut u32) {
            for (e, h) in pairs(level) {
                let el = el_mut(level, e);
                el.uid = *next;
                *next += 1;
                walk(el_mut(level, h), next);
            }
        }
        pair.element.uid = self.next_uid;
        self.next_uid += 1;
        let mut next = self.next_uid;
        walk(&mut pair.hash_tree, &mut next);
        self.next_uid = next;
    }

    fn eol(&self) -> &'static str {
        if self.prolog.contains("\r\n") || self.root.children.iter().any(|n| matches!(n, Node::Text(t) if t.contains("\r\n"))) {
            "\r\n"
        } else {
            "\n"
        }
    }

    /// The plan element with this uid.
    pub fn plan_element(&self, uid: u32) -> Option<&Element> {
        let p = self.element_path(uid)?;
        Some(follow(&self.root, &p))
    }

    pub fn plan_element_mut(&mut self, uid: u32) -> Option<&mut Element> {
        let p = self.element_path(uid)?;
        Some(follow_mut(&mut self.root, &p))
    }

    fn element_path(&self, uid: u32) -> Option<Path> {
        let top = top_index(self)?;
        let mut path = vec![top];
        fn go(level: &Element, uid: u32, path: &mut Path) -> bool {
            for (e, h) in pairs(level) {
                if el(level, e).uid == uid {
                    path.push(e);
                    return true;
                }
                path.push(h);
                if go(el(level, h), uid, path) {
                    return true;
                }
                path.pop();
            }
            false
        }
        go(el(&self.root, top), uid, &mut path).then_some(path)
    }

    /// The uids of the children of `parent` (the top level for `None`), in order.
    pub fn plan_children(&self, parent: Option<u32>) -> Option<Vec<u32>> {
        let path = level_path(self, parent)?;
        let level = follow(&self.root, &path);
        Some(pairs(level).into_iter().map(|(e, _)| el(level, e).uid).collect())
    }

    /// Where an element is: its parent's uid (`None` at the top) and its position among the siblings.
    pub fn plan_position(&self, uid: u32) -> Option<(Option<u32>, usize)> {
        let top = top_index(self)?;
        fn go(level: &Element, parent: Option<u32>, uid: u32) -> Option<(Option<u32>, usize)> {
            for (n, (e, h)) in pairs(level).into_iter().enumerate() {
                let it = el(level, e);
                if it.uid == uid {
                    return Some((parent, n));
                }
                if let Some(r) = go(el(level, h), Some(it.uid), uid) {
                    return Some(r);
                }
            }
            None
        }
        go(el(&self.root, top), None, uid)
    }

    /// A deep copy of an element and everything under it.
    pub fn copy_pair(&self, uid: u32) -> Option<Pair> {
        let (parent, index) = self.plan_position(uid)?;
        let path = level_path(self, parent)?;
        let level = follow(&self.root, &path);
        let (e, h) = pairs(level)[index];
        let lead = lead_text(level, e).unwrap_or("").to_string();
        Some(Pair { element: el(level, e).clone(), hash_tree: el(level, h).clone(), base: Some(indent_of(&lead).to_string()), lead: None, mid: None, template: false })
    }

    /// Put a pair under `parent` at position `index` among its plan children (clamped to the end).
    pub fn insert_pair(&mut self, parent: Option<u32>, index: usize, pair: Pair) -> Result<()> {
        self.insert_pair_with(parent, index, pair, None)
    }

    pub(crate) fn insert_pair_with(&mut self, parent: Option<u32>, index: usize, mut pair: Pair, closing: Option<String>) -> Result<()> {
        let Some(path) = level_path(self, parent) else { bail!("no such parent element") };
        let eol = self.eol();
        // The white space in front of the level itself, to indent its children one step deeper.
        let level_ws = {
            let (host, last) = path.split_at(path.len() - 1);
            let host = follow(&self.root, host);
            lead_text(host, last[0]).map(|t| indent_of(t).to_string()).unwrap_or_default()
        };
        let level = follow_mut(&mut self.root, &path);
        let existing = pairs(level);
        let index = index.min(existing.len());
        let (lead, at) = match (existing.get(index), pair.lead.clone()) {
            (Some(&(e, _)), lead) => {
                let own = lead_text(level, e).map(str::to_string);
                let at = if own.is_some() { e - 1 } else { e };
                (lead.or(own).unwrap_or_default(), at)
            }
            (None, lead) => {
                let sample = existing.last().and_then(|&(e, _)| lead_text(level, e).map(str::to_string));
                let at = match level.children.last() {
                    Some(Node::Text(t)) if is_blank(t) => level.children.len() - 1,
                    _ => level.children.len(),
                };
                (lead.or(sample).unwrap_or_else(|| format!("{eol}{level_ws}{INDENT}")), at)
            }
        };
        let mid = pair.mid.take().unwrap_or_else(|| lead.clone());
        let base = indent_of(&lead).to_string();
        let strip = pair.base.as_deref().unwrap_or("");
        if pair.template || strip != base {
            reindent(&mut pair.element, strip, &base, eol);
            reindent(&mut pair.hash_tree, strip, &base, eol);
        }
        let mut run = vec![Node::Text(lead), Node::Element(pair.element), Node::Text(mid), Node::Element(pair.hash_tree)];
        let was_empty = existing.is_empty();
        if was_empty {
            // A level with no pairs needs closing white space in front of its end tag.
            let close = closing.unwrap_or_else(|| format!("{eol}{level_ws}"));
            if level.children.iter().all(|n| matches!(n, Node::Text(t) if is_blank(t))) {
                level.children.clear();
                run.push(Node::Text(close));
            }
            level.expand();
            level.children.splice(0..0, run);
        } else {
            level.children.splice(at..at, run);
        }
        Ok(())
    }

    /// Take an element and its children out of the plan.
    pub fn remove_pair(&mut self, uid: u32) -> Result<Removed> {
        let Some((parent, index)) = self.plan_position(uid) else { bail!("no such element") };
        let path = level_path(self, parent).expect("parent exists");
        let level = follow_mut(&mut self.root, &path);
        let (e, h) = pairs(level)[index];
        let lead_at = if lead_text(level, e).is_some() { Some(e - 1) } else { None };
        let lead = Some(lead_at.map(|i| match &level.children[i] {
            Node::Text(t) => t.clone(),
            _ => String::new(),
        }).unwrap_or_default());
        let mid = Some(match level.children[e + 1..h].iter().collect::<Vec<_>>().as_slice() {
            [Node::Text(t)] => t.clone(),
            _ => String::new(),
        });
        let base = Some(indent_of(lead.as_deref().unwrap_or("")).to_string());
        let drained: Vec<Node> = level.children.drain(lead_at.unwrap_or(e)..=h).collect();
        let mut it = drained.into_iter().filter_map(|n| if let Node::Element(e) = n { Some(e) } else { None });
        let element = it.next().expect("element");
        let hash_tree = it.next().expect("hash tree");
        let mut closing = None;
        if pairs(level).is_empty() && level.children.iter().all(|n| matches!(n, Node::Text(t) if is_blank(t))) {
            closing = level.children.iter().find_map(|n| if let Node::Text(t) = n { Some(t.clone()) } else { None });
            level.children.clear();
            level.collapse();
        }
        Ok(Removed { parent, index, pair: Pair { element, hash_tree, lead, mid, base, template: false }, closing })
    }

    /// Undo `remove_pair`.
    pub fn restore(&mut self, r: Removed) -> Result<()> {
        self.insert_pair_with(r.parent, r.index, r.pair, r.closing)
    }

    /// Read a property (`stringProp`, `boolProp`...) of an element by name. A name like `a/b` is the property
    /// `b` inside the element property `a`.
    pub fn prop(&self, uid: u32, name: &str) -> Option<String> {
        let (segs, last) = split_prop_path(name);
        let host = nested(self.plan_element(uid)?, &segs)?;
        host.elements().find(|c| c.attr("name").as_deref() == Some(last) && c.name.ends_with("Prop")).map(|c| c.text())
    }

    /// Set a simple property, adding it (as `kind`, for example `stringProp`) if the element lacks it.
    /// Returns what is needed to put it back with `restore_prop`.
    pub fn set_prop(&mut self, uid: u32, name: &str, kind: &str, value: &str) -> Result<PropOld> {
        let (segs, name) = split_prop_path(name);
        let Some(root) = self.plan_element_mut(uid) else { bail!("no such element") };
        let Some(element) = nested_mut(root, &segs) else { bail!("no such setting group") };
        if let Some(Node::Element(p)) = element.children.iter_mut().find(|n| is_prop(n, name)) {
            let old = p.clone();
            p.set_text(value);
            // A number setting that now holds a variable or a function cannot stay a number.
            let numeric = match p.name.as_str() {
                "intProp" | "longProp" => value.trim().parse::<i64>().is_ok(),
                "doubleProp" => value.trim().parse::<f64>().is_ok(),
                _ => true,
            };
            if !numeric {
                p.rename("stringProp");
            }
            return Ok(PropOld::Existing(old));
        }
        let collapse = element.children.is_empty();
        let mut p = Element::new(kind);
        p.set_attr("name", name);
        p.set_text(value);
        // Match the white space used between the element's other children.
        let sep = element.children.iter().find_map(|n| match n {
            Node::Text(t) if is_blank(t) => Some(t.clone()),
            _ => None,
        });
        match sep {
            Some(sep) => {
                // Insert before the closing white space that precedes the end tag.
                let at = match element.children.last() {
                    Some(Node::Text(t)) if is_blank(t) => element.children.len() - 1,
                    _ => element.children.len(),
                };
                element.children.splice(at..at, [Node::Text(sep), Node::Element(p)]);
            }
            None => element.push(p),
        }
        Ok(PropOld::Added { collapse })
    }

    /// Undo a `set_prop`: the property gets its old text back exactly, or goes away if it was added.
    pub fn restore_prop(&mut self, uid: u32, name: &str, old: PropOld) -> Result<()> {
        let (segs, name) = split_prop_path(name);
        let Some(root) = self.plan_element_mut(uid) else { bail!("no such element") };
        let Some(element) = nested_mut(root, &segs) else { bail!("no such setting group") };
        let Some(i) = element.children.iter().position(|n| is_prop(n, name)) else { bail!("no such property") };
        match old {
            PropOld::Existing(e) => element.children[i] = Node::Element(e),
            PropOld::Added { collapse } => {
                let lead_at = (i > 0 && matches!(&element.children[i - 1], Node::Text(t) if is_blank(t))).then(|| i - 1);
                element.children.drain(lead_at.unwrap_or(i)..=i);
                if collapse {
                    element.children.clear();
                    element.collapse();
                }
            }
        }
        Ok(())
    }

    /// Set an attribute (`testname`, `enabled`) of a plan element. Returns the old start tag, for
    /// `restore_head`.
    pub fn set_element_attr(&mut self, uid: u32, name: &str, value: &str) -> Result<Head> {
        let Some(e) = self.plan_element_mut(uid) else { bail!("no such element") };
        let old = e.head();
        e.set_attr(name, value);
        Ok(old)
    }

    /// Put back a start tag taken with `Element::head`.
    pub fn restore_head(&mut self, uid: u32, head: Head) -> Result<Head> {
        let Some(e) = self.plan_element_mut(uid) else { bail!("no such element") };
        let current = e.head();
        e.set_head(head);
        Ok(current)
    }

    /// Every simple setting of an element with its text, including those inside element settings and tables
    /// (not the elements below it). A setting inside another is named `outer/inner`; a table row is `#n`.
    pub fn setting_texts(&self, uid: u32) -> Vec<(String, String)> {
        self.plan_element(uid).map(settings_of).unwrap_or_default()
    }

    /// The settings of every element (see [`setting_texts`](Self::setting_texts)) in one walk of the plan, in tree order. Asking
    /// for each element in turn finds each one from the top and costs as much as the plan is large every time; this costs that once.
    pub fn all_setting_texts(&self) -> Vec<(u32, Vec<(String, String)>)> {
        fn go(level: &Element, out: &mut Vec<(u32, Vec<(String, String)>)>) {
            for (e, h) in pairs(level) {
                let element = el(level, e);
                out.push((element.uid, settings_of(element)));
                go(el(level, h), out);
            }
        }
        let mut out = Vec::new();
        if let Some(top) = top_index(self) {
            go(el(&self.root, top), &mut out);
        }
        out
    }

    // ---- tables ----

    /// The table (a `collectionProp`) at `path` such as `HTTPsampler.Arguments/Arguments.arguments`, with the
    /// white space in front of it.
    fn table(&self, uid: u32, path: &str) -> Option<&Element> {
        let segs: Vec<&str> = path.split('/').collect();
        nested(self.plan_element(uid)?, &segs)
    }

    /// How many rows the table has.
    pub fn row_count(&self, uid: u32, path: &str) -> Option<usize> {
        Some(self.table(uid, path)?.elements().count())
    }

    /// The text of a row that is a plain string (as in the patterns of a Response Assertion).
    pub fn row_text(&self, uid: u32, path: &str, index: usize) -> Option<String> {
        Some(self.table(uid, path)?.elements().nth(index)?.text())
    }

    /// The simple settings in one row of a table, as (name, value).
    pub fn row_cells(&self, uid: u32, path: &str, index: usize) -> Option<Vec<(String, String)>> {
        let row = self.table(uid, path)?.elements().nth(index)?;
        Some(row.elements().filter(|c| c.name.ends_with("Prop") && c.name != "collectionProp" && c.name != "elementProp").filter_map(|c| Some((c.attr("name")?, c.text()))).collect())
    }

    fn table_parts(&mut self, uid: u32, path: &str) -> Result<(&mut Element, String)> {
        let segs: Vec<&str> = path.split('/').collect();
        let (last, parents) = segs.split_last().context("empty table path")?;
        let eol = self.eol();
        let Some(root) = self.plan_element_mut(uid) else { bail!("no such element") };
        let Some(parent) = nested_mut(root, parents) else { bail!("no such table") };
        let Some(i) = parent.children.iter().position(|n| matches!(n, Node::Element(c) if is_container(c, last))) else { bail!("no such table") };
        let indent = lead_text(parent, i).map(|t| indent_of(t).to_string()).unwrap_or_default();
        let _ = eol;
        match &mut parent.children[i] {
            Node::Element(t) => Ok((t, indent)),
            _ => unreachable!(),
        }
    }

    /// Add a row (an `elementProp`, written with indentation relative to its own first line) at `index`,
    /// clamped to the end.
    pub fn insert_row(&mut self, uid: u32, path: &str, index: usize, row: Element) -> Result<()> {
        self.insert_row_with(uid, path, index, row, None, None)
    }

    fn insert_row_with(&mut self, uid: u32, path: &str, index: usize, mut row: Element, lead: Option<String>, closing: Option<String>) -> Result<()> {
        let eol = self.eol();
        let exact = lead.is_some();
        let (table, table_indent) = self.table_parts(uid, path)?;
        let rows: Vec<usize> = table.children.iter().enumerate().filter(|(_, n)| matches!(n, Node::Element(_))).map(|(i, _)| i).collect();
        let index = index.min(rows.len());
        let sample = rows.first().and_then(|&i| lead_text(table, i)).map(str::to_string);
        let lead = lead.or(sample).unwrap_or_else(|| format!("{eol}{table_indent}{INDENT}"));
        if !exact {
            reindent(&mut row, "", indent_of(&lead), eol);
        }
        let at = match rows.get(index) {
            Some(&i) => i - usize::from(lead_text(table, i).is_some()),
            None => match table.children.last() {
                Some(Node::Text(t)) if is_blank(t) => table.children.len() - 1,
                _ => table.children.len(),
            },
        };
        if rows.is_empty() {
            let close = closing.unwrap_or_else(|| format!("{eol}{table_indent}"));
            table.children.clear();
            table.expand();
            table.children.extend([Node::Text(lead), Node::Element(row), Node::Text(close)]);
        } else {
            table.children.splice(at..at, [Node::Text(lead), Node::Element(row)]);
        }
        Ok(())
    }

    /// Take a row out of a table.
    pub fn remove_row(&mut self, uid: u32, path: &str, index: usize) -> Result<RemovedRow> {
        let (table, _) = self.table_parts(uid, path)?;
        let rows: Vec<usize> = table.children.iter().enumerate().filter(|(_, n)| matches!(n, Node::Element(_))).map(|(i, _)| i).collect();
        let Some(&i) = rows.get(index) else { bail!("no such row") };
        let lead_at = lead_text(table, i).is_some().then(|| i - 1);
        let lead = lead_at.map(|j| if let Node::Text(t) = &table.children[j] { t.clone() } else { String::new() }).unwrap_or_default();
        let mut drained = table.children.drain(lead_at.unwrap_or(i)..=i);
        let row = drained.find_map(|n| if let Node::Element(e) = n { Some(e) } else { None }).expect("row element");
        drop(drained);
        let mut closing = None;
        if table.elements().next().is_none() {
            closing = table.children.iter().find_map(|n| if let Node::Text(t) = n { Some(t.clone()) } else { None });
            table.children.clear();
            table.collapse();
        }
        Ok(RemovedRow { uid, table: path.to_string(), index, lead, row, closing })
    }

    /// Undo `remove_row`.
    pub fn restore_row(&mut self, r: RemovedRow) -> Result<()> {
        self.insert_row_with(r.uid, &r.table, r.index, r.row, Some(r.lead), r.closing)
    }

    /// An escaped copy of `text` for use in templates.
    pub fn escape_text(text: &str) -> String {
        escape(text, false)
    }
}

/// Re-indent the line breaks inside `e`: take `strip` off the start of each new line and put `add` on.
/// Line breaks are written with the document's `eol`. Only the blank text between elements is touched: a
/// value that spans several lines (a script, a request body) keeps its text exactly.
fn reindent(e: &mut Element, strip: &str, add: &str, eol: &str) {
    let layout = e.children.iter().any(|c| matches!(c, Node::Element(_)));
    for c in &mut e.children {
        match c {
            Node::Text(t) if layout && t.contains('\n') && is_blank(t) => {
                let text = t.replace("\r\n", "\n");
                let mut out = String::with_capacity(text.len() + 16);
                for (i, line) in text.split('\n').enumerate() {
                    if i > 0 {
                        out.push_str(eol);
                        out.push_str(add);
                        out.push_str(if strip.is_empty() { line } else { line.strip_prefix(strip).unwrap_or(line) });
                    } else {
                        out.push_str(line);
                    }
                }
                *t = out;
            }
            Node::Element(child) => reindent(child, strip, add, eol),
            _ => {}
        }
    }
}

// ---- elements as text, for copying between plans ----

/// The (element, hash tree) pairs directly inside `level`, as copies that remember how they were indented.
fn pairs_of(level: &Element) -> Vec<Pair> {
    pairs(level)
        .into_iter()
        .map(|(e, h)| {
            let lead = lead_text(level, e).unwrap_or("");
            Pair { element: el(level, e).clone(), hash_tree: el(level, h).clone(), lead: None, mid: None, base: Some(indent_of(lead).to_string()), template: false }
        })
        .collect()
}

impl Pair {
    /// The elements directly under this one, each with everything under it, as copies.
    pub fn children(&self) -> Vec<Pair> {
        pairs_of(&self.hash_tree)
    }

    /// How many elements this one and everything under it come to.
    pub fn element_count(&self) -> usize {
        fn count(level: &Element) -> usize {
            pairs(level).into_iter().map(|(_, h)| 1 + count(el(level, h))).sum()
        }
        1 + count(&self.hash_tree)
    }
}

const EMPTY_PLAN: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<jmeterTestPlan version=\"1.2\" properties=\"5.0\" jmeter=\"5.6.3\">\n  <hashTree>\n  </hashTree>\n</jmeterTestPlan>\n";

impl Document {
    /// The elements at the top of the plan (inside its root `<hashTree>`), each with everything under it, as
    /// copies ready to be put somewhere else. A plan has its Test Plan there; a selection saved on its own has
    /// the elements that were selected.
    pub fn top_pairs(&self) -> Vec<Pair> {
        match top_index(self) {
            Some(top) => pairs_of(el(&self.root, top)),
            None => Vec::new(),
        }
    }

    /// A plan file that holds only `pairs` at its top, which is what a copied selection looks like as text.
    pub fn with_top(pairs: Vec<Pair>) -> Result<Document> {
        let mut doc = Document::parse(EMPTY_PLAN)?;
        doc.index_plan();
        for (i, p) in pairs.into_iter().enumerate() {
            doc.insert_pair(None, i, p)?;
        }
        Ok(doc)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SRC: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<jmeterTestPlan version=\"1.2\">\n  <hashTree>\n    <TestPlan testname=\"P\">\n      <stringProp name=\"a\">1</stringProp>\n    </TestPlan>\n    <hashTree>\n      <ThreadGroup testname=\"G\"/>\n      <hashTree>\n        <HTTPSamplerProxy testname=\"S1\"/>\n        <hashTree/>\n        <HTTPSamplerProxy testname=\"S2\"/>\n        <hashTree/>\n      </hashTree>\n      <Arguments testname=\"V\"/>\n      <hashTree/>\n    </hashTree>\n  </hashTree>\n</jmeterTestPlan>\n";

    fn doc() -> Document {
        let mut d = Document::parse(SRC).unwrap();
        d.index_plan();
        d
    }

    fn name_of(d: &Document, uid: u32) -> String {
        d.plan_element(uid).unwrap().attr("testname").unwrap()
    }

    fn by_name(d: &Document, n: &str) -> u32 {
        fn walk(d: &Document, parent: Option<u32>, n: &str) -> Option<u32> {
            for c in d.plan_children(parent).unwrap() {
                if name_of(d, c) == n {
                    return Some(c);
                }
                if let Some(x) = walk(d, Some(c), n) {
                    return Some(x);
                }
            }
            None
        }
        walk(d, None, n).unwrap_or_else(|| panic!("no element {n}"))
    }

    #[test]
    fn indexing_gives_every_plan_element_a_distinct_uid_and_changes_nothing() {
        let d = doc();
        assert_eq!(d.write(), SRC);
        let mut seen = std::collections::HashSet::new();
        for n in ["P", "G", "S1", "S2", "V"] {
            assert!(seen.insert(by_name(&d, n)));
        }
        let mut again = d.clone();
        again.index_plan();
        assert_eq!(again.next_uid, d.next_uid, "indexing twice assigns nothing new");
    }

    #[test]
    fn positions_and_children() {
        let d = doc();
        let g = by_name(&d, "G");
        assert_eq!(d.plan_position(g), Some((Some(by_name(&d, "P")), 0)));
        assert_eq!(d.plan_position(by_name(&d, "S2")), Some((Some(g), 1)));
        assert_eq!(d.plan_children(Some(g)).unwrap().len(), 2);
        assert_eq!(d.plan_children(None).unwrap().len(), 1);
    }

    #[test]
    fn remove_and_restore_is_byte_identical_everywhere() {
        let d0 = doc();
        for n in ["P", "G", "S1", "S2", "V"] {
            let mut d = d0.clone();
            let r = d.remove_pair(by_name(&d, n)).unwrap();
            assert_ne!(d.write(), SRC, "{n} was removed");
            d.restore(r).unwrap();
            assert_eq!(d.write(), SRC, "restoring {n}");
        }
    }

    #[test]
    fn removing_the_last_child_collapses_the_hash_tree_and_restores() {
        let mut d = doc();
        let g = by_name(&d, "G");
        let r1 = d.remove_pair(by_name(&d, "S1")).unwrap();
        let r2 = d.remove_pair(by_name(&d, "S2")).unwrap();
        assert!(d.write().contains("<ThreadGroup testname=\"G\"/>\n      <hashTree/>"), "{}", d.write());
        assert!(d.plan_children(Some(g)).unwrap().is_empty());
        d.restore(r2).unwrap();
        d.restore(r1).unwrap();
        assert_eq!(d.write(), SRC);
    }

    #[test]
    fn inserting_matches_the_surrounding_indentation() {
        let mut d = doc();
        let g = by_name(&d, "G");
        let mut p = Pair::new(crate::xml::parse_fragment("<ConstantTimer testname=\"T\">\n  <stringProp name=\"d\">300</stringProp>\n</ConstantTimer>").unwrap());
        d.assign_uids(&mut p);
        d.insert_pair(Some(g), 1, p).unwrap();
        let out = d.write();
        assert!(out.contains("        <HTTPSamplerProxy testname=\"S1\"/>\n        <hashTree/>\n        <ConstantTimer testname=\"T\">\n          <stringProp name=\"d\">300</stringProp>\n        </ConstantTimer>\n        <hashTree/>\n        <HTTPSamplerProxy testname=\"S2\"/>"), "{out}");
        assert_eq!(d.plan_children(Some(g)).unwrap().len(), 3);
        // The result is a valid plan.
        crate::plan::plan_tree(&Document::parse(&out).unwrap()).unwrap();
    }

    #[test]
    fn inserting_at_the_end_and_into_an_empty_level() {
        let mut d = doc();
        let s1 = by_name(&d, "S1");
        let mut p = Pair::new(crate::xml::parse_fragment("<ResponseAssertion testname=\"A\"/>").unwrap());
        d.assign_uids(&mut p);
        d.insert_pair(Some(s1), 0, p).unwrap();
        let out = d.write();
        assert!(out.contains("        <HTTPSamplerProxy testname=\"S1\"/>\n        <hashTree>\n          <ResponseAssertion testname=\"A\"/>\n          <hashTree/>\n        </hashTree>\n"), "{out}");
        let g = by_name(&d, "G");
        let mut q = Pair::new(crate::xml::parse_fragment("<DebugSampler testname=\"D\"/>").unwrap());
        d.assign_uids(&mut q);
        d.insert_pair(Some(g), 99, q).unwrap();
        assert_eq!(name_of(&d, *d.plan_children(Some(g)).unwrap().last().unwrap()), "D");
        crate::plan::plan_tree(&Document::parse(&d.write()).unwrap()).unwrap();
    }

    #[test]
    fn copies_get_new_uids_and_keep_their_children() {
        let mut d = doc();
        let g = by_name(&d, "G");
        let mut c = d.copy_pair(g).unwrap();
        d.assign_uids(&mut c);
        let new_uid = c.uid();
        assert_ne!(new_uid, g);
        d.insert_pair(Some(by_name(&d, "P")), 1, c).unwrap();
        assert_eq!(d.plan_children(Some(new_uid)).unwrap().len(), 2, "children came along");
        let kids = d.plan_children(Some(new_uid)).unwrap();
        assert!(!d.plan_children(Some(g)).unwrap().iter().any(|k| kids.contains(k)), "with their own uids");
    }

    #[test]
    fn set_prop_changes_adds_and_restores() {
        let mut d = doc();
        let p = by_name(&d, "P");
        let o1 = d.set_prop(p, "a", "stringProp", "x & y").unwrap();
        assert_eq!(d.prop(p, "a").as_deref(), Some("x & y"));
        assert!(d.write().contains("<stringProp name=\"a\">x &amp; y</stringProp>"));
        let o2 = d.set_prop(p, "b", "boolProp", "true").unwrap();
        assert!(matches!(o2, PropOld::Added { collapse: false }));
        assert!(d.write().contains("<stringProp name=\"a\">x &amp; y</stringProp>\n      <boolProp name=\"b\">true</boolProp>\n    </TestPlan>"), "{}", d.write());
        d.restore_prop(p, "b", o2).unwrap();
        d.restore_prop(p, "a", o1).unwrap();
        assert_eq!(d.write(), SRC, "undoing the edits gives the file back");
        assert!(d.restore_prop(p, "nope", PropOld::Added { collapse: false }).is_err());
    }

    #[test]
    fn restoring_keeps_odd_original_text() {
        let src = "<jmeterTestPlan><hashTree><TestPlan testname='P'><stringProp name=\"a\">1 &gt; 0 &apos;x&apos;</stringProp><stringProp name=\"e\"/></TestPlan><hashTree/></hashTree></jmeterTestPlan>";
        let mut d = Document::parse(src).unwrap();
        d.index_plan();
        let p = d.plan_children(None).unwrap()[0];
        let o = d.set_prop(p, "a", "stringProp", "new").unwrap();
        let o2 = d.set_prop(p, "e", "stringProp", "filled").unwrap();
        d.restore_prop(p, "e", o2).unwrap();
        d.restore_prop(p, "a", o).unwrap();
        assert_eq!(d.write(), src);
        let h = d.set_element_attr(p, "testname", "Q").unwrap();
        assert!(d.write().contains("testname=\"Q\""));
        d.restore_head(p, h).unwrap();
        assert_eq!(d.write(), src, "single quotes in the original start tag come back");
    }

    #[test]
    fn settings_inside_an_element_property_are_reached_with_a_path() {
        let src = "<jmeterTestPlan><hashTree><TestPlan testname=\"P\"/><hashTree><ThreadGroup testname=\"G\">\n  <elementProp name=\"ThreadGroup.main_controller\" elementType=\"LoopController\">\n    <stringProp name=\"LoopController.loops\">1</stringProp>\n  </elementProp>\n  <stringProp name=\"ThreadGroup.num_threads\">5</stringProp>\n</ThreadGroup><hashTree/></hashTree></hashTree></jmeterTestPlan>";
        let mut d = Document::parse(src).unwrap();
        d.index_plan();
        let g = d.plan_children(d.plan_children(None).unwrap().first().copied()).unwrap()[0];
        assert_eq!(d.prop(g, "ThreadGroup.main_controller/LoopController.loops").as_deref(), Some("1"));
        assert_eq!(d.prop(g, "ThreadGroup.num_threads").as_deref(), Some("5"));
        assert_eq!(d.prop(g, "ThreadGroup.main_controller/nope"), None);
        assert_eq!(d.prop(g, "no.such/LoopController.loops"), None);
        let old = d.set_prop(g, "ThreadGroup.main_controller/LoopController.loops", "stringProp", "${n}").unwrap();
        assert_eq!(d.prop(g, "ThreadGroup.main_controller/LoopController.loops").as_deref(), Some("${n}"));
        assert_eq!(d.prop(g, "ThreadGroup.num_threads").as_deref(), Some("5"), "the outer setting did not change");
        let added = d.set_prop(g, "ThreadGroup.main_controller/LoopController.continue_forever", "boolProp", "false").unwrap();
        assert!(d.write().contains("<boolProp name=\"LoopController.continue_forever\">false</boolProp>\n  </elementProp>"), "{}", d.write());
        d.restore_prop(g, "ThreadGroup.main_controller/LoopController.continue_forever", added).unwrap();
        d.restore_prop(g, "ThreadGroup.main_controller/LoopController.loops", old).unwrap();
        assert_eq!(d.write(), src);
        assert!(d.set_prop(g, "missing/group", "stringProp", "x").is_err(), "a missing group is not created");
    }

    #[test]
    fn a_number_setting_given_a_variable_becomes_a_string_setting() {
        let src = "<jmeterTestPlan><hashTree><TestPlan testname=\"P\"><intProp name=\"n\">5</intProp><boolProp name=\"b\">true</boolProp></TestPlan><hashTree/></hashTree></jmeterTestPlan>";
        let mut d = Document::parse(src).unwrap();
        d.index_plan();
        let p = d.plan_children(None).unwrap()[0];
        let o = d.set_prop(p, "n", "stringProp", "7").unwrap();
        assert!(d.write().contains("<intProp name=\"n\">7</intProp>"), "a number stays a number");
        d.restore_prop(p, "n", o).unwrap();
        let o = d.set_prop(p, "n", "stringProp", "${threads}").unwrap();
        assert!(d.write().contains("<stringProp name=\"n\">${threads}</stringProp>"), "{}", d.write());
        d.restore_prop(p, "n", o).unwrap();
        assert_eq!(d.write(), src);
        d.set_prop(p, "n", "stringProp", "").unwrap();
        assert!(d.write().contains("<stringProp name=\"n\"></stringProp>"), "an empty value is not a number either");
    }

    #[test]
    fn set_prop_on_an_empty_element() {
        let mut d = doc();
        let g = by_name(&d, "G");
        let old = d.set_prop(g, "x", "stringProp", "1").unwrap();
        let out = d.write();
        assert!(out.contains("<ThreadGroup testname=\"G\"><stringProp name=\"x\">1</stringProp></ThreadGroup>"), "{out}");
        crate::plan::plan_tree(&Document::parse(&out).unwrap()).unwrap();
        d.restore_prop(g, "x", old).unwrap();
        assert_eq!(d.write(), SRC, "and the element is self-closing again");
    }

    #[test]
    fn attributes_are_escaped() {
        let mut d = doc();
        let p = by_name(&d, "P");
        d.set_element_attr(p, "testname", "A \"quoted\" & <odd>").unwrap();
        assert!(d.write().contains("testname=\"A &quot;quoted&quot; &amp; &lt;odd&gt;\""));
        assert_eq!(name_of(&d, p), "A \"quoted\" & <odd>");
    }

    #[test]
    fn crlf_files_get_crlf_in_new_elements() {
        let src = SRC.replace('\n', "\r\n");
        let mut d = Document::parse(&src).unwrap();
        d.index_plan();
        let mut p = Pair::new(crate::xml::parse_fragment("<Arguments testname=\"N\">\n  <stringProp name=\"a\">b</stringProp>\n</Arguments>").unwrap());
        d.assign_uids(&mut p);
        let g = by_name(&d, "G");
        d.insert_pair(Some(g), 0, p).unwrap();
        let out = d.write();
        assert!(!out.replace("\r\n", "").contains('\n'), "no bare line feeds were added");
    }

    #[test]
    fn missing_things_are_errors_not_panics() {
        let mut d = doc();
        assert!(d.remove_pair(9999).is_err());
        assert!(d.set_prop(9999, "a", "stringProp", "1").is_err());
        assert!(d.set_element_attr(9999, "x", "y").is_err());
        let p = Pair::new(Element::new("X"));
        assert!(d.insert_pair(Some(9999), 0, p).is_err());
        assert!(d.copy_pair(9999).is_none());
    }

    const TABLE_SRC: &str = "<jmeterTestPlan><hashTree>\n  <TestPlan testname=\"P\"/>\n  <hashTree>\n    <HTTPSamplerProxy testname=\"S\">\n      <elementProp name=\"HTTPsampler.Arguments\" elementType=\"Arguments\">\n        <collectionProp name=\"Arguments.arguments\">\n          <elementProp name=\"a\" elementType=\"HTTPArgument\">\n            <stringProp name=\"Argument.name\">a</stringProp>\n            <stringProp name=\"Argument.value\">1</stringProp>\n          </elementProp>\n          <elementProp name=\"b\" elementType=\"HTTPArgument\">\n            <stringProp name=\"Argument.name\">b</stringProp>\n            <stringProp name=\"Argument.value\">2</stringProp>\n          </elementProp>\n        </collectionProp>\n      </elementProp>\n      <elementProp name=\"Empty\" elementType=\"Arguments\">\n        <collectionProp name=\"Arguments.arguments\"/>\n      </elementProp>\n    </HTTPSamplerProxy>\n    <hashTree/>\n  </hashTree>\n</hashTree></jmeterTestPlan>";
    const ROWS: &str = "HTTPsampler.Arguments/Arguments.arguments";

    fn table_doc() -> (Document, u32) {
        let mut d = Document::parse(TABLE_SRC).unwrap();
        d.index_plan();
        let top = d.plan_children(None).unwrap()[0];
        let s = d.plan_children(Some(top)).unwrap()[0];
        (d, s)
    }

    fn row(name: &str, value: &str) -> Element {
        crate::xml::parse_fragment(&format!("<elementProp name=\"{name}\" elementType=\"HTTPArgument\">\n  <stringProp name=\"Argument.name\">{name}</stringProp>\n  <stringProp name=\"Argument.value\">{value}</stringProp>\n</elementProp>")).unwrap()
    }

    #[test]
    fn every_setting_text_of_an_element_is_listed_with_its_path() {
        let (mut d, s) = table_doc();
        d.set_prop(s, "HTTPsampler.Arguments/Arguments.arguments/#1/Argument.value", "stringProp", "${x}").unwrap();
        let t = d.setting_texts(s);
        assert!(t.contains(&("HTTPsampler.Arguments/Arguments.arguments/#0/Argument.name".to_string(), "a".to_string())), "{t:?}");
        assert!(t.contains(&("HTTPsampler.Arguments/Arguments.arguments/#1/Argument.value".to_string(), "${x}".to_string())));
        assert!(d.setting_texts(9999).is_empty());
    }

    #[test]
    fn rows_can_be_read_by_position() {
        let (d, s) = table_doc();
        assert_eq!(d.row_count(s, ROWS), Some(2));
        assert_eq!(d.row_cells(s, ROWS, 1).unwrap(), [("Argument.name".to_string(), "b".to_string()), ("Argument.value".to_string(), "2".to_string())]);
        assert_eq!(d.row_cells(s, ROWS, 2), None);
        assert_eq!(d.row_count(s, "no/such"), None);
        // A cell is a setting reached through the row segment.
        assert_eq!(d.prop(s, "HTTPsampler.Arguments/Arguments.arguments/#0/Argument.value").as_deref(), Some("1"));
        assert_eq!(d.prop(s, "HTTPsampler.Arguments/Arguments.arguments/#5/Argument.value"), None);
    }

    #[test]
    fn editing_a_cell_goes_through_set_prop_and_restores() {
        let (mut d, s) = table_doc();
        let path = "HTTPsampler.Arguments/Arguments.arguments/#1/Argument.value";
        let old = d.set_prop(s, path, "stringProp", "two & more").unwrap();
        assert!(d.write().contains("<stringProp name=\"Argument.value\">two &amp; more</stringProp>"));
        d.restore_prop(s, path, old).unwrap();
        assert_eq!(d.write(), TABLE_SRC);
    }

    #[test]
    fn inserting_a_row_matches_the_indentation_and_undoes_exactly() {
        let (mut d, s) = table_doc();
        d.insert_row(s, ROWS, 1, row("mid", "x")).unwrap();
        let out = d.write();
        assert!(out.contains("          </elementProp>\n          <elementProp name=\"mid\" elementType=\"HTTPArgument\">\n            <stringProp name=\"Argument.name\">mid</stringProp>\n            <stringProp name=\"Argument.value\">x</stringProp>\n          </elementProp>\n          <elementProp name=\"b\""), "{out}");
        assert_eq!(d.row_count(s, ROWS), Some(3));
        d.insert_row(s, ROWS, 99, row("last", "y")).unwrap();
        assert_eq!(d.row_cells(s, ROWS, 3).unwrap()[0].1, "last", "an index past the end appends");
        let again = Document::parse(&d.write()).unwrap();
        assert!(crate::plan::plan_tree(&again).is_ok());
        let r = d.remove_row(s, ROWS, 3).unwrap();
        let r2 = d.remove_row(s, ROWS, 1).unwrap();
        d.restore_row(r2).unwrap();
        d.restore_row(r).unwrap();
        d.remove_row(s, ROWS, 3).unwrap();
        d.remove_row(s, ROWS, 1).unwrap();
        assert_eq!(d.write(), TABLE_SRC);
    }

    #[test]
    fn removing_every_row_collapses_the_table_and_restoring_expands_it() {
        let (mut d, s) = table_doc();
        let a = d.remove_row(s, ROWS, 0).unwrap();
        let b = d.remove_row(s, ROWS, 0).unwrap();
        assert!(d.write().contains("<collectionProp name=\"Arguments.arguments\"/>\n      </elementProp>\n      <elementProp name=\"Empty\""), "{}", d.write());
        assert_eq!(d.row_count(s, ROWS), Some(0));
        d.restore_row(b).unwrap();
        d.restore_row(a).unwrap();
        assert_eq!(d.write(), TABLE_SRC, "restored in reverse order gives the same file");
    }

    #[test]
    fn a_row_can_be_added_to_an_empty_table() {
        let (mut d, s) = table_doc();
        d.insert_row(s, "Empty/Arguments.arguments", 0, row("n", "v")).unwrap();
        let out = d.write();
        assert!(out.contains("<collectionProp name=\"Arguments.arguments\">\n          <elementProp name=\"n\" elementType=\"HTTPArgument\">\n            <stringProp name=\"Argument.name\">n</stringProp>\n            <stringProp name=\"Argument.value\">v</stringProp>\n          </elementProp>\n        </collectionProp>"), "{out}");
        assert!(crate::plan::plan_tree(&Document::parse(&out).unwrap()).is_ok());
        let r = d.remove_row(s, "Empty/Arguments.arguments", 0).unwrap();
        assert!(d.write().ends_with(TABLE_SRC) || d.write() == TABLE_SRC);
        d.restore_row(r).unwrap();
    }

    #[test]
    fn bad_table_paths_are_errors() {
        let (mut d, s) = table_doc();
        assert!(d.insert_row(s, "nope/Arguments.arguments", 0, row("a", "b")).is_err());
        assert!(d.remove_row(s, ROWS, 9).is_err());
        assert!(d.remove_row(9999, ROWS, 0).is_err());
    }
}

#[cfg(test)]
mod copy_tests {
    use super::*;

    const SRC: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<jmeterTestPlan version=\"1.2\">\n  <hashTree>\n    <TestPlan testname=\"P\">\n      <stringProp name=\"a\">1</stringProp>\n    </TestPlan>\n    <hashTree>\n      <ThreadGroup testname=\"G\"/>\n      <hashTree>\n        <HTTPSamplerProxy testname=\"S1\">\n          <stringProp name=\"script\">line 1\n  line 2 &amp; &lt;x&gt;</stringProp>\n        </HTTPSamplerProxy>\n        <hashTree/>\n        <HTTPSamplerProxy testname=\"S2\"/>\n        <hashTree/>\n      </hashTree>\n      <Arguments testname=\"V\"/>\n      <hashTree/>\n    </hashTree>\n  </hashTree>\n</jmeterTestPlan>\n";

    fn doc() -> Document {
        let mut d = Document::parse(SRC).unwrap();
        d.index_plan();
        d
    }

    fn uid_of(d: &Document, name: &str) -> u32 {
        fn find(d: &Document, parent: Option<u32>, name: &str) -> Option<u32> {
            for uid in d.plan_children(parent)? {
                if d.plan_element(uid)?.attr("testname").as_deref() == Some(name) {
                    return Some(uid);
                }
                if let Some(u) = find(d, Some(uid), name) {
                    return Some(u);
                }
            }
            None
        }
        find(d, None, name).unwrap_or_else(|| panic!("no {name}"))
    }

    fn names(pairs: &[Pair]) -> Vec<String> {
        pairs.iter().map(|p| p.element.attr("testname").unwrap_or_default()).collect()
    }

    #[test]
    fn the_top_of_a_plan_is_its_test_plan_and_its_children_are_reachable() {
        let d = doc();
        let top = d.top_pairs();
        assert_eq!(names(&top), ["P"]);
        let kids = top[0].children();
        assert_eq!(names(&kids), ["G", "V"]);
        assert_eq!(names(&kids[0].children()), ["S1", "S2"]);
        assert_eq!(top[0].element_count(), 5);
        assert_eq!(kids[0].element_count(), 3);
        assert_eq!(kids[1].element_count(), 1);
    }

    #[test]
    fn copied_elements_make_a_plan_file_that_holds_only_them() {
        let d = doc();
        let pairs = vec![d.copy_pair(uid_of(&d, "G")).unwrap(), d.copy_pair(uid_of(&d, "V")).unwrap()];
        let text = Document::with_top(pairs).unwrap().write();
        let again = Document::parse(&text).unwrap_or_else(|e| panic!("{e:#}\n{text}"));
        let top = again.top_pairs();
        assert_eq!(names(&top), ["G", "V"]);
        assert_eq!(names(&top[0].children()), ["S1", "S2"]);
        assert!(text.contains("<jmeterTestPlan"), "{text}");
    }

    #[test]
    fn text_with_lines_and_markup_survives_being_copied() {
        let d = doc();
        let text = Document::with_top(vec![d.copy_pair(uid_of(&d, "S1")).unwrap()]).unwrap().write();
        let again = Document::parse(&text).unwrap();
        let s1 = &again.top_pairs()[0];
        let script = s1.element.elements().find(|e| e.attr("name").as_deref() == Some("script")).unwrap().text();
        assert_eq!(script, "line 1\n  line 2 & <x>", "{text}");
    }

    #[test]
    fn a_pair_from_another_document_can_be_put_under_an_element_and_gets_new_ids() {
        let src = doc();
        let mut dst = doc();
        let mut pair = src.top_pairs()[0].children().remove(0); // G, with S1 and S2
        dst.assign_uids(&mut pair);
        let g2 = pair.uid();
        let target = uid_of(&dst, "V");
        let before = dst.plan_children(Some(target)).unwrap().len();
        dst.insert_pair(Some(target), 0, pair).unwrap();
        assert_eq!(dst.plan_children(Some(target)).unwrap().len(), before + 1);
        assert_eq!(dst.plan_element(g2).unwrap().attr("testname").as_deref(), Some("G"));
        let all: Vec<u32> = dst.plan_children(Some(g2)).unwrap();
        assert_eq!(all.len(), 2);
        let reparsed = Document::parse(&dst.write()).unwrap();
        assert_eq!(reparsed.top_pairs()[0].element_count(), 8, "five elements and the three that came with the copy");
    }

    #[test]
    fn nothing_copied_is_an_empty_plan_file() {
        let text = Document::with_top(vec![]).unwrap().write();
        assert!(Document::parse(&text).unwrap().top_pairs().is_empty());
    }

    #[test]
    fn a_document_without_a_plan_tree_has_no_top_elements() {
        assert!(Document::parse("<a><b/></a>").unwrap().top_pairs().is_empty());
        assert!(Document::parse("<jmeterTestPlan/>").unwrap().top_pairs().is_empty());
    }
}
