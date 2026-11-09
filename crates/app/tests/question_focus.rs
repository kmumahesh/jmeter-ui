//! The buttons of a question can be reached with the keyboard: the default button has the keyboard first, Tab and
//! Shift+Tab move between the buttons, Enter presses the one that has it, and Escape cancels.

mod common;

use common::*;
use gpui::TestAppContext;
use plan_core::files;
use std::path::{Path, PathBuf};
use std::time::Duration;

fn copy_of(name: &str, fixture_name: &str) -> PathBuf {
    let d = temp_dir(name);
    let p = d.join(fixture_name);
    std::fs::copy(fixture(fixture_name), &p).unwrap();
    p
}

fn text_of(p: &Path) -> String {
    std::fs::read_to_string(p).unwrap()
}

/// A plan with an unsaved change, closed with Ctrl+W: the question has three buttons (Save, Don't Save, Cancel).
fn ask_to_save<'a>(cx: &'a mut TestAppContext, name: &str) -> (App<'a>, PathBuf) {
    let p = copy_of(name, WEB);
    let mut app = start(cx, Some(p.clone()), None);
    app.keys("ctrl-p");
    app.input("jmeter users");
    app.keys("enter");
    app.keys("ctrl-k");
    app.input("+debug sampler");
    app.keys("enter");
    app.cx.executor().advance_clock(Duration::from_secs(1));
    app.cx.run_until_parked();
    app.keys("ctrl-w");
    assert_eq!(app.state()["dialog"]["kind"], "unsaved");
    app.settle();
    (app, p)
}

fn focus(app: &mut App) -> serde_json::Value {
    app.settle();
    app.state()["dialog_focus"].clone()
}

#[gpui::test]
fn the_default_button_has_the_keyboard_when_a_question_opens(cx: &mut TestAppContext) {
    let (mut app, _) = ask_to_save(cx, "qfocus-first");
    assert_eq!(focus(&mut app), "dialog-save");
}

#[gpui::test]
fn tab_and_shift_tab_move_between_the_buttons_and_wrap_around(cx: &mut TestAppContext) {
    let (mut app, _) = ask_to_save(cx, "qfocus-tab");
    for want in ["dialog-discard", "dialog-cancel", "dialog-save", "dialog-discard"] {
        app.keys("tab");
        assert_eq!(focus(&mut app), want, "Tab goes to the next button");
    }
    for want in ["dialog-save", "dialog-cancel", "dialog-discard"] {
        app.keys("shift-tab");
        assert_eq!(focus(&mut app), want, "Shift+Tab goes to the previous button (and wraps)");
    }
    assert_eq!(app.state()["dialog"]["kind"], "unsaved", "the question is still open");
}

#[gpui::test]
fn enter_presses_the_button_that_has_the_keyboard(cx: &mut TestAppContext) {
    // The default button: Save.
    let (mut app, p) = ask_to_save(cx, "qfocus-save");
    app.keys("enter");
    assert!(app.state()["dialog"].is_null(), "answered");
    assert!(text_of(&p).contains("DebugSampler"), "Enter on Save saved the plan");
}

#[gpui::test]
fn enter_on_cancel_cancels_and_enter_on_dont_save_drops_the_change(cx: &mut TestAppContext) {
    let (mut app, p) = ask_to_save(cx, "qfocus-cancel");
    let before = text_of(&p);
    app.keys("tab");
    app.keys("tab");
    assert_eq!(focus(&mut app), "dialog-cancel");
    app.keys("enter");
    assert!(app.state()["dialog"].is_null());
    assert_eq!(app.tabs().len(), 1, "Cancel keeps the plan open");
    assert_eq!(text_of(&p), before, "and saves nothing");
    // Ask again and choose Don't Save.
    app.keys("ctrl-w");
    app.settle();
    app.keys("tab");
    assert_eq!(focus(&mut app), "dialog-discard");
    app.keys("enter");
    assert!(app.state()["dialog"].is_null());
    assert!(app.tabs().is_empty(), "Don't Save closed the plan");
    assert_eq!(text_of(&p), before, "without saving");
    let _ = files::recovery_path(&p);
}

#[gpui::test]
fn escape_still_cancels_whichever_button_has_the_keyboard(cx: &mut TestAppContext) {
    let (mut app, p) = ask_to_save(cx, "qfocus-escape");
    let before = text_of(&p);
    app.keys("tab");
    app.keys("escape");
    assert!(app.state()["dialog"].is_null());
    assert_eq!(app.tabs().len(), 1);
    assert_eq!(text_of(&p), before);
}

#[gpui::test]
fn clicking_a_button_still_answers_the_question(cx: &mut TestAppContext) {
    let (mut app, p) = ask_to_save(cx, "qfocus-click");
    app.click("dialog-discard");
    assert!(app.state()["dialog"].is_null());
    assert!(!text_of(&p).contains("DebugSampler"));
}

#[gpui::test]
fn a_question_that_must_be_answered_cannot_be_escaped_but_its_buttons_can_be_reached(cx: &mut TestAppContext) {
    let p = copy_of("qfocus-recover", WEB);
    std::thread::sleep(Duration::from_millis(20));
    files::autosave(&p, &text_of(&p).replacen("BugDetail Page", "Recovered Name", 1)).unwrap();
    let mut app = start(cx, Some(p.clone()), None);
    app.settle();
    assert_eq!(app.state()["dialog"]["kind"], "recover");
    assert_eq!(focus(&mut app), "dialog-recover");
    app.keys("tab");
    assert_eq!(focus(&mut app), "dialog-discard-recovery");
    app.keys("escape");
    assert_eq!(app.state()["dialog"]["kind"], "recover", "Escape does not close it");
    app.keys("enter");
    assert!(app.state()["dialog"].is_null(), "Enter on Discard Them answered it");
}
