//! Starting, watching, stopping and looking back at runs: the steps before a run (save, place for the run
//! files, scripts), the run itself, and the run history.

use crate::running::{Phase, RunSession};
use crate::shell::{Dialog, Severity, Shell};
use gpui::{Context, PathPromptOptions, Window};
use jvm_runtime::details;
use jvm_runtime::live::LiveStats;
use jvm_runtime::run::{self, RunSpec};
use jvm_runtime::runs::{self, Place, RunEntry, RunKind};
use plan_core::{NodeId, derive, scripts};
use std::path::{Path, PathBuf};
use shell_core::settings::RequestDetails;
use std::time::Duration;

/// The height the lower panel is given when a run starts, so the numbers and charts are visible.
pub const RUN_PANEL_HEIGHT: f32 = 380.0;

/// The plan text with the listener that keeps requests and responses added to it.
fn with_details_listener(text: &str) -> Result<String, String> {
    let mut doc = jmx_model::Document::parse(text).map_err(|e| format!("{e:#}"))?;
    jmx_model::runcopy::inject_script_listener(&mut doc, "JMeter UI request details", details::SCRIPT).map_err(|e| format!("{e:#}"))?;
    Ok(doc.write())
}

/// A path without the `\\?\` prefix Windows gives to canonical paths.
fn plain(p: &Path) -> PathBuf {
    run::strip_verbatim(p)
}

/// The same, for other modules.
pub fn plain_path(p: &Path) -> PathBuf {
    plain(p)
}

impl Shell {
    pub fn run_in_progress(&self) -> bool {
        self.run.as_ref().is_some_and(|r| r.phase.active())
    }

    /// Where run files go, from the settings.
    pub fn runs_place(&self) -> Place {
        let (custom, folder) = self.settings.runs_place();
        Place::from_setting(custom, folder)
    }

    /// The `runs` folder for the open plan, without making it.
    pub fn runs_root_for_active(&self) -> Option<PathBuf> {
        self.plan().map(|lp| runs::runs_root(&plain(&lp.path), &self.runs_place()))
    }

    /// The shared `runs` folder chosen in Settings (not the one next to a plan).
    pub fn shared_runs_root(&self) -> Option<PathBuf> {
        match self.runs_place() {
            Place::Folder(f) => Some(f.join(runs::RUNS_DIR)),
            Place::PlanFolder => None,
        }
    }

    // ---- before a run ----

    /// Run the plan, validate it, or send the selected request once. Each step that is not ready asks the
    /// user and starts over from here when answered: the plan must be saved (the saved file is what runs),
    /// the place for run files must be usable, and a plan with scripts must have been agreed to.
    pub fn begin_run(&mut self, kind: RunKind, sampler: Option<NodeId>, window: &mut Window, cx: &mut Context<Self>) {
        if self.run_in_progress() {
            self.notify("A run is already in progress", Severity::Info, window, cx);
            return;
        }
        if !self.runtime_ready() {
            self.notify("Java or JMeter was not found. Check the Runtime section in Settings.", Severity::Error, window, cx);
            return;
        }
        self.flush_form(window, cx);
        let Some(lp) = self.plan() else { return };
        if lp.is_dirty() || self.typing_dirty {
            self.dialog = Some(Dialog::SaveToRun { kind, sampler });
            cx.notify();
            return;
        }
        let plan_path = plain(&lp.path);
        if let Err(message) = runs::prepare_root(&plan_path, &self.runs_place()) {
            self.dialog = Some(Dialog::RunsPlace { message, kind, sampler });
            cx.notify();
            return;
        }
        let found = scripts::find(&lp.plan, &lp.edit.to_text());
        if !found.is_empty() && !self.settings.script_approved_for(&plan_path.to_string_lossy()) {
            self.dialog = Some(Dialog::Scripts { kind, sampler, text: scripts::describe(&found) });
            cx.notify();
            return;
        }
        self.launch(kind, sampler, window, cx);
    }

    pub fn runtime_ready(&self) -> bool {
        self.runtime.java.is_some() && self.runtime.jmeter.is_some()
    }

    /// "Save and run" in the question about unsaved changes.
    pub fn dialog_save_and_run(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(Dialog::SaveToRun { kind, sampler }) = self.dialog.clone() else { return };
        self.dialog = None;
        if self.save_active(window, cx) {
            self.begin_run(kind, sampler, window, cx);
        }
    }

    /// "Run anyway" in the question about scripts: remember the answer for this plan file.
    pub fn dialog_run_scripts(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(Dialog::Scripts { kind, sampler, .. }) = self.dialog.clone() else { return };
        self.dialog = None;
        if let Some(lp) = self.plan() {
            let p = plain(&lp.path).to_string_lossy().into_owned();
            self.settings.approve_scripts(&p);
            self.save_settings();
        }
        self.begin_run(kind, sampler, window, cx);
    }

    /// "Use the .jmx folder" in the question about the place for run files.
    pub fn dialog_use_plan_folder(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(Dialog::RunsPlace { kind, sampler, .. }) = self.dialog.clone() else { return };
        self.dialog = None;
        self.settings.runs_custom = false;
        self.save_settings();
        self.begin_run(kind, sampler, window, cx);
    }

    /// "Choose folder…" in the question about the place for run files.
    pub fn dialog_choose_runs_folder(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(Dialog::RunsPlace { kind, sampler, .. }) = self.dialog.clone() else { return };
        self.dialog = None;
        self.choose_runs_folder(window, cx, Some((kind, sampler)));
        cx.notify();
    }

    // ---- the run ----

    fn launch(&mut self, kind: RunKind, sampler: Option<NodeId>, window: &mut Window, cx: &mut Context<Self>) {
        let Some(lp) = self.plan() else { return };
        let plan_path = plain(&lp.path);
        let plan_dir = plan_path.parent().map(Path::to_path_buf).unwrap_or_default();
        let root = match runs::prepare_root(&plan_path, &self.runs_place()) {
            Ok(r) => r,
            Err(m) => return self.notify(m, Severity::Error, window, cx),
        };
        let java = match jvm_runtime::locate::find_java(self.settings.java_path.as_deref().map(Path::new)) {
            Ok(j) => j,
            Err(e) => return self.notify(format!("{e:#}"), Severity::Error, window, cx),
        };
        let Some(home) = self.settings.jmeter_home.clone().map(PathBuf::from) else {
            return self.notify("No JMeter folder is chosen in Settings", Severity::Error, window, cx);
        };

        // The plan JMeter is given: the saved file itself, or a plan made from it for Validate and Send once.
        // Which requests keep what was sent and received. A check of one request always does.
        let mode = match (kind, self.settings.request_details) {
            (RunKind::Validate | RunKind::SendOnce, _) => details::Mode::All,
            (RunKind::Run, RequestDetails::Failed) => details::Mode::Failed,
            (RunKind::Run, RequestDetails::All) => details::Mode::All,
            (RunKind::Run, RequestDetails::Off) => details::Mode::Off,
        };
        let mut copy_problem: Option<String> = None;
        let derived = match kind {
            // The copy is made from the saved file, which is what runs. If it cannot be read the file runs as it is.
            RunKind::Run if mode != details::Mode::Off => match std::fs::read_to_string(&plan_path).map_err(anyhow::Error::from).and_then(|t| derive::run_copy(&t, &plan_dir)) {
                Ok(d) => Some(Ok(d)),
                Err(e) => {
                    copy_problem = Some(format!("Requests and responses are not kept for this run: {e:#}"));
                    None
                }
            },
            RunKind::Run => None,
            RunKind::Validate => Some(derive::validate_plan(&lp.edit, &plan_dir)),
            RunKind::SendOnce => match sampler {
                Some(s) => Some(derive::send_once_plan(&lp.edit, s, &plan_dir)),
                None => return self.notify("Select a request first", Severity::Error, window, cx),
            },
        };
        let derived = match derived.transpose() {
            Ok(d) => d,
            Err(e) => return self.notify(format!("{e:#}"), Severity::Error, window, cx),
        };
        let saved_text = lp.edit.to_text();

        let (dir, mut meta) = match runs::new_run_now(&root, &plan_path, kind) {
            Ok(x) => x,
            Err(e) => return self.notify(format!("Cannot make a folder for the run in {}: {e}", root.display()), Severity::Error, window, cx),
        };
        let mut notes: Vec<String> = derived.as_ref().map(|d| d.notes.clone()).unwrap_or_default();
        notes.extend(copy_problem);
        let mut mode = mode;
        let plan_to_run = match &derived {
            Some(d) => {
                // The listener that keeps requests and responses is added to this copy only.
                let text = if mode == details::Mode::Off {
                    d.text.clone()
                } else {
                    match with_details_listener(&d.text) {
                        Ok(t) => t,
                        Err(e) => {
                            mode = details::Mode::Off;
                            notes.push(format!("Requests and responses are not kept for this run: {e}"));
                            d.text.clone()
                        }
                    }
                };
                let p = dir.join("plan.jmx");
                if let Err(e) = std::fs::write(&p, text) {
                    let _ = std::fs::remove_dir_all(&dir);
                    return self.notify(format!("Cannot write the plan for the run: {e}"), Severity::Error, window, cx);
                }
                if kind == RunKind::Run {
                    // The record of what ran is the saved plan, not the copy.
                    let _ = std::fs::write(dir.join("plan-as-run.jmx"), &saved_text);
                }
                p
            }
            None => {
                // The record of what ran; JMeter is given the saved file itself.
                let _ = std::fs::write(dir.join("plan-as-run.jmx"), &saved_text);
                plan_path.clone()
            }
        };
        if derived.is_none() {
            mode = details::Mode::Off;
        }
        notes.extend(run::path_warning(&plan_path));
        notes.extend(run::path_warning(&dir));
        let spec = RunSpec {
            java: java.path,
            jmeter_home: home,
            plan: plan_to_run,
            run_dir: dir.clone(),
            jvm_args: vec![format!("-Xmx{}m", self.settings.heap_mb)],
            props: {
                let mut props = vec![("summariser.interval".into(), self.settings.summary_interval_secs.to_string())];
                if mode != details::Mode::Off {
                    props.extend(details::props(mode, &dir.join(crate::running::DETAILS_FILE), details::MAX_KEPT));
                }
                props
            },
            generate_report: false,
        };
        let handle = match run::start(&spec) {
            Ok(h) => h,
            Err(e) => {
                let _ = std::fs::remove_dir_all(&dir);
                return self.notify(format!("Could not start the run: {e:#}"), Severity::Error, window, cx);
            }
        };
        meta.notes = notes.clone();
        let _ = runs::write_meta(&dir, &meta);
        let mut session = RunSession::live(kind, plan_path, dir, root, meta, handle, notes);
        session.details_off = mode == details::Mode::Off;
        self.run = Some(session);
        self.req_sel = 0;
        self.show_run_panel();
        self.refresh_history();
        self.schedule_run_tick(window, cx);
        cx.notify();
    }

    /// Make the lower panel show the run, tall enough for the numbers and charts.
    pub fn show_run_panel(&mut self) {
        self.bottom_tab = crate::shell::BottomTab::Run;
        if !self.workspace.bottom.visible {
            self.workspace.toggle_bottom();
        }
        if self.workspace.bottom.size < RUN_PANEL_HEIGHT {
            self.workspace.bottom.size = RUN_PANEL_HEIGHT;
        }
    }

    /// Look at the run now and then while it goes.
    fn schedule_run_tick(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.run_tick_on {
            return;
        }
        self.run_tick_on = true;
        cx.spawn_in(window, async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_millis(200)).await;
                let go = this.update_in(cx, |this, window, cx| this.tick_run(window, cx)).unwrap_or(false);
                if !go {
                    break;
                }
            }
            this.update(cx, |this, _| this.run_tick_on = false).ok();
        })
        .detach();
    }

    /// Take what the run reported. Returns whether it is still going.
    pub fn tick_run(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let Some(run) = &mut self.run else { return false };
        if !run.phase.active() {
            return false;
        }
        run.poll();
        let still = run.phase.active();
        if !still {
            self.run_finished(window, cx);
        }
        cx.notify();
        still
    }

    fn run_finished(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(run) = &self.run else { return };
        let (phase, root, plan, kind) = (run.phase, run.root.clone(), run.plan_path.clone(), run.kind);
        let t = run.live.total();
        let summary = format!("{} {}: {} samples, {} errors", kind.label(), phase.label().to_lowercase(), t.count, t.errors);
        let error = run.error.clone();
        // Keep the last few runs of this plan.
        runs::prune(&root, &plan, self.settings.runs_keep as usize);
        self.refresh_history();
        // A single request that was sent to look at it: show what came back.
        if kind == RunKind::SendOnce && self.run.as_ref().is_some_and(|r| !r.details.is_empty()) {
            self.open_requests(Some(0), cx);
        }
        match (phase, error) {
            (Phase::Failed, Some(e)) => self.notify(e, Severity::Error, window, cx),
            (Phase::Failed, None) => self.notify(summary, Severity::Error, window, cx),
            _ => self.notify(summary, Severity::Info, window, cx),
        }
    }

    pub fn stop_run(&mut self, now: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(run) = &mut self.run else { return };
        if let Err(e) = run.request_stop(now) {
            self.notify(format!("Could not tell JMeter to stop: {e:#}"), Severity::Error, window, cx);
        }
        cx.notify();
    }

    /// End JMeter by force, after a stop took too long.
    pub fn kill_run(&mut self, cx: &mut Context<Self>) {
        if let Some(run) = &mut self.run {
            run.kill();
        }
        cx.notify();
    }

    /// Stop everything for quitting: asks JMeter to stop, then ends it.
    pub fn stop_run_for_quit(&mut self) {
        if let Some(run) = &mut self.run
            && run.phase.active()
        {
            let _ = run.request_stop(true);
            run.kill();
            run.poll();
        }
    }

    /// "Stop and quit" in the question about quitting during a run.
    pub fn dialog_stop_and_quit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if matches!(self.dialog, Some(Dialog::QuitRunning)) {
            self.dialog = None;
            self.stop_run_for_quit();
            if self.request_quit(cx) {
                self.save_window_state(window);
                cx.quit();
            }
        }
    }

    // ---- history ----

    /// Read the runs of the open plan from its runs folder.
    pub fn refresh_history(&mut self) {
        self.history = match self.plan() {
            Some(lp) => {
                let path = plain(&lp.path);
                let root = runs::runs_root(&path, &self.runs_place());
                runs::list_runs(&root, Some(&path))
            }
            None => Vec::new(),
        };
    }

    /// Show a stored run: its numbers are read from its results file in the background.
    pub fn view_run(&mut self, dir: &Path, window: &mut Window, cx: &mut Context<Self>) {
        if self.run_in_progress() {
            self.notify("A run is in progress. Wait for it to end or stop it first.", Severity::Info, window, cx);
            return;
        }
        let Some(entry) = self.history.iter().find(|e| e.dir == dir).cloned() else { return };
        let root = entry.dir.parent().map(Path::to_path_buf).unwrap_or_default();
        self.run = Some(RunSession::stored(&entry, root));
        self.bottom_tab = crate::shell::BottomTab::Run;
        let results = entry.dir.join("results.jtl");
        let details_file = entry.dir.join(crate::running::DETAILS_FILE);
        self.req_sel = 0;
        let key = entry.dir.clone();
        cx.spawn_in(window, async move |this, cx| {
            let loaded = cx.background_executor().spawn(async move { LiveStats::from_file(&results).map_err(|e| format!("{e:#}")) }).await;
            let kept = cx.background_executor().spawn(async move { details::read_all(&details_file) }).await;
            this.update(cx, |this, cx| {
                if let Some(run) = this.run.as_mut().filter(|r| r.dir == key) {
                    match loaded {
                        Ok((live, skipped)) => run.set_stored(live, skipped),
                        Err(e) => run.set_stored_error(e),
                    }
                    run.set_stored_details(kept.0, kept.1);
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    /// Ask before removing one stored run.
    pub fn request_delete_run(&mut self, dir: &Path, cx: &mut Context<Self>) {
        if self.run.as_ref().is_some_and(|r| r.dir == dir && r.phase.active()) {
            return;
        }
        let Some(e) = self.history.iter().find(|e| e.dir == dir) else { return };
        let name = e.dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        self.dialog = Some(Dialog::DeleteRun { dir: dir.to_path_buf(), name });
        cx.notify();
    }

    pub fn dialog_delete_run(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(Dialog::DeleteRun { dir, .. }) = self.dialog.take() else { return };
        // Only a folder this app made, directly inside a runs folder.
        let ok = runs::read_meta(&dir).is_some() && dir.parent().and_then(|p| p.file_name()).is_some_and(|n| n == runs::RUNS_DIR);
        if ok && std::fs::remove_dir_all(&dir).is_ok() {
            if self.run.as_ref().is_some_and(|r| r.dir == dir) {
                self.run = None;
            }
            self.refresh_history();
            self.notify("Run removed", Severity::Info, window, cx);
        } else {
            self.notify("That run could not be removed", Severity::Error, window, cx);
        }
        cx.notify();
    }

    // ---- the shared runs folder ----

    /// Ask before emptying the shared runs folder chosen in Settings.
    pub fn request_clear_runs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(root) = self.shared_runs_root() else {
            self.notify("Only a runs folder chosen in Settings can be cleared", Severity::Info, window, cx);
            return;
        };
        let count = runs::list_runs(&root, None).len();
        if count == 0 {
            self.notify(format!("{} has no runs to clear", root.display()), Severity::Info, window, cx);
            return;
        }
        self.dialog = Some(Dialog::ClearRuns { root, count });
        cx.notify();
    }

    pub fn dialog_clear_runs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(Dialog::ClearRuns { root, .. }) = self.dialog.take() else { return };
        match runs::clear_root(&root) {
            Ok(n) => {
                if self.run.as_ref().is_some_and(|r| r.root == root && !r.phase.active()) {
                    self.run = None;
                }
                self.refresh_history();
                self.notify(format!("Removed {n} run{} from {}", if n == 1 { "" } else { "s" }, root.display()), Severity::Info, window, cx);
            }
            Err(e) => self.notify(e, Severity::Error, window, cx),
        }
        cx.notify();
    }

    // ---- settings ----

    /// Keep run files next to each `.jmx` file (`false`) or in the folder chosen below (`true`).
    pub fn set_runs_custom(&mut self, custom: bool, cx: &mut Context<Self>) {
        self.settings.runs_custom = custom;
        self.save_settings();
        self.refresh_history();
        cx.notify();
    }

    /// Open the folder chooser. `then` is a run to start once a folder is chosen.
    pub fn choose_runs_folder(&mut self, window: &mut Window, cx: &mut Context<Self>, then: Option<(RunKind, Option<NodeId>)>) {
        let rx = cx.prompt_for_paths(PathPromptOptions { files: false, directories: true, multiple: false, prompt: Some("Choose where run files are kept".into()) });
        cx.spawn_in(window, async move |this, cx| {
            if let Ok(Ok(Some(paths))) = rx.await
                && let Some(folder) = paths.into_iter().next()
            {
                this.update_in(cx, |this, window, cx| {
                    this.set_runs_folder(&folder, cx);
                    if let Some((kind, sampler)) = then {
                        this.begin_run(kind, sampler, window, cx);
                    }
                })
                .ok();
            }
        })
        .detach();
    }

    /// Use `folder` as the parent of the shared `runs` folder: the app makes and uses a folder named `runs`
    /// inside it, so the choice is where that folder goes, never its name.
    pub fn set_runs_folder(&mut self, folder: &Path, cx: &mut Context<Self>) {
        self.settings.runs_folder = Some(plain(folder).to_string_lossy().into_owned());
        self.settings.runs_custom = true;
        self.save_settings();
        self.refresh_history();
        cx.notify();
    }

    /// Show a folder in the file manager.
    pub fn open_runs_folder(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let target = self.run.as_ref().map(|r| r.dir.clone()).filter(|d| d.is_dir()).or_else(|| self.runs_root_for_active().filter(|d| d.is_dir()));
        match target {
            Some(d) => open_in_file_manager(&d),
            None => self.notify("There are no run files yet", Severity::Info, window, cx),
        }
    }

    /// The runs shown in the History list, newest first.
    pub fn history_entries(&self) -> &[RunEntry] {
        &self.history
    }
}

/// Show a folder in the system file manager.
pub fn open_in_file_manager(dir: &Path) {
    if !dir.is_dir() {
        return;
    }
    let mut cmd = if cfg!(windows) {
        std::process::Command::new("explorer")
    } else if cfg!(target_os = "macos") {
        std::process::Command::new("open")
    } else {
        std::process::Command::new("xdg-open")
    };
    cmd.arg(dir);
    jvm_runtime::locate::quiet(&mut cmd);
    let _ = cmd.spawn();
}
