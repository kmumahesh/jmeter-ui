//! Single-line text and number fields.
//!
//! Editing rules live in `shell_core::text_buffer` (tested without a window). This file connects
//! them to the keyboard, mouse, clipboard and input methods, and draws the field.

use crate::theme;
use gpui::{
    App, Bounds, ClipboardItem, Context, ContentMask, CursorStyle, Element, ElementId, ElementInputHandler, Entity, EntityInputHandler, EventEmitter,
    FocusHandle, Focusable, GlobalElementId, InspectorElementId, IntoElement, KeyBinding, LayoutId, MouseButton, MouseDownEvent, MouseMoveEvent,
    MouseUpEvent, PaintQuad, Pixels, Point, Render, ShapedLine, SharedString, Style, TextAlign, TextRun, UTF16Selection, UnderlineStyle, Window,
    actions, div, fill, point, prelude::*, px, relative, size,
};
use shell_core::number::{NumberSpec, Problem};
use shell_core::text_buffer::TextBuffer;
use std::ops::Range;

actions!(
    text_field,
    [
        Backspace,
        Delete,
        DeleteWordLeft,
        DeleteWordRight,
        Left,
        Right,
        WordLeft,
        WordRight,
        SelectLeft,
        SelectRight,
        SelectWordLeft,
        SelectWordRight,
        Home,
        End,
        SelectHome,
        SelectEnd,
        SelectAll,
        Cut,
        Copy,
        Paste,
        Undo,
        Redo,
        Submit,
        Cancel,
        StepUp,
        StepDown,
        StepUpBig,
        StepDownBig,
        FocusNextField,
        FocusPrevField,
    ]
);

pub const CONTEXT: &str = "TextField";

/// Register the editing shortcuts for every text field.
pub fn bind_keys(cx: &mut App) {
    let c = Some(CONTEXT);
    cx.bind_keys([
        KeyBinding::new("backspace", Backspace, c),
        KeyBinding::new("delete", Delete, c),
        KeyBinding::new("secondary-backspace", DeleteWordLeft, c),
        KeyBinding::new("secondary-delete", DeleteWordRight, c),
        KeyBinding::new("left", Left, c),
        KeyBinding::new("right", Right, c),
        KeyBinding::new("secondary-left", WordLeft, c),
        KeyBinding::new("secondary-right", WordRight, c),
        KeyBinding::new("shift-left", SelectLeft, c),
        KeyBinding::new("shift-right", SelectRight, c),
        KeyBinding::new("secondary-shift-left", SelectWordLeft, c),
        KeyBinding::new("secondary-shift-right", SelectWordRight, c),
        KeyBinding::new("home", Home, c),
        KeyBinding::new("end", End, c),
        KeyBinding::new("shift-home", SelectHome, c),
        KeyBinding::new("shift-end", SelectEnd, c),
        KeyBinding::new("secondary-a", SelectAll, c),
        KeyBinding::new("secondary-x", Cut, c),
        KeyBinding::new("secondary-c", Copy, c),
        KeyBinding::new("secondary-v", Paste, c),
        KeyBinding::new("secondary-z", Undo, c),
        KeyBinding::new("secondary-shift-z", Redo, c),
        KeyBinding::new("secondary-y", Redo, c),
        KeyBinding::new("enter", Submit, c),
        KeyBinding::new("escape", Cancel, c),
        KeyBinding::new("up", StepUp, c),
        KeyBinding::new("down", StepDown, c),
        KeyBinding::new("pageup", StepUpBig, c),
        KeyBinding::new("pagedown", StepDownBig, c),
        KeyBinding::new("tab", FocusNextField, c),
        KeyBinding::new("shift-tab", FocusPrevField, c),
    ]);
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TextFieldEvent {
    Changed,
    Submitted,
    Cancelled,
    /// The field lost the keyboard focus (to another field, or by a click elsewhere).
    Blurred,
    /// Up, down, page up and page down in a field that is not numeric (a list next to it can use them).
    Up,
    Down,
    PageUp,
    PageDown,
}

pub struct TextField {
    focus_handle: FocusHandle,
    buffer: TextBuffer,
    placeholder: SharedString,
    /// Text being composed by an input method, as a byte range.
    marked: Option<Range<usize>>,
    number: Option<NumberSpec>,
    read_only: bool,
    last_layout: Option<ShapedLine>,
    last_bounds: Option<Bounds<Pixels>>,
    /// How far the text is scrolled to the left to keep the caret visible.
    scroll_x: Pixels,
    is_selecting: bool,
    id: ElementId,
}

impl EventEmitter<TextFieldEvent> for TextField {}

impl TextField {
    pub fn new(cx: &mut Context<Self>, id: impl Into<ElementId>, placeholder: &str, text: &str) -> Self {
        TextField {
            focus_handle: cx.focus_handle().tab_stop(true),
            buffer: TextBuffer::new(text),
            placeholder: placeholder.to_string().into(),
            marked: None,
            number: None,
            read_only: false,
            last_layout: None,
            last_bounds: None,
            scroll_x: px(0.),
            is_selecting: false,
            id: id.into(),
        }
    }

    pub fn number(mut self, spec: NumberSpec) -> Self {
        self.number = Some(spec);
        self
    }

    pub fn read_only(mut self, v: bool) -> Self {
        self.read_only = v;
        self
    }

    pub fn text(&self) -> &str {
        self.buffer.text()
    }

    pub fn buffer(&self) -> &TextBuffer {
        &self.buffer
    }

    pub fn set_text(&mut self, text: &str, cx: &mut Context<Self>) {
        self.buffer.set_text(text);
        self.marked = None;
        cx.notify();
    }

    pub fn set_placeholder(&mut self, p: &str, cx: &mut Context<Self>) {
        self.placeholder = p.to_string().into();
        cx.notify();
    }

    /// For number fields: what is wrong with the current text, if anything.
    pub fn problem(&self) -> Option<Problem> {
        self.number.and_then(|n| n.parse(self.text()).err())
    }

    /// Emit `Blurred` when the field loses the keyboard focus.
    pub fn watch_blur(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        cx.on_focus_out(&self.focus_handle, window, |_, _, _, cx| cx.emit(TextFieldEvent::Blurred)).detach();
    }

    /// The name this field was given when it was made.
    pub fn field_id(&self) -> String {
        format!("{:?}", self.id)
    }

    pub fn select_all(&mut self, cx: &mut Context<Self>) {
        self.buffer.select_all();
        cx.notify();
    }

    pub fn is_focused(&self, window: &Window) -> bool {
        self.focus_handle.is_focused(window)
    }

    pub fn focus(&self, window: &mut Window, cx: &mut App) {
        window.focus(&self.focus_handle, cx);
    }

    fn changed(&mut self, cx: &mut Context<Self>) {
        self.scroll_x = self.scroll_x.max(px(0.));
        cx.emit(TextFieldEvent::Changed);
        cx.notify();
    }

    /// Type or paste text, refusing it in a number field if it could never become a valid number.
    fn type_text(&mut self, s: &str, window: &mut Window, cx: &mut Context<Self>) {
        if self.read_only {
            return;
        }
        if let Some(spec) = self.number {
            let mut probe = self.buffer.clone();
            probe.insert(s);
            if !spec.accepts_partial(probe.text()) {
                window.play_system_bell();
                return;
            }
        }
        if self.buffer.insert(s) {
            self.changed(cx);
        }
    }

    fn edit(&mut self, window: &mut Window, cx: &mut Context<Self>, f: impl FnOnce(&mut TextBuffer) -> bool) {
        if self.read_only {
            return;
        }
        if f(&mut self.buffer) {
            self.marked = None;
            self.changed(cx);
        } else {
            window.play_system_bell();
        }
    }

    fn navigate(&mut self, cx: &mut Context<Self>, f: impl FnOnce(&mut TextBuffer)) {
        f(&mut self.buffer);
        cx.notify();
    }

    fn step(&mut self, direction: i32, big: bool, cx: &mut Context<Self>) {
        if self.read_only {
            return;
        }
        let Some(spec) = self.number else {
            cx.emit(match (direction > 0, big) {
                (true, false) => TextFieldEvent::Up,
                (false, false) => TextFieldEvent::Down,
                (true, true) => TextFieldEvent::PageUp,
                (false, true) => TextFieldEvent::PageDown,
            });
            return;
        };
        let next = spec.step(self.text(), direction, big);
        if next != self.text() {
            let len = self.text().len();
            self.buffer.replace(0..len, &next);
            self.changed(cx);
        }
    }

    // ---- mouse ----

    fn index_for_x(&self, x: Pixels) -> usize {
        let (Some(bounds), Some(line)) = (self.last_bounds.as_ref(), self.last_layout.as_ref()) else { return 0 };
        line.closest_index_for_x(x - bounds.left() + self.scroll_x)
    }

    fn on_mouse_down(&mut self, e: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.focus_handle, cx);
        let idx = self.index_for_x(e.position.x);
        match e.click_count {
            0 | 1 => {
                self.is_selecting = true;
                self.buffer.set_cursor(idx, e.modifiers.shift);
            }
            2 => self.buffer.select_word_at(idx),
            _ => self.buffer.select_all(),
        }
        cx.notify();
    }

    fn on_mouse_up(&mut self, _: &MouseUpEvent, _: &mut Window, _: &mut Context<Self>) {
        self.is_selecting = false;
    }

    fn on_mouse_move(&mut self, e: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.is_selecting {
            self.buffer.set_cursor(self.index_for_x(e.position.x), true);
            cx.notify();
        }
    }

    // ---- clipboard ----

    fn paste(&mut self, _: &Paste, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            self.type_text(&text, window, cx);
        }
    }

    fn copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(t) = self.buffer.copy() {
            cx.write_to_clipboard(ClipboardItem::new_string(t));
        }
    }

    fn cut(&mut self, _: &Cut, window: &mut Window, cx: &mut Context<Self>) {
        if self.read_only {
            return self.copy(&Copy, window, cx);
        }
        if let Some(t) = self.buffer.cut() {
            cx.write_to_clipboard(ClipboardItem::new_string(t));
            self.changed(cx);
        }
    }

    // ---- input method support ----

    fn offset_from_utf16(&self, offset: usize) -> usize {
        let mut utf8 = 0;
        let mut utf16 = 0;
        for ch in self.text().chars() {
            if utf16 >= offset {
                break;
            }
            utf16 += ch.len_utf16();
            utf8 += ch.len_utf8();
        }
        utf8
    }

    fn offset_to_utf16(&self, offset: usize) -> usize {
        let mut utf16 = 0;
        let mut utf8 = 0;
        for ch in self.text().chars() {
            if utf8 >= offset {
                break;
            }
            utf8 += ch.len_utf8();
            utf16 += ch.len_utf16();
        }
        utf16
    }

    fn range_to_utf16(&self, r: &Range<usize>) -> Range<usize> {
        self.offset_to_utf16(r.start)..self.offset_to_utf16(r.end)
    }

    fn range_from_utf16(&self, r: &Range<usize>) -> Range<usize> {
        self.offset_from_utf16(r.start)..self.offset_from_utf16(r.end)
    }
}

impl EntityInputHandler for TextField {
    fn text_for_range(&mut self, range_utf16: Range<usize>, actual: &mut Option<Range<usize>>, _: &mut Window, _: &mut Context<Self>) -> Option<String> {
        let range = self.range_from_utf16(&range_utf16);
        actual.replace(self.range_to_utf16(&range));
        Some(self.text()[range].to_string())
    }

    fn selected_text_range(&mut self, _ignore_disabled_input: bool, _: &mut Window, _: &mut Context<Self>) -> Option<UTF16Selection> {
        let sel = self.buffer.selection();
        Some(UTF16Selection { range: self.range_to_utf16(&sel), reversed: self.buffer.cursor() == sel.start && sel.start != sel.end })
    }

    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        self.marked.as_ref().map(|r| self.range_to_utf16(r))
    }

    fn unmark_text(&mut self, _: &mut Window, _: &mut Context<Self>) {
        self.marked = None;
    }

    fn replace_text_in_range(&mut self, range_utf16: Option<Range<usize>>, new_text: &str, window: &mut Window, cx: &mut Context<Self>) {
        match range_utf16.map(|r| self.range_from_utf16(&r)).or_else(|| self.marked.clone()) {
            Some(range) => {
                if !self.read_only && self.buffer.replace(range, new_text) {
                    self.changed(cx);
                }
            }
            None => self.type_text(new_text, window, cx),
        }
        self.marked = None;
    }

    fn replace_and_mark_text_in_range(&mut self, range_utf16: Option<Range<usize>>, new_text: &str, _new_selected: Option<Range<usize>>, _: &mut Window, cx: &mut Context<Self>) {
        if self.read_only {
            return;
        }
        let range = range_utf16.map(|r| self.range_from_utf16(&r)).or_else(|| self.marked.clone()).unwrap_or_else(|| self.buffer.selection());
        if self.buffer.replace(range.clone(), new_text) {
            self.changed(cx);
        }
        self.marked = (!new_text.is_empty()).then(|| range.start..range.start + new_text.len());
    }

    fn bounds_for_range(&mut self, range_utf16: Range<usize>, bounds: Bounds<Pixels>, _: &mut Window, _: &mut Context<Self>) -> Option<Bounds<Pixels>> {
        let layout = self.last_layout.as_ref()?;
        let range = self.range_from_utf16(&range_utf16);
        Some(Bounds::from_corners(
            point(bounds.left() + layout.x_for_index(range.start) - self.scroll_x, bounds.top()),
            point(bounds.left() + layout.x_for_index(range.end) - self.scroll_x, bounds.bottom()),
        ))
    }

    fn character_index_for_point(&mut self, p: Point<Pixels>, _: &mut Window, _: &mut Context<Self>) -> Option<usize> {
        let b = self.last_bounds?;
        let layout = self.last_layout.as_ref()?;
        let idx = layout.index_for_x(p.x - b.left() + self.scroll_x)?;
        Some(self.offset_to_utf16(idx))
    }
}

impl Focusable for TextField {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for TextField {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = theme::palette(cx);
        let focused = self.focus_handle.is_focused(window);
        let invalid = self.problem().is_some();
        let border = if invalid {
            p.err
        } else if focused {
            p.accent
        } else {
            p.line
        };
        div()
            .id(self.id.clone())
            .flex()
            .key_context(CONTEXT)
            .track_focus(&self.focus_handle)
            .cursor(if self.read_only { CursorStyle::Arrow } else { CursorStyle::IBeam })
            .on_action(cx.listener(|this, _: &Backspace, w, cx| this.edit(w, cx, |b| b.backspace())))
            .on_action(cx.listener(|this, _: &Delete, w, cx| this.edit(w, cx, |b| b.delete())))
            .on_action(cx.listener(|this, _: &DeleteWordLeft, w, cx| this.edit(w, cx, |b| b.delete_word_left())))
            .on_action(cx.listener(|this, _: &DeleteWordRight, w, cx| this.edit(w, cx, |b| b.delete_word_right())))
            .on_action(cx.listener(|this, _: &Left, _, cx| this.navigate(cx, |b| b.move_left(false))))
            .on_action(cx.listener(|this, _: &Right, _, cx| this.navigate(cx, |b| b.move_right(false))))
            .on_action(cx.listener(|this, _: &WordLeft, _, cx| this.navigate(cx, |b| b.move_word_left(false))))
            .on_action(cx.listener(|this, _: &WordRight, _, cx| this.navigate(cx, |b| b.move_word_right(false))))
            .on_action(cx.listener(|this, _: &SelectLeft, _, cx| this.navigate(cx, |b| b.move_left(true))))
            .on_action(cx.listener(|this, _: &SelectRight, _, cx| this.navigate(cx, |b| b.move_right(true))))
            .on_action(cx.listener(|this, _: &SelectWordLeft, _, cx| this.navigate(cx, |b| b.move_word_left(true))))
            .on_action(cx.listener(|this, _: &SelectWordRight, _, cx| this.navigate(cx, |b| b.move_word_right(true))))
            .on_action(cx.listener(|this, _: &Home, _, cx| this.navigate(cx, |b| b.home(false))))
            .on_action(cx.listener(|this, _: &End, _, cx| this.navigate(cx, |b| b.end(false))))
            .on_action(cx.listener(|this, _: &SelectHome, _, cx| this.navigate(cx, |b| b.home(true))))
            .on_action(cx.listener(|this, _: &SelectEnd, _, cx| this.navigate(cx, |b| b.end(true))))
            .on_action(cx.listener(|this, _: &SelectAll, _, cx| this.navigate(cx, |b| b.select_all())))
            .on_action(cx.listener(Self::cut))
            .on_action(cx.listener(Self::copy))
            .on_action(cx.listener(Self::paste))
            .on_action(cx.listener(|this, _: &Undo, w, cx| this.edit(w, cx, |b| b.undo())))
            .on_action(cx.listener(|this, _: &Redo, w, cx| this.edit(w, cx, |b| b.redo())))
            .on_action(cx.listener(|_, _: &Submit, _, cx| cx.emit(TextFieldEvent::Submitted)))
            .on_action(cx.listener(|_, _: &Cancel, _, cx| cx.emit(TextFieldEvent::Cancelled)))
            .on_action(cx.listener(|_, _: &FocusNextField, window, cx| window.focus_next(cx)))
            .on_action(cx.listener(|_, _: &FocusPrevField, window, cx| window.focus_prev(cx)))
            .on_action(cx.listener(|this, _: &StepUp, _, cx| this.step(1, false, cx)))
            .on_action(cx.listener(|this, _: &StepDown, _, cx| this.step(-1, false, cx)))
            .on_action(cx.listener(|this, _: &StepUpBig, _, cx| this.step(1, true, cx)))
            .on_action(cx.listener(|this, _: &StepDownBig, _, cx| this.step(-1, true, cx)))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .line_height(px(18.))
            .text_size(px(13.))
            .w_full()
            .child(
                div()
                    .w_full()
                    .px(px(8.))
                    .py(px(4.))
                    .rounded(px(6.))
                    .bg(if self.read_only { p.panel } else { p.bg })
                    .border_1()
                    .border_color(border)
                    .child(TextElement { field: cx.entity() }),
            )
    }
}

struct TextElement {
    field: Entity<TextField>,
}

struct Prepaint {
    line: ShapedLine,
    cursor: Option<PaintQuad>,
    selection: Option<PaintQuad>,
    scroll_x: Pixels,
}

impl IntoElement for TextElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for TextElement {
    type RequestLayoutState = ();
    type PrepaintState = Prepaint;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(&mut self, _: Option<&GlobalElementId>, _: Option<&InspectorElementId>, window: &mut Window, cx: &mut App) -> (LayoutId, ()) {
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        style.size.height = window.line_height().into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(&mut self, _: Option<&GlobalElementId>, _: Option<&InspectorElementId>, bounds: Bounds<Pixels>, _: &mut (), window: &mut Window, cx: &mut App) -> Prepaint {
        let p = theme::palette(cx);
        let field = self.field.read(cx);
        let text = field.text().to_string();
        let cursor = field.buffer.cursor();
        let selection = field.buffer.selection();
        let marked = field.marked.clone();
        let focused = field.focus_handle.is_focused(window);
        let style = window.text_style();
        let (display, color) = if text.is_empty() { (field.placeholder.clone(), crate::theme::with_alpha(p.muted, 0.8)) } else { (SharedString::from(text), style.color) };
        let color = if field.read_only && !field.text().is_empty() { p.muted } else { color };
        let run = TextRun { len: display.len(), font: style.font(), color, background_color: None, underline: None, strikethrough: None };
        let runs = match marked.filter(|_| !field.text().is_empty()) {
            Some(m) if m.end <= display.len() => vec![
                TextRun { len: m.start, ..run.clone() },
                TextRun { len: m.end - m.start, underline: Some(UnderlineStyle { color: Some(run.color), thickness: px(1.0), wavy: false }), ..run.clone() },
                TextRun { len: display.len() - m.end, ..run },
            ]
            .into_iter()
            .filter(|r| r.len > 0)
            .collect(),
            _ => vec![run],
        };
        let font_size = style.font_size.to_pixels(window.rem_size());
        let line = window.text_system().shape_line(display, font_size, &runs, None);

        // Scroll so the caret stays inside the box.
        let mut scroll = field.scroll_x;
        let caret_x = line.x_for_index(cursor);
        let visible = bounds.size.width - px(2.);
        if caret_x - scroll > visible {
            scroll = caret_x - visible;
        }
        if caret_x - scroll < px(0.) {
            scroll = caret_x;
        }
        if line.width <= visible {
            scroll = px(0.);
        }
        scroll = scroll.max(px(0.));

        let (selection_quad, cursor_quad) = if selection.is_empty() {
            let q = focused.then(|| fill(Bounds::new(point(bounds.left() + caret_x - scroll, bounds.top()), size(px(1.5), bounds.size.height)), p.accent));
            (None, q)
        } else {
            let a = line.x_for_index(selection.start);
            let b = line.x_for_index(selection.end);
            (Some(fill(Bounds::from_corners(point(bounds.left() + a - scroll, bounds.top()), point(bounds.left() + b - scroll, bounds.bottom())), crate::theme::with_alpha(p.accent, 0.3))), None)
        };
        Prepaint { line, cursor: cursor_quad, selection: selection_quad, scroll_x: scroll }
    }

    fn paint(&mut self, _: Option<&GlobalElementId>, _: Option<&InspectorElementId>, bounds: Bounds<Pixels>, _: &mut (), prepaint: &mut Prepaint, window: &mut Window, cx: &mut App) {
        let focus = self.field.read(cx).focus_handle.clone();
        window.handle_input(&focus, ElementInputHandler::new(bounds, self.field.clone()), cx);
        let line = prepaint.line.clone();
        let origin = point(bounds.origin.x - prepaint.scroll_x, bounds.origin.y);
        let selection = prepaint.selection.take();
        let cursor = prepaint.cursor.take();
        window.with_content_mask(Some(ContentMask { bounds }), |window| {
            if let Some(s) = selection {
                window.paint_quad(s);
            }
            let _ = line.paint(origin, window.line_height(), TextAlign::Left, None, window, cx);
            if let Some(c) = cursor {
                window.paint_quad(c);
            }
        });
        let scroll = prepaint.scroll_x;
        self.field.update(cx, |f, _| {
            f.last_layout = Some(prepaint.line.clone());
            f.last_bounds = Some(bounds);
            f.scroll_x = scroll;
        });
    }
}
