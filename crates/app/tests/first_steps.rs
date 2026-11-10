//! What a person sees in a brand new plan: the element count, the status bar, and the hints for adding an element.

mod common;

use common::*;
use gpui::TestAppContext;
use plan_core::starters::{self, Starter};

fn new_plan(cx: &mut TestAppContext, name: &str) -> (std::path::PathBuf, ()) {
    let _ = cx;
    let d = temp_dir(name);
    let p = d.join("fresh.jmx");
    std::fs::write(&p, starters::new_plan_text(Starter::Empty, "fresh").unwrap()).unwrap();
    (p, ())
}

fn add(app: &mut App, tag: &'static str, gui: &'static str) {
    let shell = app.shell.clone();
    app.cx.update(|window, cx| shell.update(cx, |s, cx| s.add_element(tag, gui, window, cx)));
    app.cx.run_until_parked();
}

#[gpui::test]
fn the_status_bar_says_one_element_and_then_two_elements(cx: &mut TestAppContext) {
    let (p, _) = new_plan(cx, "first-count");
    let mut app = start(cx, Some(p), None);
    assert_eq!(app.state()["status_elements"], "1 element");
    add(&mut app, "ThreadGroup", "ThreadGroupGui");
    assert_eq!(app.state()["status_elements"], "2 elements");
}

#[gpui::test]
fn a_brand_new_plan_shows_no_warning_in_the_status_bar_but_keeps_it_in_the_problems_list(cx: &mut TestAppContext) {
    let (p, _) = new_plan(cx, "first-warning");
    let mut app = start(cx, Some(p), None);
    assert_eq!(app.state()["status_warnings"], 0, "nothing to blame the person for yet");
    let shell = app.shell.clone();
    let listed = app.cx.update(|_, cx| shell.read(cx).plan().unwrap().plan_notes().iter().any(|n| n.text == plan_core::problems::NO_THREAD_GROUP));
    assert!(listed, "the Problems list still has it");

    // Another element, and still no Thread Group: now it counts.
    add(&mut app, "CounterConfig", "CounterConfigGui");
    assert_eq!(app.state()["status_warnings"], 1);

    let shell = app.shell.clone();
    app.cx.update(|window, cx| shell.update(cx, |s, cx| { let root = s.plan().unwrap().plan.roots()[0]; s.select_node(root, window, cx) }));
    app.cx.run_until_parked();
    // A Thread Group (with no request in it) has its own note, but the plan can run, so the missing-group warning is gone.
    add(&mut app, "ThreadGroup", "ThreadGroupGui");
    let shell = app.shell.clone();
    let gone = app.cx.update(|_, cx| !shell.read(cx).plan().unwrap().plan_notes().iter().any(|n| n.text == plan_core::problems::NO_THREAD_GROUP));
    assert!(gone);
}

#[gpui::test]
fn the_search_box_says_how_to_add_an_element_and_the_empty_palette_repeats_it(cx: &mut TestAppContext) {
    let mut app = start(cx, None, None);
    app.settle();
    assert_eq!(shell_core::palette::SEARCH_HINT, "Search commands and elements, or type + to add one");
    assert_eq!(shell_core::palette::Mode::Everything.placeholder(), shell_core::palette::SEARCH_HINT);
    // The hint fits in its box without being cut off.
    let boxed = app.cx.debug_bounds("palette-trigger-box").expect("the title bar has the search box");
    let text = app.cx.debug_bounds("palette-trigger-text").expect("with its hint");
    assert!(f32::from(text.size.width) <= f32::from(boxed.size.width) + 0.5, "the hint is {} wide in a box {} wide", f32::from(text.size.width), f32::from(boxed.size.width));
    // The box ends before the window controls at the smallest width the window can have.
    app.cx.simulate_resize(gpui::size(gpui::px(shell_core::settings::WINDOW_MIN.0), gpui::px(shell_core::settings::WINDOW_MIN.1)));
    app.settle();
    let trigger = app.cx.debug_bounds("palette-trigger").unwrap();
    let run = app.cx.debug_bounds("run").expect("the Run button is in the title bar");
    assert!(trigger.right() <= run.left() + gpui::px(0.5), "the search box {trigger:?} must end before the Run button {run:?}");
    assert!(f32::from(trigger.size.width) >= 200.0, "and still be wide enough to use: {trigger:?}");

    app.keys("ctrl-k");
    app.settle();
    assert!(app.has("palette-hint"), "a hint row while nothing is typed");
    app.input("+");
    assert!(!app.has("palette-hint"), "gone once the person types");
}

// ---- the card that says what to do in a plan with only a Test Plan ----

#[gpui::test]
fn an_empty_plan_shows_a_card_whose_button_adds_a_thread_group(cx: &mut TestAppContext) {
    let (p, _) = new_plan(cx, "first-card");
    let mut app = start(cx, Some(p), None);
    app.settle();
    assert!(app.has("first-steps"), "the card is there for a plan with only its Test Plan");
    assert_eq!(app.state()["elements"], 1);

    app.click("add-thread-group");
    app.settle();
    assert_eq!(app.state()["elements"], 2, "the button added a Thread Group");
    assert!(!app.has("first-steps"), "and the card is gone");

    app.keys("ctrl-z");
    app.settle();
    assert_eq!(app.state()["elements"], 1, "one Undo takes the Thread Group out");
    assert!(app.has("first-steps"), "and the card is back");
}

#[gpui::test]
fn the_card_stays_away_when_the_plan_has_anything_else(cx: &mut TestAppContext) {
    let (p, _) = new_plan(cx, "first-card-other");
    let mut app = start(cx, Some(p), None);
    add(&mut app, "CounterConfig", "CounterConfigGui");
    app.settle();
    assert!(!app.has("first-steps"));
    let mut app2 = start(cx, None, None);
    app2.settle();
    assert!(!app2.has("first-steps"), "no plan, no card");
}
