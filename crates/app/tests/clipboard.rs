//! Copying, cutting and pasting elements in the window, between plans, saving a selection and merging a plan file.
//! Tests that run JMeter skip themselves without Java and JMeter.

mod common;
#[path = "../../jvm-runtime/tests/common/mod.rs"]
mod jm;

use common::*;
use gpui::{ClipboardItem, TestAppContext};
use plan_core::EditablePlan;
use plan_core::starters::{self, Starter};
use serde_json::Value;
use shell_core::settings::{self, Settings};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

fn plan_file(name: &str, starter: Starter) -> PathBuf {
    let d = temp_dir(name);
    let p = d.join(format!("{name}.jmx"));
    std::fs::write(&p, starters::new_plan_text(starter, name).unwrap()).unwrap();
    p
}

fn goto(app: &mut App, name: &str) {
    app.keys("ctrl-p");
    app.input(name);
    app.keys("enter");
}

fn clip(app: &mut App) -> Option<String> {
    app.cx.read_from_clipboard().and_then(|c| c.text())
}

fn set_clip(app: &mut App, text: &str) {
    app.cx.write_to_clipboard(ClipboardItem::new_string(text.to_string()));
}

fn elements(app: &mut App) -> u64 {
    app.state()["elements"].as_u64().unwrap()
}

fn notice(app: &mut App) -> String {
    app.state()["notice"].as_str().unwrap_or_default().to_string()
}

fn dirty(app: &mut App) -> bool {
    app.state()["dirty"].as_bool().unwrap()
}

fn names_under(app: &mut App, parent: &str) -> Vec<String> {
    app.children_of(parent)
}

fn merge(app: &mut App, path: &Path) -> bool {
    let shell = app.shell.clone();
    let path = path.to_path_buf();
    let ok = app.cx.update(|window, cx| shell.update(cx, |s, cx| s.merge_file(&path, window, cx)));
    app.cx.run_until_parked();
    ok
}

fn save_selection(app: &mut App, path: &Path) -> bool {
    let shell = app.shell.clone();
    let path = path.to_path_buf();
    let ok = app.cx.update(|window, cx| shell.update(cx, |s, cx| s.save_selection_to(&path, window, cx)));
    app.cx.run_until_parked();
    ok
}

// ---- copy ----

#[gpui::test]
fn ctrl_c_puts_the_selected_element_and_what_is_under_it_on_the_clipboard(cx: &mut TestAppContext) {
    let mut app = start(cx, Some(plan_file("clip-copy", Starter::Api)), None);
    goto(&mut app, "get items");
    app.keys("ctrl-c");
    let text = clip(&mut app).expect("something is on the clipboard");
    assert!(text.contains("testname=\"Get items\"") && text.contains("testname=\"Status is 200\""), "{text}");
    assert!(!text.contains("testname=\"Think time\"") && !text.contains("testname=\"Users\""), "only the element and what is under it");
    assert!(notice(&mut app).starts_with("Copied Get items and the 1 elements under it"), "{}", notice(&mut app));
    assert!(!dirty(&mut app), "copying changes nothing");
}

#[gpui::test]
fn copying_the_test_plan_is_not_offered_and_leaves_the_clipboard_alone(cx: &mut TestAppContext) {
    let mut app = start(cx, Some(plan_file("clip-root", Starter::Api)), None);
    set_clip(&mut app, "before");
    app.click("tree-row-0");
    app.keys("ctrl-c ctrl-x");
    assert_eq!(clip(&mut app).as_deref(), Some("before"));
    assert_eq!(elements(&mut app), 7);
    app.keys("ctrl-k");
    app.input(">copy element");
    assert!(app.palette_unavailable().contains(&"Copy Element".to_string()), "shown, but cannot be chosen");
}

#[gpui::test]
fn the_edit_commands_are_in_the_palette_with_their_shortcuts(cx: &mut TestAppContext) {
    let mut app = start(cx, Some(plan_file("clip-palette", Starter::Api)), None);
    goto(&mut app, "get items");
    for title in ["Copy Element", "Cut Element", "Paste Element", "Save Selection As\u{2026}", "Merge Plan\u{2026}"] {
        app.keys("ctrl-k");
        app.input(&format!(">{title}"));
        assert_eq!(app.palette_titles().first().map(String::as_str), Some(title), "{title}");
        assert!(!app.palette_unavailable().contains(&title.to_string()), "{title} is available with a request selected");
        app.keys("escape");
    }
}

// ---- paste ----

#[gpui::test]
fn ctrl_v_puts_a_copy_next_to_a_request_and_selects_it(cx: &mut TestAppContext) {
    let mut app = start(cx, Some(plan_file("clip-paste", Starter::Api)), None);
    goto(&mut app, "get items");
    app.keys("ctrl-c ctrl-v");
    assert_eq!(elements(&mut app), 9, "the request and its check were pasted");
    assert_eq!(names_under(&mut app, "Users"), ["Get items", "Get items", "Think time"], "right after the original");
    assert_eq!(app.selected().as_deref(), Some("Get items"));
    assert!(dirty(&mut app));
    assert_eq!(notice(&mut app), "Pasted 1 element");
    assert!(app.plan_text().matches("testname=\"Status is 200\"").count() == 2);
}

#[gpui::test]
fn a_paste_is_one_undo_step(cx: &mut TestAppContext) {
    let mut app = start(cx, Some(plan_file("clip-undo", Starter::Api)), None);
    goto(&mut app, "get items");
    let before = app.plan_text();
    app.keys("ctrl-c ctrl-v ctrl-v");
    assert_eq!(elements(&mut app), 11);
    app.keys("ctrl-z");
    assert_eq!(elements(&mut app), 9, "one paste undone, with its check");
    app.keys("ctrl-z");
    assert_eq!(app.plan_text(), before, "the file text is exactly what it was");
    assert!(!dirty(&mut app));
    app.keys("ctrl-y");
    assert_eq!(elements(&mut app), 9);
}

#[gpui::test]
fn undoing_a_paste_takes_the_unsaved_mark_away_at_once(cx: &mut TestAppContext) {
    let mut app = start(cx, Some(plan_file("clip-undo-mark", Starter::Api)), None);
    goto(&mut app, "get items");
    app.keys("ctrl-c ctrl-v");
    assert!(dirty(&mut app) && app.state()["tabs"][0]["dirty"].as_bool().unwrap());
    app.keys("ctrl-z");
    assert!(!dirty(&mut app), "nothing is left to save after the undo");
    assert!(!app.state()["tabs"][0]["dirty"].as_bool().unwrap(), "and the tab says so");
}

#[gpui::test]
fn pasting_with_a_container_selected_puts_the_copy_inside_it(cx: &mut TestAppContext) {
    let mut app = start(cx, Some(plan_file("clip-inside", Starter::Api)), None);
    goto(&mut app, "get items");
    app.keys("ctrl-c");
    goto(&mut app, "users");
    app.keys("ctrl-v");
    assert_eq!(names_under(&mut app, "Users"), ["Get items", "Think time", "Get items"], "last in the group");
}

#[gpui::test]
fn a_thread_group_pasted_with_a_request_selected_lands_under_the_test_plan(cx: &mut TestAppContext) {
    let mut app = start(cx, Some(plan_file("clip-group", Starter::Api)), None);
    goto(&mut app, "users");
    app.keys("ctrl-c");
    goto(&mut app, "get items");
    app.keys("ctrl-v");
    let top = names_under(&mut app, "clip-group");
    assert_eq!(top.iter().filter(|n| *n == "Users").count(), 2, "{top:?}");
    let users: Vec<_> = top.iter().enumerate().filter(|(_, n)| *n == "Users").map(|(i, _)| i).collect();
    assert_eq!(users[1], users[0] + 1, "right after the group it came from");
}

#[gpui::test]
fn text_that_is_not_elements_is_refused_with_a_message_and_changes_nothing(cx: &mut TestAppContext) {
    let mut app = start(cx, Some(plan_file("clip-junk", Starter::Api)), None);
    goto(&mut app, "get items");
    let before = app.plan_text();
    for junk in ["hello there", "{\"a\": 1}", "<html/>"] {
        set_clip(&mut app, junk);
        app.keys("ctrl-v");
        assert!(notice(&mut app).contains("not text copied from a JMeter plan") || notice(&mut app).contains("no plan elements"), "{junk}: {}", notice(&mut app));
    }
    assert_eq!(app.plan_text(), before);
    assert!(!dirty(&mut app));
}

#[gpui::test]
fn an_empty_clipboard_says_so(cx: &mut TestAppContext) {
    let mut app = start(cx, Some(plan_file("clip-empty", Starter::Api)), None);
    goto(&mut app, "get items");
    app.cx.write_to_clipboard(ClipboardItem::new_string(String::new()));
    app.keys("ctrl-v");
    assert!(notice(&mut app).contains("holds no text"), "{}", notice(&mut app));
    assert_eq!(elements(&mut app), 7);
}

#[gpui::test]
fn a_hostile_clipboard_is_refused_without_harm(cx: &mut TestAppContext) {
    let mut app = start(cx, Some(plan_file("clip-hostile", Starter::Api)), None);
    goto(&mut app, "users");
    let before = app.plan_text();
    let mut bomb = String::from("<?xml version=\"1.0\"?><!DOCTYPE lolz [<!ENTITY a0 \"lol\">");
    for i in 1..10 {
        bomb.push_str(&format!("<!ENTITY a{i} \"{}\">", format!("&a{};", i - 1).repeat(10)));
    }
    bomb.push_str("]><jmeterTestPlan><hashTree><ConstantTimer guiclass=\"ConstantTimerGui\" testclass=\"ConstantTimer\" testname=\"&a9;\"/><hashTree/></hashTree></jmeterTestPlan>");
    let deep = format!("<jmeterTestPlan>{}{}</jmeterTestPlan>", "<hashTree>".repeat(100_000), "</hashTree>".repeat(100_000));
    let external = "<?xml version=\"1.0\"?><!DOCTYPE p [<!ENTITY x SYSTEM \"file:///C:/Windows/win.ini\">]><jmeterTestPlan><hashTree><ConstantTimer guiclass=\"ConstantTimerGui\" testclass=\"ConstantTimer\" testname=\"&x;\"/><hashTree/></hashTree></jmeterTestPlan>";
    let t = Instant::now();
    for text in [bomb, deep, external.to_string(), "\u{0}\u{1}".repeat(10_000), "x".repeat(5_000_000)] {
        set_clip(&mut app, &text);
        app.keys("ctrl-v");
        let names = app.children_of("Users");
        assert!(names.iter().all(|n| !n.contains("lol") && !n.contains("[fonts]")), "{names:?}");
        if dirty(&mut app) {
            app.keys("ctrl-z");
        }
    }
    assert!(t.elapsed() < Duration::from_secs(40), "{:?}", t.elapsed());
    assert_eq!(app.plan_text(), before, "nothing stayed");
}

// ---- cut ----

#[gpui::test]
fn cut_removes_the_element_and_paste_or_undo_brings_it_back(cx: &mut TestAppContext) {
    let mut app = start(cx, Some(plan_file("clip-cut", Starter::Api)), None);
    goto(&mut app, "get items");
    let before = app.plan_text();
    app.keys("ctrl-x");
    assert_eq!(elements(&mut app), 5, "the request and its check are gone");
    assert!(clip(&mut app).unwrap().contains("testname=\"Get items\""));
    assert!(notice(&mut app).starts_with("Cut Get items"), "{}", notice(&mut app));
    app.keys("ctrl-v");
    assert_eq!(elements(&mut app), 7, "paste puts it back");
    app.keys("ctrl-z ctrl-z");
    assert_eq!(app.plan_text(), before, "and undo twice is the file as it was");
}

// ---- between plans ----

#[gpui::test]
fn an_element_copied_in_one_plan_is_pasted_into_another_and_the_first_is_untouched(cx: &mut TestAppContext) {
    let a = plan_file("clip-from", Starter::Api);
    let b = plan_file("clip-to", Starter::Web);
    let mut app = start(cx, Some(a.clone()), None);
    goto(&mut app, "get items");
    app.keys("ctrl-c");
    app.open_path(&b);
    assert_eq!(app.active_tab().as_deref(), Some("clip-to.jmx"));
    goto(&mut app, "visitors");
    app.keys("ctrl-v");
    assert_eq!(names_under(&mut app, "Visitors"), ["Home page", "Think time", "Get items"]);
    assert!(dirty(&mut app));
    app.click("tab-0");
    assert_eq!(app.active_tab().as_deref(), Some("clip-from.jmx"));
    assert!(!dirty(&mut app), "the plan it was copied from did not change");
    assert_eq!(std::fs::read_to_string(&a).unwrap(), starters::new_plan_text(Starter::Api, "clip-from").unwrap());
}

// ---- shortcuts belong to the box with the focus ----

#[gpui::test]
fn in_a_text_box_the_clipboard_keys_work_on_text_not_on_elements(cx: &mut TestAppContext) {
    let mut app = start(cx, Some(plan_file("clip-focus", Starter::Api)), None);
    goto(&mut app, "get items");
    app.click("field-HTTPSampler.path");
    app.keys(&secondary("ctrl-a ctrl-c"));
    assert_eq!(clip(&mut app).as_deref(), Some("/items"), "the box's text, not an element");
    app.keys("end");
    app.keys(&secondary("ctrl-v"));
    assert_eq!(elements(&mut app), 7, "text was pasted into the box, no element");
    app.keys("ctrl-x");
    assert_eq!(elements(&mut app), 7);
}

// ---- save a selection, merge ----

#[gpui::test]
fn a_selection_saved_on_its_own_can_be_merged_into_another_plan(cx: &mut TestAppContext) {
    let d = temp_dir("clip-save");
    let a = d.join("a.jmx");
    std::fs::write(&a, starters::new_plan_text(Starter::Api, "a").unwrap()).unwrap();
    let mut app = start(cx, Some(a), None);
    goto(&mut app, "users");
    let piece = d.join("piece");
    assert!(save_selection(&mut app, &piece));
    assert!(d.join("piece.jmx").is_file(), "the extension is added");
    let text = std::fs::read_to_string(d.join("piece.jmx")).unwrap();
    assert!(text.contains("testname=\"Users\"") && text.contains("testname=\"Get items\""));
    let b = d.join("b.jmx");
    std::fs::write(&b, starters::new_plan_text(Starter::Empty, "b").unwrap()).unwrap();
    app.open_path(&b);
    assert!(merge(&mut app, &d.join("piece.jmx")), "{}", notice(&mut app));
    assert_eq!(names_under(&mut app, "b"), ["Users"]);
    assert_eq!(elements(&mut app), 5, "the test plan, the group and the three under it");
}

#[gpui::test]
fn merging_a_whole_plan_brings_what_is_inside_its_test_plan_not_the_test_plan(cx: &mut TestAppContext) {
    let b = plan_file("clip-merge-b", Starter::Empty);
    let a = plan_file("clip-merge-a", Starter::Web);
    let mut app = start(cx, Some(b), None);
    assert!(merge(&mut app, &a), "{}", notice(&mut app));
    let top = names_under(&mut app, "clip-merge-b");
    assert_eq!(top, ["HTTP Request Defaults", "HTTP Cookie Manager", "HTTP Cache Manager", "HTTP Header Manager", "Visitors"]);
    assert_eq!(elements(&mut app), 9, "the test plan of the file that was merged did not come along");
    app.keys("ctrl-z");
    assert_eq!(elements(&mut app), 1, "the whole merge is one undo step");
}

#[gpui::test]
fn merging_something_that_is_not_a_plan_or_is_missing_is_refused(cx: &mut TestAppContext) {
    let mut app = start(cx, Some(plan_file("clip-merge-bad", Starter::Api)), None);
    let d = temp_dir("clip-merge-bad-files");
    std::fs::write(d.join("notes.txt"), "just some notes").unwrap();
    assert!(!merge(&mut app, &d.join("notes.txt")));
    assert!(!merge(&mut app, &d.join("missing.jmx")));
    assert!(notice(&mut app).contains("Cannot read"), "{}", notice(&mut app));
    assert_eq!(elements(&mut app), 7);
}

#[gpui::test]
fn a_selection_is_never_saved_over_an_open_plan_or_into_a_folder(cx: &mut TestAppContext) {
    let a = plan_file("clip-save-guard", Starter::Api);
    let mut app = start(cx, Some(a.clone()), None);
    goto(&mut app, "get items");
    let before = std::fs::read_to_string(&a).unwrap();
    assert!(!save_selection(&mut app, &a));
    assert!(notice(&mut app).contains("open in a tab"), "{}", notice(&mut app));
    assert_eq!(std::fs::read_to_string(&a).unwrap(), before, "the open plan is untouched");
    let d = temp_dir("clip-save-folder");
    std::fs::create_dir_all(d.join("folder.jmx")).unwrap();
    assert!(!save_selection(&mut app, &d.join("folder.jmx")));
    assert!(notice(&mut app).contains("is a folder"));
    app.click("tree-row-0");
    assert!(!save_selection(&mut app, &d.join("root.jmx")), "the Test Plan cannot be saved as a selection");
}

// ---- speed ----

#[gpui::test]
fn copying_and_pasting_a_big_group_through_the_window_is_quick(cx: &mut TestAppContext) {
    let d = temp_dir("clip-big");
    let p = d.join("big.jmx");
    let mut e = EditablePlan::open(&starters::new_plan_text(Starter::Api, "big").unwrap()).unwrap();
    let users = e.plan().find_by_name("Users").unwrap();
    for _ in 0..1500 {
        e.add(users, "HTTPSamplerProxy", "HttpTestSampleGui").unwrap();
    }
    std::fs::write(&p, e.to_text()).unwrap();
    let mut app = start(cx, Some(p), None);
    goto(&mut app, "users");
    let before = elements(&mut app);
    let t = Instant::now();
    app.keys("ctrl-c ctrl-v");
    assert_eq!(elements(&mut app), before + 1504, "the group and the 1,503 elements in it");
    app.keys("ctrl-z ctrl-y ctrl-z");
    assert_eq!(elements(&mut app), before);
    assert!(t.elapsed() < Duration::from_secs(30), "{:?}", t.elapsed());
}

// ---- a pasted request really runs ----

fn real_app<'a>(cx: &'a mut TestAppContext, name: &str) -> Option<(App<'a>, jm::Server, PathBuf, PathBuf)> {
    let env = jm::env()?;
    let server = jm::Server::start();
    let dir = temp_dir(name);
    let settings_path = dir.join("settings.json");
    settings::save(&settings_path, &Settings { jmeter_home: Some(env.home.to_string_lossy().into_owned()), ..Settings::default() }).unwrap();
    let aim = |text: String| {
        let mut e = EditablePlan::open(&text).unwrap();
        let defaults = e.plan().find_by_name("HTTP Request Defaults").unwrap();
        e.set_prop(defaults, "HTTPSampler.protocol", "stringProp", "http").unwrap();
        e.set_prop(defaults, "HTTPSampler.domain", "stringProp", "127.0.0.1").unwrap();
        e.set_prop(defaults, "HTTPSampler.port", "stringProp", &server.port.to_string()).unwrap();
        let users = e.plan().find_by_name("Users").unwrap();
        for (prop, v) in [("ThreadGroup.num_threads", "1"), ("ThreadGroup.ramp_time", "0"), ("ThreadGroup.main_controller/LoopController.loops", "2")] {
            e.set_prop(users, prop, "stringProp", v).unwrap();
        }
        e.set_prop(e.plan().find_by_name("Think time").unwrap(), "ConstantTimer.delay", "stringProp", "5").unwrap();
        e.to_text()
    };
    let from = dir.join("from.jmx");
    std::fs::write(&from, aim(starters::new_plan_text(Starter::Api, "from").unwrap())).unwrap();
    let to = dir.join("to.jmx");
    std::fs::write(&to, aim(starters::new_plan_text(Starter::Api, "to").unwrap())).unwrap();
    let app = start(cx, Some(from.clone()), Some(settings_path.clone()));
    let shell = app.shell.clone();
    app.cx.update(|_, cx| {
        shell.update(cx, |sh, _| {
            let home = settings::load(&settings_path).settings.jmeter_home.map(PathBuf::from);
            sh.runtime = jmeter_ui::shell::check_runtime(None, home.as_deref());
        })
    });
    app.cx.run_until_parked();
    Some((app, server, from, to))
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

#[gpui::test]
fn a_request_pasted_into_another_plan_runs_there(cx: &mut TestAppContext) {
    let Some((mut app, server, _from, to)) = real_app(cx, "clip-real") else { return };
    goto(&mut app, "get items");
    app.type_into("field-HTTPSampler.path", "/pasted");
    app.keys("ctrl-s");
    goto(&mut app, "get items");
    app.keys("ctrl-c");
    app.open_path(&to);
    goto(&mut app, "users");
    app.keys("ctrl-v");
    app.keys("ctrl-s");
    app.keys("f5");
    let st = wait_run(&mut app, Duration::from_secs(120));
    assert_eq!(st["run"]["phase"], "Finished", "{}", st["run"]);
    assert_eq!(st["run"]["samples"], 4, "two requests, two loops");
    assert_eq!(st["run"]["errors"], 0, "the pasted request kept its status check");
    let paths = server.paths.lock().unwrap().clone();
    assert_eq!(paths.iter().filter(|p| *p == "/pasted").count(), 2, "{paths:?}");
    assert_eq!(paths.iter().filter(|p| *p == "/items").count(), 2, "{paths:?}");
}
