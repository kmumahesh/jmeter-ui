//! The results file of a run is read while JMeter writes it, and can also be a file from anywhere: it must
//! never use unbounded memory or time, and damaged parts must be skipped, not trusted.

use jvm_runtime::results::{JtlTail, MAX_READ_PER_POLL, MAX_RECORD_BYTES};
use std::io::Write;
use std::path::PathBuf;
use std::time::Instant;

const HEADER: &str = "timeStamp,elapsed,label,responseCode,responseMessage,threadName,dataType,success,failureMessage,bytes,sentBytes,grpThreads,allThreads,Latency,IdleTime,Connect\n";

fn dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("jmeter-ui-jtl-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn row(ts: u64, ms: u64, label: &str, ok: bool) -> String {
    format!("{ts},{ms},{label},{},{},T 1-1,text,{ok},,100,50,3,7,{},0,2\n", if ok { 200 } else { 500 }, if ok { "OK" } else { "Error" }, ms / 2)
}

/// Poll until the whole file is read, as the run does.
fn read_all(t: &mut JtlTail, p: &std::path::Path) -> Vec<jvm_runtime::results::Sample> {
    let mut all = Vec::new();
    for _ in 0..10_000 {
        all.extend(t.poll(p).unwrap());
        if !t.has_more() {
            break;
        }
    }
    all
}

#[test]
fn rows_are_read_with_every_column() {
    let d = dir("columns");
    let p = d.join("r.jtl");
    std::fs::write(&p, format!("{HEADER}{}{}", row(1000, 200, "login", true), row(2000, 40, "cart", false))).unwrap();
    let rows = read_all(&mut JtlTail::new(), &p);
    assert_eq!(rows.len(), 2);
    let r = &rows[0];
    assert_eq!(
        (r.timestamp_ms, r.elapsed_ms, r.label.as_str(), r.response_code.as_str(), r.success, r.bytes, r.group_threads, r.all_threads, r.latency_ms, r.connect_ms),
        (1000, 200, "login", "200", true, 100, 3, 7, 100, 2)
    );
    assert!(!rows[1].success && rows[1].response_message == "Error");
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn crlf_line_ends_and_quoted_fields_with_commas_quotes_and_line_breaks_are_read() {
    let d = dir("quoting");
    let p = d.join("r.jtl");
    let text = format!(
        "{}\r\n1,10,\"a,b \"\"q\"\"\nline two\",200,OK,T,text,true,,1,1,1,1,1,0,0\r\n2,20,plain,200,OK,T,text,true,,1,1,1,1,1,0,0\r\n",
        HEADER.trim_end()
    );
    std::fs::write(&p, text).unwrap();
    let rows = read_all(&mut JtlTail::new(), &p);
    assert_eq!(rows.len(), 2, "{rows:?}");
    assert_eq!(rows[0].label, "a,b \"q\"\nline two");
    assert_eq!(rows[1].label, "plain");
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn rows_written_in_pieces_are_found_when_complete() {
    let d = dir("pieces");
    let p = d.join("r.jtl");
    let mut t = JtlTail::new();
    std::fs::write(&p, HEADER).unwrap();
    assert!(t.poll(&p).unwrap().is_empty());
    let line = row(1, 10, "a", true);
    let (a, b) = line.split_at(20);
    let mut f = std::fs::OpenOptions::new().append(true).open(&p).unwrap();
    f.write_all(a.as_bytes()).unwrap();
    assert!(t.poll(&p).unwrap().is_empty(), "half a row is not a row");
    f.write_all(b.as_bytes()).unwrap();
    assert_eq!(t.poll(&p).unwrap().len(), 1);
    assert!(t.poll(&p).unwrap().is_empty(), "nothing is read twice");
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn a_missing_file_gives_no_rows_and_no_error() {
    let mut t = JtlTail::new();
    assert!(t.poll(std::path::Path::new("Z:/does/not/exist.jtl")).unwrap().is_empty());
}

#[test]
fn missing_columns_and_absurd_numbers_become_zero_not_errors() {
    let d = dir("absurd");
    let p = d.join("r.jtl");
    std::fs::write(&p, "label,elapsed,bytes\nonly,99999999999999999999999999,-5\nshort\n,,\n").unwrap();
    let rows = read_all(&mut JtlTail::new(), &p);
    assert_eq!(rows.len(), 3);
    assert_eq!((rows[0].label.as_str(), rows[0].elapsed_ms, rows[0].bytes, rows[0].timestamp_ms), ("only", 0, 0, 0));
    assert_eq!(rows[1].label, "short");
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn a_record_larger_than_the_limit_is_dropped_and_the_rows_after_it_are_kept() {
    let d = dir("giant");
    let p = d.join("r.jtl");
    let giant = "x".repeat(MAX_RECORD_BYTES + 10);
    std::fs::write(&p, format!("{HEADER}{}{giant}\n{}", row(1, 1, "before", true), row(2, 2, "after", true))).unwrap();
    let mut t = JtlTail::new();
    let rows = read_all(&mut t, &p);
    assert_eq!(rows.iter().map(|r| r.label.as_str()).collect::<Vec<_>>(), ["before", "after"]);
    assert_eq!(t.skipped(), 1);
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn a_line_that_never_ends_does_not_grow_memory() {
    let d = dir("noend");
    let p = d.join("r.jtl");
    let mut f = std::fs::File::create(&p).unwrap();
    f.write_all(HEADER.as_bytes()).unwrap();
    // Three times the limit with no line break at all.
    f.write_all("y".repeat(MAX_RECORD_BYTES * 3).as_bytes()).unwrap();
    drop(f);
    let mut t = JtlTail::new();
    assert!(read_all(&mut t, &p).is_empty());
    assert!(t.skipped() >= 1, "the endless line was dropped");
    // The file goes on normally afterwards.
    let mut f = std::fs::OpenOptions::new().append(true).open(&p).unwrap();
    f.write_all(format!("\n{}", row(5, 5, "fine", true)).as_bytes()).unwrap();
    let rows = read_all(&mut t, &p);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].label, "fine");
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn an_open_quote_that_never_closes_is_dropped_too() {
    let d = dir("openquote");
    let p = d.join("r.jtl");
    std::fs::write(&p, format!("{HEADER}1,1,\"never closed{}", "z".repeat(MAX_RECORD_BYTES * 2))).unwrap();
    let mut t = JtlTail::new();
    assert!(read_all(&mut t, &p).is_empty());
    assert!(t.skipped() >= 1);
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn text_that_is_not_a_results_file_gives_no_samples() {
    let d = dir("notjtl");
    let p = d.join("r.jtl");
    for text in ["<html><body>hello</body></html>
1,2,3
4,5,6
", "a,b,c
1,2,3
", "timeStamp,bytes
1,2
", "


"] {
        std::fs::write(&p, text).unwrap();
        let mut t = JtlTail::new();
        assert!(read_all(&mut t, &p).is_empty(), "{text:?}");
    }
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn binary_junk_is_survived() {
    let d = dir("junk");
    let p = d.join("r.jtl");
    let mut junk = Vec::new();
    let mut x = 0x2545_F491_4F6C_DD1Du64;
    for _ in 0..2_000_000 {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        junk.push(x as u8);
    }
    std::fs::write(&p, &junk).unwrap();
    let mut t = JtlTail::new();
    let rows = read_all(&mut t, &p);
    assert!(rows.is_empty(), "junk made {} rows", rows.len());
    assert!(t.skipped() > 0);
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn a_huge_file_is_read_in_bounded_steps() {
    let d = dir("sparse");
    let p = d.join("r.jtl");
    let f = std::fs::File::create(&p).unwrap();
    f.set_len(200 * 1024 * 1024).unwrap();
    drop(f);
    let started = Instant::now();
    let mut t = JtlTail::new();
    let mut polls = 0;
    loop {
        let rows = t.poll(&p).unwrap();
        assert!(rows.len() < 100);
        polls += 1;
        if !t.has_more() {
            break;
        }
    }
    eprintln!("200 MB read in {polls} steps of at most {} MB, {:?}", MAX_READ_PER_POLL / 1024 / 1024, started.elapsed());
    assert!(polls >= 200 * 1024 * 1024 / MAX_READ_PER_POLL);
    assert!(started.elapsed().as_secs() < 60);
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn text_with_accents_and_emoji_survives_the_read_limit_boundary() {
    let d = dir("utf8");
    let p = d.join("r.jtl");
    let mut text = String::from(HEADER);
    let mut n = 0u64;
    while text.len() < MAX_READ_PER_POLL + MAX_READ_PER_POLL / 2 {
        text.push_str(&row(n, 1, "caf\u{e9} \u{1F600} \u{4e2d}\u{6587}", true));
        n += 1;
    }
    std::fs::write(&p, &text).unwrap();
    let rows = read_all(&mut JtlTail::new(), &p);
    assert_eq!(rows.len() as u64, n);
    assert!(rows.iter().all(|r| r.label == "caf\u{e9} \u{1F600} \u{4e2d}\u{6587}"), "a character split by the read limit was damaged");
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn a_file_replaced_by_a_shorter_one_is_read_from_the_start() {
    let d = dir("replaced");
    let p = d.join("r.jtl");
    let mut t = JtlTail::new();
    std::fs::write(&p, format!("{HEADER}{}{}{}", row(1, 1, "a", true), row(2, 2, "b", true), row(3, 3, "c", true))).unwrap();
    assert_eq!(read_all(&mut t, &p).len(), 3);
    std::fs::write(&p, format!("{HEADER}{}", row(9, 9, "new", true))).unwrap();
    let rows = read_all(&mut t, &p);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].label, "new");
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn half_a_million_rows_are_read_quickly() {
    let d = dir("speed");
    let p = d.join("r.jtl");
    let mut text = String::from(HEADER);
    for i in 0..500_000u64 {
        text.push_str(&row(i, 5 + i % 300, ["login", "search", "cart", "pay"][(i % 4) as usize], i % 40 != 0));
    }
    std::fs::write(&p, &text).unwrap();
    let started = Instant::now();
    let rows = read_all(&mut JtlTail::new(), &p);
    let took = started.elapsed();
    eprintln!("500,000 rows ({} MB) read in {took:?}", text.len() / 1024 / 1024);
    assert_eq!(rows.len(), 500_000);
    let limit = if cfg!(debug_assertions) { 40 } else { 4 };
    assert!(took.as_secs() < limit, "took {took:?}");
    let _ = std::fs::remove_dir_all(d);
}
