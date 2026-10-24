//! Starting a non-GUI JMeter run, streaming events, and stopping it.

use crate::locate::quiet;
use crate::results::{JtlTail, Sample, Summary, parse_stop_port, parse_summary};
use anyhow::{Context, Result, bail};
use std::io::BufReader;
use std::net::UdpSocket;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, atomic::{AtomicBool, AtomicUsize, Ordering}};
use std::thread::{self, JoinHandle};
use std::time::Duration;

pub struct RunSpec {
    pub java: PathBuf,
    pub jmeter_home: PathBuf,
    pub plan: PathBuf,
    pub run_dir: PathBuf,
    pub jvm_args: Vec<String>,
    /// Extra `-J` properties as `(name, value)`.
    pub props: Vec<(String, String)>,
    pub generate_report: bool,
}

#[derive(Debug)]
pub enum RunEvent {
    Started { pid: u32 },
    StopPort(u16),
    Summary(Summary),
    Samples(Vec<Sample>),
    /// Any other output line.
    Line(String),
    Exited { code: Option<i32> },
}

pub struct RunHandle {
    child: Child,
    pub events: Receiver<RunEvent>,
    stop_port: u16,
    done: Arc<AtomicBool>,
    threads: Vec<JoinHandle<()>>,
    /// Output lines waiting for the window, and how many were dropped because too many waited.
    pending_lines: Arc<AtomicUsize>,
    dropped_lines: Arc<AtomicUsize>,
}

/// Sends output lines, dropping them when the window is not keeping up.
#[derive(Clone)]
struct LineSink {
    tx: Sender<RunEvent>,
    pending: Arc<AtomicUsize>,
    dropped: Arc<AtomicUsize>,
}

impl LineSink {
    fn send(&self, mut line: String) {
        if self.pending.load(Ordering::Relaxed) >= MAX_PENDING_LINES {
            self.dropped.fetch_add(1, Ordering::Relaxed);
            return;
        }
        if line.chars().count() > MAX_LINE_CHARS {
            line = line.chars().take(MAX_LINE_CHARS).collect();
        }
        self.pending.fetch_add(1, Ordering::Relaxed);
        let _ = self.tx.send(RunEvent::Line(line));
    }
}

/// The most output lines held for the window at a time; further lines are dropped and counted.
pub const MAX_PENDING_LINES: usize = 5_000;
/// Longer output lines are cut.
pub const MAX_LINE_CHARS: usize = 8 * 1024;

/// Whether `name` can be used as the name of a `-J` property: letters, digits, `.`, `_` and `-`.
pub fn valid_property_name(name: &str) -> bool {
    !name.is_empty() && name.len() <= 200 && name.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

fn plain_text(s: &str) -> bool {
    !s.chars().any(|c| c.is_control())
}

/// Check everything that goes onto the command line before JMeter is started. Arguments are passed one by
/// one (never through a shell), so spaces, quotes and `&` in paths are safe; this refuses what could still
/// do harm: scripts that Windows would run through `cmd.exe`, options that are not memory or `-D` settings,
/// property names with odd characters, and control characters anywhere.
pub fn validate(spec: &RunSpec) -> Result<()> {
    let ext = spec.java.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    if matches!(ext.as_str(), "bat" | "cmd" | "com" | "ps1" | "vbs" | "js") {
        bail!("{} is a script, not a Java program", spec.java.display());
    }
    if !spec.java.is_file() {
        bail!("Java was not found at {}", spec.java.display());
    }
    if !spec.jmeter_home.join("bin").join("ApacheJMeter.jar").is_file() {
        bail!("{} does not hold bin/ApacheJMeter.jar", spec.jmeter_home.display());
    }
    if !spec.plan.is_file() {
        bail!("The plan {} does not exist", spec.plan.display());
    }
    for (what, p) in [("plan", &spec.plan), ("run folder", &spec.run_dir), ("Java", &spec.java), ("JMeter", &spec.jmeter_home)] {
        if !plain_text(&p.to_string_lossy()) {
            bail!("The {what} path has control characters");
        }
    }
    for a in &spec.jvm_args {
        let known = a.starts_with("-Xmx") || a.starts_with("-Xms") || a.starts_with("-Xss") || a.starts_with("-XX:") || a.starts_with("-D");
        if !known || !plain_text(a) || a.len() > 500 {
            bail!("Java option {a:?} is not allowed");
        }
    }
    for (k, v) in &spec.props {
        if !valid_property_name(k) {
            bail!("Property name {k:?} is not allowed");
        }
        if !plain_text(v) || v.len() > 2000 {
            bail!("The value of property {k} is not allowed");
        }
    }
    Ok(())
}

/// A warning for a path that JMeter on Windows may not be able to open. Java reads its command line in the
/// system code page, so letters outside it (Chinese, Cyrillic, emoji...) arrive damaged and the file "does not
/// exist". Plain accents and symbols are fine.
pub fn path_warning(path: &Path) -> Option<String> {
    if cfg!(windows) && path.to_string_lossy().chars().any(|c| (c as u32) > 0xFF) {
        return Some(format!("{} has letters that JMeter may not find on Windows. If the run fails to open the plan, move it to a folder with a plain name.", path.display()));
    }
    None
}

/// A path as an absolute path (without needing it to exist), so that a name starting with `-` can never be
/// taken for an option.
fn absolute(p: &Path) -> PathBuf {
    strip_verbatim(&std::path::absolute(p).unwrap_or_else(|_| p.to_path_buf()))
}

/// `\\?\C:\x` as `C:\x`: the long form that `canonicalize` gives on Windows is not understood by every
/// program, and Java is one of them.
pub fn strip_verbatim(p: &Path) -> PathBuf {
    let s = p.to_string_lossy();
    match s.strip_prefix(r"\\?\") {
        Some(rest) if !rest.starts_with("UNC\\") => PathBuf::from(rest),
        Some(rest) => PathBuf::from(format!(r"\\{}", &rest[4..])),
        None => p.to_path_buf(),
    }
}

impl Drop for RunHandle {
    /// Closing the app (or dropping the handle) never leaves JMeter running.
    fn drop(&mut self) {
        self.done.store(true, Ordering::Relaxed);
        if matches!(self.child.try_wait(), Ok(None)) {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
        for t in self.threads.drain(..) {
            let _ = t.join();
        }
    }
}

/// A UDP port that is free on loopback right now. JMeter only uses ports above 1000.
pub fn free_udp_port() -> Result<u16> {
    let s = UdpSocket::bind("127.0.0.1:0")?;
    Ok(s.local_addr()?.port())
}

pub fn start(spec: &RunSpec) -> Result<RunHandle> {
    let spec = &RunSpec {
        java: absolute(&spec.java),
        jmeter_home: absolute(&spec.jmeter_home),
        plan: absolute(&spec.plan),
        run_dir: absolute(&spec.run_dir),
        jvm_args: spec.jvm_args.clone(),
        props: spec.props.clone(),
        generate_report: spec.generate_report,
    };
    validate(spec)?;
    std::fs::create_dir_all(&spec.run_dir)?;
    let stop_port = free_udp_port()?;
    let results = spec.run_dir.join("results.jtl");
    let mut cmd = Command::new(&spec.java);
    cmd.args(&spec.jvm_args)
        .arg("-jar")
        .arg(spec.jmeter_home.join("bin").join("ApacheJMeter.jar"))
        .arg("-n")
        .arg("-t")
        .arg(&spec.plan)
        .arg("-l")
        .arg(&results)
        .arg("-j")
        .arg(spec.run_dir.join("jmeter.log"))
        .arg("-Jjmeter.save.saveservice.autoflush=true")
        .arg("-Jsummariser.name=summary")
        .arg("-Jsummariser.interval=1")
        .arg(format!("-Jjmeterengine.nongui.port={stop_port}"))
        .arg(format!("-Jjmeterengine.nongui.maxport={stop_port}"));
    for (k, v) in &spec.props {
        cmd.arg(format!("-J{k}={v}"));
    }
    if spec.generate_report {
        cmd.arg("-e").arg("-o").arg(spec.run_dir.join("report"));
    }
    cmd.current_dir(&spec.run_dir).stdout(Stdio::piped()).stderr(Stdio::piped()).stdin(Stdio::null());
    quiet(&mut cmd);
    let mut child = cmd.spawn().with_context(|| format!("cannot start {}", spec.java.display()))?;
    // If this fails the run still works; JMeter is then only ended by the app, not by Windows.
    let _tied = crate::job::tie_to_this_process(&child);

    let (tx, rx) = channel();
    let done = Arc::new(AtomicBool::new(false));
    let _ = tx.send(RunEvent::Started { pid: child.id() });
    let mut threads = Vec::new();

    let stdout = child.stdout.take().unwrap();
    let pending_lines = Arc::new(AtomicUsize::new(0));
    let dropped_lines = Arc::new(AtomicUsize::new(0));
    let sink = LineSink { tx: tx.clone(), pending: pending_lines.clone(), dropped: dropped_lines.clone() };
    let t = tx.clone();
    let sk = sink.clone();
    threads.push(thread::spawn(move || read_stdout(stdout, t, sk)));
    let stderr = child.stderr.take().unwrap();
    let sk = sink.clone();
    threads.push(thread::spawn(move || {
        for line in read_lines(stderr) {
            sk.send(line);
        }
    }));
    let t = tx.clone();
    let d = done.clone();
    threads.push(thread::spawn(move || {
        let mut tail = JtlTail::new();
        loop {
            let finished = d.load(Ordering::Relaxed);
            let mut more = false;
            if let Ok(rows) = tail.poll(&results) {
                more = tail.has_more();
                if !rows.is_empty() {
                    let _ = t.send(RunEvent::Samples(rows));
                }
            }
            if finished && !more {
                break;
            }
            if !more {
                thread::sleep(Duration::from_millis(250));
            }
        }
    }));
    Ok(RunHandle { child, events: rx, stop_port, done, threads, pending_lines, dropped_lines })
}

/// Lines of a pipe, cut to [`MAX_LINE_CHARS`] as they are read so that one endless line cannot fill the memory.
fn read_lines(r: impl std::io::Read) -> impl Iterator<Item = String> {
    use std::io::Read;
    let mut r = BufReader::new(r);
    let mut done = false;
    std::iter::from_fn(move || {
        if done {
            return None;
        }
        let mut line = Vec::new();
        let mut byte = [0u8; 1];
        let mut cut = false;
        loop {
            match r.read(&mut byte) {
                Ok(0) | Err(_) => {
                    done = true;
                    if line.is_empty() && !cut {
                        return None;
                    }
                    break;
                }
                Ok(_) => {
                    if byte[0] == b'\n' {
                        break;
                    }
                    if line.len() < MAX_LINE_CHARS * 4 {
                        line.push(byte[0]);
                    } else {
                        cut = true;
                    }
                }
            }
        }
        Some(String::from_utf8_lossy(&line).trim_end_matches('\r').to_string())
    })
}

fn read_stdout(stdout: impl std::io::Read, tx: Sender<RunEvent>, sink: LineSink) {
    for line in read_lines(stdout) {
        let ev = if let Some(p) = parse_stop_port(&line) {
            RunEvent::StopPort(p)
        } else if let Some(s) = line.starts_with("summary ").then(|| parse_summary(&line)).flatten() {
            RunEvent::Summary(s)
        } else {
            sink.send(line);
            continue;
        };
        let _ = tx.send(ev);
    }
}

impl RunHandle {
    pub fn stop_port(&self) -> u16 {
        self.stop_port
    }

    pub fn pid(&self) -> u32 {
        self.child.id()
    }

    /// Tell the handle that `n` lines from [`RunEvent::Line`] were taken, so that more can be queued.
    pub fn lines_taken(&self, n: usize) {
        let _ = self.pending_lines.try_update(Ordering::Relaxed, Ordering::Relaxed, |v| Some(v.saturating_sub(n)));
    }

    /// Output lines dropped because the window was not keeping up.
    pub fn dropped_lines(&self) -> usize {
        self.dropped_lines.load(Ordering::Relaxed)
    }

    /// Whether the process is still going.
    pub fn is_running(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }

    /// Ask JMeter to finish: `now` interrupts running samplers, otherwise threads finish their current iteration.
    pub fn stop(&self, now: bool) -> Result<()> {
        let sock = UdpSocket::bind("127.0.0.1:0")?;
        let msg: &[u8] = if now { b"StopTestNow" } else { b"Shutdown" };
        sock.send_to(msg, ("127.0.0.1", self.stop_port))?;
        Ok(())
    }

    pub fn kill(&mut self) -> Result<()> {
        if self.is_running() {
            self.child.kill()?;
        }
        Ok(())
    }

    /// Ask JMeter to finish and wait up to `grace` for it. `false` means it is still running.
    pub fn stop_and_wait(&mut self, now: bool, grace: Duration) -> Result<bool> {
        self.stop(now)?;
        Ok(self.wait(grace)?.is_some() || !self.is_running())
    }

    /// Wait for the process to end (up to `timeout`), then flush the remaining events.
    pub fn wait(&mut self, timeout: Duration) -> Result<Option<i32>> {
        let start = std::time::Instant::now();
        let status = loop {
            if let Some(s) = self.child.try_wait()? {
                break Some(s);
            }
            if start.elapsed() > timeout {
                break None;
            }
            thread::sleep(Duration::from_millis(50));
        };
        let Some(status) = status else { return Ok(None) };
        self.done.store(true, Ordering::Relaxed);
        for t in self.threads.drain(..) {
            let _ = t.join();
        }
        Ok(status.code())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_long_windows_form_of_a_path_is_shortened() {
        assert_eq!(strip_verbatim(Path::new(r"\\?\C:\work\plan.jmx")), Path::new(r"C:\work\plan.jmx"));
        assert_eq!(strip_verbatim(Path::new(r"\\?\UNC\server\share\p.jmx")), Path::new(r"\\server\share\p.jmx"));
        assert_eq!(strip_verbatim(Path::new(r"C:\work\plan.jmx")), Path::new(r"C:\work\plan.jmx"));
        assert_eq!(strip_verbatim(Path::new("/home/me/plan.jmx")), Path::new("/home/me/plan.jmx"));
    }
}
