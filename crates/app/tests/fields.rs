//! Drive the text field through simulated keystrokes in a headless window.

use gpui::{AppContext as _, Entity, Focusable, TestAppContext, VisualTestContext};
use jmeter_ui::fields::{self, TextField, TextFieldEvent};
use shell_core::number::NumberSpec;
use shell_core::settings::ThemeChoice;
use std::cell::RefCell;
use std::rc::Rc;

fn setup(cx: &mut TestAppContext) {
    cx.update(|cx| {
        jmeter_ui::theme::init(cx, ThemeChoice::Dark);
        fields::bind_keys(cx);
    });
}

fn field<'a>(cx: &'a mut TestAppContext, text: &str, number: Option<NumberSpec>) -> (Entity<TextField>, &'a mut VisualTestContext) {
    setup(cx);
    let text = text.to_string();
    let (f, cx) = cx.add_window_view(move |_, cx| {
        let mut f = TextField::new(cx, "field", "Type here", &text);
        if let Some(n) = number {
            f = f.number(n);
        }
        f
    });
    focus(&f, cx);
    (f, cx)
}

fn text(f: &Entity<TextField>, cx: &mut VisualTestContext) -> String {
    cx.update(|_, cx| f.read(cx).text().to_string())
}

#[gpui::test]
fn typing_and_backspace(cx: &mut TestAppContext) {
    let (f, cx) = field(cx, "", None);
    cx.simulate_input("hello");
    assert_eq!(text(&f, cx), "hello");
    cx.simulate_keystrokes("backspace backspace");
    assert_eq!(text(&f, cx), "hel");
}

#[gpui::test]
fn caret_movement_and_insertion(cx: &mut TestAppContext) {
    let (f, cx) = field(cx, "abc", None);
    cx.simulate_keystrokes("left left");
    cx.simulate_input("X");
    assert_eq!(text(&f, cx), "aXbc");
    cx.simulate_keystrokes("home");
    cx.simulate_input(">");
    assert_eq!(text(&f, cx), ">aXbc");
    cx.simulate_keystrokes("end delete");
    assert_eq!(text(&f, cx), ">aXbc", "delete at the end does nothing");
}

#[gpui::test]
fn select_all_and_replace(cx: &mut TestAppContext) {
    let (f, cx) = field(cx, "old value", None);
    cx.simulate_keystrokes("ctrl-a");
    cx.simulate_input("new");
    assert_eq!(text(&f, cx), "new");
}

#[gpui::test]
fn shift_arrows_select_and_delete(cx: &mut TestAppContext) {
    let (f, cx) = field(cx, "abcdef", None);
    cx.simulate_keystrokes("home shift-right shift-right shift-right backspace");
    assert_eq!(text(&f, cx), "def");
}

#[gpui::test]
fn word_navigation_and_deletion(cx: &mut TestAppContext) {
    let (f, cx) = field(cx, "one two three", None);
    cx.simulate_keystrokes("ctrl-backspace");
    assert_eq!(text(&f, cx), "one two ");
    cx.simulate_keystrokes("ctrl-left");
    cx.simulate_input("X");
    assert_eq!(text(&f, cx), "one Xtwo ");
    cx.simulate_keystrokes("ctrl-shift-right backspace");
    assert_eq!(text(&f, cx), "one X ", "the word after the caret is selected and deleted, the X stays");
}

#[gpui::test]
fn undo_and_redo(cx: &mut TestAppContext) {
    let (f, cx) = field(cx, "", None);
    cx.simulate_input("abc");
    assert_eq!(text(&f, cx), "abc");
    cx.simulate_keystrokes("ctrl-z");
    assert_eq!(text(&f, cx), "", "typing in a row is one undo step");
    cx.simulate_keystrokes("ctrl-shift-z");
    assert_eq!(text(&f, cx), "abc");
    cx.simulate_keystrokes("ctrl-z ctrl-y");
    assert_eq!(text(&f, cx), "abc");
}

#[gpui::test]
fn clipboard_copy_cut_paste(cx: &mut TestAppContext) {
    let (f, cx) = field(cx, "hello world", None);
    cx.simulate_keystrokes("ctrl-a ctrl-c");
    cx.simulate_keystrokes("end");
    cx.simulate_keystrokes("ctrl-v");
    assert_eq!(text(&f, cx), "hello worldhello world");
    cx.simulate_keystrokes("ctrl-a ctrl-x");
    assert_eq!(text(&f, cx), "");
    cx.simulate_keystrokes("ctrl-v");
    assert_eq!(text(&f, cx), "hello worldhello world");
}

#[gpui::test]
fn pasting_multiple_lines_gives_one_line(cx: &mut TestAppContext) {
    let (f, cx) = field(cx, "", None);
    cx.write_to_clipboard(gpui::ClipboardItem::new_string("a\nb\r\nc".into()));
    cx.simulate_keystrokes("ctrl-v");
    assert_eq!(text(&f, cx), "a b c");
}

#[gpui::test]
fn enter_and_escape_emit_events(cx: &mut TestAppContext) {
    let (f, cx) = field(cx, "x", None);
    let events = Rc::new(RefCell::new(Vec::new()));
    let log = events.clone();
    cx.update(|_, cx| {
        cx.subscribe(&f, move |_, e: &TextFieldEvent, _| log.borrow_mut().push(e.clone())).detach();
    });
    cx.simulate_input("y");
    cx.simulate_keystrokes("enter escape");
    assert_eq!(*events.borrow(), [TextFieldEvent::Changed, TextFieldEvent::Submitted, TextFieldEvent::Cancelled]);
}

#[gpui::test]
fn changed_is_emitted_for_every_edit_but_not_for_navigation(cx: &mut TestAppContext) {
    let (f, cx) = field(cx, "ab", None);
    let count = Rc::new(RefCell::new(0));
    let c = count.clone();
    cx.update(|_, cx| {
        cx.subscribe(&f, move |_, e: &TextFieldEvent, _| {
            if *e == TextFieldEvent::Changed {
                *c.borrow_mut() += 1
            }
        })
        .detach();
    });
    cx.simulate_keystrokes("left right home end");
    assert_eq!(*count.borrow(), 0);
    cx.simulate_keystrokes("backspace");
    assert_eq!(*count.borrow(), 1);
}

#[gpui::test]
fn number_field_rejects_letters(cx: &mut TestAppContext) {
    let (f, cx) = field(cx, "10", Some(NumberSpec::whole(1, 100)));
    cx.simulate_input("abc");
    assert_eq!(text(&f, cx), "10");
    cx.simulate_input("5");
    assert_eq!(text(&f, cx), "105");
    assert!(cx.update(|_, cx| f.read(cx).problem()).is_some(), "105 is above the maximum of 100");
}

#[gpui::test]
fn number_field_steps_with_arrow_keys(cx: &mut TestAppContext) {
    let (f, cx) = field(cx, "50", Some(NumberSpec::whole(1, 100)));
    cx.simulate_keystrokes("up up");
    assert_eq!(text(&f, cx), "52");
    cx.simulate_keystrokes("down");
    assert_eq!(text(&f, cx), "51");
    cx.simulate_keystrokes("pageup");
    assert_eq!(text(&f, cx), "61");
    cx.simulate_keystrokes("pagedown pagedown pagedown pagedown pagedown pagedown pagedown");
    assert_eq!(text(&f, cx), "1", "stops at the minimum");
    assert!(cx.update(|_, cx| f.read(cx).problem()).is_none());
}

#[gpui::test]
fn number_field_accepts_variables_and_does_not_step_them(cx: &mut TestAppContext) {
    let (f, cx) = field(cx, "", Some(NumberSpec::whole(1, 100)));
    cx.simulate_input("${users}");
    assert_eq!(text(&f, cx), "${users}");
    assert!(cx.update(|_, cx| f.read(cx).problem()).is_none());
    cx.simulate_keystrokes("up");
    assert_eq!(text(&f, cx), "${users}");
}

#[gpui::test]
fn number_field_reports_problems(cx: &mut TestAppContext) {
    let (f, cx) = field(cx, "", Some(NumberSpec::whole(1, 100)));
    assert!(cx.update(|_, cx| f.read(cx).problem()).is_some(), "empty is not allowed here");
    cx.simulate_input("0");
    let p = cx.update(|_, cx| f.read(cx).problem()).unwrap();
    assert!(p.message().contains("at least 1"));
}

#[gpui::test]
fn read_only_field_ignores_edits_but_allows_copy(cx: &mut TestAppContext) {
    setup(cx);
    let (f, cx) = cx.add_window_view(|_, cx| TextField::new(cx, "ro", "", "locked").read_only(true));
    focus(&f, cx);
    cx.simulate_input("x");
    cx.simulate_keystrokes("backspace ctrl-a ctrl-x");
    assert_eq!(text(&f, cx), "locked");
    assert_eq!(cx.read_from_clipboard().and_then(|c| c.text()).as_deref(), Some("locked"));
}

#[gpui::test]
fn set_text_replaces_the_content(cx: &mut TestAppContext) {
    let (f, cx) = field(cx, "before", None);
    cx.update(|_, cx| f.update(cx, |f, cx| f.set_text("after", cx)));
    assert_eq!(text(&f, cx), "after");
    cx.simulate_keystrokes("ctrl-z");
    assert_eq!(text(&f, cx), "after", "programmatic changes are not undoable");
}

#[gpui::test]
fn typing_unicode(cx: &mut TestAppContext) {
    let (f, cx) = field(cx, "", None);
    cx.simulate_input("caf\u{e9} \u{1F600}");
    assert_eq!(text(&f, cx), "caf\u{e9} \u{1F600}");
    cx.simulate_keystrokes("backspace");
    assert_eq!(text(&f, cx), "caf\u{e9} ", "the emoji is removed whole");
}

fn focus(f: &Entity<TextField>, cx: &mut VisualTestContext) {
    cx.update(|window, cx| {
        let handle = f.read(cx).focus_handle(cx);
        window.focus(&handle, cx);
    });
}

#[gpui::test]
fn losing_the_focus_is_reported_once(cx: &mut TestAppContext) {
    setup(cx);
    let events: Rc<RefCell<Vec<TextFieldEvent>>> = Rc::default();
    let (pair, cx) = cx.add_window_view(|window, cx| {
        let a = cx.new(|cx| TextField::new(cx, "a", "", "one"));
        let b = cx.new(|cx| TextField::new(cx, "b", "", "two"));
        a.update(cx, |f, cx| f.watch_blur(window, cx));
        pair_view::Pair { a, b }
    });
    let log = events.clone();
    let a = pair.read_with(cx, |p, _| p.a.clone());
    let b = pair.read_with(cx, |p, _| p.b.clone());
    cx.update(|_, app| {
        app.subscribe(&a, move |_, e: &TextFieldEvent, _| log.borrow_mut().push(e.clone())).detach();
    });
    cx.update(|window, _| window.activate_window());
    focus(&a, cx);
    cx.run_until_parked();
    assert!(events.borrow().is_empty(), "gaining the focus is not an event");
    focus(&b, cx);
    cx.run_until_parked();
    assert_eq!(*events.borrow(), [TextFieldEvent::Blurred]);
    focus(&a, cx);
    focus(&b, cx);
    cx.run_until_parked();
    assert_eq!(events.borrow().len(), 2, "and again the next time");
}

mod pair_view {
    use super::*;
    use gpui::{Context, IntoElement, ParentElement, Render, Window, div};

    pub struct Pair {
        pub a: Entity<TextField>,
        pub b: Entity<TextField>,
    }

    impl Render for Pair {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div().child(self.a.clone()).child(self.b.clone())
        }
    }
}
