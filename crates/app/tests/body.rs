//! Editing a request body and a script in a text area inside the form: showing the body only when the request
//! sends one, saving it with its lines, keeping text that one-line boxes cannot show from being changed, and that
//! a body reaches the server exactly as typed. Tests that run JMeter skip themselves without Java and JMeter.

mod common;
#[path = "../../jvm-runtime/tests/common/mod.rs"]
mod jm;

use common::*;
use gpui::TestAppContext;
use jmeter_ui::form::Widget;
use jmeter_ui::textarea::TextArea;
use plan_core::EditablePlan;
use plan_core::forms::{BODY, PARAMETERS, RAW_BODY};
use serde_json::Value;
use shell_core::actions::Action;
use shell_core::settings::{self, Settings};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const JSON: &str = "{\n  \"user\": \"demo\",\n  \"items\": [1, 2, 3],\n  \"note\": \"caf\u{e9} \u{1F600}\"\n}";

fn copy_of(name: &str) -> PathBuf {
    let d = temp_dir(name);
    let p = d.join(WEB);
    std::fs::copy(fixture(WEB), &p).unwrap();
    p
}

fn goto(app: &mut App, name: &str) {
    app.keys("ctrl-p");
    app.input(name);
    app.keys("enter");
}

fn with_request<'a>(cx: &'a mut TestAppContext, name: &str) -> App<'a> {
    let mut app = start(cx, Some(copy_of(name)), None);
    goto(&mut app, "jmeter users");
    app.keys("ctrl-k");
    app.input("+http request");
    app.keys("enter");
    assert_eq!(app.selected().as_deref(), Some("HTTP Request"));
    app
}

fn area(app: &mut App, prop: &str) -> Option<gpui::Entity<TextArea>> {
    let shell = app.shell.clone();
    let prop = prop.to_string();
    app.cx.update(|_, cx| {
        shell.read(cx).form.as_ref()?.sections.iter().flat_map(|s| s.items.iter()).find_map(|i| match (&i.widget, i.def.prop == prop) {
            (Widget::Area(a), true) => Some(a.clone()),
            _ => None,
        })
    })
}

fn area_text(app: &mut App, prop: &str) -> String {
    let a = area(app, prop).unwrap_or_else(|| panic!("no text area for {prop}"));
    app.cx.update(|_, cx| a.read(cx).text().to_string())
}

fn type_into_area(app: &mut App, selector: &'static str, text: &str) {
    app.click(selector);
    app.keys(&secondary("ctrl-a"));
    app.input(text);
}

/// Let the pause after typing pass, so what was typed is saved to the plan.
fn pause(app: &mut App) {
    app.cx.executor().advance_clock(Duration::from_millis(900));
    app.cx.run_until_parked();
}

fn rows(app: &mut App) -> usize {
    let shell = app.shell.clone();
    app.cx.update(|_, cx| {
        let s = shell.read(cx);
        let lp = s.plan().unwrap();
        let id = lp.tree.selected().unwrap();
        lp.edit.row_count(id, PARAMETERS)
    })
}

// ---- the body and the table take turns ----

#[gpui::test]
fn a_new_request_shows_the_parameters_table_and_no_body(cx: &mut TestAppContext) {
    let mut app = with_request(cx, "body-turns");
    assert!(app.has("add-row-HTTPsampler.Arguments/Arguments.arguments"));
    assert!(!app.has("area-HTTPsampler.Arguments/Arguments.arguments/#0/Argument.value"));
    assert!(area(&mut app, BODY).is_none());
}

#[gpui::test]
fn sending_the_parameters_as_a_body_swaps_the_table_for_a_text_area_and_back(cx: &mut TestAppContext) {
    let mut app = with_request(cx, "body-swap");
    app.click("check-HTTPSampler.postBodyRaw");
    assert_eq!(app.prop(RAW_BODY).as_deref(), Some("true"));
    assert!(app.has("area-HTTPsampler.Arguments/Arguments.arguments/#0/Argument.value"), "the body shows");
    assert!(!app.has("add-row-HTTPsampler.Arguments/Arguments.arguments"), "and the table goes");
    assert_eq!(rows(&mut app), 1, "the body is the first row of the parameters, made when the body was switched on");
    app.click("check-HTTPSampler.postBodyRaw");
    assert!(app.has("add-row-HTTPsampler.Arguments/Arguments.arguments"), "the table is back");
    assert!(area(&mut app, BODY).is_none());
}

#[gpui::test]
fn switching_the_body_on_and_off_is_undone_with_undo(cx: &mut TestAppContext) {
    let mut app = with_request(cx, "body-undo-toggle");
    let before = app.plan_text();
    app.click("check-HTTPSampler.postBodyRaw");
    app.click("tree-row-0");
    goto(&mut app, "http request");
    app.keys("ctrl-z ctrl-z");
    assert_eq!(app.plan_text(), before, "both the switch and the row it made are undone");
}

// ---- typing a body ----

#[gpui::test]
fn a_typed_body_keeps_its_lines_through_save_and_reload(cx: &mut TestAppContext) {
    let mut app = with_request(cx, "body-typed");
    app.click("check-HTTPSampler.postBodyRaw");
    type_into_area(&mut app, "area-HTTPsampler.Arguments/Arguments.arguments/#0/Argument.value", JSON);
    pause(&mut app);
    assert_eq!(app.prop(BODY).as_deref(), Some(JSON), "saved to the plan after the pause");
    assert!(app.state()["dirty"].as_bool().unwrap());
    app.keys("ctrl-s");
    let text = std::fs::read_to_string(app.state()["plan"].as_str().unwrap().trim_start_matches(r"\\?\")).unwrap();
    let e = EditablePlan::open(&text).unwrap();
    let id = e.plan().find_by_name("HTTP Request").unwrap();
    assert_eq!(e.prop(id, BODY).as_deref(), Some(JSON), "the file holds the lines");
    // Reload from disk: the area shows it again.
    let shell = app.shell.clone();
    app.cx.update(|window, cx| shell.update(cx, |s, cx| s.perform(Action::ReloadPlan, window, cx)));
    app.cx.run_until_parked();
    goto(&mut app, "http request");
    assert_eq!(area_text(&mut app, BODY), JSON);
}

#[gpui::test]
fn leaving_the_area_saves_the_body_without_waiting(cx: &mut TestAppContext) {
    let mut app = with_request(cx, "body-leave");
    app.click("check-HTTPSampler.postBodyRaw");
    app.click("area-HTTPsampler.Arguments/Arguments.arguments/#0/Argument.value");
    app.input("{\n}");
    goto(&mut app, "jmeter users");
    goto(&mut app, "http request");
    assert_eq!(app.prop(BODY).as_deref(), Some("{\n}"));
}

#[gpui::test]
fn typing_in_the_area_marks_the_plan_as_changed_at_once(cx: &mut TestAppContext) {
    let mut app = with_request(cx, "body-dirty");
    app.click("check-HTTPSampler.postBodyRaw");
    app.keys("ctrl-s");
    assert!(!app.state()["dirty"].as_bool().unwrap());
    app.click("area-HTTPsampler.Arguments/Arguments.arguments/#0/Argument.value");
    app.input("x");
    assert!(app.state()["dirty"].as_bool().unwrap(), "before the pause ends");
    assert!(app.state()["tabs"][0]["dirty"].as_bool().unwrap());
}

#[gpui::test]
fn a_body_that_looks_like_markup_stays_text_in_the_file(cx: &mut TestAppContext) {
    let mut app = with_request(cx, "body-markup");
    app.click("check-HTTPSampler.postBodyRaw");
    let body = "<soap:Envelope xmlns:soap=\"x\">\n  <a>&amp; ]]> <![CDATA[x]]></a>\n</soap:Envelope>\n</stringProp><evil/>";
    type_into_area(&mut app, "area-HTTPsampler.Arguments/Arguments.arguments/#0/Argument.value", body);
    pause(&mut app);
    let e = EditablePlan::open(&app.plan_text()).unwrap();
    let id = e.plan().find_by_name("HTTP Request").unwrap();
    assert_eq!(e.prop(id, BODY).as_deref(), Some(body));
    assert!(e.plan().find_by_name("evil").is_none() && e.plan().len() == app.state()["elements"].as_u64().unwrap() as usize);
}

#[gpui::test]
fn characters_a_plan_cannot_hold_never_reach_the_file(cx: &mut TestAppContext) {
    let mut app = with_request(cx, "body-control");
    app.click("check-HTTPSampler.postBodyRaw");
    type_into_area(&mut app, "area-HTTPsampler.Arguments/Arguments.arguments/#0/Argument.value", "a\u{0}b\u{1}c\u{b}d");
    pause(&mut app);
    assert_eq!(app.prop(BODY).as_deref(), Some("abcd"));
    assert!(app.plan_text().chars().all(|c| matches!(c, '\t' | '\n' | '\r') || (c as u32) >= 0x20), "a control character reached the file");
}

#[gpui::test]
fn a_very_big_body_is_saved_and_the_window_stays_usable(cx: &mut TestAppContext) {
    let mut app = with_request(cx, "body-big");
    app.click("check-HTTPSampler.postBodyRaw");
    let big: String = (0..20_000).map(|i| format!("{{\"id\": {i}, \"v\": \"${{x}}\"}}\n")).collect();
    let a = area(&mut app, BODY).unwrap();
    let t = Instant::now();
    app.cx.update(|window, cx| a.update(cx, |a, cx| a.focus(window, cx)));
    app.cx.write_to_clipboard(gpui::ClipboardItem::new_string(big.clone()));
    app.keys(&secondary("ctrl-v"));
    pause(&mut app);
    assert_eq!(app.prop(BODY).map(|b| b.len()), Some(big.len()));
    app.keys("ctrl-s");
    assert!(t.elapsed() < Duration::from_secs(20), "a 20,000-line body took {:?}", t.elapsed());
}

// ---- scripts ----

#[gpui::test]
fn a_script_is_edited_with_its_lines_and_saved(cx: &mut TestAppContext) {
    let mut app = start(cx, Some(copy_of("body-script")), None);
    goto(&mut app, "jmeter users");
    app.keys("ctrl-k");
    app.input("+jsr223 sampler");
    app.keys("enter");
    assert!(area(&mut app, "script").is_some(), "the script has a text area");
    let script = "def a = 1\nif (a == 1) {\n  log.info('one')\n}\nreturn a";
    type_into_area(&mut app, "area-script", script);
    pause(&mut app);
    assert_eq!(app.prop("script").as_deref(), Some(script));
    let e = EditablePlan::open(&app.plan_text()).unwrap();
    let id = e.plan().find_by_name("JSR223 Sampler").unwrap();
    assert_eq!(e.prop(id, "script").as_deref(), Some(script));
}

// ---- text that one-line boxes cannot show ----

#[gpui::test]
fn a_comment_with_several_lines_is_not_flattened_by_looking_at_it(cx: &mut TestAppContext) {
    let d = temp_dir("comment-lines");
    let p = d.join("c.jmx");
    let mut e = EditablePlan::open(&plan_core::starters::new_plan_text(plan_core::starters::Starter::Api, "c").unwrap()).unwrap();
    let id = e.plan().find_by_name("Get items").unwrap();
    e.set_prop(id, "TestPlan.comments", "stringProp", "line one\nline two\nline three").unwrap();
    std::fs::write(&p, e.to_text()).unwrap();
    let before = std::fs::read_to_string(&p).unwrap();
    let mut app = start(cx, Some(p.clone()), None);
    goto(&mut app, "get items");
    assert!(!app.state()["dirty"].as_bool().unwrap(), "selecting an element must not change it");
    // Doing other things flushes the form: the comment must come through untouched.
    app.click("field-HTTPSampler.path");
    app.keys("tab");
    app.keys("ctrl-s");
    goto(&mut app, "users");
    goto(&mut app, "get items");
    assert_eq!(app.prop("TestPlan.comments").as_deref(), Some("line one\nline two\nline three"));
    assert_eq!(std::fs::read_to_string(&p).unwrap(), before, "nothing was rewritten");
}

#[gpui::test]
fn a_box_that_cannot_show_all_the_lines_is_read_only_so_typing_cannot_lose_them(cx: &mut TestAppContext) {
    let d = temp_dir("comment-readonly");
    let p = d.join("c.jmx");
    let mut e = EditablePlan::open(&plan_core::starters::new_plan_text(plan_core::starters::Starter::Api, "c").unwrap()).unwrap();
    let id = e.plan().find_by_name("Get items").unwrap();
    e.set_prop(id, "TestPlan.comments", "stringProp", "keep\nthese\nlines").unwrap();
    std::fs::write(&p, e.to_text()).unwrap();
    let mut app = start(cx, Some(p), None);
    goto(&mut app, "get items");
    app.click("field-comments");
    app.input("typed");
    app.keys("enter");
    assert_eq!(app.prop("TestPlan.comments").as_deref(), Some("keep\nthese\nlines"));
    // An ordinary comment is still editable.
    goto(&mut app, "users");
    app.type_into("field-comments", "plain");
    assert_eq!(app.prop("TestPlan.comments").as_deref(), Some("plain"));
}

#[gpui::test]
fn a_table_value_with_lines_is_not_flattened_either(cx: &mut TestAppContext) {
    let d = temp_dir("cell-lines");
    let p = d.join("c.jmx");
    let mut e = EditablePlan::open(&plan_core::starters::new_plan_text(plan_core::starters::Starter::Api, "c").unwrap()).unwrap();
    let id = e.plan().find_by_name("Get items").unwrap();
    e.add_row(id, PARAMETERS, 0, plan_core::forms::new_row(plan_core::forms::RowShape::HttpArgument, &["q", "first\nsecond"])).unwrap();
    std::fs::write(&p, e.to_text()).unwrap();
    let before = std::fs::read_to_string(&p).unwrap();
    let mut app = start(cx, Some(p.clone()), None);
    goto(&mut app, "get items");
    assert!(!app.state()["dirty"].as_bool().unwrap());
    app.click("field-HTTPSampler.path");
    app.keys("tab");
    app.keys("ctrl-s");
    assert_eq!(std::fs::read_to_string(&p).unwrap(), before, "the value with two lines is still two lines");
}

// ---- a body reaches the server as typed ----

fn real_app<'a>(cx: &'a mut TestAppContext, name: &str) -> Option<(App<'a>, jm::Server, PathBuf)> {
    let env = jm::env()?;
    let server = jm::Server::start();
    let dir = temp_dir(name);
    let settings_path = dir.join("settings.json");
    settings::save(&settings_path, &Settings { jmeter_home: Some(env.home.to_string_lossy().into_owned()), ..Settings::default() }).unwrap();
    let plan = dir.join("body.jmx");
    let mut e = EditablePlan::open(&plan_core::starters::new_plan_text(plan_core::starters::Starter::Api, "body").unwrap()).unwrap();
    let defaults = e.plan().find_by_name("HTTP Request Defaults").unwrap();
    e.set_prop(defaults, "HTTPSampler.protocol", "stringProp", "http").unwrap();
    e.set_prop(defaults, "HTTPSampler.domain", "stringProp", "127.0.0.1").unwrap();
    e.set_prop(defaults, "HTTPSampler.port", "stringProp", &server.port.to_string()).unwrap();
    let users = e.plan().find_by_name("Users").unwrap();
    for (prop, v) in [("ThreadGroup.num_threads", "1"), ("ThreadGroup.ramp_time", "0"), ("ThreadGroup.main_controller/LoopController.loops", "2")] {
        e.set_prop(users, prop, "stringProp", v).unwrap();
    }
    let think = e.plan().find_by_name("Think time").unwrap();
    e.set_prop(think, "ConstantTimer.delay", "stringProp", "5").unwrap();
    let req = e.plan().find_by_name("Get items").unwrap();
    e.set_prop(req, "HTTPSampler.method", "stringProp", "POST").unwrap();
    e.set_prop(req, "HTTPSampler.path", "stringProp", "/items").unwrap();
    std::fs::write(&plan, e.to_text()).unwrap();
    let app = start(cx, Some(plan.clone()), Some(settings_path.clone()));
    let shell = app.shell.clone();
    app.cx.update(|_, cx| {
        shell.update(cx, |sh, _| {
            let home = settings::load(&settings_path).settings.jmeter_home.map(PathBuf::from);
            sh.runtime = jmeter_ui::shell::check_runtime(None, home.as_deref());
        })
    });
    app.cx.run_until_parked();
    Some((app, server, plan))
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
fn a_body_typed_in_the_window_reaches_the_server_exactly_as_typed(cx: &mut TestAppContext) {
    let Some((mut app, server, _plan)) = real_app(cx, "body-real") else { return };
    goto(&mut app, "get items");
    app.click("check-HTTPSampler.postBodyRaw");
    type_into_area(&mut app, "area-HTTPsampler.Arguments/Arguments.arguments/#0/Argument.value", JSON);
    app.keys("ctrl-s");
    app.keys("f5");
    let st = wait_run(&mut app, Duration::from_secs(120));
    assert_eq!(st["run"]["phase"], "Finished", "{}", st["run"]);
    assert_eq!(st["run"]["samples"], 2);
    let bodies = server.bodies.lock().unwrap().clone();
    assert_eq!(bodies.len(), 2, "{bodies:?}");
    for b in &bodies {
        assert_eq!(b, JSON, "the server got the body as typed, with its lines and its accents");
    }
}

#[gpui::test]
fn a_body_with_a_variable_is_filled_in_before_it_is_sent(cx: &mut TestAppContext) {
    let Some((mut app, server, _plan)) = real_app(cx, "body-variable") else { return };
    goto(&mut app, "get items");
    app.click("check-HTTPSampler.postBodyRaw");
    type_into_area(&mut app, "area-HTTPsampler.Arguments/Arguments.arguments/#0/Argument.value", "{\n  \"id\": ${__threadNum}\n}");
    app.keys("ctrl-s");
    app.keys("f5");
    let st = wait_run(&mut app, Duration::from_secs(120));
    assert_eq!(st["run"]["samples"], 2, "{}", st["run"]);
    let bodies = server.bodies.lock().unwrap().clone();
    assert!(bodies.iter().all(|b| b == "{\n  \"id\": 1\n}"), "{bodies:?}");
}

#[allow(dead_code)]
fn unused(_: &Path) {}
