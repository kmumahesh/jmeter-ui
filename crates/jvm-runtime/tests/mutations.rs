//! Damaged results files and console lines: whatever JMeter (or someone else) leaves in a results file, reading it
//! must never crash, hang or run away with memory, and a file written in pieces must read the same as a whole one.

#[path = "../../jmx-model/tests/mutate/mod.rs"]
mod mutate;

use jvm_runtime::results::{JtlTail, parse_stop_port, parse_summary};
use mutate::{Rng, mutate};
use std::io::Write;
use std::path::PathBuf;
use std::time::{Duration, Instant};

fn seed_jtl() -> Vec<u8> {
    let mut s = String::from("timeStamp,elapsed,label,responseCode,responseMessage,threadName,success,bytes,allThreads\n");
    for i in 0..60u64 {
        s.push_str(&format!(
            "{},{},{},200,OK,\"Group 1-{}\",{},{},3\n",
            1_700_000_000_000 + i * 100,
            20 + i % 9,
            if i % 2 == 0 { "Home" } else { "Search, \"quoted\"" },
            i % 5,
            i % 7 != 0,
            100 + i
        ));
    }
    s.into_bytes()
}

fn temp(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("jmeter-ui-mut-{name}-{}.jtl", std::process::id()))
}

fn read_all(path: &std::path::Path) -> (usize, u64) {
    let mut tail = JtlTail::new();
    let mut samples = 0;
    for _ in 0..100 {
        samples += tail.poll(path).expect("reading never fails on a readable file").len();
        if !tail.has_more() {
            break;
        }
    }
    (samples, tail.skipped())
}

#[test]
fn damaged_results_files_never_crash_the_reader() {
    let started = Instant::now();
    let seed = seed_jtl();
    let path = temp("whole");
    let mut rng = Rng(0xE703_7ED1_A0B4_28DB);
    let (mut with_samples, mut with_skips) = (0, 0);
    for _ in 0..500 {
        std::fs::write(&path, mutate(&mut rng, &seed)).unwrap();
        let (samples, skipped) = read_all(&path);
        assert!(samples <= 60 + 300, "damage cannot make up many more rows than the file had lines");
        with_samples += (samples > 0) as u32;
        with_skips += (skipped > 0) as u32;
    }
    let _ = std::fs::remove_file(&path);
    eprintln!("{with_samples} read some samples, {with_skips} had damaged rows skipped, in {:?}", started.elapsed());
    assert!(with_samples > 0 && with_skips > 0);
    assert!(started.elapsed() < Duration::from_secs(60));
}

#[test]
fn a_file_written_in_pieces_reads_the_same_as_the_whole_file() {
    let seed = seed_jtl();
    let whole = temp("pieces-whole");
    std::fs::write(&whole, &seed).unwrap();
    let expected = read_all(&whole);
    let mut rng = Rng(0x8EBC_6AF0_9C88_C6E3);
    let part = temp("pieces-part");
    for _ in 0..40 {
        let _ = std::fs::remove_file(&part);
        let mut tail = JtlTail::new();
        let mut f = std::fs::OpenOptions::new().create(true).append(true).open(&part).unwrap();
        let (mut samples, mut at) = (0, 0);
        while at < seed.len() {
            let end = (at + 1 + rng.below(300)).min(seed.len());
            f.write_all(&seed[at..end]).unwrap();
            f.flush().unwrap();
            at = end;
            samples += tail.poll(&part).unwrap().len();
        }
        assert_eq!((samples, tail.skipped()), expected, "pieces of any size give the same rows");
    }
    let _ = std::fs::remove_file(&whole);
    let _ = std::fs::remove_file(&part);
}

#[test]
fn damaged_console_lines_never_crash_the_parsers() {
    let lines = [
        "summary +    120 in 00:00:01 =  118.4/s Avg:    12 Min:     3 Max:    90 Err:     2 (1.67%) Active: 5 Started: 5 Finished: 0",
        "summary =   1200 in 00:00:10 =  120.0/s Avg:    11 Min:     2 Max:   120 Err:     0 (0.00%)",
        "Waiting for possible Shutdown/StopTestNow/HeapDump/ThreadDump message on port 4445",
    ];
    let mut rng = Rng(0x589_965C_C759_1A37);
    for i in 0..5000 {
        let text = String::from_utf8_lossy(&mutate(&mut rng, lines[i % 3].as_bytes())).into_owned();
        let _ = parse_summary(&text);
        let _ = parse_stop_port(&text);
    }
}
