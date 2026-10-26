//! Numbers for a run in progress: totals, response-time percentiles, a time series for the charts, per-label
//! tables and the latest errors. Memory stays flat however many samples arrive: response times are counted
//! per millisecond value, the series is merged when it gets long, and the label and error lists are capped.

use crate::results::Sample;
use std::collections::{BTreeMap, HashMap, VecDeque};

/// Longest time series kept; when it is reached, neighbouring points are merged and the step doubles.
pub const MAX_POINTS: usize = 600;
/// Most labels tracked; further ones count under [`OTHER_LABELS`].
pub const MAX_LABELS: usize = 1000;
pub const OTHER_LABELS: &str = "(other labels)";
/// Most recent failed samples kept.
pub const MAX_ERRORS: usize = 200;
/// Response times longer than this are counted as this long.
const MAX_MS: u64 = 3_600_000;

/// Statistics of one set of samples (the whole run or one label).
#[derive(Clone, Debug, Default)]
pub struct Stats {
    pub count: u64,
    pub errors: u64,
    pub sum_ms: u64,
    pub min_ms: u64,
    pub max_ms: u64,
    pub bytes: u64,
    /// How many samples took each number of milliseconds.
    times: BTreeMap<u32, u64>,
}

impl Stats {
    fn add(&mut self, s: &Sample) {
        let ms = s.elapsed_ms.min(MAX_MS);
        if self.count == 0 || ms < self.min_ms {
            self.min_ms = ms;
        }
        self.max_ms = self.max_ms.max(ms);
        self.count += 1;
        self.sum_ms += ms;
        self.bytes += s.bytes;
        if !s.success {
            self.errors += 1;
        }
        *self.times.entry(ms as u32).or_insert(0) += 1;
    }

    pub fn avg_ms(&self) -> u64 {
        if self.count == 0 { 0 } else { (self.sum_ms as f64 / self.count as f64).round() as u64 }
    }

    pub fn error_pct(&self) -> f64 {
        if self.count == 0 { 0.0 } else { self.errors as f64 * 100.0 / self.count as f64 }
    }

    /// The response time that `pct` percent of the samples are not slower than (nearest rank).
    pub fn percentile(&self, pct: f64) -> u64 {
        if self.count == 0 {
            return 0;
        }
        let rank = ((pct / 100.0) * self.count as f64).ceil().clamp(1.0, self.count as f64) as u64;
        let mut seen = 0;
        for (ms, n) in &self.times {
            seen += n;
            if seen >= rank {
                return *ms as u64;
            }
        }
        self.max_ms
    }
}

/// One step of the time series.
#[derive(Clone, Debug, PartialEq)]
pub struct Point {
    /// Seconds from the first sample to the start of this step.
    pub at_secs: u32,
    pub samples: u32,
    pub errors: u32,
    pub sum_ms: u64,
    /// Most users that were active in the step.
    pub threads: u32,
}

impl Point {
    pub fn avg_ms(&self) -> f64 {
        if self.samples == 0 { 0.0 } else { self.sum_ms as f64 / self.samples as f64 }
    }
}

/// A failed sample for the error list.
#[derive(Clone, Debug, PartialEq)]
pub struct ErrorRow {
    pub label: String,
    pub code: String,
    pub message: String,
    pub elapsed_ms: u64,
    pub thread: String,
}

#[derive(Clone, Debug)]
pub struct LiveStats {
    total: Stats,
    labels: HashMap<String, Stats>,
    series: Vec<Point>,
    step_secs: u32,
    errors: VecDeque<ErrorRow>,
    first_ms: Option<u64>,
    last_ms: u64,
    threads: u32,
    /// The most users seen in any sample.
    peak_threads: u32,
}

impl Default for LiveStats {
    fn default() -> Self {
        LiveStats { total: Stats::default(), labels: HashMap::new(), series: Vec::new(), step_secs: 1, errors: VecDeque::new(), first_ms: None, last_ms: 0, threads: 0, peak_threads: 0 }
    }
}

impl LiveStats {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_all(&mut self, rows: &[Sample]) {
        for r in rows {
            self.add(r);
        }
    }

    pub fn add(&mut self, s: &Sample) {
        self.total.add(s);
        let key = if self.labels.contains_key(&s.label) || self.labels.len() < MAX_LABELS { s.label.as_str() } else { OTHER_LABELS };
        self.labels.entry(key.to_string()).or_default().add(s);

        let first = *self.first_ms.get_or_insert(s.timestamp_ms);
        let end = s.timestamp_ms.saturating_add(s.elapsed_ms);
        self.last_ms = self.last_ms.max(end);
        self.threads = s.all_threads;
        self.peak_threads = self.peak_threads.max(s.all_threads);

        // A sample belongs to the step in which it finished.
        let at = (end.saturating_sub(first) / 1000) as u32;
        let step_start = at / self.step_secs * self.step_secs;
        if self.series.last().is_none_or(|p| p.at_secs < step_start) {
            self.series.push(Point { at_secs: step_start, samples: 0, errors: 0, sum_ms: 0, threads: 0 });
        }
        // Rows can arrive slightly out of order; put the sample in the step it belongs to.
        let idx = self.series.iter().rposition(|p| p.at_secs <= step_start).unwrap_or(0);
        let p = &mut self.series[idx];
        p.samples += 1;
        p.sum_ms += s.elapsed_ms.min(MAX_MS);
        p.threads = p.threads.max(s.all_threads);
        if !s.success {
            p.errors += 1;
            if self.errors.len() == MAX_ERRORS {
                self.errors.pop_front();
            }
            self.errors.push_back(ErrorRow { label: s.label.clone(), code: s.response_code.clone(), message: s.response_message.clone(), elapsed_ms: s.elapsed_ms, thread: s.thread_name.clone() });
        }
        if self.series.len() > MAX_POINTS {
            self.merge();
        }
    }

    fn merge(&mut self) {
        self.step_secs *= 2;
        let mut merged: Vec<Point> = Vec::with_capacity(self.series.len() / 2 + 1);
        for p in self.series.drain(..) {
            let start = p.at_secs / self.step_secs * self.step_secs;
            match merged.last_mut() {
                Some(m) if m.at_secs == start => {
                    m.samples += p.samples;
                    m.errors += p.errors;
                    m.sum_ms += p.sum_ms;
                    m.threads = m.threads.max(p.threads);
                }
                _ => merged.push(Point { at_secs: start, ..p }),
            }
        }
        self.series = merged;
    }

    pub fn total(&self) -> &Stats {
        &self.total
    }

    /// Seconds from the start of the first sample to the end of the last one.
    pub fn span_secs(&self) -> f64 {
        self.first_ms.map_or(0.0, |f| self.last_ms.saturating_sub(f) as f64 / 1000.0)
    }

    /// Samples per second over the whole run so far.
    pub fn throughput(&self) -> f64 {
        let s = self.span_secs();
        if s <= 0.0 { self.total.count as f64 } else { self.total.count as f64 / s }
    }

    /// Users active when the latest sample was written.
    pub fn threads(&self) -> u32 {
        self.threads
    }

    /// The most users seen in any sample, which is what a finished run should show: at the end the users have gone.
    pub fn peak_threads(&self) -> u32 {
        self.peak_threads
    }

    pub fn series(&self) -> &[Point] {
        &self.series
    }

    /// Seconds covered by each point of [`series`](Self::series).
    pub fn step_secs(&self) -> u32 {
        self.step_secs
    }

    /// The latest failed samples, newest last.
    pub fn errors(&self) -> impl Iterator<Item = &ErrorRow> {
        self.errors.iter()
    }

    /// Per-label statistics, in label order.
    pub fn labels(&self) -> Vec<(&str, &Stats)> {
        let mut v: Vec<_> = self.labels.iter().map(|(k, s)| (k.as_str(), s)).collect();
        v.sort_by(|a, b| a.0.cmp(b.0));
        v
    }

    pub fn label(&self, name: &str) -> Option<&Stats> {
        self.labels.get(name)
    }

    /// Read a whole results file, for a run that has ended. Returns the numbers and how many damaged
    /// records were skipped. Memory stays flat however large the file is.
    pub fn from_file(path: &std::path::Path) -> anyhow::Result<(LiveStats, u64)> {
        let mut live = LiveStats::new();
        let mut tail = crate::results::JtlTail::new();
        loop {
            let rows = tail.poll(path)?;
            live.add_all(&rows);
            if !tail.has_more() {
                break;
            }
        }
        Ok((live, tail.skipped()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(label: &str, ts: u64, ms: u64, ok: bool, threads: u32) -> Sample {
        Sample {
            timestamp_ms: ts,
            elapsed_ms: ms,
            label: label.into(),
            response_code: if ok { "200".into() } else { "500".into() },
            response_message: if ok { "OK".into() } else { "Server Error".into() },
            thread_name: "T 1-1".into(),
            success: ok,
            bytes: 100,
            group_threads: threads,
            all_threads: threads,
            latency_ms: ms / 2,
            connect_ms: 1,
        }
    }

    #[test]
    fn the_users_of_a_run_are_the_most_seen_and_not_the_last() {
        let mut l = LiveStats::new();
        for (i, users) in [1u32, 5, 10, 7, 2, 1].into_iter().enumerate() {
            l.add(&sample("A", 1_000 + i as u64 * 500, 10, true, users));
        }
        assert_eq!((l.threads(), l.peak_threads()), (1, 10), "at the end the users have gone, the peak is what the run had");
        assert_eq!(LiveStats::new().peak_threads(), 0);
    }

    #[test]
    fn totals_are_exact() {
        let mut l = LiveStats::new();
        for (i, ms) in [100, 200, 300, 400].into_iter().enumerate() {
            l.add(&sample("A", 1_000 + i as u64 * 500, ms, i != 3, 2));
        }
        let t = l.total();
        assert_eq!((t.count, t.errors, t.min_ms, t.max_ms, t.avg_ms(), t.bytes), (4, 1, 100, 400, 250, 400));
        assert_eq!(t.error_pct(), 25.0);
    }

    #[test]
    fn percentiles_use_the_nearest_rank() {
        let mut l = LiveStats::new();
        for ms in 1..=100 {
            l.add(&sample("A", 0, ms, true, 1));
        }
        let t = l.total();
        assert_eq!((t.percentile(50.0), t.percentile(90.0), t.percentile(95.0), t.percentile(99.0), t.percentile(100.0)), (50, 90, 95, 99, 100));
        assert_eq!(Stats::default().percentile(95.0), 0, "no samples, no percentile");
    }

    #[test]
    fn one_sample_has_every_percentile() {
        let mut l = LiveStats::new();
        l.add(&sample("A", 0, 42, true, 1));
        assert_eq!((l.total().percentile(1.0), l.total().percentile(99.0)), (42, 42));
    }

    #[test]
    fn throughput_is_samples_over_the_time_covered() {
        let mut l = LiveStats::new();
        for i in 0..10 {
            l.add(&sample("A", 10_000 + i * 1000, 100, true, 1));
        }
        // The first starts at 10.0 s and the last ends at 19.1 s: ten samples in 9.1 s.
        assert!((l.span_secs() - 9.1).abs() < 1e-9);
        assert!((l.throughput() - 10.0 / 9.1).abs() < 1e-9);
    }

    #[test]
    fn labels_are_kept_apart_and_capped() {
        let mut l = LiveStats::new();
        l.add(&sample("login", 0, 10, true, 1));
        l.add(&sample("login", 0, 30, true, 1));
        l.add(&sample("cart", 0, 50, false, 1));
        assert_eq!(l.label("login").unwrap().avg_ms(), 20);
        assert_eq!(l.label("cart").unwrap().errors, 1);
        assert_eq!(l.labels().iter().map(|x| x.0).collect::<Vec<_>>(), ["cart", "login"]);

        let mut many = LiveStats::new();
        for i in 0..MAX_LABELS + 500 {
            many.add(&sample(&format!("label {i}"), 0, 1, true, 1));
        }
        assert_eq!(many.labels().len(), MAX_LABELS + 1);
        assert_eq!(many.label(OTHER_LABELS).unwrap().count, 500);
        assert_eq!(many.total().count as usize, MAX_LABELS + 500, "the total still counts everything");
    }

    #[test]
    fn the_series_groups_samples_by_the_second_they_finished() {
        let mut l = LiveStats::new();
        l.add(&sample("A", 0, 100, true, 1)); // ends at 0.1 s
        l.add(&sample("A", 500, 100, true, 2)); // ends at 0.6 s
        l.add(&sample("A", 1_000, 100, false, 3)); // ends at 1.1 s
        l.add(&sample("A", 4_000, 100, true, 4)); // ends at 4.1 s: seconds 2 and 3 have no points
        let s = l.series();
        assert_eq!(s.iter().map(|p| (p.at_secs, p.samples, p.errors, p.threads)).collect::<Vec<_>>(), [(0, 2, 0, 2), (1, 1, 1, 3), (4, 1, 0, 4)]);
        assert_eq!(s[0].avg_ms(), 100.0);
    }

    #[test]
    fn a_late_row_goes_into_the_step_it_belongs_to() {
        let mut l = LiveStats::new();
        l.add(&sample("A", 0, 100, true, 1));
        l.add(&sample("A", 2_000, 100, true, 1));
        l.add(&sample("A", 100, 100, true, 1)); // arrives late
        assert_eq!(l.series().iter().map(|p| (p.at_secs, p.samples)).collect::<Vec<_>>(), [(0, 2), (2, 1)]);
    }

    #[test]
    fn a_long_run_merges_steps_instead_of_growing() {
        let mut l = LiveStats::new();
        for sec in 0..5_000u64 {
            l.add(&sample("A", sec * 1000, 10, sec % 10 != 0, 5));
        }
        assert!(l.series().len() <= MAX_POINTS, "{} points", l.series().len());
        assert!(l.step_secs() >= 8);
        assert_eq!(l.series().iter().map(|p| p.samples as u64).sum::<u64>(), 5_000, "merging loses no samples");
        assert_eq!(l.series().iter().map(|p| p.errors as u64).sum::<u64>(), 500);
        assert!(l.series().windows(2).all(|w| w[0].at_secs < w[1].at_secs), "points stay in order");
    }

    #[test]
    fn only_the_latest_errors_are_kept() {
        let mut l = LiveStats::new();
        for i in 0..MAX_ERRORS as u64 + 50 {
            l.add(&sample(&format!("e{i}"), i, 5, false, 1));
        }
        let e: Vec<_> = l.errors().collect();
        assert_eq!(e.len(), MAX_ERRORS);
        assert_eq!(e[0].label, "e50");
        assert_eq!(e.last().unwrap().label, format!("e{}", MAX_ERRORS + 49));
        assert_eq!((e[0].code.as_str(), e[0].message.as_str()), ("500", "Server Error"));
    }

    #[test]
    fn absurd_values_do_not_break_anything() {
        let mut l = LiveStats::new();
        l.add(&sample("A", u64::MAX - 5, u64::MAX, true, u32::MAX));
        l.add(&sample("A", 0, 0, true, 0));
        l.add(&sample("", 5, 5, true, 1));
        assert_eq!(l.total().count, 3);
        assert!(l.total().max_ms <= MAX_MS);
        let _ = (l.throughput(), l.span_secs(), l.total().percentile(95.0));
    }

    #[test]
    fn a_finished_results_file_is_read_whole() {
        let d = std::env::temp_dir().join(format!("jmeter-ui-live-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let p = d.join("r.jtl");
        let mut text = String::from("timeStamp,elapsed,label,responseCode,responseMessage,threadName,success,bytes,allThreads
");
        for i in 0..1000u64 {
            text.push_str(&format!("{},{},L{},200,OK,t,{},10,5
", 1000 + i * 10, 10 + i % 5, i % 3, i % 10 != 0));
        }
        text.push_str("garbage line without commas
");
        std::fs::write(&p, text).unwrap();
        let (live, skipped) = LiveStats::from_file(&p).unwrap();
        assert_eq!((live.total().count, live.total().errors, live.labels().len()), (1000, 100, 3));
        assert_eq!(skipped, 1, "the garbage line was skipped and counted");
        assert!(LiveStats::from_file(&d.join("missing.jtl")).unwrap().0.total().count == 0, "a missing file is an empty run");
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn empty_stats_are_all_zero() {
        let l = LiveStats::new();
        assert_eq!((l.total().count, l.total().avg_ms(), l.throughput(), l.span_secs(), l.threads()), (0, 0, 0.0, 0.0, 0));
        assert!(l.series().is_empty() && l.errors().next().is_none() && l.labels().is_empty());
    }

    #[test]
    fn a_million_samples_are_added_fast_and_in_bounded_memory() {
        let start = std::time::Instant::now();
        let mut l = LiveStats::new();
        for i in 0..1_000_000u64 {
            l.add(&sample(["a", "b", "c", "d"][(i % 4) as usize], i, 5 + (i * 7919) % 900, i % 50 != 0, 100));
        }
        let took = start.elapsed();
        assert_eq!(l.total().count, 1_000_000);
        assert_eq!(l.errors().count(), MAX_ERRORS);
        assert!(l.series().len() <= MAX_POINTS);
        eprintln!("1,000,000 samples added in {took:?}");
        let limit = if cfg!(debug_assertions) { 30 } else { 3 };
        assert!(took.as_secs() < limit, "took {took:?}");
    }
}
