//! Hostile input: plans from outside are untrusted, so reading them must never hang, crash, read other
//! files or reach the network, and writing text back must never change the structure of the plan.

use jmx_model::xml::{Document, Node, escape, unescape};
use std::time::{Duration, Instant};

fn count(e: &jmx_model::Element) -> usize {
    1 + e.children.iter().filter_map(|c| if let Node::Element(x) = c { Some(count(x)) } else { None }).sum::<usize>()
}

#[test]
fn a_doctype_with_an_external_entity_is_kept_as_text_and_never_read() {
    let secret = std::env::temp_dir().join(format!("jmeter-ui-xxe-{}.txt", std::process::id()));
    std::fs::write(&secret, "TOP-SECRET-CONTENT").unwrap();
    let uri = format!("file:///{}", secret.display().to_string().replace('\\', "/"));
    let src = format!("<?xml version=\"1.0\"?>\n<!DOCTYPE p [<!ENTITY x SYSTEM \"{uri}\">]>\n<p a=\"&x;\">&x;</p>");
    let d = Document::parse(&src).unwrap();
    let out = d.write();
    assert_eq!(out, src, "the file is saved as it came");
    assert!(!out.contains("TOP-SECRET-CONTENT"));
    assert!(!d.root.text().contains("TOP-SECRET-CONTENT"));
    assert!(!d.root.attr("a").unwrap_or_default().contains("TOP-SECRET-CONTENT"));
    let _ = std::fs::remove_file(secret);
}

#[test]
fn an_entity_that_points_at_a_web_address_is_never_fetched() {
    // Nothing listens here; the parse must finish at once, whatever the entity says.
    let src = "<!DOCTYPE p [<!ENTITY x SYSTEM \"http://127.0.0.1:9/never\">]><p>&x;</p>";
    let t = Instant::now();
    Document::parse(src).unwrap();
    assert!(t.elapsed() < Duration::from_secs(2));
}

#[test]
fn entity_expansion_bombs_do_not_grow() {
    // Billion laughs: each level names the previous one ten times.
    let mut src = String::from("<!DOCTYPE lolz [<!ENTITY a0 \"lol\">");
    for i in 1..10 {
        src.push_str(&format!("<!ENTITY a{i} \"{}\">", format!("&a{};", i - 1).repeat(10)));
    }
    src.push_str("]><lolz a=\"&a9;\">&a9;</lolz>");
    let t = Instant::now();
    let d = Document::parse(&src).unwrap();
    assert!(t.elapsed() < Duration::from_secs(2));
    assert!(d.root.text().len() < 1000, "entities are not expanded");
    assert_eq!(d.write(), src);
}

#[test]
fn character_references_never_make_invalid_text() {
    // Out of range, surrogate and huge numbers are left alone, not turned into something else.
    assert_eq!(unescape("&#xD800;"), "&#xD800;");
    assert_eq!(unescape("&#1114112;"), "&#1114112;");
    assert_eq!(unescape("&#99999999999999999999;"), "&#99999999999999999999;");
    assert_eq!(unescape("&#x;&#;&;&"), "&#x;&#;&;&");
    assert_eq!(unescape("&amp;lt;"), "&lt;", "one pass only: no double decoding");
}

#[test]
fn deep_nesting_is_refused_with_an_error() {
    // A plan this deep would overflow the stack when walked or written.
    for depth in [5_000usize, 100_000] {
        let src = format!("{}{}", "<a>".repeat(depth), "</a>".repeat(depth));
        let err = Document::parse(&src).expect_err("too deep");
        assert!(format!("{err:#}").contains("nested"), "{err:#}");
    }
}

#[test]
fn nesting_deeper_than_any_real_plan_still_opens() {
    let depth = jmx_model::xml::MAX_DEPTH;
    let src = format!("{}x{}", "<a>".repeat(depth), "</a>".repeat(depth));
    let d = Document::parse(&src).unwrap();
    assert_eq!(count(&d.root), depth);
    assert_eq!(d.write(), src);
}

#[test]
fn a_very_wide_plan_opens_quickly() {
    let n = 200_000;
    let src = format!("<r>{}</r>", "<e a=\"1\">t</e>".repeat(n));
    let t = Instant::now();
    let d = Document::parse(&src).unwrap();
    assert_eq!(count(&d.root), n + 1);
    assert!(t.elapsed() < Duration::from_secs(10), "took {:?}", t.elapsed());
}

#[test]
fn a_very_long_text_value_round_trips() {
    let big = "x".repeat(5_000_000);
    let src = format!("<r><v>{big}</v></r>");
    let d = Document::parse(&src).unwrap();
    assert_eq!(d.write().len(), src.len());
}

#[test]
fn truncating_a_plan_at_any_point_gives_an_error_or_a_plan_never_a_panic() {
    let src = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../plan-core/tests/fixtures/Test.jmx")).unwrap();
    let mut cut = 0;
    while cut < src.len() {
        if src.is_char_boundary(cut) {
            let _ = Document::parse(&src[..cut]);
        }
        cut += 97;
    }
}

#[test]
fn damaged_documents_are_errors() {
    for bad in [
        "", "   ", "text only", "<a>", "<a></b>", "<a><b></a></b>", "</a>", "<a/><b/>", "<a b=1/>", "<a b=\"1\" b=\"2\"/>x<",
        "<a>&unknown;</a><", "<!-- never closed", "<![CDATA[ never closed", "<a b=\"unclosed/>", "<?xml version=\"1.0\"?>",
    ] {
        // Either an error or a plan, but never a panic and never a hang.
        let _ = Document::parse(bad);
    }
    for bad in ["", "<a>", "<a></b>", "</a>", "<a/><b/>", "<?xml version=\"1.0\"?>"] {
        assert!(Document::parse(bad).is_err(), "{bad:?} should not open");
    }
}

/// A small, seeded random number generator so that a failure can be repeated.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
}

#[test]
fn mutated_plans_never_panic_and_always_write_back_what_was_read() {
    let base = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../plan-core/tests/fixtures/Test.jmx")).unwrap();
    let bytes = base.as_bytes();
    let pieces: [&[u8]; 12] = [b"<", b">", b"&", b"\"", b"'", b"/", b"<!--", b"]]>", b"&#x0;", b"<![CDATA[", b"\xff\xfe", b"<a>"];
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    let (mut opened, mut refused) = (0, 0);
    for _ in 0..3000 {
        let mut b = bytes.to_vec();
        for _ in 0..1 + rng.below(6) {
            let at = rng.below(b.len());
            match rng.below(4) {
                0 => {
                    b.remove(at);
                }
                1 => {
                    let p = pieces[rng.below(pieces.len())];
                    b.splice(at..at, p.iter().copied());
                }
                2 => b[at] = rng.next() as u8,
                _ => {
                    let end = (at + rng.below(200)).min(b.len());
                    b.drain(at..end);
                }
            }
        }
        let Ok(text) = String::from_utf8(b) else { continue };
        match Document::parse(&text) {
            Ok(d) => {
                opened += 1;
                assert_eq!(d.write(), text, "a plan that opens is written back exactly");
            }
            Err(_) => refused += 1,
        }
    }
    assert!(opened > 0 && refused > 0, "opened {opened}, refused {refused}");
}

/// Text that tries to break out of the place it is written to.
const HOSTILE: &[&str] = &[
    "</stringProp><evil/>",
    "\"><evil a=\"",
    "' onload='x",
    "]]><![CDATA[",
    "<!-- c -->",
    "<?pi x?>",
    "a & b &amp; c &#38; d",
    "line one\r\nline two\ttab\nthree",
    "\u{0}\u{1}\u{7f}",
    "<![CDATA[x]]>",
    "&lt;already&gt;",
    "\u{e9}\u{1F600}\u{202e}rtl",
    "",
    "   leading and trailing   ",
];

#[test]
fn hostile_text_in_an_element_stays_text() {
    for h in HOSTILE {
        let mut d = Document::parse("<r><v>old</v></r>").unwrap();
        let Some(Node::Element(v)) = d.root.children.iter_mut().find(|c| matches!(c, Node::Element(_))) else { panic!() };
        v.set_text(h);
        let out = d.write();
        let again = Document::parse(&out).unwrap_or_else(|e| panic!("{h:?} gives broken XML: {e:#}\n{out}"));
        assert_eq!(count(&again.root), 2, "{h:?} added or removed elements:\n{out}");
        let Some(Node::Element(v2)) = again.root.children.iter().find(|c| matches!(c, Node::Element(_))) else { panic!() };
        assert_eq!(v2.text(), *h,"{h:?} did not come back as it went in:\n{out}");
    }
}

#[test]
fn hostile_text_in_an_attribute_stays_in_that_attribute() {
    for h in HOSTILE {
        let mut d = Document::parse("<r><v testname=\"old\" enabled=\"true\"/></r>").unwrap();
        let Some(Node::Element(v)) = d.root.children.iter_mut().find(|c| matches!(c, Node::Element(_))) else { panic!() };
        v.set_attr("testname", h);
        let out = d.write();
        let again = Document::parse(&out).unwrap_or_else(|e| panic!("{h:?} gives broken XML: {e:#}\n{out}"));
        assert_eq!(count(&again.root), 2, "{h:?}\n{out}");
        let Some(Node::Element(v2)) = again.root.children.iter().find(|c| matches!(c, Node::Element(_))) else { panic!() };
        assert_eq!(v2.attr("evil"), None, "{h:?} added an attribute:\n{out}");
        assert_eq!(v2.attr("testname").as_deref(), Some(*h), "{h:?}\n{out}");
        assert_eq!(v2.attr("enabled").as_deref(), Some("true"));
    }
}

#[test]
fn escaping_never_leaves_a_character_that_can_end_the_value() {
    for h in HOSTILE {
        let t = escape(h, false);
        assert!(!t.contains('<') && !t.contains('>'), "{t:?}");
        let a = escape(h, true);
        assert!(!a.contains('<') && !a.contains('>') && !a.contains('"') && !a.contains('\n'), "{a:?}");
        assert_eq!(unescape(&a), *h, "escape then unescape is the identity for {h:?}");
    }
}
