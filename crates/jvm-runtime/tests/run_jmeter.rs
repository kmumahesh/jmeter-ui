//! Starting, watching and stopping real JMeter runs, and refusing what must not reach the command line.
//! Tests that need Java and JMeter skip themselves when they are not installed.

mod common;

use common::*;
use jvm_runtime::live::LiveStats;
use jvm_runtime::run::{self, RunSpec};
use std::net::{IpAddr, UdpSocket};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

// ---- what may reach the command line (no JMeter needed: only files that exist are checked) ----

fn fake(name: &str) -> (PathBuf, RunSpec) {
    let d = temp(name);
    let java = d.join(if cfg!(windows) { "java.exe" } else { "java" });
    std::fs::write(&java, "").unwrap();
    std::fs::create_dir_all(d.join("jmeter").join("bin")).unwrap();
    std::fs::write(d.join("jmeter").join("bin").join("ApacheJMeter.jar"), "").unwrap();
    let plan = d.join("plan.jmx");
    std::fs::write(&plan, "<x/>").unwrap();
    let spec = RunSpec { java, jmeter_home: d.join("jmeter"), plan, run_dir: d.join("run"), jvm_args: vec!["-Xmx512m".into()], props: vec![("jmeter.save.saveservice.autoflush".into(), "true".into())], generate_report: false };
    (d, spec)
}

#[test]
fn a_normal_run_is_accepted() {
    let (d, spec) = fake("v-ok");
    run::validate(&spec).unwrap();
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn a_script_is_refused_as_the_java_program() {
    for ext in ["bat", "cmd", "ps1", "vbs", "BAT"] {
        let (d, mut spec) = fake("v-script");
        let script = d.join(format!("java.{ext}"));
        std::fs::write(&script, "").unwrap();
        spec.java = script;
        let err = run::validate(&spec).unwrap_err().to_string();
        assert!(err.contains("not a Java program"), "{ext}: {err}");
        let _ = std::fs::remove_dir_all(d);
    }
}

#[test]
fn property_names_that_could_add_options_or_commands_are_refused() {
    for name in ["", "a b", "x;calc", "x&calc", "x|y", "x\"y", "a=b", "$(cmd)", "%PATH%", "x\ny", "a/b", "\u{e9}t\u{e9}", &"n".repeat(300)] {
        let (d, mut spec) = fake("v-name");
        spec.props.push((name.into(), "1".into()));
        assert!(run::validate(&spec).is_err(), "property name {name:?} was accepted");
        let _ = std::fs::remove_dir_all(d);
    }
    for name in ["jmeter.save.saveservice.autoflush", "my_prop-1", "a.b.C"] {
        assert!(run::valid_property_name(name), "{name}");
    }
}

#[test]
fn property_values_and_paths_with_control_characters_are_refused() {
    for value in ["a\nb", "a\rb", "a\0b", "a\tb"] {
        let (d, mut spec) = fake("v-value");
        spec.props.push(("ok.name".into(), value.into()));
        assert!(run::validate(&spec).is_err(), "value {value:?} was accepted");
        let _ = std::fs::remove_dir_all(d);
    }
    let (d, mut spec) = fake("v-path");
    spec.run_dir = d.join("run\nname");
    assert!(run::validate(&spec).is_err());
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn only_memory_and_define_options_are_passed_to_java() {
    for arg in ["-jar", "-jar evil.jar", "-javaagent:evil.jar", "-cp", "-Xbootclasspath/a:x", "-agentlib:jdwp=transport=dt_socket", "-Xmx1g\n-jar", "evil", "", "--add-opens=a/b=ALL-UNNAMED"] {
        let (d, mut spec) = fake("v-java-arg");
        spec.jvm_args.push(arg.into());
        assert!(run::validate(&spec).is_err(), "Java option {arg:?} was accepted");
        let _ = std::fs::remove_dir_all(d);
    }
    for arg in ["-Xmx2g", "-Xms64m", "-XX:+UseG1GC", "-Dfile.encoding=UTF-8"] {
        let (d, mut spec) = fake("v-java-ok");
        spec.jvm_args.push(arg.into());
        run::validate(&spec).unwrap();
        let _ = std::fs::remove_dir_all(d);
    }
}

#[test]
fn missing_pieces_are_named_in_the_message() {
    let (d, mut spec) = fake("v-missing");
    spec.plan = d.join("gone.jmx");
    assert!(run::validate(&spec).unwrap_err().to_string().contains("does not exist"));
    let (d2, mut spec2) = fake("v-missing2");
    spec2.jmeter_home = d2.join("nowhere");
    assert!(run::validate(&spec2).unwrap_err().to_string().contains("ApacheJMeter.jar"));
    let (d3, mut spec3) = fake("v-missing3");
    spec3.java = d3.join("no-java.exe");
    assert!(run::validate(&spec3).unwrap_err().to_string().contains("Java was not found"));
    for x in [d, d2, d3] {
        let _ = std::fs::remove_dir_all(x);
    }
}

#[test]
fn starting_with_a_refused_spec_starts_nothing() {
    let (d, mut spec) = fake("v-nostart");
    spec.props.push(("bad name".into(), "1".into()));
    assert!(run::start(&spec).is_err());
    assert!(!spec.run_dir.exists(), "nothing was made for a refused run");
    let _ = std::fs::remove_dir_all(d);
}

// ---- real runs ----

#[test]
fn a_real_run_reports_every_sample_and_ends_cleanly() {
    let Some(env) = env() else { return };
    let server = Server::start();
    let d = temp("real");
    let plan_path = d.join("shop.jmx");
    std::fs::write(&plan_path, plan(server.port, 2, 5, 0, &[request("home", "/"), request("search", "/search"), request("broken", "/fail")], "")).unwrap();
    let run_dir = d.join("run");
    let started = Instant::now();
    let mut h = run::start(&spec(&env, &plan_path, &run_dir)).unwrap();
    let seen = collect(&mut h, Duration::from_secs(90));
    let took = started.elapsed();
    eprintln!("a 30-request run took {took:?} in all");

    assert_eq!(seen.exit, Some(Some(0)), "lines: {:?}", seen.lines.iter().rev().take(5).collect::<Vec<_>>());
    assert_eq!(seen.stop_port, Some(h.stop_port()), "the run told us which port stops it");
    assert_eq!(seen.samples.len(), 30);
    assert_eq!(server.requests.load(std::sync::atomic::Ordering::Relaxed), 30);
    let mut live = LiveStats::new();
    live.add_all(&seen.samples);
    assert_eq!(live.total().count, 30);
    assert_eq!(live.total().errors, 10, "the broken request failed in every iteration");
    assert_eq!(live.label("broken").unwrap().errors, 10);
    assert_eq!(live.label("home").unwrap().errors, 0);
    assert!(run_dir.join("results.jtl").is_file() && run_dir.join("jmeter.log").is_file());
    assert_eq!(std::fs::read_dir(&d).unwrap().count(), 2, "only the plan and the run folder: the run left nothing next to the plan");
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn folders_and_plans_with_spaces_quotes_and_symbols_still_run() {
    let Some(env) = env() else { return };
    let server = Server::start();
    let d = temp("symbols");
    let weird = d.join("a b & c %PATH% ^caret 'q' (1) [2] ;x !y $z #h @at ~t + = , \u{e9}");
    std::fs::create_dir_all(&weird).unwrap();
    let plan_path = weird.join("plan & more %TEMP%.jmx");
    std::fs::write(&plan_path, plan(server.port, 1, 2, 0, &[request("home", "/")], "")).unwrap();
    let run_dir = weird.join("runs").join("run (1) & x");
    let mut h = run::start(&spec(&env, &plan_path, &run_dir)).unwrap();
    let seen = collect(&mut h, Duration::from_secs(90));
    assert_eq!(seen.exit, Some(Some(0)), "{:?}", seen.lines.iter().rev().take(5).collect::<Vec<_>>());
    assert_eq!(seen.samples.len(), 2);
    assert!(run_dir.join("results.jtl").is_file());
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn paths_with_letters_outside_the_system_code_page_get_a_warning() {
    assert!(run::path_warning(Path::new("C:/plain/folder/plan.jmx")).is_none());
    assert!(run::path_warning(Path::new("C:/with spaces & (symbols) \u{e9}/plan.jmx")).is_none());
    if cfg!(windows) {
        let w = run::path_warning(Path::new("C:/\u{4e2d}\u{6587}/plan.jmx")).expect("a warning");
        assert!(w.contains("JMeter"), "{w}");
    }
}

#[test]
fn a_plan_whose_name_looks_like_an_option_is_still_a_plan() {
    let Some(env) = env() else { return };
    let server = Server::start();
    let d = temp("dashname");
    let plan_path = d.join("-n.jmx");
    std::fs::write(&plan_path, plan(server.port, 1, 1, 0, &[request("home", "/")], "")).unwrap();
    // A relative path, as it would be if the user typed it.
    let cwd = std::env::current_dir().unwrap();
    std::env::set_current_dir(&d).unwrap();
    let mut h = run::start(&spec(&env, Path::new("-n.jmx"), Path::new("out"))).unwrap();
    std::env::set_current_dir(cwd).unwrap();
    let seen = collect(&mut h, Duration::from_secs(90));
    assert_eq!(seen.samples.len(), 1, "{:?}", seen.lines.iter().rev().take(5).collect::<Vec<_>>());
    let _ = std::fs::remove_dir_all(d);
}

fn forever(env: &Env, name: &str) -> (Server, PathBuf, jvm_runtime::run::RunHandle) {
    let server = Server::start();
    let d = temp(name);
    let plan_path = d.join("long.jmx");
    std::fs::write(&plan_path, plan(server.port, 1, -1, 100, &[request("home", "/")], "")).unwrap();
    let h = run::start(&spec(env, &plan_path, &d.join("run"))).unwrap();
    (server, d, h)
}

fn wait_running(h: &mut jvm_runtime::run::RunHandle, samples: usize) -> bool {
    let mut got = 0;
    wait_for(Duration::from_secs(60), || {
        while let Ok(ev) = h.events.try_recv() {
            if let jvm_runtime::run::RunEvent::Samples(s) = ev {
                got += s.len();
            }
        }
        got >= samples
    })
}

#[test]
fn stop_lets_a_run_that_never_ends_finish_by_itself() {
    let Some(env) = env() else { return };
    let (_server, d, mut h) = forever(&env, "stop");
    assert!(wait_running(&mut h, 3), "the run never produced samples");
    let t = Instant::now();
    assert!(h.stop_and_wait(false, Duration::from_secs(30)).unwrap(), "still running after a graceful stop");
    eprintln!("graceful stop took {:?}", t.elapsed());
    assert!(!h.is_running());
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn stop_now_ends_a_run_quickly() {
    let Some(env) = env() else { return };
    let (_server, d, mut h) = forever(&env, "stopnow");
    assert!(wait_running(&mut h, 3));
    let t = Instant::now();
    assert!(h.stop_and_wait(true, Duration::from_secs(30)).unwrap());
    eprintln!("stop now took {:?}", t.elapsed());
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn kill_ends_a_run_that_is_not_listening() {
    let Some(env) = env() else { return };
    let (_server, d, mut h) = forever(&env, "kill");
    assert!(wait_running(&mut h, 1));
    h.kill().unwrap();
    assert!(h.wait(Duration::from_secs(20)).unwrap().is_some());
    assert!(!process_alive(h.pid()), "the process is gone");
    h.kill().unwrap(); // killing again is not an error
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn dropping_the_handle_never_leaves_jmeter_running() {
    let Some(env) = env() else { return };
    let (_server, d, mut h) = forever(&env, "drop");
    assert!(wait_running(&mut h, 1));
    let pid = h.pid();
    assert!(process_alive(pid));
    drop(h);
    assert!(wait_for(Duration::from_secs(15), || !process_alive(pid)), "JMeter {pid} is still running after the handle was dropped");
    let _ = std::fs::remove_dir_all(d);
}

fn lan_ip() -> Option<IpAddr> {
    // Connecting a UDP socket sends nothing; it only picks the address that would be used.
    let s = UdpSocket::bind("0.0.0.0:0").ok()?;
    s.connect("192.0.2.1:9").ok()?;
    let ip = s.local_addr().ok()?.ip();
    (!ip.is_loopback() && !ip.is_unspecified()).then_some(ip)
}

#[test]
fn the_stop_port_answers_this_machine_only() {
    let Some(env) = env() else { return };
    let Some(ip) = lan_ip() else {
        eprintln!("skipped: this machine has no network address other than loopback");
        return;
    };
    let (_server, d, mut h) = forever(&env, "stopport");
    assert!(wait_running(&mut h, 3));
    // A message sent to the machine's own network address, as another computer would send it.
    let sock = UdpSocket::bind("0.0.0.0:0").unwrap();
    for msg in [&b"StopTestNow"[..], b"Shutdown"] {
        sock.send_to(msg, (ip, h.stop_port())).unwrap();
    }
    std::thread::sleep(Duration::from_secs(4));
    let still_running = h.is_running();
    // Then the real way: from this machine.
    let stopped = h.stop_and_wait(true, Duration::from_secs(30)).unwrap();
    assert!(still_running, "JMeter obeyed a stop message that came in on {ip}: anyone on the network could stop the run");
    assert!(stopped);
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn each_run_gets_its_own_stop_port() {
    let ports: std::collections::HashSet<u16> = (0..20).map(|_| run::free_udp_port().unwrap()).collect();
    assert!(ports.len() >= 15, "ports repeat too often: {ports:?}");
    assert!(ports.iter().all(|&p| p > 1000), "JMeter only uses ports above 1000");
}

#[test]
fn a_flood_of_output_is_bounded_and_does_not_block_the_run() {
    let Some(env) = env() else { return };
    let server = Server::start();
    let d = temp("flood");
    let script = r#"        <JSR223Sampler guiclass="TestBeanGUI" testclass="JSR223Sampler" testname="flood" enabled="true">
          <stringProp name="scriptLanguage">groovy</stringProp>
          <stringProp name="parameters"></stringProp>
          <stringProp name="filename"></stringProp>
          <stringProp name="cacheKey">true</stringProp>
          <stringProp name="script">(1..150000).each { i -> System.out.println("flood line " + i + " " + ("x" * 80)) }
System.out.println("y" * 3000000)
System.err.println("e" * 100)
SampleResult.setSuccessful(true)</stringProp>
        </JSR223Sampler>
        <hashTree/>
"#;
    let plan_path = d.join("flood.jmx");
    std::fs::write(&plan_path, plan(server.port, 1, 1, 0, &[request("home", "/")], script)).unwrap();
    let mut h = run::start(&spec(&env, &plan_path, &d.join("run"))).unwrap();
    // Do not take any events for a while, as a stalled window would.
    assert!(wait_for(Duration::from_secs(90), || h.dropped_lines() > 0 || !h.is_running()), "nothing was dropped and the run is still going");
    // What waited while nobody was reading: at most the limit, each line cut to the limit.
    let mut held = Vec::new();
    let mut samples = 0;
    while let Ok(ev) = h.events.try_recv() {
        match ev {
            jvm_runtime::run::RunEvent::Line(l) => held.push(l),
            jvm_runtime::run::RunEvent::Samples(s) => samples += s.len(),
            _ => {}
        }
    }
    eprintln!("{} lines were waiting, {} dropped so far", held.len(), h.dropped_lines());
    assert!(held.len() <= run::MAX_PENDING_LINES + 2, "{} lines were held", held.len());
    assert!(h.dropped_lines() > 0);
    assert!(held.iter().all(|l| l.chars().count() <= run::MAX_LINE_CHARS), "a long line was not cut");
    // The run is not held up by output nobody read, and still reports both samples.
    let t = Instant::now();
    let seen = collect(&mut h, Duration::from_secs(120));
    eprintln!("flood run finished {:?} later; {} lines dropped in all", t.elapsed(), h.dropped_lines());
    assert_eq!(seen.exit, Some(Some(0)), "the run was blocked by output nobody read");
    assert_eq!(samples + seen.samples.len(), 2, "both samples were still reported");
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn a_run_with_a_bad_plan_reports_the_failure() {
    let Some(env) = env() else { return };
    let d = temp("badplan");
    let plan_path = d.join("bad.jmx");
    std::fs::write(&plan_path, "<html>this is not a plan</html>").unwrap();
    let mut h = run::start(&spec(&env, &plan_path, &d.join("run"))).unwrap();
    let seen = collect(&mut h, Duration::from_secs(60));
    assert_ne!(seen.exit, Some(Some(0)), "a plan JMeter cannot read must not look like a success");
    assert!(seen.samples.is_empty());
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn time_from_start_to_the_first_sample_is_measured() {
    let Some(env) = env() else { return };
    let server = Server::start();
    let d = temp("firstsample");
    let plan_path = d.join("p.jmx");
    std::fs::write(&plan_path, plan(server.port, 1, 1, 0, &[request("home", "/")], "")).unwrap();
    let t = Instant::now();
    let h = run::start(&spec(&env, &plan_path, &d.join("run"))).unwrap();
    let mut first = None;
    wait_for(Duration::from_secs(60), || {
        while let Ok(ev) = h.events.try_recv() {
            if matches!(ev, jvm_runtime::run::RunEvent::Samples(_)) && first.is_none() {
                first = Some(t.elapsed());
            }
        }
        first.is_some()
    });
    eprintln!("first sample after {first:?}");
    assert!(first.is_some_and(|f| f < Duration::from_secs(45)), "{first:?}");
    let _ = std::fs::remove_dir_all(d);
}

// ---- JMeter must not outlive the app, even when the app crashes ----

/// Not a real test: started by `jmeter_is_ended_when_the_app_crashes` in a separate process, which starts a
/// run, writes the process id of JMeter to the file named in `JM_HELPER_PIDFILE` and then crashes without
/// any clean up.
#[test]
fn helper_start_a_run_and_crash() {
    let Some(pidfile) = std::env::var_os("JM_HELPER_PIDFILE") else { return };
    let env = env().expect("Java and JMeter");
    let (_server, _d, mut h) = forever(&env, "crash-helper");
    assert!(wait_running(&mut h, 2));
    std::fs::write(pidfile, h.pid().to_string()).unwrap();
    std::process::abort();
}

#[test]
fn jmeter_is_ended_when_the_app_crashes() {
    if env().is_none() {
        return;
    }
    let dir = temp("crash");
    let pidfile = dir.join("pid.txt");
    let exe = std::env::current_exe().unwrap();
    // No pipes to the helper: JMeter must not be able to keep anything of ours open.
    let status = std::process::Command::new(exe)
        .args(["--exact", "helper_start_a_run_and_crash", "--test-threads=1"])
        .env("JM_HELPER_PIDFILE", &pidfile)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .unwrap();
    let pid: u32 = std::fs::read_to_string(&pidfile).unwrap_or_else(|_| panic!("the helper did not start a run")).trim().parse().unwrap();
    assert!(!status.success(), "the helper was meant to crash");
    // On Windows the job object ends it; elsewhere the system does not promise this.
    if cfg!(windows) {
        let gone = wait_for(Duration::from_secs(15), || !process_alive(pid));
        if !gone {
            let _ = std::process::Command::new("taskkill").args(["/F", "/PID", &pid.to_string()]).output();
        }
        assert!(gone, "JMeter {pid} is still running after the app crashed");
    } else if process_alive(pid) {
        // Known limit: off Windows nothing ends JMeter when the app crashes. Do not leave it running after the test.
        let _ = std::process::Command::new("kill").args(["-9", &pid.to_string()]).status();
    }
    let _ = std::fs::remove_dir_all(dir);
}
