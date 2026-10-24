//! Validate and Send once plans, run in real JMeter against a local server. Skipped without Java and JMeter.

#[path = "../../jvm-runtime/tests/common/mod.rs"]
mod common;

use common::*;
use jvm_runtime::run;
use plan_core::EditablePlan;
use plan_core::derive;
use std::sync::atomic::Ordering;
use std::time::Duration;

/// A plan that would take long to run in full: 20 users, 50 loops, a one second timer, two requests, a
/// request nested in a Loop Controller, and a CSV file named relative to the plan.
fn big_plan(port: u16) -> String {
    let extra = r#"        <CSVDataSet guiclass="TestBeanGUI" testclass="CSVDataSet" testname="Users file" enabled="true">
          <stringProp name="filename">users.csv</stringProp>
          <stringProp name="variableNames">user</stringProp>
          <stringProp name="delimiter">,</stringProp>
          <boolProp name="quotedData">false</boolProp>
          <boolProp name="recycle">true</boolProp>
          <boolProp name="stopThread">false</boolProp>
          <stringProp name="shareMode">shareMode.all</stringProp>
        </CSVDataSet>
        <hashTree/>
        <LoopController guiclass="LoopControlPanel" testclass="LoopController" testname="Repeat" enabled="true">
          <stringProp name="LoopController.loops">5</stringProp>
        </LoopController>
        <hashTree>
          <HTTPSamplerProxy guiclass="HttpTestSampleGui" testclass="HTTPSamplerProxy" testname="Nested" enabled="true">
            <stringProp name="HTTPSampler.domain">127.0.0.1</stringProp>
            <stringProp name="HTTPSampler.port">PORT</stringProp>
            <stringProp name="HTTPSampler.protocol">http</stringProp>
            <stringProp name="HTTPSampler.path">/nested/${user}</stringProp>
            <stringProp name="HTTPSampler.method">GET</stringProp>
          </HTTPSamplerProxy>
          <hashTree/>
        </hashTree>
"#
    .replace("PORT", &port.to_string());
    plan(port, 20, 50, 1000, &[request("Home", "/"), request("Search", "/search")], &extra)
}

fn run_derived(env: &Env, text: &str, dir: &std::path::Path, name: &str) -> (Seen, std::path::PathBuf) {
    let run_dir = dir.join(name);
    std::fs::create_dir_all(&run_dir).unwrap();
    let plan_path = run_dir.join("plan.jmx");
    std::fs::write(&plan_path, text).unwrap();
    let mut h = run::start(&spec(env, &plan_path, &run_dir)).unwrap();
    (collect(&mut h, Duration::from_secs(120)), run_dir)
}

#[test]
fn validate_runs_every_sampler_once_with_one_user_and_no_waiting() {
    let Some(env) = env() else { return };
    let server = Server::start();
    let d = temp("validate");
    std::fs::write(d.join("users.csv"), "alice\nbob\n").unwrap();
    let e = EditablePlan::open(&big_plan(server.port)).unwrap();
    let derived = derive::validate_plan(&e, &d).unwrap();
    let started = std::time::Instant::now();
    let (seen, run_dir) = run_derived(&env, &derived.text, &d, "v");
    eprintln!("validate of a plan that takes 50 minutes in full: {:?}", started.elapsed());
    assert_eq!(seen.exit, Some(Some(0)), "{:?}", seen.lines.iter().rev().take(4).collect::<Vec<_>>());
    // Home, Search, then the nested request five times (the Loop Controller still loops).
    assert_eq!(seen.samples.len(), 2 + 5, "{:?}", seen.samples.iter().map(|s| &s.label).collect::<Vec<_>>());
    assert_eq!(server.requests.load(Ordering::Relaxed), 7);
    let _ = run_dir;
    let paths = server.paths.lock().unwrap().clone();
    let nested = paths.iter().filter(|p| p.starts_with("/nested/alice") || p.starts_with("/nested/bob")).count();
    assert_eq!(nested, 5, "the data file next to the original plan was read: {paths:?}");
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn send_once_sends_exactly_one_request() {
    let Some(env) = env() else { return };
    let server = Server::start();
    let d = temp("sendonce");
    std::fs::write(d.join("users.csv"), "alice\n").unwrap();
    let e = EditablePlan::open(&big_plan(server.port)).unwrap();
    let nested = e.plan().find_by_name("Nested").unwrap();
    let derived = derive::send_once_plan(&e, nested, &d).unwrap();
    let started = std::time::Instant::now();
    let (seen, run_dir) = run_derived(&env, &derived.text, &d, "s");
    eprintln!("send once: {:?}", started.elapsed());
    assert_eq!(seen.exit, Some(Some(0)));
    assert_eq!(seen.samples.len(), 1, "even though it sits in a Loop Controller that loops five times");
    assert_eq!(seen.samples[0].label, "Nested");
    assert_eq!(server.requests.load(Ordering::Relaxed), 1);
    let _ = run_dir;
    assert_eq!(server.paths.lock().unwrap().as_slice(), ["/nested/alice"], "the data file next to the original plan was read");
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn send_once_of_each_request_in_a_plan_works() {
    let Some(env) = env() else { return };
    let server = Server::start();
    let d = temp("sendeach");
    let e = EditablePlan::open(&plan(server.port, 3, 3, 500, &[request("A", "/a"), request("B", "/fail"), request("C", "/c")], "")).unwrap();
    for (name, ok) in [("A", true), ("B", false), ("C", true)] {
        let id = e.plan().find_by_name(name).unwrap();
        let derived = derive::send_once_plan(&e, id, &d).unwrap();
        let (seen, _) = run_derived(&env, &derived.text, &d, name);
        assert_eq!(seen.samples.len(), 1, "{name}");
        assert_eq!((seen.samples[0].label.as_str(), seen.samples[0].success), (name, ok), "{name}");
    }
    assert_eq!(server.requests.load(Ordering::Relaxed), 3);
    let _ = std::fs::remove_dir_all(d);
}
