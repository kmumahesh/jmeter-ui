//! Check against the real thing: a plan holding one of every element JMeter offers, written by this
//! crate, must load in JMeter without errors.
//!
//! Needs Java and a JMeter install (`JMETER_HOME`, or `~/Downloads/apache-jmeter-5.6.3`); skipped without.
//! All thread groups are disabled so JMeter loads and validates the plan but sends nothing.

use plan_core::{Category, EditablePlan, NodeId, rules, templates};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

fn jmeter_home() -> Option<PathBuf> {
    if let Some(h) = std::env::var_os("JMETER_HOME") {
        return Some(PathBuf::from(h));
    }
    let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).map(PathBuf::from)?;
    Some(home.join("Downloads").join("apache-jmeter-5.6.3")).filter(|p| p.join("bin/ApacheJMeter.jar").is_file())
}

fn java() -> Option<PathBuf> {
    let exe = if cfg!(windows) { "java.exe" } else { "java" };
    if let Some(h) = std::env::var_os("JAVA_HOME") {
        let p = PathBuf::from(h).join("bin").join(exe);
        if p.is_file() {
            return Some(p);
        }
    }
    std::env::var_os("PATH").and_then(|v| std::env::split_paths(&v).map(|d| d.join(exe)).find(|p| p.is_file()))
}

/// Run JMeter on a plan; returns the lines of its log that are errors or warnings about loading.
fn load_in_jmeter(home: &Path, java: &Path, dir: &Path, plan_text: &str) -> (bool, Vec<String>) {
    let plan = dir.join("plan.jmx");
    std::fs::write(&plan, plan_text).unwrap();
    let log = dir.join("jmeter.log");
    let mut child = Command::new(java)
        .current_dir(dir)
        .args(["-Djava.awt.headless=true", "-jar"])
        .arg(home.join("bin/ApacheJMeter.jar"))
        .args(["-n", "-t"])
        .arg(&plan)
        .arg("-j")
        .arg(&log)
        .arg("-l")
        .arg(dir.join("res.jtl"))
        .args(["-Jjmeterengine.nongui.port=14445", "-Jjmeterengine.nongui.maxport=14455"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("start java");
    let started = Instant::now();
    let ok = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status.success();
        }
        if started.elapsed() > Duration::from_secs(120) {
            let _ = child.kill();
            break false;
        }
        std::thread::sleep(Duration::from_millis(200));
    };
    let text = std::fs::read_to_string(&log).unwrap_or_default();
    let bad = text.lines().filter(|l| l.contains(" ERROR ") || l.contains(" FATAL ") || l.contains("Exception")).map(str::to_string).collect();
    (ok, bad)
}

#[test]
fn a_plan_with_every_element_loads_in_jmeter() {
    let (Some(home), Some(java)) = (jmeter_home(), java()) else {
        eprintln!("skipped: Java or JMeter not found (set JMETER_HOME)");
        return;
    };
    let src = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<jmeterTestPlan version=\"1.2\" properties=\"5.0\" jmeter=\"5.6.3\">\n  <hashTree>\n    <TestPlan guiclass=\"TestPlanGui\" testclass=\"TestPlan\" testname=\"Every element\" enabled=\"true\"/>\n    <hashTree/>\n  </hashTree>\n</jmeterTestPlan>\n";
    let mut e = EditablePlan::open(src).unwrap();
    let root = e.plan().roots()[0];
    let tg = e.add(root, "ThreadGroup", "ThreadGroupGui").unwrap();
    let sc = e.add(tg, "GenericController", "LogicControllerGui").unwrap();
    let sampler = e.add(sc, "HTTPSamplerProxy", "HttpTestSampleGui").unwrap();
    let parents: [NodeId; 4] = [sc, sampler, tg, root];
    let mut skipped = Vec::new();
    let mut added = 0;
    for t in templates::all() {
        // JMeter refuses to start a plan whose Module Controller has no target, as a new one has not,
        // even when it is disabled. That is JMeter's rule, not a problem with the template.
        if t.tag == "ModuleController" {
            continue;
        }
        let parent = parents.iter().copied().find(|&p| rules::can_add(e.plan().node(p).unwrap().category, t.category));
        match parent {
            Some(p) => {
                e.add(p, &t.tag, &t.guiclass).unwrap_or_else(|err| panic!("{}: {err:#}", t.label));
                added += 1;
            }
            None => skipped.push(t.label.clone()),
        }
    }
    // Nothing may run: switch off every thread group, including the ones just added.
    let groups: Vec<_> = e.plan().iter().filter(|n| n.category == Category::ThreadGroup).map(|n| n.id).collect();
    for g in groups {
        e.set_enabled(g, false).unwrap();
    }
    eprintln!("{added} elements added; not placed in this plan: {skipped:?}");
    assert!(added > 100);
    assert!(skipped.len() < 10, "{skipped:?}");

    let dir = std::env::temp_dir().join(format!("jmeter-ui-load-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let (ok, bad) = load_in_jmeter(&home, &java, &dir, &e.to_text());
    assert!(ok, "JMeter did not finish cleanly; log in {}", dir.display());
    assert!(bad.is_empty(), "JMeter reported problems loading the plan ({}):\n{}", dir.display(), bad.join("\n"));
    if std::env::var_os("JMX_KEEP").is_none() {
        let _ = std::fs::remove_dir_all(&dir);
    }
}
