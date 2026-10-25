//! Shared helpers for tests that run the real JMeter: finding Java and JMeter, a small web server to test
//! against, and plans to run. Tests skip themselves when Java or JMeter is not installed.

#![allow(dead_code)]

use jvm_runtime::locate::{JavaInfo, find_java};
use jvm_runtime::run::{RunEvent, RunHandle, RunSpec};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

pub struct Env {
    pub java: JavaInfo,
    pub home: PathBuf,
}

/// Java and a JMeter install (`JMETER_HOME`, or `~/Downloads/apache-jmeter-5.6.3`), or `None` to skip.
pub fn env() -> Option<Env> {
    let home = std::env::var_os("JMETER_HOME").map(PathBuf::from).or_else(|| {
        let h = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME"))?;
        Some(PathBuf::from(h).join("Downloads").join("apache-jmeter-5.6.3"))
    })?;
    if !home.join("bin").join("ApacheJMeter.jar").is_file() {
        eprintln!("skipped: no JMeter at {} (set JMETER_HOME)", home.display());
        return None;
    }
    let java = find_java(None).ok()?;
    Some(Env { java, home })
}

pub fn temp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("jmeter-ui-run-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// A web server on a free local port. `/fail` answers 500, `/slow` waits 300 ms, anything else answers 200.
pub struct Server {
    pub port: u16,
    pub requests: Arc<AtomicU64>,
    /// The path of every request received, in order.
    pub paths: Arc<Mutex<Vec<String>>>,
    /// The body of every request received, as text, in order.
    pub bodies: Arc<Mutex<Vec<String>>>,
    /// The request line and headers of every request received, in order.
    pub heads: Arc<Mutex<Vec<String>>>,
    stop: Arc<AtomicBool>,
}

impl Server {
    pub fn start() -> Server {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let port = listener.local_addr().unwrap().port();
        let requests = Arc::new(AtomicU64::new(0));
        let paths = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let bodies = Arc::new(Mutex::new(Vec::new()));
        let heads = Arc::new(Mutex::new(Vec::new()));
        let (r, s, pa, bo, he) = (requests.clone(), stop.clone(), paths.clone(), bodies.clone(), heads.clone());
        std::thread::spawn(move || {
            while !s.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((conn, _)) => {
                        let (r, s, pa, bo, he) = (r.clone(), s.clone(), pa.clone(), bo.clone(), he.clone());
                        std::thread::spawn(move || serve(conn, r, s, pa, bo, he));
                    }
                    Err(_) => std::thread::sleep(Duration::from_millis(10)),
                }
            }
        });
        Server { port, requests, paths, bodies, heads, stop }
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

fn serve(mut conn: TcpStream, requests: Arc<AtomicU64>, stop: Arc<AtomicBool>, paths: Arc<Mutex<Vec<String>>>, bodies: Arc<Mutex<Vec<String>>>, heads: Arc<Mutex<Vec<String>>>) {
    let _ = conn.set_nonblocking(false);
    let _ = conn.set_read_timeout(Some(Duration::from_millis(500)));
    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
    while !stop.load(Ordering::Relaxed) {
        match conn.read(&mut chunk) {
            Ok(0) => return,
            Ok(n) => buf.extend_from_slice(&chunk[..n]),
            Err(e) if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => continue,
            Err(_) => return,
        }
        while let Some(end) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            let head = String::from_utf8_lossy(&buf[..end]).to_string();
            let body_len = head.lines().find_map(|l| l.to_lowercase().strip_prefix("content-length:").and_then(|v| v.trim().parse::<usize>().ok())).unwrap_or(0);
            if buf.len() < end + 4 + body_len {
                break;
            }
            bodies.lock().unwrap().push(String::from_utf8_lossy(&buf[end + 4..end + 4 + body_len]).to_string());
            heads.lock().unwrap().push(head.clone());
            buf.drain(..end + 4 + body_len);
            requests.fetch_add(1, Ordering::Relaxed);
            let path = head.split_whitespace().nth(1).unwrap_or("/").to_string();
            paths.lock().unwrap().push(path.clone());
            if path.starts_with("/slow") {
                std::thread::sleep(Duration::from_millis(300));
            }
            let (status, body) = if path.starts_with("/fail") { ("500 Internal Server Error", "boom") } else { ("200 OK", "ok") };
            let resp = format!("HTTP/1.1 {status}\r\nContent-Type: text/plain\r\nContent-Length: {}\r\n\r\n{body}", body.len());
            if conn.write_all(resp.as_bytes()).is_err() {
                return;
            }
        }
    }
}

/// One sampler for the plan.
pub struct Request {
    pub name: String,
    pub path: String,
}

pub fn request(name: &str, path: &str) -> Request {
    Request { name: name.into(), path: path.into() }
}

/// A plan: one thread group (`users` threads, `loops` iterations or forever when negative, optional timer in
/// milliseconds) with the given requests, all to `127.0.0.1:port`. `extra` is put inside the thread group.
pub fn plan(port: u16, users: u32, loops: i32, timer_ms: u32, requests: &[Request], extra: &str) -> String {
    let mut samplers = String::new();
    for r in requests {
        samplers.push_str(&format!(
            r#"        <HTTPSamplerProxy guiclass="HttpTestSampleGui" testclass="HTTPSamplerProxy" testname="{}" enabled="true">
          <elementProp name="HTTPsampler.Arguments" elementType="Arguments" guiclass="HTTPArgumentsPanel" testclass="Arguments" testname="User Defined Variables" enabled="true">
            <collectionProp name="Arguments.arguments"/>
          </elementProp>
          <stringProp name="HTTPSampler.domain">127.0.0.1</stringProp>
          <stringProp name="HTTPSampler.port">{port}</stringProp>
          <stringProp name="HTTPSampler.protocol">http</stringProp>
          <stringProp name="HTTPSampler.path">{}</stringProp>
          <stringProp name="HTTPSampler.method">GET</stringProp>
          <boolProp name="HTTPSampler.use_keepalive">true</boolProp>
        </HTTPSamplerProxy>
        <hashTree/>
"#,
            r.name, r.path
        ));
    }
    let timer = if timer_ms > 0 {
        format!(
            r#"        <ConstantTimer guiclass="ConstantTimerGui" testclass="ConstantTimer" testname="Think time" enabled="true">
          <stringProp name="ConstantTimer.delay">{timer_ms}</stringProp>
        </ConstantTimer>
        <hashTree/>
"#
        )
    } else {
        String::new()
    };
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<jmeterTestPlan version="1.2" properties="5.0" jmeter="5.6.3">
  <hashTree>
    <TestPlan guiclass="TestPlanGui" testclass="TestPlan" testname="Test plan" enabled="true">
      <boolProp name="TestPlan.functional_mode">false</boolProp>
      <boolProp name="TestPlan.serialize_threadgroups">false</boolProp>
      <elementProp name="TestPlan.user_defined_variables" elementType="Arguments" guiclass="ArgumentsPanel" testclass="Arguments" testname="User Defined Variables" enabled="true">
        <collectionProp name="Arguments.arguments"/>
      </elementProp>
    </TestPlan>
    <hashTree>
      <ThreadGroup guiclass="ThreadGroupGui" testclass="ThreadGroup" testname="Users" enabled="true">
        <stringProp name="ThreadGroup.on_sample_error">continue</stringProp>
        <elementProp name="ThreadGroup.main_controller" elementType="LoopController" guiclass="LoopControlPanel" testclass="LoopController" testname="Loop Controller" enabled="true">
          <boolProp name="LoopController.continue_forever">false</boolProp>
          <stringProp name="LoopController.loops">{loops}</stringProp>
        </elementProp>
        <stringProp name="ThreadGroup.num_threads">{users}</stringProp>
        <stringProp name="ThreadGroup.ramp_time">0</stringProp>
        <boolProp name="ThreadGroup.scheduler">false</boolProp>
        <stringProp name="ThreadGroup.duration"></stringProp>
        <stringProp name="ThreadGroup.delay"></stringProp>
        <boolProp name="ThreadGroup.same_user_on_next_iteration">true</boolProp>
      </ThreadGroup>
      <hashTree>
{timer}{samplers}{extra}      </hashTree>
    </hashTree>
  </hashTree>
</jmeterTestPlan>
"#
    )
}

pub fn spec(env: &Env, plan: &Path, run_dir: &Path) -> RunSpec {
    RunSpec {
        java: env.java.path.clone(),
        jmeter_home: env.home.clone(),
        plan: plan.to_path_buf(),
        run_dir: run_dir.to_path_buf(),
        jvm_args: vec!["-Xmx512m".into()],
        props: vec![],
        generate_report: false,
    }
}

/// Everything the run reported, collected until it ends or `timeout` passes.
#[derive(Default)]
pub struct Seen {
    pub lines: Vec<String>,
    pub summaries: usize,
    pub samples: Vec<jvm_runtime::results::Sample>,
    pub stop_port: Option<u16>,
    pub exit: Option<Option<i32>>,
}

pub fn collect(h: &mut RunHandle, timeout: Duration) -> Seen {
    let mut seen = Seen::default();
    let start = Instant::now();
    while start.elapsed() < timeout {
        match h.events.recv_timeout(Duration::from_millis(100)) {
            Ok(ev) => take(h, &mut seen, ev),
            Err(_) => {
                if !h.is_running() {
                    // Let the reader threads finish, then take what is left.
                    let code = h.wait(Duration::from_secs(5)).ok().flatten();
                    while let Ok(ev) = h.events.try_recv() {
                        take(h, &mut seen, ev);
                    }
                    if seen.exit.is_none() {
                        seen.exit = Some(code);
                    }
                    break;
                }
            }
        }
    }
    seen
}

fn take(h: &RunHandle, seen: &mut Seen, ev: RunEvent) {
    match ev {
        RunEvent::Started { .. } => {}
        RunEvent::StopPort(p) => seen.stop_port = Some(p),
        RunEvent::Summary(_) => seen.summaries += 1,
        RunEvent::Samples(mut s) => seen.samples.append(&mut s),
        RunEvent::Line(l) => {
            h.lines_taken(1);
            seen.lines.push(l);
        }
        RunEvent::Exited { code } => seen.exit = Some(code),
    }
}

/// Wait until `cond` holds, up to `timeout`.
pub fn wait_for(timeout: Duration, mut cond: impl FnMut() -> bool) -> bool {
    let start = Instant::now();
    while start.elapsed() < timeout {
        if cond() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    cond()
}

/// Whether a process with this id is running (Windows and Unix).
pub fn process_alive(pid: u32) -> bool {
    #[cfg(windows)]
    {
        let out = std::process::Command::new("tasklist").args(["/FI", &format!("PID eq {pid}"), "/NH"]).output().unwrap();
        String::from_utf8_lossy(&out.stdout).contains(&pid.to_string())
    }
    #[cfg(unix)]
    {
        // `kill -0` sends nothing; it only says whether the process exists (there is no /proc on macOS).
        std::process::Command::new("kill").args(["-0", &pid.to_string()]).stderr(std::process::Stdio::null()).status().is_ok_and(|s| s.success())
    }
}
