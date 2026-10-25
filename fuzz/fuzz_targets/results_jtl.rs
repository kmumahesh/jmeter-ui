#![no_main]
//! Any bytes as a results file or a console line: reading never crashes or hangs, and the file reader stops at its
//! limits however long the input is.

use jvm_runtime::results::{JtlTail, parse_stop_port, parse_summary};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let path = std::env::temp_dir().join(format!("jmeter-ui-fuzz-{}.jtl", std::process::id()));
    if std::fs::write(&path, data).is_ok() {
        let mut tail = JtlTail::new();
        for _ in 0..100 {
            let _ = tail.poll(&path);
            if !tail.has_more() {
                break;
            }
        }
    }
    let text = String::from_utf8_lossy(data);
    for line in text.lines().take(50) {
        let _ = parse_summary(line);
        let _ = parse_stop_port(line);
    }
});
