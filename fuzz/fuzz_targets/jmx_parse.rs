#![no_main]
//! Any bytes as a `.jmx` file: the reader must return (never crash or hang), and a file it accepts must be saved
//! back exactly as it came.

use jmx_model::xml::Document;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let text = String::from_utf8_lossy(data);
    if let Ok(doc) = Document::parse(&text) {
        assert_eq!(doc.write(), text, "an accepted file is saved exactly as it came");
    }
});
