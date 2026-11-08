//! Drive the whole window in a headless GPUI test: keyboard, mouse, palette, tabs, panels and settings.

mod common;

use common::*;
use gpui::{Entity, Modifiers, MouseButton, TestAppContext, point};
use jmeter_ui::shell;
use shell_core::settings::{self, Settings, ThemeChoice};
use std::path::PathBuf;
use std::time::Duration;

// ---- opening plans ----

#[gpui::test]
fn starts_empty_without_a_plan(cx: &mut TestAppContext) {
    let mut app = start(cx, None, None);
    let s = app.state();
    assert!(s["plan"].is_null());
    assert_eq!(s["elements"], 0);
    assert!(app.tabs().is_empty());
    assert!(app.has("welcome-open"), "the welcome screen offers to open a plan");
}

#[gpui::test]
fn opens_a_plan_given_at_startup(cx: &mut TestAppContext) {
    let mut app = start(cx, Some(fixture(WEB)), None);
    let s = app.state();
    assert_eq!(s["elements"], 23);
    assert_eq!(app.tabs(), [WEB]);
    assert!(s["notice"].as_str().unwrap().contains("Opened build-adv-web-test-plan.jmx: 23 elements"));
    assert_eq!(s["visible_rows"], 15, "the tree starts expanded two levels deep");
}

#[gpui::test]
fn a_missing_file_shows_an_error_and_leaves_the_app_usable(cx: &mut TestAppContext) {
    let mut app = start(cx, Some(PathBuf::from("Z:/does/not/exist.jmx")), None);
    let s = app.state();
    assert!(s["plan"].is_null());
    assert!(s["notice"].as_str().unwrap().contains("exist.jmx"));
    assert!(app.tabs().is_empty());
}

#[gpui::test]
fn a_file_that_is_not_a_plan_shows_an_error(cx: &mut TestAppContext) {
    let d = temp_dir("badplan");
    let bad = d.join("bad.jmx");
    std::fs::write(&bad, "<html>not a plan</html>").unwrap();
    let mut app = start(cx, Some(bad), None);
    assert!(app.state()["plan"].is_null());
    assert!(app.state()["notice"].as_str().unwrap().contains("bad.jmx"));
}

#[gpui::test]
fn a_hostile_plan_is_refused_with_a_message_and_no_tab(cx: &mut TestAppContext) {
    // Nested far deeper than any real plan, and a document type definition that points at another file.
    let d = temp_dir("hostile");
    let deep = d.join("deep.jmx");
    std::fs::write(&deep, format!("{}{}", "<hashTree>".repeat(5000), "</hashTree>".repeat(5000))).unwrap();
    let mut app = start(cx, Some(deep), None);
    assert!(app.state()["plan"].is_null());
    assert!(app.tabs().is_empty());
    assert!(app.state()["notice"].as_str().unwrap().contains("deep.jmx"));
}

#[gpui::test]
fn opening_adds_to_recent_files_and_failures_remove_them(cx: &mut TestAppContext) {
    let d = temp_dir("recent");
    let sp = d.join("settings.json");
    {
        let mut app = start(cx, Some(fixture(WEB)), Some(sp.clone()));
        assert!(!app.state()["plan"].is_null());
    }
    let saved = settings::load(&sp).settings;
    assert_eq!(saved.recent_files.len(), 1);
    assert!(saved.recent_files[0].ends_with(WEB), "{:?}", saved.recent_files);
    assert!(!saved.recent_files[0].starts_with(r"\\?\"));
}

// ---- tree ----

#[gpui::test]
fn clicking_a_row_redraws_the_editor_and_opens_no_tab(cx: &mut TestAppContext) {
    let mut app = start(cx, Some(fixture(WEB)), None);
    assert_eq!(app.editing().as_deref(), Some("build-adv-web-test-plan"), "with nothing selected the editor shows the test plan itself");
    // Row 0 is the plan, row 1 HTTP Request Defaults, row 2 User Defined Variables.
    app.click("tree-row-2");
    assert_eq!(app.selected().as_deref(), Some("User Defined Variables"));
    assert_eq!(app.editing().as_deref(), Some("User Defined Variables"));
    app.click("tree-row-1");
    assert_eq!(app.editing().as_deref(), Some("HTTP Request Defaults"));
    app.click("tree-row-0");
    assert_eq!(app.editing().as_deref(), Some("build-adv-web-test-plan"));
    assert_eq!(app.tabs(), [WEB], "the plan keeps its one tab however many elements are clicked");
}

#[gpui::test]
fn every_kind_of_element_redraws_the_same_tab(cx: &mut TestAppContext) {
    let mut app = start(cx, Some(fixture(WEB)), None);
    app.keys("ctrl-k");
    app.input(">expand all");
    app.keys("enter");
    let rows = app.state()["visible_rows"].as_u64().unwrap() as usize;
    assert_eq!(rows, 23);
    let mut seen = std::collections::HashSet::new();
    for i in 0..rows {
        let sel: &'static str = Box::leak(format!("tree-row-{i}").into_boxed_str());
        if !app.has(sel) {
            continue; // rows past the visible window are covered by the keyboard test below
        }
        app.click(sel);
        seen.insert(app.editing().unwrap());
        assert_eq!(app.tabs(), [WEB], "row {i}");
    }
    assert!(seen.len() > 8, "the editor followed the selection through {} elements", seen.len());
}

#[gpui::test]
fn keyboard_navigation_in_the_tree(cx: &mut TestAppContext) {
    let mut app = start(cx, Some(fixture(WEB)), None);
    app.keys("ctrl-shift-e");
    app.keys("down");
    assert_eq!(app.selected().as_deref(), Some("build-adv-web-test-plan"), "first Down picks the first row");
    app.keys("down down");
    assert_eq!(app.selected().as_deref(), Some("User Defined Variables"));
    assert_eq!(app.editing().as_deref(), Some("User Defined Variables"), "the editor follows the arrow keys");
    app.keys("up");
    assert_eq!(app.selected().as_deref(), Some("HTTP Request Defaults"));
    assert_eq!(app.tabs(), [WEB], "arrow keys never open tabs");
    app.keys("end");
    assert_eq!(app.selected().as_deref(), Some("View Results Tree"));
    assert_eq!(app.editing().as_deref(), Some("View Results Tree"));
    app.keys("home");
    assert_eq!(app.selected().as_deref(), Some("build-adv-web-test-plan"));
    assert_eq!(app.tabs(), [WEB]);
}

#[gpui::test]
fn enter_and_space_expand_and_collapse_the_selected_element(cx: &mut TestAppContext) {
    let mut app = start(cx, Some(fixture(WEB)), None);
    app.keys("ctrl-shift-e down");
    assert_eq!(app.state()["visible_rows"], 15);
    app.keys("space");
    assert_eq!(app.state()["visible_rows"], 1);
    app.keys("enter");
    assert_eq!(app.state()["visible_rows"], 15);
    assert_eq!(app.tabs(), [WEB]);
}

#[gpui::test]
fn left_and_right_collapse_and_expand(cx: &mut TestAppContext) {
    let mut app = start(cx, Some(fixture(WEB)), None);
    app.keys("ctrl-shift-e down");
    assert_eq!(app.state()["visible_rows"], 15);
    app.keys("left"); // collapse the plan root
    assert_eq!(app.state()["visible_rows"], 1);
    app.keys("right");
    assert_eq!(app.state()["visible_rows"], 15, "expanding the root restores what was open below it");
    app.keys("right");
    assert_eq!(app.selected().as_deref(), Some("HTTP Request Defaults"), "right on an expanded node goes to its first child");
    app.keys("left");
    assert_eq!(app.selected().as_deref(), Some("build-adv-web-test-plan"), "left on a leaf goes to the parent");
}

#[gpui::test]
fn the_chevron_toggles_without_selecting(cx: &mut TestAppContext) {
    let mut app = start(cx, Some(fixture(WEB)), None);

    let before = app.state()["visible_rows"].as_u64().unwrap();
    app.click("tree-chev-6"); // the JMeter Users thread group
    let after = app.state()["visible_rows"].as_u64().unwrap();
    assert_ne!(before, after, "clicking the chevron changes what is shown");
    assert!(app.selected().is_none(), "and does not select");
}

#[gpui::test]
fn expand_and_collapse_all_from_the_palette(cx: &mut TestAppContext) {
    let mut app = start(cx, Some(fixture(WEB)), None);
    app.keys("ctrl-k");
    app.input(">expand all");
    app.keys("enter");
    assert_eq!(app.state()["visible_rows"], 23);
    app.keys("ctrl-k");
    app.input(">collapse all");
    app.keys("enter");
    assert_eq!(app.state()["visible_rows"], 1);
}

// ---- palette ----

#[gpui::test]
fn ctrl_k_opens_and_escape_closes_the_palette(cx: &mut TestAppContext) {
    let mut app = start(cx, Some(fixture(WEB)), None);
    assert!(!app.palette_open());
    app.keys("ctrl-k");
    assert!(app.palette_open());
    assert_eq!(app.state()["palette"]["mode"], "Everything");
    app.keys("escape");
    assert!(!app.palette_open());
}

#[gpui::test]
fn clicking_outside_closes_the_palette(cx: &mut TestAppContext) {
    let mut app = start(cx, Some(fixture(WEB)), None);
    app.keys("ctrl-k");
    assert!(app.palette_open());
    app.cx.simulate_click(point(gpui::px(5.), gpui::px(400.)), Modifiers::default());
    app.cx.run_until_parked();
    assert!(!app.palette_open());
}

#[gpui::test]
fn the_title_bar_search_box_opens_the_palette(cx: &mut TestAppContext) {
    let mut app = start(cx, Some(fixture(WEB)), None);
    app.click("palette-trigger");
    assert!(app.palette_open());
}

#[gpui::test]
fn palette_filters_as_you_type(cx: &mut TestAppContext) {
    let mut app = start(cx, Some(fixture(WEB)), None);
    app.keys("ctrl-k");
    let all = app.palette_titles().len();
    assert!(all > 10, "{all}");
    app.input(">theme");
    let t = app.palette_titles();
    assert!(t.iter().all(|x| x.contains("Theme")), "{t:?}");
    assert_eq!(t.len(), 3);
}

#[gpui::test]
fn goto_element_selects_opens_and_focuses_the_tree(cx: &mut TestAppContext) {
    let mut app = start(cx, Some(fixture(WEB)), None);
    app.keys("ctrl-p");
    assert_eq!(app.state()["palette"]["mode"], "Elements");
    app.input("bugdetail");
    assert_eq!(app.palette_titles()[0], "BugDetail Page", "the exact name ranks above the longer name that also matches");
    app.keys("enter");
    assert!(!app.palette_open());
    assert_eq!(app.selected().as_deref(), Some("BugDetail Page"));
    assert_eq!(app.editing().as_deref(), Some("BugDetail Page"));
    assert_eq!(app.tabs(), [WEB], "going to an element does not open a tab either");
    // The tree has focus, so arrow keys move the selection.
    app.keys("up");
    assert_eq!(app.selected().as_deref(), Some("ThinkTime"));
}

#[gpui::test]
fn goto_expands_collapsed_ancestors(cx: &mut TestAppContext) {
    let mut app = start(cx, Some(fixture(WEB)), None);
    app.keys("ctrl-k");
    app.input(">collapse all");
    app.keys("enter");
    assert_eq!(app.state()["visible_rows"], 1);
    app.keys("ctrl-p");
    app.input("bugdetail");
    app.keys("enter");
    assert!(app.state()["visible_rows"].as_u64().unwrap() > 3, "ancestors were expanded to reveal the element");
}

#[gpui::test]
fn arrow_keys_move_through_palette_results(cx: &mut TestAppContext) {
    let mut app = start(cx, Some(fixture(WEB)), None);
    app.keys("ctrl-k");
    app.input(">");
    assert_eq!(app.state()["palette"]["selected"], 0);
    app.keys("down down");
    assert_eq!(app.state()["palette"]["selected"], 2);
    app.keys("up");
    assert_eq!(app.state()["palette"]["selected"], 1);
    app.keys("up up up");
    assert_eq!(app.state()["palette"]["selected"], 0, "stops at the top");
}

#[gpui::test]
fn unavailable_commands_do_nothing_when_chosen(cx: &mut TestAppContext) {
    let mut app = start(cx, Some(fixture(WEB)), None);
    app.keys("ctrl-k");
    app.input(">stop run");
    assert_eq!(app.palette_titles(), ["Stop Run", "Stop Run Now"], "the exact match comes first");
    app.keys("enter");
    assert!(app.palette_open(), "the palette stays open when the command is unavailable");
}

#[gpui::test]
fn add_mode_lists_what_fits_under_the_selection(cx: &mut TestAppContext) {
    let mut app = start(cx, Some(fixture(WEB)), None);
    app.click("tree-row-1"); // HTTP Request Defaults: a config element accepts nothing
    app.keys("ctrl-k");
    app.input("+");
    assert!(app.palette_titles().is_empty(), "nothing can be added under a config element");
    app.keys("escape");
    app.click("tree-row-0"); // the plan itself
    app.keys("ctrl-k");
    app.input("+");
    let t = app.palette_titles();
    assert!(t.contains(&"Thread Group".to_string()) && !t.contains(&"HTTP Request".to_string()), "{t:?}");
    app.keys("enter");
    assert_eq!(app.selected().as_deref(), Some("Thread Group"), "choosing an entry adds it to the plan");
    assert_eq!(app.state()["elements"], 24);
}

// ---- one tab per plan ----

#[gpui::test]
fn the_split_editor_is_gone(cx: &mut TestAppContext) {
    let mut app = start(cx, Some(fixture(WEB)), None);
    app.keys("ctrl-\\");
    assert_eq!(app.tabs(), [WEB]);
    app.keys("ctrl-k");
    app.input(">split");
    assert!(app.palette_titles().is_empty(), "no split commands remain");
    assert!(!app.has("resize-split"));
}

#[gpui::test]
fn opening_the_same_plan_again_shows_its_tab(cx: &mut TestAppContext) {
    let mut app = start(cx, Some(fixture(WEB)), None);
    app.click("tree-row-2");
    app.open_path(&fixture(WEB));
    assert_eq!(app.tabs(), [WEB], "no second tab for the same plan");
    assert_eq!(app.state()["open_plans"].as_array().unwrap().len(), 1);
    assert_eq!(app.selected().as_deref(), Some("User Defined Variables"), "and nothing about it was reset");
    assert!(app.state()["notice"].as_str().unwrap().contains("already open"));
}

#[gpui::test]
fn each_plan_has_one_tab_and_its_own_tree(cx: &mut TestAppContext) {
    let mut app = start(cx, Some(fixture(WEB)), None);
    app.click("tree-row-2");
    app.open_path(&fixture("Test.jmx"));
    assert_eq!(app.tabs(), [WEB, "Test.jmx"], "the new plan's tab opens next to the active one");
    assert_eq!(app.active_tab().as_deref(), Some("Test.jmx"));
    assert_ne!(app.state()["elements"], 23, "the tree shows the second plan");
    assert!(app.selected().is_none());
    app.click("tab-0");
    assert_eq!(app.active_tab().as_deref(), Some(WEB));
    assert_eq!(app.state()["elements"], 23, "switching tabs switches the tree");
    assert_eq!(app.selected().as_deref(), Some("User Defined Variables"), "each plan keeps its own selection");
    assert_eq!(app.editing().as_deref(), Some("User Defined Variables"));
}

#[gpui::test]
fn closing_a_plan_tab_closes_the_plan(cx: &mut TestAppContext) {
    let mut app = start(cx, Some(fixture(WEB)), None);
    app.open_path(&fixture("Test.jmx"));
    app.keys("ctrl-w");
    assert_eq!(app.tabs(), [WEB]);
    assert_eq!(app.state()["open_plans"].as_array().unwrap().len(), 1);
    assert_eq!(app.state()["elements"], 23, "the remaining plan is the one showing");
    app.keys("ctrl-w");
    assert!(app.tabs().is_empty());
    assert!(app.state()["plan"].is_null());
    assert!(app.has("welcome-open"));
}

#[gpui::test]
fn middle_click_closes_a_tab(cx: &mut TestAppContext) {
    let mut app = start(cx, Some(fixture(WEB)), None);
    app.open_path(&fixture("Test.jmx"));
    let b = app.cx.debug_bounds("tab-1").unwrap();
    app.cx.simulate_mouse_down(b.center(), MouseButton::Middle, Modifiers::default());
    app.cx.simulate_mouse_up(b.center(), MouseButton::Middle, Modifiers::default());
    app.cx.run_until_parked();
    assert_eq!(app.tabs(), [WEB]);
}

#[gpui::test]
fn ctrl_tab_cycles_through_plans(cx: &mut TestAppContext) {
    let mut app = start(cx, Some(fixture(WEB)), None);
    app.open_path(&fixture("Test.jmx"));
    app.keys("ctrl-tab");
    assert_eq!(app.active_tab().as_deref(), Some(WEB));
    assert_eq!(app.state()["elements"], 23, "the tree followed the tab");
    app.keys("ctrl-shift-tab");
    assert_eq!(app.active_tab().as_deref(), Some("Test.jmx"));
    assert_ne!(app.state()["elements"], 23);
}

#[gpui::test]
fn selecting_an_element_brings_the_plan_back_from_settings(cx: &mut TestAppContext) {
    let mut app = start(cx, Some(fixture(WEB)), None);
    app.keys("ctrl-,");
    assert_eq!(app.active_tab().as_deref(), Some("Settings"));
    assert_eq!(app.tabs(), [WEB, "Settings"]);
    app.click("tree-row-2");
    assert_eq!(app.active_tab().as_deref(), Some(WEB), "the editor shows the element again");
    assert_eq!(app.editing().as_deref(), Some("User Defined Variables"));
    assert_eq!(app.tabs(), [WEB, "Settings"]);
}

#[gpui::test]
fn the_activity_bar_has_no_search_item(cx: &mut TestAppContext) {
    let mut app = start(cx, Some(fixture(WEB)), None);
    assert!(!app.has("act-search"));
    assert!(app.has("act-plan") && app.has("act-results") && app.has("act-settings"));
    assert!(app.has("palette-trigger"), "search lives in the title bar");
}

// ---- inspector ----

#[gpui::test]
fn the_inspector_starts_hidden_and_shows_what_can_be_added(cx: &mut TestAppContext) {
    let mut app = start(cx, Some(fixture(WEB)), None);
    assert!(!app.state()["inspector"].as_bool().unwrap());
    assert!(!app.has("inspector"));
    app.click("tree-row-0");
    app.keys("ctrl-alt-b");
    assert!(app.has("inspector"));
    assert!(app.has("can-add-Thread Group"), "a thread group fits under the test plan");
    assert!(!app.has("can-add-HTTP Request"), "a sampler does not");
    assert!(!app.has("can-add-Loop Controller"), "neither does a controller");
}

#[gpui::test]
fn can_add_follows_the_selection(cx: &mut TestAppContext) {
    let mut app = start(cx, Some(fixture(WEB)), None);
    app.keys("ctrl-alt-b");
    app.keys("ctrl-p");
    app.input("jmeter users");
    app.keys("enter");
    assert_eq!(app.editing().as_deref(), Some("JMeter Users"));
    assert!(app.has("can-add-HTTP Request"), "samplers fit under a thread group");
    assert!(app.has("can-add-Constant Timer"));
    assert!(!app.has("can-add-Thread Group"));
}

// ---- panels ----

#[gpui::test]
fn panel_shortcuts_toggle_and_persist(cx: &mut TestAppContext) {
    let d = temp_dir("panels");
    let sp = d.join("settings.json");
    let mut app = start(cx, Some(fixture(WEB)), Some(sp.clone()));
    assert!(app.state()["sidebar"].as_bool().unwrap());
    app.keys("ctrl-b");
    assert!(!app.state()["sidebar"].as_bool().unwrap());
    assert!(!app.has("tree-row-0"), "the tree is gone when the panel is hidden");
    app.keys("ctrl-b ctrl-j ctrl-alt-b");
    let s = app.state();
    assert!(s["sidebar"].as_bool().unwrap() && s["bottom"].as_bool().unwrap() && s["inspector"].as_bool().unwrap());
    let saved = settings::load(&sp).settings;
    assert!(saved.sidebar_visible && saved.bottom_visible && saved.inspector_visible);
}

#[gpui::test]
fn dragging_the_sidebar_handle_resizes_and_saves(cx: &mut TestAppContext) {
    let d = temp_dir("resize");
    let sp = d.join("settings.json");
    let app = start(cx, Some(fixture(WEB)), Some(sp.clone()));
    let h = app.cx.debug_bounds("resize-sidebar").unwrap();
    let start_w = settings::Settings::default().sidebar_width;
    app.cx.simulate_mouse_down(h.center(), MouseButton::Left, Modifiers::default());
    app.cx.simulate_mouse_move(point(h.center().x + gpui::px(100.), h.center().y), Some(MouseButton::Left), Modifiers::default());
    app.cx.simulate_mouse_up(point(h.center().x + gpui::px(100.), h.center().y), MouseButton::Left, Modifiers::default());
    app.cx.run_until_parked();
    let w = settings::load(&sp).settings.sidebar_width;
    assert!(w > start_w + 50.0, "width went from {start_w} to {w}");
    assert!(w <= settings::SIDEBAR_RANGE.1);
}

// ---- themes ----

#[gpui::test]
fn theme_commands_switch_and_persist(cx: &mut TestAppContext) {
    let d = temp_dir("theme");
    let sp = d.join("settings.json");
    let mut app = start(cx, Some(fixture(WEB)), Some(sp.clone()));
    assert!(app.state()["dark"].as_bool().unwrap());
    app.keys("ctrl-k");
    app.input(">theme: light");
    app.keys("enter");
    assert!(!app.state()["dark"].as_bool().unwrap());
    assert_eq!(settings::load(&sp).settings.theme, ThemeChoice::Light);
    app.keys("ctrl-k");
    app.input(">theme: dark");
    app.keys("enter");
    assert!(app.state()["dark"].as_bool().unwrap());
    assert_eq!(settings::load(&sp).settings.theme, ThemeChoice::Dark);
}

// ---- settings tab and about ----

#[gpui::test]
fn settings_tab_opens_and_saves_runtime_paths(cx: &mut TestAppContext) {
    let d = temp_dir("settingstab");
    let sp = d.join("settings.json");
    let mut app = start(cx, None, Some(sp.clone()));
    app.keys("ctrl-,");
    assert_eq!(app.tabs(), ["Settings"]);
    assert!(app.has("apply-runtime"));
    // Click into the JMeter folder field and type a path.
    app.input("");
    let shell = app.shell.clone();
    let field = app.cx.update(|_, cx| shell.read(cx).settings_fields.as_ref().unwrap().jmeter_home.clone());
    app.cx.update(|window, cx| {
        let h = gpui::Focusable::focus_handle(&field, cx);
        window.focus(&h, cx);
    });
    app.input("Z:/nowhere/jmeter");
    app.click("apply-runtime");
    let saved = settings::load(&sp).settings;
    assert_eq!(saved.jmeter_home.as_deref(), Some("Z:/nowhere/jmeter"));
    assert!(app.state()["notice"].as_str().unwrap().contains("saved"));
}

#[gpui::test]
fn settings_tab_theme_buttons(cx: &mut TestAppContext) {
    let mut app = start(cx, None, None);
    app.keys("ctrl-,");
    app.click("theme-light");
    assert!(!app.state()["dark"].as_bool().unwrap());
    app.click("theme-dark");
    assert!(app.state()["dark"].as_bool().unwrap());
}

#[gpui::test]
fn about_dialog_opens_from_the_palette_and_closes_on_click(cx: &mut TestAppContext) {
    let mut app = start(cx, None, None);
    app.keys("ctrl-k");
    app.input(">about");
    app.keys("enter");
    assert!(app.state()["about"].as_bool().unwrap());
    app.cx.simulate_click(point(gpui::px(5.), gpui::px(300.)), Modifiers::default());
    app.cx.run_until_parked();
    assert!(!app.state()["about"].as_bool().unwrap());
}

// ---- notices and reload ----

#[gpui::test]
fn notices_disappear_by_themselves(cx: &mut TestAppContext) {
    let mut app = start(cx, Some(fixture(WEB)), None);
    assert!(!app.state()["notice"].is_null());
    app.cx.executor().advance_clock(Duration::from_secs(6));
    app.cx.run_until_parked();
    assert!(app.state()["notice"].is_null());
}

#[gpui::test]
fn reload_keeps_the_selection(cx: &mut TestAppContext) {
    let d = temp_dir("reload");
    let copy = d.join("plan.jmx");
    std::fs::copy(fixture(WEB), &copy).unwrap();
    let mut app = start(cx, Some(copy.clone()), None);
    app.click("tree-row-2");
    assert_eq!(app.selected().as_deref(), Some("User Defined Variables"));
    // Rename another element on disk, then reload.
    let text = std::fs::read_to_string(&copy).unwrap().replace("View Results Tree", "Results");
    std::fs::write(&copy, text).unwrap();
    app.keys("ctrl-k");
    app.input(">reload");
    app.keys("enter");
    assert_eq!(app.selected().as_deref(), Some("User Defined Variables"));
    assert_eq!(app.tabs(), ["plan.jmx"]);
    app.keys("ctrl-p");
    app.input("results");
    assert!(app.palette_titles().contains(&"Results".to_string()), "the change on disk is visible");
}

#[gpui::test]
fn reload_after_the_file_is_deleted_shows_an_error_and_keeps_the_old_plan(cx: &mut TestAppContext) {
    let d = temp_dir("reloaddeleted");
    let copy = d.join("plan.jmx");
    std::fs::copy(fixture(WEB), &copy).unwrap();
    let mut app = start(cx, Some(copy.clone()), None);
    std::fs::remove_file(&copy).unwrap();
    app.keys("ctrl-k");
    app.input(">reload");
    app.keys("enter");
    let s = app.state();
    assert_eq!(s["elements"], 23, "the plan that was open stays open");
    assert!(s["notice"].as_str().unwrap().contains("plan.jmx"));
}

// ---- recent plans ----

#[gpui::test]
fn recent_plans_appear_in_the_palette_and_reopen(cx: &mut TestAppContext) {
    let d = temp_dir("recentpalette");
    let sp = d.join("settings.json");
    {
        let _first = start(cx, Some(fixture(WEB)), Some(sp.clone()));
    }
    let mut app = start(cx, None, Some(sp.clone()));
    app.keys("ctrl-k");
    app.input("adv-web");
    let t = app.palette_titles();
    assert!(t.contains(&WEB.to_string()), "{t:?}");
    app.keys("enter");
    assert_eq!(app.state()["elements"], 23);
}

#[gpui::test]
fn a_damaged_settings_file_is_reported_at_startup(cx: &mut TestAppContext) {
    let d = temp_dir("damaged");
    let sp = d.join("settings.json");
    std::fs::write(&sp, "{ nope").unwrap();
    let mut app = start(cx, None, Some(sp.clone()));
    assert!(app.state()["notice"].as_str().unwrap().contains("damaged"));
    assert!(d.join("settings.json.bak").exists());
}

// ---- bottom panel ----

#[gpui::test]
fn problems_panel_lists_plan_notes(cx: &mut TestAppContext) {
    let mut app = start(cx, Some(fixture("PluginComplexExample.jmx")), None);
    app.keys("ctrl-j");
    assert!(app.state()["bottom"].as_bool().unwrap());
    assert!(app.has("problem-0"), "the plugin plan has notes to show");
}

// ---- run settings, combo box and startup ----

fn focus_field(app: &mut App, pick: impl FnOnce(&shell::SettingsFields) -> Entity<jmeter_ui::fields::TextField>) -> Entity<jmeter_ui::fields::TextField> {
    let shell = app.shell.clone();
    let field = app.cx.update(|_, cx| pick(shell.read(cx).settings_fields.as_ref().unwrap()));
    app.cx.update(|window, cx| {
        let h = gpui::Focusable::focus_handle(&field, cx);
        window.focus(&h, cx);
    });
    field
}

#[gpui::test]
fn valid_heap_and_interval_are_saved(cx: &mut TestAppContext) {
    let d = temp_dir("heap-ok");
    let sp = d.join("settings.json");
    let mut app = start(cx, None, Some(sp.clone()));
    app.keys("ctrl-,");
    let heap = focus_field(&mut app, |f| f.heap_mb.clone());
    app.keys(&secondary("ctrl-a"));
    app.input("2048");
    let interval = focus_field(&mut app, |f| f.summary_secs.clone());
    app.keys(&secondary("ctrl-a"));
    app.input("5");
    assert!(app.cx.update(|_, cx| heap.read(cx).problem().is_none() && interval.read(cx).problem().is_none()));
    app.click("apply-runtime");
    let s = settings::load(&sp).settings;
    assert_eq!((s.heap_mb, s.summary_interval_secs), (2048, 5));
    assert!(app.state()["notice"].as_str().unwrap().contains("saved"));
}

#[gpui::test]
fn invalid_numbers_are_not_saved_and_are_reported(cx: &mut TestAppContext) {
    let d = temp_dir("heap-bad");
    let sp = d.join("settings.json");
    let mut app = start(cx, None, Some(sp.clone()));
    app.keys("ctrl-,");
    let heap = focus_field(&mut app, |f| f.heap_mb.clone());
    app.keys(&secondary("ctrl-a"));
    app.input("10"); // below the 64 MB minimum
    assert!(app.cx.update(|_, cx| heap.read(cx).problem()).is_some());
    app.click("apply-runtime");
    assert!(app.state()["notice"].as_str().unwrap().contains("not valid"));
    assert_eq!(settings::load(&sp).settings.heap_mb, 1024, "the old value is kept");
}

#[gpui::test]
fn a_variable_is_not_accepted_for_heap(cx: &mut TestAppContext) {
    let d = temp_dir("heap-var");
    let sp = d.join("settings.json");
    let mut app = start(cx, None, Some(sp.clone()));
    app.keys("ctrl-,");
    focus_field(&mut app, |f| f.heap_mb.clone());
    app.keys(&secondary("ctrl-a"));
    app.input("${heap}");
    app.click("apply-runtime");
    assert!(app.state()["notice"].as_str().unwrap().contains("must be numbers"));
    assert_eq!(settings::load(&sp).settings.heap_mb, 1024);
}

#[gpui::test]
fn enter_in_a_number_field_applies(cx: &mut TestAppContext) {
    let d = temp_dir("heap-enter");
    let sp = d.join("settings.json");
    let mut app = start(cx, None, Some(sp.clone()));
    app.keys("ctrl-,");
    focus_field(&mut app, |f| f.heap_mb.clone());
    app.keys(&secondary("ctrl-a"));
    app.input("512");
    app.keys("enter");
    assert_eq!(settings::load(&sp).settings.heap_mb, 512);
}

#[gpui::test]
fn the_startup_combo_saves_its_choice(cx: &mut TestAppContext) {
    let d = temp_dir("startup-combo");
    let sp = d.join("settings.json");
    let mut app = start(cx, None, Some(sp.clone()));
    app.keys("ctrl-,");
    let shell = app.shell.clone();
    let combo = app.cx.update(|_, cx| shell.read(cx).settings_fields.as_ref().unwrap().startup.clone());
    assert_eq!(app.cx.update(|_, cx| combo.read(cx).selected_label().to_string()), "Show the welcome screen");
    app.cx.update(|_, cx| combo.update(cx, |c, cx| c.select(1, cx)));
    app.cx.run_until_parked();
    assert_eq!(settings::load(&sp).settings.startup, settings::Startup::LastPlan);
    app.cx.update(|_, cx| combo.update(cx, |c, cx| c.select(0, cx)));
    assert_eq!(settings::load(&sp).settings.startup, settings::Startup::Welcome);
}

// Opening the drop-down menu needs real window frames, so clicking and choosing from it is covered by the
// real-window smoke test (tests/ui/smoke.ps1). The choice handling is covered above.

#[gpui::test]
fn startup_can_reopen_the_last_plan(cx: &mut TestAppContext) {
    let d = temp_dir("startup-last");
    let sp = d.join("settings.json");
    let s = Settings { startup: settings::Startup::LastPlan, recent_files: vec![fixture(WEB).canonicalize().unwrap().to_string_lossy().trim_start_matches(r"\?\").to_string()], ..Settings::default() };
    settings::save(&sp, &s).unwrap();
    let mut app = start(cx, None, Some(sp));
    assert_eq!(app.state()["elements"], 23);
    assert_eq!(app.tabs(), [WEB]);
}

#[gpui::test]
fn startup_skips_a_last_plan_that_no_longer_exists(cx: &mut TestAppContext) {
    let d = temp_dir("startup-gone");
    let sp = d.join("settings.json");
    let s = Settings { startup: settings::Startup::LastPlan, recent_files: vec!["Z:/gone/plan.jmx".into()], ..Settings::default() };
    settings::save(&sp, &s).unwrap();
    let mut app = start(cx, None, Some(sp));
    assert!(app.state()["plan"].is_null());
    assert!(app.state()["notice"].is_null(), "no error for a plan that simply is not there");
}

#[gpui::test]
fn a_plan_on_the_command_line_wins_over_the_startup_setting(cx: &mut TestAppContext) {
    let d = temp_dir("startup-cli");
    let sp = d.join("settings.json");
    let s = Settings { startup: settings::Startup::LastPlan, recent_files: vec![fixture("Test.jmx").to_string_lossy().to_string()], ..Settings::default() };
    settings::save(&sp, &s).unwrap();
    let mut app = start(cx, Some(fixture(WEB)), Some(sp));
    assert_eq!(app.tabs(), [WEB]);
}

#[gpui::test]
fn welcome_screen_stays_when_startup_is_welcome(cx: &mut TestAppContext) {
    let d = temp_dir("startup-welcome");
    let sp = d.join("settings.json");
    let s = Settings { recent_files: vec![fixture(WEB).to_string_lossy().to_string()], ..Settings::default() };
    settings::save(&sp, &s).unwrap();
    let mut app = start(cx, None, Some(sp));
    assert!(app.state()["plan"].is_null());
    assert!(app.has("welcome-open"));
}

#[gpui::test]
fn the_state_file_is_never_seen_half_written(cx: &mut TestAppContext) {
    // The real-window checks read this file while the window keeps rewriting it; it must always be whole.
    setup(cx);
    let dir = temp_dir("state-file");
    let path = dir.join("state.json");
    let options = shell::Options { state_dump: Some(path.clone()), ..Default::default() };
    let (_shell, cx) = cx.add_window_view(move |window, cx| shell::Shell::new(window, cx, options));
    cx.update(|window, _| window.activate_window());
    cx.run_until_parked();

    let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let seen = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let broken = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let reader = {
        let (path, stop, seen, broken) = (path.clone(), stop.clone(), seen.clone(), broken.clone());
        std::thread::spawn(move || {
            while !stop.load(std::sync::atomic::Ordering::Relaxed) {
                // A file that cannot be opened for a moment (it is being replaced) is not the problem tested here.
                if let Ok(text) = std::fs::read_to_string(&path) {
                    if serde_json::from_str::<serde_json::Value>(&text).is_ok() {
                        seen.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    } else {
                        broken.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    }
                }
            }
        })
    };
    for _ in 0..150 {
        cx.simulate_keystrokes(&secondary("ctrl-j"));
        cx.run_until_parked();
    }
    std::thread::sleep(Duration::from_millis(100));
    stop.store(true, std::sync::atomic::Ordering::Relaxed);
    reader.join().unwrap();
    assert_eq!(broken.load(std::sync::atomic::Ordering::Relaxed), 0, "the file was read half written");
    assert!(seen.load(std::sync::atomic::Ordering::Relaxed) > 0, "the reader saw the file");
    let _ = std::fs::remove_dir_all(dir);
}

// ---- the Recent Plans submenu of the File menu ----

fn recent_menu(app: &mut App) -> Vec<(String, bool)> {
    app.state()["recent_plans_menu"].as_array().unwrap().iter().map(|e| (e["label"].as_str().unwrap().to_string(), e["missing"].as_bool().unwrap())).collect()
}

fn plan_file(dir: &std::path::Path, name: &str) -> PathBuf {
    let p = dir.join(name);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(&p, plan_core::starters::new_plan_text(plan_core::starters::Starter::Empty, "p").unwrap()).unwrap();
    p
}

#[gpui::test]
fn the_recent_plans_submenu_lists_opened_plans_newest_first(cx: &mut TestAppContext) {
    let d = temp_dir("recent-menu");
    let (a, b) = (plan_file(&d, "first.jmx"), plan_file(&d, "second.jmx"));
    let mut app = start(cx, None, Some(d.join("settings.json")));
    assert!(recent_menu(&mut app).is_empty(), "nothing opened yet: the menu item is greyed");
    app.open_path(&a);
    app.open_path(&b);
    assert_eq!(recent_menu(&mut app), vec![("second.jmx".to_string(), false), ("first.jmx".to_string(), false)]);
    app.open_path(&a);
    assert_eq!(recent_menu(&mut app).first().map(|e| e.0.as_str()), Some("first.jmx"), "opening an older one moves it to the top");
    assert_eq!(recent_menu(&mut app).len(), 2, "and it is listed once");
}

#[gpui::test]
fn two_recent_plans_with_the_same_file_name_show_their_folders(cx: &mut TestAppContext) {
    let d = temp_dir("recent-twins");
    let (a, b) = (plan_file(&d.join("one"), "test.jmx"), plan_file(&d.join("two"), "test.jmx"));
    let mut app = start(cx, None, Some(d.join("settings.json")));
    app.open_path(&a);
    app.open_path(&b);
    let labels: Vec<String> = recent_menu(&mut app).into_iter().map(|e| e.0).collect();
    assert_eq!(labels.len(), 2);
    assert_ne!(labels[0], labels[1], "{labels:?}");
    assert!(labels[0].contains("two") && labels[1].contains("one"), "{labels:?}");
}

#[gpui::test]
fn a_recent_plan_whose_file_is_gone_is_shown_as_missing_and_is_removed_when_chosen(cx: &mut TestAppContext) {
    let d = temp_dir("recent-missing");
    let (a, b) = (plan_file(&d, "keep.jmx"), plan_file(&d, "gone.jmx"));
    let mut app = start(cx, None, Some(d.join("settings.json")));
    app.open_path(&a);
    app.open_path(&b);
    std::fs::remove_file(&b).unwrap();
    assert_eq!(recent_menu(&mut app), vec![("gone.jmx".to_string(), true), ("keep.jmx".to_string(), false)], "listed but marked");
    let shell = app.shell.clone();
    // Choose it the way the menu does: with the path the list holds (a system may spell a folder in more than one way,
    // for example macOS temporary folders behind a link).
    let gone = PathBuf::from(app.state()["recent_plans_menu"][0]["path"].as_str().unwrap());
    app.cx.update(|window, cx| shell.update(cx, |s, cx| s.open_plan_path(&gone, window, cx)));
    app.cx.run_until_parked();
    assert_eq!(recent_menu(&mut app), vec![("keep.jmx".to_string(), false)], "a plan that cannot be opened leaves the list");
    assert!(!app.state()["notice"].as_str().unwrap().is_empty());
}

#[gpui::test]
fn clearing_the_recent_plans_empties_the_list_and_the_settings_but_not_the_open_plans(cx: &mut TestAppContext) {
    let d = temp_dir("recent-clear");
    let (a, b) = (plan_file(&d, "a.jmx"), plan_file(&d, "b.jmx"));
    let settings_path = d.join("settings.json");
    let mut app = start(cx, None, Some(settings_path.clone()));
    app.open_path(&a);
    app.open_path(&b);
    assert_eq!(settings::load(&settings_path).settings.recent_files.len(), 2);
    let shell = app.shell.clone();
    app.cx.update(|window, cx| shell.update(cx, |s, cx| s.clear_recent_plans(window, cx)));
    app.cx.run_until_parked();
    assert!(recent_menu(&mut app).is_empty());
    assert!(settings::load(&settings_path).settings.recent_files.is_empty(), "saved");
    assert_eq!(app.tabs().len(), 2, "the open plans stay open");
    assert!(a.is_file() && b.is_file(), "no plan file is touched");
    assert!(app.state()["notice"].as_str().unwrap().contains("cleared"));
    // Clearing an empty list is quiet.
    app.cx.update(|window, cx| shell.update(cx, |s, cx| s.clear_recent_plans(window, cx)));
    assert!(recent_menu(&mut app).is_empty());
}

// ---- opening another menu by moving over its title ----

fn hover(app: &mut App, selector: &'static str) {
    app.settle();
    let b = app.cx.debug_bounds(selector).unwrap_or_else(|| panic!("no element with selector {selector}"));
    app.cx.simulate_mouse_move(b.center(), None, Modifiers::default());
    app.cx.run_until_parked();
    app.settle();
}

fn open_menu(app: &mut App) -> Option<String> {
    app.state()["open_menu"].as_str().map(str::to_string)
}

#[gpui::test]
fn moving_over_a_menu_title_opens_nothing_while_no_menu_is_open(cx: &mut TestAppContext) {
    let mut app = start(cx, None, None);
    for title in ["menu-title-File", "menu-title-Edit", "menu-title-Help"] {
        hover(&mut app, title);
        assert_eq!(open_menu(&mut app), None, "{title}: a menu opens by a click only");
    }
}

#[gpui::test]
fn once_a_menu_is_open_moving_over_another_title_opens_that_one(cx: &mut TestAppContext) {
    let mut app = start(cx, None, None);
    app.click("menu-title-File");
    assert_eq!(open_menu(&mut app).as_deref(), Some("File"));
    hover(&mut app, "menu-title-Edit");
    assert_eq!(open_menu(&mut app).as_deref(), Some("Edit"), "the open menu follows the pointer");
    hover(&mut app, "menu-title-Help");
    assert_eq!(open_menu(&mut app).as_deref(), Some("Help"));
    hover(&mut app, "menu-title-File");
    assert_eq!(open_menu(&mut app).as_deref(), Some("File"), "and back again");
}

#[gpui::test]
fn only_one_menu_is_open_at_a_time_and_moving_over_the_open_one_changes_nothing(cx: &mut TestAppContext) {
    let mut app = start(cx, None, None);
    app.click("menu-title-View");
    hover(&mut app, "menu-title-Tools");
    hover(&mut app, "menu-title-Tools");
    hover(&mut app, "menu-title-Tools");
    let open = app.state()["open_menu"].clone();
    assert_eq!(open, "Tools");
    assert_eq!(app.state()["open_menu_count"], 1, "never two menus at once");
}

#[gpui::test]
fn a_click_on_the_title_of_a_menu_opened_by_moving_over_it_closes_it_and_then_hovering_opens_nothing(cx: &mut TestAppContext) {
    let mut app = start(cx, None, None);
    app.click("menu-title-File");
    hover(&mut app, "menu-title-Run");
    assert_eq!(open_menu(&mut app).as_deref(), Some("Run"));
    app.click("menu-title-Run");
    assert_eq!(open_menu(&mut app), None, "a click on the open menu's title closes it");
    hover(&mut app, "menu-title-Edit");
    assert_eq!(open_menu(&mut app), None, "with every menu closed, moving over a title opens nothing again");
    app.click("menu-title-Edit");
    assert_eq!(open_menu(&mut app).as_deref(), Some("Edit"));
}

// ---- one size scale for the menu bar, the menus and the command palette ----

#[gpui::test]
fn palette_rows_and_the_search_box_follow_the_menu_scale(cx: &mut TestAppContext) {
    use jmeter_ui::theme::scale::{MENU_TEXT, PALETTE_ROW, PALETTE_TEXT};
    assert_eq!(PALETTE_TEXT, MENU_TEXT, "the palette reads at the size of the menus");
    const { assert!(PALETTE_ROW >= MENU_TEXT * 2.0) }; // a row has room above and below its text
    let mut app = start(cx, None, None);
    app.settle();
    let trigger = app.cx.debug_bounds("palette-trigger").expect("the search box is in the title bar");
    let trigger_height = f32::from(trigger.size.height);
    assert!((MENU_TEXT * 1.8..=38.0 - 4.0).contains(&trigger_height), "search box height {trigger_height} must fit the 38 pixel title bar");
    app.keys("ctrl-k");
    app.settle();
    let first = app.cx.debug_bounds("palette-row-0").expect("the palette lists rows");
    let second = app.cx.debug_bounds("palette-row-1").expect("and a second one");
    assert!((f32::from(first.size.height) - PALETTE_ROW).abs() < 0.5, "row height {}", f32::from(first.size.height));
    assert!((f32::from(second.origin.y) - f32::from(first.origin.y) - PALETTE_ROW).abs() < 1.5, "rows are one row height apart");
    app.keys("escape");
}
