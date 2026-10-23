//! The editing logic behind single-line text fields, independent of any UI toolkit.
//!
//! Positions are byte offsets that always sit on a grapheme boundary, so an accented letter or an
//! emoji is never split. Typing in a row is one undo step.

use std::ops::Range;
use unicode_segmentation::UnicodeSegmentation;

#[derive(Clone, Debug, PartialEq)]
struct Snapshot {
    text: String,
    anchor: usize,
    head: usize,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Edit {
    Typing,
    Backspace,
    Delete,
    Other,
}

#[derive(Clone, Debug)]
pub struct TextBuffer {
    text: String,
    /// Where the selection started; equal to `head` when nothing is selected.
    anchor: usize,
    /// The caret.
    head: usize,
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
    last: Option<Edit>,
    max_chars: Option<usize>,
    accept: Option<fn(char) -> bool>,
}

impl Default for TextBuffer {
    fn default() -> Self {
        TextBuffer::new("")
    }
}

const UNDO_LIMIT: usize = 200;

impl TextBuffer {
    pub fn new(text: &str) -> TextBuffer {
        let text = sanitize(text);
        let end = text.len();
        TextBuffer { text, anchor: end, head: end, undo: vec![], redo: vec![], last: None, max_chars: None, accept: None }
    }

    /// Only characters for which `f` is true can be typed or pasted.
    pub fn with_filter(mut self, f: fn(char) -> bool) -> TextBuffer {
        self.accept = Some(f);
        self
    }

    pub fn with_max_chars(mut self, n: usize) -> TextBuffer {
        self.max_chars = Some(n);
        self
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    /// The caret position.
    pub fn cursor(&self) -> usize {
        self.head
    }

    pub fn selection(&self) -> Range<usize> {
        self.anchor.min(self.head)..self.anchor.max(self.head)
    }

    pub fn has_selection(&self) -> bool {
        self.anchor != self.head
    }

    pub fn selected_text(&self) -> &str {
        &self.text[self.selection()]
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Replace the whole text from outside (for example after loading a plan). Clears undo history.
    pub fn set_text(&mut self, text: &str) {
        self.text = sanitize(text);
        self.head = self.text.len();
        self.anchor = self.head;
        self.undo.clear();
        self.redo.clear();
        self.last = None;
    }

    // ---- caret and selection ----

    /// Put the caret at `pos` (clamped to a grapheme boundary). With `extend` the selection grows.
    pub fn set_cursor(&mut self, pos: usize, extend: bool) {
        self.head = self.snap(pos);
        if !extend {
            self.anchor = self.head;
        }
        self.last = None;
    }

    pub fn select_all(&mut self) {
        self.anchor = 0;
        self.head = self.text.len();
        self.last = None;
    }

    pub fn select_range(&mut self, range: Range<usize>) {
        self.anchor = self.snap(range.start);
        self.head = self.snap(range.end);
        self.last = None;
    }

    pub fn move_left(&mut self, extend: bool) {
        if self.has_selection() && !extend {
            self.set_cursor(self.selection().start, false);
        } else {
            self.set_cursor(self.prev_boundary(self.head), extend);
        }
    }

    pub fn move_right(&mut self, extend: bool) {
        if self.has_selection() && !extend {
            self.set_cursor(self.selection().end, false);
        } else {
            self.set_cursor(self.next_boundary(self.head), extend);
        }
    }

    pub fn move_word_left(&mut self, extend: bool) {
        self.set_cursor(self.word_start_before(self.head), extend);
    }

    pub fn move_word_right(&mut self, extend: bool) {
        self.set_cursor(self.word_end_after(self.head), extend);
    }

    pub fn home(&mut self, extend: bool) {
        self.set_cursor(0, extend);
    }

    pub fn end(&mut self, extend: bool) {
        self.set_cursor(self.text.len(), extend);
    }

    /// The word, or the run of spaces or punctuation, at `pos`. Used for double-click.
    pub fn word_range_at(&self, pos: usize) -> Range<usize> {
        let pos = self.snap(pos);
        for (start, tok) in self.text.split_word_bound_indices() {
            let end = start + tok.len();
            if pos >= start && pos < end {
                return start..end;
            }
        }
        pos..pos
    }

    pub fn select_word_at(&mut self, pos: usize) {
        let r = self.word_range_at(pos);
        self.select_range(r);
    }

    // ---- editing ----

    /// Type or paste `s`, replacing the selection. Newlines become spaces. Returns whether the text changed.
    pub fn insert(&mut self, s: &str) -> bool {
        let range = self.selection();
        self.replace_range(range, s, Edit::Typing)
    }

    /// Replace an arbitrary range, as an input method does. The caret ends after the new text.
    pub fn replace(&mut self, range: Range<usize>, s: &str) -> bool {
        let r = self.snap(range.start)..self.snap(range.end);
        self.replace_range(r, s, Edit::Other)
    }

    fn replace_range(&mut self, range: Range<usize>, s: &str, kind: Edit) -> bool {
        let mut ins: String = sanitize(s);
        if let Some(f) = self.accept {
            ins.retain(f);
        }
        if let Some(max) = self.max_chars {
            let kept = self.text.chars().count() - self.text[range.clone()].chars().count();
            let room = max.saturating_sub(kept);
            ins = ins.chars().take(room).collect();
        }
        if ins.is_empty() && range.is_empty() {
            return false;
        }
        // A paste of several characters is its own undo step; single characters coalesce.
        let kind = if kind == Edit::Typing && (ins.chars().count() != 1 || !range.is_empty()) { Edit::Other } else { kind };
        self.record(kind);
        self.text.replace_range(range.clone(), &ins);
        self.head = range.start + ins.len();
        self.anchor = self.head;
        true
    }

    pub fn backspace(&mut self) -> bool {
        if self.has_selection() {
            return self.delete_selection(Edit::Other);
        }
        let start = self.prev_boundary(self.head);
        self.delete_range(start..self.head, Edit::Backspace)
    }

    pub fn delete(&mut self) -> bool {
        if self.has_selection() {
            return self.delete_selection(Edit::Other);
        }
        let end = self.next_boundary(self.head);
        self.delete_range(self.head..end, Edit::Delete)
    }

    pub fn delete_word_left(&mut self) -> bool {
        if self.has_selection() {
            return self.delete_selection(Edit::Other);
        }
        let start = self.word_start_before(self.head);
        self.delete_range(start..self.head, Edit::Other)
    }

    pub fn delete_word_right(&mut self) -> bool {
        if self.has_selection() {
            return self.delete_selection(Edit::Other);
        }
        let end = self.word_end_after(self.head);
        self.delete_range(self.head..end, Edit::Other)
    }

    fn delete_selection(&mut self, kind: Edit) -> bool {
        let r = self.selection();
        self.delete_range(r, kind)
    }

    fn delete_range(&mut self, range: Range<usize>, kind: Edit) -> bool {
        if range.is_empty() {
            return false;
        }
        self.record(kind);
        self.text.replace_range(range.clone(), "");
        self.head = range.start;
        self.anchor = self.head;
        true
    }

    /// Text for the clipboard, if anything is selected.
    pub fn copy(&self) -> Option<String> {
        self.has_selection().then(|| self.selected_text().to_string())
    }

    pub fn cut(&mut self) -> Option<String> {
        let t = self.copy()?;
        self.delete_selection(Edit::Other);
        Some(t)
    }

    // ---- undo ----

    fn record(&mut self, kind: Edit) {
        let coalesce = kind != Edit::Other && self.last == Some(kind) && !self.has_selection();
        if !coalesce {
            self.undo.push(Snapshot { text: self.text.clone(), anchor: self.anchor, head: self.head });
            if self.undo.len() > UNDO_LIMIT {
                self.undo.remove(0);
            }
        }
        self.redo.clear();
        self.last = Some(kind);
    }

    pub fn undo(&mut self) -> bool {
        let Some(s) = self.undo.pop() else { return false };
        self.redo.push(Snapshot { text: self.text.clone(), anchor: self.anchor, head: self.head });
        self.restore(s);
        true
    }

    pub fn redo(&mut self) -> bool {
        let Some(s) = self.redo.pop() else { return false };
        self.undo.push(Snapshot { text: self.text.clone(), anchor: self.anchor, head: self.head });
        self.restore(s);
        true
    }

    fn restore(&mut self, s: Snapshot) {
        self.text = s.text;
        self.anchor = s.anchor;
        self.head = s.head;
        self.last = None;
    }

    // ---- boundaries ----

    fn snap(&self, pos: usize) -> usize {
        let pos = pos.min(self.text.len());
        if self.text.is_char_boundary(pos) && self.is_grapheme_boundary(pos) {
            return pos;
        }
        self.prev_boundary(pos)
    }

    fn is_grapheme_boundary(&self, pos: usize) -> bool {
        pos == 0 || pos == self.text.len() || self.text.grapheme_indices(true).any(|(i, _)| i == pos)
    }

    fn prev_boundary(&self, pos: usize) -> usize {
        self.text.grapheme_indices(true).map(|(i, _)| i).take_while(|&i| i < pos).last().unwrap_or(0)
    }

    fn next_boundary(&self, pos: usize) -> usize {
        self.text.grapheme_indices(true).map(|(i, _)| i).find(|&i| i > pos).unwrap_or(self.text.len())
    }

    /// Start of the word to the left of `pos`, skipping spaces and punctuation first.
    fn word_start_before(&self, pos: usize) -> usize {
        let mut result = 0;
        for (start, tok) in self.text.split_word_bound_indices() {
            if start >= pos {
                break;
            }
            if is_word(tok) {
                result = start;
            }
        }
        result
    }

    /// End of the word to the right of `pos`, skipping spaces and punctuation first.
    fn word_end_after(&self, pos: usize) -> usize {
        for (start, tok) in self.text.split_word_bound_indices() {
            let end = start + tok.len();
            if end > pos && is_word(tok) {
                return end;
            }
        }
        self.text.len()
    }
}

fn is_word(tok: &str) -> bool {
    tok.chars().any(char::is_alphanumeric)
}

fn sanitize(s: &str) -> String {
    s.replace("\r\n", " ").replace(['\n', '\r'], " ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn buf(s: &str) -> TextBuffer {
        TextBuffer::new(s)
    }

    #[test]
    fn typing_appends_at_the_caret() {
        let mut b = buf("");
        assert!(b.insert("a"));
        assert!(b.insert("b"));
        assert_eq!(b.text(), "ab");
        assert_eq!(b.cursor(), 2);
        b.move_left(false);
        b.insert("X");
        assert_eq!(b.text(), "aXb");
        assert_eq!(b.cursor(), 2);
    }

    #[test]
    fn inserting_nothing_changes_nothing() {
        let mut b = buf("abc");
        assert!(!b.insert(""));
        assert!(!b.can_undo());
    }

    #[test]
    fn typing_replaces_the_selection() {
        let mut b = buf("hello world");
        b.select_range(0..5);
        b.insert("J");
        assert_eq!(b.text(), "J world");
        assert!(!b.has_selection());
    }

    #[test]
    fn backspace_and_delete() {
        let mut b = buf("abc");
        assert!(b.backspace());
        assert_eq!(b.text(), "ab");
        b.home(false);
        assert!(!b.backspace(), "nothing before the caret");
        assert!(b.delete());
        assert_eq!(b.text(), "b");
        b.end(false);
        assert!(!b.delete(), "nothing after the caret");
    }

    #[test]
    fn selection_is_deleted_as_a_whole() {
        let mut b = buf("abcdef");
        b.select_range(1..4);
        assert!(b.backspace());
        assert_eq!(b.text(), "aef");
        assert_eq!(b.cursor(), 1);
        b.select_all();
        assert!(b.delete());
        assert!(b.is_empty());
    }

    #[test]
    fn arrows_collapse_a_selection_to_its_edge() {
        let mut b = buf("abcdef");
        b.select_range(1..4);
        b.move_left(false);
        assert_eq!((b.cursor(), b.has_selection()), (1, false));
        b.select_range(1..4);
        b.move_right(false);
        assert_eq!((b.cursor(), b.has_selection()), (4, false));
    }

    #[test]
    fn shift_arrows_extend_in_either_direction() {
        let mut b = buf("abcdef");
        b.set_cursor(3, false);
        b.move_right(true);
        b.move_right(true);
        assert_eq!(b.selection(), 3..5);
        b.move_left(true);
        b.move_left(true);
        b.move_left(true);
        assert_eq!(b.selection(), 2..3, "anchor stays at 3");
        assert_eq!(b.selected_text(), "c");
    }

    #[test]
    fn movement_stops_at_the_ends() {
        let mut b = buf("ab");
        b.home(false);
        b.move_left(false);
        assert_eq!(b.cursor(), 0);
        b.end(false);
        b.move_right(false);
        assert_eq!(b.cursor(), 2);
    }

    #[test]
    fn words() {
        let mut b = buf("find me, quickly");
        b.home(false);
        b.move_word_right(false);
        assert_eq!(b.cursor(), 4, "end of 'find'");
        b.move_word_right(false);
        assert_eq!(b.cursor(), 7, "end of 'me'");
        b.move_word_right(false);
        assert_eq!(b.cursor(), 16, "skips ', ' to the end of 'quickly'");
        b.move_word_left(false);
        assert_eq!(b.cursor(), 9, "start of 'quickly'");
        b.move_word_left(false);
        assert_eq!(b.cursor(), 5, "start of 'me'");
        b.move_word_left(false);
        b.move_word_left(false);
        assert_eq!(b.cursor(), 0);
    }

    #[test]
    fn delete_by_word() {
        let mut b = buf("one two three");
        assert!(b.delete_word_left());
        assert_eq!(b.text(), "one two ");
        assert!(b.delete_word_left());
        assert_eq!(b.text(), "one ");
        b.home(false);
        assert!(b.delete_word_right());
        assert_eq!(b.text(), " ");
    }

    #[test]
    fn double_click_selects_a_word() {
        let mut b = buf("GET /api/login now");
        b.select_word_at(1);
        assert_eq!(b.selected_text(), "GET");
        b.select_word_at(6);
        assert_eq!(b.selected_text(), "api");
        b.select_word_at(3);
        assert_eq!(b.selected_text(), " ", "the gap after GET");
        b.select_word_at(999);
        assert!(b.selected_text().is_empty() || b.selected_text() == "now");
    }

    #[test]
    fn graphemes_are_never_split() {
        let mut b = buf("e\u{301}a"); // e + combining acute, then a
        b.home(false);
        b.move_right(false);
        assert_eq!(b.cursor(), 3, "moves past the whole accented letter");
        b.move_left(false);
        assert_eq!(b.cursor(), 0);
        b.end(false);
        b.backspace();
        b.backspace();
        assert!(b.is_empty(), "the accent is removed with its letter");
        let mut e = buf("a\u{1F468}\u{200D}\u{1F469}b"); // family-style emoji joined by ZWJ
        e.set_cursor(2, false);
        assert_eq!(e.cursor(), 1, "a caret inside an emoji snaps back to its start");
    }

    #[test]
    fn positions_past_the_end_are_clamped() {
        let mut b = buf("abc");
        b.set_cursor(100, false);
        assert_eq!(b.cursor(), 3);
        b.select_range(0..100);
        assert_eq!(b.selected_text(), "abc");
    }

    #[test]
    fn newlines_become_spaces() {
        let mut b = buf("");
        b.insert("line1\r\nline2\nline3");
        assert_eq!(b.text(), "line1 line2 line3");
        assert_eq!(buf("a\nb").text(), "a b");
    }

    #[test]
    fn filter_and_length_limit() {
        let mut b = buf("").with_filter(|c| c.is_ascii_digit()).with_max_chars(4);
        b.insert("a1b2c3d4e5");
        assert_eq!(b.text(), "1234");
        assert!(!b.insert("9"), "full");
        b.select_range(0..2);
        b.insert("98765");
        assert_eq!(b.text(), "9834", "limit counts what the selection frees");
    }

    #[test]
    fn typing_in_a_row_is_one_undo_step() {
        let mut b = buf("");
        for c in ["h", "e", "l", "l", "o"] {
            b.insert(c);
        }
        assert!(b.undo());
        assert!(b.is_empty(), "all five letters undone at once");
        assert!(!b.undo());
        assert!(b.redo());
        assert_eq!(b.text(), "hello");
        assert!(!b.redo());
    }

    #[test]
    fn moving_the_caret_ends_the_typing_run() {
        let mut b = buf("");
        b.insert("a");
        b.insert("b");
        b.move_left(false);
        b.insert("X");
        assert_eq!(b.text(), "aXb");
        b.undo();
        assert_eq!(b.text(), "ab");
        b.undo();
        assert!(b.is_empty());
    }

    #[test]
    fn paste_cut_and_delete_are_separate_steps() {
        let mut b = buf("");
        b.insert("one");
        b.insert(" two three"); // a multi-character insert is its own step
        assert_eq!(b.text(), "one two three");
        b.undo();
        assert_eq!(b.text(), "one", "the paste is undone as one step, typing before it stays");
        b.select_range(0..3);
        assert_eq!(b.cut(), Some("one".to_string()));
        assert!(b.is_empty());
        b.undo();
        assert_eq!(b.text(), "one");
        assert_eq!(b.selection(), 0..3, "the selection comes back");
    }

    #[test]
    fn new_edits_clear_redo() {
        let mut b = buf("");
        b.insert("a");
        b.undo();
        assert!(b.can_redo());
        b.insert("b");
        assert!(!b.can_redo());
    }

    #[test]
    fn backspaces_in_a_row_coalesce() {
        let mut b = buf("abcd");
        b.backspace();
        b.backspace();
        assert_eq!(b.text(), "ab");
        b.undo();
        assert_eq!(b.text(), "abcd");
    }

    #[test]
    fn copy_needs_a_selection() {
        let mut b = buf("abc");
        assert_eq!(b.copy(), None);
        assert_eq!(b.cut(), None);
        b.select_all();
        assert_eq!(b.copy(), Some("abc".into()));
        assert_eq!(b.text(), "abc", "copy leaves the text alone");
    }

    #[test]
    fn replace_range_for_input_methods() {
        let mut b = buf("hello");
        assert!(b.replace(1..4, "ipp"));
        assert_eq!(b.text(), "hippo");
        assert_eq!(b.cursor(), 4);
        b.undo();
        assert_eq!(b.text(), "hello");
    }

    #[test]
    fn set_text_resets_history() {
        let mut b = buf("x");
        b.insert("y");
        b.set_text("fresh");
        assert_eq!(b.text(), "fresh");
        assert_eq!(b.cursor(), 5);
        assert!(!b.can_undo() && !b.can_redo());
    }

    #[test]
    fn undo_history_is_bounded() {
        let mut b = buf("");
        for i in 0..(UNDO_LIMIT + 50) {
            b.select_all();
            b.insert(&format!("{i}x"));
        }
        let mut n = 0;
        while b.undo() {
            n += 1;
        }
        assert_eq!(n, UNDO_LIMIT);
    }
}
