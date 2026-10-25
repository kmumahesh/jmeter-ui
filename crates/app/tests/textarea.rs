//! The text area, driven through simulated keystrokes, typing, the mouse and the wheel in a headless window.

mod common;

use common::secondary;
use gpui::{Entity, Focusable, Modifiers, ScrollDelta, ScrollWheelEvent, TestAppContext, TouchPhase, VisualTestContext, point, px};
use jmeter_ui::fields;
use jmeter_ui::textarea::{LINE_HEIGHT, MAX_CHARS, TextArea, TextAreaEvent};
use shell_core::highlight::Language;
use shell_core::settings::ThemeChoice;
use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

fn setup(cx: &mut TestAppContext) {
    cx.update(|cx| {
        jmeter_ui::theme::init(cx, ThemeChoice::Dark);
        fields::bind_keys(cx);
    });
}

fn area<'a>(cx: &'a mut TestAppContext, text: &str, rows: usize) -> (Entity<TextArea>, &'a mut VisualTestContext) {
    setup(cx);
    let text = text.to_string();
    let (a, cx) = cx.add_window_view(move |_, cx| TextArea::new(cx, "area", "Type here", &text).rows(rows));
    focus(&a, cx);
    (a, cx)
}

fn focus(a: &Entity<TextArea>, cx: &mut VisualTestContext) {
    cx.update(|window, cx| {
        let handle = a.read(cx).focus_handle(cx);
        window.focus(&handle, cx);
    });
}

fn text(a: &Entity<TextArea>, cx: &mut VisualTestContext) -> String {
    cx.update(|_, cx| a.read(cx).text().to_string())
}

fn caret(a: &Entity<TextArea>, cx: &mut VisualTestContext) -> (usize, usize) {
    cx.update(|_, cx| a.read(cx).caret())
}

fn first_line(a: &Entity<TextArea>, cx: &mut VisualTestContext) -> usize {
    cx.update(|_, cx| a.read(cx).first_visible_line())
}

fn wheel(cx: &mut VisualTestContext, dx: f32, dy: f32, shift: bool) {
    cx.simulate_event(ScrollWheelEvent {
        position: point(px(200.), px(40.)),
        delta: ScrollDelta::Pixels(point(px(dx), px(dy))),
        modifiers: Modifiers { shift, ..Modifiers::default() },
        touch_phase: TouchPhase::Moved,
    });
}

fn events(a: &Entity<TextArea>, cx: &mut VisualTestContext) -> Rc<RefCell<Vec<TextAreaEvent>>> {
    let log = Rc::new(RefCell::new(Vec::new()));
    let l = log.clone();
    cx.update(|_, cx| {
        cx.subscribe(a, move |_, e: &TextAreaEvent, _| l.borrow_mut().push(e.clone())).detach();
    });
    log
}

fn lines(n: usize) -> String {
    (0..n).map(|i| format!("line {i}")).collect::<Vec<_>>().join("\n")
}

// ---- typing and lines ----

#[gpui::test]
fn enter_starts_a_line_and_backspace_joins_it_again(cx: &mut TestAppContext) {
    let (a, cx) = area(cx, "", 6);
    cx.simulate_input("one");
    cx.simulate_keystrokes("enter");
    cx.simulate_input("two");
    assert_eq!(text(&a, cx), "one\ntwo");
    assert_eq!(caret(&a, cx), (1, 3));
    cx.simulate_keystrokes("home backspace");
    assert_eq!(text(&a, cx), "onetwo", "backspace at the start of a line joins it to the one above");
    assert_eq!(caret(&a, cx), (0, 3));
}

#[gpui::test]
fn arrows_move_along_and_between_lines_keeping_the_column(cx: &mut TestAppContext) {
    let (a, cx) = area(cx, "long first line\nab\nthird line here", 6);
    cx.simulate_keystrokes(&secondary("ctrl-home"));
    cx.simulate_keystrokes("right right right right right right right right");
    assert_eq!(caret(&a, cx), (0, 8));
    cx.simulate_keystrokes("down");
    assert_eq!(caret(&a, cx), (1, 2), "the short line ends the move");
    cx.simulate_keystrokes("down");
    assert_eq!(caret(&a, cx), (2, 8), "and the column comes back");
    cx.simulate_keystrokes("up up");
    assert_eq!(caret(&a, cx), (0, 8));
    cx.simulate_keystrokes("end");
    assert_eq!(caret(&a, cx), (0, 15));
    cx.simulate_keystrokes("home");
    assert_eq!(caret(&a, cx), (0, 0));
    cx.simulate_keystrokes(&secondary("ctrl-end"));
    assert_eq!(caret(&a, cx), (2, 15), "Ctrl+End goes to the end of the text");
    cx.simulate_keystrokes(&secondary("ctrl-home"));
    assert_eq!(caret(&a, cx), (0, 0));
}

#[gpui::test]
fn enter_keeps_the_indent_and_goes_deeper_after_a_bracket(cx: &mut TestAppContext) {
    let (a, cx) = area(cx, "", 6);
    cx.simulate_input("{");
    cx.simulate_keystrokes("enter");
    cx.simulate_input("\"a\": 1");
    cx.simulate_keystrokes("enter");
    assert_eq!(text(&a, cx), "{\n  \"a\": 1\n  ");
    cx.simulate_keystrokes("shift-tab");
    assert_eq!(text(&a, cx), "{\n  \"a\": 1\n");
    cx.simulate_input("}");
    assert_eq!(text(&a, cx), "{\n  \"a\": 1\n}");
}

#[gpui::test]
fn tab_indents_and_shift_tab_takes_the_indent_off_every_selected_line(cx: &mut TestAppContext) {
    let (a, cx) = area(cx, "a\nb\nc", 6);
    cx.simulate_keystrokes(&secondary("ctrl-a"));
    cx.simulate_keystrokes("tab");
    assert_eq!(text(&a, cx), "  a\n  b\n  c");
    cx.simulate_keystrokes("shift-tab");
    assert_eq!(text(&a, cx), "a\nb\nc");
    cx.simulate_keystrokes(&secondary("ctrl-home"));
    cx.simulate_keystrokes("tab");
    assert_eq!(text(&a, cx), "  a\nb\nc", "a lone caret gets an indent at the caret");
}

#[gpui::test]
fn selecting_with_shift_and_arrows_then_typing_replaces_it(cx: &mut TestAppContext) {
    let (a, cx) = area(cx, "one\ntwo\nthree", 6);
    cx.simulate_keystrokes(&secondary("ctrl-home"));
    cx.simulate_keystrokes("shift-down shift-down shift-end");
    cx.simulate_input("X");
    assert_eq!(text(&a, cx), "X");
}

#[gpui::test]
fn word_moves_and_deletes_work_across_lines(cx: &mut TestAppContext) {
    let (a, cx) = area(cx, "alpha beta\ngamma", 6);
    cx.simulate_keystrokes(&secondary("ctrl-backspace"));
    assert_eq!(text(&a, cx), "alpha beta\n");
    cx.simulate_keystrokes(&secondary("ctrl-left"));
    assert_eq!(caret(&a, cx), (0, 6), "to the start of the last word of the line above");
}

// ---- clipboard ----

#[gpui::test]
fn copy_cut_and_paste_keep_the_lines(cx: &mut TestAppContext) {
    let (a, cx) = area(cx, "one\ntwo\nthree", 6);
    cx.simulate_keystrokes(&secondary("ctrl-a ctrl-c"));
    assert_eq!(cx.read_from_clipboard().and_then(|c| c.text()).as_deref(), Some("one\ntwo\nthree"));
    cx.simulate_keystrokes(&secondary("ctrl-x"));
    assert_eq!(text(&a, cx), "");
    cx.simulate_keystrokes(&secondary("ctrl-v ctrl-v"));
    assert_eq!(text(&a, cx), "one\ntwo\nthreeone\ntwo\nthree");
}

#[gpui::test]
fn pasted_line_breaks_are_unified_and_characters_a_plan_cannot_hold_are_dropped(cx: &mut TestAppContext) {
    let (a, cx) = area(cx, "", 6);
    cx.write_to_clipboard(gpui::ClipboardItem::new_string("a\r\nb\rc\u{0}d\u{1}e\u{b}f".into()));
    cx.simulate_keystrokes(&secondary("ctrl-v"));
    assert_eq!(text(&a, cx), "a\nb\ncdef");
    cx.simulate_input("x\u{0}y");
    assert_eq!(text(&a, cx), "a\nb\ncdefxy", "typing is cleaned the same way");
}

#[gpui::test]
fn a_paste_beyond_the_limit_is_cut_not_refused(cx: &mut TestAppContext) {
    let (a, cx) = area(cx, "", 6);
    cx.write_to_clipboard(gpui::ClipboardItem::new_string("x".repeat(MAX_CHARS + 1000)));
    cx.simulate_keystrokes(&secondary("ctrl-v"));
    assert_eq!(text(&a, cx).chars().count(), MAX_CHARS);
    cx.simulate_input("more");
    assert_eq!(text(&a, cx).chars().count(), MAX_CHARS, "a full area takes nothing more");
}

// ---- undo, events, read only ----

#[gpui::test]
fn undo_and_redo_step_through_the_edits(cx: &mut TestAppContext) {
    let (a, cx) = area(cx, "", 6);
    cx.simulate_input("abc");
    cx.simulate_keystrokes("enter");
    cx.simulate_input("def");
    assert_eq!(text(&a, cx), "abc\ndef");
    cx.simulate_keystrokes(&secondary("ctrl-z"));
    assert_eq!(text(&a, cx), "abc\n", "typing in a row is one step");
    cx.simulate_keystrokes(&secondary("ctrl-z"));
    assert_eq!(text(&a, cx), "abc", "a new line is its own step");
    cx.simulate_keystrokes(&secondary("ctrl-shift-z"));
    assert_eq!(text(&a, cx), "abc\n");
    cx.simulate_keystrokes(&secondary("ctrl-y"));
    assert_eq!(text(&a, cx), "abc\ndef");
}

#[gpui::test]
fn changed_is_emitted_for_edits_and_not_for_moves(cx: &mut TestAppContext) {
    let (a, cx) = area(cx, "ab\ncd", 6);
    let log = events(&a, cx);
    cx.simulate_keystrokes("up down left right home end shift-left");
    assert!(log.borrow().is_empty(), "{:?}", log.borrow());
    cx.simulate_keystrokes("backspace");
    cx.simulate_keystrokes("enter");
    assert_eq!(*log.borrow(), [TextAreaEvent::Changed, TextAreaEvent::Changed]);
}

#[gpui::test]
fn a_read_only_area_copies_but_never_changes(cx: &mut TestAppContext) {
    setup(cx);
    let (a, cx) = cx.add_window_view(|_, cx| TextArea::new(cx, "ro", "", "locked\ntext").read_only(true));
    focus(&a, cx);
    cx.simulate_input("x");
    cx.simulate_keystrokes("enter tab backspace delete");
    cx.simulate_keystrokes(&secondary("ctrl-a ctrl-x"));
    assert_eq!(text(&a, cx), "locked\ntext");
    assert_eq!(cx.read_from_clipboard().and_then(|c| c.text()).as_deref(), Some("locked\ntext"), "cut copies in a read-only area");
    cx.write_to_clipboard(gpui::ClipboardItem::new_string("pasted".into()));
    cx.simulate_keystrokes(&secondary("ctrl-v"));
    assert_eq!(text(&a, cx), "locked\ntext");
}

#[gpui::test]
fn set_text_replaces_everything_and_goes_back_to_the_top(cx: &mut TestAppContext) {
    let (a, cx) = area(cx, &lines(200), 5);
    cx.simulate_keystrokes(&secondary("ctrl-end"));
    cx.update(|_, cx| a.update(cx, |a, cx| a.set_text("fresh\ntext", cx)));
    assert_eq!(text(&a, cx), "fresh\ntext");
    assert_eq!(first_line(&a, cx), 0);
    cx.simulate_keystrokes(&secondary("ctrl-z"));
    assert_eq!(text(&a, cx), "fresh\ntext", "a programmatic change is not undoable");
}

#[gpui::test]
fn unicode_is_typed_and_deleted_whole(cx: &mut TestAppContext) {
    let (a, cx) = area(cx, "", 6);
    cx.simulate_input("caf\u{e9} \u{1F600}");
    cx.simulate_keystrokes("enter");
    cx.simulate_input("e\u{301}x");
    assert_eq!(text(&a, cx), "caf\u{e9} \u{1F600}\ne\u{301}x");
    cx.simulate_keystrokes("left backspace");
    assert_eq!(text(&a, cx), "caf\u{e9} \u{1F600}\nx", "the accent goes with its letter");
    cx.simulate_keystrokes("up end backspace");
    assert_eq!(text(&a, cx), "caf\u{e9} \nx", "the emoji goes whole");
}

// ---- scrolling ----

#[gpui::test]
fn the_view_follows_the_caret_down_and_back_up(cx: &mut TestAppContext) {
    let (a, cx) = area(cx, &lines(100), 5);
    cx.simulate_keystrokes(&secondary("ctrl-home"));
    assert_eq!(first_line(&a, cx), 0);
    for _ in 0..30 {
        cx.simulate_keystrokes("down");
    }
    let top = first_line(&a, cx);
    assert!(top > 0 && top <= 30 && 30 < top + 6, "line 30 must show: the first visible line is {top}");
    cx.simulate_keystrokes(&secondary("ctrl-end"));
    let end = first_line(&a, cx);
    assert!((94..=99).contains(&end), "the end of the text shows: {end}");
    cx.simulate_keystrokes(&secondary("ctrl-home"));
    assert_eq!(first_line(&a, cx), 0);
}

#[gpui::test]
fn page_up_and_page_down_move_by_what_shows(cx: &mut TestAppContext) {
    let (a, cx) = area(cx, &lines(200), 8);
    cx.simulate_keystrokes(&secondary("ctrl-home"));
    cx.simulate_keystrokes("pagedown");
    let (l1, _) = caret(&a, cx);
    assert!((6..=8).contains(&l1), "one page is about the rows that show: {l1}");
    cx.simulate_keystrokes("pagedown pagedown");
    let (l3, _) = caret(&a, cx);
    assert!(l3 > l1 * 2, "{l3}");
    cx.simulate_keystrokes("pageup");
    assert!(caret(&a, cx).0 < l3);
    cx.simulate_keystrokes("shift-pagedown");
    cx.simulate_input("X");
    assert_eq!(text(&a, cx).matches('X').count(), 1, "a selection by pages is replaced by typing");
}

#[gpui::test]
fn the_wheel_scrolls_the_area_and_stops_at_its_ends(cx: &mut TestAppContext) {
    let (a, cx) = area(cx, &lines(100), 5);
    assert_eq!(first_line(&a, cx), 0);
    wheel(cx, 0., -(LINE_HEIGHT * 10.), false);
    cx.run_until_parked();
    assert_eq!(first_line(&a, cx), 10, "ten lines down");
    wheel(cx, 0., LINE_HEIGHT * 4., false);
    assert_eq!(first_line(&a, cx), 6, "four back up");
    wheel(cx, 0., -(LINE_HEIGHT * 10_000.), false);
    let end = first_line(&a, cx);
    assert!((95..=100).contains(&end), "stops at the end: {end}");
    wheel(cx, 0., LINE_HEIGHT * 10_000., false);
    assert_eq!(first_line(&a, cx), 0, "stops at the start");
}

#[gpui::test]
fn a_short_text_does_not_scroll(cx: &mut TestAppContext) {
    let (a, cx) = area(cx, "a\nb", 8);
    wheel(cx, 0., -500., false);
    assert_eq!(first_line(&a, cx), 0);
}

// ---- the mouse ----

#[gpui::test]
fn a_click_puts_the_caret_on_that_line_and_double_and_triple_clicks_select(cx: &mut TestAppContext) {
    let (a, cx) = area(cx, "alpha beta\ngamma delta\nlast", 6);
    // The text starts a few pixels below the top of the area (border and padding).
    let at = |line: f32| point(px(400.), px(3. + LINE_HEIGHT * line + LINE_HEIGHT / 2.));
    cx.simulate_click(at(1.), Modifiers::default());
    assert_eq!(caret(&a, cx), (1, 11), "a click right of the text goes to the end of that line");
    cx.simulate_click(at(0.), Modifiers::default());
    assert_eq!(caret(&a, cx).0, 0);
    cx.simulate_click(point(px(2.), px(3. + LINE_HEIGHT * 2. + 4.)), Modifiers::default());
    assert_eq!(caret(&a, cx).0, 2, "a click in the line numbers goes to that line too");
    // Past the last line is the last line.
    cx.simulate_click(point(px(400.), px(3. + LINE_HEIGHT * 5.5)), Modifiers::default());
    assert_eq!(caret(&a, cx).0, 2);
}

#[gpui::test]
fn dragging_selects_across_lines(cx: &mut TestAppContext) {
    let (_a, cx) = area(cx, "one\ntwo\nthree", 6);
    let y = |line: f32| px(3. + LINE_HEIGHT * line + LINE_HEIGHT / 2.);
    cx.simulate_mouse_down(point(px(30.), y(0.)), gpui::MouseButton::Left, Modifiers::default());
    cx.simulate_mouse_move(point(px(400.), y(1.)), Some(gpui::MouseButton::Left), Modifiers::default());
    cx.simulate_mouse_move(point(px(400.), y(2.)), Some(gpui::MouseButton::Left), Modifiers::default());
    cx.simulate_mouse_up(point(px(400.), y(2.)), gpui::MouseButton::Left, Modifiers::default());
    cx.simulate_keystrokes(&secondary("ctrl-c"));
    assert_eq!(cx.read_from_clipboard().and_then(|c| c.text()).as_deref(), Some("one\ntwo\nthree"));
}

// ---- colour and language ----

#[gpui::test]
fn the_language_is_guessed_from_the_text_and_follows_it(cx: &mut TestAppContext) {
    let (a, cx) = area(cx, "", 6);
    let lang = |a: &Entity<TextArea>, cx: &mut VisualTestContext| cx.update(|_, cx| a.read(cx).language_in_use());
    assert_eq!(lang(&a, cx), Language::Plain);
    cx.simulate_input("{");
    assert_eq!(lang(&a, cx), Language::Json);
    cx.simulate_keystrokes(&secondary("ctrl-a"));
    cx.simulate_input("<a/>");
    assert_eq!(lang(&a, cx), Language::Xml);
    cx.simulate_keystrokes(&secondary("ctrl-a"));
    cx.simulate_input("plain words");
    assert_eq!(lang(&a, cx), Language::Plain);
}

#[gpui::test]
fn a_fixed_language_is_kept(cx: &mut TestAppContext) {
    setup(cx);
    let (a, cx) = cx.add_window_view(|_, cx| TextArea::new(cx, "j", "", "plain").language(Language::Json));
    cx.update(|_, cx| assert_eq!(a.read(cx).language_in_use(), Language::Json));
    focus(&a, cx);
    cx.simulate_input(" <x>");
    cx.update(|_, cx| assert_eq!(a.read(cx).language_in_use(), Language::Json));
}

// ---- big and awkward texts ----

#[gpui::test]
fn a_text_of_many_thousands_of_lines_draws_and_scrolls_quickly(cx: &mut TestAppContext) {
    let (a, cx) = area(cx, "", 12);
    let big = (0..200_000).map(|i| format!("{{\"id\": {i}, \"name\": \"item ${{n}}\"}}")).collect::<Vec<_>>().join("\n");
    let t = Instant::now();
    cx.update(|_, cx| a.update(cx, |a, cx| a.set_text(&big, cx)));
    cx.run_until_parked();
    for _ in 0..40 {
        wheel(cx, 0., -(LINE_HEIGHT * 5000.), false);
        cx.run_until_parked();
    }
    cx.simulate_keystrokes(&secondary("ctrl-end"));
    cx.simulate_keystrokes(&secondary("ctrl-home"));
    cx.simulate_keystrokes("pagedown pagedown pagedown");
    assert!(t.elapsed() < Duration::from_secs(20), "200,000 lines took {:?}", t.elapsed());
    assert_eq!(cx.update(|_, cx| a.read(cx).line_count()), 200_000);
}

#[gpui::test]
fn typing_into_a_long_text_stays_responsive(cx: &mut TestAppContext) {
    let (a, cx) = area(cx, &lines(50_000), 10);
    cx.simulate_keystrokes(&secondary("ctrl-end"));
    let t = Instant::now();
    for _ in 0..100 {
        cx.simulate_input("x");
        cx.simulate_keystrokes("enter");
    }
    assert!(t.elapsed() < Duration::from_secs(15), "{:?}", t.elapsed());
    assert_eq!(cx.update(|_, cx| a.read(cx).line_count()), 50_100);
}

#[gpui::test]
fn one_very_long_line_does_not_hang_the_window(cx: &mut TestAppContext) {
    let (a, cx) = area(cx, &"x".repeat(300_000), 6);
    let t = Instant::now();
    cx.simulate_keystrokes("end");
    cx.simulate_input("y");
    cx.simulate_keystrokes("home");
    assert!(t.elapsed() < Duration::from_secs(20), "{:?}", t.elapsed());
    assert_eq!(text(&a, cx).len(), 300_001);
}

#[gpui::test]
fn many_blank_lines_and_odd_text_are_fine(cx: &mut TestAppContext) {
    let (a, cx) = area(cx, &"\n".repeat(5000), 6);
    cx.simulate_keystrokes(&secondary("ctrl-end"));
    cx.simulate_input("end");
    assert_eq!(caret(&a, cx), (5000, 3));
    cx.update(|_, cx| a.update(cx, |a, cx| a.set_text("\u{1F600}\u{301}\n\t\ttabs\n  ", cx)));
    cx.simulate_keystrokes("up up end");
    assert_eq!(caret(&a, cx).0, 0);
}
