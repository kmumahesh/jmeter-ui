//! Colouring a line of text in an editor: JSON, XML, and `${variables}` in any text.
//!
//! The colouring works one line at a time, so a view only has to colour the lines it draws. It is a guide for the
//! eye, not a parser: a broken line is coloured as far as it makes sense and never causes an error.

use std::ops::Range;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Language {
    Plain,
    Json,
    Xml,
}

impl Language {
    /// A guess from the text: JSON starts with `{` or `[`, XML with `<`, anything else is plain.
    pub fn detect(text: &str) -> Language {
        match text.trim_start().chars().next() {
            Some('{' | '[') => Language::Json,
            Some('<') => Language::Xml,
            _ => Language::Plain,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Token {
    Plain,
    /// A text in quotes (a JSON value, an XML attribute value).
    String,
    /// A JSON key.
    Key,
    Number,
    /// `true`, `false` and `null`.
    Keyword,
    /// Braces, brackets, commas, colons, `<`, `>`, `=`.
    Punct,
    /// An XML element name.
    Tag,
    /// An XML attribute name.
    Attribute,
    Comment,
    /// A `${variable}` or `${__function(...)}` reference.
    Variable,
}

/// Lines longer than this are not coloured: they are drawn plain, which keeps a huge line from slowing the editor.
pub const MAX_COLOURED_LINE: usize = 20_000;

/// The longest `${...}` reference recognised.
const MAX_VARIABLE: usize = 300;

/// Split a line into runs of one kind. The runs follow each other without gaps from the start of the line to its
/// end, none is empty, and every edge is on a character boundary.
pub fn tokens(line: &str, language: Language) -> Vec<(Range<usize>, Token)> {
    if line.is_empty() {
        return Vec::new();
    }
    if line.len() > MAX_COLOURED_LINE {
        return vec![(0..line.len(), Token::Plain)];
    }
    let raw = match language {
        Language::Plain => Vec::new(),
        Language::Json => json(line),
        Language::Xml => xml(line),
    };
    let filled = fill(raw, line.len());
    overlay_variables(line, filled)
}

/// Make the runs cover the whole line: gaps become plain, empty runs go, and runs are merged when equal.
fn fill(raw: Vec<(Range<usize>, Token)>, len: usize) -> Vec<(Range<usize>, Token)> {
    let mut out: Vec<(Range<usize>, Token)> = Vec::new();
    let mut push = |range: Range<usize>, tok: Token| {
        if range.is_empty() {
            return;
        }
        if let Some((last, t)) = out.last_mut()
            && *t == tok
            && last.end == range.start
        {
            last.end = range.end;
            return;
        }
        out.push((range, tok));
    };
    let mut at = 0;
    for (r, t) in raw {
        let start = r.start.max(at);
        if start > at {
            push(at..start, Token::Plain);
        }
        if r.end > start {
            push(start..r.end, t);
            at = r.end;
        }
    }
    if at < len {
        push(at..len, Token::Plain);
    }
    out
}

fn json(line: &str) -> Vec<(Range<usize>, Token)> {
    let b = line.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'"' => {
                let start = i;
                i += 1;
                while i < b.len() {
                    match b[i] {
                        b'\\' => i = (i + 2).min(b.len()),
                        b'"' => {
                            i += 1;
                            break;
                        }
                        _ => i += 1,
                    }
                }
                let key = b[i..].iter().find(|c| !c.is_ascii_whitespace()) == Some(&b':');
                out.push((start..i, if key { Token::Key } else { Token::String }));
            }
            b'{' | b'}' | b'[' | b']' | b',' | b':' => {
                out.push((i..i + 1, Token::Punct));
                i += 1;
            }
            c if c == b'-' || c.is_ascii_digit() => {
                let start = i;
                i += 1;
                while i < b.len() && matches!(b[i], b'0'..=b'9' | b'.' | b'e' | b'E' | b'+' | b'-') {
                    i += 1;
                }
                out.push((start..i, Token::Number));
            }
            c if c.is_ascii_alphabetic() => {
                let start = i;
                while i < b.len() && b[i].is_ascii_alphabetic() {
                    i += 1;
                }
                if matches!(&line[start..i], "true" | "false" | "null") {
                    out.push((start..i, Token::Keyword));
                }
            }
            _ => i += 1,
        }
    }
    out
}

fn xml(line: &str) -> Vec<(Range<usize>, Token)> {
    let b = line.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if b[i] != b'<' {
            i += 1;
            continue;
        }
        if line[i..].starts_with("<!--") {
            let end = line[i + 4..].find("-->").map_or(b.len(), |e| i + 4 + e + 3);
            out.push((i..end, Token::Comment));
            i = end;
            continue;
        }
        // `<`, then `/`, `?` or `!`, then the name.
        let mut j = i + 1;
        if j < b.len() && matches!(b[j], b'/' | b'?' | b'!') {
            j += 1;
        }
        out.push((i..j, Token::Punct));
        let name_start = j;
        while j < b.len() && !b[j].is_ascii_whitespace() && !matches!(b[j], b'>' | b'/') {
            j += 1;
        }
        out.push((name_start..j, Token::Tag));
        // Attributes, up to the end of the tag.
        while j < b.len() && b[j] != b'>' {
            match b[j] {
                c if c.is_ascii_whitespace() => j += 1,
                b'"' | b'\'' => {
                    let quote = b[j];
                    let start = j;
                    j += 1;
                    while j < b.len() && b[j] != quote {
                        j += 1;
                    }
                    j = (j + 1).min(b.len());
                    out.push((start..j, Token::String));
                }
                b'=' | b'/' | b'?' => {
                    out.push((j..j + 1, Token::Punct));
                    j += 1;
                }
                _ => {
                    let start = j;
                    while j < b.len() && !b[j].is_ascii_whitespace() && !matches!(b[j], b'=' | b'>' | b'/' | b'"' | b'\'') {
                        j += 1;
                    }
                    if j == start {
                        j += 1;
                    }
                    out.push((start..j, Token::Attribute));
                }
            }
        }
        if j < b.len() {
            out.push((j..j + 1, Token::Punct));
            j += 1;
        }
        i = j;
    }
    // Names and strings were found by bytes: keep every edge on a character boundary.
    for (r, _) in out.iter_mut() {
        while !line.is_char_boundary(r.start) {
            r.start += 1;
        }
        while !line.is_char_boundary(r.end) {
            r.end += 1;
        }
    }
    out
}

/// The `${...}` references in a line, as byte ranges. Nested ones (`${__P(${name})}`) are one reference.
fn variable_ranges(line: &str) -> Vec<Range<usize>> {
    let b = line.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i + 1 < b.len() {
        if b[i] == b'$' && b[i + 1] == b'{' {
            let mut depth = 0usize;
            let mut j = i + 1;
            let limit = (i + MAX_VARIABLE).min(b.len());
            let mut end = None;
            while j < limit {
                match b[j] {
                    b'{' => depth += 1,
                    b'}' => {
                        depth -= 1;
                        if depth == 0 {
                            end = Some(j + 1);
                            break;
                        }
                    }
                    _ => {}
                }
                j += 1;
            }
            match end {
                Some(e) => {
                    out.push(i..e);
                    i = e;
                }
                None => i += 2,
            }
        } else {
            i += 1;
        }
    }
    out
}

fn overlay_variables(line: &str, spans: Vec<(Range<usize>, Token)>) -> Vec<(Range<usize>, Token)> {
    let vars = variable_ranges(line);
    if vars.is_empty() {
        return spans;
    }
    let mut out: Vec<(Range<usize>, Token)> = Vec::with_capacity(spans.len() + vars.len() * 2);
    for (range, tok) in spans {
        if tok == Token::Comment {
            out.push((range, tok));
            continue;
        }
        let mut at = range.start;
        for v in vars.iter().filter(|v| v.start < range.end && v.end > range.start) {
            let (vs, ve) = (v.start.max(range.start), v.end.min(range.end));
            if vs > at {
                out.push((at..vs, tok));
            }
            out.push((vs..ve, Token::Variable));
            at = ve;
        }
        if at < range.end {
            out.push((at..range.end, tok));
        }
    }
    // Neighbouring pieces of one reference (cut by the edges of a span) are one run again.
    let mut merged: Vec<(Range<usize>, Token)> = Vec::with_capacity(out.len());
    for (r, t) in out {
        if let Some((last, lt)) = merged.last_mut()
            && *lt == t
            && last.end == r.start
        {
            last.end = r.end;
        } else {
            merged.push((r, t));
        }
    }
    merged
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(line: &str, lang: Language) -> Vec<(&str, Token)> {
        tokens(line, lang).into_iter().map(|(r, t)| (&line[r], t)).collect()
    }

    fn assert_covers(line: &str, lang: Language) {
        let t = tokens(line, lang);
        let mut at = 0;
        for (r, tok) in &t {
            assert_eq!(r.start, at, "gap or overlap in {line:?} as {lang:?}: {t:?}");
            assert!(r.end > r.start, "empty run in {line:?}: {t:?} ({tok:?})");
            assert!(line.is_char_boundary(r.start) && line.is_char_boundary(r.end), "{line:?}: {r:?}");
            at = r.end;
        }
        assert_eq!(at, line.len(), "{line:?} as {lang:?} is not covered: {t:?}");
    }

    #[test]
    fn the_language_is_guessed_from_the_first_character() {
        assert_eq!(Language::detect("  {\"a\":1}"), Language::Json);
        assert_eq!(Language::detect("[1,2]"), Language::Json);
        assert_eq!(Language::detect("\n\n<a/>"), Language::Xml);
        assert_eq!(Language::detect("name=value"), Language::Plain);
        assert_eq!(Language::detect(""), Language::Plain);
        assert_eq!(Language::detect("   "), Language::Plain);
    }

    #[test]
    fn json_keys_values_numbers_and_keywords() {
        use Token::*;
        assert_eq!(
            kinds(r#"  "name": "Ann", "age": -3.5e2, "ok": true, "x": null"#, Language::Json),
            vec![
                ("  ", Plain),
                ("\"name\"", Key),
                (":", Punct),
                (" ", Plain),
                ("\"Ann\"", String),
                (",", Punct),
                (" ", Plain),
                ("\"age\"", Key),
                (":", Punct),
                (" ", Plain),
                ("-3.5e2", Number),
                (",", Punct),
                (" ", Plain),
                ("\"ok\"", Key),
                (":", Punct),
                (" ", Plain),
                ("true", Keyword),
                (",", Punct),
                (" ", Plain),
                ("\"x\"", Key),
                (":", Punct),
                (" ", Plain),
                ("null", Keyword),
            ]
        );
    }

    #[test]
    fn json_strings_with_escapes_and_a_missing_end() {
        use Token::*;
        assert_eq!(kinds(r#""a \" b""#, Language::Json), vec![(r#""a \" b""#, String)]);
        assert_eq!(kinds(r#""never ends"#, Language::Json), vec![(r#""never ends"#, String)]);
        assert_eq!(kinds(r#""ends in a slash\"#, Language::Json), vec![(r#""ends in a slash\"#, String)]);
    }

    #[test]
    fn json_words_that_are_not_keywords_stay_plain() {
        assert_eq!(kinds("truthy", Language::Json), vec![("truthy", Token::Plain)]);
        assert_eq!(kinds("nullable", Language::Json), vec![("nullable", Token::Plain)]);
    }

    #[test]
    fn xml_tags_attributes_and_text() {
        use Token::*;
        assert_eq!(
            kinds(r#"<item id="7" ok>text</item>"#, Language::Xml),
            vec![
                ("<", Punct),
                ("item", Tag),
                (" ", Plain),
                ("id", Attribute),
                ("=", Punct),
                ("\"7\"", String),
                (" ", Plain),
                ("ok", Attribute),
                (">", Punct),
                ("text", Plain),
                ("</", Punct),
                ("item", Tag),
                (">", Punct),
            ]
        );
        assert_eq!(kinds("<a/>", Language::Xml), vec![("<", Token::Punct), ("a", Token::Tag), ("/>", Token::Punct)]);
        assert_eq!(kinds("<?xml version='1.0'?>", Language::Xml)[0], ("<?", Token::Punct));
    }

    #[test]
    fn xml_comments_run_to_their_end_or_the_end_of_the_line() {
        use Token::*;
        assert_eq!(kinds("a<!-- note -->b", Language::Xml), vec![("a", Plain), ("<!-- note -->", Comment), ("b", Plain)]);
        assert_eq!(kinds("<!-- open", Language::Xml), vec![("<!-- open", Comment)]);
    }

    #[test]
    fn variables_are_marked_in_any_language() {
        use Token::*;
        assert_eq!(kinds("/cart?token=${token}&x=1", Language::Plain), vec![("/cart?token=", Plain), ("${token}", Variable), ("&x=1", Plain)]);
        assert_eq!(kinds(r#""id": "${user_id}""#, Language::Json), vec![("\"id\"", Key), (":", Punct), (" ", Plain), ("\"", String), ("${user_id}", Variable), ("\"", String)]);
        assert_eq!(kinds("${__P(a,${b})}", Language::Plain), vec![("${__P(a,${b})}", Variable)], "a nested reference is one");
        assert_eq!(kinds("${open", Language::Plain), vec![("${open", Plain)], "an unfinished one is plain");
        assert_eq!(kinds("$ {no}", Language::Plain), vec![("$ {no}", Plain)]);
        assert_eq!(kinds("<!-- ${x} -->", Language::Xml), vec![("<!-- ${x} -->", Comment)], "not inside a comment");
        assert_eq!(kinds("<a b=\"${c}\"/>", Language::Xml).iter().filter(|(_, t)| *t == Variable).count(), 1);
    }

    #[test]
    fn every_line_is_covered_without_gaps_whatever_it_holds() {
        let samples = [
            "", " ", "\t", "{", "}", "\"", "\\", "\"\\", "<", "</", "<!--", "<a b=", "<a b=\"", "<a b='c", "${", "${}", "}${", "<<<>>>", "{{{{", ":::",
            "caf\u{e9} \"\u{1F600}\" <\u{e9}\u{e9}>", "<\u{e9}", "\"\u{e9}", "-", "--", "1e", "-e-", "<a\u{301}b c\u{301}='d\u{301}'/>",
        ];
        for s in samples {
            for lang in [Language::Plain, Language::Json, Language::Xml] {
                assert_covers(s, lang);
            }
        }
    }

    #[test]
    fn random_lines_never_break_the_rules() {
        let alphabet: Vec<char> = "{}[],:\"'\\<>/=!?-$ \t01.eEtrufalsn\u{e9}\u{301}\u{1F600}abc".chars().collect();
        let mut seed = 0x9E3779B97F4A7C15u64;
        for _ in 0..3000 {
            let len = {
                seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                (seed >> 33) as usize % 60
            };
            let line: String = (0..len)
                .map(|_| {
                    seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                    alphabet[(seed >> 33) as usize % alphabet.len()]
                })
                .collect();
            for lang in [Language::Plain, Language::Json, Language::Xml] {
                assert_covers(&line, lang);
            }
        }
    }

    #[test]
    fn a_very_long_line_is_not_coloured_and_is_quick() {
        let long = format!("{{\"k\": \"{}\"}}", "x".repeat(MAX_COLOURED_LINE * 5));
        let t = std::time::Instant::now();
        let toks = tokens(&long, Language::Json);
        assert_eq!(toks, vec![(0..long.len(), Token::Plain)]);
        assert!(t.elapsed() < std::time::Duration::from_millis(200));
        // Just under the limit is coloured, and fast.
        let near = format!("{{\"k\": \"{}\"}}", "x".repeat(MAX_COLOURED_LINE - 20));
        let t = std::time::Instant::now();
        assert!(tokens(&near, Language::Json).len() > 1);
        assert!(t.elapsed() < std::time::Duration::from_millis(500), "{:?}", t.elapsed());
    }

    #[test]
    fn many_variables_in_a_line_are_quick() {
        let line = "${a}".repeat(4000);
        let t = std::time::Instant::now();
        let toks = tokens(&line, Language::Plain);
        assert_eq!(toks.len(), 1, "neighbouring references merge into one run");
        assert!(t.elapsed() < std::time::Duration::from_millis(500), "{:?}", t.elapsed());
    }
}
