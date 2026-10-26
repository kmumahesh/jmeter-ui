//! Importing a HAR file from the window: what the dialog shows, the choices, how the requests land in the plan, and what
//! must not happen with bad or hostile files.

mod common;

use common::*;
use gpui::TestAppContext;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

fn entry(page: &str, method: &str, url: &str, mime: &str, extra: &str) -> String {
    format!(r#"{{"pageref":"{page}","request":{{"method":"{method}","url":"{url}","headers":[{{"name":"Accept","value":"*/*"}},{{"name":"Cookie","value":"sid=secret"}}]{extra}}},"response":{{"content":{{"mimeType":"{mime}"}}}}}}"#)
}

fn shop_har() -> String {
    let entries = [
        entry("p1", "GET", "https://shop.example.com/", "text/html", ""),
        entry("p1", "GET", "https://shop.example.com/logo.png", "image/png", ""),
        entry("p1", "GET", "https://cdn.example.net/app.js", "application/javascript", ""),
        entry("p1", "GET", "https://shop.example.com/api/items", "application/json", ""),
        entry("p2", "POST", "https://shop.example.com/api/login", "application/json", r#","postData":{"mimeType":"application/json","text":"{\"user\":\"demo\"}"}"#),
    ]
    .join(",");
    format!(r#"{{"log":{{"pages":[{{"id":"p1","title":"Home"}},{{"id":"p2","title":"Sign in"}}],"entries":[{entries}]}}}}"#)
}

struct Setup {
    dir: PathBuf,
    plan: PathBuf,
}

fn setup(name: &str) -> Setup {
    let dir = temp_dir(name);
    let plan = dir.join("web.jmx");
    std::fs::copy(fixture(WEB), &plan).unwrap();
    Setup { dir, plan }
}

fn open_har(app: &mut App, path: &Path) {
    let shell = app.shell.clone();
    let path = path.to_path_buf();
    app.cx.update(|window, cx| shell.update(cx, |s, cx| s.open_har_path(&path, window, cx)));
    app.cx.run_until_parked();
}

fn dialog(app: &mut App) -> Value {
    app.state()["dialog"].clone()
}

fn goto(app: &mut App, name: &str) {
    app.keys("ctrl-p");
    app.input(name);
    app.keys("enter");
}

fn elements(app: &mut App) -> u64 {
    app.state()["elements"].as_u64().unwrap()
}

#[gpui::test]
fn the_dialog_shows_what_the_file_holds_and_what_is_left_out(cx: &mut TestAppContext) {
    let s = setup("har-dialog");
    std::fs::write(s.dir.join("shop.har"), shop_har()).unwrap();
    let mut app = start(cx, Some(s.plan.clone()), None);
    open_har(&mut app, &s.dir.join("shop.har"));
    let d = dialog(&mut app);
    assert_eq!(d["kind"], "har", "{d}");
    assert_eq!((d["total"].clone(), d["kept"].clone()), (5.into(), 3.into()));
    assert!(d["left_out"].as_str().unwrap().contains("2 images, styles, scripts and fonts"), "{d}");
    for sel in ["har-summary", "har-skip-static", "har-group", "har-hosts", "dialog-har-add", "dialog-cancel"] {
        assert!(app.has(sel), "{sel}");
    }
    // The choices change what will be brought in.
    app.click("har-skip-static");
    let d = dialog(&mut app);
    assert_eq!((d["skip_static"].clone(), d["kept"].clone(), d["left_out"].clone()), (false.into(), 5.into(), "".into()));
    app.click("har-skip-static");
    app.click("har-group");
    assert_eq!(dialog(&mut app)["group_by_page"], false);
    app.type_into("har-hosts", "cdn.example.net");
    let d = dialog(&mut app);
    assert_eq!(d["kept"], 0, "the only static file of that server is left out: {d}");
    assert!(d["error"].as_str().unwrap().contains("Nothing is left"), "{d}");
    app.type_into("har-hosts", "SHOP.example.com");
    assert_eq!(dialog(&mut app)["kept"], 3);
    app.click("dialog-cancel");
    assert!(app.state()["dialog"].is_null());
}

#[gpui::test]
fn adding_puts_the_requests_in_the_plan_grouped_by_page_in_one_undoable_step(cx: &mut TestAppContext) {
    let s = setup("har-add");
    std::fs::write(s.dir.join("shop.har"), shop_har()).unwrap();
    let mut app = start(cx, Some(s.plan.clone()), None);
    goto(&mut app, "Home Page");
    let before = elements(&mut app);
    open_har(&mut app, &s.dir.join("shop.har"));
    app.click("dialog-har-add");
    assert!(app.state()["dialog"].is_null());
    assert!(app.state()["notice"].as_str().unwrap().contains("Added 3 requests from the HAR file"));
    // Two pages (controllers) and three requests.
    assert_eq!(elements(&mut app), before + 8, "two pages, three requests and a header manager under each");
    let text = app.plan_text();
    for want in ["testname=\"Home\"", "testname=\"Sign in\"", "testname=\"POST /api/login\"", "testname=\"GET /api/items\"", "shop.example.com"] {
        assert!(text.contains(want), "{want}");
    }
    assert!(!text.contains("sid=secret") && !text.contains("logo.png") && !text.contains("app.js"), "cookies and static files are not brought in");
    // One undo takes it all away.
    app.keys(&secondary("ctrl-z"));
    let back = elements(&mut app);
    assert_eq!(back, before, "one undo takes it all away");
}

#[gpui::test]
fn files_that_are_not_har_files_are_refused_or_explained(cx: &mut TestAppContext) {
    let s = setup("har-bad");
    let mut app = start(cx, Some(s.plan.clone()), None);
    let notice = |app: &mut App| app.state()["notice"].as_str().unwrap_or("").to_string();
    open_har(&mut app, &s.dir.join("missing.har"));
    assert!(notice(&mut app).contains("cannot be read"));
    std::fs::write(s.dir.join("empty.har"), "").unwrap();
    open_har(&mut app, &s.dir.join("empty.har"));
    assert!(notice(&mut app).contains("is empty"));
    std::fs::write(s.dir.join("bin.har"), [0xff, 0xfe, 0x00, 0x80]).unwrap();
    open_har(&mut app, &s.dir.join("bin.har"));
    assert!(notice(&mut app).contains("not text"));
    assert!(app.state()["dialog"].is_null());
    // Text that is not a HAR file opens the dialog with the reason, and adding does nothing.
    std::fs::write(s.dir.join("notes.har"), "just some notes").unwrap();
    open_har(&mut app, &s.dir.join("notes.har"));
    let d = dialog(&mut app);
    assert!(d["error"].as_str().unwrap().contains("not a HAR file"), "{d}");
    let before = elements(&mut app);
    app.click("dialog-har-add");
    assert_eq!(elements(&mut app), before);
    assert_eq!(dialog(&mut app)["kind"], "har", "the dialog stays for another try");
}

#[gpui::test]
fn without_a_plan_there_is_nothing_to_add_to(cx: &mut TestAppContext) {
    let dir = temp_dir("har-noplan");
    std::fs::write(dir.join("shop.har"), shop_har()).unwrap();
    let mut app = start(cx, None, None);
    open_har(&mut app, &dir.join("shop.har"));
    assert!(app.state()["dialog"].is_null());
    assert!(app.state()["notice"].as_str().unwrap().contains("Open a plan first"));
}

#[gpui::test]
fn a_large_recording_is_read_and_added_without_freezing_the_window(cx: &mut TestAppContext) {
    let s = setup("har-large");
    let entries: Vec<String> = (0..1500).map(|i| entry(&format!("p{}", i / 10), "GET", &format!("https://shop.example.com/api/{i}?q=1"), "application/json", "")).collect();
    let pages: Vec<String> = (0..150).map(|i| format!(r#"{{"id":"p{i}","title":"Page {i}"}}"#)).collect();
    std::fs::write(s.dir.join("big.har"), format!(r#"{{"log":{{"pages":[{}],"entries":[{}]}}}}"#, pages.join(","), entries.join(","))).unwrap();
    let mut app = start(cx, Some(s.plan.clone()), None);
    let t = Instant::now();
    open_har(&mut app, &s.dir.join("big.har"));
    assert_eq!(dialog(&mut app)["kept"], 1500);
    assert!(t.elapsed() < Duration::from_secs(10), "reading took {:?}", t.elapsed());
    let t = Instant::now();
    app.click("dialog-har-add");
    eprintln!("1500 requests added in {:?}", t.elapsed());
    assert!(app.state()["dialog"].is_null(), "{}", app.state()["dialog"]);
    assert!(t.elapsed() < Duration::from_secs(60), "adding took {:?}", t.elapsed());
    assert!(app.state()["notice"].as_str().unwrap().contains("Added 1500 requests"));
}

#[gpui::test]
fn hostile_text_in_a_recording_ends_up_as_text_in_a_plan_that_saves_and_reads_back(cx: &mut TestAppContext) {
    let s = setup("har-hostile");
    let evil = "</testname><evil/>&\"'<![CDATA[x]]>";
    let url = serde_json::to_string("https://shop.example.com/a?x=<y>&z=\"q\"").unwrap();
    let url = &url[1..url.len() - 1];
    let e = entry("p", "POST", url, "text/plain", &format!(r#","postData":{{"mimeType":"text/plain","text":{}}}"#, serde_json::to_string(evil).unwrap()));
    let text = format!(r#"{{"log":{{"pages":[{{"id":"p","title":{}}}],"entries":[{e}]}}}}"#, serde_json::to_string(evil).unwrap());
    std::fs::write(s.dir.join("evil.har"), text).unwrap();
    let mut app = start(cx, Some(s.plan.clone()), None);
    open_har(&mut app, &s.dir.join("evil.har"));
    app.click("dialog-har-add");
    assert!(app.state()["dialog"].is_null(), "{}", app.state()["dialog"]);
    let shell = app.shell.clone();
    app.cx.update(|window, cx| shell.update(cx, |sh, cx| sh.save_active(window, cx)));
    let saved = std::fs::read_to_string(&s.plan).unwrap();
    assert!(!saved.contains("<evil/>"), "the markup is text, not structure");
    assert!(plan_core::EditablePlan::open(&saved).is_ok(), "the saved plan reads back");
}
