//! The forms in the middle area: editing the selected element's name, comments and settings.

mod common;

use common::*;
use gpui::TestAppContext;
use std::path::PathBuf;

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

fn dirty(app: &mut App) -> bool {
    app.state()["dirty"].as_bool().unwrap()
}

/// A plan with a thread group holding an HTTP request, for the form tests.
fn with_request<'a>(cx: &'a mut TestAppContext, name: &str) -> App<'a> {
    let mut app = start(cx, Some(copy_of(name, WEB)), None);
    goto(&mut app, "jmeter users");
    app.keys("ctrl-k");
    app.input("+http request");
    app.keys("enter");
    assert_eq!(app.selected().as_deref(), Some("HTTP Request"));
    app
}

// ---- text fields ----

#[gpui::test]
fn a_typed_value_is_saved_with_enter_and_undone_with_undo(cx: &mut TestAppContext) {
    let mut app = with_request(cx, "form-path");
    assert_eq!(app.prop("HTTPSampler.path"), None, "a new request has no path");
    app.type_into("field-HTTPSampler.path", "/bugzilla/index.cgi");
    assert_eq!(app.prop("HTTPSampler.path").as_deref(), Some("/bugzilla/index.cgi"));
    assert!(app.plan_text().contains("<stringProp name=\"HTTPSampler.path\">/bugzilla/index.cgi</stringProp>"));
    app.click("tree-row-0"); // the tree has the focus, so Ctrl+Z is the plan's undo
    goto(&mut app, "http request");
    app.keys("ctrl-z");
    assert_eq!(app.prop("HTTPSampler.path"), None, "undo takes the setting away again");
}

#[gpui::test]
fn leaving_a_box_saves_what_was_typed(cx: &mut TestAppContext) {
    let mut app = with_request(cx, "form-blur");
    app.click("field-HTTPSampler.domain");
    app.input("example.org");
    // No Enter: pick another element.
    goto(&mut app, "jmeter users");
    goto(&mut app, "http request");
    assert_eq!(app.prop("HTTPSampler.domain").as_deref(), Some("example.org"), "saved on the element it was typed for");
}

#[gpui::test]
fn tab_moves_to_the_next_box_and_saves_the_one_left(cx: &mut TestAppContext) {
    let mut app = with_request(cx, "form-tab");
    app.click("field-name");
    app.keys("tab");
    app.input("a comment");
    app.keys("tab");
    assert_eq!(app.prop("TestPlan.comments").as_deref(), Some("a comment"));
}

#[gpui::test]
fn escape_puts_the_saved_value_back(cx: &mut TestAppContext) {
    let mut app = with_request(cx, "form-escape");
    app.type_into("field-HTTPSampler.path", "/one");
    app.click("field-HTTPSampler.path");
    app.keys(&secondary("ctrl-a"));
    app.input("/two");
    app.keys("escape");
    assert_eq!(app.prop("HTTPSampler.path").as_deref(), Some("/one"));
    goto(&mut app, "jmeter users");
    goto(&mut app, "http request");
    assert_eq!(app.prop("HTTPSampler.path").as_deref(), Some("/one"));
}

#[gpui::test]
fn the_name_box_renames_the_element_in_the_tree(cx: &mut TestAppContext) {
    let mut app = with_request(cx, "form-rename");
    app.type_into("field-name", "Landing request");
    assert_eq!(app.selected().as_deref(), Some("Landing request"));
    assert_eq!(app.editing().as_deref(), Some("Landing request"));
    assert!(dirty(&mut app));
    goto(&mut app, "jmeter users");
    goto(&mut app, "landing request");
    assert_eq!(app.selected().as_deref(), Some("Landing request"), "go to element finds it by the new name");
    app.keys("ctrl-z");
    assert_eq!(app.selected().as_deref(), Some("HTTP Request"));
}

#[gpui::test]
fn f2_puts_the_cursor_in_the_name_box(cx: &mut TestAppContext) {
    let mut app = with_request(cx, "form-f2");
    app.click("tree-row-0");
    goto(&mut app, "http request");
    app.keys("f2");
    app.input("Renamed");
    app.keys("enter");
    assert_eq!(app.selected().as_deref(), Some("Renamed"), "F2 selected the old name, so typing replaced it");
}

#[gpui::test]
fn the_comment_box_edits_the_comments(cx: &mut TestAppContext) {
    let mut app = with_request(cx, "form-comment");
    app.type_into("field-comments", "Checks the home page");
    assert_eq!(app.prop("TestPlan.comments").as_deref(), Some("Checks the home page"));
    app.click("tree-row-0");
    goto(&mut app, "http request");
    // The comment shows again when the element is selected.
    let shell = app.shell.clone();
    let text = app.cx.update(|_, cx| shell.read(cx).form.as_ref().unwrap().comment.read(cx).text().to_string());
    assert_eq!(text, "Checks the home page");
}

#[gpui::test]
fn ctrl_s_saves_text_that_was_typed_but_not_confirmed(cx: &mut TestAppContext) {
    let p = copy_of("form-save", WEB);
    let mut app = start(cx, Some(p.clone()), None);
    goto(&mut app, "jmeter users");
    app.keys("ctrl-k");
    app.input("+http request");
    app.keys("enter");
    app.click("field-HTTPSampler.path");
    app.input("/unsaved");
    app.keys("ctrl-s");
    let saved = std::fs::read_to_string(&p).unwrap();
    assert!(saved.contains("/unsaved"), "the pending text went into the file");
}

// ---- numbers ----

#[gpui::test]
fn number_fields_accept_numbers_and_variables_but_not_letters(cx: &mut TestAppContext) {
    let mut app = with_request(cx, "form-number");
    app.click("field-HTTPSampler.port");
    app.input("80a");
    app.keys("enter");
    assert_eq!(app.prop("HTTPSampler.port").as_deref(), Some("80"), "the letter was refused as it was typed");
    app.type_into("field-HTTPSampler.port", "${port}");
    assert_eq!(app.prop("HTTPSampler.port").as_deref(), Some("${port}"));
}

#[gpui::test]
fn a_variable_in_a_number_setting_turns_it_into_a_string_setting(cx: &mut TestAppContext) {
    let mut app = start(cx, Some(copy_of("form-int", WEB)), None);
    app.keys("ctrl-k");
    app.input("+thread group");
    app.keys("enter");
    assert!(app.plan_text().contains("<intProp name=\"ThreadGroup.num_threads\">1</intProp>"));
    app.type_into("field-ThreadGroup.num_threads", "20");
    assert!(app.plan_text().contains("<intProp name=\"ThreadGroup.num_threads\">20</intProp>"), "a number stays a number");
    app.type_into("field-ThreadGroup.num_threads", "${users}");
    assert!(app.plan_text().contains("<stringProp name=\"ThreadGroup.num_threads\">${users}</stringProp>"));
    app.click("tree-row-0");
    goto(&mut app, "thread group");
    app.keys("ctrl-z");
    assert!(app.plan_text().contains("<intProp name=\"ThreadGroup.num_threads\">20</intProp>"), "undo brings the number setting back");
}

#[gpui::test]
fn the_loop_count_lives_inside_the_main_controller_setting(cx: &mut TestAppContext) {
    let mut app = start(cx, Some(copy_of("form-loops", WEB)), None);
    app.keys("ctrl-k");
    app.input("+thread group");
    app.keys("enter");
    assert_eq!(app.prop("ThreadGroup.main_controller/LoopController.loops").as_deref(), Some("1"));
    app.type_into("field-ThreadGroup.main_controller/LoopController.loops", "-1");
    assert_eq!(app.prop("ThreadGroup.main_controller/LoopController.loops").as_deref(), Some("-1"));
    assert!(app.plan_text().contains("<stringProp name=\"LoopController.loops\">-1</stringProp>"));
}

// ---- checkboxes and drop-downs ----

#[gpui::test]
fn checkboxes_toggle_a_setting_and_undo_reverts(cx: &mut TestAppContext) {
    let mut app = with_request(cx, "form-check");
    assert_eq!(app.prop("HTTPSampler.use_keepalive").as_deref(), Some("true"));
    app.click("check-HTTPSampler.use_keepalive");
    assert_eq!(app.prop("HTTPSampler.use_keepalive").as_deref(), Some("false"));
    app.click("check-HTTPSampler.DO_MULTIPART_POST");
    assert_eq!(app.prop("HTTPSampler.DO_MULTIPART_POST").as_deref(), Some("true"), "a setting that was not there is added");
    app.click("tree-row-0");
    goto(&mut app, "http request");
    app.keys("ctrl-z ctrl-z");
    assert_eq!(app.prop("HTTPSampler.use_keepalive").as_deref(), Some("true"));
    assert_eq!(app.prop("HTTPSampler.DO_MULTIPART_POST"), None);
}

#[gpui::test]
fn a_drop_down_saves_the_chosen_value(cx: &mut TestAppContext) {
    let mut app = with_request(cx, "form-combo");
    app.choose("HTTPSampler.method", 1);
    assert_eq!(app.prop("HTTPSampler.method").as_deref(), Some("POST"));
    app.choose("HTTPSampler.method", 4);
    assert_eq!(app.prop("HTTPSampler.method").as_deref(), Some("PATCH"));
    let c = app.combo("HTTPSampler.method");
    assert_eq!(app.cx.update(|_, cx| c.read(cx).selected_label().to_string()), "PATCH");
}

#[gpui::test]
fn undo_updates_the_drop_down(cx: &mut TestAppContext) {
    let mut app = with_request(cx, "form-combo-undo");
    app.choose("HTTPSampler.method", 2);
    app.click("tree-row-0");
    goto(&mut app, "http request");
    app.keys("ctrl-z");
    let c = app.combo("HTTPSampler.method");
    assert_eq!(app.cx.update(|_, cx| c.read(cx).selected_label().to_string()), "GET", "the box shows the setting again");
}

#[gpui::test]
fn a_value_jmeter_does_not_list_is_kept_as_a_custom_choice(cx: &mut TestAppContext) {
    let mut app = with_request(cx, "form-custom");
    app.type_into("field-name", "x");
    let shell = app.shell.clone();
    app.cx.update(|window, cx| {
        shell.update(cx, |s, cx| {
            let id = s.plan().unwrap().tree.selected().unwrap();
            s.set_element_prop(id, "HTTPSampler.method", "stringProp", "PROPFIND", window, cx);
        })
    });
    app.cx.run_until_parked();
    let c = app.combo("HTTPSampler.method");
    assert_eq!(app.cx.update(|_, cx| c.read(cx).selected_label().to_string()), "PROPFIND (custom)");
}

#[gpui::test]
fn a_setting_made_of_several_flags_is_edited_flag_by_flag(cx: &mut TestAppContext) {
    let mut app = start(cx, Some(copy_of("form-bits", WEB)), None);
    goto(&mut app, "bugdetail");
    app.keys("ctrl-k");
    app.input("+response assertion");
    app.keys("enter");
    assert_eq!(app.prop("Assertion.test_type").as_deref(), Some("16"), "substring by default");
    // Rule: Contains, Matches, Equals, Substring.
    app.choose("Assertion.test_type", 2);
    assert_eq!(app.prop("Assertion.test_type").as_deref(), Some("8"), "equals");
    app.click("check-Assertion.test_type#4");
    assert_eq!(app.prop("Assertion.test_type").as_deref(), Some("12"), "equals, and not");
    app.click("check-Assertion.test_type#32");
    assert_eq!(app.prop("Assertion.test_type").as_deref(), Some("44"), "and or");
    app.choose("Assertion.test_type", 0);
    assert_eq!(app.prop("Assertion.test_type").as_deref(), Some("38"), "changing the rule keeps the other flags");
    app.click("check-Assertion.test_type#4");
    assert_eq!(app.prop("Assertion.test_type").as_deref(), Some("34"));
}

// ---- tables ----

const PARAMS: &str = "HTTPsampler.Arguments/Arguments.arguments";

#[gpui::test]
fn parameters_are_added_edited_and_removed_in_a_table(cx: &mut TestAppContext) {
    let mut app = with_request(cx, "form-table");
    let rows_before = app.plan_text().matches("elementType=\"HTTPArgument\"").count();
    assert!(app.has("add-row-HTTPsampler.Arguments/Arguments.arguments"));
    app.click("add-row-HTTPsampler.Arguments/Arguments.arguments");
    assert!(app.has("cell-HTTPsampler.Arguments/Arguments.arguments-0-0"), "the new row is drawn");
    app.input("id");
    app.keys("tab");
    app.input("${itemId}");
    app.keys("enter");
    let text = app.plan_text();
    assert!(text.contains("<stringProp name=\"Argument.name\">id</stringProp>"), "{text}");
    assert!(text.contains("<stringProp name=\"Argument.value\">${itemId}</stringProp>"), "{text}");
    assert!(text.contains("elementType=\"HTTPArgument\""));
    // A second row.
    app.click("add-row-HTTPsampler.Arguments/Arguments.arguments");
    app.input("page");
    app.keys("tab");
    app.input("2");
    app.keys("enter");
    assert_eq!(app.plan_text().matches("elementType=\"HTTPArgument\"").count(), rows_before + 2);
    // Remove the first.
    let shell = app.shell.clone();
    app.cx.update(|window, cx| shell.update(cx, |s, cx| s.remove_table_row(PARAMS, 0, window, cx)));
    app.cx.run_until_parked();
    let text = app.plan_text();
    assert_eq!(text.matches("elementType=\"HTTPArgument\"").count(), rows_before + 1);
    assert!(!text.contains("<stringProp name=\"Argument.name\">id</stringProp>") && text.contains(">page</stringProp>"));
    // The checkboxes in a row switch the encoding flags.
    assert!(app.has("cellcheck-HTTPsampler.Arguments/Arguments.arguments-0-HTTPArgument.use_equals"));
    app.click("cellcheck-HTTPsampler.Arguments/Arguments.arguments-0-HTTPArgument.use_equals");
    assert!(app.plan_text().contains("<boolProp name=\"HTTPArgument.use_equals\">false</boolProp>"));
}

#[gpui::test]
fn table_changes_undo_in_order(cx: &mut TestAppContext) {
    let mut app = with_request(cx, "form-table-undo");
    let before = app.plan_text();
    app.click("add-row-HTTPsampler.Arguments/Arguments.arguments");
    app.input("a");
    app.keys("enter");
    app.click("tree-row-0");
    goto(&mut app, "http request");
    app.keys("ctrl-z");
    app.keys("ctrl-z");
    assert_eq!(app.plan_text(), before, "the value, then the row, are undone");
}

#[gpui::test]
fn user_defined_variables_have_their_own_table(cx: &mut TestAppContext) {
    let mut app = start(cx, Some(copy_of("form-udv", WEB)), None);
    goto(&mut app, "jmeter users");
    app.keys("ctrl-k");
    app.input("+user defined variables");
    app.keys("enter");
    assert_eq!(app.selected().as_deref(), Some("User Defined Variables"));
    app.click("add-row-Arguments.arguments");
    app.input("baseUrl");
    app.keys("tab");
    app.input("https://example.org");
    app.keys("enter");
    let text = app.plan_text();
    assert!(text.contains("<stringProp name=\"Argument.name\">baseUrl</stringProp>"), "{text}");
    assert!(text.contains("elementType=\"Argument\""));
}

#[gpui::test]
fn assertion_patterns_are_a_list_of_strings(cx: &mut TestAppContext) {
    let mut app = start(cx, Some(copy_of("form-patterns", WEB)), None);
    goto(&mut app, "bugdetail");
    app.keys("ctrl-k");
    app.input("+response assertion");
    app.keys("enter");
    app.click("add-row-Asserion.test_strings");
    app.input("Welcome");
    app.keys("enter");
    assert!(app.plan_text().contains(">Welcome</stringProp>"));
    app.click("cell-Asserion.test_strings-0-0");
    app.keys(&secondary("ctrl-a"));
    app.input("Hello");
    app.keys("enter");
    let text = app.plan_text();
    assert!(text.contains(">Hello</stringProp>") && !text.contains(">Welcome</stringProp>"), "{text}");
}

// ---- other elements ----

#[gpui::test]
fn elements_without_a_hand_made_form_get_one_from_their_settings(cx: &mut TestAppContext) {
    let mut app = start(cx, Some(copy_of("form-generic", WEB)), None);
    goto(&mut app, "jmeter users");
    app.keys("ctrl-k");
    app.input("+counter");
    app.keys("enter");
    assert_eq!(app.selected().as_deref(), Some("Counter"));
    let shell = app.shell.clone();
    let n = app.cx.update(|_, cx| shell.read(cx).form.as_ref().unwrap().sections.iter().map(|s| s.items.len()).sum::<usize>());
    assert!(n >= 3, "a field for each saved setting, got {n}");
}

#[gpui::test]
fn a_script_is_edited_in_a_text_area(cx: &mut TestAppContext) {
    let mut app = start(cx, Some(copy_of("form-script", WEB)), None);
    goto(&mut app, "jmeter users");
    app.keys("ctrl-k");
    app.input("+jsr223 sampler");
    app.keys("enter");
    let shell = app.shell.clone();
    let multiline = app.cx.update(|_, cx| {
        let f = shell.read(cx).form.as_ref().unwrap();
        f.sections.iter().flat_map(|s| s.items.iter()).any(|i| matches!(&i.widget, jmeter_ui::form::Widget::Area(_)) && i.def.prop == "script")
    });
    assert!(multiline, "the script is edited in a text area");
}

#[gpui::test]
fn the_test_plan_has_a_variables_table_and_flags(cx: &mut TestAppContext) {
    let mut app = start(cx, Some(copy_of("form-testplan", WEB)), None);
    app.click("tree-row-0");
    assert!(app.has("check-TestPlan.functional_mode"));
    assert!(app.has("add-row-TestPlan.user_defined_variables/Arguments.arguments"));
    app.click("check-TestPlan.functional_mode");
    assert_eq!(app.prop("TestPlan.functional_mode").as_deref(), Some("true"));
}

#[gpui::test]
fn enabled_button_switches_the_element_off(cx: &mut TestAppContext) {
    let mut app = with_request(cx, "form-enabled");
    app.click("toggle-enabled");
    assert!(app.plan_text().contains("testname=\"HTTP Request\" enabled=\"false\""));
    app.click("toggle-enabled");
    assert!(app.plan_text().contains("testname=\"HTTP Request\" enabled=\"true\""));
}

#[gpui::test]
fn the_form_follows_the_selection(cx: &mut TestAppContext) {
    let mut app = with_request(cx, "form-follow");
    assert!(app.has("field-HTTPSampler.path"));
    goto(&mut app, "jmeter users");
    assert!(!app.has("field-HTTPSampler.path"), "a thread group has no path");
    assert!(app.has("field-ThreadGroup.num_threads"));
}

// ---- variable hints ----

#[gpui::test]
fn a_variable_nothing_defines_shows_up_in_the_problems_list(cx: &mut TestAppContext) {
    let mut app = with_request(cx, "form-hints");
    app.type_into("field-HTTPSampler.path", "/item/${missingItem}");
    let problems: Vec<String> = app.state()["problems"].as_array().unwrap().iter().map(|p| p.as_str().unwrap().to_string()).collect();
    assert!(problems.iter().any(|p| p.contains("${missingItem}") && p.starts_with("HTTP Request")), "{problems:?}");
    if !app.state()["bottom"].as_bool().unwrap() {
        app.keys("ctrl-j");
    }
    assert!(app.has("problem-0"));
    // Defining it takes the hint away.
    goto(&mut app, "jmeter users");
    app.keys("ctrl-k");
    app.input("+regular expression extractor");
    app.keys("enter");
    app.type_into("field-RegexExtractor.refname", "missingItem");
    let problems: Vec<String> = app.state()["problems"].as_array().unwrap().iter().map(|p| p.as_str().unwrap().to_string()).collect();
    assert!(!problems.iter().any(|p| p.contains("${missingItem}")), "{problems:?}");
}

#[gpui::test]
fn the_variable_hints_stay_up_to_date_with_undo(cx: &mut TestAppContext) {
    let mut app = with_request(cx, "form-hints-undo");
    app.type_into("field-HTTPSampler.path", "/${ghost}");
    let has = |app: &mut App| app.state()["problems"].as_array().unwrap().iter().any(|p| p.as_str().unwrap().contains("${ghost}"));
    assert!(has(&mut app));
    app.click("tree-row-0");
    goto(&mut app, "http request");
    app.keys("ctrl-z");
    assert!(!has(&mut app));
}

// ---- the keyboard stays where the user is typing ----

#[gpui::test]
fn the_box_keeps_the_focus_after_enter_even_when_the_form_changes_shape(cx: &mut TestAppContext) {
    let mut app = with_request(cx, "form-focus");
    // Saving the first value adds a setting the element did not have; the box is still the one typed in.
    app.click("field-HTTPSampler.path");
    app.input("/a");
    app.keys("enter");
    app.input("b");
    app.keys("enter");
    assert_eq!(app.prop("HTTPSampler.path").as_deref(), Some("/ab"), "the second letter went into the same box");
}

#[gpui::test]
fn shortcuts_work_right_after_editing_a_box(cx: &mut TestAppContext) {
    let mut app = with_request(cx, "form-shortcut");
    let bottom = app.state()["bottom"].as_bool().unwrap();
    app.type_into("field-HTTPSampler.path", "/x");
    app.keys("ctrl-j");
    assert_ne!(app.state()["bottom"].as_bool().unwrap(), bottom, "Ctrl+J still reaches the window");
    app.keys("ctrl-alt-b");
    assert!(app.state()["inspector"].as_bool().unwrap());
}

#[gpui::test]
fn undo_from_a_box_keeps_the_window_usable(cx: &mut TestAppContext) {
    let mut app = with_request(cx, "form-undo-focus");
    app.type_into("field-HTTPSampler.path", "/x");
    app.click("tree-row-0");
    goto(&mut app, "http request");
    app.keys("ctrl-z");
    assert_eq!(app.prop("HTTPSampler.path"), None);
    app.click("field-HTTPSampler.path");
    app.input("/again");
    app.keys("enter");
    assert_eq!(app.prop("HTTPSampler.path").as_deref(), Some("/again"));
}

// ---- the unsaved mark appears while typing ----

#[gpui::test]
fn typing_marks_the_plan_unsaved_at_once_and_deleting_it_again_clears_the_mark(cx: &mut TestAppContext) {
    let p = copy_of("form-typing-mark", WEB);
    let mut app = start(cx, Some(p.clone()), None);
    goto(&mut app, "bugdetail");
    assert!(!dirty(&mut app));
    app.click("field-comments");
    app.keys("end");
    app.input("Z");
    assert!(app.state()["tabs"][0]["dirty"].as_bool().unwrap() && dirty(&mut app), "no Enter, no focus change, and the tab already shows it");
    app.keys("backspace");
    assert!(!app.state()["tabs"][0]["dirty"].as_bool().unwrap(), "back to the saved text, so no mark");
    app.input("Y");
    app.keys("ctrl-s");
    assert!(!dirty(&mut app));
    assert!(std::fs::read_to_string(&p).unwrap().contains("bugId variable extracted in Changes by randomBugIdExtractorY"));
}

#[gpui::test]
fn closing_a_tab_asks_about_text_that_was_only_typed(cx: &mut TestAppContext) {
    let mut app = start(cx, Some(copy_of("form-typing-close", WEB)), None);
    goto(&mut app, "bugdetail");
    app.click("field-comments");
    app.input("note");
    app.keys("ctrl-w");
    assert_eq!(app.state()["dialog"]["kind"], "unsaved");
}

// ---- typing is saved to the plan and the recovery copy after a pause ----

fn pause(app: &mut App, secs: f32) {
    app.cx.executor().advance_clock(std::time::Duration::from_secs_f32(secs));
    app.cx.run_until_parked();
}

#[gpui::test]
fn typing_in_a_table_cell_marks_the_plan_at_once(cx: &mut TestAppContext) {
    let mut app = with_request(cx, "form-cell-mark");
    app.click("add-row-HTTPsampler.Arguments/Arguments.arguments");
    app.keys("enter");
    let clean = app.state()["dirty"].as_bool().unwrap();
    assert!(clean, "adding the row already changed the plan");
    app.input("na");
    assert!(app.state()["tabs"][0]["dirty"].as_bool().unwrap());
}

#[gpui::test]
fn text_typed_in_a_box_reaches_the_plan_and_the_recovery_copy_after_a_pause(cx: &mut TestAppContext) {
    let p = copy_of("form-live", WEB);
    let mut app = start(cx, Some(p.clone()), None);
    goto(&mut app, "jmeter users");
    app.keys("ctrl-k");
    app.input("+http request");
    app.keys("enter");
    app.click("field-HTTPSampler.path");
    app.input("/live");
    assert_eq!(app.prop("HTTPSampler.path"), None, "not at once");
    pause(&mut app, 1.0);
    assert_eq!(app.prop("HTTPSampler.path").as_deref(), Some("/live"), "saved to the plan without Enter");
    pause(&mut app, 4.0);
    let bak = std::fs::read_to_string(plan_core::files::recovery_path(&p)).unwrap();
    assert!(bak.contains("/live"), "and into the .bak file");
    app.input("er");
    pause(&mut app, 1.0);
    assert_eq!(app.prop("HTTPSampler.path").as_deref(), Some("/liveer"));
    app.click("tree-row-0");
    goto(&mut app, "http request");
    app.keys("ctrl-z");
    assert_eq!(app.prop("HTTPSampler.path").as_deref(), Some("/live"), "one pause is one undo step");
}

#[gpui::test]
fn text_typed_in_a_table_cell_reaches_the_recovery_copy(cx: &mut TestAppContext) {
    let p = copy_of("form-live-cell", WEB);
    let mut app = start(cx, Some(p.clone()), None);
    goto(&mut app, "jmeter users");
    app.keys("ctrl-k");
    app.input("+http request");
    app.keys("enter");
    app.click("add-row-HTTPsampler.Arguments/Arguments.arguments");
    app.input("token");
    pause(&mut app, 5.0);
    assert!(app.plan_text().contains("<stringProp name=\"Argument.name\">token</stringProp>"));
    let bak = std::fs::read_to_string(plan_core::files::recovery_path(&p)).unwrap();
    assert!(bak.contains("<stringProp name=\"Argument.name\">token</stringProp>"), "the cell text is in the .bak file");
}

#[gpui::test]
fn half_a_number_does_not_raise_an_error_while_typing(cx: &mut TestAppContext) {
    let mut app = with_request(cx, "form-live-number");
    app.click("field-HTTPSampler.port");
    app.input("${");
    pause(&mut app, 1.0);
    let n = app.state()["notice"].as_str().unwrap_or("").to_string();
    assert!(!n.contains("Port"), "{n}");
    app.input("p}");
    pause(&mut app, 1.0);
    assert_eq!(app.prop("HTTPSampler.port").as_deref(), Some("${p}"));
}

#[gpui::test]
fn the_name_box_renames_while_typing(cx: &mut TestAppContext) {
    let mut app = with_request(cx, "form-live-name");
    app.click("field-name");
    app.keys(&secondary("ctrl-a"));
    app.input("Checkout");
    pause(&mut app, 1.0);
    assert_eq!(app.selected().as_deref(), Some("Checkout"));
}
