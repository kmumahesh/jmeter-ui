//! Everything the user can do from menus, the palette and keyboard shortcuts.

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Action {
    OpenPlan,
    SavePlan,
    SavePlanAs,
    ReloadPlan,
    CloseTab,
    CloseAllTabs,
    NextTab,
    PrevTab,
    Undo,
    Redo,
    DeleteElement,
    DuplicateElement,
    RenameElement,
    ToggleElementEnabled,
    MoveElementUp,
    MoveElementDown,
    ToggleSidebar,
    ToggleInspector,
    ToggleBottomPanel,
    FocusTree,
    ExpandAll,
    CollapseAll,
    /// Open the palette in "go to element" mode.
    GotoElement,
    /// Open the palette in "add element" mode.
    AddElement,
    CommandPalette,
    ThemeSystem,
    ThemeDark,
    ThemeLight,
    OpenSettings,
    About,
    RunPlan,
    StopRun,
    Quit,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Group {
    File,
    Navigate,
    Edit,
    View,
    Run,
    Help,
}

impl Group {
    pub fn label(self) -> &'static str {
        match self {
            Group::File => "File",
            Group::Navigate => "Navigate",
            Group::Edit => "Edit",
            Group::View => "View",
            Group::Run => "Run",
            Group::Help => "Help",
        }
    }
}

/// What the app currently has open, which decides which actions are available.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct AppContext {
    pub has_plan: bool,
    /// The plan has changes that are not saved.
    pub is_dirty: bool,
    pub can_undo: bool,
    pub can_redo: bool,
    /// An element is selected in the tree.
    pub has_selection: bool,
    /// The selected element may be removed, duplicated and moved (the Test Plan may not).
    pub selection_editable: bool,
    pub can_move_up: bool,
    pub can_move_down: bool,
    pub tab_count: usize,
    pub run_in_progress: bool,
    /// Whether this build can run plans at all.
    pub can_run: bool,
    /// Whether this build can change the plan (add, move, delete elements).
    pub can_edit: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Availability {
    Enabled,
    Disabled(&'static str),
}

impl Availability {
    pub fn is_enabled(&self) -> bool {
        matches!(self, Availability::Enabled)
    }
}

impl Action {
    pub const ALL: [Action; 33] = [
        Action::OpenPlan,
        Action::SavePlan,
        Action::SavePlanAs,
        Action::ReloadPlan,
        Action::CloseTab,
        Action::CloseAllTabs,
        Action::NextTab,
        Action::PrevTab,
        Action::Undo,
        Action::Redo,
        Action::DeleteElement,
        Action::DuplicateElement,
        Action::RenameElement,
        Action::ToggleElementEnabled,
        Action::MoveElementUp,
        Action::MoveElementDown,
        Action::ToggleSidebar,
        Action::ToggleInspector,
        Action::ToggleBottomPanel,
        Action::FocusTree,
        Action::ExpandAll,
        Action::CollapseAll,
        Action::GotoElement,
        Action::AddElement,
        Action::CommandPalette,
        Action::ThemeSystem,
        Action::ThemeDark,
        Action::ThemeLight,
        Action::OpenSettings,
        Action::About,
        Action::RunPlan,
        Action::StopRun,
        Action::Quit,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Action::OpenPlan => "Open Plan…",
            Action::SavePlan => "Save Plan",
            Action::SavePlanAs => "Save Plan As…",
            Action::Undo => "Undo",
            Action::Redo => "Redo",
            Action::DeleteElement => "Delete Element",
            Action::DuplicateElement => "Duplicate Element",
            Action::RenameElement => "Rename Element",
            Action::ToggleElementEnabled => "Enable or Disable Element",
            Action::MoveElementUp => "Move Element Up",
            Action::MoveElementDown => "Move Element Down",
            Action::ReloadPlan => "Reload Plan from Disk",
            Action::CloseTab => "Close Tab",
            Action::CloseAllTabs => "Close All Tabs",
            Action::NextTab => "Next Tab",
            Action::PrevTab => "Previous Tab",
            Action::ToggleSidebar => "Toggle Test Plan Panel",
            Action::ToggleInspector => "Toggle Inspector",
            Action::ToggleBottomPanel => "Toggle Results Panel",
            Action::FocusTree => "Focus Test Plan Tree",
            Action::ExpandAll => "Expand All Elements",
            Action::CollapseAll => "Collapse All Elements",
            Action::GotoElement => "Go to Element…",
            Action::AddElement => "Add Element…",
            Action::CommandPalette => "Open Command Palette",
            Action::ThemeSystem => "Color Theme: Follow System",
            Action::ThemeDark => "Color Theme: Dark",
            Action::ThemeLight => "Color Theme: Light",
            Action::OpenSettings => "Open Settings",
            Action::About => "About JMeter UI",
            Action::RunPlan => "Run Plan",
            Action::StopRun => "Stop Run",
            Action::Quit => "Quit",
        }
    }

    pub fn group(self) -> Group {
        match self {
            Action::OpenPlan | Action::SavePlan | Action::SavePlanAs | Action::ReloadPlan | Action::CloseTab | Action::CloseAllTabs | Action::OpenSettings | Action::Quit => Group::File,
            Action::Undo
            | Action::Redo
            | Action::DeleteElement
            | Action::DuplicateElement
            | Action::RenameElement
            | Action::ToggleElementEnabled
            | Action::MoveElementUp
            | Action::MoveElementDown
            | Action::AddElement => Group::Edit,
            Action::NextTab | Action::PrevTab | Action::FocusTree | Action::GotoElement | Action::CommandPalette => Group::Navigate,
            Action::ToggleSidebar
            | Action::ToggleInspector
            | Action::ToggleBottomPanel
            | Action::ExpandAll
            | Action::CollapseAll
            | Action::ThemeSystem
            | Action::ThemeDark
            | Action::ThemeLight => Group::View,
            Action::RunPlan | Action::StopRun => Group::Run,
            Action::About => Group::Help,
        }
    }

    /// Shortcut as shown to the user, for example `Ctrl+O`.
    pub fn shortcut(self) -> Option<&'static str> {
        match self {
            Action::OpenPlan => Some("Ctrl+O"),
            Action::SavePlan => Some("Ctrl+S"),
            Action::SavePlanAs => Some("Ctrl+Shift+S"),
            Action::Undo => Some("Ctrl+Z"),
            Action::Redo => Some("Ctrl+Y"),
            Action::DeleteElement => Some("Delete"),
            Action::DuplicateElement => Some("Ctrl+D"),
            Action::RenameElement => Some("F2"),
            Action::ToggleElementEnabled => Some("Ctrl+T"),
            Action::MoveElementUp => Some("Alt+Up"),
            Action::MoveElementDown => Some("Alt+Down"),
            Action::CloseTab => Some("Ctrl+W"),
            Action::NextTab => Some("Ctrl+Tab"),
            Action::PrevTab => Some("Ctrl+Shift+Tab"),
            Action::ToggleSidebar => Some("Ctrl+B"),
            Action::ToggleInspector => Some("Ctrl+Alt+B"),
            Action::ToggleBottomPanel => Some("Ctrl+J"),
            Action::FocusTree => Some("Ctrl+Shift+E"),
            Action::GotoElement => Some("Ctrl+P"),
            Action::AddElement => Some("Ctrl+Shift+A"),
            Action::CommandPalette => Some("Ctrl+K"),
            Action::OpenSettings => Some("Ctrl+,"),
            Action::RunPlan => Some("F5"),
            Action::StopRun => Some("Shift+F5"),
            Action::Quit => Some("Alt+F4"),
            _ => None,
        }
    }

    /// Shortcuts that only work while the tree has the focus, so a stray key elsewhere never changes the plan.
    pub fn tree_only(self) -> bool {
        matches!(self, Action::DeleteElement | Action::RenameElement | Action::MoveElementUp | Action::MoveElementDown | Action::DuplicateElement | Action::ToggleElementEnabled)
    }

    pub fn availability(self, c: &AppContext) -> Availability {
        use Availability::*;
        match self {
            Action::ReloadPlan | Action::ExpandAll | Action::CollapseAll | Action::GotoElement | Action::FocusTree if !c.has_plan => Disabled("Open a plan first"),
            Action::AddElement if !c.has_plan => Disabled("Open a plan first"),
            Action::AddElement if !c.can_edit => Disabled("Editing is not available"),
            Action::SavePlan | Action::SavePlanAs if !c.has_plan => Disabled("Open a plan first"),
            Action::SavePlan if !c.is_dirty => Disabled("There is nothing to save"),
            Action::Undo if !c.can_undo => Disabled("Nothing to undo"),
            Action::Redo if !c.can_redo => Disabled("Nothing to redo"),
            Action::DeleteElement | Action::DuplicateElement | Action::RenameElement | Action::ToggleElementEnabled | Action::MoveElementUp | Action::MoveElementDown if !c.has_plan => Disabled("Open a plan first"),
            Action::DeleteElement | Action::DuplicateElement | Action::RenameElement | Action::ToggleElementEnabled | Action::MoveElementUp | Action::MoveElementDown if !c.has_selection => Disabled("Select an element first"),
            Action::DeleteElement | Action::DuplicateElement | Action::MoveElementUp | Action::MoveElementDown if !c.selection_editable => Disabled("The Test Plan itself cannot be changed this way"),
            Action::MoveElementUp if !c.can_move_up => Disabled("The element is already first"),
            Action::MoveElementDown if !c.can_move_down => Disabled("The element is already last"),
            Action::CloseTab | Action::NextTab | Action::PrevTab if c.tab_count == 0 => Disabled("No tab is open"),
            Action::NextTab | Action::PrevTab if c.tab_count < 2 => Disabled("Only one tab is open"),
            Action::CloseAllTabs if c.tab_count == 0 => Disabled("No tab is open"),
            Action::RunPlan if !c.can_run => Disabled("Running plans arrives in a later release"),
            Action::RunPlan if !c.has_plan => Disabled("Open a plan first"),
            Action::RunPlan if c.run_in_progress => Disabled("A run is already in progress"),
            Action::StopRun if !c.run_in_progress => Disabled("Nothing is running"),
            _ => Enabled,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn all_lists_every_action_once() {
        let set: HashSet<_> = Action::ALL.iter().collect();
        assert_eq!(set.len(), Action::ALL.len());
        for a in Action::ALL {
            assert!(!a.title().is_empty());
            let _ = a.group().label();
        }
    }

    #[test]
    fn shortcuts_are_unique() {
        let mut seen = HashSet::new();
        for a in Action::ALL {
            if let Some(s) = a.shortcut() {
                assert!(seen.insert(s), "{s} is used twice");
            }
        }
    }

    #[test]
    fn nothing_open() {
        let c = AppContext::default();
        for a in [Action::ReloadPlan, Action::GotoElement, Action::ExpandAll, Action::CollapseAll, Action::CloseTab, Action::CloseAllTabs, Action::NextTab, Action::RunPlan, Action::StopRun] {
            assert!(!a.availability(&c).is_enabled(), "{a:?}");
        }
        for a in [Action::OpenPlan, Action::OpenSettings, Action::About, Action::Quit, Action::CommandPalette, Action::ToggleSidebar, Action::ThemeDark] {
            assert!(a.availability(&c).is_enabled(), "{a:?}");
        }
    }

    #[test]
    fn with_a_plan_and_tabs() {
        let c = AppContext { has_plan: true, tab_count: 3, ..Default::default() };
        for a in [Action::ReloadPlan, Action::GotoElement, Action::ExpandAll, Action::CloseTab, Action::NextTab, Action::PrevTab] {
            assert!(a.availability(&c).is_enabled(), "{a:?}");
        }
        assert!(!Action::RunPlan.availability(&c).is_enabled(), "this build cannot run yet");
    }

    #[test]
    fn next_tab_needs_two_tabs() {
        let one = AppContext { tab_count: 1, ..Default::default() };
        assert_eq!(Action::NextTab.availability(&one), Availability::Disabled("Only one tab is open"));
        assert!(Action::CloseTab.availability(&one).is_enabled());
    }

    #[test]
    fn run_and_stop_follow_the_run_state() {
        let ready = AppContext { has_plan: true, can_run: true, ..Default::default() };
        assert!(Action::RunPlan.availability(&ready).is_enabled());
        assert!(!Action::StopRun.availability(&ready).is_enabled());
        let running = AppContext { run_in_progress: true, ..ready };
        assert!(!Action::RunPlan.availability(&running).is_enabled());
        assert!(Action::StopRun.availability(&running).is_enabled());
    }

    #[test]
    fn saving_needs_unsaved_changes_and_save_as_only_a_plan() {
        let clean = AppContext { has_plan: true, ..Default::default() };
        assert_eq!(Action::SavePlan.availability(&clean), Availability::Disabled("There is nothing to save"));
        assert!(Action::SavePlanAs.availability(&clean).is_enabled());
        let dirty = AppContext { is_dirty: true, ..clean };
        assert!(Action::SavePlan.availability(&dirty).is_enabled());
        assert!(!Action::SavePlanAs.availability(&AppContext::default()).is_enabled());
    }

    #[test]
    fn undo_and_redo_follow_the_history() {
        let c = AppContext { has_plan: true, can_undo: true, ..Default::default() };
        assert!(Action::Undo.availability(&c).is_enabled());
        assert!(!Action::Redo.availability(&c).is_enabled());
        let c = AppContext { can_undo: false, can_redo: true, ..c };
        assert!(!Action::Undo.availability(&c).is_enabled());
        assert!(Action::Redo.availability(&c).is_enabled());
    }

    #[test]
    fn element_commands_need_a_selection_the_test_plan_is_special() {
        let none = AppContext { has_plan: true, ..Default::default() };
        for a in [Action::DeleteElement, Action::DuplicateElement, Action::RenameElement, Action::ToggleElementEnabled, Action::MoveElementUp, Action::MoveElementDown] {
            assert_eq!(a.availability(&none), Availability::Disabled("Select an element first"), "{a:?}");
        }
        let root = AppContext { has_selection: true, ..none };
        assert!(Action::RenameElement.availability(&root).is_enabled(), "the Test Plan can be renamed");
        assert!(Action::ToggleElementEnabled.availability(&root).is_enabled());
        for a in [Action::DeleteElement, Action::DuplicateElement, Action::MoveElementUp, Action::MoveElementDown] {
            assert!(!a.availability(&root).is_enabled(), "{a:?}");
        }
        let el = AppContext { selection_editable: true, can_move_up: true, ..root };
        assert!(Action::DeleteElement.availability(&el).is_enabled());
        assert!(Action::MoveElementUp.availability(&el).is_enabled());
        assert_eq!(Action::MoveElementDown.availability(&el), Availability::Disabled("The element is already last"));
    }

    #[test]
    fn every_group_has_actions() {
        for g in [Group::File, Group::Navigate, Group::View, Group::Run, Group::Help] {
            assert!(Action::ALL.iter().any(|a| a.group() == g), "{g:?}");
        }
    }
}

#[cfg(test)]
mod edit_tests {
    use super::*;

    #[test]
    fn adding_follows_the_edit_capability() {
        let on = AppContext { has_plan: true, can_edit: true, ..Default::default() };
        assert!(Action::AddElement.availability(&on).is_enabled());
        let off = AppContext { can_edit: false, ..on };
        assert!(!Action::AddElement.availability(&off).is_enabled());
        let no_plan = AppContext { has_plan: false, ..on };
        assert_eq!(Action::AddElement.availability(&no_plan), Availability::Disabled("Open a plan first"));
    }
}
