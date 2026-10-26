//! Import from cURL in the window: the dialog, what gets added and where, what is refused, and that imported
//! requests reach a real server as the command described them. Tests that run JMeter skip themselves without Java and
//! JMeter.

mod common;
#[path = "../../jvm-runtime/tests/common/mod.rs"]
mod jm;

use common::*;
use gpui::{ClipboardItem, TestAppContext};
use jmeter_ui::shell::Dialog;
use jmeter_ui::textarea::TextArea;
use plan_core::EditablePlan;
use plan_core::starters::{self, Starter};
use serde_json::Value;
use shell_core::settings::{self, Settings};
use std::path::PathBuf;
use std::time::{Duration, Instant};

fn plan_file(name: &str) -> PathBuf {
    let d = temp_dir(name);
    let p = d.join(format!("{name}.jmx"));
    std::fs::write(&p, starters::new_plan_text(Starter::Api, name).unwrap()).unwrap();
    p
}

fn goto(app: &mut App, name: &str) {
    app.keys("ctrl-p");
    app.input(name);
    app.keys("enter");
}

fn dialog_kind(app: &mut App) -> Option<String> {
    app.state()["dialog"]["kind"].as_str().map(str::to_string)
}

fn dialog_error(app: &mut App) -> Option<String> {
    app.state()["dialog"]["error"].as_str().map(str::to_string)
}

fn notice(app: &mut App) -> String {
    app.state()["notice"].as_str().unwrap_or_default().to_string()
}

fn elements(app: &mut App) -> u64 {
    app.state()["elements"].as_u64().unwrap()
}

fn area(app: &mut App) -> gpui::Entity<TextArea> {
    let shell = app.shell.clone();
    app.cx.update(|_, cx| match &shell.read(cx).dialog {
        Some(Dialog::Curl { area, .. }) => area.clone(),
        other => panic!("the cURL dialog is not open: {other:?}"),
    })
}

fn area_text(app: &mut App) -> String {
    let a = area(app);
    app.cx.update(|_, cx| a.read(cx).text().to_string())
}

/// Open the dialog, type the command into it and press Add to Plan.
fn import(app: &mut App, command: &str) {
    app.keys("ctrl-shift-i");
    assert_eq!(dialog_kind(app).as_deref(), Some("curl"));
    let a = area(app);
    app.cx.update(|_, cx| a.update(cx, |a, cx| a.set_text(command, cx)));
    app.click("dialog-curl-add");
}

// ---- the dialog ----

#[gpui::test]
fn ctrl_shift_i_opens_the_dialog_and_cancel_closes_it(cx: &mut TestAppContext) {
    let mut app = start(cx, Some(plan_file("curl-open")), None);
    app.keys("ctrl-shift-i");
    assert_eq!(dialog_kind(&mut app).as_deref(), Some("curl"));
    assert!(app.has("curl-text") && app.has("dialog-curl-add") && app.has("dialog-cancel"));
    assert_eq!(area_text(&mut app), "", "nothing to fill in from an empty clipboard");
    app.click("dialog-cancel");
    assert!(dialog_kind(&mut app).is_none());
    assert_eq!(elements(&mut app), 7, "nothing was added");
}

#[gpui::test]
fn import_from_curl_is_in_the_palette_and_needs_a_plan(cx: &mut TestAppContext) {
    let mut app = start(cx, None, None);
    app.keys("ctrl-shift-i");
    assert!(dialog_kind(&mut app).is_none(), "without a plan there is nowhere to add to");
    app.keys("ctrl-k");
    app.input(">import from curl");
    assert!(app.palette_unavailable().contains(&"Import from cURL\u{2026}".to_string()));
    app.keys("escape");
    app.open_path(&plan_file("curl-palette"));
    app.keys("ctrl-k");
    app.input(">import from curl");
    assert_eq!(app.palette_titles().first().map(String::as_str), Some("Import from cURL\u{2026}"));
    assert!(!app.palette_unavailable().contains(&"Import from cURL\u{2026}".to_string()));
}

#[gpui::test]
fn the_dialog_is_filled_from_the_clipboard_only_when_it_holds_a_curl_command(cx: &mut TestAppContext) {
    let mut app = start(cx, Some(plan_file("curl-clip")), None);
    app.cx.write_to_clipboard(ClipboardItem::new_string("curl 'https://x.org/a' -H 'a: b'".into()));
    app.keys("ctrl-shift-i");
    assert_eq!(area_text(&mut app), "curl 'https://x.org/a' -H 'a: b'");
    app.click("dialog-cancel");
    app.cx.write_to_clipboard(ClipboardItem::new_string("some other text".into()));
    app.keys("ctrl-shift-i");
    assert_eq!(area_text(&mut app), "");
}

// ---- adding ----

#[gpui::test]
fn adding_a_command_puts_the_request_under_the_selected_group_and_selects_it(cx: &mut TestAppContext) {
    let mut app = start(cx, Some(plan_file("curl-add")), None);
    goto(&mut app, "users");
    import(&mut app, "curl 'https://api.example.com/login' -H 'content-type: application/json' -H 'x-id: 7' --data-raw '{\"user\":\"demo\"}'");
    assert!(dialog_kind(&mut app).is_none(), "{:?}", dialog_error(&mut app));
    assert_eq!(app.children_of("Users").last().map(String::as_str), Some("POST /login"));
    assert_eq!(app.selected().as_deref(), Some("POST /login"));
    assert_eq!(elements(&mut app), 9, "the request and its header manager");
    assert_eq!(notice(&mut app), "Added 1 request from cURL");
    assert!(app.state()["dirty"].as_bool().unwrap());
    // The body and the headers are in the form.
    assert_eq!(app.prop("HTTPSampler.domain").as_deref(), Some("api.example.com"));
    assert_eq!(app.prop("HTTPSampler.postBodyRaw").as_deref(), Some("true"));
    assert!(app.plan_text().contains("{\"user\":\"demo\"}") && app.plan_text().contains("x-id"));
    app.keys("ctrl-z");
    assert_eq!(elements(&mut app), 7, "one undo takes the request and its headers away");
}

#[gpui::test]
fn the_imported_request_shows_its_body_in_the_text_area(cx: &mut TestAppContext) {
    let mut app = start(cx, Some(plan_file("curl-form")), None);
    goto(&mut app, "users");
    import(&mut app, "curl https://x.org/a -H 'content-type: application/json' --data-raw $'{\\n  \"a\": 1\\n}'");
    assert!(app.has("area-HTTPsampler.Arguments/Arguments.arguments/#0/Argument.value"), "the body editor shows");
    assert!(!app.has("add-row-HTTPsampler.Arguments/Arguments.arguments"));
}

#[gpui::test]
fn several_commands_are_added_together_in_order_with_one_undo(cx: &mut TestAppContext) {
    let mut app = start(cx, Some(plan_file("curl-many")), None);
    goto(&mut app, "users");
    import(&mut app, "curl https://x.org/one\ncurl -X DELETE https://x.org/two\ncurl https://x.org/three -d 'a=1'");
    let kids = app.children_of("Users");
    assert_eq!(kids[kids.len() - 3..], ["GET /one", "DELETE /two", "POST /three"]);
    assert_eq!(notice(&mut app), "Added 3 requests from cURL");
    app.keys("ctrl-z");
    assert_eq!(elements(&mut app), 7);
}

#[gpui::test]
fn what_was_left_out_is_written_in_the_requests_comment(cx: &mut TestAppContext) {
    let mut app = start(cx, Some(plan_file("curl-notes")), None);
    goto(&mut app, "users");
    import(&mut app, "curl -u demo:s3cret -H 'Host: x.org' -H 'Content-Length: 4' https://x.org/private -d @/etc/passwd");
    let comment = app.prop("TestPlan.comments").unwrap_or_default();
    assert!(comment.contains("Replace them with variables"), "{comment}");
    assert!(comment.contains("Content-Length") && comment.contains("files are not read"), "{comment}");
    assert!(app.plan_text().contains("Basic ZGVtbzpzM2NyZXQ="));
    assert!(!app.plan_text().contains("/etc/passwd\n") || !app.plan_text().contains("root:"));
}

#[gpui::test]
fn with_a_request_selected_the_new_one_goes_right_after_it(cx: &mut TestAppContext) {
    let mut app = start(cx, Some(plan_file("curl-near")), None);
    goto(&mut app, "get items");
    import(&mut app, "curl https://x.org/near");
    assert_eq!(app.children_of("Users"), ["Get items", "GET /near", "Think time"]);
}

// ---- what is refused ----

#[gpui::test]
fn text_with_no_command_or_an_address_that_is_not_http_keeps_the_dialog_open_with_a_message(cx: &mut TestAppContext) {
    let mut app = start(cx, Some(plan_file("curl-refuse")), None);
    goto(&mut app, "users");
    for (text, expect) in [("", "no curl command"), ("   \n  ", "no curl command"), ("curl", "no address"), ("curl file:///etc/passwd", "only http and https"), ("curl ftp://x.org/a", "only http and https"), ("curl http://", "no server name")] {
        import(&mut app, text);
        assert_eq!(dialog_kind(&mut app).as_deref(), Some("curl"), "{text:?} closed the dialog");
        let err = dialog_error(&mut app).unwrap_or_default();
        assert!(err.contains(expect), "{text:?}: {err}");
        assert!(app.has("curl-error"), "the problem shows in the dialog");
        app.click("dialog-cancel");
    }
    assert_eq!(elements(&mut app), 7, "nothing was added by any of them");
    assert!(!app.state()["dirty"].as_bool().unwrap());
}

#[gpui::test]
fn with_the_test_plan_selected_the_request_goes_under_the_first_thread_group(cx: &mut TestAppContext) {
    let mut app = start(cx, Some(plan_file("curl-aimed")), None);
    app.click("tree-row-0"); // the Test Plan takes no requests itself
    import(&mut app, "curl https://x.org/a");
    assert_eq!(dialog_kind(&mut app), None, "{:?}", dialog_error(&mut app));
    assert_eq!(elements(&mut app), 8);
    assert!(app.children_of("Users").iter().any(|n| n == "GET /a"), "{:?}", app.children_of("Users"));
}

#[gpui::test]
fn a_plan_with_nowhere_to_put_a_request_explains_and_nothing_is_added(cx: &mut TestAppContext) {
    let d = temp_dir("curl-nowhere");
    let p = d.join("empty.jmx");
    std::fs::write(&p, starters::new_plan_text(Starter::Empty, "empty").unwrap()).unwrap();
    let mut app = start(cx, Some(p), None);
    app.click("tree-row-0");
    let before = elements(&mut app);
    import(&mut app, "curl https://x.org/a");
    assert_eq!(dialog_kind(&mut app).as_deref(), Some("curl"));
    assert!(dialog_error(&mut app).unwrap_or_default().contains("cannot go under"), "{:?}", dialog_error(&mut app));
    assert_eq!(elements(&mut app), before);
}

#[gpui::test]
fn shell_tricks_in_a_command_are_text_and_nothing_runs(cx: &mut TestAppContext) {
    let mut app = start(cx, Some(plan_file("curl-tricks")), None);
    goto(&mut app, "users");
    let marker = std::env::temp_dir().join(format!("jmeter-ui-curl-marker-{}", std::process::id()));
    let _ = std::fs::remove_file(&marker);
    import(&mut app, &format!("curl 'https://x.org/$(echo pwned > {m})' -H \"X: `echo pwned > {m}`\" -d 'a=$(echo pwned > {m})'", m = marker.display()));
    assert!(!marker.exists(), "no command was run");
    assert!(app.plan_text().contains("$(echo pwned"), "the text is kept as text");
}

#[gpui::test]
fn a_huge_paste_into_the_dialog_is_refused_without_a_hang(cx: &mut TestAppContext) {
    let mut app = start(cx, Some(plan_file("curl-huge")), None);
    goto(&mut app, "users");
    let t = Instant::now();
    import(&mut app, &format!("curl https://x.org/a -d '{}'", "x".repeat(3_000_000)));
    let big_ok = dialog_kind(&mut app).is_none();
    if !big_ok {
        app.click("dialog-cancel");
    }
    import(&mut app, &"curl https://x.org/a\n".repeat(500));
    assert!(dialog_error(&mut app).unwrap_or_default().contains("at most"), "{:?}", dialog_error(&mut app));
    app.click("dialog-cancel");
    assert!(t.elapsed() < Duration::from_secs(60), "{:?}", t.elapsed());
}

// ---- imported requests reach a server as described ----

fn real_app<'a>(cx: &'a mut TestAppContext, name: &str) -> Option<(App<'a>, jm::Server)> {
    let env = jm::env()?;
    let server = jm::Server::start();
    let dir = temp_dir(name);
    let settings_path = dir.join("settings.json");
    settings::save(&settings_path, &Settings { jmeter_home: Some(env.home.to_string_lossy().into_owned()), ..Settings::default() }).unwrap();
    let plan = dir.join("curl.jmx");
    let mut e = EditablePlan::open(&starters::new_plan_text(Starter::Empty, "curl").unwrap()).unwrap();
    let root = e.plan().roots()[0];
    let g = e.add(root, "ThreadGroup", "ThreadGroupGui").unwrap();
    e.rename(g, "Users").unwrap();
    for (prop, v) in [("ThreadGroup.num_threads", "1"), ("ThreadGroup.ramp_time", "0"), ("ThreadGroup.main_controller/LoopController.loops", "1")] {
        e.set_prop(g, prop, "stringProp", v).unwrap();
    }
    std::fs::write(&plan, e.to_text()).unwrap();
    let app = start(cx, Some(plan), Some(settings_path.clone()));
    let shell = app.shell.clone();
    app.cx.update(|_, cx| {
        shell.update(cx, |sh, _| {
            let home = settings::load(&settings_path).settings.jmeter_home.map(PathBuf::from);
            sh.runtime = jmeter_ui::shell::check_runtime(None, home.as_deref());
        })
    });
    app.cx.run_until_parked();
    Some((app, server))
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
fn imported_requests_send_what_the_commands_said(cx: &mut TestAppContext) {
    let Some((mut app, server)) = real_app(cx, "curl-real") else { return };
    goto(&mut app, "users");
    let p = server.port;
    let cmd = format!(
        "curl 'http://127.0.0.1:{p}/api/login?src=cli' -X POST -H 'Content-Type: application/json' -H 'X-Demo: 1' --data-raw $'{{\\n  \"user\": \"demo\",\\n  \"n\": \"caf\\u00e9\"\\n}}'\n\
         curl http://127.0.0.1:{p}/form -d 'a=1&b=two+words&c=%26'\n\
         curl -u demo:s3cret 'http://127.0.0.1:{p}/private?x=1' -H 'Accept: text/plain'\n\
         curl -X DELETE http://127.0.0.1:{p}/items/7"
    );
    import(&mut app, &cmd);
    assert!(dialog_kind(&mut app).is_none(), "{:?}", dialog_error(&mut app));
    app.keys("ctrl-s");
    app.keys("f5");
    let st = wait_run(&mut app, Duration::from_secs(120));
    assert_eq!(st["run"]["phase"], "Finished", "{}", st["run"]);
    assert_eq!(st["run"]["samples"], 4);
    assert_eq!(st["run"]["errors"], 0);
    let heads = server.heads.lock().unwrap().clone();
    let bodies = server.bodies.lock().unwrap().clone();
    assert!(heads[0].starts_with("POST /api/login?src=cli HTTP/1.1"), "{}", heads[0]);
    assert!(heads[0].to_lowercase().contains("content-type: application/json"), "{}", heads[0]);
    assert!(heads[0].contains("X-Demo: 1"), "{}", heads[0]);
    assert_eq!(bodies[0], "{\n  \"user\": \"demo\",\n  \"n\": \"caf\u{e9}\"\n}", "the JSON body arrived as the command had it");
    assert!(heads[1].starts_with("POST /form "), "{}", heads[1]);
    let form: Vec<&str> = bodies[1].split('&').collect();
    assert!(form.contains(&"a=1") && form.contains(&"c=%26") && form.iter().any(|f| *f == "b=two+words" || *f == "b=two%20words"), "{:?}", bodies[1]);
    assert!(heads[2].starts_with("GET /private?x=1 "), "{}", heads[2]);
    assert!(heads[2].contains("Authorization: Basic ZGVtbzpzM2NyZXQ="), "{}", heads[2]);
    assert!(heads[2].contains("Accept: text/plain"));
    assert!(heads[3].starts_with("DELETE /items/7 "), "{}", heads[3]);
}
