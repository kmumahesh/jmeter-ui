//! Add Think Times to Children from the window: when it is available, the question, the checks on the two pauses, what
//! is added, and one step to undo.

mod common;

use common::*;
use gpui::TestAppContext;
use serde_json::Value;
use shell_core::actions::Action;
use std::path::PathBuf;

fn plan_file(name: &str) -> PathBuf {
    let d = temp_dir(name);
    let p = d.join("web.jmx");
    std::fs::copy(fixture(WEB), &p).unwrap();
    p
}

fn perform(app: &mut App, action: Action) {
    let shell = app.shell.clone();
    app.cx.update(|window, cx| shell.update(cx, |s, cx| s.perform(action, window, cx)));
    app.cx.run_until_parked();
}

fn goto(app: &mut App, name: &str) {
    app.keys("ctrl-p");
    app.input(name);
    app.keys("enter");
}

/// Replace what is in a box without pressing Enter (which would send the question).
fn fill(app: &mut App, selector: &'static str, text: &str) {
    app.click(selector);
    app.keys(&secondary("ctrl-a"));
    if text.is_empty() {
        app.keys("backspace");
    } else {
        app.input(text);
    }
}

fn dialog(app: &mut App) -> Value {
    app.state()["dialog"].clone()
}

fn elements(app: &mut App) -> u64 {
    app.state()["elements"].as_u64().unwrap()
}

fn notice(app: &mut App) -> String {
    app.state()["notice"].as_str().unwrap_or("").to_string()
}

#[gpui::test]
fn it_needs_a_selection_that_can_hold_requests(cx: &mut TestAppContext) {
    let mut app = start(cx, Some(plan_file("think-avail")), None);
    // With a thread group selected it can be chosen.
    goto(&mut app, "Home Page");
    app.keys("ctrl-k");
    app.input("think times");
    assert!(app.palette_titles().iter().any(|t| t.contains("Add Think Times to Children")), "{:?}", app.palette_titles());
    assert!(app.palette_unavailable().is_empty(), "{:?}", app.palette_unavailable());
    app.keys("escape");
    // An extractor holds no requests, so it is shown but cannot be chosen.
    goto(&mut app, "randomBugIdExtractor");
    app.keys("ctrl-k");
    app.input("think times");
    assert!(app.palette_unavailable().iter().any(|t| t.contains("Add Think Times to Children")), "{:?}", app.palette_unavailable());
    app.keys("escape");
    perform(&mut app, Action::AddThinkTimes);
    assert!(app.state()["dialog"].is_null(), "it does not open for an extractor");
}

#[gpui::test]
fn the_question_adds_the_pauses_and_one_undo_takes_them_away(cx: &mut TestAppContext) {
    let mut app = start(cx, Some(plan_file("think-add")), None);
    goto(&mut app, "Home Page");
    let before = elements(&mut app);
    let text_before = app.plan_text();
    perform(&mut app, Action::AddThinkTimes);
    let d = dialog(&mut app);
    assert_eq!((d["kind"].clone(), d["min"].clone(), d["max"].clone()), ("think_times".into(), "1000".into(), "3000".into()), "{d}");
    for sel in ["think-min", "think-max", "dialog-think-add", "dialog-cancel"] {
        assert!(app.has(sel), "{sel}");
    }
    app.click("dialog-think-add");
    assert!(app.state()["dialog"].is_null(), "{}", dialog(&mut app));
    let n = notice(&mut app);
    assert!(n.contains("Added") && n.contains("think time") && n.contains("1000 to 3000 ms"), "{n}");
    let after = elements(&mut app);
    assert!(after > before, "{before} -> {after}");
    assert!(app.plan_text().contains("testname=\"Think time\""));
    assert_eq!(app.state()["dirty"], true);
    perform(&mut app, Action::Undo);
    assert_eq!(elements(&mut app), before, "one undo takes all of them away");
    assert_eq!(app.plan_text(), text_before);
    perform(&mut app, Action::Redo);
    assert_eq!(elements(&mut app), after);
}

#[gpui::test]
fn wrong_pauses_are_explained_and_nothing_changes(cx: &mut TestAppContext) {
    let mut app = start(cx, Some(plan_file("think-bad")), None);
    goto(&mut app, "Home Page");
    let before = elements(&mut app);
    perform(&mut app, Action::AddThinkTimes);
    for (min, max, want) in [("", "3000", "Write the shortest"), ("1000", "", "Write the longest"), ("3000", "1000", "shortest pause is longer"), ("0", "99999999", "")] {
        fill(&mut app, "think-min", min);
        fill(&mut app, "think-max", max);
        if !want.is_empty() {
            app.click("dialog-think-add");
            let d = dialog(&mut app);
            assert_eq!(d["kind"], "think_times", "the question stays open for {min:?} {max:?}");
            assert!(d["error"].as_str().unwrap_or("").contains(want), "{min:?} {max:?}: {d}");
            assert!(app.has("think-error"));
        }
    }
    assert_eq!(elements(&mut app), before, "nothing was added");
    assert!(app.state()["dirty"] == false);
    // Fixing the pauses lets it through.
    fill(&mut app, "think-min", "200");
    fill(&mut app, "think-max", "200");
    app.click("dialog-think-add");
    assert!(app.state()["dialog"].is_null());
    assert!(notice(&mut app).contains("200 ms"), "a single pause is said as one: {}", notice(&mut app));
    assert!(app.plan_text().contains("ConstantTimer"));
}

#[gpui::test]
fn asking_twice_says_nothing_is_left_to_do_and_cancel_changes_nothing(cx: &mut TestAppContext) {
    let mut app = start(cx, Some(plan_file("think-twice")), None);
    goto(&mut app, "Home Page");
    perform(&mut app, Action::AddThinkTimes);
    app.click("dialog-think-add");
    let after_first = elements(&mut app);
    perform(&mut app, Action::AddThinkTimes);
    app.click("dialog-think-add");
    let d = dialog(&mut app);
    assert_eq!(d["kind"], "think_times");
    assert!(d["error"].as_str().unwrap().contains("already has a pause"), "{d}");
    assert_eq!(elements(&mut app), after_first);
    app.click("dialog-cancel");
    assert!(app.state()["dialog"].is_null());
    // The keyboard is back in the window.
    app.keys("ctrl-k");
    assert!(app.palette_open());
}
