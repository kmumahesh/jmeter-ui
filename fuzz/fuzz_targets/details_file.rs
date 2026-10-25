#![no_main]
//! Any bytes as the file of request details: reading never crashes or hangs, the reader stops at its limits however
//! long the input is, and whatever is read can be laid out as text.

use jvm_runtime::details::{DetailsTail, MAX_ITEMS};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let path = std::env::temp_dir().join(format!("jmeter-ui-fuzz-{}.jsonl", std::process::id()));
    if std::fs::write(&path, data).is_err() {
        return;
    }
    let mut tail = DetailsTail::new();
    let mut all = Vec::new();
    for _ in 0..100 {
        if let Ok(mut items) = tail.poll(&path) {
            all.append(&mut items);
        }
        if !tail.has_more() {
            break;
        }
    }
    assert!(all.len() <= MAX_ITEMS);
    for d in all.iter().take(20) {
        let _ = d.request_line();
        let _ = d.request_body();
        let _ = d.request_text();
        let _ = d.response_text();
    }
});
