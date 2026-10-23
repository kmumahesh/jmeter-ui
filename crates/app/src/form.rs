//! The form for the selected element: its widgets, and what happens when a value is changed.
//!
//! A form is built when an element is selected and updated in place after edits, so the field being typed in
//! keeps the keyboard focus. Values are saved when a field is confirmed with Enter or left, and the box shows
//! red while the text could never be valid.

use crate::combo::{ComboBox, ComboEvent};
use crate::fields::{TextField, TextFieldEvent};
use crate::shell::{Severity, Shell};
use gpui::{AppContext as _, Context, Entity, Subscription, Window};
use plan_core::forms::{self, Column, Kind, RowShape};
use plan_core::{NodeId, Plan};
use shell_core::layout::PlanId;
use shell_core::number::NumberSpec;

/// How long typing may pause before what was typed is saved to the plan.
pub const LIVE_COMMIT_DELAY: std::time::Duration = std::time::Duration::from_millis(600);

const WHOLE: NumberSpec = NumberSpec { min: Some(i32::MIN as f64), max: Some(i32::MAX as f64), step: 1.0, integer: true, allow_empty: true };
const DECIMAL: NumberSpec = NumberSpec { min: Some(-1e15), max: Some(1e15), step: 1.0, integer: false, allow_empty: true };

/// What one setting looks like in the form.
#[derive(Clone, Debug)]
pub struct FieldDef {
    pub label: String,
    pub prop: String,
    pub kind: Kind,
    pub default: String,
}

pub enum Widget {
    Text(Entity<TextField>),
    Check,
    Combo(Entity<ComboBox>),
    /// Shown but not editable in this form.
    ReadOnly(String),
}

pub struct Item {
    pub def: FieldDef,
    pub widget: Widget,
}

pub enum Cell {
    Text(Entity<TextField>),
    Check { prop: String, on: bool },
}

pub struct TableForm {
    pub title: String,
    pub path: String,
    pub shape: RowShape,
    pub columns: Vec<Column>,
    pub rows: Vec<Vec<Cell>>,
    pub add_label: String,
}

pub struct SectionForm {
    pub title: String,
    pub items: Vec<Item>,
    pub tables: Vec<TableForm>,
}

pub struct Form {
    pub plan: PlanId,
    pub node: NodeId,
    pub name: Entity<TextField>,
    pub comment: Entity<TextField>,
    pub sections: Vec<SectionForm>,
    /// The number of rows in each table when the form was built, to notice rows being added or removed.
    shape: Vec<usize>,
    _subs: Vec<Subscription>,
}

impl Form {
    /// Every text box in the form.
    fn text_fields(&self) -> Vec<Entity<TextField>> {
        let mut v = vec![self.name.clone(), self.comment.clone()];
        for s in &self.sections {
            for i in &s.items {
                if let Widget::Text(f) = &i.widget {
                    v.push(f.clone());
                }
            }
            for t in &s.tables {
                for r in &t.rows {
                    for c in r {
                        if let Cell::Text(f) = c {
                            v.push(f.clone());
                        }
                    }
                }
            }
        }
        v
    }

    /// The name of the box that has the keyboard focus, if it is one of this form's.
    fn focused_field(&self, window: &Window, cx: &gpui::App) -> Option<String> {
        self.text_fields().into_iter().find(|f| f.read(cx).is_focused(window)).map(|f| f.read(cx).field_id())
    }
}

/// The saved value of a setting, or what the element does when it is missing.
fn value_in(plan_edit: &plan_core::EditablePlan, node: NodeId, def: &FieldDef) -> String {
    plan_edit.prop(node, &def.prop).unwrap_or_else(|| def.default.clone())
}

fn sections_for(plan: &Plan, node: NodeId) -> Vec<(String, Vec<FieldDef>, Vec<forms::Table>)> {
    let Some(n) = plan.node(node) else { return vec![] };
    let def = |f: &forms::Field| FieldDef { label: f.label.to_string(), prop: f.prop.to_string(), kind: f.kind, default: f.default.to_string() };
    match forms::curated(&n.tag, &n.guiclass) {
        Some(sections) => sections.iter().map(|s| (s.title.to_string(), s.fields.iter().map(def).collect(), s.tables.to_vec())).collect(),
        None => {
            let fields: Vec<FieldDef> =
                forms::generic_fields(n).into_iter().map(|g| FieldDef { label: g.label, prop: g.prop, kind: if g.summary_only { Kind::Multiline } else { g.kind }, default: String::new() }).collect();
            if fields.is_empty() { vec![] } else { vec![("Settings".to_string(), fields, vec![])] }
        }
    }
}

fn new_kind(kind: Kind) -> &'static str {
    match kind {
        Kind::Bool => "boolProp",
        Kind::BitChoice(_) | Kind::BitFlag(_) => "intProp",
        _ => "stringProp",
    }
}

fn int_of(s: &str) -> i64 {
    s.trim().parse::<i64>().unwrap_or(0)
}

impl Shell {
    /// Drop what the form shows so the next draw takes the plan's values again.
    pub fn reset_form(&mut self) {
        self.form_stale = true;
    }

    /// Make the form match the selected element, building or updating it. Call before drawing.
    pub(crate) fn ensure_form(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(lp) = self.plan() else {
            self.form = None;
            return;
        };
        let node = lp.tree.selected().or_else(|| lp.plan.roots().first().copied()).filter(|&n| lp.plan.node(n).is_some());
        let Some(node) = node else {
            self.form = None;
            return;
        };
        let key = (lp.id, node);
        let same = self.form.as_ref().is_some_and(|f| (f.plan, f.node) == key);
        if same && !self.form_stale {
            self.apply_pending_focus(window, cx);
            return;
        }
        if same && self.form.as_ref().is_some_and(|f| f.shape == self.shape_of(key.0, key.1)) {
            self.refresh_form(window, cx);
            self.form_stale = false;
            self.apply_pending_focus(window, cx);
            return;
        }
        // A rebuilt form has new boxes; the one being typed in keeps the keyboard focus by name.
        let focused = self.form.as_ref().and_then(|f| f.focused_field(window, cx));
        let form = self.build_form(key.0, key.1, window, cx);
        if let Some(name) = focused
            && let Some(f) = form.text_fields().into_iter().find(|f| f.read(cx).field_id() == name)
        {
            f.update(cx, |f, cx| f.focus(window, cx));
        }
        self.form = Some(form);
        self.form_stale = false;
        self.apply_pending_focus(window, cx);
    }

    /// After a row was added, put the cursor in its first cell.
    fn apply_pending_focus(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some((path, row)) = self.pending_focus.take() else { return };
        let Some(form) = &self.form else { return };
        let target = form.sections.iter().flat_map(|s| s.tables.iter()).find(|t| t.path == path).and_then(|t| t.rows.get(row)).and_then(|r| {
            r.iter().find_map(|c| if let Cell::Text(f) = c { Some(f.clone()) } else { None })
        });
        if let Some(f) = target {
            f.update(cx, |f, cx| f.focus(window, cx));
        }
    }

    /// The number of rows in each table of the form for `node`, to compare with a form already built.
    fn shape_of(&self, plan: PlanId, node: NodeId) -> Vec<usize> {
        let Some(lp) = self.plans.iter().find(|p| p.id == plan) else { return vec![] };
        // A generated form has a box for each saved setting; a hand-made one has its own fixed list.
        let generated = lp.plan.node(node).is_some_and(|n| forms::curated(&n.tag, &n.guiclass).is_none());
        let mut v = vec![if generated { lp.plan.node(node).map_or(0, |n| n.props.len()) } else { 0 }];
        for (_, fields, tables) in sections_for(&lp.plan, node) {
            for def in fields {
                // A value that is not one of the listed choices gets a choice of its own, so the form
                // has to be built again when that changes.
                if let Kind::Choice(opts) = def.kind {
                    let cur = value_in(&lp.edit, node, &def);
                    if !opts.iter().any(|(_, v)| *v == cur) {
                        v.push(cur.bytes().fold(1usize, |h, b| h.wrapping_mul(31).wrapping_add(usize::from(b))));
                    }
                }
            }
            for t in tables {
                v.push(lp.edit.row_count(node, t.path));
            }
        }
        v
    }

    fn make_field(&mut self, id: String, placeholder: &str, text: &str, kind: Kind, window: &mut Window, cx: &mut Context<Self>) -> Entity<TextField> {
        let spec = match kind {
            Kind::Number => Some(WHOLE),
            Kind::Decimal => Some(DECIMAL),
            _ => None,
        };
        let (placeholder, text) = (placeholder.to_string(), text.to_string());
        let field = cx.new(|cx| {
            let f = TextField::new(cx, gpui::SharedString::from(id), &placeholder, &text);
            match spec {
                Some(s) => f.number(s),
                None => f,
            }
        });
        field.update(cx, |f, cx| f.watch_blur(window, cx));
        field
    }

    fn build_form(&mut self, plan_id: PlanId, node: NodeId, window: &mut Window, cx: &mut Context<Self>) -> Form {
        let mut subs: Vec<Subscription> = Vec::new();
        let (name_text, comment_text, sections) = {
            let lp = self.plans.iter().find(|p| p.id == plan_id).expect("plan is open");
            let n = lp.plan.node(node).expect("node exists");
            (n.name.clone(), lp.edit.prop(node, forms::COMMENTS_PROP).unwrap_or_default(), sections_for(&lp.plan, node))
        };
        let name = self.make_field("field-name".into(), "Name", &name_text, Kind::Text, window, cx);
        let comment = self.make_field("field-comments".into(), "Comments", &comment_text, Kind::Text, window, cx);
        let name_def = FieldDef { label: "Name".into(), prop: "@name".into(), kind: Kind::Text, default: String::new() };
        let comment_def = FieldDef { label: "Comments".into(), prop: forms::COMMENTS_PROP.into(), kind: Kind::Text, default: String::new() };
        subs.push(self.watch_text(&name, plan_id, node, name_def, window, cx));
        subs.push(self.watch_text(&comment, plan_id, node, comment_def, window, cx));

        let mut out = Vec::new();
        for (si, (title, fields, tables)) in sections.into_iter().enumerate() {
            let mut items = Vec::new();
            for (fi, def) in fields.into_iter().enumerate() {
                let current = {
                    let lp = self.plans.iter().find(|p| p.id == plan_id).expect("plan is open");
                    value_in(&lp.edit, node, &def)
                };
                let id = format!("field-{}", def.prop);
                let widget = match def.kind {
                    Kind::Text | Kind::Number | Kind::Decimal => {
                        let stored = self.plans.iter().find(|p| p.id == plan_id).and_then(|lp| lp.edit.prop(node, &def.prop)).unwrap_or_default();
                        let f = self.make_field(id, &def.default, &stored, def.kind, window, cx);
                        subs.push(self.watch_text(&f, plan_id, node, def.clone(), window, cx));
                        Widget::Text(f)
                    }
                    Kind::Bool | Kind::BitFlag(_) => Widget::Check,
                    Kind::Choice(opts) => {
                        let mut labels: Vec<String> = opts.iter().map(|(l, _)| l.to_string()).collect();
                        let mut values: Vec<String> = opts.iter().map(|(_, v)| v.to_string()).collect();
                        let at = match values.iter().position(|v| *v == current) {
                            Some(i) => i,
                            None => {
                                labels.push(format!("{current} (custom)"));
                                values.push(current.clone());
                                labels.len() - 1
                            }
                        };
                        let combo = self.make_combo(format!("combo-{}", def.prop), &labels, at, cx);
                        subs.push(self.watch_combo(&combo, plan_id, node, def.clone(), values, window, cx));
                        Widget::Combo(combo)
                    }
                    Kind::BitChoice(opts) => {
                        let labels: Vec<String> = opts.iter().map(|(l, _)| l.to_string()).collect();
                        let mask: i64 = opts.iter().map(|(_, b)| *b).sum();
                        let at = opts.iter().position(|(_, b)| int_of(&current) & mask == *b).unwrap_or(0);
                        let combo = self.make_combo(format!("combo-{}-{si}-{fi}", def.prop), &labels, at, cx);
                        let values: Vec<String> = opts.iter().map(|(_, b)| b.to_string()).collect();
                        subs.push(self.watch_combo(&combo, plan_id, node, def.clone(), values, window, cx));
                        Widget::Combo(combo)
                    }
                    Kind::Multiline => {
                        let first = current.lines().next().unwrap_or("").to_string();
                        let more = current.lines().count().saturating_sub(1);
                        Widget::ReadOnly(if more > 0 { format!("{first}  (+{more} more lines)") } else { first })
                    }
                };
                items.push(Item { def, widget });
            }
            let mut table_forms = Vec::new();
            for t in tables {
                table_forms.push(self.build_table(plan_id, node, &t, &mut subs, window, cx));
            }
            out.push(SectionForm { title, items, tables: table_forms });
        }
        let shape = self.shape_of(plan_id, node);
        Form { plan: plan_id, node, name, comment, sections: out, shape, _subs: subs }
    }

    fn make_combo(&mut self, id: String, labels: &[String], at: usize, cx: &mut Context<Self>) -> Entity<ComboBox> {
        let refs: Vec<&str> = labels.iter().map(String::as_str).collect();
        cx.new(|_| ComboBox::new(id, &refs, at))
    }

    fn build_table(&mut self, plan_id: PlanId, node: NodeId, t: &forms::Table, subs: &mut Vec<Subscription>, window: &mut Window, cx: &mut Context<Self>) -> TableForm {
        let rows_data: Vec<Vec<String>> = {
            let lp = self.plans.iter().find(|p| p.id == plan_id).expect("plan is open");
            (0..lp.edit.row_count(node, t.path))
                .map(|i| {
                    if t.shape == RowShape::Text {
                        vec![lp.edit.row_text(node, t.path, i)]
                    } else {
                        let cells = lp.edit.row_cells(node, t.path, i);
                        t.columns.iter().map(|c| cells.iter().find(|(n, _)| n == c.prop).map(|(_, v)| v.clone()).unwrap_or_default()).collect()
                    }
                })
                .collect()
        };
        let mut rows = Vec::new();
        for (ri, data) in rows_data.iter().enumerate() {
            let mut cells = Vec::new();
            for (ci, col) in t.columns.iter().enumerate() {
                let value = data.get(ci).cloned().unwrap_or_default();
                match col.kind {
                    Kind::Bool => cells.push(Cell::Check { prop: col.prop.to_string(), on: value == "true" }),
                    _ => {
                        let f = self.make_field(format!("cell-{}-{ri}-{ci}", t.path), "", &value, Kind::Text, window, cx);
                        let (path, shape, cell_prop) = (t.path.to_string(), t.shape, col.prop.to_string());
                        let sub = cx.subscribe_in(&f, window, move |this, field, e: &TextFieldEvent, window, cx| match e {
                            TextFieldEvent::Changed => {
                                this.refresh_dirty_mark(cx);
                                this.schedule_live_commit(window, cx);
                            }
                            TextFieldEvent::Submitted | TextFieldEvent::Blurred => {
                                let text = field.read(cx).text().to_string();
                                this.commit_cell(plan_id, node, &path, shape, ri, &cell_prop, &text, window, cx);
                            }
                            TextFieldEvent::Cancelled => this.reset_form(),
                            _ => {}
                        });
                        subs.push(sub);
                        cells.push(Cell::Text(f));
                    }
                }
            }
            rows.push(cells);
        }
        TableForm { title: t.title.to_string(), path: t.path.to_string(), shape: t.shape, columns: t.columns.to_vec(), rows, add_label: t.add_label.to_string() }
    }

    fn watch_text(&mut self, field: &Entity<TextField>, plan_id: PlanId, node: NodeId, def: FieldDef, window: &mut Window, cx: &mut Context<Self>) -> Subscription {
        cx.subscribe_in(field, window, move |this, field, e: &TextFieldEvent, window, cx| match e {
            TextFieldEvent::Submitted | TextFieldEvent::Blurred => {
                let text = field.read(cx).text().to_string();
                this.commit_text(plan_id, node, &def, &text, window, cx);
            }
            TextFieldEvent::Changed => {
                this.refresh_dirty_mark(cx);
                this.schedule_live_commit(window, cx);
            }
            TextFieldEvent::Cancelled => {
                // Escape puts the saved value back.
                let stored = this.stored(plan_id, node, &def);
                field.update(cx, |f, cx| f.set_text(&stored, cx));
                this.refresh_dirty_mark(cx);
            }
            _ => {}
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn watch_combo(&mut self, combo: &Entity<ComboBox>, plan_id: PlanId, node: NodeId, def: FieldDef, values: Vec<String>, window: &mut Window, cx: &mut Context<Self>) -> Subscription {
        cx.subscribe_in(combo, window, move |this, _, e: &ComboEvent, window, cx| {
            let ComboEvent::Selected(i) = e;
            let Some(v) = values.get(*i).cloned() else { return };
            let value = match def.kind {
                Kind::BitChoice(opts) => {
                    let mask: i64 = opts.iter().map(|(_, b)| *b).sum();
                    let cur = this.current_value(plan_id, node, &def);
                    ((int_of(&cur) & !mask) | int_of(&v)).to_string()
                }
                _ => v,
            };
            this.save_value(plan_id, node, &def, &value, window, cx);
        })
    }

    fn current_value(&self, plan_id: PlanId, node: NodeId, def: &FieldDef) -> String {
        self.plans.iter().find(|p| p.id == plan_id).map(|lp| value_in(&lp.edit, node, def)).unwrap_or_default()
    }

    /// Put the plan's current values into the widgets that are not being typed in.
    fn refresh_form(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(form) = &self.form else { return };
        let (plan_id, node) = (form.plan, form.node);
        let Some(lp) = self.plans.iter().find(|p| p.id == plan_id) else { return };
        let Some(n) = lp.plan.node(node) else { return };
        let set = |f: &Entity<TextField>, text: &str, window: &Window, cx: &mut Context<Shell>| {
            let focused = f.read(cx).is_focused(window);
            if !focused && f.read(cx).text() != text {
                f.update(cx, |f, cx| f.set_text(text, cx));
            }
        };
        set(&form.name, &n.name, window, cx);
        set(&form.comment, &lp.edit.prop(node, forms::COMMENTS_PROP).unwrap_or_default(), window, cx);
        for s in &form.sections {
            for item in &s.items {
                let cur = value_in(&lp.edit, node, &item.def);
                match (&item.widget, item.def.kind) {
                    (Widget::Text(f), _) => set(f, &lp.edit.prop(node, &item.def.prop).unwrap_or_default(), window, cx),
                    (Widget::Combo(c), Kind::Choice(opts)) => {
                        let at = opts.iter().position(|(_, v)| *v == cur);
                        if let Some(at) = at {
                            c.update(cx, |c, cx| c.set_selected_silently(at, cx));
                        }
                    }
                    (Widget::Combo(c), Kind::BitChoice(opts)) => {
                        let mask: i64 = opts.iter().map(|(_, b)| *b).sum();
                        if let Some(at) = opts.iter().position(|(_, b)| int_of(&cur) & mask == *b) {
                            c.update(cx, |c, cx| c.set_selected_silently(at, cx));
                        }
                    }
                    _ => {}
                }
            }
            for t in &s.tables {
                for (ri, row) in t.rows.iter().enumerate() {
                    let cells: Vec<(String, String)> = lp.edit.row_cells(node, &t.path, ri);
                    for (ci, cell) in row.iter().enumerate() {
                        if let Cell::Text(f) = cell {
                            let text = if t.shape == RowShape::Text {
                                lp.edit.row_text(node, &t.path, ri)
                            } else {
                                t.columns.get(ci).and_then(|c| cells.iter().find(|(n, _)| n == c.prop).map(|(_, v)| v.clone())).unwrap_or_default()
                            };
                            set(f, &text, window, cx);
                        }
                    }
                }
            }
        }
    }

    // ---- saving values ----

    /// Whether a box holds text that is not in the plan yet.
    fn has_uncommitted_text(&self, cx: &gpui::App) -> bool {
        let Some(form) = &self.form else { return false };
        let (plan_id, node) = (form.plan, form.node);
        let name_def = FieldDef { label: "Name".into(), prop: "@name".into(), kind: Kind::Text, default: String::new() };
        let comment_def = FieldDef { label: "Comments".into(), prop: forms::COMMENTS_PROP.into(), kind: Kind::Text, default: String::new() };
        form.name.read(cx).text() != self.stored(plan_id, node, &name_def)
            || form.comment.read(cx).text() != self.stored(plan_id, node, &comment_def)
            || form.sections.iter().flat_map(|s| s.items.iter()).any(|i| matches!(&i.widget, Widget::Text(f) if f.read(cx).text() != self.stored(plan_id, node, &i.def)))
            || !self.uncommitted_cells(cx).is_empty()
    }

    /// Table cells whose text differs from the plan: (table path, shape, row, cell setting, text).
    fn uncommitted_cells(&self, cx: &gpui::App) -> Vec<(String, RowShape, usize, String, String)> {
        let Some(form) = &self.form else { return vec![] };
        let Some(lp) = self.plans.iter().find(|p| p.id == form.plan) else { return vec![] };
        let mut out = Vec::new();
        for t in form.sections.iter().flat_map(|s| s.tables.iter()) {
            for (ri, row) in t.rows.iter().enumerate() {
                if ri >= lp.edit.row_count(form.node, &t.path) {
                    continue;
                }
                let cells = lp.edit.row_cells(form.node, &t.path, ri);
                for (ci, cell) in row.iter().enumerate() {
                    let Cell::Text(f) = cell else { continue };
                    let Some(col) = t.columns.get(ci) else { continue };
                    let stored = if t.shape == RowShape::Text { lp.edit.row_text(form.node, &t.path, ri) } else { cells.iter().find(|(n, _)| n == col.prop).map(|(_, v)| v.clone()).unwrap_or_default() };
                    let text = f.read(cx).text();
                    if text != stored {
                        out.push((t.path.clone(), t.shape, ri, col.prop.to_string(), text.to_string()));
                    }
                }
            }
        }
        out
    }

    /// Save what is typed a moment after the typing pauses, so it is part of the plan (and of the recovery copy)
    /// without waiting for the box to be left. One pause is one undo step.
    pub(crate) fn schedule_live_commit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.typing_seq += 1;
        let seq = self.typing_seq;
        cx.spawn_in(window, async move |this, cx| {
            cx.background_executor().timer(LIVE_COMMIT_DELAY).await;
            this.update_in(cx, |this, window, cx| {
                if this.typing_seq == seq {
                    this.live_commit = true;
                    this.flush_form(window, cx);
                    this.live_commit = false;
                }
            })
            .ok();
        })
        .detach();
    }

    /// Mark the plan as changed as soon as something is typed, not only once the box is confirmed.
    pub(crate) fn refresh_dirty_mark(&mut self, cx: &mut Context<Self>) {
        let typing = self.has_uncommitted_text(cx);
        self.typing_dirty = typing;
        if let Some(form) = &self.form {
            let id = form.plan;
            let dirty = typing || self.plans.iter().any(|p| p.id == id && p.is_dirty());
            self.workspace.set_dirty(shell_core::layout::TabKind::Plan(id), dirty);
        }
        cx.notify();
    }

    /// Save every text box whose text differs from the plan: before something else happens that could lose it.
    pub fn flush_form(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(form) = &self.form else { return };
        let (plan_id, node) = (form.plan, form.node);
        let mut pending: Vec<(FieldDef, String)> = Vec::new();
        for s in &form.sections {
            for item in &s.items {
                if let Widget::Text(f) = &item.widget {
                    let text = f.read(cx).text().to_string();
                    if text != self.stored(plan_id, node, &item.def) {
                        pending.push((item.def.clone(), text));
                    }
                }
            }
        }
        let name_text = form.name.read(cx).text().to_string();
        let comment_text = form.comment.read(cx).text().to_string();
        let name_def = FieldDef { label: "Name".into(), prop: "@name".into(), kind: Kind::Text, default: String::new() };
        let comment_def = FieldDef { label: "Comments".into(), prop: forms::COMMENTS_PROP.into(), kind: Kind::Text, default: String::new() };
        if name_text != self.stored(plan_id, node, &name_def) {
            pending.push((name_def, name_text));
        }
        if comment_text != self.stored(plan_id, node, &comment_def) {
            pending.push((comment_def, comment_text));
        }
        let cells = self.uncommitted_cells(cx);
        for (def, text) in pending {
            self.commit_text(plan_id, node, &def, &text, window, cx);
        }
        for (path, shape, row, prop, text) in cells {
            self.commit_cell(plan_id, node, &path, shape, row, &prop, &text, window, cx);
        }
        self.refresh_dirty_mark(cx);
    }

    /// What the plan has for a setting, as text ("" when the setting is not there).
    fn stored(&self, plan_id: PlanId, node: NodeId, def: &FieldDef) -> String {
        let Some(lp) = self.plans.iter().find(|p| p.id == plan_id) else { return String::new() };
        if def.prop == "@name" {
            return lp.plan.node(node).map(|n| n.name.clone()).unwrap_or_default();
        }
        lp.edit.prop(node, &def.prop).unwrap_or_default()
    }

    fn commit_text(&mut self, plan_id: PlanId, node: NodeId, def: &FieldDef, text: &str, window: &mut Window, cx: &mut Context<Self>) {
        if !self.plans.iter().any(|p| p.id == plan_id && p.plan.node(node).is_some()) {
            return;
        }
        if def.prop == "@name" {
            self.with_plan(plan_id, window, cx, |this, window, cx| this.rename_element(node, text, window, cx));
            return;
        }
        let spec = match def.kind {
            Kind::Number => Some(WHOLE),
            Kind::Decimal => Some(DECIMAL),
            _ => None,
        };
        if let Some(spec) = spec
            && let Err(problem) = spec.parse(text)
        {
            // While typing, half a number is not worth an error.
            if !self.live_commit {
                self.notify(format!("{}: {}", def.label, problem.message()), Severity::Error, window, cx);
            }
            return;
        }
        let stored = self.stored(plan_id, node, def);
        let exists = self.plans.iter().find(|p| p.id == plan_id).is_some_and(|lp| lp.edit.prop(node, &def.prop).is_some());
        if stored == text || (!exists && text.is_empty()) {
            return;
        }
        self.save_value(plan_id, node, def, text, window, cx);
    }

    /// Write a setting to the plan.
    fn save_value(&mut self, plan_id: PlanId, node: NodeId, def: &FieldDef, value: &str, window: &mut Window, cx: &mut Context<Self>) {
        let (prop, kind) = (def.prop.clone(), new_kind(def.kind));
        let value = value.to_string();
        self.with_plan(plan_id, window, cx, |this, window, cx| this.set_element_prop(node, &prop, kind, &value, window, cx));
    }

    /// Run an edit on a plan that may not be the one in front (a field is saved when it loses focus, which can
    /// be because another tab was chosen).
    fn with_plan(&mut self, plan_id: PlanId, window: &mut Window, cx: &mut Context<Self>, f: impl FnOnce(&mut Shell, &mut Window, &mut Context<Shell>)) {
        let before = self.active_plan;
        self.active_plan = Some(plan_id);
        f(self, window, cx);
        if self.plans.iter().any(|p| p.id == before.unwrap_or(plan_id)) {
            self.active_plan = before;
        }
    }

    pub fn toggle_setting(&mut self, def: &FieldDef, window: &mut Window, cx: &mut Context<Self>) {
        let Some(form) = &self.form else { return };
        let (plan_id, node) = (form.plan, form.node);
        let cur = self.current_value(plan_id, node, def);
        let value = match def.kind {
            Kind::BitFlag(bit) => {
                let v = int_of(&cur);
                (if v & bit != 0 { v & !bit } else { v | bit }).to_string()
            }
            _ => (if cur == "true" { "false" } else { "true" }).to_string(),
        };
        self.save_value(plan_id, node, def, &value, window, cx);
    }

    // ---- tables ----

    #[allow(clippy::too_many_arguments)]
    fn commit_cell(&mut self, plan_id: PlanId, node: NodeId, path: &str, shape: RowShape, row: usize, cell_prop: &str, text: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(lp) = self.plans.iter().find(|p| p.id == plan_id) else { return };
        if row >= lp.edit.row_count(node, path) {
            return;
        }
        let path = path.to_string();
        if shape == RowShape::Text {
            if lp.edit.row_text(node, &path, row) == text {
                return;
            }
            let new = forms::new_row(RowShape::Text, &[text]);
            self.with_plan(plan_id, window, cx, |this, window, cx| {
                let Some(lp) = this.plan_mut() else { return };
                let res = lp.edit.replace_row(node, &path, row, new);
                match res {
                    Ok(()) => this.after_edit(Some(node), window, cx),
                    Err(e) => this.notify(format!("{e:#}"), Severity::Error, window, cx),
                }
            });
            return;
        }
        let current = lp.edit.row_cells(node, &path, row).into_iter().find(|(n, _)| n == cell_prop).map(|(_, v)| v);
        if current.as_deref().unwrap_or("") == text {
            return;
        }
        let setting = format!("{path}/#{row}/{cell_prop}");
        self.with_plan(plan_id, window, cx, |this, window, cx| this.set_element_prop(node, &setting, "stringProp", text, window, cx));
    }

    /// Toggle a checkbox cell of a table row.
    pub fn toggle_cell(&mut self, path: &str, row: usize, cell_prop: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(form) = &self.form else { return };
        let (plan_id, node) = (form.plan, form.node);
        let Some(lp) = self.plans.iter().find(|p| p.id == plan_id) else { return };
        let on = lp.edit.row_cells(node, path, row).into_iter().find(|(n, _)| n == cell_prop).is_some_and(|(_, v)| v == "true");
        let setting = format!("{path}/#{row}/{cell_prop}");
        self.with_plan(plan_id, window, cx, |this, window, cx| this.set_element_prop(node, &setting, "boolProp", if on { "false" } else { "true" }, window, cx));
    }

    pub fn add_table_row(&mut self, path: &str, shape: RowShape, window: &mut Window, cx: &mut Context<Self>) {
        let Some(form) = &self.form else { return };
        let (plan_id, node) = (form.plan, form.node);
        let path = path.to_string();
        let at = self.plans.iter().find(|p| p.id == plan_id).map_or(0, |lp| lp.edit.row_count(node, &path));
        self.pending_focus = Some((path.clone(), at));
        self.with_plan(plan_id, window, cx, |this, window, cx| {
            let Some(lp) = this.plan_mut() else { return };
            match lp.edit.add_row(node, &path, at, forms::new_row(shape, &["", ""])) {
                Ok(()) => this.after_edit(Some(node), window, cx),
                Err(e) => this.notify(format!("{e:#}"), Severity::Error, window, cx),
            }
        });
    }

    pub fn remove_table_row(&mut self, path: &str, row: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some(form) = &self.form else { return };
        let (plan_id, node) = (form.plan, form.node);
        let path = path.to_string();
        self.with_plan(plan_id, window, cx, |this, window, cx| {
            let Some(lp) = this.plan_mut() else { return };
            match lp.edit.remove_row(node, &path, row) {
                Ok(()) => this.after_edit(Some(node), window, cx),
                Err(e) => this.notify(format!("{e:#}"), Severity::Error, window, cx),
            }
        });
    }

    /// Put the keyboard cursor in the name box.
    pub fn focus_name_field(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(form) = &self.form {
            form.name.update(cx, |f, cx| {
                f.focus(window, cx);
                f.select_all(cx);
            });
        }
    }
}
