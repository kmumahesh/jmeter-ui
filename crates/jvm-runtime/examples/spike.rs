//! Spikes 3 and 4: run JMeter from Rust with live results and a UDP stop, and capture a failed
//! sample's response body through an injected XML collector.
//!
//! Usage: spike <jmeter-home> <work-dir>

use jmx_model::{Document, runcopy};
use jvm_runtime::locate::{find_java, inspect_jmeter};
use jvm_runtime::results::read_error_samples;
use jvm_runtime::run::{RunEvent, RunSpec, start};
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener};
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// A tiny HTTP server: /api/checkout answers 500 with a JSON error, everything else 200.
fn test_server() -> SocketAddr {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = l.local_addr().unwrap();
    std::thread::spawn(move || {
        for conn in l.incoming().flatten() {
            std::thread::spawn(move || {
                let mut conn = conn;
                let mut buf = [0u8; 4096];
                let n = conn.read(&mut buf).unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..n]);
                let (status, body) = if req.contains("/api/checkout") {
                    ("500 Internal Server Error", r#"{"error":"payment_gateway_timeout","retryable":true}"#)
                } else {
                    ("200 OK", r#"{"ok":true}"#)
                };
                let _ = write!(conn, "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
            });
        }
    });
    addr
}

fn check(name: &str, ok: bool, detail: impl std::fmt::Display) -> bool {
    println!("  [{}] {name}: {detail}", if ok { "PASS" } else { "FAIL" });
    ok
}

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let home = PathBuf::from(&args[1]);
    let work = PathBuf::from(&args[2]);
    let mut all_ok = true;

    println!("== Runtime checks");
    let java = find_java(None)?;
    all_ok &= check("java", java.major >= 8, &java.version_line);
    let jm = inspect_jmeter(&home, &java)?;
    all_ok &= check("jmeter version", jm.version == "5.6.3", &jm.version);
    all_ok &= check("lib/ext jars", !jm.ext_jars.is_empty(), jm.ext_jars.len());

    println!("== Spike 3: run extras/Test.jmx, live events");
    let run3 = work.join("run-3");
    let mut h = start(&RunSpec {
        java: java.path.clone(),
        jmeter_home: home.clone(),
        plan: home.join("extras").join("Test.jmx"),
        run_dir: run3.clone(),
        jvm_args: vec!["-Xms256m".into(), "-Xmx512m".into()],
        props: vec![],
        generate_report: false,
    })?;
    let (mut summaries, mut rows, mut port) = (0, 0, None);
    let t0 = Instant::now();
    while t0.elapsed() < Duration::from_secs(60) {
        match h.events.recv_timeout(Duration::from_millis(200)) {
            Ok(RunEvent::Summary(s)) => {
                summaries += 1;
                println!("    summary {} samples={} avg={}ms err={}", if s.cumulative { "=" } else { "+" }, s.samples, s.avg_ms, s.errors)
            }
            Ok(RunEvent::Samples(v)) => rows += v.len(),
            Ok(RunEvent::StopPort(p)) => port = Some(p),
            _ => {}
        }
        if let Some(code) = h.wait(Duration::from_millis(1))? {
            // drain
            while let Ok(ev) = h.events.try_recv() {
                match ev {
                    RunEvent::Summary(_) => summaries += 1,
                    RunEvent::Samples(v) => rows += v.len(),
                    RunEvent::StopPort(p) => port = Some(p),
                    _ => {}
                }
            }
            all_ok &= check("exit code", code == 0, code);
            break;
        }
    }
    all_ok &= check("stop port reported matches the one we set", port == Some(h.stop_port()), format!("{port:?}"));
    all_ok &= check("summary lines parsed", summaries >= 2, summaries);
    all_ok &= check("results rows tailed", rows == 30, rows);

    println!("== Spike 4: injected XML error collector + UDP stop");
    let addr = test_server();
    let tmpl = std::fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/plan_http.jmx.tmpl"))?;
    let src = tmpl.replace("{PORT}", &addr.port().to_string());
    let mut doc = Document::parse(&src)?;
    let original = doc.write();
    all_ok &= check("template round-trips byte-for-byte", original == src, "");
    let run4 = work.join("run-4");
    std::fs::create_dir_all(&run4)?;
    let errors_xml = run4.join("errors.xml");
    runcopy::inject_collector(&mut doc, &runcopy::Collector::XmlErrors { path: &errors_xml.to_string_lossy() })?;
    let n = runcopy::set_loops(&mut doc, -1);
    all_ok &= check("loops set to forever", n == 1, n);
    let plan4 = run4.join("plan.jmx");
    std::fs::write(&plan4, doc.write())?;
    // The user's plan is untouched: the template text on disk still has 5 loops.
    all_ok &= check("original text unchanged", src.contains(">5</stringProp>"), "");

    let mut h = start(&RunSpec {
        java: java.path.clone(),
        jmeter_home: home.clone(),
        plan: plan4,
        run_dir: run4.clone(),
        jvm_args: vec!["-Xms256m".into(), "-Xmx512m".into()],
        props: vec![],
        generate_report: false,
    })?;
    let mut rows4 = 0usize;
    let mut failed_rows = 0usize;
    let t0 = Instant::now();
    let mut stopped = false;
    let mut exit = None;
    while t0.elapsed() < Duration::from_secs(90) {
        while let Ok(ev) = h.events.try_recv() {
            if let RunEvent::Samples(v) = ev {
                rows4 += v.len();
                failed_rows += v.iter().filter(|s| !s.success).count();
            }
        }
        if !stopped && t0.elapsed() > Duration::from_secs(6) {
            println!("    sending Shutdown to UDP port {} after {} rows", h.stop_port(), rows4);
            h.stop(false)?;
            stopped = true;
        }
        if let Some(code) = h.wait(Duration::from_millis(100))? {
            exit = Some(code);
            break;
        }
    }
    while let Ok(ev) = h.events.try_recv() {
        if let RunEvent::Samples(v) = ev {
            rows4 += v.len();
            failed_rows += v.iter().filter(|s| !s.success).count();
        }
    }
    all_ok &= check("run ended after Shutdown (before the 90 s cap)", exit == Some(0), format!("{exit:?} after {:?}", t0.elapsed()));
    all_ok &= check("rows streamed while running", rows4 > 10, rows4);
    all_ok &= check("failed rows seen", failed_rows > 0, failed_rows);

    let errs = read_error_samples(&errors_xml)?;
    all_ok &= check("error samples captured", !errs.is_empty(), errs.len());
    if let Some(e) = errs.first() {
        println!("    first error: {} rc={} url={}", e.label, e.response_code, e.url);
        println!("    response body: {}", e.response_body.trim());
        println!("    assertion: {:?}", e.assertion_failures);
        all_ok &= check("response body readable", e.response_body.contains("payment_gateway_timeout"), "");
        all_ok &= check("only the failing sampler appears", errs.iter().all(|e| e.label.contains("checkout")), "");
    }
    println!("\n{}", if all_ok { "ALL CHECKS PASSED" } else { "SOME CHECKS FAILED" });
    std::process::exit(if all_ok { 0 } else { 1 });
}
