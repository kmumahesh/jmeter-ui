//! Editing a plan: every change is a step that can be undone and redone, and the plan text is only
//! touched where the change is.

use crate::catalog::Category;
use crate::model::{NodeId, Plan};
use crate::rules;
use crate::templates;
use anyhow::{Context, Result, bail};
use jmx_model::{Document, Element, Head, PropOld, Removed, RemovedRow};
use std::sync::Arc;

/// Where an element sits: its parent (`None` for the top level) and its position among the siblings.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Place {
    pub parent: Option<u32>,
    pub index: usize,
}

/// One change, in the state it is in now. `undo` and `redo` flip it.
#[derive(Debug)]
enum Step {
    /// An element was added. While undone, `held` keeps it.
    Add { uid: u32, held: Option<Removed> },
    /// An element was removed. While applied, `held` keeps it.
    Remove { uid: u32, held: Option<Removed> },
    /// An element was moved to `to`; `from` is what it was taken out of.
    Move { uid: u32, to: Place, from: Removed },
    /// The start tag of an element changed (its name or whether it is enabled).
    Attr { uid: u32, old: Head, new: Head },
    /// A simple property was set.
    Prop { uid: u32, name: String, kind: String, old: PropOld, new: String },
    /// A table row was added. While undone, `held` keeps it.
    RowAdd { uid: u32, table: String, index: usize, held: Option<RemovedRow> },
    /// A table row was removed. While applied, `held` keeps it.
    RowRemove { uid: u32, table: String, index: usize, held: Option<RemovedRow> },
    /// A table row was replaced by `new`; `old` is what it was.
    RowReplace { uid: u32, table: String, index: usize, old: RemovedRow, new: Element },
}

/// What the app should show after an undo or redo.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Outcome {
    pub select: Option<NodeId>,
}

impl Step {
    fn undo(&mut self, doc: &mut Document) -> Result<Outcome> {
        let select = match self {
            Step::Add { uid, held } => {
                let r = doc.remove_pair(*uid)?;
                let parent = r.parent;
                *held = Some(r);
                parent
            }
            Step::Remove { uid, held } => {
                doc.restore(held.take().context("nothing to restore")?)?;
                Some(*uid)
            }
            Step::Move { uid, from, .. } => {
                doc.remove_pair(*uid)?;
                doc.restore(from.clone())?;
                Some(*uid)
            }
            Step::Attr { uid, old, .. } => {
                doc.restore_head(*uid, old.clone())?;
                Some(*uid)
            }
            Step::Prop { uid, name, old, .. } => {
                doc.restore_prop(*uid, name, old.clone())?;
                Some(*uid)
            }
            Step::RowAdd { uid, table, index, held } => {
                *held = Some(doc.remove_row(*uid, table, *index)?);
                Some(*uid)
            }
            Step::RowRemove { uid, held, .. } => {
                doc.restore_row(held.take().context("nothing to restore")?)?;
                Some(*uid)
            }
            Step::RowReplace { uid, table, index, old, .. } => {
                doc.remove_row(*uid, table, *index)?;
                doc.restore_row(old.clone())?;
                Some(*uid)
            }
        };
        Ok(Outcome { select: select.map(NodeId) })
    }

    fn redo(&mut self, doc: &mut Document) -> Result<Outcome> {
        let select = match self {
            Step::Add { uid, held } => {
                doc.restore(held.take().context("nothing to restore")?)?;
                Some(*uid)
            }
            Step::Remove { uid, held } => {
                let r = doc.remove_pair(*uid)?;
                let parent = r.parent;
                *held = Some(r);
                parent
            }
            Step::Move { uid, to, from } => {
                let r = doc.remove_pair(*uid)?;
                doc.insert_pair(to.parent, to.index, fresh_place(&r))?;
                *from = r;
                Some(*uid)
            }
            Step::Attr { uid, new, .. } => {
                doc.restore_head(*uid, new.clone())?;
                Some(*uid)
            }
            Step::Prop { uid, name, kind, old, new } => {
                *old = doc.set_prop(*uid, name, kind, new)?;
                Some(*uid)
            }
            Step::RowAdd { uid, held, .. } => {
                doc.restore_row(held.take().context("nothing to restore")?)?;
                Some(*uid)
            }
            Step::RowRemove { uid, table, index, held } => {
                *held = Some(doc.remove_row(*uid, table, *index)?);
                Some(*uid)
            }
            Step::RowReplace { uid, table, index, old, new } => {
                *old = doc.remove_row(*uid, table, *index)?;
                doc.insert_row(*uid, table, *index, new.clone())?;
                Some(*uid)
            }
        };
        Ok(Outcome { select: select.map(NodeId) })
    }
}

/// The removed pair as it should be written somewhere else: its old white space is dropped, so the
/// new place decides the indentation.
fn fresh_place(r: &Removed) -> jmx_model::Pair {
    let mut p = r.pair.clone();
    p.lead = None;
    p.mid = None;
    p
}

/// A plan open for editing: the text of the file, the plan read from it, and the undo history.
pub struct EditablePlan {
    doc: Document,
    plan: Arc<Plan>,
    undo: Vec<Step>,
    redo: Vec<Step>,
    /// Length of the undo history when the plan was last saved or opened; `None` once that state is gone.
    saved_at: Option<usize>,
}

const HISTORY_LIMIT: usize = 500;

impl EditablePlan {
    pub fn open(src: &str) -> Result<EditablePlan> {
        let mut doc = Document::parse(src)?;
        doc.index_plan();
        let plan = Arc::new(Plan::from_document(&doc)?);
        Ok(EditablePlan { doc, plan, undo: Vec::new(), redo: Vec::new(), saved_at: Some(0) })
    }

    /// Open text that is a recovered copy of unsaved work: the plan counts as having unsaved changes.
    pub fn open_recovered(src: &str) -> Result<EditablePlan> {
        let mut e = EditablePlan::open(src)?;
        e.saved_at = None;
        Ok(e)
    }

    /// A copy of the plan as it is now, with no undo history, for making a changed plan to run without
    /// touching this one. Elements keep their ids.
    pub fn fork(&self) -> EditablePlan {
        EditablePlan { doc: self.doc.clone(), plan: self.plan.clone(), undo: Vec::new(), redo: Vec::new(), saved_at: Some(0) }
    }

    pub fn plan(&self) -> &Arc<Plan> {
        &self.plan
    }

    /// The plan as text, ready to write to the file.
    pub fn to_text(&self) -> String {
        self.doc.write()
    }

    pub fn is_dirty(&self) -> bool {
        self.saved_at != Some(self.undo.len())
    }

    /// Call after the text was written to disk.
    pub fn mark_saved(&mut self) {
        self.saved_at = Some(self.undo.len());
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    fn refresh(&mut self) -> Result<()> {
        self.plan = Arc::new(Plan::from_document(&self.doc)?);
        Ok(())
    }

    fn record(&mut self, step: Step) -> Result<()> {
        if self.saved_at.is_some_and(|s| s > self.undo.len()) {
            self.saved_at = None; // the saved state was in the part of the history that is now gone
        }
        self.redo.clear();
        self.undo.push(step);
        if self.undo.len() > HISTORY_LIMIT {
            self.undo.remove(0);
            self.saved_at = match self.saved_at {
                Some(0) | None => None,
                Some(n) => Some(n - 1),
            };
        }
        self.refresh()
    }

    pub fn undo(&mut self) -> Result<Option<Outcome>> {
        let Some(mut step) = self.undo.pop() else { return Ok(None) };
        let out = step.undo(&mut self.doc)?;
        self.redo.push(step);
        self.refresh()?;
        Ok(Some(out))
    }

    pub fn redo(&mut self) -> Result<Option<Outcome>> {
        let Some(mut step) = self.redo.pop() else { return Ok(None) };
        let out = step.redo(&mut self.doc)?;
        self.undo.push(step);
        self.refresh()?;
        Ok(Some(out))
    }

    fn node(&self, id: NodeId) -> Result<&crate::model::Node> {
        self.plan.node(id).context("that element is no longer in the plan")
    }

    /// Where an element is now.
    pub fn place(&self, id: NodeId) -> Option<Place> {
        self.doc.plan_position(id.0).map(|(parent, index)| Place { parent, index })
    }

    // ---- structure ----

    /// Add a new element of the given type (its XML tag and `guiclass`) as the last child of `parent`.
    pub fn add(&mut self, parent: NodeId, tag: &str, guiclass: &str) -> Result<NodeId> {
        let n = self.node(parent)?.children.len();
        self.add_at(parent, n, tag, guiclass)
    }

    pub fn add_at(&mut self, parent: NodeId, index: usize, tag: &str, guiclass: &str) -> Result<NodeId> {
        let tpl = templates::find(tag, guiclass).with_context(|| format!("unknown element type {tag}"))?;
        let p = self.node(parent)?;
        if !rules::can_add(p.category, tpl.category) {
            bail!("{} cannot hold a {}", p.type_label, tpl.label);
        }
        let mut pair = tpl.instantiate();
        self.doc.assign_uids(&mut pair);
        let uid = pair.uid();
        self.doc.insert_pair(Some(parent.0), index, pair)?;
        self.record(Step::Add { uid, held: None })?;
        Ok(NodeId(uid))
    }

    /// Remove an element and everything under it. The Test Plan itself cannot be removed.
    pub fn remove(&mut self, id: NodeId) -> Result<()> {
        if self.node(id)?.category == Category::TestPlan {
            bail!("the Test Plan cannot be removed");
        }
        let r = self.doc.remove_pair(id.0)?;
        self.record(Step::Remove { uid: id.0, held: Some(r) })
    }

    /// Copy an element and everything under it, right after the original.
    pub fn duplicate(&mut self, id: NodeId) -> Result<NodeId> {
        let n = self.node(id)?;
        if n.category == Category::TestPlan {
            bail!("the Test Plan cannot be duplicated");
        }
        let place = self.place(id).context("that element is no longer in the plan")?;
        let mut pair = self.doc.copy_pair(id.0).context("that element is no longer in the plan")?;
        self.doc.assign_uids(&mut pair);
        let uid = pair.uid();
        self.doc.insert_pair(place.parent, place.index + 1, pair)?;
        self.record(Step::Add { uid, held: None })?;
        Ok(NodeId(uid))
    }

    /// Move an element under `new_parent` at `index` among that parent's other children.
    pub fn move_to(&mut self, id: NodeId, new_parent: NodeId, index: usize) -> Result<()> {
        if !self.plan.can_move(id, new_parent) {
            bail!("that element cannot go there");
        }
        let from = self.place(id).context("that element is no longer in the plan")?;
        if from.parent == Some(new_parent.0) && from.index == index {
            return Ok(());
        }
        let r = self.doc.remove_pair(id.0)?;
        let to = Place { parent: Some(new_parent.0), index };
        if let Err(e) = self.doc.insert_pair(to.parent, to.index, fresh_place(&r)) {
            self.doc.restore(r)?;
            return Err(e);
        }
        self.record(Step::Move { uid: id.0, to, from: r })
    }

    /// Move an element one place up or down among its siblings. Returns whether it moved.
    pub fn nudge(&mut self, id: NodeId, down: bool) -> Result<bool> {
        let place = self.place(id).context("that element is no longer in the plan")?;
        let Some(parent) = place.parent else { return Ok(false) };
        let siblings = self.node(NodeId(parent))?.children.len();
        let target = if down { place.index + 1 } else { place.index.wrapping_sub(1) };
        if target >= siblings {
            return Ok(false);
        }
        self.move_to(id, NodeId(parent), target)?;
        Ok(true)
    }

    // ---- settings ----

    pub fn rename(&mut self, id: NodeId, name: &str) -> Result<()> {
        self.set_attr(id, "testname", name)
    }

    pub fn set_enabled(&mut self, id: NodeId, enabled: bool) -> Result<()> {
        self.set_attr(id, "enabled", if enabled { "true" } else { "false" })
    }

    fn set_attr(&mut self, id: NodeId, name: &str, value: &str) -> Result<()> {
        let old = self.doc.set_element_attr(id.0, name, value)?;
        let new = self.doc.plan_element(id.0).context("that element is no longer in the plan")?.head();
        self.record(Step::Attr { uid: id.0, old, new })
    }

    /// Set a simple property such as `HTTPSampler.path`; `kind` (for example `stringProp`) is used if the
    /// element does not have it yet.
    pub fn set_prop(&mut self, id: NodeId, name: &str, kind: &str, value: &str) -> Result<()> {
        if self.doc.prop(id.0, name).as_deref() == Some(value) {
            return Ok(()); // nothing changed
        }
        let old = self.doc.set_prop(id.0, name, kind, value)?;
        self.record(Step::Prop { uid: id.0, name: name.to_string(), kind: kind.to_string(), old, new: value.to_string() })
    }

    // ---- tables ----

    /// Add a row to a table, for example `HTTPsampler.Arguments/Arguments.arguments`, at `index` (clamped).
    pub fn add_row(&mut self, id: NodeId, table: &str, index: usize, row: Element) -> Result<()> {
        let index = index.min(self.doc.row_count(id.0, table).context("no such table")?);
        self.doc.insert_row(id.0, table, index, row)?;
        self.record(Step::RowAdd { uid: id.0, table: table.to_string(), index, held: None })
    }

    pub fn remove_row(&mut self, id: NodeId, table: &str, index: usize) -> Result<()> {
        let r = self.doc.remove_row(id.0, table, index)?;
        self.record(Step::RowRemove { uid: id.0, table: table.to_string(), index, held: Some(r) })
    }

    /// Swap a row for another, as one change.
    pub fn replace_row(&mut self, id: NodeId, table: &str, index: usize, row: Element) -> Result<()> {
        let old = self.doc.remove_row(id.0, table, index)?;
        if let Err(e) = self.doc.insert_row(id.0, table, index, row.clone()) {
            self.doc.restore_row(old)?;
            return Err(e);
        }
        self.record(Step::RowReplace { uid: id.0, table: table.to_string(), index, old, new: row })
    }

    pub fn row_count(&self, id: NodeId, table: &str) -> usize {
        self.doc.row_count(id.0, table).unwrap_or(0)
    }

    /// A row's simple settings as (name, value).
    pub fn row_cells(&self, id: NodeId, table: &str, index: usize) -> Vec<(String, String)> {
        self.doc.row_cells(id.0, table, index).unwrap_or_default()
    }

    /// The text of a row that is a plain string.
    pub fn row_text(&self, id: NodeId, table: &str, index: usize) -> String {
        self.doc.row_text(id.0, table, index).unwrap_or_default()
    }

    /// Every simple setting of an element with its text (see `Document::setting_texts`).
    pub fn setting_texts(&self, id: NodeId) -> Vec<(String, String)> {
        self.doc.setting_texts(id.0)
    }

    /// The current value of a simple property.
    pub fn prop(&self, id: NodeId, name: &str) -> Option<String> {
        self.doc.prop(id.0, name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SRC: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<jmeterTestPlan version=\"1.2\" properties=\"5.0\" jmeter=\"5.6.3\">\n  <hashTree>\n    <TestPlan guiclass=\"TestPlanGui\" testclass=\"TestPlan\" testname=\"Shop\" enabled=\"true\">\n      <stringProp name=\"TestPlan.comments\">notes</stringProp>\n    </TestPlan>\n    <hashTree>\n      <ThreadGroup guiclass=\"ThreadGroupGui\" testclass=\"ThreadGroup\" testname=\"Users\" enabled=\"true\"/>\n      <hashTree>\n        <HTTPSamplerProxy guiclass=\"HttpTestSampleGui\" testclass=\"HTTPSamplerProxy\" testname=\"Home\" enabled=\"true\"/>\n        <hashTree/>\n        <HTTPSamplerProxy guiclass=\"HttpTestSampleGui\" testclass=\"HTTPSamplerProxy\" testname=\"Cart\" enabled=\"true\"/>\n        <hashTree/>\n      </hashTree>\n    </hashTree>\n  </hashTree>\n</jmeterTestPlan>\n";

    fn open() -> EditablePlan {
        EditablePlan::open(SRC).unwrap()
    }

    fn id(e: &EditablePlan, name: &str) -> NodeId {
        e.plan().find_by_name(name).unwrap_or_else(|| panic!("no {name}"))
    }

    fn names(e: &EditablePlan, parent: &str) -> Vec<String> {
        e.plan().node(id(e, parent)).unwrap().children.iter().map(|&c| e.plan().node(c).unwrap().name.clone()).collect()
    }

    #[test]
    fn a_fresh_plan_is_clean_and_writes_back_unchanged() {
        let e = open();
        assert!(!e.is_dirty() && !e.can_undo() && !e.can_redo());
        assert_eq!(e.to_text(), SRC);
    }

    #[test]
    fn adding_puts_the_element_last_and_gives_it_jmeters_defaults() {
        let mut e = open();
        let users = id(&e, "Users");
        let new = e.add(users, "ConstantTimer", "ConstantTimerGui").unwrap();
        assert_eq!(names(&e, "Users"), ["Home", "Cart", "Constant Timer"]);
        assert_eq!(e.plan().node(new).unwrap().category, Category::Timer);
        assert!(e.is_dirty());
        let text = e.to_text();
        assert!(text.contains("<ConstantTimer guiclass=\"ConstantTimerGui\" testclass=\"ConstantTimer\" testname=\"Constant Timer\" enabled=\"true\">"), "{text}");
        assert!(Plan::from_jmx_str(&text).is_ok(), "the result is still a plan");
    }

    #[test]
    fn the_rules_decide_what_can_be_added_where() {
        let mut e = open();
        let home = id(&e, "Home");
        assert!(e.add(home, "ThreadGroup", "ThreadGroupGui").is_err(), "no thread group under a sampler");
        assert!(e.add(id(&e, "Shop"), "ThreadGroup", "ThreadGroupGui").is_ok());
        assert!(e.add(home, "NoSuchThing", "x").is_err());
        assert!(!e.can_redo());
    }

    #[test]
    fn undo_and_redo_restore_the_text_exactly() {
        let mut e = open();
        let users = id(&e, "Users");
        e.add(users, "ConstantTimer", "ConstantTimerGui").unwrap();
        e.rename(id(&e, "Home"), "Start & go").unwrap();
        e.set_enabled(id(&e, "Cart"), false).unwrap();
        e.set_prop(id(&e, "Shop"), "TestPlan.comments", "stringProp", "new notes").unwrap();
        let edited = e.to_text();
        assert_ne!(edited, SRC);
        while e.can_undo() {
            e.undo().unwrap();
        }
        assert_eq!(e.to_text(), SRC, "undoing everything gives the file back byte for byte");
        assert!(!e.is_dirty());
        while e.can_redo() {
            e.redo().unwrap();
        }
        assert_eq!(e.to_text(), edited, "redoing everything gives the edits back");
        assert!(e.is_dirty());
    }

    #[test]
    fn removing_and_undoing_keeps_the_same_ids() {
        let mut e = open();
        let cart = id(&e, "Cart");
        e.remove(cart).unwrap();
        assert_eq!(names(&e, "Users"), ["Home"]);
        assert!(e.plan().node(cart).is_none());
        let out = e.undo().unwrap().unwrap();
        assert_eq!(out.select, Some(cart), "the restored element is the one to select");
        assert_eq!(names(&e, "Users"), ["Home", "Cart"]);
        assert_eq!(id(&e, "Cart"), cart, "the id did not change");
        assert_eq!(e.to_text(), SRC);
    }

    #[test]
    fn the_test_plan_cannot_be_removed() {
        let mut e = open();
        assert!(e.remove(id(&e, "Shop")).is_err());
        assert!(e.duplicate(id(&e, "Shop")).is_err());
        assert!(!e.is_dirty());
    }

    #[test]
    fn duplicate_copies_children_and_sits_after_the_original() {
        let mut e = open();
        let users = id(&e, "Users");
        let copy = e.duplicate(users).unwrap();
        assert_ne!(copy, users);
        let top = e.plan().node(id(&e, "Shop")).unwrap();
        assert_eq!(top.children, vec![users, copy]);
        assert_eq!(e.plan().node(copy).unwrap().children.len(), 2, "its samplers came along");
        assert_eq!(e.plan().len(), 7);
        e.undo().unwrap();
        assert_eq!(e.to_text(), SRC);
    }

    #[test]
    fn moving_reorders_and_reparents_and_undoes() {
        let mut e = open();
        let (home, cart, users) = (id(&e, "Home"), id(&e, "Cart"), id(&e, "Users"));
        e.move_to(home, users, 1).unwrap();
        assert_eq!(names(&e, "Users"), ["Cart", "Home"]);
        e.undo().unwrap();
        assert_eq!(e.to_text(), SRC);
        e.redo().unwrap();
        assert_eq!(names(&e, "Users"), ["Cart", "Home"]);
        assert!(e.nudge(home, false).unwrap());
        assert_eq!(names(&e, "Users"), ["Home", "Cart"]);
        assert!(!e.nudge(home, false).unwrap(), "already first");
        assert!(!e.nudge(cart, true).unwrap(), "already last");
        // A sampler cannot be moved under another sampler, and nothing moves into itself.
        assert!(e.move_to(home, cart, 0).is_err());
        assert!(e.move_to(users, users, 0).is_err());
    }

    #[test]
    fn moving_between_levels_re_indents_and_undoes_exactly() {
        let mut e = open();
        let home = id(&e, "Home");
        e.add(id(&e, "Users"), "TransactionController", "TransactionControllerGui").unwrap();
        let tc = id(&e, "Transaction Controller");
        let before = e.to_text();
        e.move_to(home, tc, 0).unwrap();
        assert_eq!(names(&e, "Transaction Controller"), ["Home"]);
        let after = e.to_text();
        assert!(after.contains("          <HTTPSamplerProxy guiclass=\"HttpTestSampleGui\" testclass=\"HTTPSamplerProxy\" testname=\"Home\" enabled=\"true\"/>"), "{after}");
        e.undo().unwrap();
        assert_eq!(e.to_text(), before);
    }

    #[test]
    fn settings_changes_are_undoable_and_a_no_op_is_not_recorded() {
        let mut e = open();
        let shop = id(&e, "Shop");
        e.set_prop(shop, "TestPlan.comments", "stringProp", "notes").unwrap();
        assert!(!e.can_undo(), "setting the same value changes nothing");
        e.set_prop(shop, "TestPlan.functional_mode", "boolProp", "true").unwrap();
        assert_eq!(e.prop(shop, "TestPlan.functional_mode").as_deref(), Some("true"));
        e.undo().unwrap();
        assert_eq!(e.prop(shop, "TestPlan.functional_mode"), None, "a property that was added goes away again");
        assert_eq!(e.to_text(), SRC);
        e.rename(shop, "Store").unwrap();
        assert_eq!(e.plan().node(shop).unwrap().name, "Store");
        e.undo().unwrap();
        assert_eq!(e.plan().node(shop).unwrap().name, "Shop");
    }

    #[test]
    fn disabling_shows_in_the_plan() {
        let mut e = open();
        let home = id(&e, "Home");
        e.set_enabled(home, false).unwrap();
        assert!(!e.plan().node(home).unwrap().enabled);
        e.set_enabled(home, true).unwrap();
        assert!(e.plan().node(home).unwrap().enabled);
    }

    #[test]
    fn dirty_follows_saves_and_history() {
        let mut e = open();
        let shop = id(&e, "Shop");
        e.rename(shop, "A").unwrap();
        assert!(e.is_dirty());
        e.mark_saved();
        assert!(!e.is_dirty());
        e.undo().unwrap();
        assert!(e.is_dirty(), "undoing past the saved state");
        e.redo().unwrap();
        assert!(!e.is_dirty(), "and back to it");
        e.undo().unwrap();
        e.rename(shop, "B").unwrap();
        assert!(e.is_dirty(), "a new edit after undoing is not the saved state even if the history is as long");
    }

    #[test]
    fn a_new_edit_clears_redo() {
        let mut e = open();
        let shop = id(&e, "Shop");
        e.rename(shop, "A").unwrap();
        e.undo().unwrap();
        assert!(e.can_redo());
        e.rename(shop, "B").unwrap();
        assert!(!e.can_redo());
    }

    #[test]
    fn missing_elements_are_errors() {
        let mut e = open();
        assert!(e.rename(NodeId(9999), "x").is_err());
        assert!(e.remove(NodeId(9999)).is_err());
        assert!(e.add(NodeId(9999), "ConstantTimer", "ConstantTimerGui").is_err());
        assert!(e.move_to(NodeId(9999), id(&e, "Users"), 0).is_err());
    }
}
