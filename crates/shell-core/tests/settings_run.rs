//! Settings added for running plans and starting up.

use shell_core::settings::{self, HEAP_RANGE_MB, SUMMARY_INTERVAL_RANGE, Settings, Startup};

fn temp(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("shell-core-run-{name}-{}-{:?}", std::process::id(), std::thread::current().id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d.join("settings.json")
}

#[test]
fn defaults_match_jmeters_own() {
    let s = Settings::default();
    assert_eq!(s.heap_mb, 1024, "JMeter starts with a 1 GB heap");
    assert_eq!(s.summary_interval_secs, 1);
    assert_eq!(s.startup, Startup::Welcome);
}

#[test]
fn values_round_trip() {
    let p = temp("roundtrip");
    let s = Settings { startup: Startup::LastPlan, heap_mb: 4096, summary_interval_secs: 5, ..Settings::default() };
    settings::save(&p, &s).unwrap();
    let back = settings::load(&p).settings;
    assert_eq!(back.startup, Startup::LastPlan);
    assert_eq!((back.heap_mb, back.summary_interval_secs), (4096, 5));
}

#[test]
fn startup_is_stored_as_a_word() {
    let p = temp("word");
    settings::save(&p, &Settings { startup: Startup::LastPlan, ..Settings::default() }).unwrap();
    assert!(std::fs::read_to_string(&p).unwrap().contains("\"startup\": \"lastplan\""));
}

#[test]
fn out_of_range_values_are_clamped() {
    let p = temp("clamp");
    std::fs::write(&p, r#"{ "heap_mb": 1, "summary_interval_secs": 100000 }"#).unwrap();
    let s = settings::load(&p).settings;
    assert_eq!(s.heap_mb, HEAP_RANGE_MB.0);
    assert_eq!(s.summary_interval_secs, SUMMARY_INTERVAL_RANGE.1);
    std::fs::write(&p, r#"{ "heap_mb": 999999999 }"#).unwrap();
    assert_eq!(settings::load(&p).settings.heap_mb, HEAP_RANGE_MB.1);
}

#[test]
fn an_unknown_startup_word_counts_as_damage() {
    let p = temp("badstartup");
    std::fs::write(&p, r#"{ "startup": "sometimes" }"#).unwrap();
    let l = settings::load(&p);
    assert!(l.notice.is_some());
    assert_eq!(l.settings.startup, Startup::Welcome);
}

#[test]
fn old_files_without_the_new_fields_get_defaults() {
    let p = temp("old");
    std::fs::write(&p, r#"{ "theme": "dark", "recent_files": ["a.jmx"] }"#).unwrap();
    let s = settings::load(&p).settings;
    assert_eq!(s.heap_mb, 1024);
    assert_eq!(s.recent_files, ["a.jmx"]);
}
