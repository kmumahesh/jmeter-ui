//! Where the lines of a text start, so a view can find the line of a position, and the text of a line, without
//! scanning the whole text each time.

use std::ops::Range;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LineIndex {
    /// The byte offset where each line starts. Never empty: an empty text is one empty line.
    starts: Vec<usize>,
    len: usize,
}

impl LineIndex {
    pub fn new(text: &str) -> LineIndex {
        let mut starts = vec![0];
        starts.extend(text.bytes().enumerate().filter(|&(_, b)| b == b'\n').map(|(i, _)| i + 1));
        LineIndex { starts, len: text.len() }
    }

    /// The number of lines. A text that ends with a line break has an empty last line.
    pub fn count(&self) -> usize {
        self.starts.len()
    }

    /// Where `line` starts (clamped to the last line).
    pub fn start(&self, line: usize) -> usize {
        self.starts[line.min(self.starts.len() - 1)]
    }

    /// The bytes of `line` without its line break.
    pub fn range(&self, line: usize) -> Range<usize> {
        let line = line.min(self.starts.len() - 1);
        let start = self.starts[line];
        let end = self.starts.get(line + 1).map_or(self.len, |&next| next - 1);
        start..end
    }

    /// The line that holds the byte offset (an offset past the end is on the last line).
    pub fn line_of(&self, offset: usize) -> usize {
        match self.starts.binary_search(&offset) {
            Ok(i) => i,
            Err(i) => i - 1,
        }
    }

    /// The width in digits of the largest line number, for a gutter.
    pub fn digits(&self) -> usize {
        self.count().to_string().len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_text_is_one_empty_line() {
        let l = LineIndex::new("");
        assert_eq!((l.count(), l.range(0), l.line_of(0), l.digits()), (1, 0..0, 0, 1));
    }

    #[test]
    fn lines_are_found_by_number_and_by_offset() {
        let t = "ab\ncde\n\nf";
        let l = LineIndex::new(t);
        assert_eq!(l.count(), 4);
        assert_eq!(&t[l.range(0)], "ab");
        assert_eq!(&t[l.range(1)], "cde");
        assert_eq!(&t[l.range(2)], "");
        assert_eq!(&t[l.range(3)], "f");
        for (offset, line) in [(0, 0), (1, 0), (2, 0), (3, 1), (5, 1), (6, 1), (7, 2), (8, 3), (9, 3), (500, 3)] {
            assert_eq!(l.line_of(offset), line, "offset {offset}");
        }
    }

    #[test]
    fn a_final_line_break_starts_an_empty_line() {
        let t = "a\n";
        let l = LineIndex::new(t);
        assert_eq!(l.count(), 2);
        assert_eq!(l.range(1), 2..2);
        assert_eq!(l.line_of(2), 1);
    }

    #[test]
    fn lines_past_the_end_are_the_last_line() {
        let l = LineIndex::new("a\nb");
        assert_eq!(l.range(99), l.range(1));
        assert_eq!(l.start(99), 2);
    }

    #[test]
    fn digits_follow_the_line_count() {
        assert_eq!(LineIndex::new(&"x\n".repeat(8)).digits(), 1);
        assert_eq!(LineIndex::new(&"x\n".repeat(9)).digits(), 2, "ten lines");
        assert_eq!(LineIndex::new(&"x\n".repeat(99)).digits(), 3, "a hundred lines");
    }

    #[test]
    fn many_lines_are_indexed_quickly() {
        let text = "some text on a line\n".repeat(500_000);
        let t = std::time::Instant::now();
        let l = LineIndex::new(&text);
        assert_eq!(l.count(), 500_001);
        for i in (0..text.len()).step_by(997) {
            let _ = l.line_of(i);
        }
        assert!(t.elapsed() < std::time::Duration::from_secs(3), "{:?}", t.elapsed());
    }
}
