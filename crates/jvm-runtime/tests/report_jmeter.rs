//! The HTML report made by JMeter's own generator: from a results file of this app and from a hand-made one, in folders
//! with odd names, and what happens when the file or the folder is wrong. Tests that need Java and JMeter skip themselves
//! when they are not installed.

mod common;

use common::*;
use jvm_runtime::report;
use jvm_runtime::run;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// Tests that count the folders left behind must not overlap with the others, so they take turns.
static TURN: std::sync::Mutex<()> = std::sync::Mutex::new(());

const HEADER: &str = "timeStamp,elapsed,label,responseCode,responseMessage,threadName,dataType,success,failureMessage,bytes,sentBytes,grpThreads,allThreads,URL,Latency,IdleTime,Connect";

/// A results file with `rows` rows in the shape JMeter writes.
fn results_file(dir: &Path, name: &str, rows: u64) -> PathBuf {
    let mut text = String::from(HEADER);
    text.push('\n');
    for i in 0..rows {
        let ok = i % 10 != 0;
        text.push_str(&format!(
            "{},{},{},{},{},Users 1-{},text,{},{},{},100,5,5,http://127.0.0.1/x,{},0,1\n",
            1_700_000_000_000u64 + i * 20,
            5 + (i % 50),
            if i % 2 == 0 { "Home" } else { "Search" },
            if ok { "200" } else { "500" },
            if ok { "OK" } else { "Server Error" },
            i % 5,
            ok,
            if ok { "" } else { "boom" },
            120,
            3 + (i % 20)
        ));
    }
    let p = dir.join(name);
    std::fs::write(&p, text).unwrap();
    p
}

fn make(env: &Env, results: &Path, out: &Path) -> Result<PathBuf, String> {
    report::generate(&env.java.path, &env.home, results, out, Duration::from_secs(240))
}

fn leftover_work_dirs() -> usize {
    let prefix = format!("jmeter-ui-report-{}-", std::process::id());
    std::fs::read_dir(std::env::temp_dir()).unwrap().flatten().filter(|e| e.file_name().to_string_lossy().starts_with(&prefix)).count()
}

#[test]
fn a_results_file_becomes_a_report_that_opens_in_a_browser() {
    let Some(env) = env() else { return };
    let _turn = TURN.lock().unwrap_or_else(|e| e.into_inner());
    let dir = temp("rep-basic");
    let jtl = results_file(&dir, "results.jtl", 500);
    let out = report::default_folder(&jtl);
    assert!(report::existing(&out).is_none());
    let t = Instant::now();
    let index = make(&env, &jtl, &out).unwrap();
    eprintln!("a report of 500 rows: {:?}", t.elapsed());
    assert_eq!(index, out.join("index.html"));
    assert_eq!(report::existing(&out), Some(index.clone()));
    let page = std::fs::read_to_string(&index).unwrap();
    assert!(page.contains("Dashboard"), "{}", &page[..page.len().min(300)]);
    assert!(out.join("content").is_dir() && out.join("sbadmin2-1.0.7").is_dir());
    assert!(jtl.is_file(), "the results are left as they were");
    assert_eq!(leftover_work_dirs(), 0, "the folder the generator worked in is removed");
}

#[test]
fn the_results_of_a_real_run_of_this_app_make_a_report() {
    let Some(env) = env() else { return };
    let _turn = TURN.lock().unwrap_or_else(|e| e.into_inner());
    let server = Server::start();
    let dir = temp("rep-real");
    let plan_text = plan(server.port, 2, 5, 0, &[request("Home", "/"), request("Broken", "/fail")], "");
    let plan_file = dir.join("plan.jmx");
    std::fs::write(&plan_file, plan_text).unwrap();
    let mut h = run::start(&spec(&env, &plan_file, &dir)).unwrap();
    let seen = collect(&mut h, Duration::from_secs(120));
    assert_eq!(seen.exit, Some(Some(0)));
    let out = dir.join("report");
    let index = make(&env, &dir.join("results.jtl"), &out).unwrap();
    assert!(index.is_file());
}

#[test]
fn folders_and_files_with_odd_names_work() {
    let Some(env) = env() else { return };
    let _turn = TURN.lock().unwrap_or_else(|e| e.into_inner());
    let base = temp("rep-odd");
    let odd = base.join("caf\u{e9} & 100% (1)");
    std::fs::create_dir_all(&odd).unwrap();
    let jtl = results_file(&odd, "r\u{e9}sultats & more.jtl", 50);
    let out = odd.join("my report; calc");
    let index = make(&env, &jtl, &out).unwrap();
    assert!(index.is_file());
}

#[test]
fn a_folder_that_already_has_files_is_refused_and_left_alone() {
    let Some(env) = env() else { return };
    let _turn = TURN.lock().unwrap_or_else(|e| e.into_inner());
    let dir = temp("rep-busy");
    let jtl = results_file(&dir, "results.jtl", 10);
    let out = dir.join("report");
    std::fs::create_dir_all(&out).unwrap();
    std::fs::write(out.join("mine.txt"), "keep me").unwrap();
    let e = make(&env, &jtl, &out).unwrap_err();
    assert!(e.contains("already has files"), "{e}");
    assert_eq!(std::fs::read_to_string(out.join("mine.txt")).unwrap(), "keep me");
    // An empty folder is fine.
    std::fs::remove_file(out.join("mine.txt")).unwrap();
    assert!(make(&env, &jtl, &out).is_ok());
}

#[test]
fn a_file_that_is_not_a_results_file_gives_a_sentence_and_leaves_no_half_report() {
    let Some(env) = env() else { return };
    let _turn = TURN.lock().unwrap_or_else(|e| e.into_inner());
    let dir = temp("rep-bad");
    for (name, text) in [("notes.txt", "this is not a results file\nat all\n".to_string()), ("empty.jtl", String::new()), ("header.jtl", format!("{HEADER}\n")), ("binary.jtl", "\u{0}\u{1}\u{2}".repeat(1000))] {
        let p = dir.join(name);
        std::fs::write(&p, text).unwrap();
        let out = dir.join(format!("{name}-report"));
        let t = Instant::now();
        let r = make(&env, &p, &out);
        assert!(t.elapsed() < Duration::from_secs(120));
        match r {
            Ok(index) => assert!(index.is_file(), "{name}"),
            Err(e) => {
                assert!(!e.is_empty(), "{name}");
                assert!(!out.exists(), "{name}: no half-made report is left behind");
            }
        }
    }
    let missing = make(&env, &dir.join("nothing.jtl"), &dir.join("x-report")).unwrap_err();
    assert!(missing.contains("is not a file"), "{missing}");
    assert_eq!(leftover_work_dirs(), 0);
}

#[test]
fn a_report_that_takes_too_long_is_stopped_and_removed() {
    let Some(env) = env() else { return };
    let _turn = TURN.lock().unwrap_or_else(|e| e.into_inner());
    let dir = temp("rep-slow");
    let jtl = results_file(&dir, "results.jtl", 100);
    let out = dir.join("report");
    let t = Instant::now();
    let e = report::generate(&env.java.path, &env.home, &jtl, &out, Duration::from_millis(300)).unwrap_err();
    assert!(e.contains("Stopped after 0.3 seconds"), "{e}");
    assert!(t.elapsed() < Duration::from_secs(20), "{:?}", t.elapsed());
    assert!(!out.exists());
    assert_eq!(leftover_work_dirs(), 0);
}

#[test]
fn a_large_results_file_makes_a_report_in_reasonable_time() {
    let Some(env) = env() else { return };
    let _turn = TURN.lock().unwrap_or_else(|e| e.into_inner());
    let dir = temp("rep-large");
    let jtl = results_file(&dir, "results.jtl", 200_000);
    let out = dir.join("report");
    let t = Instant::now();
    let index = make(&env, &jtl, &out).unwrap();
    let took = t.elapsed();
    eprintln!("a report of 200,000 rows ({} MB): {took:?}", std::fs::metadata(&jtl).unwrap().len() / (1024 * 1024));
    assert!(index.is_file());
    assert!(took < Duration::from_secs(180), "{took:?}");
}
