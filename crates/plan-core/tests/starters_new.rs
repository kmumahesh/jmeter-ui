//! The templates for load testing: what each one holds, that it is a plan JMeter accepts and runs, and that what is
//! typed into a name can never become structure. Tests that need Java and JMeter skip themselves when they are not
//! installed.

use jvm_runtime::run::{self, RunEvent, RunSpec};
use plan_core::starters::{self, Starter};
use plan_core::{EditablePlan, NodeId, problems};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

fn open(s: Starter) -> EditablePlan {
    EditablePlan::open(&starters::new_plan_text(s, "T").unwrap()).unwrap()
}

fn find(e: &EditablePlan, name: &str) -> NodeId {
    e.plan().find_by_name(name).unwrap_or_else(|| panic!("no element called {name}"))
}

fn names(e: &EditablePlan) -> Vec<String> {
    e.plan().iter().map(|n| n.name.clone()).collect()
}

#[test]
fn the_login_template_keeps_the_token_and_sends_it() {
    let e = open(Starter::Login);
    assert_eq!(names(&e), ["T", "HTTP Request Defaults", "HTTP Header Manager", "Users", "Users file", "Log in", "Status is 200", "Keep the token", "Get my profile", "Status is 200", "Send the token", "Think time"]);
    let csv = find(&e, "Users file");
    assert_eq!((e.prop(csv, "filename").as_deref(), e.prop(csv, "variableNames").as_deref()), (Some("users.csv"), Some("username,password")));
    let login = find(&e, "Log in");
    assert_eq!(e.prop(login, "HTTPSampler.method").as_deref(), Some("POST"));
    assert_eq!(e.prop(login, "HTTPSampler.postBodyRaw").as_deref(), Some("true"));
    assert!(e.setting_texts(login).iter().any(|(_, v)| v.contains("${username}") && v.contains("${password}")));
    let x = find(&e, "Keep the token");
    assert_eq!((e.prop(x, "JSONPostProcessor.referenceNames").as_deref(), e.prop(x, "JSONPostProcessor.jsonPathExprs").as_deref()), (Some("token"), Some("$.token")));
    let h = find(&e, "Send the token");
    let row = e.row_cells(h, "HeaderManager.headers", 0);
    assert_eq!(row.iter().map(|(_, v)| v.as_str()).collect::<Vec<_>>(), ["Authorization", "Bearer ${token}"], "{row:?}");
}

#[test]
fn the_data_template_uses_a_term_from_each_row() {
    let e = open(Starter::Data);
    let csv = find(&e, "Search terms");
    assert_eq!(e.prop(csv, "variableNames").as_deref(), Some("term"));
    assert_eq!(e.prop(find(&e, "Search"), "HTTPSampler.path").as_deref(), Some("/search?q=${term}"));
}

#[test]
fn the_load_template_runs_for_ten_minutes_after_a_minute_of_ramp_up() {
    let e = open(Starter::Load);
    let g = find(&e, "Users");
    let p = |n: &str| e.prop(g, n);
    assert_eq!((p("ThreadGroup.num_threads").as_deref(), p("ThreadGroup.ramp_time").as_deref()), (Some("50"), Some("60")));
    assert_eq!((p("ThreadGroup.scheduler").as_deref(), p("ThreadGroup.duration").as_deref(), p("ThreadGroup.delay").as_deref()), (Some("true"), Some("600"), Some("0")));
    assert_eq!(p("ThreadGroup.main_controller/LoopController.loops").as_deref(), Some("-1"), "loops until the time is up");
}

#[test]
fn the_spike_template_adds_a_burst_after_a_delay() {
    let e = open(Starter::Spike);
    let (base, spike) = (find(&e, "Steady users"), find(&e, "Spike"));
    assert_eq!((e.prop(base, "ThreadGroup.num_threads").as_deref(), e.prop(base, "ThreadGroup.delay").as_deref(), e.prop(base, "ThreadGroup.duration").as_deref()), (Some("10"), Some("0"), Some("300")));
    assert_eq!((e.prop(spike, "ThreadGroup.num_threads").as_deref(), e.prop(spike, "ThreadGroup.delay").as_deref(), e.prop(spike, "ThreadGroup.duration").as_deref(), e.prop(spike, "ThreadGroup.ramp_time").as_deref()), (Some("200"), Some("120"), Some("60"), Some("5")));
}

#[test]
fn no_template_starts_with_a_warning_and_each_has_a_distinct_button_and_file_name() {
    let mut ids = std::collections::HashSet::new();
    let mut stems = std::collections::HashSet::new();
    for s in Starter::ALL {
        let e = open(s);
        let warnings = if s == Starter::Empty { Vec::new() } else { problems::check(e.plan()) };
        assert!(warnings.iter().all(|w| w.level != problems::Level::Warning), "{s:?} starts with a warning: {warnings:?}");
        assert!(ids.insert(s.button_id()), "{s:?}");
        assert!(stems.insert(s.file_stem()), "{s:?}");
        assert!(!s.description().is_empty() && !s.label().is_empty());
    }
    assert_eq!(ids.len(), Starter::ALL.len());
}

#[test]
fn a_name_with_markup_stays_text_in_every_template() {
    let evil = "</testname><evil/>&\"'";
    for s in Starter::ALL {
        let text = starters::new_plan_text(s, evil).unwrap();
        assert!(!text.contains("<evil/>"), "{s:?}");
        let e = EditablePlan::open(&text).unwrap();
        assert_eq!(e.plan().node(e.plan().roots()[0]).unwrap().name, evil);
        assert_eq!(e.to_text(), text);
    }
}

// ---- with the real JMeter ----

fn jmeter() -> Option<(PathBuf, PathBuf)> {
    let home = std::env::var_os("JMETER_HOME").map(PathBuf::from).or_else(|| {
        let h = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME"))?;
        Some(PathBuf::from(h).join("Downloads").join("apache-jmeter-5.6.3"))
    })?;
    if !home.join("bin").join("ApacheJMeter.jar").is_file() {
        eprintln!("skipped: no JMeter at {}", home.display());
        return None;
    }
    let java = jvm_runtime::locate::find_java(None).ok()?.path;
    Some((java, home))
}

/// Run a template against a local server that answers 200 to everything (after pointing it there and shortening it) and
/// return how many requests were made and how many failed.
fn run_template(s: Starter, java: &Path, home: &Path) -> (usize, usize) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    listener.set_nonblocking(true).unwrap();
    let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let stop2 = stop.clone();
    let server = std::thread::spawn(move || {
        use std::io::{Read, Write};
        while !stop2.load(std::sync::atomic::Ordering::Relaxed) {
            match listener.accept() {
                Ok((mut c, _)) => {
                    let _ = c.set_nonblocking(false);
                    let mut buf = [0u8; 4096];
                    let _ = c.read(&mut buf);
                    let body = r#"{"token":"abc123"}"#;
                    let _ = write!(c, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
                }
                Err(_) => std::thread::sleep(Duration::from_millis(5)),
            }
        }
    });
    let dir = std::env::temp_dir().join(format!("jmeter-ui-starter-{:?}-{}", s, std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("users.csv"), "ann,pw1\nbo,pw2\n").unwrap();
    std::fs::write(dir.join("terms.csv"), "shoes\nhats\n").unwrap();
    let mut e = open(s);
    for (id, prop, value) in [("HTTP Request Defaults", "HTTPSampler.protocol", "http".to_string()), ("HTTP Request Defaults", "HTTPSampler.domain", "127.0.0.1".to_string()), ("HTTP Request Defaults", "HTTPSampler.port", port.to_string())] {
        e.set_prop(find(&e, id), prop, "stringProp", &value).unwrap();
    }
    // Short and small: two users, one pass, short pauses.
    let groups: Vec<NodeId> = e.plan().iter().filter(|n| n.tag == "ThreadGroup").map(|n| n.id).collect();
    for g in groups {
        e.set_prop(g, "ThreadGroup.num_threads", "stringProp", "2").unwrap();
        e.set_prop(g, "ThreadGroup.ramp_time", "stringProp", "1").unwrap();
        e.set_prop(g, "ThreadGroup.scheduler", "boolProp", "false").unwrap();
        e.set_prop(g, "ThreadGroup.delay", "stringProp", "").unwrap();
        e.set_prop(g, "ThreadGroup.duration", "stringProp", "").unwrap();
        e.set_prop(g, "ThreadGroup.main_controller/LoopController.loops", "stringProp", "1").unwrap();
        e.set_prop(g, "ThreadGroup.main_controller/LoopController.continue_forever", "boolProp", "false").unwrap();
    }
    let timers: Vec<NodeId> = e.plan().iter().filter(|n| n.tag == "UniformRandomTimer").map(|n| n.id).collect();
    for t in timers {
        e.set_prop(t, "ConstantTimer.delay", "stringProp", "10").unwrap();
        e.set_prop(t, "RandomTimer.range", "stringProp", "10.0").unwrap();
    }
    let plan = dir.join("plan.jmx");
    std::fs::write(&plan, e.to_text()).unwrap();
    let spec = RunSpec { java: java.to_path_buf(), jmeter_home: home.to_path_buf(), plan, run_dir: dir.clone(), jvm_args: vec!["-Xmx256m".into()], props: vec![], generate_report: false };
    let mut h = run::start(&spec).unwrap();
    let start = Instant::now();
    let (mut total, mut failed, mut exit) = (0usize, 0usize, None);
    while start.elapsed() < Duration::from_secs(120) {
        match h.events.recv_timeout(Duration::from_millis(200)) {
            Ok(RunEvent::Samples(rows)) => {
                total += rows.len();
                failed += rows.iter().filter(|r| !r.success).count();
            }
            Ok(RunEvent::Exited { code }) => {
                exit = Some(code);
                break;
            }
            Ok(_) => {}
            Err(_) => {
                if !h.is_running() {
                    let _ = h.wait(Duration::from_secs(3));
                    while let Ok(ev) = h.events.try_recv() {
                        if let RunEvent::Samples(rows) = ev {
                            total += rows.len();
                            failed += rows.iter().filter(|r| !r.success).count();
                        }
                    }
                    exit = Some(Some(0));
                    break;
                }
            }
        }
    }
    stop.store(true, std::sync::atomic::Ordering::Relaxed);
    let _ = server.join();
    assert!(exit.is_some(), "{s:?} did not end");
    let _ = std::fs::remove_dir_all(&dir);
    (total, failed)
}

#[test]
fn every_template_is_a_plan_the_real_jmeter_runs_without_a_failed_request() {
    let Some((java, home)) = jmeter() else { return };
    for s in [Starter::Api, Starter::Web, Starter::Login, Starter::Data, Starter::Load, Starter::Spike] {
        let (total, failed) = run_template(s, &java, &home);
        eprintln!("{s:?}: {total} requests, {failed} failed");
        assert!(total >= 2, "{s:?} made {total} requests");
        assert_eq!(failed, 0, "{s:?}: every request answers 200 here, so a failure is the template's own");
    }
}
