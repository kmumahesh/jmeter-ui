//! Changing plans: adding, removing and moving elements, undo, saving, the recovery copy, and the
//! questions asked before unsaved work could be lost.

use crate::shell::{Dialog, LoadedPlan, Severity, Shell, Then, display_path};
use anyhow::Context as _;
use gpui::{Context, Window};
use plan_core::{EditablePlan, NodeId, Plan, TreeState, files};
use shell_core::layout::{PlanId, TabKind};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// How long after an edit the recovery copy is written.
pub const AUTOSAVE_DELAY: Duration = Duration::from_secs(3);

/// Read a plan file for editing, with errors that name the file.
pub fn read_plan(path: &Path) -> anyhow::Result<EditablePlan> {
    let text = plan_core::files::read_text(path).with_context(|| format!("cannot read {}", path.display()))?;
    EditablePlan::open(&text).with_context(|| format!("cannot open {} as a JMeter plan", path.display()))
}

impl LoadedPlan {
    pub fn is_dirty(&self) -> bool {
        self.edit.is_dirty()
    }

    /// Take the plan the editor now holds, after any change.
    pub fn sync(&mut self) {
        self.plan = self.edit.plan().clone();
    }

    /// Notes about the structure of the plan (no thread group, an empty one, plugins it needs...).
    pub fn plan_notes(&self) -> std::rc::Rc<Vec<plan_core::problems::Problem>> {
        let mut slot = self.notes.borrow_mut();
        if let Some((v, notes)) = slot.as_ref()
            && *v == self.version
        {
            return notes.clone();
        }
        let notes = std::rc::Rc::new(plan_core::problems::check(&self.plan));
        *slot = Some((self.version, notes.clone()));
        notes
    }

    /// Everything worth a note: the structure of the plan, then variables nothing defines.
    pub fn problems(&self) -> std::rc::Rc<Vec<plan_core::problems::Problem>> {
        let mut slot = self.hints.borrow_mut();
        if let Some((v, hints)) = slot.as_ref()
            && *v == self.version
        {
            return hints.clone();
        }
        let mut all = self.plan_notes().to_vec();
        all.extend(plan_core::variables::check(&self.edit));
        let all = std::rc::Rc::new(all);
        *slot = Some((self.version, all.clone()));
        all
    }
}

impl Shell {
    // ---- opening ----

    /// Open a plan in its own tab, or switch to its tab if it is already open. If an earlier session left
    /// unsaved work for it, ask what to do with that first.
    pub fn open_plan_path(&mut self, path: &Path, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let abs = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        if let Some(existing) = self.plans.iter().find(|p| p.path == abs).map(|p| (p.id, p.name())) {
            self.show_plan(existing.0);
            self.settings.push_recent(&display_path(&abs));
            self.save_settings();
            self.notify(format!("{} is already open", existing.1), Severity::Info, window, cx);
            cx.notify();
            return true;
        }
        if let Some(rec) = files::find_recovery(&abs).filter(|r| r.newer) {
            let saved = std::fs::read_to_string(&abs).ok();
            self.dialog = Some(Dialog::Recover { path: abs, saved, recovered: rec.text });
            cx.notify();
            return true;
        }
        match read_plan(path) {
            Ok(edit) => {
                self.finish_open(abs, edit, window, cx);
                true
            }
            Err(e) => {
                self.settings.remove_recent(&display_path(path));
                self.save_settings();
                self.notify(format!("{e:#}"), Severity::Error, window, cx);
                false
            }
        }
    }

    pub(crate) fn finish_open(&mut self, abs: PathBuf, edit: EditablePlan, window: &mut Window, cx: &mut Context<Self>) {
        let plan = edit.plan().clone();
        let n = plan.len();
        let tree = TreeState::expanded_to_depth(&plan, 2);
        let id = self.new_plan_id();
        let dirty = edit.is_dirty();
        let lp = LoadedPlan { id, path: abs.clone(), plan, edit, tree, version: 0, hints: Default::default(), notes: Default::default() };
        let title = lp.name();
        self.plans.push(lp);
        self.workspace.open_tab(TabKind::Plan(id), &title);
        self.workspace.set_dirty(TabKind::Plan(id), dirty);
        self.active_plan = Some(id);
        self.refresh_history();
        self.settings.push_recent(&display_path(&abs));
        self.save_settings();
        let msg = if dirty { format!("Opened {title} with your unsaved changes: {n} elements") } else { format!("Opened {title}: {n} elements") };
        self.notify(msg, Severity::Info, window, cx);
        cx.notify();
    }

    /// The answer to the "unsaved work was found" question.
    pub fn resolve_recovery(&mut self, recover: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(Dialog::Recover { path, saved, recovered }) = self.dialog.take() else { return };
        if recover {
            match EditablePlan::open_recovered(&recovered) {
                Ok(edit) => self.finish_open(path, edit, window, cx),
                Err(e) => self.notify(format!("The recovered copy cannot be opened: {e:#}"), Severity::Error, window, cx),
            }
        } else {
            files::discard_recovery(&path);
            match saved {
                Some(_) => {
                    self.open_plan_path(&path, window, cx);
                }
                None => self.notify("The unsaved copy was discarded. The plan file itself no longer exists.", Severity::Info, window, cx),
            }
        }
        cx.notify();
    }

    /// Read the active plan again from disk. Unsaved edits are replaced, so this asks first when there are any.
    pub fn reload_plan(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(lp) = self.plan() else { return };
        if lp.is_dirty() {
            self.dialog = Some(Dialog::DiscardForReload(lp.id));
            cx.notify();
            return;
        }
        self.do_reload(window, cx);
    }

    /// Reload, keeping the selection where the same names still exist.
    pub fn do_reload(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(lp) = self.plan() else { return };
        let (id, path) = (lp.id, lp.path.clone());
        let selected = lp.tree.selected().map(|s| lp.plan.breadcrumb(s));
        let edit = match read_plan(&path) {
            Ok(p) => p,
            Err(e) => {
                self.notify(format!("{e:#}"), Severity::Error, window, cx);
                return;
            }
        };
        files::discard_recovery(&path);
        let n = edit.plan().len();
        let tree = TreeState::expanded_to_depth(edit.plan(), 2);
        let Some(lp) = self.plans.iter_mut().find(|p| p.id == id) else { return };
        lp.edit = edit;
        lp.sync();
        lp.tree = tree;
        lp.version += 1;
        let plan = lp.plan.clone();
        let find = |crumbs: &Vec<String>| plan.iter().find(|n| plan.breadcrumb(n.id) == *crumbs).map(|n| n.id);
        if let Some(sel) = selected.as_ref().and_then(find) {
            lp.tree.select(&plan, sel);
        }
        let name = lp.name();
        self.workspace.set_dirty(TabKind::Plan(id), false);
        self.reset_form();
        self.notify(format!("Reloaded {name}: {n} elements"), Severity::Info, window, cx);
        cx.notify();
    }

    // ---- what changed ----

    /// Run an edit on the active plan. The closure returns the element to select afterwards. Errors are shown
    /// to the user and change nothing.
    pub(crate) fn edit_active(&mut self, window: &mut Window, cx: &mut Context<Self>, f: impl FnOnce(&mut EditablePlan, &Plan) -> anyhow::Result<Option<NodeId>>) -> bool {
        let Some(lp) = self.plan_mut() else { return false };
        let plan = lp.plan.clone();
        match f(&mut lp.edit, &plan) {
            Ok(select) => {
                self.after_edit(select, window, cx);
                true
            }
            Err(e) => {
                self.notify(format!("{e:#}"), Severity::Error, window, cx);
                false
            }
        }
    }

    /// Bring the window up to date after the plan changed: the tree, the tab's unsaved mark, and the
    /// recovery copy.
    pub(crate) fn after_edit(&mut self, select: Option<NodeId>, _window: &mut Window, cx: &mut Context<Self>) {
        let Some(lp) = self.plan_mut() else { return };
        lp.sync();
        lp.version += 1;
        let (id, version, dirty, path) = (lp.id, lp.version, lp.is_dirty(), lp.path.clone());
        let plan = lp.plan.clone();
        match select.filter(|&s| plan.node(s).is_some()) {
            Some(s) => lp.tree.select(&plan, s),
            None => {
                // The selected element may be gone.
                if lp.tree.selected().is_some_and(|s| plan.node(s).is_none()) {
                    lp.tree.clear_selection();
                }
            }
        }
        let row = lp.tree.selected().and_then(|s| lp.tree.row_index(&plan, s));
        self.workspace.set_dirty(TabKind::Plan(id), dirty);
        if let Some(i) = row {
            self.tree_scroll.scroll_to_item(i, gpui::ScrollStrategy::Nearest);
        }
        if dirty {
            self.schedule_autosave(id, version, cx);
        } else {
            files::discard_recovery(&path);
        }
        self.reset_form();
        self.refresh_dirty_mark(cx);
    }

    /// Write the recovery copy a few seconds after the last edit.
    fn schedule_autosave(&mut self, id: PlanId, version: u64, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(AUTOSAVE_DELAY).await;
            this.update(cx, |this, cx| {
                let Some(lp) = this.plans.iter().find(|p| p.id == id) else { return };
                if lp.version != version || !lp.is_dirty() {
                    return;
                }
                if let Err(e) = files::autosave(&lp.path, &lp.edit.to_text()) {
                    this.notify_cx(format!("Could not write the recovery copy: {e}"), Severity::Error, cx);
                }
            })
            .ok();
        })
        .detach();
    }

    // ---- commands on the selected element ----

    pub(crate) fn selected_id(&self) -> Option<NodeId> {
        self.plan().and_then(|lp| lp.tree.selected())
    }

    /// Add an element of a type from the Add lists as the last child of the selected element (or of the
    /// Test Plan when nothing is selected). The new element becomes the selection.
    pub fn add_element(&mut self, tag: &str, gui: &str, window: &mut Window, cx: &mut Context<Self>) {
        let (tag, gui) = (tag.to_string(), gui.to_string());
        let sel = self.selected_id();
        let mut added = None;
        self.edit_active(window, cx, |e, plan| {
            let parent = sel.filter(|&s| plan.node(s).is_some()).or_else(|| plan.roots().first().copied()).context("there is no element to add to")?;
            let id = e.add(parent, &tag, &gui)?;
            added = Some(id);
            Ok(Some(id))
        });
        if let Some(id) = added
            && let Some(n) = self.plan().and_then(|lp| lp.plan.node(id))
        {
            let msg = format!("Added {}", n.name);
            self.notify(msg, Severity::Info, window, cx);
        }
    }

    pub fn delete_selected(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(id) = self.selected_id() else { return };
        let mut name = String::new();
        let done = self.edit_active(window, cx, |e, plan| {
            let n = plan.node(id).context("that element is no longer in the plan")?;
            name = n.name.clone();
            // After deleting, select the next sibling, else the previous one, else the parent.
            let mut next = None;
            if let Some(parent) = n.parent.and_then(|p| plan.node(p)) {
                let at = parent.children.iter().position(|&c| c == id).unwrap_or(0);
                next = parent.children.get(at + 1).or_else(|| at.checked_sub(1).and_then(|i| parent.children.get(i))).copied().or(Some(parent.id));
            }
            e.remove(id)?;
            Ok(next)
        });
        if done {
            self.notify(format!("Deleted {name}. Ctrl+Z brings it back."), Severity::Info, window, cx);
        }
    }

    pub fn duplicate_selected(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(id) = self.selected_id() else { return };
        self.edit_active(window, cx, |e, _| Ok(Some(e.duplicate(id)?)));
    }

    /// Move the selected element one place up or down among its siblings.
    pub fn move_selected(&mut self, down: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(id) = self.selected_id() else { return };
        self.edit_active(window, cx, |e, _| {
            e.nudge(id, down)?;
            Ok(Some(id))
        });
    }

    /// Move an element somewhere else in the tree, as when it is dropped there.
    pub fn move_element(&mut self, id: NodeId, new_parent: NodeId, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.edit_active(window, cx, |e, _| {
            e.move_to(id, new_parent, index)?;
            Ok(Some(id))
        });
    }

    /// An element was dropped on a row of the tree. Dropping onto a row makes it the last child of that row's
    /// element; dropping above or below puts it next to that element.
    pub fn drop_node(&mut self, drag: &crate::shell::TreeDrag, target: NodeId, zone: crate::shell::DropZone, window: &mut Window, cx: &mut Context<Self>) {
        use crate::shell::DropZone;
        let Some(lp) = self.plan() else { return };
        if lp.id != drag.plan || drag.node == target {
            return;
        }
        let plan = lp.plan.clone();
        let (Some(t), Some(d)) = (plan.node(target), plan.node(drag.node)) else { return };
        let (parent, index) = match zone {
            DropZone::Into => (target, t.children.len()),
            DropZone::Before | DropZone::After => {
                let Some(parent) = t.parent.and_then(|p| plan.node(p)) else {
                    self.notify("Nothing can go next to the Test Plan itself.", Severity::Error, window, cx);
                    return;
                };
                let mut at = parent.children.iter().position(|&c| c == target).unwrap_or(0) + usize::from(zone == DropZone::After);
                // The element leaves its old place first, which shifts everything after it.
                if let Some(old) = parent.children.iter().position(|&c| c == drag.node)
                    && old < at
                {
                    at -= 1;
                }
                (parent.id, at)
            }
        };
        if !plan.can_move(drag.node, parent) {
            let p = plan.node(parent).map(|p| p.type_label.clone()).unwrap_or_default();
            self.notify(format!("{} cannot go under {p}.", d.type_label), Severity::Error, window, cx);
            return;
        }
        self.move_element(drag.node, parent, index, window, cx);
    }

    pub fn toggle_selected_enabled(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(id) = self.selected_id() else { return };
        self.edit_active(window, cx, |e, plan| {
            let on = plan.node(id).map(|n| n.enabled).unwrap_or(true);
            e.set_enabled(id, !on)?;
            Ok(Some(id))
        });
    }

    pub fn rename_element(&mut self, id: NodeId, name: &str, window: &mut Window, cx: &mut Context<Self>) {
        let name = name.to_string();
        self.edit_active(window, cx, |e, plan| {
            if plan.node(id).is_some_and(|n| n.name == name) {
                return Ok(None); // nothing changed
            }
            e.rename(id, &name)?;
            Ok(Some(id))
        });
    }

    pub fn set_element_prop(&mut self, id: NodeId, name: &str, kind: &str, value: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.edit_active(window, cx, |e, _| {
            e.set_prop(id, name, kind, value)?;
            Ok(Some(id))
        });
    }

    pub fn undo(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.edit_active(window, cx, |e, _| Ok(e.undo()?.and_then(|o| o.select)));
    }

    pub fn redo(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.edit_active(window, cx, |e, _| Ok(e.redo()?.and_then(|o| o.select)));
    }

    // ---- saving ----

    /// Write a plan to its file. Returns whether it worked.
    pub fn save_plan_id(&mut self, id: PlanId, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let Some(lp) = self.plans.iter_mut().find(|p| p.id == id) else { return false };
        let text = lp.edit.to_text();
        let name = lp.name();
        match files::save(&lp.path, &text) {
            Ok(()) => {
                lp.edit.mark_saved();
                self.workspace.set_dirty(TabKind::Plan(id), false);
                self.notify(format!("Saved {name}"), Severity::Info, window, cx);
                cx.notify();
                true
            }
            Err(e) => {
                self.notify(format!("Could not save {name}: {e}"), Severity::Error, window, cx);
                false
            }
        }
    }

    pub fn save_active(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        match self.active_plan {
            Some(id) => self.save_plan_id(id, window, cx),
            None => false,
        }
    }

    /// Ask for a new file name and save the active plan there.
    pub fn save_active_as(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(lp) = self.plan() else { return };
        let (id, dir, name) = (lp.id, lp.path.parent().map(Path::to_path_buf).unwrap_or_default(), lp.name());
        let rx = cx.prompt_for_new_path(&dir, Some(&name));
        cx.spawn_in(window, async move |this, cx| {
            if let Ok(Ok(Some(path))) = rx.await {
                this.update_in(cx, |this, window, cx| this.save_plan_as_path(id, &path, window, cx)).ok();
            }
        })
        .detach();
    }

    pub fn save_plan_as_path(&mut self, id: PlanId, path: &Path, window: &mut Window, cx: &mut Context<Self>) {
        let mut path = path.to_path_buf();
        if path.extension().is_none() {
            path.set_extension("jmx");
        }
        let abs = std::fs::canonicalize(&path).unwrap_or_else(|_| path.clone());
        if self.plans.iter().any(|p| p.id != id && p.path == abs) {
            self.notify("That plan is open in another tab. Close it first or pick another name.", Severity::Error, window, cx);
            return;
        }
        let Some(lp) = self.plans.iter_mut().find(|p| p.id == id) else { return };
        let old = lp.path.clone();
        let text = lp.edit.to_text();
        if let Err(e) = files::save(&path, &text) {
            self.notify(format!("Could not save {}: {e}", path.display()), Severity::Error, window, cx);
            return;
        }
        let abs = std::fs::canonicalize(&path).unwrap_or(path);
        if old != abs {
            files::discard_recovery(&old);
        }
        let Some(lp) = self.plans.iter_mut().find(|p| p.id == id) else { return };
        lp.edit.mark_saved();
        lp.path = abs.clone();
        let name = lp.name();
        self.workspace.rename_tab(TabKind::Plan(id), &name);
        self.workspace.set_dirty(TabKind::Plan(id), false);
        self.settings.push_recent(&display_path(&abs));
        self.save_settings();
        self.notify(format!("Saved as {name}"), Severity::Info, window, cx);
        cx.notify();
    }

    // ---- leaving with unsaved work ----

    pub fn dirty_plans(&self) -> Vec<PlanId> {
        self.plans.iter().filter(|p| p.is_dirty()).map(|p| p.id).collect()
    }

    /// Close one tab, asking first if it is a plan with unsaved changes.
    pub fn request_close_tab(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.flush_form(window, cx);
        let Some(kind) = self.workspace.tabs().get(index).map(|t| t.kind) else { return };
        if let TabKind::Plan(id) = kind
            && self.plans.iter().any(|p| p.id == id && p.is_dirty())
        {
            self.dialog = Some(Dialog::Unsaved { plans: vec![id], then: Then::CloseTab(id) });
            cx.notify();
            return;
        }
        self.close_tab(index, cx);
    }

    pub fn request_close_all(&mut self, cx: &mut Context<Self>) {
        let dirty = self.dirty_plans();
        if dirty.is_empty() {
            self.close_all_tabs(cx);
        } else {
            self.dialog = Some(Dialog::Unsaved { plans: dirty, then: Then::CloseAll });
            cx.notify();
        }
    }

    /// Whether the window may close now. If plans have unsaved changes, asks first and returns false.
    pub fn request_quit(&mut self, cx: &mut Context<Self>) -> bool {
        if self.run_in_progress() {
            self.dialog = Some(Dialog::QuitRunning);
            cx.notify();
            return false;
        }
        let dirty = self.dirty_plans();
        if dirty.is_empty() {
            return true;
        }
        self.dialog = Some(Dialog::Unsaved { plans: dirty, then: Then::Quit });
        cx.notify();
        false
    }

    /// "Save" in the unsaved-changes question.
    pub fn dialog_save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(Dialog::Unsaved { plans, then }) = self.dialog.clone() else { return };
        for id in &plans {
            if !self.save_plan_id(*id, window, cx) {
                return; // the error is showing; stay where we are
            }
        }
        self.dialog = None;
        self.finish_leaving(then, window, cx);
    }

    /// "Don't save" in the unsaved-changes question.
    pub fn dialog_discard(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(Dialog::Unsaved { plans, then }) = self.dialog.take() else { return };
        for id in plans {
            if let Some(lp) = self.plans.iter_mut().find(|p| p.id == id) {
                files::discard_recovery(&lp.path);
                lp.edit.mark_saved(); // so closing does not ask again
            }
        }
        self.finish_leaving(then, window, cx);
    }

    pub fn dialog_cancel(&mut self, cx: &mut Context<Self>) {
        self.dialog = None;
        cx.notify();
    }

    fn finish_leaving(&mut self, then: Then, window: &mut Window, cx: &mut Context<Self>) {
        match then {
            Then::CloseTab(id) => {
                if let Some(i) = self.workspace.find(TabKind::Plan(id)) {
                    self.close_tab(i, cx);
                }
            }
            Then::CloseAll => self.close_all_tabs(cx),
            Then::Quit => {
                self.save_window_state(window);
                cx.quit();
            }
        }
        cx.notify();
    }

    /// "Discard my changes and reload" in the reload question.
    pub fn dialog_reload(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if matches!(self.dialog, Some(Dialog::DiscardForReload(_))) {
            self.dialog = None;
            self.do_reload(window, cx);
        }
    }
}
