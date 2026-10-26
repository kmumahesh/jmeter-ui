//! Starting a new plan from the window: the question, the starters, what ends up on disk, what is refused, and
//! that a starter runs for real. Tests that run JMeter skip themselves without Java and JMeter.

mod common;
#[path = "../../jvm-runtime/tests/common/mod.rs"]
mod jm;

use common::*;
use gpui::TestAppContext;
use plan_core::EditablePlan;
use plan_core::starters::{self, Starter};
use serde_json::Value;
use shell_core::actions::Action;
use shell_core::settings::{self, Settings};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

fn perform(app: &mut App, action: Action) {
    let shell = app.shell.clone();
    app.cx.update(|window, cx| shell.update(cx, |s, cx| s.perform(action, window, cx)));
    app.cx.run_until_parked();
}

fn create(app: &mut App, starter: Starter, path: &Path) -> bool {
    let shell = app.shell.clone();
    let path = path.to_path_buf();
    let ok = app.cx.update(|window, cx| shell.update(cx, |s, cx| s.create_plan_at(starter, &path, window, cx)));
    app.cx.run_until_parked();
    ok
}

fn dialog_kind(app: &mut App) -> Option<String> {
    app.state()["dialog"]["kind"].as_str().map(str::to_string)
}

fn notice(app: &mut App) -> String {
    app.state()["notice"].as_str().unwrap_or_default().to_string()
}

fn empty_app<'a>(cx: &'a mut TestAppContext) -> App<'a> {
    start(cx, None, None)
}

// ---- the question ----

#[gpui::test]
fn ctrl_n_asks_which_kind_of_plan_and_cancel_changes_nothing(cx: &mut TestAppContext) {
    let mut app = empty_app(cx);
    assert!(dialog_kind(&mut app).is_none());
    app.keys("ctrl-n");
    assert_eq!(dialog_kind(&mut app).as_deref(), Some("new_plan"));
    for sel in ["dialog-new-api", "dialog-new-web", "dialog-new-login", "dialog-new-data", "dialog-new-load", "dialog-new-spike", "dialog-new-empty", "dialog-cancel"] {
        assert!(app.has(sel), "{sel}");
    }
    app.click("dialog-cancel");
    assert!(dialog_kind(&mut app).is_none());
    assert!(app.tabs().is_empty());
}

#[gpui::test]
fn new_plan_is_in_the_palette_the_welcome_screen_and_works_with_a_plan_open(cx: &mut TestAppContext) {
    let mut app = empty_app(cx);
    assert!(app.has("welcome-new"));
    app.click("welcome-new");
    assert_eq!(dialog_kind(&mut app).as_deref(), Some("new_plan"));
    app.keys("escape");
    app.click("dialog-cancel");
    app.keys("ctrl-shift-p");
    app.input(">new plan");
    assert_eq!(app.palette_titles().first().map(String::as_str), Some("New Plan\u{2026}"));
    app.keys("escape");
    let d = temp_dir("new-with-open");
    let p = d.join("one.jmx");
    assert!(create(&mut app, Starter::Empty, &p));
    app.keys("ctrl-n");
    assert_eq!(dialog_kind(&mut app).as_deref(), Some("new_plan"), "a plan being open does not stop a new one");
}

#[gpui::test]
fn the_question_blocks_everything_behind_it(cx: &mut TestAppContext) {
    let d = temp_dir("new-blocks");
    let p = d.join("a.jmx");
    std::fs::write(&p, starters::new_plan_text(Starter::Api, "a").unwrap()).unwrap();
    let mut app = start(cx, Some(p), None);
    app.keys("ctrl-n");
    app.keys("ctrl-shift-a");
    assert!(!app.palette_open(), "nothing else opens while the question is showing");
}

// ---- what ends up on disk ----

#[gpui::test]
fn each_starter_is_written_opened_clean_and_named_after_the_file(cx: &mut TestAppContext) {
    let d = temp_dir("new-each");
    let mut app = empty_app(cx);
    for (starter, file, elements) in [(Starter::Api, "my api.jmx", 7), (Starter::Web, "site.jmx", 9), (Starter::Empty, "bare.jmx", 1)] {
        let path = d.join(file);
        assert!(create(&mut app, starter, &path), "{file}: {}", notice(&mut app));
        assert_eq!(app.active_tab().as_deref(), Some(file));
        assert_eq!(app.state()["elements"], elements, "{file}");
        let stem = file.trim_end_matches(".jmx");
        assert_eq!(app.editing().as_deref(), Some(stem), "the Test Plan is named after the file");
        assert!(!app.state()["dirty"].as_bool().unwrap(), "{file} opens with nothing to save");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), starters::new_plan_text(starter, stem).unwrap());
    }
    assert_eq!(app.tabs().len(), 3);
}

#[gpui::test]
fn a_file_name_without_an_extension_gets_jmx(cx: &mut TestAppContext) {
    let d = temp_dir("new-ext");
    let mut app = empty_app(cx);
    assert!(create(&mut app, Starter::Empty, &d.join("plain")));
    assert!(d.join("plain.jmx").is_file());
    assert!(!d.join("plain").exists());
    assert_eq!(app.active_tab().as_deref(), Some("plain.jmx"));
}

#[gpui::test]
fn the_new_plan_is_added_to_the_recent_list(cx: &mut TestAppContext) {
    let d = temp_dir("new-recent");
    let settings_path = d.join("settings.json");
    let mut app = start(cx, None, Some(settings_path.clone()));
    assert!(create(&mut app, Starter::Api, &d.join("fresh.jmx")));
    let saved = settings::load(&settings_path).settings;
    assert!(saved.recent_files.first().is_some_and(|p| p.ends_with("fresh.jmx")), "{:?}", saved.recent_files);
}

#[gpui::test]
fn the_new_plan_can_be_edited_saved_and_undone_like_any_other(cx: &mut TestAppContext) {
    let d = temp_dir("new-edit");
    let p = d.join("edit.jmx");
    let mut app = empty_app(cx);
    assert!(create(&mut app, Starter::Api, &p));
    app.keys("ctrl-p");
    app.input("get items");
    app.keys("enter");
    app.keys("ctrl-d");
    assert_eq!(app.state()["elements"], 9, "the request and its check were copied");
    assert!(app.state()["dirty"].as_bool().unwrap());
    app.keys("ctrl-s");
    assert!(!app.state()["dirty"].as_bool().unwrap());
    let saved = std::fs::read_to_string(&p).unwrap();
    assert_eq!(saved.matches("testname=\"Get items\"").count(), 2);
    app.keys("ctrl-z");
    assert_eq!(app.state()["elements"], 7);
}

// ---- what is refused ----

#[gpui::test]
fn a_plan_that_is_already_open_is_not_replaced(cx: &mut TestAppContext) {
    let d = temp_dir("new-open-twice");
    let p = d.join("busy.jmx");
    let mut app = empty_app(cx);
    assert!(create(&mut app, Starter::Api, &p));
    let before = std::fs::read_to_string(&p).unwrap();
    assert!(!create(&mut app, Starter::Empty, &p));
    assert!(notice(&mut app).contains("open in a tab"), "{}", notice(&mut app));
    assert_eq!(std::fs::read_to_string(&p).unwrap(), before, "the file was not touched");
    assert_eq!(app.tabs().len(), 1);
}

#[gpui::test]
fn a_folder_or_an_unwritable_place_is_refused_with_a_message(cx: &mut TestAppContext) {
    let d = temp_dir("new-refuse");
    std::fs::create_dir_all(d.join("folder.jmx")).unwrap();
    let mut app = empty_app(cx);
    assert!(!create(&mut app, Starter::Api, &d.join("folder.jmx")));
    assert!(notice(&mut app).contains("is a folder"), "{}", notice(&mut app));
    assert!(!create(&mut app, Starter::Api, &d.join("no such folder").join("x.jmx")));
    assert!(notice(&mut app).contains("Could not save"), "{}", notice(&mut app));
    assert!(app.tabs().is_empty(), "nothing opened");
    assert!(!d.join("no such folder").exists(), "no folders are made behind the user's back");
}

#[gpui::test]
fn choosing_an_existing_file_replaces_it_whole_and_removes_its_old_recovery_copy(cx: &mut TestAppContext) {
    let d = temp_dir("new-replace");
    let p = d.join("old.jmx");
    std::fs::write(&p, "not a plan at all").unwrap();
    plan_core::files::autosave(&p, "stale recovery").unwrap();
    let mut app = empty_app(cx);
    assert!(create(&mut app, Starter::Web, &p));
    assert!(!plan_core::files::recovery_path(&p).exists(), "no question about an old recovery copy");
    assert!(std::fs::read_to_string(&p).unwrap().contains("Visitors"));
    assert!(!d.join("old.jmx.tmp").exists(), "no temporary file is left behind");
}

#[gpui::test]
fn names_that_look_like_markup_or_are_very_long_make_a_valid_plan(cx: &mut TestAppContext) {
    let d = temp_dir("new-names");
    let mut app = empty_app(cx);
    let long = "n".repeat(200);
    for stem in ["a&b'c", "x]]y", "&amp;", "caf\u{e9} \u{1F600}", long.as_str()] {
        let path = d.join(format!("{stem}.jmx"));
        assert!(create(&mut app, Starter::Api, &path), "{stem}: {}", notice(&mut app));
        let e = EditablePlan::open(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(e.plan().len(), 7, "{stem}: the name did not change the shape of the plan");
        assert_eq!(e.plan().node(e.plan().roots()[0]).unwrap().name, starters::clean_name(stem));
    }
}

// ---- a starter works for real ----

/// Java and JMeter, a local web server and an app that knows where JMeter is, or `None` to skip.
fn real_app<'a>(cx: &'a mut TestAppContext, name: &str) -> Option<(App<'a>, jm::Server, PathBuf)> {
    let env = jm::env()?;
    let server = jm::Server::start();
    let dir = temp_dir(name);
    let settings_path = dir.join("settings.json");
    settings::save(&settings_path, &Settings { jmeter_home: Some(env.home.to_string_lossy().into_owned()), ..Settings::default() }).unwrap();
    let app = start(cx, None, Some(settings_path.clone()));
    let shell = app.shell.clone();
    app.cx.update(|_, cx| {
        shell.update(cx, |sh, _| {
            let home = settings::load(&settings_path).settings.jmeter_home.map(PathBuf::from);
            sh.runtime = jmeter_ui::shell::check_runtime(None, home.as_deref());
        })
    });
    app.cx.run_until_parked();
    Some((app, server, dir))
}

fn wait_run(app: &mut App, max: Duration) -> Value {
    let start = Instant::now();
    loop {
        app.cx.executor().advance_clock(Duration::from_millis(250));
        app.cx.run_until_parked();
        let st = app.state();
        if matches!(st["run"]["phase"].as_str(), Some("Finished" | "Failed" | "Stopped")) {
            return st;
        }
        assert!(start.elapsed() < max, "the run did not end in {max:?}: {}", st["run"]);
        std::thread::sleep(Duration::from_millis(80));
    }
}

/// Point a saved plan at the local server and make it small and fast, then read it again in the window.
fn aim_at(app: &mut App, path: &Path, port: u16, tweak: impl FnOnce(&mut EditablePlan)) {
    let mut e = EditablePlan::open(&std::fs::read_to_string(path).unwrap()).unwrap();
    let p = e.plan().clone();
    let defaults = p.find_by_name("HTTP Request Defaults").unwrap();
    e.set_prop(defaults, "HTTPSampler.protocol", "stringProp", "http").unwrap();
    e.set_prop(defaults, "HTTPSampler.domain", "stringProp", "127.0.0.1").unwrap();
    e.set_prop(defaults, "HTTPSampler.port", "stringProp", &port.to_string()).unwrap();
    tweak(&mut e);
    std::fs::write(path, e.to_text()).unwrap();
    perform(app, Action::ReloadPlan);
}

#[gpui::test]
fn the_api_starter_runs_against_a_server_and_every_check_passes(cx: &mut TestAppContext) {
    let Some((mut app, server, dir)) = real_app(cx, "new-run-api") else { return };
    let path = dir.join("api.jmx");
    assert!(create(&mut app, Starter::Api, &path));
    aim_at(&mut app, &path, server.port, |e| {
        let p = e.plan().clone();
        let users = p.find_by_name("Users").unwrap();
        e.set_prop(users, "ThreadGroup.num_threads", "stringProp", "3").unwrap();
        e.set_prop(users, "ThreadGroup.ramp_time", "stringProp", "0").unwrap();
        e.set_prop(users, "ThreadGroup.main_controller/LoopController.loops", "stringProp", "4").unwrap();
        e.set_prop(p.find_by_name("Think time").unwrap(), "ConstantTimer.delay", "stringProp", "5").unwrap();
    });
    app.keys("f5");
    let st = wait_run(&mut app, Duration::from_secs(120));
    assert_eq!(st["run"]["phase"], "Finished", "{}", st["run"]);
    assert_eq!(st["run"]["samples"], 12, "3 users, 4 loops, one request each");
    assert_eq!(st["run"]["errors"], 0, "the status check passed for every request");
    assert!(server.paths.lock().unwrap().iter().all(|p| p == "/items"), "{:?}", server.paths.lock().unwrap());
}

#[gpui::test]
fn the_web_starter_runs_against_a_server_and_every_check_passes(cx: &mut TestAppContext) {
    let Some((mut app, server, dir)) = real_app(cx, "new-run-web") else { return };
    let path = dir.join("web.jmx");
    assert!(create(&mut app, Starter::Web, &path));
    aim_at(&mut app, &path, server.port, |e| {
        let p = e.plan().clone();
        let g = p.find_by_name("Visitors").unwrap();
        e.set_prop(g, "ThreadGroup.num_threads", "stringProp", "2").unwrap();
        e.set_prop(g, "ThreadGroup.ramp_time", "stringProp", "0").unwrap();
        e.set_prop(g, "ThreadGroup.main_controller/LoopController.loops", "stringProp", "3").unwrap();
        let t = p.find_by_name("Think time").unwrap();
        e.set_prop(t, "ConstantTimer.delay", "stringProp", "5").unwrap();
        e.set_prop(t, "RandomTimer.range", "stringProp", "5.0").unwrap();
    });
    app.keys("f5");
    let st = wait_run(&mut app, Duration::from_secs(120));
    assert_eq!(st["run"]["phase"], "Finished", "{}", st["run"]);
    assert_eq!((st["run"]["samples"].as_u64(), st["run"]["errors"].as_u64()), (Some(6), Some(0)));
}

#[gpui::test]
fn a_starter_checks_the_status_so_a_failing_server_shows_as_errors(cx: &mut TestAppContext) {
    let Some((mut app, server, dir)) = real_app(cx, "new-run-fail") else { return };
    let path = dir.join("fail.jmx");
    assert!(create(&mut app, Starter::Api, &path));
    aim_at(&mut app, &path, server.port, |e| {
        let p = e.plan().clone();
        let users = p.find_by_name("Users").unwrap();
        e.set_prop(users, "ThreadGroup.num_threads", "stringProp", "1").unwrap();
        e.set_prop(users, "ThreadGroup.ramp_time", "stringProp", "0").unwrap();
        e.set_prop(users, "ThreadGroup.main_controller/LoopController.loops", "stringProp", "3").unwrap();
        e.set_prop(p.find_by_name("Think time").unwrap(), "ConstantTimer.delay", "stringProp", "5").unwrap();
        e.set_prop(p.find_by_name("Get items").unwrap(), "HTTPSampler.path", "stringProp", "/fail").unwrap();
    });
    app.keys("f5");
    let st = wait_run(&mut app, Duration::from_secs(120));
    assert_eq!((st["run"]["samples"].as_u64(), st["run"]["errors"].as_u64()), (Some(3), Some(3)), "{}", st["run"]);
}

// ---- speed ----

#[gpui::test]
fn making_ten_plans_through_the_window_is_quick(cx: &mut TestAppContext) {
    let d = temp_dir("new-speed");
    let mut app = empty_app(cx);
    let t = Instant::now();
    for i in 0..10 {
        assert!(create(&mut app, Starter::ALL[i % 3], &d.join(format!("p{i}.jmx"))));
    }
    assert!(t.elapsed() < Duration::from_secs(15), "ten new plans took {:?}", t.elapsed());
    assert_eq!(app.tabs().len(), 10);
}
