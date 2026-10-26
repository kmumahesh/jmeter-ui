//! The first start: the welcome screen says whether Java and JMeter are found, what to do when one is not, and finds
//! a JMeter install by itself when there is one in the usual places.

mod common;

use common::*;
use gpui::TestAppContext;
use jmeter_ui::shell::RuntimeStatus;
use shell_core::settings::{self, Settings};

fn set_runtime(app: &mut App, runtime: RuntimeStatus, checked: bool) {
    let shell = app.shell.clone();
    app.cx.update(|_, cx| {
        shell.update(cx, |sh, cx| {
            sh.runtime = runtime;
            sh.runtime_checked = checked;
            cx.notify();
        })
    });
    app.cx.run_until_parked();
}

fn ready() -> RuntimeStatus {
    RuntimeStatus { java: Some("Java 21.0.8".into()), jmeter: Some("5.6.3".into()), problem: None }
}

#[gpui::test]
fn nothing_is_said_until_the_look_for_java_and_jmeter_has_finished(cx: &mut TestAppContext) {
    let mut app = start(cx, None, None);
    set_runtime(&mut app, RuntimeStatus::default(), false);
    assert!(!app.has("setup-needed") && !app.has("setup-ready"), "no flash of 'not found' while it is still looking");
    assert!(app.state()["setup_headline"].is_null());
}

#[gpui::test]
fn missing_java_is_explained_with_a_way_forward(cx: &mut TestAppContext) {
    let mut app = start(cx, None, None);
    set_runtime(&mut app, RuntimeStatus { java: None, jmeter: None, problem: Some("java was not found on PATH".into()) }, true);
    assert!(app.has("setup-needed") && app.has("setup-problem") && app.has("setup-settings") && app.has("setup-recheck"));
    assert!(!app.has("setup-ready"));
    assert_eq!(app.state()["setup_headline"], "Java was not found");
    // Open Settings takes the person to the place to fix it.
    app.click("setup-settings");
    assert_eq!(app.active_tab().as_deref(), Some("Settings"));
}

#[gpui::test]
fn missing_jmeter_is_explained_when_java_is_there(cx: &mut TestAppContext) {
    let mut app = start(cx, None, None);
    set_runtime(&mut app, RuntimeStatus { java: Some("Java 21.0.8".into()), jmeter: None, problem: None }, true);
    assert!(app.has("setup-needed"));
    assert!(!app.has("setup-problem"), "there is nothing technical to show");
    assert_eq!(app.state()["setup_headline"], "JMeter was not found");
}

#[gpui::test]
fn when_everything_is_found_the_welcome_says_ready(cx: &mut TestAppContext) {
    let mut app = start(cx, None, None);
    set_runtime(&mut app, ready(), true);
    assert!(app.has("setup-ready") && !app.has("setup-needed"));
    assert!(app.state()["setup_headline"].is_null());
    assert!(app.has("welcome-new") && app.has("welcome-open"), "the usual buttons are still there");
}

#[gpui::test]
fn checking_again_looks_again_and_a_wrong_choice_stays_explained(cx: &mut TestAppContext) {
    let dir = temp_dir("setup-recheck");
    let settings_path = dir.join("settings.json");
    settings::save(&settings_path, &Settings { jmeter_home: Some(dir.join("no-such-jmeter").to_string_lossy().into_owned()), java_path: Some(dir.join("no-such-java.exe").to_string_lossy().into_owned()), ..Settings::default() }).unwrap();
    let mut app = start(cx, None, Some(settings_path));
    set_runtime(&mut app, RuntimeStatus::default(), false);
    let shell = app.shell.clone();
    app.cx.update(|window, cx| shell.update(cx, |sh, cx| sh.detect_runtime(window, cx)));
    // The look runs in the background; give it time.
    for _ in 0..200 {
        app.cx.executor().advance_clock(std::time::Duration::from_millis(100));
        app.cx.run_until_parked();
        if app.state()["runtime_checked"] == true {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    assert_eq!(app.state()["runtime_checked"], true);
    // Java that is chosen but missing falls back to the Java on the path; the JMeter folder that is chosen but missing does not
    // fall back to anything, and is not replaced by one found elsewhere either.
    let st = app.state();
    if jvm_runtime::locate::find_java(None).is_ok() {
        assert_eq!(st["setup_headline"], "JMeter was not found");
    } else {
        assert_eq!(st["setup_headline"], "Java was not found");
    }
    assert!(st["jmeter_home"].as_str().unwrap().contains("no-such-jmeter"), "a folder the person chose is never replaced: {}", st["jmeter_home"]);
    assert!(app.has("setup-needed"));
}

#[gpui::test]
fn a_jmeter_in_the_usual_places_is_found_and_remembered_without_any_setup(cx: &mut TestAppContext) {
    // Only where this machine has one of the usual installs and Java: otherwise there is nothing to find.
    let Some(found) = jvm_runtime::locate::find_jmeter_home() else { return };
    if jvm_runtime::locate::find_java(None).is_err() {
        return;
    }
    let dir = temp_dir("setup-found");
    let settings_path = dir.join("settings.json");
    settings::save(&settings_path, &Settings::default()).unwrap();
    let mut app = start(cx, None, Some(settings_path.clone()));
    let shell = app.shell.clone();
    app.cx.update(|window, cx| shell.update(cx, |sh, cx| sh.detect_runtime(window, cx)));
    for _ in 0..400 {
        app.cx.executor().advance_clock(std::time::Duration::from_millis(100));
        app.cx.run_until_parked();
        if app.state()["runtime_checked"] == true {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    let st = app.state();
    assert_eq!(st["runtime_ready"], true, "{st}");
    assert!(app.has("setup-ready"));
    let saved = settings::load(&settings_path).settings.jmeter_home.expect("the folder that was found is remembered");
    assert_eq!(std::path::PathBuf::from(&saved).canonicalize().ok(), found.canonicalize().ok());
    assert!(st["notice"].as_str().unwrap_or("").starts_with("Found JMeter in"), "{}", st["notice"]);
}

#[gpui::test]
fn applying_settings_with_an_empty_jmeter_folder_does_not_fill_it_in(cx: &mut TestAppContext) {
    // What was typed in Settings is taken as it is: an emptied folder stays empty even when JMeter is in Downloads.
    let dir = temp_dir("setup-empty");
    let settings_path = dir.join("settings.json");
    settings::save(&settings_path, &Settings::default()).unwrap();
    let mut app = start(cx, None, Some(settings_path.clone()));
    let shell = app.shell.clone();
    app.cx.update(|window, cx| shell.update(cx, |sh, cx| sh.detect_runtime_with(false, window, cx)));
    for _ in 0..200 {
        app.cx.executor().advance_clock(std::time::Duration::from_millis(100));
        app.cx.run_until_parked();
        if app.state()["runtime_checked"] == true {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    assert_eq!(app.state()["runtime_checked"], true);
    assert!(app.state()["jmeter_home"].is_null(), "{}", app.state()["jmeter_home"]);
    assert!(settings::load(&settings_path).settings.jmeter_home.is_none());
    assert!(!app.state()["notice"].as_str().unwrap_or("").starts_with("Found JMeter"));
}
