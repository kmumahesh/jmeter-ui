//! The application window: state, actions and the pieces that tie the views together.

use crate::combo::{ComboBox, ComboEvent};
use crate::fields::TextField;
use crate::palette_view::{Palette, PaletteEvent, Snapshot};
use crate::theme;
use gpui::{
    App, Context, DismissEvent, Entity, EventEmitter, FocusHandle, Focusable, KeyBinding, PathPromptOptions, ScrollHandle, Subscription, UniformListScrollHandle,
    Window, prelude::*,
};
use plan_core::{EditablePlan, Move, NodeId, Plan, TreeState};
pub use shell_core::actions::AppContext;
use shell_core::actions::Action;
use shell_core::layout::{PlanId, TabKind, Workspace};
use shell_core::palette::Target;
use shell_core::number::{NumberSpec, Value};
use shell_core::settings::{self, Settings, Startup, ThemeChoice};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

pub struct LoadedPlan {
    pub id: PlanId,
    pub path: PathBuf,
    /// The plan and its undo history.
    pub edit: EditablePlan,
    /// The plan as last read from `edit`, shared with the views.
    pub plan: Arc<Plan>,
    pub tree: TreeState,
    /// Counts changes, so the recovery copy is only written for the latest one.
    pub version: u64,
    /// The notes about undefined variables, worked out when the Problems list is drawn and kept until the next change.
    pub(crate) hints: std::cell::RefCell<Option<(u64, std::rc::Rc<Vec<plan_core::problems::Problem>>)>>,
    /// The notes about the structure of the plan, kept the same way.
    pub(crate) notes: std::cell::RefCell<Option<(u64, std::rc::Rc<Vec<plan_core::problems::Problem>>)>>,
}

/// An element being dragged in the tree.
#[derive(Clone, Debug)]
pub struct TreeDrag {
    pub plan: PlanId,
    pub node: NodeId,
    pub label: String,
    pub icon: &'static str,
    pub color: gpui::Hsla,
}

/// Where on a row an element was dropped: above it, onto it (as a child) or below it.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum DropZone {
    Before,
    Into,
    After,
}

/// What happens after the unsaved-changes question is answered.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Then {
    CloseTab(PlanId),
    CloseAll,
    Quit,
}

/// A question shown over the window.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Dialog {
    /// These plans have unsaved changes.
    Unsaved { plans: Vec<PlanId>, then: Then },
    /// Reloading would throw away this plan's unsaved changes.
    DiscardForReload(PlanId),
    /// A recovery copy was left by an earlier session.
    Recover { path: PathBuf, saved: Option<String>, recovered: String },
    /// The plan has unsaved changes and a run uses the saved file.
    SaveToRun { kind: jvm_runtime::runs::RunKind, sampler: Option<NodeId> },
    /// The plan contains scripts and has not been agreed to run.
    Scripts { kind: jvm_runtime::runs::RunKind, sampler: Option<NodeId>, text: String },
    /// The place for run files cannot be used.
    RunsPlace { message: String, kind: jvm_runtime::runs::RunKind, sampler: Option<NodeId> },
    /// Empty the shared runs folder?
    ClearRuns { root: PathBuf, count: usize },
    /// Remove one stored run?
    DeleteRun { dir: PathBuf, name: String },
    /// Quitting while a run is going.
    QuitRunning,
}

impl LoadedPlan {
    pub fn name(&self) -> String {
        self.path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "plan".into())
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Resize {
    Sidebar,
    Inspector,
    Bottom,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Severity {
    Info,
    Error,
}

#[derive(Clone, Debug)]
pub struct Notice {
    pub text: String,
    pub severity: Severity,
    id: u64,
}

/// What was found when looking for Java and JMeter.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RuntimeStatus {
    pub java: Option<String>,
    pub jmeter: Option<String>,
    pub problem: Option<String>,
}

/// Which lower panel tab is showing.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum BottomTab {
    /// The live numbers and charts of a run, or of a stored run.
    Run,
    /// The earlier runs of the open plan.
    History,
    Problems,
}

/// What the lower part of the run view shows below the charts.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum RunDetail {
    Errors,
    Labels,
    Log,
}

pub struct SettingsFields {
    pub jmeter_home: Entity<TextField>,
    pub java_path: Entity<TextField>,
    pub heap_mb: Entity<TextField>,
    pub summary_secs: Entity<TextField>,
    pub startup: Entity<ComboBox>,
    pub runs_keep: Entity<TextField>,
}

pub const HEAP_SPEC: NumberSpec = NumberSpec::whole(settings::HEAP_RANGE_MB.0 as i64, settings::HEAP_RANGE_MB.1 as i64);
pub const KEEP_SPEC: NumberSpec = NumberSpec::whole(settings::RUNS_KEEP_RANGE.0 as i64, settings::RUNS_KEEP_RANGE.1 as i64);
pub const INTERVAL_SPEC: NumberSpec = NumberSpec::whole(settings::SUMMARY_INTERVAL_RANGE.0 as i64, settings::SUMMARY_INTERVAL_RANGE.1 as i64);

pub struct Shell {
    pub settings: Settings,
    pub settings_path: Option<PathBuf>,
    pub workspace: Workspace,
    /// Every open plan, in the order they were opened. Each has one tab.
    pub plans: Vec<LoadedPlan>,
    /// The plan whose tree is showing. The editor shows this plan's selected element.
    pub active_plan: Option<PlanId>,
    next_plan_id: u32,
    pub dialog: Option<Dialog>,
    /// The form for the selected element, and whether the plan changed since it was last updated.
    pub form: Option<crate::form::Form>,
    pub(crate) form_stale: bool,
    /// Text typed into a box and not yet saved to the plan.
    pub(crate) typing_dirty: bool,
    pub(crate) typing_seq: u64,
    pub(crate) live_commit: bool,
    /// A table row that was just added: its table and position, to put the cursor in it.
    pub(crate) pending_focus: Option<(String, usize)>,
    pub palette: Option<Entity<Palette>>,
    pub notice: Option<Notice>,
    notice_seq: u64,
    pub runtime: RuntimeStatus,
    pub focus: FocusHandle,
    pub tree_focus: FocusHandle,
    pub tree_scroll: UniformListScrollHandle,
    pub editor_scroll: ScrollHandle,
    pub resizing: Option<Resize>,
    pub about_open: bool,
    pub bottom_tab: BottomTab,
    pub run_detail: RunDetail,
    /// The run being watched, or the stored run being looked at.
    pub run: Option<crate::running::RunSession>,
    pub(crate) run_tick_on: bool,
    /// The earlier runs of the open plan.
    pub history: Vec<jvm_runtime::runs::RunEntry>,
    pub settings_fields: Option<SettingsFields>,
    state_dump: Option<PathBuf>,
    _subscriptions: Vec<Subscription>,
}

impl EventEmitter<DismissEvent> for Shell {}

/// Options for starting the shell, so tests and the command line can control the environment.
#[derive(Default, Clone)]
pub struct Options {
    /// Settings as loaded at startup (see `shell_core::settings::load`).
    pub settings: Settings,
    /// A message from loading the settings, such as a damaged file being set aside.
    pub notice: Option<String>,
    pub settings_path: Option<PathBuf>,
    pub open: Option<PathBuf>,
    pub state_dump: Option<PathBuf>,
    pub theme: Option<ThemeChoice>,
    /// Look for Java and JMeter in the background.
    pub detect_runtime: bool,
}

macro_rules! shell_actions {
    ($($name:ident),* $(,)?) => {
        pub mod act {
            gpui::actions!(jmeter, [$($name),*]);
        }

        impl Shell {
            /// Connect every gpui action to the matching `shell_core::Action`.
            pub fn attach_actions<E: gpui::InteractiveElement>(&self, el: E, cx: &mut Context<Self>) -> E {
                el $( .on_action(cx.listener(|this, _: &act::$name, window, cx| this.perform(Action::$name, window, cx))) )*
            }
        }

        /// The gpui action for `action`, for menus that show its shortcut.
        pub fn boxed(action: Action) -> Box<dyn gpui::Action> {
            match action {
                $( Action::$name => Box::new(act::$name), )*
            }
        }

        /// The gpui key binding for `action` with keystroke `keys`.
        pub fn key_binding(action: Action, keys: &str) -> KeyBinding {
            key_binding_in(action, keys, None)
        }

        /// The same, only active where the key context `context` is set.
        pub fn key_binding_in(action: Action, keys: &str, context: Option<&str>) -> KeyBinding {
            match action {
                $( Action::$name => KeyBinding::new(keys, act::$name, context), )*
            }
        }
    };
}

shell_actions!(
    OpenPlan, SavePlan, SavePlanAs, ReloadPlan, CloseTab, CloseAllTabs, NextTab, PrevTab, Undo, Redo, DeleteElement, DuplicateElement, RenameElement,
    ToggleElementEnabled, MoveElementUp, MoveElementDown, ToggleSidebar, ToggleInspector, ToggleBottomPanel, FocusTree, ExpandAll,
    CollapseAll, GotoElement, AddElement, CommandPalette, ThemeSystem, ThemeDark, ThemeLight, OpenSettings, About, RunPlan, ValidatePlan, SendOnce, StopRun,
    StopRunNow, ClearRunsFolder, OpenRunsFolder, Quit,
);

gpui::actions!(
    tree,
    [TreeUp, TreeDown, TreeLeft, TreeRight, TreeFirst, TreeLast, TreePageUp, TreePageDown, TreeOpen, TreeToggle]
);

/// Convert `Ctrl+Shift+Tab` to gpui's `ctrl-shift-tab`.
pub fn to_gpui_keys(shortcut: &str) -> String {
    shortcut.split('+').map(|p| p.trim().to_lowercase()).collect::<Vec<_>>().join("-")
}

/// Register all keyboard shortcuts for the shell and the tree.
pub fn bind_keys(cx: &mut App) {
    let mut bindings = Vec::new();
    for a in Action::ALL {
        if let Some(s) = a.shortcut() {
            if s == "Alt+F4" {
                continue; // handled by the operating system
            }
            if a.tree_only() {
                continue; // bound below, where the tree has the focus
            }
            bindings.push(key_binding(a, &to_gpui_keys(s)));
        }
    }
    bindings.push(key_binding(Action::Redo, "ctrl-shift-z"));
    bindings.push(key_binding(Action::CommandPalette, "ctrl-shift-p"));
    let t = Some("Tree");
    bindings.extend([
        KeyBinding::new("up", TreeUp, t),
        KeyBinding::new("down", TreeDown, t),
        KeyBinding::new("left", TreeLeft, t),
        KeyBinding::new("right", TreeRight, t),
        KeyBinding::new("home", TreeFirst, t),
        KeyBinding::new("end", TreeLast, t),
        KeyBinding::new("pageup", TreePageUp, t),
        KeyBinding::new("pagedown", TreePageDown, t),
        KeyBinding::new("enter", TreeOpen, t),
        KeyBinding::new("space", TreeToggle, t),
    ]);
    for a in Action::ALL {
        if a.tree_only()
            && let Some(s) = a.shortcut()
        {
            bindings.push(key_binding_in(a, &to_gpui_keys(s), t));
        }
    }
    // Zed menus (the menu bar and drop-downs) take their keys from a keymap file we do not load.
    let m = Some("menu");
    bindings.extend([
        KeyBinding::new("up", menu::SelectPrevious, m),
        KeyBinding::new("down", menu::SelectNext, m),
        KeyBinding::new("home", menu::SelectFirst, m),
        KeyBinding::new("end", menu::SelectLast, m),
        KeyBinding::new("enter", menu::Confirm, m),
        KeyBinding::new("escape", menu::Cancel, m),
        KeyBinding::new("right", menu::SelectChild, m),
        KeyBinding::new("left", menu::SelectParent, m),
    ]);
    cx.bind_keys(bindings);
}

impl Shell {
    pub fn new(window: &mut Window, cx: &mut Context<Self>, options: Options) -> Shell {
        let notice = options.notice.clone();
        let mut settings = options.settings.clone();
        if let Some(t) = options.theme {
            settings.theme = t;
        }
        let workspace = Workspace::new(&settings);
        let focus = cx.focus_handle();
        let tree_focus = cx.focus_handle();
        let mut shell = Shell {
            settings,
            settings_path: options.settings_path.clone(),
            workspace,
            plans: Vec::new(),
            active_plan: None,
            next_plan_id: 1,
            dialog: None,
            form: None,
            form_stale: false,
            typing_dirty: false,
            typing_seq: 0,
            live_commit: false,
            pending_focus: None,
            palette: None,
            notice: None,
            notice_seq: 0,
            runtime: RuntimeStatus::default(),
            focus: focus.clone(),
            tree_focus,
            tree_scroll: UniformListScrollHandle::new(),
            editor_scroll: ScrollHandle::new(),
            resizing: None,
            about_open: false,
            bottom_tab: BottomTab::Problems,
            run_detail: RunDetail::Errors,
            run: None,
            run_tick_on: false,
            history: Vec::new(),
            settings_fields: None,
            state_dump: options.state_dump.clone(),
            _subscriptions: Vec::new(),
        };
        // Follow the system colour mode while the choice is "system".
        let sub = window.observe_window_appearance(|_, cx| theme::system_appearance_changed(cx));
        shell._subscriptions.push(sub);
        window.focus(&focus, cx);
        if let Some(n) = notice {
            shell.notify(n, Severity::Error, window, cx);
        }
        match options.open {
            Some(path) => {
                shell.open_plan_path(&path, window, cx);
            }
            None if shell.settings.startup == Startup::LastPlan => {
                // Reopen the last plan if it is still there.
                let last = shell.settings.recent_files.first().cloned();
                if let Some(p) = last.filter(|p| Path::new(p).is_file()) {
                    shell.open_plan_path(Path::new(&p), window, cx);
                }
            }
            None => {}
        }
        if options.detect_runtime {
            shell.detect_runtime(window, cx);
        }
        shell
    }

    /// Remember the window size for next time.
    pub fn save_window_state(&mut self, window: &Window) {
        let b = window.bounds();
        self.settings.window.maximized = window.is_maximized();
        if !window.is_maximized() {
            self.settings.window.width = f32::from(b.size.width);
            self.settings.window.height = f32::from(b.size.height);
        }
        self.save_settings();
    }

    pub fn context(&self) -> AppContext {
        let lp = self.plan();
        let sel = lp.and_then(|lp| lp.tree.selected().and_then(|id| lp.plan.node(id).map(|n| (lp, n))));
        // Where the selected element sits among its siblings, read from the plan (cheap even for a huge plan).
        let at = sel.and_then(|(lp, n)| {
            let parent = lp.plan.node(n.parent?)?;
            Some((parent.children.iter().position(|&c| c == n.id)?, parent.children.len()))
        });
        let root = sel.is_some_and(|(lp, n)| lp.plan.roots().first() == Some(&n.id));
        AppContext {
            has_plan: lp.is_some(),
            is_dirty: self.typing_dirty || lp.is_some_and(|lp| lp.is_dirty()),
            can_undo: lp.is_some_and(|lp| lp.edit.can_undo()),
            can_redo: lp.is_some_and(|lp| lp.edit.can_redo()),
            has_selection: sel.is_some(),
            selection_editable: sel.is_some() && !root,
            can_move_up: at.is_some_and(|(i, _)| i > 0),
            can_move_down: at.is_some_and(|(i, n)| i + 1 < n),
            tab_count: self.workspace.tab_count(),
            run_in_progress: self.run_in_progress(),
            can_run: self.runtime_ready(),
            can_send_once: sel.is_some_and(|(_, n)| n.category == plan_core::Category::Sampler),
            custom_runs_folder: self.shared_runs_root().is_some(),
            can_edit: lp.is_some(),
        }
    }

    // ---- notices ----

    pub fn notify(&mut self, text: impl Into<String>, severity: Severity, _window: &mut Window, cx: &mut Context<Self>) {
        self.notify_cx(text, severity, cx);
    }

    pub fn notify_cx(&mut self, text: impl Into<String>, severity: Severity, cx: &mut Context<Self>) {
        self.notice_seq += 1;
        let id = self.notice_seq;
        self.notice = Some(Notice { text: text.into(), severity, id });
        cx.notify();
        let ttl = if severity == Severity::Error { 8 } else { 4 };
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_secs(ttl)).await;
            this.update(cx, |this, cx| {
                if this.notice.as_ref().is_some_and(|n| n.id == id) {
                    this.notice = None;
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    pub fn dismiss_notice(&mut self, cx: &mut Context<Self>) {
        self.notice = None;
        cx.notify();
    }

    // ---- settings ----

    pub fn save_settings(&mut self) {
        self.workspace.write_to(&mut self.settings);
        if let Some(p) = &self.settings_path
            && let Err(e) = settings::save(p, &self.settings) {
                eprintln!("could not save settings: {e:#}");
            }
    }

    // ---- plans ----

    /// The plan whose tree is showing.
    pub fn plan(&self) -> Option<&LoadedPlan> {
        let id = self.active_plan?;
        self.plans.iter().find(|p| p.id == id)
    }

    pub fn plan_mut(&mut self) -> Option<&mut LoadedPlan> {
        let id = self.active_plan?;
        self.plans.iter_mut().find(|p| p.id == id)
    }

    fn plan_index(&self, id: PlanId) -> Option<usize> {
        self.plans.iter().position(|p| p.id == id)
    }

    pub(crate) fn new_plan_id(&mut self) -> PlanId {
        let id = PlanId(self.next_plan_id);
        self.next_plan_id += 1;
        id
    }

    /// Make a plan the one showing, activating its tab.
    pub fn show_plan(&mut self, id: PlanId) {
        if let Some(i) = self.workspace.find(TabKind::Plan(id)) {
            self.workspace.activate(i);
            self.active_plan = Some(id);
            self.refresh_history();
        }
    }

    pub fn open_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let rx = cx.prompt_for_paths(PathPromptOptions { files: true, directories: false, multiple: false, prompt: Some("Open JMeter Plan".into()) });
        cx.spawn_in(window, async move |this, cx| {
            if let Ok(Ok(Some(paths))) = rx.await
                && let Some(path) = paths.into_iter().next() {
                    this.update_in(cx, |this, window, cx| {
                        this.open_plan_path(&path, window, cx);
                    })
                    .ok();
                }
        })
        .detach();
    }

    /// Select an element in the active plan. The editor redraws with that element; no tab opens.
    pub fn select_node(&mut self, id: NodeId, window: &mut Window, cx: &mut Context<Self>) {
        self.flush_form(window, cx);
        let Some(lp) = self.plan_mut() else { return };
        let plan = lp.plan.clone();
        let plan_id = lp.id;
        lp.tree.select(&plan, id);
        let row = lp.tree.row_index(&plan, id);
        if let Some(i) = row {
            self.tree_scroll.scroll_to_item(i, gpui::ScrollStrategy::Center);
        }
        // Selecting while the Settings tab is in front brings the plan back.
        self.show_plan(plan_id);
        let _ = window;
        cx.notify();
    }

    /// Make a tab active. A plan tab also makes that plan the one in the tree.
    pub fn activate_tab(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.flush_form(window, cx);
        if self.workspace.activate(index)
            && let Some(TabKind::Plan(id)) = self.workspace.active_tab().map(|t| t.kind)
        {
            self.active_plan = Some(id);
        }
        cx.notify();
    }

    /// Close a tab. Closing a plan tab closes the plan.
    pub fn close_tab(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(kind) = self.workspace.tabs().get(index).map(|t| t.kind) else { return };
        self.workspace.close_tab(index);
        if let TabKind::Plan(id) = kind {
            self.plans.retain(|p| p.id != id);
        }
        self.sync_active_plan();
        cx.notify();
    }

    pub fn close_all_tabs(&mut self, cx: &mut Context<Self>) {
        self.workspace.close_all();
        self.plans.clear();
        self.active_plan = None;
        self.refresh_history();
        cx.notify();
    }

    /// After tabs change, keep the tree on a plan that is still open: the plan of the active tab, or else
    /// the last plan shown.
    fn sync_active_plan(&mut self) {
        if let Some(TabKind::Plan(id)) = self.workspace.active_tab().map(|t| t.kind) {
            self.active_plan = Some(id);
        } else if !self.active_plan.is_some_and(|id| self.plan_index(id).is_some()) {
            self.active_plan = self.workspace.tabs().iter().find_map(|t| if let TabKind::Plan(id) = t.kind { Some(id) } else { None });
        }
        self.refresh_history();
    }

    pub fn tree_move(&mut self, mv: Move, window: &mut Window, cx: &mut Context<Self>) {
        self.flush_form(window, cx);
        let Some(lp) = self.plan_mut() else { return };
        let plan = lp.plan.clone();
        if let Some(id) = lp.tree.move_selection(&plan, mv)
            && let Some(i) = lp.tree.row_index(&plan, id) {
                self.tree_scroll.scroll_to_item(i, gpui::ScrollStrategy::Nearest);
            }
        cx.notify();
    }

    pub fn tree_key_left(&mut self, cx: &mut Context<Self>) {
        if let Some(lp) = self.plan_mut() {
            let plan = lp.plan.clone();
            lp.tree.key_left(&plan);
        }
        cx.notify();
    }

    pub fn tree_key_right(&mut self, cx: &mut Context<Self>) {
        if let Some(lp) = self.plan_mut() {
            let plan = lp.plan.clone();
            lp.tree.key_right(&plan);
        }
        cx.notify();
    }

    pub fn toggle_selected(&mut self, cx: &mut Context<Self>) {
        if let Some(id) = self.plan().and_then(|lp| lp.tree.selected()) {
            self.toggle_node(id, cx);
        }
    }

    pub fn toggle_node(&mut self, id: NodeId, cx: &mut Context<Self>) {
        if let Some(lp) = self.plan_mut() {
            let plan = lp.plan.clone();
            lp.tree.toggle(&plan, id);
        }
        cx.notify();
    }

    // ---- palette ----

    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            context: self.context(),
            plan: self.plan().map(|lp| lp.plan.clone()),
            selected: self.plan().and_then(|lp| lp.tree.selected()),
            recent: self.settings.recent_files.clone(),
        }
    }

    pub fn open_palette(&mut self, initial: &str, window: &mut Window, cx: &mut Context<Self>) {
        let snapshot = self.snapshot();
        let initial = initial.to_string();
        let palette = cx.new(|cx| Palette::new(window, cx, snapshot, &initial));
        let confirm = cx.subscribe_in(&palette, window, |this, _, e: &PaletteEvent, window, cx| {
            let PaletteEvent::Confirmed(target) = e;
            this.close_palette(window, cx);
            this.run_target(target.clone(), window, cx);
        });
        let dismiss = cx.subscribe_in(&palette, window, |this, _, _: &DismissEvent, window, cx| this.close_palette(window, cx));
        self._subscriptions.push(confirm);
        self._subscriptions.push(dismiss);
        self.palette = Some(palette);
        cx.notify();
    }

    pub fn close_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.palette.take().is_some() {
            // Return keyboard focus to the main window.
            window.focus(&self.focus, cx);
            cx.notify();
        }
    }

    fn run_target(&mut self, target: Target, window: &mut Window, cx: &mut Context<Self>) {
        match target {
            Target::Action(a) => self.perform(a, window, cx),
            Target::Goto(id) => {
                self.select_node(id, window, cx);
                window.focus(&self.tree_focus, cx);
            }
            Target::Add(t) => self.add_element(t.tag, t.gui.unwrap_or_default(), window, cx),
            Target::OpenRecent(p) => {
                self.open_plan_path(Path::new(&p), window, cx);
            }
        }
    }

    // ---- actions ----

    pub fn perform(&mut self, action: Action, window: &mut Window, cx: &mut Context<Self>) {
        if !action.availability(&self.context()).is_enabled() {
            return;
        }
        // While a question is showing, nothing behind it changes. (Quit still asks again.)
        if self.dialog.is_some() && !matches!(action, Action::Quit | Action::About) {
            return;
        }
        // Text typed into a box but not yet confirmed is saved first, so it is part of what happens next.
        self.flush_form(window, cx);
        match action {
            Action::OpenPlan => self.open_dialog(window, cx),
            Action::SavePlan => {
                self.save_active(window, cx);
            }
            Action::SavePlanAs => self.save_active_as(window, cx),
            Action::ReloadPlan => self.reload_plan(window, cx),
            Action::Undo => self.undo(window, cx),
            Action::Redo => self.redo(window, cx),
            Action::DeleteElement => self.delete_selected(window, cx),
            Action::DuplicateElement => self.duplicate_selected(window, cx),
            Action::RenameElement => self.focus_name_field(window, cx),
            Action::ToggleElementEnabled => self.toggle_selected_enabled(window, cx),
            Action::MoveElementUp => self.move_selected(false, window, cx),
            Action::MoveElementDown => self.move_selected(true, window, cx),
            Action::CloseTab => {
                if let Some(i) = self.workspace.active() {
                    self.request_close_tab(i, window, cx);
                }
            }
            Action::CloseAllTabs => self.request_close_all(cx),
            Action::NextTab => {
                self.workspace.next_tab();
                self.sync_active_plan();
            }
            Action::PrevTab => {
                self.workspace.prev_tab();
                self.sync_active_plan();
            }
            Action::ToggleSidebar => {
                self.workspace.toggle_sidebar();
                self.save_settings();
            }
            Action::ToggleInspector => {
                self.workspace.toggle_inspector();
                self.save_settings();
            }
            Action::ToggleBottomPanel => {
                self.workspace.toggle_bottom();
                self.save_settings();
            }
            Action::FocusTree => {
                if !self.workspace.sidebar.visible {
                    self.workspace.toggle_sidebar();
                }
                window.focus(&self.tree_focus, cx);
            }
            Action::ExpandAll => {
                if let Some(lp) = self.plan_mut() {
                    let plan = lp.plan.clone();
                    lp.tree.expand_all(&plan);
                }
            }
            Action::CollapseAll => {
                if let Some(lp) = self.plan_mut() {
                    let plan = lp.plan.clone();
                    lp.tree.collapse_all(&plan);
                }
            }
            Action::GotoElement => self.open_palette("@", window, cx),
            Action::AddElement => self.open_palette("+", window, cx),
            Action::CommandPalette => self.open_palette("", window, cx),
            Action::ThemeSystem => self.set_theme(ThemeChoice::System, cx),
            Action::ThemeDark => self.set_theme(ThemeChoice::Dark, cx),
            Action::ThemeLight => self.set_theme(ThemeChoice::Light, cx),
            Action::OpenSettings => self.open_settings_tab(window, cx),
            Action::About => self.about_open = true,
            Action::RunPlan => self.begin_run(jvm_runtime::runs::RunKind::Run, None, window, cx),
            Action::ValidatePlan => self.begin_run(jvm_runtime::runs::RunKind::Validate, None, window, cx),
            Action::SendOnce => {
                let sampler = self.selected_id();
                self.begin_run(jvm_runtime::runs::RunKind::SendOnce, sampler, window, cx)
            }
            Action::StopRun => self.stop_run(false, window, cx),
            Action::StopRunNow => self.stop_run(true, window, cx),
            Action::ClearRunsFolder => self.request_clear_runs(window, cx),
            Action::OpenRunsFolder => self.open_runs_folder(window, cx),
            Action::Quit => {
                if self.request_quit(cx) {
                    self.save_window_state(window);
                    cx.quit();
                }
            }
        }
        cx.notify();
    }

    pub fn set_theme(&mut self, choice: ThemeChoice, cx: &mut Context<Self>) {
        self.settings.theme = choice;
        theme::apply(cx, choice);
        self.save_settings();
        cx.notify();
    }

    pub fn open_settings_tab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.settings_fields.is_none() {
            let home = self.settings.jmeter_home.clone().unwrap_or_default();
            let java = self.settings.java_path.clone().unwrap_or_default();
            let heap = self.settings.heap_mb.to_string();
            let interval = self.settings.summary_interval_secs.to_string();
            let jmeter_home = cx.new(|cx| TextField::new(cx, "settings-jmeter-home", "Folder that contains bin/ApacheJMeter.jar", &home));
            let java_path = cx.new(|cx| TextField::new(cx, "settings-java-path", "Leave empty to use JAVA_HOME or PATH", &java));
            let heap_mb = cx.new(|cx| TextField::new(cx, "settings-heap", "1024", &heap).number(HEAP_SPEC));
            let summary_secs = cx.new(|cx| TextField::new(cx, "settings-interval", "1", &interval).number(INTERVAL_SPEC));
            let keep = self.settings.runs_keep.to_string();
            let runs_keep = cx.new(|cx| TextField::new(cx, "settings-runs-keep", "10", &keep).number(KEEP_SPEC));
            let sub = cx.subscribe_in(&runs_keep, window, |this, _, e: &crate::fields::TextFieldEvent, window, cx| {
                if matches!(e, crate::fields::TextFieldEvent::Submitted | crate::fields::TextFieldEvent::Blurred) {
                    this.apply_runs_keep(window, cx);
                }
            });
            self._subscriptions.push(sub);
            let startup = cx.new(|_| ComboBox::new("startup-combo", &["Show the welcome screen", "Reopen the last plan"], if self.settings.startup == Startup::LastPlan { 1 } else { 0 }));
            for f in [&jmeter_home, &java_path, &heap_mb, &summary_secs] {
                let sub = cx.subscribe_in(f, window, |this, _, e: &crate::fields::TextFieldEvent, window, cx| {
                    if *e == crate::fields::TextFieldEvent::Submitted {
                        this.apply_runtime_settings(window, cx);
                    }
                });
                self._subscriptions.push(sub);
            }
            let sub = cx.subscribe(&startup, |this, _, e: &ComboEvent, cx| {
                let ComboEvent::Selected(i) = e;
                this.settings.startup = if *i == 1 { Startup::LastPlan } else { Startup::Welcome };
                this.save_settings();
                cx.notify();
            });
            self._subscriptions.push(sub);
            self.settings_fields = Some(SettingsFields { jmeter_home, java_path, heap_mb, summary_secs, startup, runs_keep });
        }
        self.workspace.open_tab(TabKind::Settings, "Settings");
        cx.notify();
    }

    /// Save the settings tab: Java and JMeter locations, heap size and summary interval. Numbers that are
    /// not valid are left unsaved and reported.
    pub fn apply_runtime_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(f) = &self.settings_fields else { return };
        let home = f.jmeter_home.read(cx).text().trim().to_string();
        let java = f.java_path.read(cx).text().trim().to_string();
        let heap = HEAP_SPEC.parse(f.heap_mb.read(cx).text());
        let interval = INTERVAL_SPEC.parse(f.summary_secs.read(cx).text());
        let (Ok(heap), Ok(interval)) = (heap, interval) else {
            self.notify("Some values are not valid. Fix the fields marked in red.", Severity::Error, window, cx);
            return;
        };
        self.settings.jmeter_home = (!home.is_empty()).then_some(home);
        self.settings.java_path = (!java.is_empty()).then_some(java);
        // Variables such as ${heap} are valid in plans but not here, where a real number is needed.
        match (heap, interval) {
            (Value::Number(h), Value::Number(i)) => {
                self.settings.heap_mb = h as u32;
                self.settings.summary_interval_secs = i as u32;
            }
            _ => {
                self.notify("Heap size and summary interval must be numbers.", Severity::Error, window, cx);
                return;
            }
        }
        self.save_settings();
        self.detect_runtime(window, cx);
        self.notify("Settings saved", Severity::Info, window, cx);
    }

    /// Save how many runs of each plan are kept, and remove the older ones of the open plan at once.
    pub fn apply_runs_keep(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(f) = &self.settings_fields else { return };
        match KEEP_SPEC.parse(f.runs_keep.read(cx).text()) {
            Ok(Value::Number(n)) if n as u32 != self.settings.runs_keep => {
                self.settings.runs_keep = n as u32;
                self.save_settings();
                if let Some(lp) = self.plan() {
                    let path = crate::run_ctl::plain_path(&lp.path);
                    let root = jvm_runtime::runs::runs_root(&path, &self.runs_place());
                    jvm_runtime::runs::prune(&root, &path, self.settings.runs_keep as usize);
                }
                self.refresh_history();
                cx.notify();
            }
            Ok(_) => {}
            Err(_) => self.notify("Keep how many runs must be a whole number from 1 to 1000.", Severity::Error, window, cx),
        }
    }

    /// Look for Java and JMeter without blocking the window.
    pub fn detect_runtime(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let java_path = self.settings.java_path.clone().map(PathBuf::from);
        let home = self.settings.jmeter_home.clone().map(PathBuf::from);
        cx.spawn_in(window, async move |this, cx| {
            let status = cx.background_executor().spawn(async move { check_runtime(java_path.as_deref(), home.as_deref()) }).await;
            this.update(cx, |this, cx| {
                this.runtime = status;
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    // ---- snapshot for tests and automation ----

    /// A JSON description of what is on screen, written when `--state` is given.
    pub fn state_json(&self, cx: &App) -> serde_json::Value {
        use serde_json::json;
        let tabs: Vec<_> = self.workspace.tabs().iter().map(|t| json!({ "title": t.title, "dirty": t.dirty })).collect();
        let palette = self.palette.as_ref().map(|p| {
            let p = p.read(cx);
            json!({ "query": p.query(cx), "mode": format!("{:?}", p.mode(cx)), "results": p.results().iter().map(|r| r.item.title.clone()).collect::<Vec<_>>(), "unavailable": p.results().iter().filter(|r| r.item.disabled.is_some()).map(|r| r.item.title.clone()).collect::<Vec<_>>(), "selected": p.selected_index() })
        });
        let (plan, selected, rows) = match self.plan() {
            Some(lp) => (
                Some(lp.path.to_string_lossy().to_string()),
                lp.tree.selected().and_then(|s| lp.plan.node(s)).map(|n| n.name.clone()),
                lp.tree.rows(&lp.plan).len(),
            ),
            None => (None, None, 0),
        };
        json!({
            "plan": plan,
            "elements": self.plan().map(|lp| lp.plan.len()).unwrap_or(0),
            "open_plans": self.plans.iter().map(|p| p.name()).collect::<Vec<_>>(),
            "visible_rows": rows,
            "selected": selected,
            "editor": self.plan().and_then(|lp| lp.tree.selected().or_else(|| lp.plan.roots().first().copied()).and_then(|id| lp.plan.node(id))).map(|n| json!({ "name": n.name, "type": n.type_label })),
            "tabs": tabs,
            "active_tab": self.workspace.active_tab().map(|t| t.title.clone()),
            "palette": palette,
            "theme": format!("{:?}", theme::choice(cx)),
            "dark": theme::palette(cx).dark,
            "sidebar": self.workspace.sidebar.visible,
            "inspector": self.workspace.inspector.visible,
            "bottom": self.workspace.bottom.visible,
            "notice": self.notice.as_ref().map(|n| n.text.clone()),
            "dialog": self.dialog.as_ref().map(|d| match d {
                Dialog::Unsaved { plans, then } => json!({ "kind": "unsaved", "plans": plans.len(), "then": format!("{then:?}") }),
                Dialog::DiscardForReload(_) => json!({ "kind": "reload" }),
                Dialog::Recover { .. } => json!({ "kind": "recover" }),
                Dialog::SaveToRun { kind, .. } => json!({ "kind": "save_to_run", "run": kind.label() }),
                Dialog::Scripts { text, .. } => json!({ "kind": "scripts", "text": text }),
                Dialog::RunsPlace { message, .. } => json!({ "kind": "runs_place", "message": message }),
                Dialog::ClearRuns { root, count } => json!({ "kind": "clear_runs", "root": root.to_string_lossy(), "count": count }),
                Dialog::DeleteRun { name, .. } => json!({ "kind": "delete_run", "name": name }),
                Dialog::QuitRunning => json!({ "kind": "quit_running" }),
            }),
            "run": self.run.as_ref().map(|r| {
                let t = r.live.total();
                json!({
                    "kind": r.kind.label(),
                    "phase": r.phase.label(),
                    "dir": r.dir.to_string_lossy(),
                    "samples": t.count,
                    "errors": t.errors,
                    "avg_ms": t.avg_ms(),
                    "p95_ms": t.percentile(95.0),
                    "points": r.live.series().len(),
                    "labels": r.live.labels().len(),
                    "error_rows": r.live.errors().count(),
                    "exit_code": r.exit_code,
                    "error": r.error,
                    "notes": r.notes,
                    "can_kill": r.can_kill(),
                    "skipped": r.skipped,
                })
            }),
            "history": self.history.iter().map(|e| json!({ "dir": e.dir.to_string_lossy(), "kind": e.meta.kind.label(), "outcome": e.meta.outcome.label(), "samples": e.meta.samples })).collect::<Vec<_>>(),
            "bottom_tab": format!("{:?}", self.bottom_tab),
            "runs_place": format!("{:?}", self.runs_place()),
            "runs_keep": self.settings.runs_keep,
            "runtime_ready": self.runtime_ready(),
            "can_undo": self.plan().is_some_and(|lp| lp.edit.can_undo()),
            "can_redo": self.plan().is_some_and(|lp| lp.edit.can_redo()),
            "dirty": self.typing_dirty || self.plan().is_some_and(|lp| lp.is_dirty()),
            "problems": self.plan().map(|lp| lp.problems().iter().map(|p| p.text.clone()).collect::<Vec<_>>()).unwrap_or_default(),
            "about": self.about_open,
            "java": self.runtime.java,
            "jmeter": self.runtime.jmeter,
        })
    }

    pub fn write_state_dump(&self, cx: &App) {
        if let Some(p) = &self.state_dump {
            let _ = std::fs::write(p, serde_json::to_string_pretty(&self.state_json(cx)).unwrap_or_default());
        }
    }
}

impl Focusable for Shell {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

/// How a path is shown and stored in the recent list (no `\\?\` prefix on Windows).
pub fn display_path(p: &Path) -> String {
    let s = p.to_string_lossy().to_string();
    s.strip_prefix(r"\\?\").map(str::to_string).unwrap_or(s)
}

/// Run the Java and JMeter checks. Blocking: call from a background task.
pub fn check_runtime(java_path: Option<&Path>, home: Option<&Path>) -> RuntimeStatus {
    let mut s = RuntimeStatus::default();
    let java = match jvm_runtime::locate::find_java(java_path) {
        Ok(j) => {
            s.java = Some(j.version_line.clone());
            Some(j)
        }
        Err(e) => {
            s.problem = Some(format!("{e:#}"));
            None
        }
    };
    if let (Some(j), Some(h)) = (java, home) {
        match jvm_runtime::locate::inspect_jmeter(h, &j) {
            Ok(info) => s.jmeter = Some(info.version),
            Err(e) => s.problem = Some(format!("{e:#}")),
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shortcut_conversion() {
        assert_eq!(to_gpui_keys("Ctrl+O"), "ctrl-o");
        assert_eq!(to_gpui_keys("Ctrl+Shift+Tab"), "ctrl-shift-tab");
        assert_eq!(to_gpui_keys("F5"), "f5");
        assert_eq!(to_gpui_keys("Ctrl+,"), "ctrl-,");
        assert_eq!(to_gpui_keys("Ctrl+\\"), "ctrl-\\");
    }

    #[test]
    fn every_shortcut_is_a_valid_keystroke() {
        for a in Action::ALL {
            if let Some(s) = a.shortcut() {
                if s == "Alt+F4" {
                    continue;
                }
                let keys = to_gpui_keys(s);
                for part in keys.split(' ') {
                    assert!(gpui::Keystroke::parse(part).is_ok(), "{s} -> {keys} is not valid");
                }
            }
        }
    }

    #[test]
    fn display_path_strips_the_verbatim_prefix() {
        assert_eq!(display_path(Path::new(r"\\?\C:\work\a.jmx")), r"C:\work\a.jmx");
        assert_eq!(display_path(Path::new("rel/a.jmx")), "rel/a.jmx");
    }

    #[test]
    fn runtime_check_reports_a_missing_java() {
        let s = check_runtime(Some(Path::new("Z:/no/such/java.exe")), None);
        // Falls back to JAVA_HOME and PATH, so either it works or it explains why not.
        assert!(s.java.is_some() || s.problem.is_some());
    }

    #[test]
    fn runtime_check_reports_a_bad_jmeter_folder() {
        let s = check_runtime(None, Some(Path::new("Z:/no/such/jmeter")));
        if s.java.is_some() {
            assert!(s.jmeter.is_none());
            assert!(s.problem.as_deref().unwrap_or("").contains("ApacheJMeter.jar"), "{:?}", s.problem);
        }
    }
}
