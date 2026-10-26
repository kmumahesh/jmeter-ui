//! A text area: several lines of editable text with line numbers, colours for JSON, XML and `${variables}`, and
//! its own scrolling.
//!
//! The editing rules live in `shell_core::text_buffer` (multiline mode) and the colouring in
//! `shell_core::highlight`, both tested without a window. This file connects them to the keyboard, mouse,
//! clipboard and input methods, and draws the visible lines only, so a text of many thousands of lines is as
//! quick to draw as a short one.

use crate::fields::{
    Backspace, Copy, Cut, Delete, DeleteWordLeft, DeleteWordRight, End, Home, Left, Paste, Redo, Right, SelectAll, SelectEnd, SelectHome, SelectLeft, SelectRight,
    SelectWordLeft, SelectWordRight, Undo, WordLeft, WordRight,
};
use crate::theme::{self, Palette, with_alpha};
use gpui::{
    App, Bounds, ClipboardItem, Context, ContentMask, CursorStyle, Element, ElementId, ElementInputHandler, Entity, EntityInputHandler, EventEmitter, FocusHandle, Focusable,
    GlobalElementId, Hsla, InspectorElementId, IntoElement, KeyBinding, LayoutId, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Point, Render, ScrollWheelEvent,
    ShapedLine, SharedString, Style, TextAlign, TextRun, UTF16Selection, Window, actions, div, fill, point, prelude::*, px, relative, size,
};
use plan_core::Category;
use shell_core::highlight::{self, Language, Token};
use shell_core::lines::LineIndex;
use shell_core::text_buffer::TextBuffer;
use std::ops::Range;

actions!(
    text_area,
    [Up, Down, SelectUp, SelectDown, PageUp, PageDown, SelectPageUp, SelectPageDown, DocStart, DocEnd, SelectDocStart, SelectDocEnd, Newline, Tab, ShiftTab, Leave]
);

pub const CONTEXT: &str = "TextArea";

/// The height of one line, in pixels.
pub const LINE_HEIGHT: f32 = 18.0;

/// The most text an area takes (characters). A request body or a script is far below it.
pub const MAX_CHARS: usize = 4_000_000;

/// Register the editing shortcuts for every text area.
pub fn bind_keys(cx: &mut App) {
    let c = Some(CONTEXT);
    cx.bind_keys([
        KeyBinding::new("backspace", Backspace, c),
        KeyBinding::new("delete", Delete, c),
        KeyBinding::new("secondary-backspace", DeleteWordLeft, c),
        KeyBinding::new("secondary-delete", DeleteWordRight, c),
        KeyBinding::new("left", Left, c),
        KeyBinding::new("right", Right, c),
        KeyBinding::new("up", Up, c),
        KeyBinding::new("down", Down, c),
        KeyBinding::new("secondary-left", WordLeft, c),
        KeyBinding::new("secondary-right", WordRight, c),
        KeyBinding::new("shift-left", SelectLeft, c),
        KeyBinding::new("shift-right", SelectRight, c),
        KeyBinding::new("shift-up", SelectUp, c),
        KeyBinding::new("shift-down", SelectDown, c),
        KeyBinding::new("secondary-shift-left", SelectWordLeft, c),
        KeyBinding::new("secondary-shift-right", SelectWordRight, c),
        KeyBinding::new("home", Home, c),
        KeyBinding::new("end", End, c),
        KeyBinding::new("shift-home", SelectHome, c),
        KeyBinding::new("shift-end", SelectEnd, c),
        KeyBinding::new("secondary-home", DocStart, c),
        KeyBinding::new("secondary-end", DocEnd, c),
        KeyBinding::new("secondary-shift-home", SelectDocStart, c),
        KeyBinding::new("secondary-shift-end", SelectDocEnd, c),
        KeyBinding::new("pageup", PageUp, c),
        KeyBinding::new("pagedown", PageDown, c),
        KeyBinding::new("shift-pageup", SelectPageUp, c),
        KeyBinding::new("shift-pagedown", SelectPageDown, c),
        KeyBinding::new("secondary-a", SelectAll, c),
        KeyBinding::new("secondary-x", Cut, c),
        KeyBinding::new("secondary-c", Copy, c),
        KeyBinding::new("secondary-v", Paste, c),
        KeyBinding::new("secondary-z", Undo, c),
        KeyBinding::new("secondary-shift-z", Redo, c),
        KeyBinding::new("secondary-y", Redo, c),
        KeyBinding::new("enter", Newline, c),
        KeyBinding::new("tab", Tab, c),
        KeyBinding::new("shift-tab", ShiftTab, c),
        KeyBinding::new("escape", Leave, c),
    ]);
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TextAreaEvent {
    Changed,
    /// The area lost the keyboard focus.
    Blurred,
    /// Escape was pressed in it (the keyboard also moves on to the next control).
    Escaped,
}

/// What was drawn last, so a click or an input method can be mapped back to the text.
#[derive(Clone)]
struct Drawn {
    bounds: Bounds<Pixels>,
    /// The window x where the text starts, before it is scrolled sideways.
    text_left: Pixels,
    first_line: usize,
    /// The shaped lines from `first_line` on; `None` for an empty line.
    lines: Vec<Option<ShapedLine>>,
    /// How far the first line is scrolled up out of view.
    top_offset: Pixels,
}

pub struct TextArea {
    focus_handle: FocusHandle,
    buffer: TextBuffer,
    index: LineIndex,
    index_version: u64,
    language: Language,
    fixed_language: Option<Language>,
    placeholder: SharedString,
    marked: Option<Range<usize>>,
    read_only: bool,
    rows: usize,
    scroll_y: Pixels,
    scroll_x: Pixels,
    /// Scroll so the caret shows, the next time the area is drawn.
    reveal: bool,
    drawn: Option<Drawn>,
    is_selecting: bool,
    id: ElementId,
}

impl EventEmitter<TextAreaEvent> for TextArea {}

impl TextArea {
    pub fn new(cx: &mut Context<Self>, id: impl Into<ElementId>, placeholder: &str, text: &str) -> Self {
        let buffer = TextBuffer::multiline(text).with_max_chars(MAX_CHARS);
        let mut a = TextArea {
            focus_handle: cx.focus_handle().tab_stop(true),
            index: LineIndex::new(""),
            index_version: u64::MAX,
            language: Language::Plain,
            fixed_language: None,
            buffer,
            // Text shaping takes one line, so only the first line of a hint is shown.
            placeholder: placeholder.lines().next().unwrap_or("").to_string().into(),
            marked: None,
            read_only: false,
            rows: 10,
            scroll_y: px(0.),
            scroll_x: px(0.),
            reveal: false,
            drawn: None,
            is_selecting: false,
            id: id.into(),
        };
        a.sync();
        a
    }

    /// How many lines tall the area is.
    pub fn rows(mut self, rows: usize) -> Self {
        self.rows = rows.clamp(2, 60);
        self
    }

    pub fn read_only(mut self, v: bool) -> Self {
        self.read_only = v;
        self
    }

    /// Colour as this language instead of guessing it from the text.
    pub fn language(mut self, l: Language) -> Self {
        self.fixed_language = Some(l);
        self.language = l;
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
        self.scroll_y = px(0.);
        self.scroll_x = px(0.);
        self.sync();
        cx.notify();
    }

    pub fn is_focused(&self, window: &Window) -> bool {
        self.focus_handle.is_focused(window)
    }

    pub fn focus(&self, window: &mut Window, cx: &mut App) {
        window.focus(&self.focus_handle, cx);
    }

    /// The name this area was given when it was made.
    pub fn field_id(&self) -> String {
        format!("{:?}", self.id)
    }

    pub fn watch_blur(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        cx.on_focus_out(&self.focus_handle, window, |_, _, _, cx| cx.emit(TextAreaEvent::Blurred)).detach();
    }

    pub fn line_count(&self) -> usize {
        self.index.count()
    }

    /// The zero-based line and column of the caret.
    pub fn caret(&self) -> (usize, usize) {
        self.buffer.line_col(self.buffer.cursor())
    }

    /// The first line that shows, as last drawn.
    pub fn first_visible_line(&self) -> usize {
        (f32::from(self.scroll_y) / LINE_HEIGHT).floor().max(0.0) as usize
    }

    pub fn language_in_use(&self) -> Language {
        self.language
    }

    /// Bring what is worked out from the text (the line index, the language) up to date.
    fn sync(&mut self) {
        if self.index_version != self.buffer.version() {
            self.index = LineIndex::new(self.buffer.text());
            self.index_version = self.buffer.version();
            self.language = self.fixed_language.unwrap_or_else(|| Language::detect(self.buffer.text()));
        }
    }

    fn changed(&mut self, cx: &mut Context<Self>) {
        self.sync();
        self.reveal = true;
        cx.emit(TextAreaEvent::Changed);
        cx.notify();
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
        self.reveal = true;
        cx.notify();
    }

    /// How many lines a page up or down moves: what shows, less one for context.
    fn page(&self) -> isize {
        let rows = self.drawn.as_ref().map_or(self.rows, |d| (f32::from(d.bounds.size.height) / LINE_HEIGHT) as usize);
        rows.saturating_sub(1).max(1) as isize
    }

    fn max_scroll_y(&self) -> Pixels {
        let height = self.drawn.as_ref().map_or(px(self.rows as f32 * LINE_HEIGHT), |d| d.bounds.size.height);
        (px(self.index.count() as f32 * LINE_HEIGHT) - height).max(px(0.))
    }

    fn type_text(&mut self, s: &str) -> bool {
        !self.read_only && self.buffer.insert(s)
    }

    // ---- mouse ----

    /// The byte offset in the text under a window position.
    fn offset_at(&self, p: Point<Pixels>) -> usize {
        let Some(d) = &self.drawn else { return self.buffer.cursor() };
        let rel_y = f32::from(p.y - d.bounds.top() + self.scroll_y);
        let line = ((rel_y / LINE_HEIGHT).floor().max(0.0) as usize).min(self.index.count() - 1);
        let range = self.index.range(line);
        let shaped = line.checked_sub(d.first_line).and_then(|i| d.lines.get(i)).and_then(|l| l.as_ref());
        match shaped {
            Some(l) => (range.start + l.closest_index_for_x(p.x - d.text_left + self.scroll_x)).min(range.end),
            None => range.end,
        }
    }

    fn on_mouse_down(&mut self, e: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.focus_handle, cx);
        let at = self.offset_at(e.position);
        match e.click_count {
            0 | 1 => {
                self.is_selecting = true;
                self.buffer.set_cursor(at, e.modifiers.shift);
            }
            2 => self.buffer.select_word_at(at),
            _ => self.buffer.select_line_at(at),
        }
        self.reveal = false;
        cx.notify();
    }

    fn on_mouse_up(&mut self, _: &MouseUpEvent, _: &mut Window, _: &mut Context<Self>) {
        self.is_selecting = false;
    }

    fn on_mouse_move(&mut self, e: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.is_selecting {
            let at = self.offset_at(e.position);
            self.buffer.set_cursor(at, true);
            cx.notify();
        }
    }

    fn on_scroll(&mut self, e: &ScrollWheelEvent, _: &mut Window, cx: &mut Context<Self>) {
        let d = e.delta.pixel_delta(px(LINE_HEIGHT));
        let (mut dy, mut dx) = (d.y, d.x);
        if e.modifiers.shift && dx == px(0.) {
            std::mem::swap(&mut dx, &mut dy);
        }
        let y = (self.scroll_y - dy).clamp(px(0.), self.max_scroll_y());
        let x = (self.scroll_x - dx).max(px(0.));
        if y != self.scroll_y || x != self.scroll_x {
            self.scroll_y = y;
            self.scroll_x = x;
            self.reveal = false;
            // The wheel scrolls this area while it can; at its ends the page behind it scrolls.
            cx.stop_propagation();
            cx.notify();
        }
    }

    // ---- clipboard ----

    fn paste(&mut self, _: &Paste, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text())
            && self.type_text(&text)
        {
            self.changed(cx);
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

impl EntityInputHandler for TextArea {
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

    fn replace_text_in_range(&mut self, range_utf16: Option<Range<usize>>, new_text: &str, _: &mut Window, cx: &mut Context<Self>) {
        let changed = match range_utf16.map(|r| self.range_from_utf16(&r)).or_else(|| self.marked.clone()) {
            Some(range) => !self.read_only && self.buffer.replace(range, new_text),
            None => self.type_text(new_text),
        };
        if changed {
            self.changed(cx);
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
        let d = self.drawn.as_ref()?;
        let range = self.range_from_utf16(&range_utf16);
        let line = self.index.line_of(range.start);
        let lr = self.index.range(line);
        let shaped = line.checked_sub(d.first_line).and_then(|i| d.lines.get(i)).and_then(|l| l.as_ref());
        let x = |at: usize| shaped.map_or(px(0.), |l| l.x_for_index(at.saturating_sub(lr.start).min(lr.end - lr.start)));
        let top = d.bounds.top() + px((line.saturating_sub(d.first_line)) as f32 * LINE_HEIGHT) - d.top_offset;
        let end_x = if self.index.line_of(range.end) == line { x(range.end) } else { x(lr.end) };
        let _ = bounds;
        Some(Bounds::from_corners(point(d.text_left + x(range.start) - self.scroll_x, top), point(d.text_left + end_x - self.scroll_x, top + px(LINE_HEIGHT))))
    }

    fn character_index_for_point(&mut self, p: Point<Pixels>, _: &mut Window, _: &mut Context<Self>) -> Option<usize> {
        self.drawn.as_ref()?;
        Some(self.offset_to_utf16(self.offset_at(p)))
    }
}

impl Focusable for TextArea {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for TextArea {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = theme::palette(cx);
        let focused = self.focus_handle.is_focused(window);
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
            .on_action(cx.listener(|this, _: &Up, _, cx| this.navigate(cx, |b| b.move_up(false))))
            .on_action(cx.listener(|this, _: &Down, _, cx| this.navigate(cx, |b| b.move_down(false))))
            .on_action(cx.listener(|this, _: &WordLeft, _, cx| this.navigate(cx, |b| b.move_word_left(false))))
            .on_action(cx.listener(|this, _: &WordRight, _, cx| this.navigate(cx, |b| b.move_word_right(false))))
            .on_action(cx.listener(|this, _: &SelectLeft, _, cx| this.navigate(cx, |b| b.move_left(true))))
            .on_action(cx.listener(|this, _: &SelectRight, _, cx| this.navigate(cx, |b| b.move_right(true))))
            .on_action(cx.listener(|this, _: &SelectUp, _, cx| this.navigate(cx, |b| b.move_up(true))))
            .on_action(cx.listener(|this, _: &SelectDown, _, cx| this.navigate(cx, |b| b.move_down(true))))
            .on_action(cx.listener(|this, _: &SelectWordLeft, _, cx| this.navigate(cx, |b| b.move_word_left(true))))
            .on_action(cx.listener(|this, _: &SelectWordRight, _, cx| this.navigate(cx, |b| b.move_word_right(true))))
            .on_action(cx.listener(|this, _: &Home, _, cx| this.navigate(cx, |b| b.home(false))))
            .on_action(cx.listener(|this, _: &End, _, cx| this.navigate(cx, |b| b.end(false))))
            .on_action(cx.listener(|this, _: &SelectHome, _, cx| this.navigate(cx, |b| b.home(true))))
            .on_action(cx.listener(|this, _: &SelectEnd, _, cx| this.navigate(cx, |b| b.end(true))))
            .on_action(cx.listener(|this, _: &DocStart, _, cx| this.navigate(cx, |b| b.doc_start(false))))
            .on_action(cx.listener(|this, _: &DocEnd, _, cx| this.navigate(cx, |b| b.doc_end(false))))
            .on_action(cx.listener(|this, _: &SelectDocStart, _, cx| this.navigate(cx, |b| b.doc_start(true))))
            .on_action(cx.listener(|this, _: &SelectDocEnd, _, cx| this.navigate(cx, |b| b.doc_end(true))))
            .on_action(cx.listener(|this, _: &PageUp, _, cx| {
                let n = this.page();
                this.navigate(cx, |b| b.move_lines(-n, false))
            }))
            .on_action(cx.listener(|this, _: &PageDown, _, cx| {
                let n = this.page();
                this.navigate(cx, |b| b.move_lines(n, false))
            }))
            .on_action(cx.listener(|this, _: &SelectPageUp, _, cx| {
                let n = this.page();
                this.navigate(cx, |b| b.move_lines(-n, true))
            }))
            .on_action(cx.listener(|this, _: &SelectPageDown, _, cx| {
                let n = this.page();
                this.navigate(cx, |b| b.move_lines(n, true))
            }))
            .on_action(cx.listener(|this, _: &SelectAll, _, cx| this.navigate(cx, |b| b.select_all())))
            .on_action(cx.listener(Self::cut))
            .on_action(cx.listener(Self::copy))
            .on_action(cx.listener(Self::paste))
            .on_action(cx.listener(|this, _: &Undo, w, cx| this.edit(w, cx, |b| b.undo())))
            .on_action(cx.listener(|this, _: &Redo, w, cx| this.edit(w, cx, |b| b.redo())))
            .on_action(cx.listener(|this, _: &Newline, w, cx| this.edit(w, cx, |b| b.insert_newline())))
            .on_action(cx.listener(|this, _: &Tab, w, cx| this.edit(w, cx, |b| b.indent())))
            .on_action(cx.listener(|this, _: &ShiftTab, w, cx| this.edit(w, cx, |b| b.outdent())))
            .on_action(cx.listener(|_, _: &Leave, window, cx| {
                cx.emit(TextAreaEvent::Escaped);
                window.focus_next(cx)
            }))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .on_scroll_wheel(cx.listener(Self::on_scroll))
            .line_height(px(LINE_HEIGHT))
            .text_size(px(12.5))
            .font_family(crate::assets::MONO_FONT)
            .w_full()
            .child(
                div()
                    .w_full()
                    .py(px(2.))
                    .rounded(px(6.))
                    .bg(if self.read_only { p.panel } else { p.bg })
                    .border_1()
                    .border_color(if focused { p.accent } else { p.line })
                    .overflow_hidden()
                    .child(TextAreaElement { area: cx.entity() }),
            )
    }
}

fn colour(p: &Palette, t: Token) -> Hsla {
    match t {
        Token::Plain => p.fg,
        Token::String => p.ok,
        Token::Key => p.accent,
        Token::Number => p.warn,
        Token::Keyword => p.err,
        Token::Punct => p.muted,
        Token::Tag => p.accent,
        Token::Attribute => p.warn,
        Token::Comment => with_alpha(p.muted, 0.8),
        Token::Variable => p.category(Category::PostProcessor),
    }
}

struct TextAreaElement {
    area: Entity<TextArea>,
}

struct Prepaint {
    drawn: Drawn,
    gutter: Pixels,
    numbers: Vec<(ShapedLine, usize)>,
    selection: Vec<gpui::PaintQuad>,
    line_bg: Option<gpui::PaintQuad>,
    cursor: Option<gpui::PaintQuad>,
    bar: Option<gpui::PaintQuad>,
    placeholder: Option<ShapedLine>,
    scroll_y: Pixels,
    scroll_x: Pixels,
}

impl IntoElement for TextAreaElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for TextAreaElement {
    type RequestLayoutState = ();
    type PrepaintState = Prepaint;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(&mut self, _: Option<&GlobalElementId>, _: Option<&InspectorElementId>, window: &mut Window, cx: &mut App) -> (LayoutId, ()) {
        let rows = self.area.read(cx).rows;
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        style.size.height = px(rows as f32 * LINE_HEIGHT).into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(&mut self, _: Option<&GlobalElementId>, _: Option<&InspectorElementId>, bounds: Bounds<Pixels>, _: &mut (), window: &mut Window, cx: &mut App) -> Prepaint {
        let p = theme::palette(cx);
        let area = self.area.read(cx);
        let lh = px(LINE_HEIGHT);
        let style = window.text_style();
        let font_size = style.font_size.to_pixels(window.rem_size());
        let shape = |window: &mut Window, text: &str, runs: &[TextRun]| window.text_system().shape_line(SharedString::from(text.to_string()), font_size, runs, None);
        let plain = |len: usize, color: Hsla| TextRun { len, font: style.font(), color, background_color: None, underline: None, strikethrough: None };

        let count = area.index.count();
        let digits = area.index.digits().max(2);
        let digit_w = shape(window, "0", &[plain(1, p.muted)]).width;
        let gutter = digit_w * digits as f32 + px(18.);
        let text_left = bounds.left() + gutter + px(8.);
        let visible_w = bounds.right() - text_left - px(10.);

        // Where the text is scrolled to.
        let total_h = px(count as f32 * LINE_HEIGHT);
        let max_y = (total_h - bounds.size.height).max(px(0.));
        let mut scroll_y = area.scroll_y.clamp(px(0.), max_y);
        let (caret_line, _) = area.buffer.line_col(area.buffer.cursor());
        if area.reveal {
            let top = px(caret_line as f32 * LINE_HEIGHT);
            if top < scroll_y {
                scroll_y = top;
            }
            if top + lh > scroll_y + bounds.size.height {
                scroll_y = top + lh - bounds.size.height;
            }
            scroll_y = scroll_y.clamp(px(0.), max_y);
        }
        let first = (f32::from(scroll_y) / LINE_HEIGHT).floor().max(0.0) as usize;
        let top_offset = scroll_y - px(first as f32 * LINE_HEIGHT);
        let shown = ((f32::from(bounds.size.height) / LINE_HEIGHT).ceil() as usize + 1).min(count.saturating_sub(first));

        // Shape the lines that show, coloured.
        let text = area.text();
        let mut lines: Vec<Option<ShapedLine>> = Vec::with_capacity(shown);
        let mut widest = px(0.);
        for i in first..first + shown {
            let range = area.index.range(i);
            let line = &text[range];
            if line.is_empty() {
                lines.push(None);
                continue;
            }
            let runs: Vec<TextRun> = highlight::tokens(line, area.language).into_iter().map(|(r, t)| plain(r.len(), colour(&p, t))).collect();
            let shaped = shape(window, line, &runs);
            widest = widest.max(shaped.width);
            lines.push(Some(shaped));
        }

        // Sideways scrolling keeps the caret inside.
        let (cursor, selection) = (area.buffer.cursor(), area.buffer.selection());
        let caret_x = caret_line.checked_sub(first).and_then(|i| lines.get(i)).and_then(|l| l.as_ref()).map_or(px(0.), |l| l.x_for_index(cursor - area.index.start(caret_line)));
        let mut scroll_x = area.scroll_x;
        if area.reveal {
            if caret_x - scroll_x > visible_w {
                scroll_x = caret_x - visible_w;
            }
            if caret_x - scroll_x < px(0.) {
                scroll_x = caret_x;
            }
        }
        scroll_x = scroll_x.clamp(px(0.), (widest - visible_w).max(px(0.)));

        let focused = area.focus_handle.is_focused(window);
        let row_top = |i: usize| bounds.top() + px((i - first) as f32 * LINE_HEIGHT) - top_offset;

        // Selection, one quad per line it touches.
        let mut quads = Vec::new();
        if !selection.is_empty() {
            for i in first..first + shown {
                let lr = area.index.range(i);
                if selection.end <= lr.start || selection.start > lr.end {
                    continue;
                }
                let a = selection.start.max(lr.start) - lr.start;
                let b = selection.end.min(lr.end) - lr.start;
                let shaped = lines[i - first].as_ref();
                let xa = shaped.map_or(px(0.), |l| l.x_for_index(a));
                let mut xb = shaped.map_or(px(0.), |l| l.x_for_index(b));
                if selection.end > lr.end {
                    xb += px(6.); // the line break is selected too
                }
                if xb > xa {
                    quads.push(fill(Bounds::from_corners(point(text_left + xa - scroll_x, row_top(i)), point(text_left + xb - scroll_x, row_top(i) + lh)), with_alpha(p.accent, 0.3)));
                }
            }
        }
        let line_bg = (focused && selection.is_empty() && caret_line >= first && caret_line < first + shown)
            .then(|| fill(Bounds::new(point(bounds.left() + gutter, row_top(caret_line)), size(bounds.size.width - gutter, lh)), with_alpha(p.fg, 0.05)));
        let cursor = (focused && selection.is_empty() && caret_line >= first && caret_line < first + shown)
            .then(|| fill(Bounds::new(point(text_left + caret_x - scroll_x, row_top(caret_line)), size(px(1.5), lh)), p.accent));

        // Line numbers.
        let numbers: Vec<(ShapedLine, usize)> = (first..first + shown)
            .map(|i| {
                let n = (i + 1).to_string();
                let color = if i == caret_line && focused { p.fg } else { p.muted };
                (shape(window, &n, &[plain(n.len(), color)]), i)
            })
            .collect();

        // The scroll bar, when the text is taller than the area.
        let bar = (max_y > px(0.)).then(|| {
            let thumb = (bounds.size.height * (bounds.size.height / total_h)).max(px(24.));
            let y = bounds.top() + (scroll_y / max_y) * (bounds.size.height - thumb);
            fill(Bounds::new(point(bounds.right() - px(7.), y), size(px(4.), thumb)), with_alpha(p.muted, 0.45))
        });

        let placeholder = (text.is_empty() && !area.placeholder.is_empty()).then(|| {
            let t = area.placeholder.to_string();
            shape(window, &t, &[plain(t.len(), with_alpha(p.muted, 0.8))])
        });

        Prepaint { drawn: Drawn { bounds, text_left, first_line: first, lines, top_offset }, gutter, numbers, selection: quads, line_bg, cursor, bar, placeholder, scroll_y, scroll_x }
    }

    fn paint(&mut self, _: Option<&GlobalElementId>, _: Option<&InspectorElementId>, bounds: Bounds<Pixels>, _: &mut (), prepaint: &mut Prepaint, window: &mut Window, cx: &mut App) {
        let p = theme::palette(cx);
        let focus = self.area.read(cx).focus_handle.clone();
        window.handle_input(&focus, ElementInputHandler::new(bounds, self.area.clone()), cx);
        let lh = px(LINE_HEIGHT);
        let d = prepaint.drawn.clone();
        let (gutter, scroll_x) = (prepaint.gutter, prepaint.scroll_x);
        let numbers = std::mem::take(&mut prepaint.numbers);
        let selection = std::mem::take(&mut prepaint.selection);
        let line_bg = prepaint.line_bg.take();
        let cursor = prepaint.cursor.take();
        let bar = prepaint.bar.take();
        let placeholder = prepaint.placeholder.take();
        window.with_content_mask(Some(ContentMask { bounds }), |window| {
            // The gutter.
            window.paint_quad(fill(Bounds::new(bounds.origin, size(gutter, bounds.size.height)), p.panel));
            window.paint_quad(fill(Bounds::new(point(bounds.left() + gutter, bounds.top()), size(px(1.), bounds.size.height)), p.line));
            if let Some(q) = line_bg {
                window.paint_quad(q);
            }
            for (n, i) in &numbers {
                let y = bounds.top() + px((i - d.first_line) as f32 * LINE_HEIGHT) - d.top_offset;
                let _ = n.paint(point(bounds.left() + gutter - n.width - px(8.), y), lh, TextAlign::Left, None, window, cx);
            }
            for q in selection {
                window.paint_quad(q);
            }
            if let Some(ph) = &placeholder {
                let _ = ph.paint(point(d.text_left, bounds.top()), lh, TextAlign::Left, None, window, cx);
            }
            // The text, clipped to its own column so it never runs under the gutter.
            let column = Bounds::from_corners(point(bounds.left() + gutter + px(1.), bounds.top()), bounds.bottom_right());
            window.with_content_mask(Some(ContentMask { bounds: column }), |window| {
                for (k, line) in d.lines.iter().enumerate() {
                    if let Some(l) = line {
                        let y = bounds.top() + px(k as f32 * LINE_HEIGHT) - d.top_offset;
                        let _ = l.paint(point(d.text_left - scroll_x, y), lh, TextAlign::Left, None, window, cx);
                    }
                }
                if let Some(c) = cursor {
                    window.paint_quad(c);
                }
            });
            if let Some(b) = bar {
                window.paint_quad(b);
            }
        });
        let (sy, sx) = (prepaint.scroll_y, prepaint.scroll_x);
        self.area.update(cx, |a, _| {
            a.drawn = Some(d);
            a.scroll_y = sy;
            a.scroll_x = sx;
            a.reveal = false;
        });
    }
}
