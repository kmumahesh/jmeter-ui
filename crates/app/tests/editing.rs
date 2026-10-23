//! Editing plans through the window: adding, deleting, moving, undo, saving, the recovery copy and the
//! questions asked before work could be lost.

mod common;

use common::*;
use gpui::TestAppContext;
use plan_core::files;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// A copy of a fixture in a fresh folder, so saving cannot touch the committed file.
fn copy_of(name: &str, fixture_name: &str) -> PathBuf {
    let d = temp_dir(name);
    let p = d.join(fixture_name);
    std::fs::copy(fixture(fixture_name), &p).unwrap();
    p
}

fn goto(app: &mut App, name: &str) {
    app.keys("ctrl-p");
    app.input(name);
    app.keys("enter");
}

fn text_of(p: &Path) -> String {
    std::fs::read_to_string(p).unwrap()
}

fn dirty(app: &mut App) -> bool {
    app.state()["dirty"].as_bool().unwrap()
}

fn tab_dirty(app: &mut App, index: usize) -> bool {
    app.state()["tabs"][index]["dirty"].as_bool().unwrap()
}

fn elements(app: &mut App) -> u64 {
    app.state()["elements"].as_u64().unwrap()
}

fn dialog_kind(app: &mut App) -> Option<String> {
    app.state()["dialog"]["kind"].as_str().map(str::to_string)
}

// ---- adding ----

#[gpui::test]
fn adding_from_the_palette_puts_the_element_under_the_selection_and_selects_it(cx: &mut TestAppContext) {
    let p = copy_of("add-palette", WEB);
    let mut app = start(cx, Some(p), None);
    goto(&mut app, "jmeter users");
    let before = elements(&mut app);
    app.keys("ctrl-k");
    app.input("+constant timer");
    assert_eq!(app.palette_titles()[0], "Constant Timer", "the exact name comes before longer names that also match");
    app.keys("enter");
    assert_eq!(elements(&mut app), before + 1);
    assert_eq!(app.selected().as_deref(), Some("Constant Timer"));
    assert_eq!(app.editing().as_deref(), Some("Constant Timer"), "the editor shows the new element");
    assert!(dirty(&mut app) && tab_dirty(&mut app, 0), "the tab carries the unsaved mark");
    assert!(app.state()["notice"].as_str().unwrap().contains("Added Constant Timer"));
    assert!(app.state()["can_undo"].as_bool().unwrap());
}

#[gpui::test]
fn adding_with_nothing_selected_goes_under_the_test_plan(cx: &mut TestAppContext) {
    let mut app = start(cx, Some(copy_of("add-root", WEB)), None);
    assert!(app.selected().is_none());
    app.keys("ctrl-k");
    app.input("+thread group");
    app.keys("enter");
    assert_eq!(app.selected().as_deref(), Some("Thread Group"));
    assert_eq!(app.state()["visible_rows"].as_u64().unwrap(), 16, "the tree grew by the new row");
}

#[gpui::test]
fn clicking_a_name_in_can_add_adds_it_at_once(cx: &mut TestAppContext) {
    let mut app = start(cx, Some(copy_of("add-click", WEB)), None);
    goto(&mut app, "jmeter users");
    app.keys("ctrl-alt-b");
    let before = elements(&mut app);
    app.click("can-add-HTTP Request");
    assert_eq!(elements(&mut app), before + 1);
    assert_eq!(app.selected().as_deref(), Some("HTTP Request"));
    assert_eq!(app.editing().as_deref(), Some("HTTP Request"));
    // The inspector now describes the new sampler, which can hold other things.
    assert!(app.has("can-add-CSV Data Set Config"));
    assert!(!app.has("can-add-HTTP Request"), "samplers do not go under samplers");
    app.click("can-add-CSV Data Set Config");
    assert_eq!(elements(&mut app), before + 2);
    assert_eq!(app.selected().as_deref(), Some("CSV Data Set Config"));
}

#[gpui::test]
fn only_allowed_types_can_be_added_under_the_test_plan(cx: &mut TestAppContext) {
    let mut app = start(cx, Some(copy_of("add-rules", WEB)), None);
    app.click("tree-row-0");
    app.keys("ctrl-k");
    app.input("+http request");
    assert!(app.palette_unavailable().contains(&"HTTP Request".to_string()), "a sampler is listed but cannot be chosen under the test plan");
    assert_eq!(app.palette_titles()[0], "HTTP Request Defaults", "what fits comes first");
    app.keys("down enter");
}

// ---- deleting, duplicating, moving, toggling ----

#[gpui::test]
fn delete_removes_the_element_selects_a_neighbour_and_undo_brings_it_back(cx: &mut TestAppContext) {
    let mut app = start(cx, Some(copy_of("delete", WEB)), None);
    goto(&mut app, "bugdetail");
    let before = elements(&mut app);
    app.click("tree-row-0"); // focus the tree
    goto(&mut app, "bugdetail");
    app.keys("delete");
    assert!(elements(&mut app) < before, "the element and everything under it is gone");
    assert!(app.selected().is_some() && app.selected().as_deref() != Some("BugDetail Page"));
    assert_eq!(app.selected().as_deref(), Some("Login"), "the next sibling is selected");
    assert!(app.state()["notice"].as_str().unwrap().contains("Deleted BugDetail Page"));
    app.keys("ctrl-z");
    assert_eq!(elements(&mut app), before);
    assert_eq!(app.selected().as_deref(), Some("BugDetail Page"), "undo selects what came back");
    assert!(!dirty(&mut app), "back to the saved state");
}

#[gpui::test]
fn the_test_plan_cannot_be_deleted(cx: &mut TestAppContext) {
    let mut app = start(cx, Some(copy_of("delete-root", WEB)), None);
    app.click("tree-row-0");
    app.keys("delete");
    assert_eq!(elements(&mut app), 23);
    assert!(!dirty(&mut app));
    app.keys("ctrl-k");
    app.input(">delete element");
    app.keys("enter");
    assert!(app.palette_open(), "the command is unavailable for the test plan");
}

#[gpui::test]
fn duplicate_copies_the_element_with_its_children(cx: &mut TestAppContext) {
    let mut app = start(cx, Some(copy_of("dup", WEB)), None);
    goto(&mut app, "jmeter users");
    let before = elements(&mut app);
    app.keys("ctrl-d");
    let after = elements(&mut app);
    assert!(after > before + 5, "the thread group's whole subtree was copied: {before} -> {after}");
    assert_eq!(app.selected().as_deref(), Some("JMeter Users"), "the copy has the same name and is selected");
    app.keys("ctrl-z");
    assert_eq!(elements(&mut app), before);
}

#[gpui::test]
fn alt_arrows_move_an_element_among_its_siblings(cx: &mut TestAppContext) {
    let mut app = start(cx, Some(copy_of("move", WEB)), None);
    goto(&mut app, "bugdetail");
    app.click("tree-row-0");
    goto(&mut app, "bugdetail");
    app.keys("alt-up");
    app.keys("up");
    assert_eq!(app.selected().as_deref(), Some("Changes"), "BugDetail Page moved above ThinkTime, so the row above is Changes");
    app.keys("down down");
    assert_eq!(app.selected().as_deref(), Some("ThinkTime"));
    goto(&mut app, "bugdetail");
    app.keys("alt-down");
    assert!(dirty(&mut app));
    app.keys("ctrl-z ctrl-z");
    assert!(!dirty(&mut app));
}

#[gpui::test]
fn ctrl_t_switches_an_element_off_and_on(cx: &mut TestAppContext) {
    let p = copy_of("toggle", WEB);
    let mut app = start(cx, Some(p.clone()), None);
    goto(&mut app, "bugdetail");
    app.keys("ctrl-t");
    app.keys("ctrl-s");
    assert!(text_of(&p).contains("testname=\"BugDetail Page\" enabled=\"false\""), "the file records the change");
    app.keys("ctrl-t ctrl-s");
    assert!(text_of(&p).contains("testname=\"BugDetail Page\" enabled=\"true\""));
}

// ---- undo and redo ----

#[gpui::test]
fn undo_and_redo_walk_through_the_history(cx: &mut TestAppContext) {
    let mut app = start(cx, Some(copy_of("undo", WEB)), None);
    goto(&mut app, "jmeter users");
    app.keys("ctrl-k");
    app.input("+constant timer");
    app.keys("enter");
    assert_eq!(app.selected().as_deref(), Some("Constant Timer"));
    goto(&mut app, "jmeter users");
    app.keys("ctrl-k");
    app.input("+uniform random timer");
    app.keys("enter");
    assert_eq!(app.selected().as_deref(), Some("Uniform Random Timer"));
    assert_eq!(elements(&mut app), 25);
    app.keys("ctrl-z");
    assert_eq!(elements(&mut app), 24);
    assert!(dirty(&mut app), "one edit is still unsaved");
    assert!(app.state()["can_redo"].as_bool().unwrap());
    app.keys("ctrl-z");
    assert_eq!(elements(&mut app), 23);
    assert!(!dirty(&mut app), "undone to the opened state");
    assert!(!app.state()["can_undo"].as_bool().unwrap());
    app.keys("ctrl-y");
    assert_eq!(app.selected().as_deref(), Some("Constant Timer"));
    app.keys("ctrl-y");
    assert_eq!(app.selected().as_deref(), Some("Uniform Random Timer"));
    assert_eq!(elements(&mut app), 25);
    assert!(!app.state()["can_redo"].as_bool().unwrap());
    // Redo is also on Ctrl+Shift+Z.
    app.keys("ctrl-z ctrl-shift-z");
    assert_eq!(elements(&mut app), 25);
}

#[gpui::test]
fn a_new_edit_after_undo_clears_redo(cx: &mut TestAppContext) {
    let mut app = start(cx, Some(copy_of("redo-clear", WEB)), None);
    goto(&mut app, "jmeter users");
    app.keys("ctrl-k");
    app.input("+constant timer");
    app.keys("enter");
    app.keys("ctrl-z");
    assert!(app.state()["can_redo"].as_bool().unwrap());
    goto(&mut app, "jmeter users");
    app.keys("ctrl-k");
    app.input("+debug sampler");
    app.keys("enter");
    assert!(!app.state()["can_redo"].as_bool().unwrap());
}

#[gpui::test]
fn the_edit_commands_are_in_the_palette_with_their_shortcuts(cx: &mut TestAppContext) {
    let mut app = start(cx, Some(copy_of("palette-edit", WEB)), None);
    for (query, title) in [("undo", "Undo"), ("redo", "Redo"), ("save plan as", "Save Plan As…"), ("duplicate", "Duplicate Element"), ("delete", "Delete Element"), ("move element up", "Move Element Up"), ("rename", "Rename Element")] {
        app.keys("ctrl-k");
        app.input(&format!(">{query}"));
        assert!(app.palette_titles().iter().any(|t| t == title), "{query}: {:?}", app.palette_titles());
        app.keys("escape");
    }
}

// ---- saving and the recovery copy ----

#[gpui::test]
fn ctrl_s_saves_the_edits_to_the_file_and_clears_the_unsaved_mark(cx: &mut TestAppContext) {
    let p = copy_of("save", WEB);
    let original = text_of(&p);
    let mut app = start(cx, Some(p.clone()), None);
    goto(&mut app, "jmeter users");
    app.keys("ctrl-k");
    app.input("+debug sampler");
    app.keys("enter");
    assert!(dirty(&mut app));
    assert_eq!(text_of(&p), original, "nothing is written until Save");
    app.keys("ctrl-s");
    assert!(!dirty(&mut app) && !tab_dirty(&mut app, 0));
    let saved = text_of(&p);
    assert_ne!(saved, original);
    assert!(saved.contains("<DebugSampler guiclass=\"TestBeanGUI\" testclass=\"DebugSampler\" testname=\"Debug Sampler\" enabled=\"true\">"), "{saved}");
    assert!(saved.len() > original.len());
    assert!(app.state()["notice"].as_str().unwrap().contains("Saved build-adv-web-test-plan.jmx"));
    // Save is not available again until something changes.
    app.keys("ctrl-k");
    app.input(">save plan");
    assert!(app.palette_unavailable().contains(&"Save Plan".to_string()));
}

#[gpui::test]
fn the_recovery_copy_is_written_after_a_pause_and_removed_on_save(cx: &mut TestAppContext) {
    let p = copy_of("autosave", WEB);
    let mut app = start(cx, Some(p.clone()), None);
    goto(&mut app, "jmeter users");
    app.keys("ctrl-k");
    app.input("+debug sampler");
    app.keys("enter");
    assert!(!files::recovery_path(&p).exists(), "not written at once");
    app.cx.executor().advance_clock(Duration::from_secs(1));
    app.cx.run_until_parked();
    assert!(!files::recovery_path(&p).exists(), "nor after a second");
    app.cx.executor().advance_clock(Duration::from_secs(3));
    app.cx.run_until_parked();
    let bak = files::recovery_path(&p);
    assert!(bak.exists(), "{}", bak.display());
    assert!(text_of(&bak).contains("Debug Sampler"));
    assert!(bak.to_string_lossy().ends_with("build-adv-web-test-plan.jmx.bak"), "next to the plan, named name.jmx.bak");
    assert!(!text_of(&p).contains("Debug Sampler"), "the plan itself is untouched");
    app.keys("ctrl-s");
    assert!(!bak.exists(), "saving removes the recovery copy");
}

#[gpui::test]
fn only_the_latest_edit_writes_the_recovery_copy(cx: &mut TestAppContext) {
    let p = copy_of("autosave-latest", WEB);
    let mut app = start(cx, Some(p.clone()), None);
    goto(&mut app, "jmeter users");
    for t in ["+debug sampler", "+flow control"] {
        app.keys("ctrl-k");
        app.input(t);
        app.keys("enter");
        goto(&mut app, "jmeter users");
        app.cx.executor().advance_clock(Duration::from_secs(1));
        app.cx.run_until_parked();
    }
    app.cx.executor().advance_clock(Duration::from_secs(5));
    app.cx.run_until_parked();
    let bak = text_of(&files::recovery_path(&p));
    assert!(bak.contains("Debug Sampler") && bak.contains("Flow Control Action"), "the copy holds both edits");
}

#[gpui::test]
fn undoing_back_to_the_saved_state_removes_the_recovery_copy(cx: &mut TestAppContext) {
    let p = copy_of("autosave-undo", WEB);
    let mut app = start(cx, Some(p.clone()), None);
    goto(&mut app, "jmeter users");
    app.keys("ctrl-k");
    app.input("+debug sampler");
    app.keys("enter");
    app.cx.executor().advance_clock(Duration::from_secs(4));
    app.cx.run_until_parked();
    assert!(files::recovery_path(&p).exists());
    app.keys("ctrl-z");
    assert!(!files::recovery_path(&p).exists());
}

#[gpui::test]
fn a_recovery_copy_from_an_earlier_session_can_be_recovered(cx: &mut TestAppContext) {
    let p = copy_of("recover", WEB);
    std::thread::sleep(Duration::from_millis(20));
    let edited = text_of(&p).replacen("BugDetail Page", "Recovered Name", 1);
    files::autosave(&p, &edited).unwrap();
    let mut app = start(cx, Some(p.clone()), None);
    assert_eq!(dialog_kind(&mut app).as_deref(), Some("recover"));
    assert!(app.state()["plan"].is_null(), "nothing opens until the question is answered");
    app.click("dialog-recover");
    assert!(dialog_kind(&mut app).is_none());
    assert_eq!(app.tabs(), [WEB]);
    assert!(dirty(&mut app) && tab_dirty(&mut app, 0), "recovered work counts as unsaved");
    goto(&mut app, "recovered name");
    assert_eq!(app.editing().as_deref(), Some("Recovered Name"));
    assert!(!text_of(&p).contains("Recovered Name"), "the plan file is not touched until Save");
    app.keys("ctrl-s");
    assert!(text_of(&p).contains("Recovered Name"));
    assert!(!files::recovery_path(&p).exists());
}

#[gpui::test]
fn the_recovery_copy_can_be_discarded(cx: &mut TestAppContext) {
    let p = copy_of("recover-discard", WEB);
    std::thread::sleep(Duration::from_millis(20));
    files::autosave(&p, &text_of(&p).replacen("BugDetail Page", "Recovered Name", 1)).unwrap();
    let mut app = start(cx, Some(p.clone()), None);
    app.click("dialog-discard-recovery");
    assert!(dialog_kind(&mut app).is_none());
    assert_eq!(app.tabs(), [WEB]);
    assert!(!dirty(&mut app));
    assert!(!files::recovery_path(&p).exists());
    app.keys("ctrl-p");
    app.input("recovered");
    assert!(app.palette_titles().is_empty(), "the saved plan opened");
}

#[gpui::test]
fn a_copy_that_matches_the_file_is_ignored(cx: &mut TestAppContext) {
    let p = copy_of("recover-same", WEB);
    files::autosave(&p, &text_of(&p)).unwrap();
    let mut app = start(cx, Some(p.clone()), None);
    assert!(dialog_kind(&mut app).is_none());
    assert_eq!(elements(&mut app), 23);
}

// ---- unsaved work and leaving ----

#[gpui::test]
fn closing_a_changed_plan_asks_and_cancel_keeps_it(cx: &mut TestAppContext) {
    let p = copy_of("close-cancel", WEB);
    let mut app = start(cx, Some(p), None);
    goto(&mut app, "jmeter users");
    app.keys("ctrl-k");
    app.input("+debug sampler");
    app.keys("enter");
    app.keys("ctrl-w");
    assert_eq!(dialog_kind(&mut app).as_deref(), Some("unsaved"));
    assert_eq!(app.tabs(), [WEB], "still open behind the question");
    app.click("dialog-cancel");
    assert!(dialog_kind(&mut app).is_none());
    assert_eq!(app.tabs(), [WEB]);
    assert!(dirty(&mut app));
}

#[gpui::test]
fn dont_save_closes_and_drops_the_recovery_copy(cx: &mut TestAppContext) {
    let p = copy_of("close-discard", WEB);
    let original = text_of(&p);
    let mut app = start(cx, Some(p.clone()), None);
    goto(&mut app, "jmeter users");
    app.keys("ctrl-k");
    app.input("+debug sampler");
    app.keys("enter");
    app.cx.executor().advance_clock(Duration::from_secs(4));
    app.cx.run_until_parked();
    assert!(files::recovery_path(&p).exists());
    app.keys("ctrl-w");
    app.click("dialog-discard");
    assert!(app.tabs().is_empty());
    assert_eq!(text_of(&p), original);
    assert!(!files::recovery_path(&p).exists());
}

#[gpui::test]
fn save_in_the_question_saves_then_closes(cx: &mut TestAppContext) {
    let p = copy_of("close-save", WEB);
    let mut app = start(cx, Some(p.clone()), None);
    goto(&mut app, "jmeter users");
    app.keys("ctrl-k");
    app.input("+debug sampler");
    app.keys("enter");
    app.keys("ctrl-w");
    app.click("dialog-save");
    assert!(app.tabs().is_empty());
    assert!(text_of(&p).contains("Debug Sampler"));
}

#[gpui::test]
fn closing_an_unchanged_plan_does_not_ask(cx: &mut TestAppContext) {
    let mut app = start(cx, Some(copy_of("close-clean", WEB)), None);
    app.keys("ctrl-w");
    assert!(dialog_kind(&mut app).is_none());
    assert!(app.tabs().is_empty());
}

#[gpui::test]
fn quitting_with_unsaved_changes_asks_first(cx: &mut TestAppContext) {
    let mut app = start(cx, Some(copy_of("quit", WEB)), None);
    goto(&mut app, "jmeter users");
    app.keys("ctrl-k");
    app.input("+debug sampler");
    app.keys("enter");
    app.keys("ctrl-k");
    app.input(">quit");
    app.keys("enter");
    let s = app.state();
    assert_eq!(s["dialog"]["kind"], "unsaved");
    assert_eq!(s["dialog"]["then"], "Quit");
    app.click("dialog-cancel");
    assert!(dialog_kind(&mut app).is_none());
    assert_eq!(app.tabs(), [WEB], "the app is still running");
}

#[gpui::test]
fn close_all_asks_once_for_every_changed_plan(cx: &mut TestAppContext) {
    let a = copy_of("close-all-a", WEB);
    let b = copy_of("close-all-b", "Test.jmx");
    let mut app = start(cx, Some(a), None);
    app.open_path(&b);
    app.keys("ctrl-k");
    app.input("+debug sampler");
    app.keys("enter"); // under the Test plan root: a sampler is refused
    goto(&mut app, "thread group");
    app.keys("ctrl-k");
    app.input("+debug sampler");
    app.keys("enter");
    assert!(dirty(&mut app));
    app.keys("ctrl-k");
    app.input(">close all");
    app.keys("enter");
    assert_eq!(app.state()["dialog"]["plans"], 1, "only the changed plan is listed");
    app.click("dialog-discard");
    assert!(app.tabs().is_empty());
}

#[gpui::test]
fn reloading_a_changed_plan_asks_first(cx: &mut TestAppContext) {
    let p = copy_of("reload-dirty", WEB);
    let mut app = start(cx, Some(p), None);
    goto(&mut app, "jmeter users");
    app.keys("ctrl-k");
    app.input("+debug sampler");
    app.keys("enter");
    let n = elements(&mut app);
    app.keys("ctrl-k");
    app.input(">reload");
    app.keys("enter");
    assert_eq!(dialog_kind(&mut app).as_deref(), Some("reload"));
    app.click("dialog-cancel");
    assert_eq!(elements(&mut app), n, "nothing was thrown away");
    app.keys("ctrl-k");
    app.input(">reload");
    app.keys("enter");
    app.click("dialog-reload");
    assert_eq!(elements(&mut app), 23);
    assert!(!dirty(&mut app));
}

// ---- several plans ----

#[gpui::test]
fn each_plan_has_its_own_history_and_unsaved_mark(cx: &mut TestAppContext) {
    let a = copy_of("two-a", WEB);
    let b = copy_of("two-b", "Test.jmx");
    let mut app = start(cx, Some(a), None);
    goto(&mut app, "jmeter users");
    app.keys("ctrl-k");
    app.input("+debug sampler");
    app.keys("enter");
    app.open_path(&b);
    assert!(!dirty(&mut app), "the second plan is clean");
    assert!(!app.state()["can_undo"].as_bool().unwrap(), "and has no history of its own");
    assert!(tab_dirty(&mut app, 0) && !tab_dirty(&mut app, 1));
    app.click("tab-0");
    assert!(dirty(&mut app));
    assert!(app.state()["can_undo"].as_bool().unwrap());
}

#[gpui::test]
fn save_as_writes_a_new_file_and_renames_the_tab(cx: &mut TestAppContext) {
    let p = copy_of("save-as", WEB);
    let original = text_of(&p);
    let mut app = start(cx, Some(p.clone()), None);
    goto(&mut app, "jmeter users");
    app.keys("ctrl-k");
    app.input("+debug sampler");
    app.keys("enter");
    app.cx.executor().advance_clock(Duration::from_secs(4));
    app.cx.run_until_parked();
    let new = p.with_file_name("copy-of-plan");
    let shell = app.shell.clone();
    app.cx.update(|window, cx| {
        shell.update(cx, |s, cx| {
            let id = s.active_plan.unwrap();
            s.save_plan_as_path(id, &new, window, cx);
        })
    });
    app.cx.run_until_parked();
    let saved_as = p.with_file_name("copy-of-plan.jmx");
    assert!(text_of(&saved_as).contains("Debug Sampler"), "the .jmx extension was added");
    assert_eq!(text_of(&p), original, "the first file is untouched");
    assert_eq!(app.tabs(), ["copy-of-plan.jmx"]);
    assert!(!dirty(&mut app));
    assert!(!files::recovery_path(&p).exists(), "the old recovery copy is gone");
    assert!(app.state()["plan"].as_str().unwrap().ends_with("copy-of-plan.jmx"));
}

#[gpui::test]
fn edits_keep_the_selection_and_the_tree_in_step(cx: &mut TestAppContext) {
    let mut app = start(cx, Some(copy_of("tree-step", WEB)), None);
    goto(&mut app, "jmeter users");
    app.keys("ctrl-k");
    app.input(">collapse all");
    app.keys("enter");
    assert_eq!(app.state()["visible_rows"], 1);
    goto(&mut app, "jmeter users");
    app.keys("ctrl-k");
    app.input("+debug sampler");
    app.keys("enter");
    assert_eq!(app.selected().as_deref(), Some("Debug Sampler"));
    assert!(app.state()["visible_rows"].as_u64().unwrap() >= 4, "the parents were expanded to show the new element");
}

// ---- dragging in the tree ----

#[gpui::test]
fn dropping_onto_an_element_makes_it_the_last_child(cx: &mut TestAppContext) {
    let mut app = start(cx, Some(copy_of("drag-into", WEB)), None);
    // Row 14 is View Results Tree, at the top level; row 6 is the thread group JMeter Users.
    assert!(!app.children_of("JMeter Users").contains(&"View Results Tree".to_string()));
    app.drag("tree-row-14", "drop-into-6");
    assert_eq!(app.children_of("JMeter Users").last().map(String::as_str), Some("View Results Tree"));
    assert!(dirty(&mut app));
    assert_eq!(app.selected().as_deref(), Some("View Results Tree"));
    app.keys("ctrl-z");
    assert!(!app.children_of("JMeter Users").contains(&"View Results Tree".to_string()));
    assert!(!dirty(&mut app));
}

#[gpui::test]
fn dropping_above_or_below_a_row_reorders(cx: &mut TestAppContext) {
    let mut app = start(cx, Some(copy_of("drag-order", WEB)), None);
    let before = app.children_of("JMeter Users");
    assert_eq!(before[1], "Home Page");
    assert_eq!(before.last().map(String::as_str), Some("Login"));
    // Row 12 is BugDetail Page; drop it above row 8, Home Page.
    app.drag("tree-row-12", "drop-before-8");
    let after = app.children_of("JMeter Users");
    assert_eq!(after[1], "BugDetail Page", "{after:?}");
    assert_eq!(after[2], "Home Page");
    assert_eq!(after.len(), before.len());
    app.keys("ctrl-z");
    assert_eq!(app.children_of("JMeter Users"), before);
}

#[gpui::test]
fn dropping_below_the_last_sibling_moves_to_the_end(cx: &mut TestAppContext) {
    let mut app = start(cx, Some(copy_of("drag-after", WEB)), None);
    // Row 8 is Home Page; drop it below row 13, Login (the last child).
    app.drag("tree-row-8", "drop-after-13");
    let after = app.children_of("JMeter Users");
    assert_eq!(after.last().map(String::as_str), Some("Home Page"), "{after:?}");
}

#[gpui::test]
fn a_move_the_rules_forbid_is_refused_with_a_message(cx: &mut TestAppContext) {
    let mut app = start(cx, Some(copy_of("drag-refused", WEB)), None);
    let before = app.plan_text();
    // A sampler (Home Page, row 8) cannot go into a listener (View Results Tree, row 14).
    app.drag("tree-row-8", "drop-into-14");
    assert_eq!(app.plan_text(), before);
    assert!(!dirty(&mut app));
    assert!(app.state()["notice"].as_str().unwrap().contains("cannot go under"));
}

#[gpui::test]
fn the_test_plan_cannot_be_dragged_and_nothing_sits_beside_it(cx: &mut TestAppContext) {
    let mut app = start(cx, Some(copy_of("drag-root", WEB)), None);
    let before = app.plan_text();
    app.drag("tree-row-0", "drop-into-6");
    assert_eq!(app.plan_text(), before, "the test plan stays where it is");
    app.drag("tree-row-14", "drop-before-0");
    assert_eq!(app.plan_text(), before);
    assert!(app.state()["notice"].as_str().unwrap().contains("Test Plan"));
}

#[gpui::test]
fn an_element_cannot_be_dropped_into_its_own_subtree(cx: &mut TestAppContext) {
    let mut app = start(cx, Some(copy_of("drag-cycle", WEB)), None);
    let before = app.plan_text();
    // The thread group (row 6) onto one of its own children (row 8).
    app.drag("tree-row-6", "drop-into-8");
    assert_eq!(app.plan_text(), before);
}

// ---- scope and order ----

fn id_of(app: &mut App, name: &str) -> u32 {
    let shell = app.shell.clone();
    let name = name.to_string();
    app.cx.update(|_, cx| shell.read(cx).plan().unwrap().plan.find_by_name(&name).unwrap().0)
}

fn leak(s: String) -> &'static str {
    Box::leak(s.into_boxed_str())
}

#[gpui::test]
fn the_inspector_lists_what_applies_to_a_sampler_and_clicking_selects_it(cx: &mut TestAppContext) {
    let mut app = start(cx, Some(copy_of("scope-sampler", WEB)), None);
    goto(&mut app, "bugdetail");
    app.keys("ctrl-alt-b");
    let defaults = id_of(&mut app, "loginData");
    assert!(app.has(leak(format!("scope-{defaults}"))), "the CSV data set at the top applies to every request");
    let own = id_of(&mut app, "BugDetail Page");
    assert!(app.has(leak(format!("scope-{own}"))), "the sampler is listed among its own steps");
    app.click(leak(format!("scope-{defaults}")));
    assert_eq!(app.selected().as_deref(), Some("loginData"));
    assert_eq!(app.editing().as_deref(), Some("loginData"), "the editor followed");
}

#[gpui::test]
fn the_inspector_shows_what_a_config_element_applies_to(cx: &mut TestAppContext) {
    let mut app = start(cx, Some(copy_of("scope-config", WEB)), None);
    goto(&mut app, "loginData");
    app.keys("ctrl-alt-b");
    let home = id_of(&mut app, "Home Page");
    assert!(app.has(leak(format!("scope-{home}"))), "a sampler in its scope");
}

#[gpui::test]
fn scope_follows_edits(cx: &mut TestAppContext) {
    let mut app = start(cx, Some(copy_of("scope-edit", WEB)), None);
    goto(&mut app, "bugdetail");
    app.keys("ctrl-alt-b");
    app.keys("ctrl-k");
    app.input("+response assertion");
    app.keys("enter");
    // The new assertion sits under the sampler, so it is part of the sampler's run... and it is selected.
    let a = id_of(&mut app, "Response Assertion");
    assert_eq!(app.selected().as_deref(), Some("Response Assertion"));
    let bug = id_of(&mut app, "BugDetail Page");
    assert!(app.has(leak(format!("scope-{bug}"))), "an assertion applies to the sampler it is under");
    let _ = a;
}

// ---- big plans ----

#[gpui::test]
fn a_plan_of_twenty_thousand_elements_stays_usable(cx: &mut TestAppContext) {
    let mut s = String::from("<jmeterTestPlan><hashTree><TestPlan testname=\"Big\"/><hashTree>");
    for g in 0..200 {
        s.push_str(&format!("<ThreadGroup testname=\"G{g}\"/><hashTree>"));
        for r in 0..100 {
            s.push_str(&format!("<HTTPSamplerProxy guiclass=\"HttpTestSampleGui\" testname=\"GET /g{g}/r{r}\"><stringProp name=\"HTTPSampler.path\">/g{g}/r{r}</stringProp></HTTPSamplerProxy><hashTree/>"));
        }
        s.push_str("</hashTree>");
    }
    s.push_str("</hashTree></hashTree></jmeterTestPlan>");
    let d = temp_dir("big");
    let p = d.join("big.jmx");
    std::fs::write(&p, s).unwrap();
    let t = std::time::Instant::now();
    let mut app = start(cx, Some(p), None);
    let open = t.elapsed();
    assert_eq!(elements(&mut app), 20_201);
    goto(&mut app, "g150 r42");
    assert_eq!(app.editing().as_deref(), Some("GET /g150/r42"), "go to element picked the request");
    let t = std::time::Instant::now();
    app.type_into("field-HTTPSampler.path", "/changed");
    let edit = t.elapsed();
    assert_eq!(app.prop("HTTPSampler.path").as_deref(), Some("/changed"));
    let t = std::time::Instant::now();
    app.keys("ctrl-k");
    app.input("+constant timer");
    app.keys("enter");
    let add = t.elapsed();
    eprintln!("20k elements: open {open:?}, edit a field {edit:?}, add an element {add:?}");
    // Debug builds on a slow machine: the point is that nothing is quadratic or re-reads the whole file per key.
    assert!(open.as_secs() < 10, "{open:?}");
    assert!(edit.as_secs() < 5, "{edit:?}");
    assert!(add.as_secs() < 5, "{add:?}");
}
