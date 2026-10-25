//! Starting a new plan: choose a starter, choose where to save it, and the plan opens ready to edit and run.

use crate::shell::{Dialog, Severity, Shell};
use gpui::{Context, Window};
use plan_core::files;
use plan_core::starters::{self, Starter};
use std::path::{Path, PathBuf};

impl Shell {
    /// The starter was chosen in the New Plan question: ask where to save it.
    pub fn dialog_new_plan(&mut self, starter: Starter, window: &mut Window, cx: &mut Context<Self>) {
        if !matches!(self.dialog, Some(Dialog::NewPlan)) {
            return;
        }
        self.dialog = None;
        let dir = self.new_plan_folder();
        let rx = cx.prompt_for_new_path(&dir, Some(&format!("{}.jmx", starter.file_stem())));
        cx.spawn_in(window, async move |this, cx| {
            if let Ok(Ok(Some(path))) = rx.await {
                this.update_in(cx, |this, window, cx| {
                    this.create_plan_at(starter, &path, window, cx);
                })
                .ok();
            }
        })
        .detach();
        cx.notify();
    }

    /// Where the save question starts: next to the plan being edited, else next to the last plan opened, else the
    /// user's home folder.
    pub fn new_plan_folder(&self) -> PathBuf {
        let beside = |p: &Path| p.parent().filter(|d| d.is_dir()).map(Path::to_path_buf);
        self.plan()
            .and_then(|lp| beside(&lp.path))
            .or_else(|| self.settings.recent_files.first().and_then(|p| beside(Path::new(p))))
            .or_else(|| std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).map(PathBuf::from).filter(|d| d.is_dir()))
            .unwrap_or_else(std::env::temp_dir)
    }

    /// Write a new plan of the given kind to `path` (`.jmx` is added when there is no extension) and open it.
    /// The Test Plan is named after the file. Returns whether the plan was made.
    pub fn create_plan_at(&mut self, starter: Starter, path: &Path, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let mut path = path.to_path_buf();
        if path.extension().is_none() {
            path.set_extension("jmx");
        }
        let abs = std::fs::canonicalize(&path).unwrap_or_else(|_| path.clone());
        if self.plans.iter().any(|p| p.path == abs) {
            self.notify("That plan is open in a tab. Close it first or pick another name.", Severity::Error, window, cx);
            return false;
        }
        if path.is_dir() {
            self.notify(format!("{} is a folder. Pick a file name.", path.display()), Severity::Error, window, cx);
            return false;
        }
        let name = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        let text = match starters::new_plan_text(starter, &name) {
            Ok(t) => t,
            Err(e) => {
                self.notify(format!("Could not make the plan: {e:#}"), Severity::Error, window, cx);
                return false;
            }
        };
        if let Err(e) = files::save(&path, &text) {
            self.notify(format!("Could not save {}: {e}", path.display()), Severity::Error, window, cx);
            return false;
        }
        self.open_plan_path(&path, window, cx)
    }
}
