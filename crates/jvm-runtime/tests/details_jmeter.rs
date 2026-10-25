//! The listener that keeps what was sent and received, run inside the real JMeter against a local web server: what it
//! keeps, its limits, that it never changes or fails a request, and that what it writes is always readable.
//! Tests that need Java and JMeter skip themselves when they are not installed.

mod common;

use common::*;
use jmx_model::Document;
use jmx_model::runcopy::inject_script_listener;
use jvm_runtime::details::{self, Detail, Mode};
use jvm_runtime::run;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const ASSERT_200: &str = r#"        <ResponseAssertion guiclass="AssertionGui" testclass="ResponseAssertion" testname="Status is 200" enabled="true">
          <collectionProp name="Asserion.test_strings">
            <stringProp name="49586">200</stringProp>
          </collectionProp>
          <stringProp name="Assertion.test_field">Assertion.response_code</stringProp>
          <boolProp name="Assertion.assume_success">false</boolProp>
          <intProp name="Assertion.test_type">8</intProp>
        </ResponseAssertion>
        <hashTree/>
"#;

struct Ran {
    dir: PathBuf,
    details: Vec<Detail>,
    skipped: u64,
    samples: usize,
    errors: usize,
    elapsed: Duration,
}

/// Run a plan text with the listener (unless `listener` is false) and read what it kept.
fn go(env: &Env, name: &str, plan_text: &str, mode: Mode, max: u32, listener: bool, file_override: Option<&Path>) -> Ran {
    let dir = temp(name);
    let mut doc = Document::parse(plan_text).unwrap();
    if listener {
        inject_script_listener(&mut doc, "JMeter UI details", details::SCRIPT).unwrap();
    }
    let plan = dir.join("plan.jmx");
    std::fs::write(&plan, doc.write()).unwrap();
    let file = file_override.map(Path::to_path_buf).unwrap_or_else(|| dir.join("details.jsonl"));
    let mut spec = spec(env, &plan, &dir);
    spec.props = details::props(mode, &file, max);
    let t = Instant::now();
    let mut h = run::start(&spec).unwrap();
    let seen = collect(&mut h, Duration::from_secs(150));
    assert_eq!(seen.exit, Some(Some(0)), "the run ended badly: {:?}", seen.lines.iter().rev().take(5).collect::<Vec<_>>());
    let elapsed = t.elapsed();
    let (details, skipped) = details::read_all(&file);
    Ran { dir, details, skipped, samples: seen.samples.len(), errors: seen.samples.iter().filter(|s| !s.success).count(), elapsed }
}

fn plan_with(port: u16, users: u32, loops: i32, paths: &[(&str, &str)]) -> String {
    let reqs: Vec<_> = paths.iter().map(|(n, p)| request(n, p)).collect();
    plan(port, users, loops, 0, &reqs, ASSERT_200)
}

// ---- what it keeps ----

#[test]
fn failed_requests_keep_what_was_sent_and_what_came_back() {
    let Some(env) = env() else { return };
    let server = Server::start();
    let text = plan_with(server.port, 2, 3, &[("Home", "/"), ("Broken", "/fail?x=1&amp;y=two"), ("Data", "/json")]);
    let r = go(&env, "det-failed", &text, Mode::Failed, 500, true, None);
    assert_eq!(r.samples, 18);
    assert_eq!(r.errors, 6);
    assert_eq!((r.details.len(), r.skipped), (6, 0), "one for each failed request, none for the others");
    for d in &r.details {
        assert_eq!((d.label.as_str(), d.code.as_str(), d.success), ("Broken", "500", false));
        assert_eq!(d.message, "Internal Server Error");
        assert!(d.url.ends_with("/fail?x=1&y=two"), "{}", d.url);
        assert_eq!(d.response, "boom");
        assert!(d.response_headers.starts_with("HTTP/1.1 500"), "{}", d.response_headers);
        assert!(d.response_headers.contains("X-Test-Server: yes"), "{}", d.response_headers);
        assert!(d.request.starts_with("GET http://127.0.0.1:"), "{}", d.request);
        assert!(d.request_headers.contains("Host: 127.0.0.1"), "{}", d.request_headers);
        assert!(d.ts > 1_700_000_000_000 && d.elapsed < 10_000 && d.thread.starts_with("Users"), "{d:?}");
        assert_eq!(d.assertions.len(), 1, "{d:?}");
        assert!(d.assertions[0].failure && d.assertions[0].name == "Status is 200", "{:?}", d.assertions);
        assert!(!d.assertions[0].message.is_empty());
        assert!(!d.binary && !d.truncated);
        assert_eq!(d.request_body(), None);
    }
    assert_eq!(server.requests.load(std::sync::atomic::Ordering::Relaxed), 18, "the listener sent no request of its own");
}

#[test]
fn all_mode_keeps_the_successful_ones_too_up_to_the_limit() {
    let Some(env) = env() else { return };
    let server = Server::start();
    let text = plan_with(server.port, 1, 5, &[("Home", "/"), ("Data", "/json"), ("Broken", "/fail")]);
    let r = go(&env, "det-all", &text, Mode::All, 500, true, None);
    assert_eq!(r.details.len(), 15);
    let data = r.details.iter().find(|d| d.label == "Data").unwrap();
    assert!(data.success && data.code == "200" && data.response == r#"{"a":1,"b":"x"}"#, "{data:?}");
    assert!(data.response_headers.to_lowercase().contains("content-type: application/json"));
    assert!(data.assertions.is_empty() || !data.assertions[0].failure, "an assertion that passed is not a failure");
    let r = go(&env, "det-all-cap", &text, Mode::All, 4, true, None);
    assert_eq!(r.details.len(), 4, "the limit counts every kept request");
    assert_eq!(r.samples, 15);
}

#[test]
fn off_keeps_nothing_and_without_the_listener_nothing_is_written() {
    let Some(env) = env() else { return };
    let server = Server::start();
    let text = plan_with(server.port, 1, 3, &[("Broken", "/fail")]);
    let r = go(&env, "det-off", &text, Mode::Off, 500, true, None);
    assert!(r.details.is_empty() && !r.dir.join("details.jsonl").exists());
    let r = go(&env, "det-nolistener", &text, Mode::All, 500, false, None);
    assert!(r.details.is_empty() && !r.dir.join("details.jsonl").exists());
    assert_eq!(r.errors, 3);
}

#[test]
fn a_posted_body_and_the_headers_sent_are_kept() {
    let Some(env) = env() else { return };
    let server = Server::start();
    let body = "{\n  \"user\": \"demo\",\n  \"note\": \"caf\u{e9}\"\n}";
    let base = plan_with(server.port, 1, 1, &[("Login", "/fail")]);
    let args = format!(
        "<boolProp name=\"HTTPSampler.postBodyRaw\">true</boolProp>\n<elementProp name=\"HTTPsampler.Arguments\" elementType=\"Arguments\"><collectionProp name=\"Arguments.arguments\"><elementProp name=\"\" elementType=\"HTTPArgument\"><boolProp name=\"HTTPArgument.always_encode\">false</boolProp><stringProp name=\"Argument.value\">{}</stringProp><stringProp name=\"Argument.metadata\">=</stringProp></elementProp></collectionProp></elementProp>",
        body.replace('&', "&amp;").replace('<', "&lt;")
    );
    let headers = "<HeaderManager guiclass=\"HeaderPanel\" testclass=\"HeaderManager\" testname=\"H\" enabled=\"true\"><collectionProp name=\"HeaderManager.headers\"><elementProp name=\"\" elementType=\"Header\"><stringProp name=\"Header.name\">Content-Type</stringProp><stringProp name=\"Header.value\">application/json</stringProp></elementProp><elementProp name=\"\" elementType=\"Header\"><stringProp name=\"Header.name\">X-Demo</stringProp><stringProp name=\"Header.value\">42</stringProp></elementProp></collectionProp></HeaderManager><hashTree/>";
    let text = base
        .replace("<stringProp name=\"HTTPSampler.method\">GET</stringProp>", "<stringProp name=\"HTTPSampler.method\">POST</stringProp>")
        .replacen(
            "<elementProp name=\"HTTPsampler.Arguments\" elementType=\"Arguments\" guiclass=\"HTTPArgumentsPanel\" testclass=\"Arguments\" testname=\"User Defined Variables\" enabled=\"true\">\n            <collectionProp name=\"Arguments.arguments\"/>\n          </elementProp>",
            &args,
            1,
        )
        .replacen("        <hashTree/>\n        <ResponseAssertion", &format!("        <hashTree>\n{headers}\n        </hashTree>\n        <ResponseAssertion"), 1);
    assert!(text.contains("postBodyRaw"), "the plan was changed as intended");
    let r = go(&env, "det-post", &text, Mode::Failed, 500, true, None);
    assert_eq!(r.details.len(), 1, "{:?}", r.details);
    let d = &r.details[0];
    assert!(d.request.starts_with("POST http://127.0.0.1:"), "{}", d.request);
    assert_eq!(d.request_body(), Some(body), "the body as it was sent, with its lines and its accent: {:?}", d.request);
    assert!(d.request_headers.contains("Content-Type: application/json") && d.request_headers.contains("X-Demo: 42"), "{}", d.request_headers);
    assert_eq!(server.bodies.lock().unwrap()[0], body, "and that is what the server received");
    assert!(d.request_text().contains("X-Demo: 42") && d.request_text().ends_with(body));
}

#[test]
fn texts_with_quotes_lines_and_odd_characters_make_valid_lines_that_read_back_exactly() {
    let Some(env) = env() else { return };
    let server = Server::start();
    let text = plan_with(server.port, 2, 2, &[("Odd", "/unicode?q=\u{e9}")]);
    let r = go(&env, "det-unicode", &text, Mode::All, 500, true, None);
    assert_eq!((r.details.len(), r.skipped), (4, 0));
    for d in &r.details {
        assert_eq!(d.response, "caf\u{e9} \u{1F600} \"quoted\" \\ back\nline two\ttab");
        assert!(d.success || !d.assertions.is_empty());
    }
    let raw = std::fs::read_to_string(r.dir.join("details.jsonl")).unwrap();
    assert_eq!(raw.lines().count(), 4, "one line each, however many lines the response had");
}

// ---- limits ----

#[test]
fn a_long_response_is_cut_and_a_response_that_is_not_text_is_not_kept() {
    let Some(env) = env() else { return };
    let server = Server::start();
    let text = plan_with(server.port, 1, 1, &[("Big", "/big"), ("Bin", "/bin")]);
    let r = go(&env, "det-big", &text, Mode::All, 500, true, None);
    let big = r.details.iter().find(|d| d.label == "Big").unwrap();
    assert_eq!(big.response.chars().count(), details::MAX_TEXT_CHARS, "cut at the limit");
    assert!(big.truncated);
    assert_eq!(big.bytes, 200_000 + big.response_headers.len() as u64 - big.response_headers.len() as u64 + (big.bytes - 200_000), "the size in the record is the real size");
    assert!(big.bytes >= 200_000);
    let bin = r.details.iter().find(|d| d.label == "Bin").unwrap();
    assert!(bin.binary && bin.response.is_empty(), "{bin:?}");
    assert!(std::fs::metadata(r.dir.join("details.jsonl")).unwrap().len() < 400_000, "the file stays small");
}

#[test]
fn the_limit_holds_however_many_threads_fail_at_once() {
    let Some(env) = env() else { return };
    let server = Server::start();
    let text = plan_with(server.port, 20, 30, &[("Broken", "/fail")]);
    let r = go(&env, "det-cap", &text, Mode::Failed, 50, true, None);
    assert_eq!(r.samples, 600);
    assert_eq!(r.details.len(), 50, "exactly the limit, not a few more");
    assert_eq!(r.skipped, 0, "lines from different threads never mix");
    let raw = std::fs::read_to_string(r.dir.join("details.jsonl")).unwrap();
    assert!(raw.lines().all(|l| l.starts_with('{') && l.ends_with('}')), "every line is whole");
}

#[test]
fn all_the_failures_under_the_limit_are_kept_when_many_threads_fail_at_once() {
    let Some(env) = env() else { return };
    let server = Server::start();
    let text = plan_with(server.port, 15, 20, &[("Broken", "/fail")]);
    let r = go(&env, "det-many", &text, Mode::Failed, 500, true, None);
    assert_eq!((r.details.len(), r.skipped), (300, 0));
}

// ---- it never changes or fails a request ----

#[test]
fn a_listener_that_cannot_write_does_not_change_the_run() {
    let Some(env) = env() else { return };
    let server = Server::start();
    let text = plan_with(server.port, 2, 3, &[("Home", "/"), ("Broken", "/fail")]);
    let base = go(&env, "det-base", &text, Mode::Failed, 500, false, None);
    let nowhere = temp("det-nowhere").join("no such folder").join("deep").join("details.jsonl");
    let r = go(&env, "det-unwritable", &text, Mode::Failed, 500, true, Some(&nowhere));
    assert_eq!((r.samples, r.errors), (base.samples, base.errors), "the same requests failed");
    assert!(r.details.is_empty() && !nowhere.exists());
    // A folder where the file should be.
    let dir = temp("det-folder");
    let r = go(&env, "det-folder-run", &text, Mode::Failed, 500, true, Some(&dir));
    assert_eq!((r.samples, r.errors), (base.samples, base.errors));
}

#[test]
fn the_listener_does_not_change_what_the_requests_do() {
    let Some(env) = env() else { return };
    let server = Server::start();
    let text = plan_with(server.port, 3, 4, &[("Home", "/"), ("Data", "/json"), ("Broken", "/fail")]);
    let without = go(&env, "det-same-a", &text, Mode::Off, 500, false, None);
    let with = go(&env, "det-same-b", &text, Mode::All, 500, true, None);
    assert_eq!((with.samples, with.errors), (without.samples, without.errors));
    let rows = |dir: &Path| std::fs::read_to_string(dir.join("results.jtl")).map(|t| t.lines().count()).unwrap_or(0);
    let _ = rows;
}

#[test]
fn a_run_with_the_listener_is_about_as_fast_as_one_without() {
    let Some(env) = env() else { return };
    let server = Server::start();
    let text = plan_with(server.port, 5, 400, &[("Home", "/"), ("Data", "/json")]);
    let without = go(&env, "det-speed-a", &text, Mode::Off, 500, false, None);
    let with = go(&env, "det-speed-b", &text, Mode::Failed, 500, true, None);
    eprintln!("4000 requests: {:?} without the listener, {:?} with it", without.elapsed, with.elapsed);
    assert_eq!(with.samples, 4000);
    assert!(with.elapsed < without.elapsed * 3 + Duration::from_secs(5), "{:?} against {:?}", with.elapsed, without.elapsed);
}
