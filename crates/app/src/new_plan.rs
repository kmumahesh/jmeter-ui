//! Starting a new plan: File > New Plan makes an empty plan in a file and opens it, ready to edit and run.

use crate::shell::{Severity, Shell};
use gpui::{Context, Window};
use plan_core::files;
use plan_core::starters::{self, Starter};
use std::path::{Path, PathBuf};

impl Shell {
    /// File > New Plan: make an empty plan in a file of its own and open it, with no question. The file is named
    /// "New plan.jmx" (then "New plan 2.jmx" and so on) in the folder of the plan being edited, else the folder of the
    /// last plan opened, else the user's home folder. Save As moves it where the user wants.
    pub fn new_empty_plan(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let path = unique_new_plan_path(&self.new_plan_folder());
        self.create_plan_at(Starter::Empty, &path, window, cx)
    }

    /// Where a plan made without a file question starts: next to the plan being edited, else next to the last plan opened,
    /// else the user's home folder.
    pub fn new_plan_folder(&self) -> PathBuf {
        let beside_plan = self.plan().and_then(|lp| lp.path.parent().map(Path::to_path_buf));
        let recent = self.settings.recent_files.first().map(PathBuf::from);
        let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).map(PathBuf::from);
        pick_new_plan_folder(beside_plan, recent, home)
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

/// The folder a new plan is made in: next to the plan being edited, else next to the last plan opened, else the user's
/// home folder, else the temporary folder. Only folders that exist are used; none is made.
pub fn pick_new_plan_folder(beside_plan: Option<PathBuf>, recent: Option<PathBuf>, home: Option<PathBuf>) -> PathBuf {
    let existing = |p: Option<PathBuf>| p.filter(|d| d.is_dir());
    existing(beside_plan)
        .or_else(|| existing(recent.and_then(|p| p.parent().map(Path::to_path_buf))))
        .or_else(|| existing(home))
        .unwrap_or_else(std::env::temp_dir)
}

/// "New plan.jmx" in `dir`, or "New plan 2.jmx", "New plan 3.jmx"... for the first name nothing uses, so a new plan never
/// replaces a file.
pub fn unique_new_plan_path(dir: &Path) -> PathBuf {
    let first = dir.join("New plan.jmx");
    if !first.exists() {
        return first;
    }
    (2..10_000).map(|n| dir.join(format!("New plan {n}.jmx"))).find(|p| !p.exists()).unwrap_or_else(|| dir.join(format!("New plan {}.jmx", std::process::id())))
}
