//! Tests for the XML layer: exact round trips, attribute and text editing, and malformed input.

use jmx_model::xml::{Document, Element, Node, escape, parse_fragment, unescape};

fn round_trip(src: &str) {
    let doc = Document::parse(src).unwrap_or_else(|e| panic!("{e:#}\n{src}"));
    assert_eq!(doc.write(), src);
}

#[test]
fn unchanged_documents_are_written_back_exactly() {
    round_trip("<a/>");
    round_trip("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<a b='1'  c=\"2\" >\n  <b/>\n  text &amp; more &#xd; &lt;\n</a>\n");
    round_trip("<a><!-- note --><![CDATA[ <raw> ]]><?pi data?></a>");
    round_trip("<!DOCTYPE a>\n<a>x</a>");
    round_trip("<a>\r\n<b />\r\n</a>");
    round_trip("<root>\u{e9}\u{1F600}</root>");
}

#[test]
fn attributes_are_decoded_on_read() {
    let d = Document::parse(r#"<e a="x &amp; y" b='q&quot;r' c="&#65;&#x42;"/>"#).unwrap();
    assert_eq!(d.root.attr("a").as_deref(), Some("x & y"));
    assert_eq!(d.root.attr("b").as_deref(), Some("q\"r"));
    assert_eq!(d.root.attr("c").as_deref(), Some("AB"));
    assert_eq!(d.root.attr("missing"), None);
}

#[test]
fn editing_an_attribute_rewrites_only_that_start_tag() {
    let src = "<a x=\"1\">\n  <b y='keep'   z=\"2\"/>\n</a>";
    let mut d = Document::parse(src).unwrap();
    d.root.set_attr("x", "2 & 3");
    let out = d.write();
    assert_eq!(out, "<a x=\"2 &amp; 3\">\n  <b y='keep'   z=\"2\"/>\n</a>", "the child keeps its odd spacing");
    let again = Document::parse(&out).unwrap();
    assert_eq!(again.root.attr("x").as_deref(), Some("2 & 3"));
}

#[test]
fn adding_a_new_attribute() {
    let mut d = Document::parse("<a/>").unwrap();
    d.root.set_attr("n", "v");
    assert_eq!(d.write(), "<a n=\"v\"/>");
    d.root.set_attr("n", "w\"q");
    assert_eq!(d.write(), "<a n=\"w&quot;q\"/>");
}

#[test]
fn set_text_escapes_and_expands_self_closing_elements() {
    let mut d = Document::parse("<a><b/></a>").unwrap();
    let Node::Element(b) = &mut d.root.children[0] else { panic!() };
    b.set_text("1 < 2 & 3 > 2");
    assert_eq!(d.write(), "<a><b>1 &lt; 2 &amp; 3 &gt; 2</b></a>");
    let Node::Element(b) = &d.root.children[0] else { panic!() };
    assert_eq!(b.text(), "1 < 2 & 3 > 2");
}

#[test]
fn text_reading_joins_runs_and_decodes() {
    let d = Document::parse("<a>one &amp; two &#33;</a>").unwrap();
    assert_eq!(d.root.text(), "one & two !");
}

#[test]
fn pushing_children_into_a_self_closing_element() {
    let mut d = Document::parse("<root><hashTree/></root>").unwrap();
    let Node::Element(h) = &mut d.root.children[0] else { panic!() };
    h.push(Element::new("x"));
    let out = d.write();
    assert_eq!(out, "<root><hashTree><x></x></hashTree></root>");
    assert!(Document::parse(&out).is_ok());
}

#[test]
fn walking_visits_every_element_depth_first() {
    let mut d = Document::parse("<a><b><c/></b><d/></a>").unwrap();
    let mut seen = vec![];
    d.root.walk_mut(&mut |e| seen.push(e.name.clone()));
    assert_eq!(seen, ["a", "b", "c", "d"]);
}

#[test]
fn elements_skips_text_and_comments() {
    let d = Document::parse("<a>t<!--c--><b/>u<c/></a>").unwrap();
    assert_eq!(d.root.elements().map(|e| e.name.as_str()).collect::<Vec<_>>(), ["b", "c"]);
}

#[test]
fn malformed_input_is_an_error_not_a_panic() {
    for bad in ["", "<a>", "<a></b>", "<a/><b/>", "text only", "<a x=1/>", "<a><b></a></b>", "</a>"] {
        assert!(Document::parse(bad).is_err(), "{bad:?} should not parse");
    }
}

#[test]
fn escape_and_unescape_are_inverse() {
    for s in ["plain", "a & b", "<tag attr=\"x\">", "&amp;already", "ü ✓ 😀"] {
        assert_eq!(unescape(&escape(s, true)), s);
        assert_eq!(unescape(&escape(s, false)), s);
    }
    assert_eq!(escape("\"", false), "\"", "quotes are only escaped in attributes");
    assert_eq!(escape("\"", true), "&quot;");
}

#[test]
fn unknown_entities_are_left_alone() {
    assert_eq!(unescape("&nbsp; &foo; & alone"), "&nbsp; &foo; & alone");
    assert_eq!(unescape("&#xZZ;"), "&#xZZ;");
}

#[test]
fn deeply_nested_documents_work() {
    let depth = 500;
    let src = format!("{}{}", "<a>".repeat(depth), "</a>".repeat(depth));
    round_trip(&src);
}

#[test]
fn fragments_parse_to_one_element() {
    let e = parse_fragment("<ResultCollector><stringProp name=\"filename\">C:\\x\\y.xml</stringProp></ResultCollector>").unwrap();
    assert_eq!(e.name, "ResultCollector");
    assert_eq!(e.elements().next().unwrap().text(), "C:\\x\\y.xml");
}
