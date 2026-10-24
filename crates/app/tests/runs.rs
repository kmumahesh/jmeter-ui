//! Running plans from the window: the questions before a run, live numbers, stopping, run history, the folder
//! for run files, and what must not happen. Tests that run JMeter skip themselves without Java and JMeter.

mod common;
#[path = "../../jvm-runtime/tests/common/mod.rs"]
mod jm;

use common::*;
use gpui::TestAppContext;
use jvm_runtime::runs::{self, Outcome, RunKind};
use serde_json::Value;
use shell_core::settings::{self, Settings};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// A folder with a plan in it and settings that point at the JMeter install.
struct Setup {
    dir: PathBuf,
    plan: PathBuf,
    settings: PathBuf,
}

fn setup(name: &str, plan_text: &str, home: Option<&Path>, tweak: impl FnOnce(&mut Settings)) -> Setup {
    let dir = temp_dir(name);
    let plan = dir.join("shop.jmx");
    std::fs::write(&plan, plan_text).unwrap();
    let settings_path = dir.join("settings.json");
    let mut s = Settings { jmeter_home: home.map(|h| h.to_string_lossy().into_owned()), ..Settings::default() };
    tweak(&mut s);
    settings::save(&settings_path, &s).unwrap();
    Setup { dir, plan, settings: settings_path }
}

fn open<'a>(cx: &'a mut TestAppContext, s: &Setup, runtime: bool) -> App<'a> {
    let app = start(cx, Some(s.plan.clone()), Some(s.settings.clone()));
    let shell = app.shell.clone();
    let home = s.settings.clone();
    app.cx.update(|_, cx| {
        shell.update(cx, |sh, _| {
            sh.runtime = if runtime {
                let h = settings::load(&home).settings.jmeter_home.map(PathBuf::from);
                jmeter_ui::shell::check_runtime(None, h.as_deref())
            } else {
                // Enough for the questions before a run, which come before anything is started.
                jmeter_ui::shell::RuntimeStatus { java: Some("java".into()), jmeter: Some("5.6.3".into()), problem: None }
            };
        })
    });
    app.cx.run_until_parked();
    app
}

fn dialog_kind(app: &mut App) -> Option<String> {
    app.state()["dialog"]["kind"].as_str().map(str::to_string)
}

fn run_state(app: &mut App) -> Value {
    app.state()["run"].clone()
}

fn phase(app: &mut App) -> Option<String> {
    run_state(app)["phase"].as_str().map(str::to_string)
}

/// Let time pass until the run has ended, as it does on screen.
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

/// Let time pass until `cond` holds on the state.
fn wait_until(app: &mut App, max: Duration, what: &str, mut cond: impl FnMut(&Value) -> bool) -> Value {
    let start = Instant::now();
    loop {
        app.cx.executor().advance_clock(Duration::from_millis(250));
        app.cx.run_until_parked();
        let st = app.state();
        if cond(&st) {
            return st;
        }
        assert!(start.elapsed() < max, "gave up waiting for {what}: {}", st["run"]);
        std::thread::sleep(Duration::from_millis(80));
    }
}

fn shop_plan(port: u16, users: u32, loops: i32) -> String {
    jm::plan(port, users, loops, 0, &[jm::request("Home", "/"), jm::request("Search", "/search"), jm::request("Broken", "/fail")], "")
}

fn run_dirs(root: &Path) -> Vec<PathBuf> {
    runs::list_runs(root, None).into_iter().map(|r| r.dir).collect()
}

fn perform(app: &mut App, action: shell_core::actions::Action) {
    let shell = app.shell.clone();
    app.cx.update(|window, cx| shell.update(cx, |s, cx| s.perform(action, window, cx)));
    app.cx.run_until_parked();
}

fn goto(app: &mut App, name: &str) {
    app.keys("ctrl-p");
    app.input(name);
    app.keys("enter");
}

/// A finished run folder made by hand, with a results file of `rows` rows.
fn stored_run(root: &Path, plan: &Path, second: u32, rows: u32, outcome: Outcome) -> PathBuf {
    use chrono::TimeZone;
    let when = chrono::Local.with_ymd_and_hms(2026, 1, 2, 3, 4, second).unwrap();
    let (dir, mut meta) = runs::new_run(root, plan, RunKind::Run, when).unwrap();
    let mut jtl = String::from("timeStamp,elapsed,label,responseCode,responseMessage,threadName,success,bytes,allThreads\n");
    for i in 0..rows {
        jtl.push_str(&format!("{},{},{},200,OK,t,{},10,3\n", 1_000_000 + i as u64 * 100, 20 + i % 7, if i % 2 == 0 { "Home" } else { "Search" }, i % 10 != 0));
    }
    std::fs::write(dir.join("results.jtl"), jtl).unwrap();
    std::fs::write(dir.join("jmeter.log"), "log").unwrap();
    meta.outcome = outcome;
    meta.ended_ms = Some(meta.started_ms + 5000);
    meta.samples = rows as u64;
    write(&dir, &meta);
    dir
}

/// The real path, so that a short Windows name (`RUNNER~1`) and the long name compare equal.
fn canon(p: &Path) -> PathBuf {
    std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf())
}

fn write(dir: &Path, meta: &runs::RunMeta) {
    runs::write_meta(dir, meta).unwrap();
}

// ---- the questions before a run (no JMeter needed) ----

#[gpui::test]
fn without_java_or_jmeter_the_run_buttons_say_why(cx: &mut TestAppContext) {
    let s = setup("noruntime", &shop_plan(1, 1, 1), None, |_| {});
    let mut app = open(cx, &s, false);
    let shell = app.shell.clone();
    app.cx.update(|_, cx| shell.update(cx, |sh, _| sh.runtime = Default::default()));
    app.keys("f5");
    assert!(app.state()["run"].is_null() && dialog_kind(&mut app).is_none(), "nothing started and nothing asked");
    assert_eq!(app.state()["runtime_ready"], false);
}

#[gpui::test]
fn the_run_menu_and_palette_list_every_run_command(cx: &mut TestAppContext) {
    let s = setup("palette", &shop_plan(1, 1, 1), None, |_| {});
    let mut app = open(cx, &s, false);
    app.keys("ctrl-k");
    app.input("run");
    let titles = app.palette_titles();
    for want in ["Run Plan", "Stop Run", "Stop Run Now"] {
        assert!(titles.iter().any(|t| t == want), "{want} not in {titles:?}");
    }
    app.keys("escape");
    app.keys("ctrl-k");
    app.input("send");
    assert!(app.palette_titles().iter().any(|t| t.contains("Send Selected Request Once")));
    assert!(app.palette_unavailable().iter().any(|t| t.contains("Send Selected Request Once")), "nothing selected, so it cannot be chosen yet");
}

#[gpui::test]
fn a_plan_with_unsaved_changes_asks_to_save_before_running(cx: &mut TestAppContext) {
    let s = setup("savefirst", &shop_plan(1, 1, 1), None, |_| {});
    let before = std::fs::read_to_string(&s.plan).unwrap();
    let mut app = open(cx, &s, false);
    goto(&mut app, "Home");
    app.keys("f2");
    app.keys("ctrl-a");
    app.input("Front page");
    app.keys("enter");
    assert!(app.state()["dirty"].as_bool().unwrap());
    app.keys("f5");
    assert_eq!(dialog_kind(&mut app).as_deref(), Some("save_to_run"));
    assert!(app.has("dialog-save-run"));
    // Cancel: nothing runs and the file on disk is as it was.
    app.click("dialog-cancel");
    assert!(dialog_kind(&mut app).is_none() && app.state()["run"].is_null());
    assert_eq!(std::fs::read_to_string(&s.plan).unwrap(), before);
}

#[gpui::test]
fn saving_to_run_writes_the_file_then_goes_on_to_the_next_question(cx: &mut TestAppContext) {
    // A custom folder that is gone is the next question; it shows that saving happened and the run went on.
    let missing = std::env::temp_dir().join("jmeter-ui-no-such-folder-xyz");
    let s = setup("saveandrun", &shop_plan(1, 1, 1), None, |s| {
        s.runs_custom = true;
        s.runs_folder = Some(missing.to_string_lossy().into_owned());
    });
    let mut app = open(cx, &s, false);
    goto(&mut app, "Home");
    app.keys("f2");
    app.keys("ctrl-a");
    app.input("Front page");
    app.keys("enter");
    app.keys("f5");
    app.click("dialog-save-run");
    assert!(std::fs::read_to_string(&s.plan).unwrap().contains("Front page"), "the file was saved");
    assert!(!app.state()["dirty"].as_bool().unwrap());
    assert_eq!(dialog_kind(&mut app).as_deref(), Some("runs_place"), "and the run moved on to the next check");
}

#[gpui::test]
fn a_runs_folder_that_cannot_be_used_stops_the_run_and_offers_a_way_out(cx: &mut TestAppContext) {
    let gone = std::env::temp_dir().join("jmeter-ui-gone-folder-xyz");
    let s = setup("badplace", &shop_plan(1, 1, 1), None, |s| {
        s.runs_custom = true;
        s.runs_folder = Some(gone.to_string_lossy().into_owned());
    });
    let mut app = open(cx, &s, false);
    app.keys("f5");
    let st = app.state();
    assert_eq!(st["dialog"]["kind"], "runs_place");
    assert!(st["dialog"]["message"].as_str().unwrap().contains("does not exist"), "{}", st["dialog"]);
    assert!(st["run"].is_null());
    assert!(!gone.exists(), "the missing folder is not made up");
    for b in ["dialog-use-plan-folder", "dialog-choose-folder", "dialog-cancel"] {
        assert!(app.has(b), "{b}");
    }
    app.click("dialog-cancel");
    assert!(dialog_kind(&mut app).is_none());
    assert!(!s.dir.join("runs").exists(), "nothing was made next to the plan either");
}

#[gpui::test]
fn using_the_jmx_folder_from_the_question_changes_the_setting_and_carries_on(cx: &mut TestAppContext) {
    let gone = std::env::temp_dir().join("jmeter-ui-gone-folder-abc");
    let s = setup("useplan", &shop_plan(1, 1, 1), None, |s| {
        s.runs_custom = true;
        s.runs_folder = Some(gone.to_string_lossy().into_owned());
    });
    let mut app = open(cx, &s, false);
    app.keys("f5");
    app.click("dialog-use-plan-folder");
    assert!(!settings::load(&s.settings).settings.runs_custom, "the choice is saved");
    assert!(s.dir.join("runs").is_dir(), "the runs folder was made next to the plan");
    assert!(dialog_kind(&mut app).is_none() || dialog_kind(&mut app).as_deref() != Some("runs_place"));
}

#[gpui::test]
fn a_custom_folder_that_is_a_file_is_refused(cx: &mut TestAppContext) {
    let s = setup("filefolder", &shop_plan(1, 1, 1), None, |_| {});
    let file = s.dir.join("not-a-folder");
    std::fs::write(&file, "x").unwrap();
    let mut s2 = settings::load(&s.settings).settings;
    s2.runs_custom = true;
    s2.runs_folder = Some(file.to_string_lossy().into_owned());
    settings::save(&s.settings, &s2).unwrap();
    let mut app = open(cx, &s, false);
    app.keys("f5");
    assert!(app.state()["dialog"]["message"].as_str().unwrap().contains("not a folder"));
}

#[gpui::test]
fn a_plan_with_scripts_asks_once_for_that_plan_file(cx: &mut TestAppContext) {
    let script = r#"        <JSR223Sampler guiclass="TestBeanGUI" testclass="JSR223Sampler" testname="Script" enabled="true">
          <stringProp name="scriptLanguage">groovy</stringProp>
          <stringProp name="script">log.info("hi")</stringProp>
        </JSR223Sampler>
        <hashTree/>
"#;
    let text = jm::plan(1, 1, 1, 0, &[jm::request("Home", "/")], script);
    // A usable runs folder is needed to get as far as the question about scripts.
    let s = setup("scripts", &text, None, |_| {});
    let mut app = open(cx, &s, false);
    app.keys("f5");
    let st = app.state();
    assert_eq!(st["dialog"]["kind"], "scripts");
    assert!(st["dialog"]["text"].as_str().unwrap().contains("JSR223 script"), "{}", st["dialog"]);
    assert!(app.has("dialog-run-scripts"));
    app.click("dialog-cancel");
    assert!(settings::load(&s.settings).settings.script_approved.is_empty(), "cancel remembers nothing");
    // Agreeing is remembered for this plan file (the run itself then fails to start here: no real JMeter).
    app.keys("f5");
    app.click("dialog-run-scripts");
    let saved = settings::load(&s.settings).settings;
    assert_eq!(saved.script_approved.len(), 1);
    assert!(saved.script_approved[0].ends_with("shop.jmx"));
    app.keys("f5");
    assert_ne!(dialog_kind(&mut app).as_deref(), Some("scripts"), "not asked again for the same plan file");
}

#[gpui::test]
fn a_plan_without_scripts_is_never_asked_about_them(cx: &mut TestAppContext) {
    let s = setup("noscripts", &shop_plan(1, 1, 1), None, |_| {});
    let mut app = open(cx, &s, false);
    app.keys("f5");
    assert_ne!(dialog_kind(&mut app).as_deref(), Some("scripts"));
}

// ---- settings ----

#[gpui::test]
fn the_settings_tab_chooses_where_run_files_go(cx: &mut TestAppContext) {
    let s = setup("settings-runs", &shop_plan(1, 1, 1), None, |_| {});
    let mut app = open(cx, &s, false);
    app.keys("ctrl-,");
    app.scroll_down(900.0);
    assert!(app.has("settings-runs") && app.has("runs-plan-folder") && app.has("runs-custom-folder"));
    assert!(!app.has("runs-choose-folder") && !app.has("runs-folder-path"), "no chooser while the plan folder is used");
    app.click("runs-custom-folder");
    assert!(app.has("runs-choose-folder") && app.has("runs-folder-path"));
    assert!(settings::load(&s.settings).settings.runs_custom);
    assert_eq!(app.state()["runs_place"], "PlanFolder", "custom without a folder still keeps runs next to the plan");
    // The folder comes from the chooser, never from typing: set as the chooser would.
    let chosen = s.dir.join("elsewhere");
    std::fs::create_dir_all(&chosen).unwrap();
    let shell = app.shell.clone();
    app.cx.update(|_, cx| shell.update(cx, |sh, cx| sh.set_runs_folder(&chosen, cx)));
    app.settle();
    assert!(app.state()["runs_place"].as_str().unwrap().contains("elsewhere"));
    assert_eq!(settings::load(&s.settings).settings.runs_folder.as_deref().map(|f| f.ends_with("elsewhere")), Some(true));
    app.click("runs-plan-folder");
    assert_eq!(app.state()["runs_place"], "PlanFolder");
    assert_eq!(settings::load(&s.settings).settings.runs_folder.as_deref().map(|f| f.ends_with("elsewhere")), Some(true), "the chosen folder is remembered for later");
}

#[gpui::test]
fn the_number_of_runs_kept_is_a_checked_number(cx: &mut TestAppContext) {
    let s = setup("settings-keep", &shop_plan(1, 1, 1), None, |_| {});
    let mut app = open(cx, &s, false);
    app.keys("ctrl-,");
    app.scroll_down(900.0);
    assert_eq!(app.state()["runs_keep"], 10);
    app.type_into("settings-runs-keep", "4");
    assert_eq!(app.state()["runs_keep"], 4);
    assert_eq!(settings::load(&s.settings).settings.runs_keep, 4);
    app.type_into("settings-runs-keep", "0");
    assert_eq!(app.state()["runs_keep"], 4, "0 is refused");
    app.type_into("settings-runs-keep", "5000");
    assert_eq!(app.state()["runs_keep"], 4, "above 1000 is refused");
    app.type_into("settings-runs-keep", "abc");
    assert_eq!(app.state()["runs_keep"], 4);
}

#[gpui::test]
fn lowering_the_number_kept_removes_the_older_runs_of_the_open_plan_at_once(cx: &mut TestAppContext) {
    let s = setup("keep-lower", &shop_plan(1, 1, 1), None, |_| {});
    let root = s.dir.join("runs");
    for sec in 1..=5 {
        stored_run(&root, &s.plan, sec, 3, Outcome::Finished);
    }
    let other = s.dir.join("other.jmx");
    stored_run(&root, &other, 9, 3, Outcome::Finished);
    let mut app = open(cx, &s, false);
    app.keys("ctrl-,");
    app.scroll_down(900.0);
    app.type_into("settings-runs-keep", "2");
    assert_eq!(runs::list_runs(&root, Some(&s.plan)).len(), 2);
    assert_eq!(runs::list_runs(&root, Some(&other)).len(), 1, "another plan's runs are not touched");
}

// ---- history (no JMeter needed) ----

#[gpui::test]
fn the_history_tab_lists_the_runs_of_the_open_plan_newest_first(cx: &mut TestAppContext) {
    let s = setup("history", &shop_plan(1, 1, 1), None, |_| {});
    let root = s.dir.join("runs");
    stored_run(&root, &s.plan, 1, 10, Outcome::Finished);
    stored_run(&root, &s.plan, 2, 20, Outcome::Stopped);
    stored_run(&root, &s.plan, 3, 30, Outcome::Failed);
    stored_run(&root, &s.dir.join("other.jmx"), 4, 99, Outcome::Finished);
    let mut app = open(cx, &s, false);
    app.keys("ctrl-j");
    app.click("bottom-history");
    let h = app.state()["history"].clone();
    let rows: Vec<_> = h.as_array().unwrap().iter().map(|r| (r["outcome"].as_str().unwrap().to_string(), r["samples"].as_u64().unwrap())).collect();
    assert_eq!(rows, [("Failed".into(), 30), ("Stopped".into(), 20), ("Finished".into(), 10)]);
    for i in 0..3 {
        let sel: &'static str = ["history-row-0", "history-row-1", "history-row-2"][i];
        assert!(app.has(sel), "{sel}");
    }
    assert!(app.has("history-root") && app.has("history-open-folder"));
}

#[gpui::test]
fn history_shows_runs_of_the_plan_in_the_tab_not_every_plan(cx: &mut TestAppContext) {
    let s = setup("history-per-plan", &shop_plan(1, 1, 1), None, |_| {});
    let root = s.dir.join("runs");
    stored_run(&root, &s.plan, 1, 5, Outcome::Finished);
    let other = s.dir.join("cart.jmx");
    std::fs::write(&other, shop_plan(1, 1, 1)).unwrap();
    stored_run(&root, &other, 2, 7, Outcome::Finished);
    stored_run(&root, &other, 3, 8, Outcome::Finished);
    let mut app = open(cx, &s, false);
    assert_eq!(app.state()["history"].as_array().unwrap().len(), 1);
    app.open_path(&other);
    assert_eq!(app.state()["history"].as_array().unwrap().len(), 2, "switching plan switches the list");
    app.keys("ctrl-1");
    let _ = app.state();
}

#[gpui::test]
fn clicking_a_stored_run_shows_its_numbers_and_charts(cx: &mut TestAppContext) {
    let s = setup("view", &shop_plan(1, 1, 1), None, |_| {});
    let root = s.dir.join("runs");
    let dir = stored_run(&root, &s.plan, 1, 1000, Outcome::Finished);
    let mut app = open(cx, &s, false);
    app.keys("ctrl-j");
    app.click("bottom-history");
    app.click("history-row-0");
    let st = wait_until(&mut app, Duration::from_secs(30), "the stored run to load", |st| st["run"]["phase"] == "Finished");
    assert_eq!(st["run"]["samples"], 1000);
    assert_eq!(st["run"]["errors"], 100);
    assert_eq!(st["run"]["labels"], 2);
    assert_eq!(canon(Path::new(st["run"]["dir"].as_str().unwrap())), canon(&dir));
    assert_eq!(st["bottom_tab"], "Run");
    for sel in ["run-kpi-samples", "run-kpi-rate", "run-kpi-avg", "run-kpi-p95", "run-kpi-errors", "run-kpi-users", "run-chart-response", "run-chart-rate", "run-chart-errors", "run-chart-users", "run-files", "run-phase"] {
        assert!(app.has(sel), "{sel}");
    }
    app.click("run-tab-labels");
    app.click("run-tab-log");
    app.click("run-tab-errors");
}

#[gpui::test]
fn a_damaged_results_file_still_opens_as_a_stored_run(cx: &mut TestAppContext) {
    let s = setup("view-damaged", &shop_plan(1, 1, 1), None, |_| {});
    let root = s.dir.join("runs");
    let dir = stored_run(&root, &s.plan, 1, 10, Outcome::Finished);
    let mut junk = vec![0u8; 3_000_000];
    for (i, b) in junk.iter_mut().enumerate() {
        *b = (i * 31 % 251) as u8;
    }
    std::fs::write(dir.join("results.jtl"), junk).unwrap();
    let mut app = open(cx, &s, false);
    app.keys("ctrl-j");
    app.click("bottom-history");
    app.click("history-row-0");
    let st = wait_until(&mut app, Duration::from_secs(60), "the damaged run to load", |st| st["run"]["phase"] != "Loading");
    assert_eq!(st["run"]["samples"], 0, "junk is not a set of samples: {}", st["run"]);
    assert!(st["run"]["skipped"].as_u64().unwrap() > 0, "and the skipped lines are counted");
}

#[gpui::test]
fn a_run_that_was_cut_short_is_shown_as_interrupted(cx: &mut TestAppContext) {
    let s = setup("interrupted", &shop_plan(1, 1, 1), None, |_| {});
    let root = s.dir.join("runs");
    stored_run(&root, &s.plan, 1, 5, Outcome::Running);
    let mut app = open(cx, &s, false);
    assert_eq!(app.state()["history"][0]["outcome"], "Running");
    app.keys("ctrl-j");
    app.click("bottom-history");
    assert!(app.has("history-row-0"));
}

#[gpui::test]
fn removing_one_run_asks_first_and_removes_only_that_run(cx: &mut TestAppContext) {
    let s = setup("delete-run", &shop_plan(1, 1, 1), None, |_| {});
    let root = s.dir.join("runs");
    let old = stored_run(&root, &s.plan, 1, 5, Outcome::Finished);
    let new = stored_run(&root, &s.plan, 2, 5, Outcome::Finished);
    let mut app = open(cx, &s, false);
    app.keys("ctrl-j");
    app.click("bottom-history");
    app.click("history-delete-1"); // the older one is second
    assert_eq!(dialog_kind(&mut app).as_deref(), Some("delete_run"));
    app.click("dialog-cancel");
    assert!(old.exists() && new.exists(), "cancel removes nothing");
    app.click("history-delete-1");
    app.click("dialog-delete-run");
    assert!(!old.exists() && new.exists());
    assert_eq!(app.state()["history"].as_array().unwrap().len(), 1);
}

#[gpui::test]
fn removing_never_touches_a_folder_that_is_not_a_run(cx: &mut TestAppContext) {
    let s = setup("delete-foreign", &shop_plan(1, 1, 1), None, |_| {});
    let foreign = s.dir.join("runs").join("my-own-stuff");
    std::fs::create_dir_all(&foreign).unwrap();
    std::fs::write(foreign.join("precious.txt"), "x").unwrap();
    let mut app = open(cx, &s, false);
    let shell = app.shell.clone();
    let f = foreign.clone();
    app.cx.update(|_, cx| shell.update(cx, |sh, _| sh.dialog = Some(jmeter_ui::shell::Dialog::DeleteRun { dir: f, name: "x".into() })));
    app.settle();
    app.click("dialog-delete-run");
    assert!(foreign.join("precious.txt").exists(), "a folder without this app's run.json is not removed");
    // Nor one that only looks like a run but sits outside a runs folder.
    let elsewhere = s.dir.join("somewhere");
    std::fs::create_dir_all(&elsewhere).unwrap();
    runs::write_meta(&elsewhere, &runs::RunMeta::new(&s.plan, RunKind::Run, chrono::Local::now())).unwrap();
    let e = elsewhere.clone();
    app.cx.update(|_, cx| shell.update(cx, |sh, _| sh.dialog = Some(jmeter_ui::shell::Dialog::DeleteRun { dir: e, name: "x".into() })));
    app.settle();
    app.click("dialog-delete-run");
    assert!(elsewhere.exists(), "only a folder directly inside a runs folder can be removed");
}

// ---- the shared runs folder ----

#[gpui::test]
fn clearing_the_shared_runs_folder_asks_and_removes_only_runs(cx: &mut TestAppContext) {
    let shared = temp_dir("shared");
    let s = setup("clear", &shop_plan(1, 1, 1), None, |s| {
        s.runs_custom = true;
        s.runs_folder = Some(shared.to_string_lossy().into_owned());
    });
    let root = shared.join("runs");
    stored_run(&root, &s.plan, 1, 3, Outcome::Finished);
    stored_run(&root, &s.dir.join("other.jmx"), 2, 3, Outcome::Finished);
    std::fs::create_dir_all(root.join("keep me")).unwrap();
    std::fs::write(root.join("keep me").join("a.txt"), "x").unwrap();
    // Runs next to the plan, which Clear never touches.
    let beside = stored_run(&s.dir.join("runs"), &s.plan, 3, 3, Outcome::Finished);
    let mut app = open(cx, &s, false);
    perform(&mut app, shell_core::actions::Action::ClearRunsFolder);
    let st = app.state();
    assert_eq!(st["dialog"]["kind"], "clear_runs");
    assert_eq!(st["dialog"]["count"], 2);
    assert!(st["dialog"]["root"].as_str().unwrap().ends_with("runs"));
    app.click("dialog-cancel");
    assert_eq!(run_dirs(&root).len(), 2, "cancel removes nothing");
    perform(&mut app, shell_core::actions::Action::ClearRunsFolder);
    app.click("dialog-clear-runs");
    assert!(run_dirs(&root).is_empty());
    assert!(root.join("keep me").join("a.txt").exists(), "other files are left alone");
    assert!(beside.exists(), "the runs next to the .jmx are never cleared");
}

#[gpui::test]
fn clear_is_not_available_for_the_runs_next_to_the_plan(cx: &mut TestAppContext) {
    let s = setup("clear-plan", &shop_plan(1, 1, 1), None, |_| {});
    let beside = stored_run(&s.dir.join("runs"), &s.plan, 1, 3, Outcome::Finished);
    let mut app = open(cx, &s, false);
    perform(&mut app, shell_core::actions::Action::ClearRunsFolder);
    assert!(dialog_kind(&mut app).is_none());
    assert!(beside.exists());
    app.keys("ctrl-,");
    app.scroll_down(900.0);
    assert!(app.has("runs-clear"));
    app.keys("ctrl-k");
    app.input("clear runs");
    assert!(app.palette_unavailable().iter().any(|t| t.contains("Clear Runs Folder")), "listed, but greyed out");
}

#[gpui::test]
fn clearing_an_empty_shared_folder_says_so_without_asking(cx: &mut TestAppContext) {
    let shared = temp_dir("shared-empty");
    let s = setup("clear-empty", &shop_plan(1, 1, 1), None, |s| {
        s.runs_custom = true;
        s.runs_folder = Some(shared.to_string_lossy().into_owned());
    });
    let mut app = open(cx, &s, false);
    perform(&mut app, shell_core::actions::Action::ClearRunsFolder);
    assert!(dialog_kind(&mut app).is_none());
    assert!(app.state()["notice"].as_str().unwrap().contains("no runs to clear"));
}

// ---- real runs ----

fn real(name: &str, plan: impl FnOnce(u16) -> String, tweak: impl FnOnce(&mut Settings, &Path)) -> Option<(Setup, jm::Server)> {
    let env = jm::env()?;
    let server = jm::Server::start();
    let text = plan(server.port);
    let s = setup(name, &text, Some(&env.home), |_| {});
    let mut st = settings::load(&s.settings).settings;
    tweak(&mut st, &s.dir);
    settings::save(&s.settings, &st).unwrap();
    Some((s, server))
}

#[gpui::test]
fn f5_runs_the_saved_plan_and_shows_the_live_numbers(cx: &mut TestAppContext) {
    let Some((s, server)) = real("run-f5", |p| shop_plan(p, 2, 5), |_, _| {}) else { return };
    let before = std::fs::read_to_string(&s.plan).unwrap();
    let mut app = open(cx, &s, true);
    assert_eq!(app.state()["runtime_ready"], true);
    let t = Instant::now();
    app.keys("f5");
    assert_eq!(phase(&mut app).as_deref(), Some("Starting"));
    assert_eq!(app.state()["bottom"], true, "the lower panel opens");
    assert_eq!(app.state()["bottom_tab"], "Run");
    // While it goes the buttons say what is possible.
    assert!(app.has("run-stop") && app.has("run-stop-now"));
    let st = wait_run(&mut app, Duration::from_secs(120));
    eprintln!("a 30-request run from F5 to the end: {:?}", t.elapsed());
    assert_eq!(st["run"]["phase"], "Finished", "{}", st["run"]);
    assert_eq!(st["run"]["samples"], 30);
    assert_eq!(st["run"]["errors"], 10);
    assert_eq!(st["run"]["labels"], 3);
    assert_eq!(st["run"]["error_rows"], 10);
    assert!(st["run"]["points"].as_u64().unwrap() >= 1);
    assert_eq!(server.requests.load(std::sync::atomic::Ordering::Relaxed), 30);
    // The plan file is untouched, and the run is in its own folder under runs next to it.
    assert_eq!(std::fs::read_to_string(&s.plan).unwrap(), before);
    let dir = PathBuf::from(st["run"]["dir"].as_str().unwrap());
    assert_eq!(canon(dir.parent().unwrap()), canon(&s.dir.join("runs")));
    assert!(dir.join("results.jtl").is_file() && dir.join("jmeter.log").is_file() && dir.join("plan-as-run.jmx").is_file() && dir.join("run.json").is_file());
    let meta = runs::read_meta(&dir).unwrap();
    assert_eq!((meta.outcome, meta.samples, meta.errors), (Outcome::Finished, 30, 10));
    assert_eq!(meta.exit_code, Some(0));
    assert_eq!(st["history"].as_array().unwrap().len(), 1);
    // What is on screen.
    for sel in ["run-kpi-samples", "run-chart-response", "run-tab-errors", "run-files"] {
        assert!(app.has(sel), "{sel}");
    }
    app.click("run-tab-labels");
    app.click("run-tab-log");
    assert!(!app.has("run-stop"), "the stop buttons go when the run ends");
    // The run can be run again at once.
    assert!(app.state()["run"]["phase"] == "Finished");
}

#[gpui::test]
fn the_run_header_says_where_the_files_are_and_the_status_bar_follows_the_run(cx: &mut TestAppContext) {
    let Some((s, _server)) = real("run-header", |p| jm::plan(p, 1, -1, 100, &[jm::request("Home", "/")], ""), |_, _| {}) else { return };
    let mut app = open(cx, &s, true);
    app.keys("f5");
    wait_until(&mut app, Duration::from_secs(90), "samples", |st| st["run"]["samples"].as_u64().unwrap_or(0) >= 3);
    assert!(app.has("run-files"));
    let st = app.state();
    assert!(st["run"]["dir"].as_str().unwrap().contains("runs"), "{}", st["run"]["dir"]);
    perform(&mut app, shell_core::actions::Action::StopRunNow);
    wait_run(&mut app, Duration::from_secs(60));
}

#[gpui::test]
fn stop_ends_a_run_that_would_never_end_and_it_is_kept_as_stopped(cx: &mut TestAppContext) {
    let Some((s, _server)) = real("run-stop", |p| jm::plan(p, 1, -1, 100, &[jm::request("Home", "/")], ""), |_, _| {}) else { return };
    let mut app = open(cx, &s, true);
    app.keys("f5");
    wait_until(&mut app, Duration::from_secs(90), "samples", |st| st["run"]["samples"].as_u64().unwrap_or(0) >= 3);
    assert_eq!(app.state()["run"]["phase"], "Running");
    // While it goes: Run is not available again, Stop is.
    app.keys("f5");
    assert_eq!(app.state()["run"]["kind"], "Run");
    let t = Instant::now();
    app.keys("shift-f5");
    assert_eq!(app.state()["run"]["phase"], "Stopping");
    let st = wait_run(&mut app, Duration::from_secs(60));
    eprintln!("stop took {:?}", t.elapsed());
    assert_eq!(st["run"]["phase"], "Stopped");
    let dir = PathBuf::from(st["run"]["dir"].as_str().unwrap());
    assert_eq!(runs::read_meta(&dir).unwrap().outcome, Outcome::Stopped);
    assert!(runs::read_meta(&dir).unwrap().samples >= 3);
    assert_eq!(st["history"][0]["outcome"], "Stopped");
}

#[gpui::test]
fn stop_now_and_ending_jmeter_by_force_both_work(cx: &mut TestAppContext) {
    let Some((s, _server)) = real("run-kill", |p| jm::plan(p, 1, -1, 100, &[jm::request("Home", "/")], ""), |_, _| {}) else { return };
    let mut app = open(cx, &s, true);
    app.keys("f5");
    wait_until(&mut app, Duration::from_secs(90), "samples", |st| st["run"]["samples"].as_u64().unwrap_or(0) >= 2);
    assert_eq!(app.state()["run"]["can_kill"], false, "the force button only appears when a stop takes too long");
    // Force it straight away, as the button would once the stop took too long.
    let shell = app.shell.clone();
    app.cx.update(|_, cx| shell.update(cx, |sh, cx| sh.kill_run(cx)));
    let st = wait_run(&mut app, Duration::from_secs(60));
    assert_eq!(st["run"]["phase"], "Stopped");
}

#[gpui::test]
fn the_chosen_folder_gets_a_runs_folder_with_a_folder_for_each_run(cx: &mut TestAppContext) {
    let shared = temp_dir("run-shared");
    let sh2 = shared.clone();
    let Some((s, _server)) = real("run-custom", |p| shop_plan(p, 1, 1), move |st, _| {
        st.runs_custom = true;
        st.runs_folder = Some(sh2.to_string_lossy().into_owned());
    }) else {
        return;
    };
    let mut app = open(cx, &s, true);
    app.keys("f5");
    let st = wait_run(&mut app, Duration::from_secs(120));
    let dir = PathBuf::from(st["run"]["dir"].as_str().unwrap());
    assert_eq!(canon(dir.parent().unwrap()), canon(&shared.join("runs")), "inside the runs folder of the chosen folder");
    assert!(dir.join("results.jtl").is_file());
    assert!(!s.dir.join("runs").exists(), "nothing was written next to the plan");
    assert_eq!(st["run"]["samples"], 3);
}

#[gpui::test]
fn only_the_newest_runs_are_kept_after_a_run_and_their_files_stay(cx: &mut TestAppContext) {
    let Some((s, _server)) = real("run-keep", |p| shop_plan(p, 1, 1), |st, _| st.runs_keep = 3) else { return };
    let root = s.dir.join("runs");
    let olds: Vec<PathBuf> = (1..=4).map(|sec| stored_run(&root, &s.plan, sec, 3, Outcome::Finished)).collect();
    let mut app = open(cx, &s, true);
    app.keys("f5");
    let st = wait_run(&mut app, Duration::from_secs(120));
    let new = PathBuf::from(st["run"]["dir"].as_str().unwrap());
    let left = run_dirs(&root);
    assert_eq!(left.len(), 3, "{left:?}");
    assert!(left.iter().any(|l| canon(l) == canon(&new)), "the new run is kept");
    assert!(!olds[0].exists() && !olds[1].exists(), "the oldest two were removed");
    assert!(olds[2].join("results.jtl").exists() && olds[3].join("jmeter.log").exists(), "kept runs keep all their files");
    assert_eq!(st["history"].as_array().unwrap().len(), 3);
}

#[gpui::test]
fn validate_runs_the_whole_plan_once_with_one_user(cx: &mut TestAppContext) {
    let Some((s, server)) = real("run-validate", |p| jm::plan(p, 20, 50, 1000, &[jm::request("Home", "/"), jm::request("Search", "/search")], ""), |_, _| {}) else { return };
    let mut app = open(cx, &s, true);
    let t = Instant::now();
    app.keys("ctrl-f5");
    assert_eq!(app.state()["run"]["kind"], "Validate");
    let st = wait_run(&mut app, Duration::from_secs(120));
    eprintln!("validate of a plan that takes 50 minutes in full: {:?}", t.elapsed());
    assert_eq!(st["run"]["phase"], "Finished");
    assert_eq!(st["run"]["samples"], 2);
    assert_eq!(server.requests.load(std::sync::atomic::Ordering::Relaxed), 2);
    let notes = st["run"]["notes"].as_array().unwrap();
    assert!(notes.iter().any(|n| n.as_str().unwrap().contains("one user")), "{notes:?}");
    let dir = PathBuf::from(st["run"]["dir"].as_str().unwrap());
    assert!(dir.join("plan.jmx").is_file(), "the plan that was run is kept with the run");
    assert!(std::fs::read_to_string(&s.plan).unwrap().contains(">50<"), "the original plan keeps its 50 loops");
}

#[gpui::test]
fn send_once_sends_the_selected_request_and_only_that(cx: &mut TestAppContext) {
    let Some((s, server)) = real("run-sendonce", |p| shop_plan(p, 5, 5), |_, _| {}) else { return };
    let mut app = open(cx, &s, true);
    goto(&mut app, "Search");
    assert_eq!(app.selected().as_deref(), Some("Search"));
    app.keys("f6");
    assert_eq!(app.state()["run"]["kind"], "Send once");
    let st = wait_run(&mut app, Duration::from_secs(120));
    assert_eq!(st["run"]["phase"], "Finished");
    assert_eq!(st["run"]["samples"], 1);
    assert_eq!(server.paths.lock().unwrap().as_slice(), ["/search"]);
    assert_eq!(st["history"][0]["kind"], "Send once");
}

#[gpui::test]
fn send_once_is_not_available_on_something_that_is_not_a_request(cx: &mut TestAppContext) {
    let Some((s, server)) = real("run-sendonce-no", |p| shop_plan(p, 1, 1), |_, _| {}) else { return };
    let mut app = open(cx, &s, true);
    goto(&mut app, "Users");
    app.keys("f6");
    assert!(app.state()["run"].is_null(), "a thread group is not a request");
    assert_eq!(server.requests.load(std::sync::atomic::Ordering::Relaxed), 0);
}

#[gpui::test]
fn quitting_during_a_run_asks_and_cancelling_leaves_the_run_going(cx: &mut TestAppContext) {
    let Some((s, _server)) = real("run-quit", |p| jm::plan(p, 1, -1, 100, &[jm::request("Home", "/")], ""), |_, _| {}) else { return };
    let mut app = open(cx, &s, true);
    app.keys("f5");
    wait_until(&mut app, Duration::from_secs(90), "samples", |st| st["run"]["samples"].as_u64().unwrap_or(0) >= 2);
    perform(&mut app, shell_core::actions::Action::Quit);
    assert_eq!(dialog_kind(&mut app).as_deref(), Some("quit_running"));
    assert!(app.has("dialog-stop-quit"));
    app.click("dialog-cancel");
    assert_eq!(app.state()["run"]["phase"], "Running");
    perform(&mut app, shell_core::actions::Action::StopRunNow);
    wait_run(&mut app, Duration::from_secs(60));
}

#[gpui::test]
fn a_plan_jmeter_cannot_read_ends_as_failed_with_a_reason(cx: &mut TestAppContext) {
    let Some((s, _server)) = real("run-failed", |p| shop_plan(p, 1, 1), |_, _| {}) else { return };
    // The text is damaged on disk after the window opened it, so the run itself fails.
    let mut app = open(cx, &s, true);
    std::fs::write(&s.plan, "<html>this is not a plan</html>").unwrap();
    app.keys("f5");
    let st = wait_run(&mut app, Duration::from_secs(90));
    assert_eq!(st["run"]["phase"], "Failed");
    assert!(st["run"]["error"].as_str().unwrap().contains("JMeter ended"), "{}", st["run"]);
    assert_eq!(st["history"][0]["outcome"], "Failed");
}

#[gpui::test]
fn a_run_with_a_flood_of_samples_keeps_the_window_responsive(cx: &mut TestAppContext) {
    // 20 users in a tight loop against the local server for a while: many samples per second.
    let Some((s, server)) = real("run-flood", |p| jm::plan(p, 20, 400, 0, &[jm::request("Home", "/")], ""), |_, _| {}) else { return };
    let mut app = open(cx, &s, true);
    let t = Instant::now();
    app.keys("f5");
    let mut worst = Duration::ZERO;
    let st = loop {
        let f = Instant::now();
        app.cx.executor().advance_clock(Duration::from_millis(250));
        app.cx.run_until_parked();
        app.settle();
        worst = worst.max(f.elapsed());
        let st = app.state();
        if matches!(st["run"]["phase"].as_str(), Some("Finished" | "Failed" | "Stopped")) {
            break st;
        }
        assert!(t.elapsed() < Duration::from_secs(180));
        std::thread::sleep(Duration::from_millis(50));
    };
    eprintln!("8,000 samples: finished after {:?}; the slowest redraw while it ran took {worst:?}", t.elapsed());
    assert_eq!(st["run"]["samples"], 8000);
    assert_eq!(server.requests.load(std::sync::atomic::Ordering::Relaxed), 8000);
    assert!(worst < Duration::from_secs(5), "a redraw took {worst:?}");
}

#[gpui::test]
fn viewing_a_large_stored_run_is_quick_and_does_not_block_the_window(cx: &mut TestAppContext) {
    let s = setup("view-large", &shop_plan(1, 1, 1), None, |_| {});
    let root = s.dir.join("runs");
    stored_run(&root, &s.plan, 1, 200_000, Outcome::Finished);
    let mut app = open(cx, &s, false);
    app.keys("ctrl-j");
    app.click("bottom-history");
    let t = Instant::now();
    app.click("history-row-0");
    let st = wait_until(&mut app, Duration::from_secs(120), "200,000 rows", |st| st["run"]["phase"] == "Finished");
    eprintln!("200,000 rows of a stored run read and shown in {:?}", t.elapsed());
    assert_eq!(st["run"]["samples"], 200_000);
    app.settle();
    assert!(app.has("run-chart-response"));
}
