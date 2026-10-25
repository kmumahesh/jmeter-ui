//! The editing logic behind text fields and text areas, independent of any UI toolkit.
//!
//! Positions are byte offsets that always sit on a grapheme boundary, so an accented letter or an
//! emoji is never split. Typing in a row is one undo step. A buffer is one line (new lines typed or pasted become
//! spaces) unless it is made with `multiline`, which keeps lines and adds moving up and down, per-line Home and
//! End, and indenting. Characters that cannot be stored in a plan file (most control characters) are dropped.

use std::ops::Range;
use unicode_segmentation::{GraphemeCursor, UnicodeSegmentation};

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
    multiline: bool,
    /// The column an up or down move aims for, kept while moving along lines of different lengths.
    goal_col: Option<usize>,
    /// Counts changes of the text, so a view can keep what it worked out until the text changes.
    version: u64,
}

impl Default for TextBuffer {
    fn default() -> Self {
        TextBuffer::new("")
    }
}

const UNDO_LIMIT: usize = 200;

/// The most text kept in the undo history. Snapshots are whole copies, so a big text keeps fewer steps.
const UNDO_BYTES: usize = 32 * 1024 * 1024;

/// The columns of an indent.
const INDENT: &str = "  ";

impl TextBuffer {
    pub fn new(text: &str) -> TextBuffer {
        TextBuffer::make(text, false)
    }

    /// A buffer that keeps its lines.
    pub fn multiline(text: &str) -> TextBuffer {
        TextBuffer::make(text, true)
    }

    fn make(text: &str, multiline: bool) -> TextBuffer {
        let text = sanitize(text, multiline);
        let end = text.len();
        TextBuffer { text, anchor: end, head: end, undo: vec![], redo: vec![], last: None, max_chars: None, accept: None, multiline, goal_col: None, version: 0 }
    }

    pub fn is_multiline(&self) -> bool {
        self.multiline
    }

    /// Changes whenever the text does.
    pub fn version(&self) -> u64 {
        self.version
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
        self.text = sanitize(text, self.multiline);
        self.head = self.text.len();
        self.anchor = self.head;
        self.undo.clear();
        self.redo.clear();
        self.last = None;
        self.goal_col = None;
        self.version += 1;
    }

    // ---- caret and selection ----

    /// Put the caret at `pos` (clamped to a grapheme boundary). With `extend` the selection grows.
    pub fn set_cursor(&mut self, pos: usize, extend: bool) {
        self.head = self.snap(pos);
        if !extend {
            self.anchor = self.head;
        }
        self.last = None;
        self.goal_col = None;
    }

    pub fn select_all(&mut self) {
        self.anchor = 0;
        self.head = self.text.len();
        self.last = None;
        self.goal_col = None;
    }

    pub fn select_range(&mut self, range: Range<usize>) {
        self.anchor = self.snap(range.start);
        self.head = self.snap(range.end);
        self.last = None;
        self.goal_col = None;
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

    /// To the start of the text, or of the line in a multiline buffer.
    pub fn home(&mut self, extend: bool) {
        let to = if self.multiline { self.line_start(self.head) } else { 0 };
        self.set_cursor(to, extend);
    }

    /// To the end of the text, or of the line in a multiline buffer.
    pub fn end(&mut self, extend: bool) {
        let to = if self.multiline { self.line_end(self.head) } else { self.text.len() };
        self.set_cursor(to, extend);
    }

    pub fn doc_start(&mut self, extend: bool) {
        self.set_cursor(0, extend);
    }

    pub fn doc_end(&mut self, extend: bool) {
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
        // A reversed or out-of-range request from outside is made sound rather than trusted.
        let (a, b) = (self.snap(range.start), self.snap(range.end));
        self.replace_range(a.min(b)..a.max(b), s, Edit::Other)
    }

    fn replace_range(&mut self, range: Range<usize>, s: &str, kind: Edit) -> bool {
        let mut ins: String = sanitize(s, self.multiline);
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
        self.goal_col = None;
        self.version += 1;
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
        self.goal_col = None;
        self.version += 1;
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
            // A big text keeps fewer steps: the oldest go first, but the latest step always stays.
            let mut bytes: usize = self.undo.iter().map(|s| s.text.len()).sum();
            while bytes > UNDO_BYTES && self.undo.len() > 1 {
                bytes -= self.undo.remove(0).text.len();
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
        self.goal_col = None;
        self.version += 1;
    }

    // ---- boundaries ----

    /// The nearest character boundary at or before `pos` (and not past the end).
    fn floor_char(&self, pos: usize) -> usize {
        let mut pos = pos.min(self.text.len());
        while !self.text.is_char_boundary(pos) {
            pos -= 1;
        }
        pos
    }

    fn snap(&self, pos: usize) -> usize {
        let pos = self.floor_char(pos);
        if self.is_grapheme_boundary(pos) { pos } else { self.prev_boundary(pos) }
    }

    fn is_grapheme_boundary(&self, pos: usize) -> bool {
        pos == 0 || pos == self.text.len() || GraphemeCursor::new(pos, self.text.len(), true).is_boundary(&self.text, 0).unwrap_or(true)
    }

    fn prev_boundary(&self, pos: usize) -> usize {
        let pos = self.floor_char(pos);
        if pos == 0 {
            return 0;
        }
        GraphemeCursor::new(pos, self.text.len(), true).prev_boundary(&self.text, 0).ok().flatten().unwrap_or(0)
    }

    fn next_boundary(&self, pos: usize) -> usize {
        let pos = self.floor_char(pos);
        if pos >= self.text.len() {
            return self.text.len();
        }
        GraphemeCursor::new(pos, self.text.len(), true).next_boundary(&self.text, 0).ok().flatten().unwrap_or(self.text.len())
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

    // ---- lines (multiline buffers) ----

    /// The start of the line that holds `pos`.
    pub fn line_start(&self, pos: usize) -> usize {
        let pos = self.floor_char(pos);
        self.text[..pos].rfind('\n').map_or(0, |i| i + 1)
    }

    /// The end of the line that holds `pos`, before its line break.
    pub fn line_end(&self, pos: usize) -> usize {
        let pos = self.floor_char(pos);
        self.text[pos..].find('\n').map_or(self.text.len(), |i| pos + i)
    }

    /// The number of lines (an empty text is one line; a final line break starts another).
    pub fn line_count(&self) -> usize {
        self.text.bytes().filter(|&b| b == b'\n').count() + 1
    }

    /// The zero-based line and column (in characters the caret passes) of `pos`.
    pub fn line_col(&self, pos: usize) -> (usize, usize) {
        let pos = self.snap(pos);
        let line = self.text[..pos].bytes().filter(|&b| b == b'\n').count();
        let col = self.text[self.line_start(pos)..pos].graphemes(true).count();
        (line, col)
    }

    fn move_to_column(&self, line_start: usize, col: usize) -> usize {
        let end = self.line_end(line_start);
        let mut at = line_start;
        for (i, g) in self.text[line_start..end].grapheme_indices(true).take(col) {
            at = line_start + i + g.len();
        }
        at.min(end)
    }

    /// Move the caret `lines` lines (negative is up), keeping the column it aimed for. Moving up from the first
    /// line goes to the start of the text, and down from the last line to its end, as in other editors.
    pub fn move_lines(&mut self, lines: isize, extend: bool) {
        if !self.multiline || lines == 0 {
            return;
        }
        let goal = self.goal_col.unwrap_or_else(|| self.text[self.line_start(self.head)..self.head].graphemes(true).count());
        let mut start = self.line_start(self.head);
        let mut hit_edge = false;
        if lines < 0 {
            for _ in 0..lines.unsigned_abs() {
                if start == 0 {
                    hit_edge = true;
                    break;
                }
                start = self.line_start(start - 1);
            }
        } else {
            for _ in 0..lines.unsigned_abs() {
                let end = self.line_end(start);
                if end >= self.text.len() {
                    hit_edge = true;
                    break;
                }
                start = end + 1;
            }
        }
        let to = if hit_edge && lines < 0 && start == 0 && self.line_start(self.head) == 0 {
            0
        } else if hit_edge && lines > 0 && self.line_end(start) >= self.text.len() && self.line_end(self.head) >= self.text.len() {
            self.text.len()
        } else {
            self.move_to_column(start, goal)
        };
        self.head = self.snap(to);
        if !extend {
            self.anchor = self.head;
        }
        self.last = None;
        self.goal_col = Some(goal);
    }

    pub fn move_up(&mut self, extend: bool) {
        self.move_lines(-1, extend);
    }

    pub fn move_down(&mut self, extend: bool) {
        self.move_lines(1, extend);
    }

    /// Select the whole line that holds `pos`, with its line break.
    pub fn select_line_at(&mut self, pos: usize) {
        let start = self.line_start(pos);
        let end = (self.line_end(pos) + 1).min(self.text.len());
        self.select_range(start..end);
    }

    /// Start a new line, with the indentation of the current line (one level deeper after `{`, `[` or `(`).
    pub fn insert_newline(&mut self) -> bool {
        if !self.multiline {
            return false;
        }
        let sel = self.selection();
        let start = self.line_start(sel.start);
        let indent: String = self.text[start..sel.start].chars().take_while(|c| *c == ' ' || *c == '\t').collect();
        let deeper = self.text[..sel.start].trim_end_matches([' ', '\t']).ends_with(['{', '[', '(']);
        let mut ins = String::from("\n");
        ins.push_str(&indent);
        if deeper {
            ins.push_str(INDENT);
        }
        self.replace_range(sel, &ins, Edit::Other)
    }

    /// Put an indent at the caret, or in front of every line a selection touches.
    pub fn indent(&mut self) -> bool {
        if !self.multiline {
            return false;
        }
        let sel = self.selection();
        if self.text[sel.clone()].contains('\n') {
            return self.shift_lines(sel, true);
        }
        let col = self.text[self.line_start(sel.start)..sel.start].chars().count();
        let spaces = INDENT.len() - col % INDENT.len();
        self.replace_range(sel, &" ".repeat(spaces), Edit::Other)
    }

    /// Take one indent off the start of the caret's line, or of every line a selection touches.
    pub fn outdent(&mut self) -> bool {
        if !self.multiline {
            return false;
        }
        let sel = self.selection();
        self.shift_lines(sel, false)
    }

    fn shift_lines(&mut self, sel: Range<usize>, deeper: bool) -> bool {
        let first = self.line_start(sel.start);
        let last_end = self.line_end(if sel.end > sel.start && self.text.as_bytes().get(sel.end - 1) == Some(&b'\n') { sel.end - 1 } else { sel.end });
        let block = &self.text[first..last_end];
        let mut out = String::with_capacity(block.len() + 16);
        for (i, line) in block.split('\n').enumerate() {
            if i > 0 {
                out.push('\n');
            }
            if deeper {
                if !line.is_empty() {
                    out.push_str(INDENT);
                }
                out.push_str(line);
            } else {
                let strip = if line.starts_with('\t') {
                    1
                } else {
                    line.bytes().take(INDENT.len()).take_while(|&b| b == b' ').count()
                };
                out.push_str(&line[strip..]);
            }
        }
        if out == block {
            return false;
        }
        self.record(Edit::Other);
        self.text.replace_range(first..last_end, &out);
        self.anchor = first;
        self.head = first + out.len();
        self.goal_col = None;
        self.version += 1;
        true
    }
}

fn is_word(tok: &str) -> bool {
    tok.chars().any(char::is_alphanumeric)
}

/// The text as a one-line box shows it: line breaks become spaces and characters a plan file cannot hold are
/// dropped. A box whose stored text differs from this one cannot show it whole.
pub fn single_line(s: &str) -> String {
    sanitize(s, false)
}

/// Characters a plan file can hold (XML 1.0): tab, line breaks, and everything from space up except the
/// surrogate range and the two non-characters at the end of the first plane.
fn is_storable(c: char) -> bool {
    matches!(c, '\u{9}' | '\u{A}' | '\u{D}' | '\u{20}'..='\u{D7FF}' | '\u{E000}'..='\u{FFFD}' | '\u{10000}'..='\u{10FFFF}')
}

/// Line breaks become `\n` in a multiline buffer and a space otherwise; characters that cannot be stored are
/// dropped.
fn sanitize(s: &str, multiline: bool) -> String {
    let break_as = if multiline { '\n' } else { ' ' };
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\r' => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
                out.push(break_as);
            }
            '\n' => out.push(break_as),
            c if is_storable(c) => out.push(c),
            _ => {}
        }
    }
    out
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

#[cfg(test)]
mod multiline_tests {
    use super::*;

    fn ml(s: &str) -> TextBuffer {
        TextBuffer::multiline(s)
    }

    // ---- what is kept and what is dropped ----

    #[test]
    fn a_multiline_buffer_keeps_its_lines_and_unifies_line_breaks() {
        assert_eq!(ml("a\nb").text(), "a\nb");
        assert_eq!(ml("a\r\nb\rc").text(), "a\nb\nc");
        let mut b = ml("");
        b.insert("one\r\ntwo\n\nthree");
        assert_eq!(b.text(), "one\ntwo\n\nthree");
        assert_eq!(TextBuffer::new("a\r\nb").text(), "a b", "a one-line buffer still turns breaks into spaces");
    }

    #[test]
    fn characters_a_plan_file_cannot_hold_are_dropped() {
        for bad in ['\u{0}', '\u{1}', '\u{8}', '\u{b}', '\u{c}', '\u{e}', '\u{1f}', '\u{FFFE}', '\u{FFFF}'] {
            let text = format!("a{bad}b");
            assert_eq!(TextBuffer::new(&text).text(), "ab", "{bad:?} one line");
            assert_eq!(ml(&text).text(), "ab", "{bad:?} many lines");
            let mut b = ml("");
            b.insert(&text);
            assert_eq!(b.text(), "ab", "{bad:?} typed");
            let mut b = TextBuffer::new("x");
            b.replace(0..1, &text);
            assert_eq!(b.text(), "ab", "{bad:?} replaced");
        }
        assert_eq!(ml("a\tb").text(), "a\tb", "tabs are fine");
        assert_eq!(TextBuffer::new("a\tb").text(), "a\tb");
        assert_eq!(ml("caf\u{e9} \u{1F600} \u{7f}").text(), "caf\u{e9} \u{1F600} \u{7f}", "ordinary and unusual characters stay");
    }

    // ---- lines ----

    #[test]
    fn lines_are_counted_and_found() {
        let b = ml("ab\ncde\n\nf");
        assert_eq!(b.line_count(), 4);
        assert_eq!((b.line_start(4), b.line_end(4)), (3, 6));
        assert_eq!((b.line_start(7), b.line_end(7)), (7, 7), "an empty line");
        assert_eq!(b.line_col(5), (1, 2));
        assert_eq!(b.line_col(0), (0, 0));
        assert_eq!(b.line_col(999), (3, 1), "past the end is the end");
        assert_eq!(ml("").line_count(), 1);
        assert_eq!(ml("a\n").line_count(), 2);
    }

    #[test]
    fn columns_count_whole_characters() {
        let b = ml("\u{1F600}\u{1F600}x\ny");
        assert_eq!(b.line_col(b.text().find('x').unwrap()), (0, 2));
        assert_eq!(b.line_col(b.text().find('y').unwrap()), (1, 0));
    }

    // ---- moving ----

    #[test]
    fn up_and_down_keep_the_column_across_short_lines() {
        let mut b = ml("long line here\nab\nanother long one");
        b.set_cursor(10, false); // column 10 of the first line
        b.move_down(false);
        assert_eq!(b.line_col(b.cursor()), (1, 2), "the short line ends the move");
        b.move_down(false);
        assert_eq!(b.line_col(b.cursor()), (2, 10), "and the column comes back on the long one");
        b.move_up(false);
        b.move_up(false);
        assert_eq!(b.line_col(b.cursor()), (0, 10));
    }

    #[test]
    fn up_from_the_first_line_goes_to_the_start_and_down_from_the_last_to_the_end() {
        let mut b = ml("abc\ndef");
        b.set_cursor(2, false);
        b.move_up(false);
        assert_eq!(b.cursor(), 0);
        b.set_cursor(5, false);
        b.move_down(false);
        assert_eq!(b.cursor(), 7);
        b.move_down(false);
        assert_eq!(b.cursor(), 7, "and stays there");
        let mut one = ml("single");
        one.set_cursor(3, false);
        one.move_up(false);
        assert_eq!(one.cursor(), 0);
        one.move_down(false);
        assert_eq!(one.cursor(), 6);
    }

    #[test]
    fn shift_with_up_and_down_selects_lines() {
        let mut b = ml("one\ntwo\nthree");
        b.set_cursor(1, false);
        b.move_down(true);
        b.move_down(true);
        assert_eq!(b.selected_text(), "ne\ntwo\nt");
        b.move_up(true);
        assert_eq!(b.selected_text(), "ne\nt");
        b.move_up(false);
        assert!(!b.has_selection());
    }

    #[test]
    fn moving_many_lines_at_once_for_page_up_and_down() {
        let text: String = (0..100).map(|i| format!("line {i}\n")).collect();
        let mut b = ml(&text);
        b.set_cursor(0, false);
        b.move_lines(30, false);
        assert_eq!(b.line_col(b.cursor()).0, 30);
        b.move_lines(-12, false);
        assert_eq!(b.line_col(b.cursor()).0, 18);
        b.move_lines(1000, false);
        assert_eq!(b.cursor(), b.text().len());
        b.move_lines(-1000, false);
        assert_eq!(b.cursor(), 0);
    }

    #[test]
    fn up_and_down_do_nothing_in_a_one_line_buffer() {
        let mut b = TextBuffer::new("abc");
        b.set_cursor(1, false);
        b.move_up(false);
        b.move_down(false);
        assert_eq!(b.cursor(), 1);
    }

    #[test]
    fn home_and_end_work_per_line_in_a_multiline_buffer() {
        let mut b = ml("first\nsecond line\nthird");
        b.set_cursor(9, false);
        b.home(false);
        assert_eq!(b.cursor(), 6);
        b.end(false);
        assert_eq!(b.cursor(), 17);
        b.home(true);
        assert_eq!(b.selected_text(), "second line");
        b.doc_start(false);
        assert_eq!(b.cursor(), 0);
        b.doc_end(true);
        assert_eq!(b.selection(), 0..b.text().len());
        let mut one = TextBuffer::new("abc def");
        one.set_cursor(3, false);
        one.home(false);
        assert_eq!(one.cursor(), 0);
        one.end(false);
        assert_eq!(one.cursor(), 7);
    }

    #[test]
    fn a_triple_click_selects_the_whole_line() {
        let mut b = ml("one\ntwo\nthree");
        b.select_line_at(5);
        assert_eq!(b.selected_text(), "two\n");
        b.select_line_at(12);
        assert_eq!(b.selected_text(), "three");
    }

    // ---- new lines and indents ----

    #[test]
    fn enter_copies_the_indent_and_goes_one_deeper_after_an_opening_bracket() {
        let mut b = ml("");
        b.insert("  {");
        assert!(b.insert_newline());
        assert_eq!(b.text(), "  {\n    ", "two spaces more after the brace");
        b.insert("\"a\": 1");
        b.insert_newline();
        assert_eq!(b.text(), "  {\n    \"a\": 1\n    ", "plain lines keep their indent");
        b.undo();
        assert_eq!(b.text(), "  {\n    \"a\": 1", "a new line is its own undo step");
    }

    #[test]
    fn enter_replaces_a_selection_and_works_in_the_middle_of_a_line() {
        let mut b = ml("  abXYcd");
        b.select_range(4..6);
        b.insert_newline();
        assert_eq!(b.text(), "  ab\n  cd");
        assert_eq!(b.cursor(), 7);
        let mut one = TextBuffer::new("abc");
        assert!(!one.insert_newline(), "a one-line buffer has no new lines");
        assert_eq!(one.text(), "abc");
    }

    #[test]
    fn tab_moves_to_the_next_stop_and_shift_tab_takes_an_indent_off() {
        let mut b = ml("x");
        b.doc_start(false);
        b.indent();
        assert_eq!(b.text(), "  x");
        b.doc_end(false);
        b.indent();
        assert_eq!(b.text(), "  x ", "from column 3 the next stop of two is one space away");
        b.outdent();
        assert_eq!(b.text(), "x ");
        assert!(!b.outdent(), "nothing left to take off");
    }

    #[test]
    fn tab_and_shift_tab_shift_every_line_of_a_selection() {
        let mut b = ml("a\n  b\n\nc");
        b.select_range(0..b.text().len());
        assert!(b.indent());
        assert_eq!(b.text(), "  a\n    b\n\n  c", "empty lines stay empty");
        assert!(b.outdent());
        assert_eq!(b.text(), "a\n  b\n\nc");
        b.undo();
        assert_eq!(b.text(), "  a\n    b\n\n  c");
        b.undo();
        assert_eq!(b.text(), "a\n  b\n\nc", "each shift is one undo step");
    }

    #[test]
    fn a_selection_that_ends_after_a_line_break_does_not_shift_the_next_line() {
        let mut b = ml("a\nb\nc");
        b.select_range(0..4); // "a\nb\n"
        b.indent();
        assert_eq!(b.text(), "  a\n  b\nc");
    }

    #[test]
    fn indenting_a_one_line_buffer_does_nothing() {
        let mut b = TextBuffer::new("abc");
        assert!(!b.indent() && !b.outdent());
        assert_eq!(b.text(), "abc");
    }

    // ---- bookkeeping ----

    #[test]
    fn the_version_changes_with_the_text_and_not_with_the_caret() {
        let mut b = ml("abc");
        let v = b.version();
        b.move_left(false);
        b.home(true);
        b.select_all();
        assert_eq!(b.version(), v);
        b.insert("x");
        assert_ne!(b.version(), v);
        let v2 = b.version();
        b.undo();
        assert_ne!(b.version(), v2);
        let v3 = b.version();
        b.set_text("fresh");
        assert_ne!(b.version(), v3);
    }

    #[test]
    fn a_big_text_keeps_fewer_undo_steps_but_always_the_latest() {
        let mut b = ml(&"x".repeat(3 * 1024 * 1024));
        for i in 0..40 {
            b.select_range(0..1);
            b.insert(&format!("{}", i % 10));
        }
        let mut steps = 0;
        while b.undo() {
            steps += 1;
        }
        assert!(steps >= 1, "the latest step is always there");
        assert!(steps <= 12, "{steps} steps of 3 MB each exceed the budget");
    }

    // ---- speed ----

    #[test]
    fn moving_through_a_big_text_is_quick() {
        let text = "some words on a line of text, \u{e9}\u{1F600} and more\n".repeat(20_000); // about 1 MB
        let mut b = ml(&text);
        b.set_cursor(text.len() / 2, false);
        let t = std::time::Instant::now();
        for _ in 0..20_000 {
            b.move_right(false);
        }
        for _ in 0..20_000 {
            b.move_left(false);
        }
        for _ in 0..2000 {
            b.move_down(false);
            b.move_up(false);
        }
        assert!(t.elapsed() < std::time::Duration::from_secs(5), "{:?}", t.elapsed());
    }

    #[test]
    fn typing_into_a_big_text_is_quick() {
        let text = "x".repeat(1024 * 1024);
        let mut b = ml(&text);
        b.set_cursor(500_000, false);
        let t = std::time::Instant::now();
        for _ in 0..300 {
            b.insert("a");
            b.insert_newline();
            b.backspace();
        }
        assert!(t.elapsed() < std::time::Duration::from_secs(10), "{:?}", t.elapsed());
    }

    #[test]
    fn a_huge_paste_is_kept_whole_and_cleaned_in_one_pass() {
        let mut b = ml("");
        let line = "{\"k\": \"v\"}\r\n\u{0}";
        let paste = line.repeat(100_000);
        let t = std::time::Instant::now();
        b.insert(&paste);
        assert!(t.elapsed() < std::time::Duration::from_secs(3), "{:?}", t.elapsed());
        assert_eq!(b.line_count(), 100_001);
        assert!(!b.text().contains('\r') && !b.text().contains('\u{0}'));
    }

    // ---- whatever is done, the buffer stays sound ----

    #[test]
    fn random_operations_keep_the_buffer_sound_and_undo_gives_everything_back() {
        let mut seed = 0xC0FFEEu64;
        let mut next = move |n: usize| {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (seed >> 33) as usize % n
        };
        let pieces = ["a", "bc", "\n", "\r\n", "  ", "\t", "{", "}", "\u{e9}", "\u{1F600}", "e\u{301}", "\u{0}", "line one\nline two", "${v}"];
        for round in 0..30 {
            let mut b = ml(if round % 3 == 0 { "start\n  indented\n" } else { "" });
            let first = b.text().to_string();
            for _ in 0..300 {
                match next(22) {
                    0 => {
                        b.insert(pieces[next(pieces.len())]);
                    }
                    1 => {
                        b.insert_newline();
                    }
                    2 => {
                        b.backspace();
                    }
                    3 => {
                        b.delete();
                    }
                    4 => b.move_left(next(2) == 0),
                    5 => b.move_right(next(2) == 0),
                    6 => b.move_up(next(2) == 0),
                    7 => b.move_down(next(2) == 0),
                    8 => b.home(next(2) == 0),
                    9 => b.end(next(2) == 0),
                    10 => b.doc_start(next(2) == 0),
                    11 => b.doc_end(next(2) == 0),
                    12 => b.move_word_left(next(2) == 0),
                    13 => b.move_word_right(next(2) == 0),
                    14 => {
                        b.indent();
                    }
                    15 => {
                        b.outdent();
                    }
                    16 => {
                        let n = b.text().len();
                        let (x, y) = (next(n + 2), next(n + 2));
                        b.select_range(x.min(y)..x.max(y));
                    }
                    17 => b.select_line_at(next(b.text().len() + 2)),
                    18 => {
                        b.delete_word_left();
                    }
                    19 => {
                        let n = b.text().len();
                        b.replace(next(n + 1)..next(n + 1), pieces[next(pieces.len())]);
                    }
                    20 => {
                        b.cut();
                    }
                    _ => {
                        b.move_lines(next(7) as isize - 3, false);
                    }
                }
                let t = b.text();
                assert!(b.cursor() <= t.len() && t.is_char_boundary(b.cursor()), "caret off the text: {:?}", t);
                let sel = b.selection();
                assert!(sel.end <= t.len() && t.is_char_boundary(sel.start) && t.is_char_boundary(sel.end));
                assert!(!t.contains('\r') && t.chars().all(is_storable), "an unstorable character got in: {t:?}");
            }
            while b.undo() {}
            assert_eq!(b.text(), first, "round {round}: undoing everything gives the start back");
        }
    }
}
