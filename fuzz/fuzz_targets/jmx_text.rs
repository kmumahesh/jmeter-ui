#![no_main]
//! Any text as an XML fragment, an attribute value or element text: parsing never crashes, unescaping accepts
//! anything, and escaping then unescaping gives the text back, apart from the characters XML 1.0 cannot hold, which
//! are never written.

use jmx_model::xml::{escape, parse_fragment, unescape};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let text = String::from_utf8_lossy(data);
    let _ = parse_fragment(&text);
    let _ = unescape(&text);
    let kept: String = text.chars().filter(|&c| jmx_model::xml::is_xml_char(c)).collect();
    assert_eq!(unescape(&escape(&text, false)), kept);
    assert_eq!(unescape(&escape(&text, true)), kept);
});
