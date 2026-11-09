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
    assert!(d["heads_up"].as_str().unwrap().contains("Cookie Manager"), "the recording had a cookie: {d}");
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
    assert_eq!(elements(&mut app), before + 7, "two pages, three requests, one Header Manager for the headers they share and a Cookie Manager");
    let text = app.plan_text();
    for want in ["testname=\"Home\"", "testname=\"Sign in\"", "testname=\"POST /api/login\"", "testname=\"GET /api/items\"", "shop.example.com"] {
        assert!(text.contains(want), "{want}");
    }
    assert!(text.contains("Headers for all requests") && text.contains("CookieManager"), "shared headers and the recording's cookies get one manager each");
    assert!(!text.contains("sid=secret") && !text.contains("logo.png") && !text.contains("app.js"), "cookies and static files are not brought in");
    // One undo takes it all away.
    let shell = app.shell.clone();
    app.cx.update(|window, cx| shell.update(cx, |s, cx| s.perform(shell_core::actions::Action::Undo, window, cx)));
    app.cx.run_until_parked();
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

/// An app with no plan open whose last plan was in `dir`, so a plan made for a recording goes there and nowhere else.
fn app_without_plan<'a>(cx: &'a mut TestAppContext, dir: &Path) -> App<'a> {
    let settings_path = dir.join("settings.json");
    let mut s = shell_core::settings::Settings::default();
    s.push_recent(&dir.join("earlier.jmx").display().to_string());
    shell_core::settings::save(&settings_path, &s).unwrap();
    start(cx, None, Some(settings_path))
}

#[gpui::test]
fn with_no_plan_open_the_import_makes_a_plan_named_after_the_file_and_opens_it(cx: &mut TestAppContext) {
    let dir = temp_dir("har-noplan");
    std::fs::write(dir.join("shop journey.har"), shop_har()).unwrap();
    let mut app = app_without_plan(cx, &dir);
    assert!(app.tabs().is_empty());
    open_har(&mut app, &dir.join("shop journey.har"));
    assert_eq!(dialog(&mut app)["kind"], "har", "the dialog opens with no plan open: {}", app.state()["notice"]);
    assert!(app.tabs().is_empty(), "nothing is made until Add to Plan");
    assert!(!dir.join("shop journey.jmx").exists());
    app.click("dialog-har-add");
    assert!(app.state()["dialog"].is_null(), "{}", app.state()["notice"]);
    assert_eq!(app.tabs(), vec!["shop journey.jmx".to_string()]);
    assert!(dir.join("shop journey.jmx").is_file(), "the plan is a file of its own");
    assert!(elements(&mut app) >= 7, "Test Plan, a Thread Group, two pages and three requests: {}", elements(&mut app));
    goto(&mut app, "Sign in");
    assert_eq!(app.selected().as_deref(), Some("Sign in"), "the page became a Transaction Controller");
    assert!(app.state()["notice"].as_str().unwrap().contains("Added 3 requests"));
}

#[gpui::test]
fn a_plan_with_no_thread_group_gets_one_for_the_recording(cx: &mut TestAppContext) {
    let dir = temp_dir("har-no-group");
    let plan = dir.join("bare.jmx");
    std::fs::write(&plan, plan_core::starters::new_plan_text(plan_core::starters::Starter::Empty, "bare").unwrap()).unwrap();
    std::fs::write(dir.join("shop.har"), shop_har()).unwrap();
    let mut app = start(cx, Some(plan.clone()), None);
    assert_eq!(elements(&mut app), 1, "only the Test Plan");
    open_har(&mut app, &dir.join("shop.har"));
    app.click("dialog-har-add");
    assert!(app.state()["dialog"].is_null(), "{}", app.state()["notice"]);
    assert_eq!(app.tabs(), vec!["bare.jmx".to_string()], "no second plan is made when one is open");
    goto(&mut app, "Thread Group");
    assert_eq!(app.selected().as_deref(), Some("Thread Group"));
    assert!(elements(&mut app) >= 7);
}

#[gpui::test]
fn a_recording_with_nothing_left_makes_no_plan_and_says_why(cx: &mut TestAppContext) {
    let dir = temp_dir("har-nothing-left");
    let only_images = format!(r#"{{"log":{{"entries":[{}]}}}}"#, entry("p", "GET", "https://shop.example.com/a.png", "image/png", ""));
    std::fs::write(dir.join("images.har"), only_images).unwrap();
    let mut app = app_without_plan(cx, &dir);
    open_har(&mut app, &dir.join("images.har"));
    assert_eq!(dialog(&mut app)["kind"], "har");
    app.click("dialog-har-add");
    assert_eq!(dialog(&mut app)["kind"], "har", "the dialog stays with its message");
    assert!(dialog(&mut app)["error"].as_str().is_some());
    assert!(app.tabs().is_empty(), "a file that gives nothing leaves no plan behind");
    assert!(!dir.join("images.jmx").exists());
}

#[gpui::test]
fn import_from_har_is_available_with_no_plan_open(cx: &mut TestAppContext) {
    let dir = temp_dir("har-available");
    let mut app = app_without_plan(cx, &dir);
    app.keys("ctrl-shift-p");
    app.input(">import from har");
    assert!(app.palette_titles().iter().any(|t| t.starts_with("Import from HAR")), "{:?}", app.palette_titles());
    assert!(!app.palette_unavailable().iter().any(|t| t.starts_with("Import from HAR")), "it can be chosen with no plan open");
    app.keys("escape");
}

#[test]
fn plan_file_names_made_from_a_recording_are_safe() {
    use jmeter_ui::new_plan::unique_plan_path;
    let d = temp_dir("har-names");
    let name = |stem: &str| unique_plan_path(&d, stem).file_name().unwrap().to_string_lossy().to_string();
    assert_eq!(name("shop journey"), "shop journey.jmx");
    assert_eq!(name("a:b*c?d"), "a_b_c_d.jmx");
    assert_eq!(name("x/y\\z"), "x_y_z.jmx");
    assert_eq!(name(""), "New plan.jmx");
    assert_eq!(name("   "), "New plan.jmx");
    assert_eq!(name(".."), "New plan.jmx");
    assert_eq!(name("con"), "con plan.jmx", "device names are not used");
    assert_eq!(name("COM1"), "COM1 plan.jmx");
    assert_eq!(name("lpt9"), "lpt9 plan.jmx");
    assert_eq!(name("com10"), "com10.jmx", "only COM1 to COM9 are devices");
    assert_eq!(name("tab\there"), "tab_here.jmx");
    assert_eq!(name(&"long".repeat(100)).chars().count(), 80 + ".jmx".len());
    std::fs::write(d.join("taken.jmx"), "x").unwrap();
    assert_eq!(name("taken"), "taken 2.jmx", "an existing file is never replaced");
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
