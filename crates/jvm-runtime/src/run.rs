//! Starting a non-GUI JMeter run, streaming events, and stopping it.

use crate::locate::quiet;
use crate::results::{JtlTail, Sample, Summary, parse_stop_port, parse_summary};
use anyhow::{Context, Result};
use std::io::{BufRead, BufReader};
use std::net::UdpSocket;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
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
}

/// A UDP port that is free on loopback right now. JMeter only uses ports above 1000.
pub fn free_udp_port() -> Result<u16> {
    let s = UdpSocket::bind("127.0.0.1:0")?;
    Ok(s.local_addr()?.port())
}

pub fn start(spec: &RunSpec) -> Result<RunHandle> {
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

    let (tx, rx) = channel();
    let done = Arc::new(AtomicBool::new(false));
    let _ = tx.send(RunEvent::Started { pid: child.id() });
    let mut threads = Vec::new();

    let stdout = child.stdout.take().unwrap();
    let t = tx.clone();
    threads.push(thread::spawn(move || read_stdout(stdout, t)));
    let stderr = child.stderr.take().unwrap();
    let t = tx.clone();
    threads.push(thread::spawn(move || {
        for line in BufReader::new(stderr).lines().map_while(Result::ok) {
            let _ = t.send(RunEvent::Line(line));
        }
    }));
    let t = tx.clone();
    let d = done.clone();
    threads.push(thread::spawn(move || {
        let mut tail = JtlTail::new();
        loop {
            let finished = d.load(Ordering::Relaxed);
            if let Ok(rows) = tail.poll(&results)
                && !rows.is_empty() {
                    let _ = t.send(RunEvent::Samples(rows));
                }
            if finished {
                break;
            }
            thread::sleep(Duration::from_millis(250));
        }
    }));
    Ok(RunHandle { child, events: rx, stop_port, done, threads })
}

fn read_stdout(stdout: impl std::io::Read, tx: Sender<RunEvent>) {
    for line in BufReader::new(stdout).lines().map_while(Result::ok) {
        let ev = if let Some(p) = parse_stop_port(&line) {
            RunEvent::StopPort(p)
        } else if let Some(s) = line.starts_with("summary ").then(|| parse_summary(&line)).flatten() {
            RunEvent::Summary(s)
        } else {
            RunEvent::Line(line)
        };
        let _ = tx.send(ev);
    }
}

impl RunHandle {
    pub fn stop_port(&self) -> u16 {
        self.stop_port
    }

    /// Ask JMeter to finish: `now` interrupts running samplers, otherwise threads finish their current iteration.
    pub fn stop(&self, now: bool) -> Result<()> {
        let sock = UdpSocket::bind("127.0.0.1:0")?;
        let msg: &[u8] = if now { b"StopTestNow" } else { b"Shutdown" };
        sock.send_to(msg, ("127.0.0.1", self.stop_port))?;
        Ok(())
    }

    pub fn kill(&mut self) -> Result<()> {
        self.child.kill()?;
        Ok(())
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
