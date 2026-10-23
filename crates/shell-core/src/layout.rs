//! The arrangement of the window: three side panels and one row of tabs.
//!
//! Each open plan has exactly one tab. What the middle area shows for a plan is decided by the tree
//! selection, not by more tabs. Settings is the only other kind of tab.

use crate::settings::{BOTTOM_RANGE, INSPECTOR_RANGE, SIDEBAR_RANGE, Settings};

/// Identifies an open plan for as long as it stays open in this window.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct PlanId(pub u32);

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum TabKind {
    Plan(PlanId),
    Settings,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Tab {
    pub kind: TabKind,
    pub title: String,
    /// Unsaved edits, shown as a dot on the tab.
    pub dirty: bool,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Panel {
    pub visible: bool,
    /// Width for side panels, height for the bottom panel.
    pub size: f32,
    range: (f32, f32),
}

impl Panel {
    fn new(visible: bool, size: f32, range: (f32, f32)) -> Panel {
        Panel { visible, size: size.clamp(range.0, range.1), range }
    }

    pub fn resize(&mut self, size: f32) {
        if size.is_finite() {
            self.size = size.clamp(self.range.0, self.range.1);
        }
    }
}

#[derive(Clone, Debug)]
pub struct Workspace {
    tabs: Vec<Tab>,
    active: Option<usize>,
    pub sidebar: Panel,
    pub inspector: Panel,
    pub bottom: Panel,
}

impl Workspace {
    pub fn new(s: &Settings) -> Workspace {
        Workspace {
            tabs: Vec::new(),
            active: None,
            sidebar: Panel::new(s.sidebar_visible, s.sidebar_width, SIDEBAR_RANGE),
            inspector: Panel::new(s.inspector_visible, s.inspector_width, INSPECTOR_RANGE),
            bottom: Panel::new(s.bottom_visible, s.bottom_height, BOTTOM_RANGE),
        }
    }

    /// Copy panel state back into settings for saving.
    pub fn write_to(&self, s: &mut Settings) {
        s.sidebar_visible = self.sidebar.visible;
        s.sidebar_width = self.sidebar.size;
        s.inspector_visible = self.inspector.visible;
        s.inspector_width = self.inspector.size;
        s.bottom_visible = self.bottom.visible;
        s.bottom_height = self.bottom.size;
    }

    pub fn tabs(&self) -> &[Tab] {
        &self.tabs
    }

    pub fn active(&self) -> Option<usize> {
        self.active
    }

    pub fn active_tab(&self) -> Option<&Tab> {
        self.active.and_then(|i| self.tabs.get(i))
    }

    pub fn tab_count(&self) -> usize {
        self.tabs.len()
    }

    pub fn find(&self, kind: TabKind) -> Option<usize> {
        self.tabs.iter().position(|t| t.kind == kind)
    }

    /// Show a tab: if it is already open, activate it; otherwise open it to the right of the active
    /// tab. Returns its index.
    pub fn open_tab(&mut self, kind: TabKind, title: &str) -> usize {
        if let Some(i) = self.find(kind) {
            self.active = Some(i);
            return i;
        }
        let at = self.active.map_or(self.tabs.len(), |a| a + 1);
        self.tabs.insert(at, Tab { kind, title: title.to_string(), dirty: false });
        self.active = Some(at);
        at
    }

    /// Close a tab. The neighbour to the right becomes active, or the one to the left.
    pub fn close_tab(&mut self, index: usize) -> bool {
        if index >= self.tabs.len() {
            return false;
        }
        self.tabs.remove(index);
        self.active = if self.tabs.is_empty() {
            None
        } else {
            match self.active {
                Some(a) if a > index => Some(a - 1),
                Some(a) if a == index => Some(index.min(self.tabs.len() - 1)),
                other => other,
            }
        };
        true
    }

    pub fn close_active(&mut self) -> bool {
        match self.active {
            Some(i) => self.close_tab(i),
            None => false,
        }
    }

    pub fn close_others(&mut self, keep: usize) {
        if keep < self.tabs.len() {
            let t = self.tabs.remove(keep);
            self.tabs = vec![t];
            self.active = Some(0);
        }
    }

    pub fn close_all(&mut self) {
        self.tabs.clear();
        self.active = None;
    }

    pub fn rename_tab(&mut self, kind: TabKind, title: &str) {
        if let Some(i) = self.find(kind) {
            self.tabs[i].title = title.to_string();
        }
    }

    pub fn set_dirty(&mut self, kind: TabKind, dirty: bool) {
        if let Some(i) = self.find(kind) {
            self.tabs[i].dirty = dirty;
        }
    }

    pub fn activate(&mut self, index: usize) -> bool {
        if index < self.tabs.len() {
            self.active = Some(index);
            true
        } else {
            false
        }
    }

    /// Activate the next tab, wrapping around.
    pub fn next_tab(&mut self) {
        self.step_tab(1);
    }

    pub fn prev_tab(&mut self) {
        self.step_tab(-1);
    }

    fn step_tab(&mut self, delta: isize) {
        let n = self.tabs.len() as isize;
        if n < 2 {
            return;
        }
        let cur = self.active.unwrap_or(0) as isize;
        self.active = Some((cur + delta).rem_euclid(n) as usize);
    }

    pub fn toggle_sidebar(&mut self) {
        self.sidebar.visible = !self.sidebar.visible;
    }

    pub fn toggle_inspector(&mut self) {
        self.inspector.visible = !self.inspector.visible;
    }

    pub fn toggle_bottom(&mut self) {
        self.bottom.visible = !self.bottom.visible;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ws() -> Workspace {
        Workspace::new(&Settings::default())
    }

    fn plan(n: u32) -> TabKind {
        TabKind::Plan(PlanId(n))
    }

    fn titles(w: &Workspace) -> Vec<String> {
        w.tabs().iter().map(|t| t.title.clone()).collect()
    }

    #[test]
    fn starts_empty_with_panels_from_settings() {
        let s = Settings { sidebar_visible: false, bottom_visible: true, bottom_height: 300.0, ..Settings::default() };
        let w = Workspace::new(&s);
        assert!(w.active_tab().is_none());
        assert_eq!(w.tab_count(), 0);
        assert!(!w.sidebar.visible && w.bottom.visible);
        assert_eq!(w.bottom.size, 300.0);
    }

    #[test]
    fn the_inspector_is_hidden_by_default() {
        assert!(!ws().inspector.visible);
    }

    #[test]
    fn opening_a_tab_places_it_after_the_active_one() {
        let mut w = ws();
        w.open_tab(plan(1), "A");
        w.open_tab(plan(2), "B");
        w.open_tab(TabKind::Settings, "Settings");
        w.activate(0);
        w.open_tab(plan(3), "C");
        assert_eq!(titles(&w), ["A", "C", "B", "Settings"]);
        assert_eq!(w.active_tab().unwrap().title, "C");
    }

    #[test]
    fn a_plan_never_gets_a_second_tab() {
        let mut w = ws();
        w.open_tab(plan(1), "A");
        w.open_tab(plan(2), "B");
        let at = w.open_tab(plan(1), "A again");
        assert_eq!(at, 0);
        assert_eq!(w.tab_count(), 2);
        assert_eq!(w.active_tab().unwrap().title, "A");
        w.open_tab(TabKind::Settings, "Settings");
        w.open_tab(TabKind::Settings, "Settings");
        assert_eq!(w.tab_count(), 3);
    }

    #[test]
    fn closing_activates_the_right_neighbour_then_the_left() {
        let mut w = ws();
        for (i, t) in ["A", "B", "C"].iter().enumerate() {
            w.open_tab(plan(i as u32), t);
        }
        w.activate(1);
        w.close_active();
        assert_eq!(w.active_tab().unwrap().title, "C", "right neighbour");
        w.close_active();
        assert_eq!(w.active_tab().unwrap().title, "A", "no right neighbour, so left");
        w.close_active();
        assert!(w.active_tab().is_none());
        assert!(!w.close_active(), "nothing left to close");
    }

    #[test]
    fn closing_a_tab_before_the_active_one_keeps_it_active() {
        let mut w = ws();
        for i in 0..3 {
            w.open_tab(plan(i), &format!("T{i}"));
        }
        assert_eq!(w.active_tab().unwrap().title, "T2");
        w.close_tab(0);
        assert_eq!(w.active_tab().unwrap().title, "T2");
        assert!(!w.close_tab(9));
    }

    #[test]
    fn next_and_previous_wrap() {
        let mut w = ws();
        for i in 0..3 {
            w.open_tab(plan(i), &format!("T{i}"));
        }
        w.next_tab();
        assert_eq!(w.active_tab().unwrap().title, "T0");
        w.prev_tab();
        assert_eq!(w.active_tab().unwrap().title, "T2");
        let mut one = ws();
        one.open_tab(plan(1), "x");
        one.next_tab();
        assert_eq!(one.active_tab().unwrap().title, "x");
    }

    #[test]
    fn close_others_and_close_all() {
        let mut w = ws();
        for i in 0..4 {
            w.open_tab(plan(i), &format!("T{i}"));
        }
        w.close_others(2);
        assert_eq!(titles(&w), ["T2"]);
        w.close_all();
        assert_eq!(w.tab_count(), 0);
        assert!(w.active().is_none());
    }

    #[test]
    fn rename_and_dirty_apply_to_the_matching_tab_only() {
        let mut w = ws();
        w.open_tab(plan(7), "Old");
        w.open_tab(plan(8), "Other");
        w.rename_tab(plan(7), "New");
        w.set_dirty(plan(7), true);
        assert_eq!(w.tabs()[0].title, "New");
        assert!(w.tabs()[0].dirty && !w.tabs()[1].dirty);
        w.rename_tab(plan(99), "ignored");
        w.set_dirty(plan(99), true);
        assert_eq!(w.tab_count(), 2);
    }

    #[test]
    fn panel_sizes_clamp_and_ignore_nan() {
        let mut w = ws();
        w.sidebar.resize(10.0);
        assert_eq!(w.sidebar.size, SIDEBAR_RANGE.0);
        w.sidebar.resize(f32::NAN);
        assert_eq!(w.sidebar.size, SIDEBAR_RANGE.0);
        w.bottom.resize(5000.0);
        assert_eq!(w.bottom.size, BOTTOM_RANGE.1);
    }

    #[test]
    fn toggles_and_saving_back_to_settings() {
        let mut w = ws();
        w.toggle_sidebar();
        w.toggle_bottom();
        w.toggle_inspector();
        w.inspector.resize(400.0);
        let mut s = Settings::default();
        w.write_to(&mut s);
        assert!(!s.sidebar_visible && s.bottom_visible && s.inspector_visible);
        assert_eq!(s.inspector_width, 400.0);
        w.toggle_inspector();
        assert!(!w.inspector.visible);
    }
}
