//! Make JMeter's HTML report from a results file, with JMeter's own report generator (`-g results.jtl -o folder`).
//!
//! The generator runs in a folder of its own that is removed afterwards, so nothing is left next to the results
//! except the report folder. It runs in the background of the window and is ended when it takes too long.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// The file that opens the report.
pub const INDEX: &str = "index.html";
/// How long the generator may take. Large files take minutes.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10 * 60);
/// The most of the generator's own output that is kept, for a message when it fails.
const MAX_OUTPUT_BYTES: u64 = 64 * 1024;

fn plain(p: &Path) -> bool {
    !p.to_string_lossy().chars().any(|c| c.is_control())
}

/// Where the report of a results file goes by default: a folder next to it named after it.
pub fn default_folder(results: &Path) -> PathBuf {
    let stem = results.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "results".into());
    results.parent().unwrap_or_else(|| Path::new(".")).join(format!("{stem}-report"))
}

/// The report in `folder`, when there is one.
pub fn existing(folder: &Path) -> Option<PathBuf> {
    let index = folder.join(INDEX);
    index.is_file().then_some(index)
}

/// The settings the report generator needs from a plain results file, so that a file from a run of this app and a file from
/// another tool both work: JMeter reads the column names from the first line.
fn jmeter_args(home: &Path, results: &Path, out: &Path, log: &Path) -> Vec<std::ffi::OsString> {
    vec![
        "-Xmx1g".into(),
        "-jar".into(),
        home.join("bin").join("ApacheJMeter.jar").into(),
        "-j".into(),
        log.into(),
        "-g".into(),
        results.into(),
        "-o".into(),
        out.into(),
    ]
}

/// Make the report of `results` in `out`, which must not exist or must be empty. Returns the page that opens it. The
/// error is a sentence for the person who asked: what is wrong with the file or the folder, or what JMeter said.
pub fn generate(java: &Path, jmeter_home: &Path, results: &Path, out: &Path, timeout: Duration) -> Result<PathBuf, String> {
    let ext = java.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    if matches!(ext.as_str(), "bat" | "cmd" | "com" | "ps1" | "vbs" | "js") || !java.is_file() {
        return Err("Java was not found. Check the Runtime section in Settings.".into());
    }
    if !jmeter_home.join("bin").join("ApacheJMeter.jar").is_file() {
        return Err("JMeter was not found. Check the Runtime section in Settings.".into());
    }
    if !results.is_file() {
        return Err(format!("{} is not a file.", results.display()));
    }
    for p in [java, jmeter_home, results, out] {
        if !plain(p) {
            return Err("A path has control characters.".into());
        }
    }
    // JMeter refuses a folder that already holds files, and would otherwise write into it.
    match std::fs::read_dir(out) {
        Ok(mut it) => {
            if it.next().is_some() {
                return Err(format!("{} already has files in it. Remove it or choose another folder.", out.display()));
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(format!("Cannot use {}: {e}", out.display())),
    }
    let (results, out) = (std::path::absolute(results).unwrap_or_else(|_| results.to_path_buf()), std::path::absolute(out).unwrap_or_else(|_| out.to_path_buf()));
    let (results, out) = (crate::run::strip_verbatim(&results), crate::run::strip_verbatim(&out));
    let work = std::env::temp_dir().join(format!("jmeter-ui-report-{}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0)));
    std::fs::create_dir_all(&work).map_err(|e| format!("Could not make a folder to work in: {e}"))?;
    let outcome = run(java, jmeter_home, &results, &out, &work, timeout);
    let _ = std::fs::remove_dir_all(&work);
    outcome
}

fn run(java: &Path, home: &Path, results: &Path, out: &Path, work: &Path, timeout: Duration) -> Result<PathBuf, String> {
    let mut cmd = Command::new(java);
    cmd.args(jmeter_args(home, results, out, &work.join("jmeter.log"))).current_dir(work).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    crate::locate::quiet(&mut cmd);
    let mut child = cmd.spawn().map_err(|e| format!("Could not start Java: {e}"))?;
    let _tied = crate::job::tie_to_this_process(&child);
    let mut stdout = child.stdout.take().ok_or("Could not read from Java")?;
    let mut stderr = child.stderr.take().ok_or("Could not read from Java")?;
    let read_out = std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = (&mut stdout).take(MAX_OUTPUT_BYTES).read_to_end(&mut buf);
        // Keep reading so JMeter never blocks on a full pipe.
        let _ = std::io::copy(&mut stdout, &mut std::io::sink());
        buf
    });
    let read_err = std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = (&mut stderr).take(MAX_OUTPUT_BYTES).read_to_end(&mut buf);
        let _ = std::io::copy(&mut stderr, &mut std::io::sink());
        buf
    });
    let start = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break Some(s),
            Ok(None) if start.elapsed() >= timeout => break None,
            Ok(None) => std::thread::sleep(Duration::from_millis(50)),
            Err(e) => return Err(format!("Lost track of Java: {e}")),
        }
    };
    if status.is_none() {
        let _ = child.kill();
        let _ = child.wait();
    }
    let out_text = String::from_utf8_lossy(&read_out.join().unwrap_or_default()).into_owned();
    let err_text = String::from_utf8_lossy(&read_err.join().unwrap_or_default()).into_owned();
    let Some(status) = status else {
        let _ = std::fs::remove_dir_all(out);
        return Err(format!("Stopped after {}. The results file may be too large for a report.", words(timeout)));
    };
    match (status.success(), existing(out)) {
        (true, Some(index)) => Ok(index),
        _ => {
            // A report that was started and not finished is not left for someone to open.
            let _ = std::fs::remove_dir_all(out);
            Err(why(&out_text, &err_text))
        }
    }
}

/// A length of time in words: `10 minutes`, `1 minute`, `90 seconds`, `0.1 seconds`.
fn words(d: Duration) -> String {
    let secs = d.as_secs_f32();
    if d.as_secs() >= 60 && d.as_secs().is_multiple_of(60) {
        let m = d.as_secs() / 60;
        format!("{m} minute{}", if m == 1 { "" } else { "s" })
    } else {
        format!("{secs} seconds")
    }
}

/// The lines JMeter printed that say what went wrong, as one sentence or two.
fn why(out: &str, err: &str) -> String {
    let lines: Vec<&str> = err.lines().chain(out.lines()).map(str::trim).filter(|l| !l.is_empty()).collect();
    let hints: Vec<&str> = lines.iter().copied().filter(|l| l.contains("ERROR") || l.contains("Error") || l.contains("Exception") || l.contains("error")).take(2).collect();
    let pick = if hints.is_empty() { lines.iter().rev().take(2).rev().copied().collect::<Vec<_>>() } else { hints };
    let text: String = pick.join(" / ");
    if text.is_empty() { "JMeter did not make a report and said nothing about why.".into() } else { format!("JMeter could not make the report: {}", text.chars().take(400).collect::<String>()) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_report_goes_in_a_folder_next_to_the_results_named_after_them() {
        assert_eq!(default_folder(Path::new("C:/work/run1/results.jtl")), PathBuf::from("C:/work/run1/results-report"));
        assert_eq!(default_folder(Path::new("a.csv")), PathBuf::from("a-report"));
    }

    #[test]
    fn what_cannot_be_done_is_refused_before_anything_starts() {
        let d = Duration::from_secs(1);
        let e = generate(Path::new("no-such-java.exe"), Path::new("x"), Path::new("r.jtl"), Path::new("o"), d).unwrap_err();
        assert!(e.contains("Java was not found"));
        for script in ["x.bat", "x.cmd", "x.ps1"] {
            assert!(generate(Path::new(script), Path::new("x"), Path::new("r.jtl"), Path::new("o"), d).unwrap_err().contains("Java was not found"), "{script}");
        }
    }

    #[test]
    fn times_are_said_in_words() {
        assert_eq!(words(Duration::from_secs(600)), "10 minutes");
        assert_eq!(words(Duration::from_secs(60)), "1 minute");
        assert_eq!(words(Duration::from_secs(90)), "90 seconds");
        assert_eq!(words(Duration::from_millis(100)), "0.1 seconds");
    }

    #[test]
    fn the_reason_is_taken_from_what_jmeter_printed() {
        assert!(why("", "").contains("said nothing"));
        let m = why("INFO starting\nERROR - Could not read the file", "");
        assert!(m.contains("Could not read the file"), "{m}");
        let m = why("one\ntwo\nthree", "");
        assert!(m.contains("two") && m.contains("three") && !m.contains("one"), "{m}");
    }
}
