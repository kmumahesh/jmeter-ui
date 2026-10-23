//! Fuzzy matching for the palette and element search.

use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher, Utf32Str};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Match {
    /// Index into the candidate list.
    pub index: usize,
    pub score: u32,
    /// Character positions in the candidate that matched, ascending, for highlighting.
    pub positions: Vec<u32>,
}

pub struct Fuzzy {
    matcher: Matcher,
}

impl Default for Fuzzy {
    fn default() -> Self {
        Self::new()
    }
}

impl Fuzzy {
    pub fn new() -> Self {
        Fuzzy { matcher: Matcher::new(Config::DEFAULT) }
    }

    /// Candidates that match `query`, best first; ties keep their original order. An empty query
    /// matches everything in the original order with score 0. Lower-case queries ignore case.
    pub fn rank(&mut self, query: &str, candidates: &[&str]) -> Vec<Match> {
        let query = query.trim();
        if query.is_empty() {
            return candidates.iter().enumerate().map(|(index, _)| Match { index, score: 0, positions: vec![] }).collect();
        }
        let pattern = Pattern::parse(query, CaseMatching::Smart, Normalization::Smart);
        let mut out = Vec::new();
        let mut buf = Vec::new();
        for (index, text) in candidates.iter().enumerate() {
            let mut positions = Vec::new();
            if let Some(score) = pattern.indices(Utf32Str::new(text, &mut buf), &mut self.matcher, &mut positions) {
                positions.sort_unstable();
                positions.dedup();
                out.push(Match { index, score, positions });
            }
        }
        out.sort_by(|a, b| b.score.cmp(&a.score).then(a.index.cmp(&b.index)));
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ITEMS: [&str; 5] = ["JSON Extractor", "Regular Expression Extractor", "Response Assertion", "HTTP Request", "Loop Controller"];

    #[test]
    fn empty_query_keeps_everything_in_order() {
        let m = Fuzzy::new().rank("  ", &ITEMS);
        assert_eq!(m.iter().map(|m| m.index).collect::<Vec<_>>(), [0, 1, 2, 3, 4]);
    }

    #[test]
    fn matches_subsequences_and_reports_positions() {
        let m = Fuzzy::new().rank("jsex", &ITEMS);
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].index, 0);
        assert_eq!(m[0].positions.len(), 4);
        assert!(m[0].positions.windows(2).all(|w| w[0] < w[1]));
    }

    #[test]
    fn better_matches_rank_first() {
        let m = Fuzzy::new().rank("ext", &ITEMS);
        let idx: Vec<_> = m.iter().map(|m| m.index).collect();
        assert!(idx.contains(&0) && idx.contains(&1));
        assert!(!idx.contains(&3), "HTTP Request has no e-x-t");
    }

    #[test]
    fn lower_case_ignores_case_and_upper_case_does_not() {
        let mut f = Fuzzy::new();
        assert_eq!(f.rank("http", &ITEMS).len(), 1);
        assert_eq!(f.rank("HTTP", &ITEMS).len(), 1);
        let m = f.rank("Request", &["request", "Request"]);
        assert_eq!(m.iter().map(|m| m.index).collect::<Vec<_>>(), [1], "an upper-case letter makes the match case-sensitive");
        assert!(f.rank("Zz", &ITEMS).is_empty());
    }

    #[test]
    fn exact_prefix_beats_scattered_match() {
        let m = Fuzzy::new().rank("loop", &["Interleave Controller Pool", "Loop Controller"]);
        assert_eq!(m[0].index, 1);
    }

    #[test]
    fn ties_keep_input_order() {
        let m = Fuzzy::new().rank("a", &["a", "a", "a"]);
        assert_eq!(m.iter().map(|m| m.index).collect::<Vec<_>>(), [0, 1, 2]);
    }
}
