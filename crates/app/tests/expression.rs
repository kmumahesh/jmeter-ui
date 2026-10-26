//! Trying an expression from the window: the dialog, what it starts with, what it shows, and what must not happen.
//! Tests that run Java skip themselves when Java and JMeter are not installed.

mod common;
#[path = "../../jvm-runtime/tests/common/mod.rs"]
mod jm;

use common::*;
use gpui::TestAppContext;
use serde_json::Value;
use shell_core::actions::Action;
use shell_core::settings::{self, Settings};
use std::path::PathBuf;
use std::time::{Duration, Instant};

struct Setup {
    plan: PathBuf,
    settings: PathBuf,
}

fn setup(name: &str, home: Option<&std::path::Path>) -> Setup {
    let dir = temp_dir(name);
    let plan = dir.join("web.jmx");
    std::fs::copy(fixture(WEB), &plan).unwrap();
    let settings_path = dir.join("settings.json");
    settings::save(&settings_path, &Settings { jmeter_home: home.map(|h| h.to_string_lossy().into_owned()), ..Settings::default() }).unwrap();
    Setup { plan, settings: settings_path }
}

fn open<'a>(cx: &'a mut TestAppContext, s: &Setup, real: bool) -> App<'a> {
    let app = start(cx, Some(s.plan.clone()), Some(s.settings.clone()));
    let shell = app.shell.clone();
    let home = s.settings.clone();
    app.cx.update(|_, cx| {
        shell.update(cx, |sh, _| {
            sh.runtime = if real {
                let h = settings::load(&home).settings.jmeter_home.map(PathBuf::from);
                jmeter_ui::shell::check_runtime(None, h.as_deref())
            } else {
                jmeter_ui::shell::RuntimeStatus { java: Some("java".into()), jmeter: Some("5.6.3".into()), problem: None }
            };
        })
    });
    app.cx.run_until_parked();
    app
}

fn dialog(app: &mut App) -> Value {
    app.state()["dialog"].clone()
}

fn wait_done(app: &mut App) -> Value {
    let start = Instant::now();
    loop {
        app.cx.executor().advance_clock(Duration::from_millis(250));
        app.cx.run_until_parked();
        let d = dialog(app);
        if d["busy"] == false {
            return d;
        }
        assert!(start.elapsed() < Duration::from_secs(60), "the test did not end: {d}");
        std::thread::sleep(Duration::from_millis(80));
    }
}

fn goto(app: &mut App, name: &str) {
    app.keys("ctrl-p");
    app.input(name);
    app.keys("enter");
}

#[gpui::test]
fn the_shortcut_and_the_menu_open_the_dialog_and_it_needs_java_and_jmeter(cx: &mut TestAppContext) {
    let s = setup("expr-open", None);
    let mut app = open(cx, &s, false);
    app.keys("ctrl-shift-t");
    let d = dialog(&mut app);
    assert_eq!(d["kind"], "expression");
    assert_eq!(d["expression_kind"], "jsonpath", "it starts with JSON Path");
    for sel in ["expr-kind-jsonpath", "expr-kind-regex", "expr-kind-xpath", "expr-kind-css", "expr-text", "expr-sample", "expr-result", "dialog-expr-try", "dialog-cancel"] {
        assert!(app.has(sel), "{sel}");
    }
    app.click("expr-kind-xpath");
    assert_eq!(dialog(&mut app)["expression_kind"], "xpath");
    app.click("expr-kind-css");
    assert_eq!(dialog(&mut app)["expression_kind"], "css");
    app.click("dialog-cancel");
    assert!(app.state()["dialog"].is_null());
    // The keyboard is back in the window, so shortcuts work without a click.
    app.keys("ctrl-k");
    assert!(app.palette_open());
}

#[gpui::test]
fn without_java_or_jmeter_the_action_says_why(cx: &mut TestAppContext) {
    let s = setup("expr-nojava", None);
    let mut app = open(cx, &s, false);
    let shell = app.shell.clone();
    app.cx.update(|_, cx| shell.update(cx, |sh, _| sh.runtime = jmeter_ui::shell::RuntimeStatus::default()));
    app.keys("ctrl-k");
    app.input("expression");
    assert!(app.palette_unavailable().iter().any(|t| t.contains("Try an Expression")));
    app.keys("escape");
    app.keys("ctrl-shift-t");
    assert!(app.state()["dialog"].is_null(), "nothing opens");
}

#[gpui::test]
fn an_extractor_starts_the_dialog_with_its_own_expression(cx: &mut TestAppContext) {
    let s = setup("expr-extractor", None);
    let mut app = open(cx, &s, false);
    goto(&mut app, "randomBugIdExtractor");
    assert_eq!(app.selected().as_deref(), Some("randomBugIdExtractor"));
    perform(&mut app, Action::TestExpression);
    let d = dialog(&mut app);
    assert_eq!(d["expression_kind"], "regex");
    assert!(d["expression"].as_str().unwrap().starts_with("https://bz.apache.org/bugzilla/show_bug.cgi"), "{d}");
    // Something that is not an extractor starts empty.
    app.click("dialog-cancel");
    goto(&mut app, "Home");
    perform(&mut app, Action::TestExpression);
    let d = dialog(&mut app);
    assert_eq!(d["expression"], "");
}

fn perform(app: &mut App, action: Action) {
    let shell = app.shell.clone();
    app.cx.update(|window, cx| shell.update(cx, |s, cx| s.perform(action, window, cx)));
    app.cx.run_until_parked();
}

/// Type into the two boxes of the open dialog.
fn fill(app: &mut App, expr: &str, sample: &str) {
    app.click("expr-text");
    app.keys(&secondary("ctrl-a"));
    app.input(expr);
    app.click("expr-sample");
    app.keys(&secondary("ctrl-a"));
    app.input(sample);
}

#[gpui::test]
fn a_try_shows_what_was_found_and_what_is_wrong(cx: &mut TestAppContext) {
    let Some(env) = jm::env() else { return };
    let s = setup("expr-try", Some(&env.home));
    let mut app = open(cx, &s, true);
    assert_eq!(app.state()["runtime_ready"], true);
    app.keys("ctrl-shift-t");
    // JSON Path.
    fill(&mut app, "$.items[*].id", r#"{"items":[{"id":7},{"id":9}]}"#);
    app.click("dialog-expr-try");
    let d = wait_done(&mut app);
    assert_eq!((d["error"].clone(), d["status"].as_str()), (Value::Bool(false), Some("2 found")), "{d}");
    assert_eq!(d["result"], "1: 7\n2: 9");
    // Nothing found is not an error.
    fill(&mut app, "$.nothing", r#"{"a":1}"#);
    app.click("dialog-expr-try");
    let d = wait_done(&mut app);
    assert_eq!((d["error"].clone(), d["status"].as_str(), d["result"].as_str()), (Value::Bool(false), Some("Nothing found"), Some("")), "{d}");
    // A regular expression, started with Enter in the expression box.
    app.click("expr-kind-regex");
    app.click("expr-sample");
    app.keys(&secondary("ctrl-a"));
    app.input("id=17 id=42");
    app.click("expr-text");
    app.keys(&secondary("ctrl-a"));
    app.input(r"id=(\d+)");
    app.keys("enter");
    let d = wait_done(&mut app);
    assert!(d["result"].as_str().unwrap().contains("group 1: 17") && d["result"].as_str().unwrap().contains("group 1: 42"), "{d}");
    // A broken expression says so in words, and shows no old result.
    fill(&mut app, "(oops", "text");
    app.click("dialog-expr-try");
    let d = wait_done(&mut app);
    assert_eq!(d["error"], true);
    assert!(d["status"].as_str().unwrap().to_lowercase().contains("unclosed"), "{d}");
    assert_eq!(d["result"], "");
    // Nothing typed.
    fill(&mut app, " ", "text");
    app.click("dialog-expr-try");
    let d = wait_done(&mut app);
    assert_eq!((d["error"].clone(), d["status"].as_str()), (Value::Bool(true), Some("Write an expression first.")));
}

#[gpui::test]
fn a_text_that_is_too_large_is_refused_without_starting_anything(cx: &mut TestAppContext) {
    let Some(env) = jm::env() else { return };
    let s = setup("expr-big", Some(&env.home));
    let mut app = open(cx, &s, true);
    let big = "x".repeat(jvm_runtime::tester::MAX_SAMPLE_BYTES + 1);
    let shell = app.shell.clone();
    app.cx.update(|window, cx| shell.update(cx, |sh, cx| sh.open_expression_with(jvm_runtime::tester::Kind::Regex, "x", &big, window, cx)));
    app.cx.run_until_parked();
    // That no Java is started for it is checked by the unit test of the tester; drawing a 1 MB text is slow on some machines.
    app.click("dialog-expr-try");
    let d = wait_done(&mut app);
    assert_eq!(d["error"], true);
    assert!(d["status"].as_str().unwrap().contains("larger than 1 MB"), "{d}");
}

#[gpui::test]
fn closing_the_dialog_while_it_works_is_safe(cx: &mut TestAppContext) {
    let Some(env) = jm::env() else { return };
    let s = setup("expr-close", Some(&env.home));
    let mut app = open(cx, &s, true);
    app.keys("ctrl-shift-t");
    fill(&mut app, "$.a", r#"{"a":1}"#);
    app.click("dialog-expr-try");
    app.click("dialog-cancel");
    assert!(app.state()["dialog"].is_null());
    // The answer arrives with no dialog to show it in, and nothing breaks.
    for _ in 0..40 {
        app.cx.executor().advance_clock(Duration::from_millis(250));
        app.cx.run_until_parked();
        std::thread::sleep(Duration::from_millis(80));
    }
    assert!(app.state()["dialog"].is_null());
    // The window still works.
    app.keys("ctrl-shift-t");
    assert_eq!(dialog(&mut app)["kind"], "expression");
}
