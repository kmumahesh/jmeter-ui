//! The state of one run as the window shows it: a live run, a run that has ended, or a stored run read back
//! from its folder. This holds the numbers and moves through the steps of a run; drawing is in `views_run`.

use jvm_runtime::live::LiveStats;
use jvm_runtime::results::Summary;
use jvm_runtime::run::{RunEvent, RunHandle};
use jvm_runtime::runs::{self, Outcome, RunEntry, RunKind, RunMeta};
use std::collections::VecDeque;
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// How long a graceful stop may take before the window offers to end JMeter by force.
pub const GRACE: Duration = Duration::from_secs(15);
/// Most output lines kept for the log.
pub const MAX_LOG_LINES: usize = 300;
/// Most events taken from the run in one go, so the window stays responsive during a flood.
const EVENTS_PER_POLL: usize = 400;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Phase {
    /// JMeter was started and has not reported yet.
    Starting,
    Running,
    /// Stop was pressed; waiting for JMeter to finish.
    Stopping,
    /// A stored run is being read from its folder.
    Loading,
    Finished,
    Stopped,
    Failed,
}

impl Phase {
    /// The process is still going.
    pub fn active(self) -> bool {
        matches!(self, Phase::Starting | Phase::Running | Phase::Stopping)
    }

    pub fn label(self) -> &'static str {
        match self {
            Phase::Starting => "Starting",
            Phase::Running => "Running",
            Phase::Stopping => "Stopping",
            Phase::Loading => "Loading",
            Phase::Finished => "Finished",
            Phase::Stopped => "Stopped",
            Phase::Failed => "Failed",
        }
    }
}

pub struct RunSession {
    pub kind: RunKind,
    pub plan_path: PathBuf,
    pub plan_name: String,
    /// The folder of this run.
    pub dir: PathBuf,
    /// The `runs` folder it is in.
    pub root: PathBuf,
    pub meta: RunMeta,
    pub live: LiveStats,
    pub handle: Option<RunHandle>,
    pub phase: Phase,
    /// For a stored run being read: the phase to show when the numbers are in.
    stored_phase: Option<Phase>,
    pub started: Instant,
    pub ended: Option<Instant>,
    pub last_summary: Option<Summary>,
    /// The latest output lines of JMeter.
    pub log: VecDeque<String>,
    /// What was changed or limited for this run, shown in the header.
    pub notes: Vec<String>,
    pub stop_requested: Option<Instant>,
    pub exit_code: Option<i32>,
    /// A message when the run could not be started or read.
    pub error: Option<String>,
    /// Damaged records skipped when a stored run was read.
    pub skipped: u64,
}

impl RunSession {
    /// A run that was just started.
    pub fn live(kind: RunKind, plan_path: PathBuf, dir: PathBuf, root: PathBuf, meta: RunMeta, handle: RunHandle, notes: Vec<String>) -> RunSession {
        RunSession {
            kind,
            plan_name: meta.plan_name.clone(),
            plan_path,
            dir,
            root,
            meta,
            live: LiveStats::new(),
            handle: Some(handle),
            phase: Phase::Starting,
            stored_phase: None,
            started: Instant::now(),
            ended: None,
            last_summary: None,
            log: VecDeque::new(),
            notes,
            stop_requested: None,
            exit_code: None,
            error: None,
            skipped: 0,
        }
    }

    /// A run read back from its folder; its numbers come later with [`set_stored`](Self::set_stored).
    pub fn stored(entry: &RunEntry, root: PathBuf) -> RunSession {
        let final_phase = match entry.meta.outcome {
            Outcome::Finished | Outcome::Running => Phase::Finished,
            Outcome::Stopped => Phase::Stopped,
            Outcome::Failed => Phase::Failed,
        };
        RunSession {
            kind: entry.meta.kind,
            plan_path: PathBuf::from(&entry.meta.plan_path),
            plan_name: entry.meta.plan_name.clone(),
            dir: entry.dir.clone(),
            root,
            meta: entry.meta.clone(),
            live: LiveStats::new(),
            handle: None,
            phase: Phase::Loading,
            stored_phase: Some(final_phase),
            started: Instant::now(),
            ended: Some(Instant::now()),
            last_summary: None,
            log: VecDeque::new(),
            notes: entry.meta.notes.clone(),
            stop_requested: None,
            exit_code: entry.meta.exit_code,
            error: None,
            skipped: 0,
        }
    }

    /// The numbers of a stored run have been read.
    pub fn set_stored(&mut self, live: LiveStats, skipped: u64) {
        self.live = live;
        self.skipped = skipped;
        self.phase = self.stored_phase.take().unwrap_or(Phase::Finished);
    }

    pub fn set_stored_error(&mut self, message: String) {
        self.error = Some(message);
        self.phase = self.stored_phase.take().unwrap_or(Phase::Finished);
    }

    /// Time since the run started, or the length of a run that has ended.
    pub fn elapsed(&self) -> Duration {
        if let (Some(a), None) = (self.meta.ended_ms, self.handle.as_ref()) {
            return Duration::from_millis(a.saturating_sub(self.meta.started_ms));
        }
        self.ended.unwrap_or_else(Instant::now).duration_since(self.started)
    }

    /// Whether the window should offer to end JMeter by force: a stop was asked for and took too long.
    pub fn can_kill(&self) -> bool {
        self.phase == Phase::Stopping && self.stop_requested.is_some_and(|t| t.elapsed() >= GRACE)
    }

    /// Take in one thing JMeter reported.
    pub fn apply(&mut self, ev: RunEvent) {
        match ev {
            RunEvent::Started { .. } => {}
            RunEvent::StopPort(_) => self.running(),
            RunEvent::Summary(s) => {
                self.running();
                self.last_summary = Some(s);
            }
            RunEvent::Samples(rows) => {
                self.running();
                self.live.add_all(&rows);
            }
            RunEvent::Line(l) => {
                if let Some(h) = &self.handle {
                    h.lines_taken(1);
                }
                if self.log.len() == MAX_LOG_LINES {
                    self.log.pop_front();
                }
                self.log.push_back(l);
            }
            RunEvent::Exited { code } => self.exit_code = code,
        }
    }

    fn running(&mut self) {
        if self.phase == Phase::Starting {
            self.phase = Phase::Running;
        }
    }

    /// Take what JMeter has reported since the last call, and notice when the process has ended. Returns
    /// whether anything changed.
    pub fn poll(&mut self) -> bool {
        let Some(mut h) = self.handle.take() else { return false };
        let mut changed = false;
        for _ in 0..EVENTS_PER_POLL {
            match h.events.try_recv() {
                Ok(ev) => {
                    self.apply(ev);
                    changed = true;
                }
                Err(_) => break,
            }
        }
        if self.phase.active() && !h.is_running() {
            // Let the reader threads finish, then take the rest.
            let code = h.wait(Duration::from_secs(3)).ok().flatten();
            while let Ok(ev) = h.events.try_recv() {
                self.apply(ev);
            }
            self.finish(code.or(self.exit_code));
            changed = true;
        }
        self.handle = Some(h);
        changed
    }

    /// The process has ended: settle the outcome, write it to `run.json` and keep the handle for dropping.
    fn finish(&mut self, code: Option<i32>) {
        self.exit_code = code;
        self.ended = Some(Instant::now());
        self.phase = if self.stop_requested.is_some() {
            Phase::Stopped
        } else if code == Some(0) {
            Phase::Finished
        } else {
            Phase::Failed
        };
        if self.phase == Phase::Failed && self.error.is_none() {
            self.error = Some(self.failure_text());
        }
        let t = self.live.total();
        self.meta.ended_ms = Some(runs::now_ms());
        self.meta.outcome = match self.phase {
            Phase::Finished => Outcome::Finished,
            Phase::Stopped => Outcome::Stopped,
            _ => Outcome::Failed,
        };
        self.meta.exit_code = code;
        self.meta.samples = t.count;
        self.meta.errors = t.errors;
        self.meta.avg_ms = t.avg_ms();
        self.meta.p95_ms = t.percentile(95.0);
        self.meta.notes = self.notes.clone();
        let _ = runs::write_meta(&self.dir, &self.meta);
    }

    /// Why a run failed, in a sentence: the last lines JMeter printed that look like errors.
    fn failure_text(&self) -> String {
        let hints: Vec<&String> = self.log.iter().rev().filter(|l| l.contains("ERROR") || l.contains("Error") || l.contains("Exception") || l.contains("doesn't exist") || l.contains("can't be opened")).take(2).collect();
        let tail = if hints.is_empty() { String::new() } else { format!(": {}", hints.into_iter().rev().map(|s| s.trim().to_string()).collect::<Vec<_>>().join(" / ")) };
        match self.exit_code {
            Some(c) => format!("JMeter ended with code {c}{tail}"),
            None => format!("JMeter ended without a result{tail}"),
        }
    }

    /// Ask JMeter to stop. `now` interrupts requests in progress.
    pub fn request_stop(&mut self, now: bool) -> anyhow::Result<()> {
        if !self.phase.active() {
            return Ok(());
        }
        if let Some(h) = &self.handle {
            h.stop(now)?;
        }
        self.phase = Phase::Stopping;
        self.stop_requested.get_or_insert_with(Instant::now);
        Ok(())
    }

    /// End JMeter by force.
    pub fn kill(&mut self) {
        self.stop_requested.get_or_insert_with(Instant::now);
        if let Some(h) = &mut self.handle {
            let _ = h.kill();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jvm_runtime::results::Sample;
    use jvm_runtime::runs::RunMeta;

    fn session() -> RunSession {
        let dir = std::env::temp_dir().join(format!("jmeter-ui-session-{}-{:?}", std::process::id(), std::thread::current().id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let plan = dir.join("p.jmx");
        let meta = RunMeta::new(&plan, RunKind::Run, chrono_now());
        let entry = RunEntry { dir: dir.clone(), meta };
        let mut s = RunSession::stored(&entry, dir.clone());
        // Make it behave like a live run that has just started.
        s.phase = Phase::Starting;
        s.stored_phase = None;
        s
    }

    fn chrono_now() -> chrono_shim::Now {
        chrono_shim::now()
    }

    /// `RunMeta::new` wants a local time; the runs module builds one for us.
    mod chrono_shim {
        pub type Now = jvm_runtime::runs::LocalTime;
        pub fn now() -> Now {
            jvm_runtime::runs::local_now()
        }
    }

    fn sample(ok: bool) -> Sample {
        Sample { timestamp_ms: 1000, elapsed_ms: 10, label: "A".into(), response_code: "200".into(), response_message: "OK".into(), thread_name: "t".into(), success: ok, bytes: 1, group_threads: 1, all_threads: 1, latency_ms: 5, connect_ms: 1 }
    }

    #[test]
    fn a_run_starts_as_starting_and_becomes_running_on_the_first_report() {
        for ev in [RunEvent::StopPort(4000), RunEvent::Samples(vec![sample(true)])] {
            let mut s = session();
            assert_eq!(s.phase, Phase::Starting);
            s.apply(ev);
            assert_eq!(s.phase, Phase::Running);
        }
    }

    #[test]
    fn samples_feed_the_numbers_and_lines_the_log() {
        let mut s = session();
        s.apply(RunEvent::Samples(vec![sample(true), sample(false)]));
        s.apply(RunEvent::Line("hello".into()));
        assert_eq!((s.live.total().count, s.live.total().errors), (2, 1));
        assert_eq!(s.log.back().map(String::as_str), Some("hello"));
    }

    #[test]
    fn the_log_keeps_only_the_latest_lines() {
        let mut s = session();
        for i in 0..MAX_LOG_LINES + 50 {
            s.apply(RunEvent::Line(format!("line {i}")));
        }
        assert_eq!(s.log.len(), MAX_LOG_LINES);
        assert_eq!(s.log.front().map(String::as_str), Some("line 50"));
    }

    #[test]
    fn how_a_run_ends_decides_its_outcome() {
        let cases = [(None, Some(0), Phase::Finished, Outcome::Finished), (Some(()), Some(0), Phase::Stopped, Outcome::Stopped), (Some(()), Some(1), Phase::Stopped, Outcome::Stopped), (None, Some(1), Phase::Failed, Outcome::Failed), (None, None, Phase::Failed, Outcome::Failed)];
        for (stopped, code, phase, outcome) in cases {
            let mut s = session();
            s.apply(RunEvent::Samples(vec![sample(true), sample(true), sample(false)]));
            if stopped.is_some() {
                s.stop_requested = Some(Instant::now());
            }
            s.finish(code);
            assert_eq!((s.phase, s.meta.outcome), (phase, outcome), "stopped {stopped:?}, code {code:?}");
            assert_eq!((s.meta.samples, s.meta.errors, s.exit_code), (3, 1, code));
            // The outcome is written to run.json at once.
            let on_disk = runs::read_meta(&s.dir).unwrap();
            assert_eq!((on_disk.outcome, on_disk.samples), (outcome, 3));
            assert!(on_disk.ended_ms.is_some());
        }
    }

    #[test]
    fn a_failed_run_says_why_from_what_jmeter_printed() {
        let mut s = session();
        s.apply(RunEvent::Line("INFO starting".into()));
        s.apply(RunEvent::Line("An error occurred: The file x.jmx doesn't exist or can't be opened".into()));
        s.finish(Some(1));
        let e = s.error.unwrap();
        assert!(e.contains("code 1") && e.contains("doesn't exist"), "{e}");
        let mut s2 = session();
        s2.finish(None);
        assert!(s2.error.unwrap().contains("without a result"));
    }

    #[test]
    fn asking_to_stop_moves_to_stopping_and_force_is_offered_only_after_the_grace_time() {
        let mut s = session();
        s.apply(RunEvent::Samples(vec![sample(true)]));
        s.request_stop(false).unwrap();
        assert_eq!(s.phase, Phase::Stopping);
        assert!(!s.can_kill(), "not at once");
        s.stop_requested = Instant::now().checked_sub(GRACE + Duration::from_secs(1));
        assert!(s.can_kill());
        s.request_stop(true).unwrap();
        assert_eq!(s.phase, Phase::Stopping, "stopping again keeps the first time");
        assert!(s.can_kill());
    }

    #[test]
    fn stopping_something_that_is_not_running_does_nothing() {
        let mut s = session();
        s.finish(Some(0));
        s.request_stop(true).unwrap();
        assert_eq!(s.phase, Phase::Finished);
        assert!(!s.can_kill());
    }

    #[test]
    fn a_stored_run_takes_its_final_phase_once_its_numbers_are_in() {
        let dir = std::env::temp_dir().join(format!("jmeter-ui-stored-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for (outcome, want) in [(Outcome::Finished, Phase::Finished), (Outcome::Stopped, Phase::Stopped), (Outcome::Failed, Phase::Failed), (Outcome::Running, Phase::Finished)] {
            let mut meta = RunMeta::new(&dir.join("p.jmx"), RunKind::Run, chrono_now());
            meta.outcome = outcome;
            let mut s = RunSession::stored(&RunEntry { dir: dir.clone(), meta }, dir.clone());
            assert_eq!(s.phase, Phase::Loading);
            s.set_stored(LiveStats::new(), 3);
            assert_eq!((s.phase, s.skipped), (want, 3));
        }
        let mut s = RunSession::stored(&RunEntry { dir: dir.clone(), meta: RunMeta::new(&dir.join("p.jmx"), RunKind::Run, chrono_now()) }, dir.clone());
        s.set_stored_error("cannot read".into());
        assert_eq!((s.phase, s.error.as_deref()), (Phase::Finished, Some("cannot read")));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_stored_run_shows_the_length_it_had_not_the_time_since_it_was_opened() {
        let dir = std::env::temp_dir().join(format!("jmeter-ui-stored2-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut meta = RunMeta::new(&dir.join("p.jmx"), RunKind::Run, chrono_now());
        meta.ended_ms = Some(meta.started_ms + 65_000);
        meta.outcome = Outcome::Finished;
        let s = RunSession::stored(&RunEntry { dir: dir.clone(), meta }, dir.clone());
        assert_eq!(s.elapsed().as_secs(), 65);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn phases_know_whether_the_process_is_going() {
        for p in [Phase::Starting, Phase::Running, Phase::Stopping] {
            assert!(p.active(), "{p:?}");
        }
        for p in [Phase::Loading, Phase::Finished, Phase::Stopped, Phase::Failed] {
            assert!(!p.active(), "{p:?}");
        }
    }
}
