//! Looking at results that did not come from a run just made: opening a JMeter results file, and making or opening the
//! HTML report of a run.

use crate::running::RunSession;
use crate::shell::{Severity, Shell};
use gpui::{Context, PathPromptOptions, Window};
use jvm_runtime::live::LiveStats;
use jvm_runtime::report;
use std::io::Read;
use std::path::{Path, PathBuf};

/// A results file larger than this is refused. The numbers are read in pieces, so memory stays flat, but a file this
/// large would take many minutes to read.
pub const MAX_RESULTS_BYTES: u64 = 8 * 1024 * 1024 * 1024;

/// What is wrong with a file that cannot be looked at as results, as a sentence, or `None` when it can be tried.
pub fn refuse(path: &Path) -> Option<String> {
    let Ok(meta) = std::fs::metadata(path) else { return Some(format!("{} cannot be read.", path.display())) };
    if !meta.is_file() {
        return Some(format!("{} is not a file.", path.display()));
    }
    if meta.len() == 0 {
        return Some(format!("{} is empty.", path.display()));
    }
    if meta.len() > MAX_RESULTS_BYTES {
        return Some(format!("{} is larger than {} GB.", path.display(), MAX_RESULTS_BYTES / (1024 * 1024 * 1024)));
    }
    let mut head = [0u8; 512];
    let n = std::fs::File::open(path).and_then(|mut f| f.read(&mut head)).unwrap_or(0);
    let start = String::from_utf8_lossy(&head[..n]);
    let start = start.trim_start_matches('\u{feff}').trim_start();
    if start.starts_with('<') {
        return Some("This is an XML results file. Only CSV results files are read; JMeter writes CSV unless jmeter.save.saveservice.output_format is set to xml.".to_string());
    }
    None
}

impl Shell {
    /// Ask for a results file and look at it.
    pub fn open_results_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let rx = cx.prompt_for_paths(PathPromptOptions { files: true, directories: false, multiple: false, prompt: Some("Open JMeter Results File".into()) });
        cx.spawn_in(window, async move |this, cx| {
            if let Ok(Ok(Some(paths))) = rx.await
                && let Some(path) = paths.into_iter().next()
            {
                this.update_in(cx, |this, window, cx| this.open_results_path(&path, window, cx)).ok();
            }
        })
        .detach();
    }

    /// Show a results file as a run: its numbers are read in the background.
    pub fn open_results_path(&mut self, path: &Path, window: &mut Window, cx: &mut Context<Self>) {
        if self.run_in_progress() {
            self.notify("A run is in progress. Wait for it to end or stop it first.", Severity::Info, window, cx);
            return;
        }
        let path = crate::run_ctl::plain_path(&std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf()));
        if let Some(why) = refuse(&path) {
            self.notify(why, Severity::Error, window, cx);
            return;
        }
        self.run = Some(RunSession::from_results(&path));
        self.req_sel = 0;
        self.show_run_panel();
        let key = path.clone();
        cx.spawn_in(window, async move |this, cx| {
            let file = path.clone();
            let loaded = cx.background_executor().spawn(async move { LiveStats::from_file(&file).map_err(|e| format!("{e:#}")) }).await;
            this.update(cx, |this, cx| {
                if let Some(run) = this.run.as_mut().filter(|r| r.loose && r.results_file == key) {
                    match loaded {
                        Ok((live, skipped)) => run.set_stored(live, skipped),
                        Err(e) => run.set_stored_error(e),
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    /// Open the HTML report of the run in the lower panel in the browser. When there is none it is made first, in the
    /// background, with JMeter's own report generator, and opened when it is ready.
    pub fn html_report(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(run) = &self.run else { return };
        if run.phase.active() {
            self.notify("A run is in progress. The report can be made when it ends.", Severity::Info, window, cx);
            return;
        }
        if self.report_busy {
            self.notify("The report is being made", Severity::Info, window, cx);
            return;
        }
        let folder = run.report_dir();
        if let Some(index) = report::existing(&folder) {
            self.open_external(&index);
            return;
        }
        let results = run.results_file.clone();
        if !results.is_file() {
            self.notify("This run has no results file, so there is nothing to make a report from.", Severity::Error, window, cx);
            return;
        }
        let java = match jvm_runtime::locate::find_java(self.settings.java_path.as_deref().map(Path::new)) {
            Ok(j) => j.path,
            Err(e) => return self.notify(format!("{e:#}"), Severity::Error, window, cx),
        };
        let Some(home) = self.settings.jmeter_home.clone().map(PathBuf::from) else {
            return self.notify("No JMeter folder is chosen in Settings", Severity::Error, window, cx);
        };
        self.report_busy = true;
        self.notify(format!("Making the report in {}", folder.display()), Severity::Info, window, cx);
        cx.spawn_in(window, async move |this, cx| {
            let out = folder.clone();
            let outcome = cx.background_executor().spawn(async move { report::generate(&java, &home, &results, &out, report::DEFAULT_TIMEOUT) }).await;
            this.update_in(cx, |this, window, cx| {
                this.report_busy = false;
                match outcome {
                    Ok(index) => {
                        this.notify("The report is ready", Severity::Info, window, cx);
                        this.open_external(&index);
                    }
                    Err(e) => this.notify(e, Severity::Error, window, cx),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("jmeter-ui-results-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn files_that_cannot_be_looked_at_are_refused_with_a_reason() {
        let d = temp("refuse");
        assert!(refuse(&d.join("missing.jtl")).unwrap().contains("cannot be read"));
        assert!(refuse(&d).unwrap().contains("is not a file"));
        std::fs::write(d.join("empty.jtl"), "").unwrap();
        assert!(refuse(&d.join("empty.jtl")).unwrap().contains("is empty"));
        for xml in ["<?xml version=\"1.0\"?><testResults/>", "\u{feff}  <testResults>", "<testResults version=\"1.2\">"] {
            std::fs::write(d.join("x.jtl"), xml).unwrap();
            assert!(refuse(&d.join("x.jtl")).unwrap().contains("XML results file"), "{xml}");
        }
        std::fs::write(d.join("ok.jtl"), "timeStamp,elapsed,label\n1,2,A\n").unwrap();
        assert!(refuse(&d.join("ok.jtl")).is_none());
        // Anything else is tried, and said not to be results when nothing in it reads as a request.
        std::fs::write(d.join("text.txt"), "hello\nworld\n").unwrap();
        assert!(refuse(&d.join("text.txt")).is_none());
        let _ = std::fs::remove_dir_all(d);
    }
}
