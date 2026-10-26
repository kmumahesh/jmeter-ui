//! Comparing two runs: for each request the numbers of a baseline run next to those of a candidate run, how much each
//! changed, and whether the change is one to worry about.
//!
//! A change only counts when it is large in both ways: a share of the baseline and an amount of milliseconds, so that a
//! request that went from 2 ms to 3 ms is not reported as 50% slower.

use crate::live::{LiveStats, Stats};
use std::collections::BTreeSet;

/// A response time must change by at least this share of the baseline to count.
pub const SHARE: f64 = 0.10;
/// ...and by at least this many milliseconds.
pub const MIN_MS: u64 = 5;
/// An error rate must rise by at least this many percentage points to count.
pub const ERROR_POINTS: f64 = 1.0;

/// The numbers of one request in one run.
#[derive(Clone, Debug, PartialEq)]
pub struct Metrics {
    pub count: u64,
    pub avg_ms: u64,
    pub median_ms: u64,
    pub p90_ms: u64,
    pub p95_ms: u64,
    pub p99_ms: u64,
    pub error_pct: f64,
    pub per_sec: f64,
}

impl Metrics {
    fn of(s: &Stats) -> Metrics {
        Metrics { count: s.count, avg_ms: s.avg_ms(), median_ms: s.median_ms(), p90_ms: s.percentile(90.0), p95_ms: s.percentile(95.0), p99_ms: s.percentile(99.0), error_pct: s.error_pct(), per_sec: s.throughput() }
    }
}

/// What a change means.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// Slower, or failing more, beyond the limits.
    Worse,
    /// Faster, or failing less, beyond the limits.
    Better,
    /// Not different enough to matter.
    Same,
    /// Only in the candidate.
    New,
    /// Only in the baseline.
    Gone,
}

/// One request in both runs.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub label: String,
    pub base: Option<Metrics>,
    pub cand: Option<Metrics>,
}

/// How much `cand` is more (positive) or less (negative) than `base`, in percent; `None` when the baseline is zero.
pub fn change_pct(base: f64, cand: f64) -> Option<f64> {
    if base == 0.0 || !base.is_finite() || !cand.is_finite() { None } else { Some((cand - base) * 100.0 / base) }
}

fn time_verdict(base: u64, cand: u64) -> Verdict {
    let diff = cand.abs_diff(base);
    if diff >= MIN_MS && diff as f64 >= base as f64 * SHARE {
        if cand > base { Verdict::Worse } else { Verdict::Better }
    } else {
        Verdict::Same
    }
}

impl Row {
    /// Whether this request got worse, better or stayed as it was: its 95th percentile and its error rate decide, and the
    /// worse of the two wins.
    pub fn verdict(&self) -> Verdict {
        match (&self.base, &self.cand) {
            (None, Some(_)) => Verdict::New,
            (Some(_), None) => Verdict::Gone,
            (None, None) => Verdict::Same,
            (Some(b), Some(c)) => {
                let t = time_verdict(b.p95_ms, c.p95_ms);
                let e = if c.error_pct - b.error_pct >= ERROR_POINTS {
                    Verdict::Worse
                } else if b.error_pct - c.error_pct >= ERROR_POINTS {
                    Verdict::Better
                } else {
                    Verdict::Same
                };
                match (t, e) {
                    (Verdict::Worse, _) | (_, Verdict::Worse) => Verdict::Worse,
                    (Verdict::Better, _) | (_, Verdict::Better) => Verdict::Better,
                    _ => Verdict::Same,
                }
            }
        }
    }

    /// How much slower the 95th percentile got, in percent, for ordering the worst first (requests in one run only come last).
    pub fn p95_change(&self) -> Option<f64> {
        match (&self.base, &self.cand) {
            (Some(b), Some(c)) => change_pct(b.p95_ms as f64, c.p95_ms as f64),
            _ => None,
        }
    }
}

/// Two runs side by side.
#[derive(Clone, Debug, PartialEq)]
pub struct Comparison {
    pub rows: Vec<Row>,
    pub total: Row,
}

impl Comparison {
    /// Compare `cand` with `base`: a row for every request in either, in name order, and a total.
    pub fn of(base: &LiveStats, cand: &LiveStats) -> Comparison {
        let names: BTreeSet<&str> = base.labels().iter().map(|(n, _)| *n).chain(cand.labels().iter().map(|(n, _)| *n)).collect();
        let rows = names
            .into_iter()
            .map(|n| Row { label: n.to_string(), base: base.label(n).map(Metrics::of), cand: cand.label(n).map(Metrics::of) })
            .collect();
        let total = Row { label: "Total".into(), base: Some(Metrics::of(base.total())).filter(|m| m.count > 0), cand: Some(Metrics::of(cand.total())).filter(|m| m.count > 0) };
        Comparison { rows, total }
    }

    /// The rows with the worst change first: requests that got worse by the most, then the others by name, then those in one run only.
    pub fn worst_first(&self) -> Vec<&Row> {
        let mut v: Vec<&Row> = self.rows.iter().collect();
        v.sort_by(|a, b| {
            let key = |r: &Row| match (r.verdict(), r.p95_change()) {
                (Verdict::Worse, c) => (0, -(c.unwrap_or(f64::MAX))),
                (Verdict::New | Verdict::Gone, _) => (3, 0.0),
                (Verdict::Same, _) => (2, 0.0),
                (Verdict::Better, c) => (1, c.unwrap_or(0.0)),
            };
            let (ka, kb) = (key(a), key(b));
            ka.0.cmp(&kb.0).then(ka.1.partial_cmp(&kb.1).unwrap_or(std::cmp::Ordering::Equal)).then(a.label.cmp(&b.label))
        });
        v
    }

    pub fn count(&self, v: Verdict) -> usize {
        self.rows.iter().filter(|r| r.verdict() == v).count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::results::Sample;

    fn sample(label: &str, ts: u64, ms: u64, ok: bool) -> Sample {
        Sample { timestamp_ms: ts, elapsed_ms: ms, label: label.into(), response_code: "200".into(), response_message: "OK".into(), thread_name: "t".into(), success: ok, bytes: 100, group_threads: 1, all_threads: 1, latency_ms: 1, connect_ms: 1 }
    }

    fn run(rows: &[(&str, u64, usize, usize)]) -> LiveStats {
        // (label, response ms, samples, failures)
        let mut l = LiveStats::new();
        let mut t = 1_000;
        for (label, ms, n, bad) in rows {
            for i in 0..*n {
                l.add(&sample(label, t, *ms, i >= *bad));
                t += 10;
            }
        }
        l
    }

    #[test]
    fn changes_are_in_percent_of_the_baseline_and_never_divide_by_zero() {
        assert_eq!(change_pct(100.0, 150.0), Some(50.0));
        assert_eq!(change_pct(200.0, 100.0), Some(-50.0));
        assert_eq!(change_pct(0.0, 5.0), None);
        assert_eq!(change_pct(f64::NAN, 5.0), None);
        assert_eq!(change_pct(5.0, f64::INFINITY), None);
        assert_eq!(change_pct(100.0, 100.0), Some(0.0));
    }

    #[test]
    fn a_change_counts_only_when_it_is_big_in_share_and_in_milliseconds() {
        let v = |b, c| time_verdict(b, c);
        assert_eq!(v(2, 3), Verdict::Same, "50% of nothing");
        assert_eq!(v(1000, 1040), Verdict::Same, "4% of a lot");
        assert_eq!(v(1000, 1100), Verdict::Worse, "exactly the share and 100 ms");
        assert_eq!(v(1000, 900), Verdict::Better);
        assert_eq!(v(100, 105), Verdict::Same, "5 ms but only 5%");
        assert_eq!(v(40, 45), Verdict::Worse, "5 ms and 12% reaches both limits");
        assert_eq!(v(40, 44), Verdict::Same, "4 ms is under the limit");
    }

    #[test]
    fn every_request_in_either_run_is_listed_with_what_happened_to_it() {
        let base = run(&[("Home", 100, 50, 0), ("Search", 100, 50, 0), ("Old", 100, 50, 0), ("Steady", 100, 50, 0)]);
        let cand = run(&[("Home", 300, 50, 0), ("Search", 50, 50, 0), ("New", 100, 50, 0), ("Steady", 102, 50, 0)]);
        let c = Comparison::of(&base, &cand);
        let verdicts: Vec<(&str, Verdict)> = c.rows.iter().map(|r| (r.label.as_str(), r.verdict())).collect();
        assert_eq!(verdicts, [("Home", Verdict::Worse), ("New", Verdict::New), ("Old", Verdict::Gone), ("Search", Verdict::Better), ("Steady", Verdict::Same)]);
        assert_eq!((c.count(Verdict::Worse), c.count(Verdict::Better), c.count(Verdict::New), c.count(Verdict::Gone)), (1, 1, 1, 1));
        assert_eq!(c.total.label, "Total");
        assert!(c.total.base.is_some() && c.total.cand.is_some());
    }

    #[test]
    fn more_failures_make_a_request_worse_even_when_it_is_as_fast() {
        let base = run(&[("A", 100, 100, 0)]);
        let cand = run(&[("A", 100, 100, 5)]);
        let c = Comparison::of(&base, &cand);
        assert_eq!(c.rows[0].verdict(), Verdict::Worse);
        let c = Comparison::of(&cand, &base);
        assert_eq!(c.rows[0].verdict(), Verdict::Better, "fewer failures is better");
        // Less than a point of difference does not count.
        let slight = run(&[("A", 100, 1000, 5)]);
        assert_eq!(Comparison::of(&base, &slight).rows[0].verdict(), Verdict::Same);
    }

    #[test]
    fn the_worst_comes_first_then_the_better_ones_then_the_rest() {
        let base = run(&[("a", 100, 20, 0), ("b", 100, 20, 0), ("c", 100, 20, 0), ("d", 100, 20, 0), ("e", 100, 20, 0), ("gone", 100, 20, 0)]);
        let cand = run(&[("a", 150, 20, 0), ("b", 400, 20, 0), ("c", 50, 20, 0), ("d", 100, 20, 0), ("e", 10, 20, 0), ("new", 1, 20, 0)]);
        let c = Comparison::of(&base, &cand);
        let order: Vec<&str> = c.worst_first().iter().map(|r| r.label.as_str()).collect();
        assert_eq!(order, ["b", "a", "e", "c", "d", "gone", "new"], "worst, then the most improved, then unchanged, then in one run only");
    }

    #[test]
    fn empty_runs_compare_without_trouble() {
        let c = Comparison::of(&LiveStats::new(), &LiveStats::new());
        assert!(c.rows.is_empty());
        assert_eq!((c.total.base.clone(), c.total.cand.clone()), (None, None));
        let one = run(&[("A", 10, 5, 0)]);
        let c = Comparison::of(&LiveStats::new(), &one);
        assert_eq!(c.rows[0].verdict(), Verdict::New);
        assert!(c.total.base.is_none() && c.total.cand.is_some());
    }

    #[test]
    fn two_runs_of_a_thousand_requests_compare_quickly() {
        let rows: Vec<(String, u64)> = (0..1000).map(|i| (format!("Request {i:04}"), 50 + i % 90)).collect();
        let mk = |factor: u64| {
            let mut l = LiveStats::new();
            for (i, (name, ms)) in rows.iter().enumerate() {
                for k in 0..50u64 {
                    l.add(&sample(name, 1_000 + i as u64 * 50 + k, ms * factor, true));
                }
            }
            l
        };
        let (a, b) = (mk(1), mk(2));
        let t = std::time::Instant::now();
        let c = Comparison::of(&a, &b);
        let w = c.worst_first().len();
        eprintln!("compared 1,000 requests in {:?}", t.elapsed());
        assert_eq!((c.rows.len(), w), (1000, 1000));
        assert_eq!(c.count(Verdict::Worse), 1000);
        assert!(t.elapsed() < std::time::Duration::from_secs(2));
    }
}
