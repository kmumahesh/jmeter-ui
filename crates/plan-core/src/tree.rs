//! Expand, collapse and selection state for the plan tree, and the rows to draw.

use crate::model::{NodeId, Plan};
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub id: NodeId,
    pub depth: usize,
    pub has_children: bool,
    pub expanded: bool,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Move {
    Up,
    Down,
    First,
    Last,
    PageUp(usize),
    PageDown(usize),
}

#[derive(Debug, Clone, Default)]
pub struct TreeState {
    expanded: HashSet<NodeId>,
    selected: Option<NodeId>,
}

impl TreeState {
    /// Expand every element that has children and sits above `depth` (0 expands nothing).
    pub fn expanded_to_depth(plan: &Plan, depth: usize) -> TreeState {
        let mut s = TreeState::default();
        for n in plan.iter() {
            if !n.children.is_empty() && plan.depth(n.id) < depth {
                s.expanded.insert(n.id);
            }
        }
        s
    }

    pub fn selected(&self) -> Option<NodeId> {
        self.selected
    }

    pub fn is_expanded(&self, id: NodeId) -> bool {
        self.expanded.contains(&id)
    }

    pub fn expand(&mut self, id: NodeId) {
        self.expanded.insert(id);
    }

    /// Collapse `id`. If the selection was inside it, the selection moves to `id`.
    pub fn collapse(&mut self, plan: &Plan, id: NodeId) {
        self.expanded.remove(&id);
        if let Some(sel) = self.selected
            && sel != id && plan.path_to(sel).contains(&id) {
                self.selected = Some(id);
            }
    }

    pub fn toggle(&mut self, plan: &Plan, id: NodeId) {
        if self.is_expanded(id) {
            self.collapse(plan, id);
        } else {
            self.expand(id);
        }
    }

    pub fn expand_all(&mut self, plan: &Plan) {
        self.expanded = plan.iter().filter(|n| !n.children.is_empty()).map(|n| n.id).collect();
    }

    /// Collapse everything. A selection below a root moves up to its root.
    pub fn collapse_all(&mut self, plan: &Plan) {
        self.expanded.clear();
        if let Some(sel) = self.selected
            && let Some(&root) = plan.path_to(sel).first() {
                self.selected = Some(root);
            }
    }

    /// Select `id` and expand its ancestors so it is visible.
    pub fn select(&mut self, plan: &Plan, id: NodeId) {
        if plan.node(id).is_none() {
            return;
        }
        let path = plan.path_to(id);
        for a in &path[..path.len() - 1] {
            self.expanded.insert(*a);
        }
        self.selected = Some(id);
    }

    pub fn clear_selection(&mut self) {
        self.selected = None;
    }

    /// The rows to draw: each visible element in order with its depth.
    pub fn rows(&self, plan: &Plan) -> Vec<Row> {
        let mut out = Vec::with_capacity(plan.len());
        for &r in plan.roots() {
            self.push_rows(plan, r, 0, &mut out);
        }
        out
    }

    fn push_rows(&self, plan: &Plan, id: NodeId, depth: usize, out: &mut Vec<Row>) {
        let Some(n) = plan.node(id) else { return };
        let expanded = !n.children.is_empty() && self.is_expanded(id);
        out.push(Row { id, depth, has_children: !n.children.is_empty(), expanded });
        if expanded {
            for &c in &n.children {
                self.push_rows(plan, c, depth + 1, out);
            }
        }
    }

    pub fn row_index(&self, plan: &Plan, id: NodeId) -> Option<usize> {
        self.rows(plan).iter().position(|r| r.id == id)
    }

    /// Move the selection among visible rows. With nothing selected, `Down` and `First` pick the
    /// first row and `Up` and `Last` pick the last. Returns the new selection.
    pub fn move_selection(&mut self, plan: &Plan, mv: Move) -> Option<NodeId> {
        let rows = self.rows(plan);
        if rows.is_empty() {
            return None;
        }
        let cur = self.selected.and_then(|s| rows.iter().position(|r| r.id == s));
        let last = rows.len() - 1;
        let next = match (mv, cur) {
            (Move::First, _) | (Move::Down, None) => 0,
            (Move::Last, _) | (Move::Up, None) => last,
            (Move::Up, Some(i)) => i.saturating_sub(1),
            (Move::Down, Some(i)) => (i + 1).min(last),
            (Move::PageUp(n), Some(i)) => i.saturating_sub(n),
            (Move::PageDown(n), Some(i)) => (i + n).min(last),
            (Move::PageUp(_), None) => 0,
            (Move::PageDown(_), None) => last,
        };
        self.selected = Some(rows[next].id);
        self.selected
    }

    /// Left arrow: collapse the selected element, or go to its parent if it is already collapsed.
    pub fn key_left(&mut self, plan: &Plan) {
        let Some(sel) = self.selected else { return };
        let Some(n) = plan.node(sel) else { return };
        if !n.children.is_empty() && self.is_expanded(sel) {
            self.collapse(plan, sel);
        } else if let Some(p) = n.parent {
            self.selected = Some(p);
        }
    }

    /// Right arrow: expand the selected element, or go to its first child if it is already expanded.
    pub fn key_right(&mut self, plan: &Plan) {
        let Some(sel) = self.selected else { return };
        let Some(n) = plan.node(sel) else { return };
        if n.children.is_empty() {
            return;
        }
        if self.is_expanded(sel) {
            self.selected = Some(n.children[0]);
        } else {
            self.expand(sel);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Shop > [Users > [Loop > [GET, POST], Think], Tree]
    fn plan() -> Plan {
        Plan::from_jmx_str(
            r#"<jmeterTestPlan><hashTree>
<TestPlan testname="Shop"/><hashTree>
 <ThreadGroup testname="Users"/><hashTree>
  <LoopController testname="Loop"/><hashTree>
   <HTTPSamplerProxy testname="GET"/><hashTree/>
   <HTTPSamplerProxy testname="POST"/><hashTree/>
  </hashTree>
  <ConstantTimer testname="Think"/><hashTree/>
 </hashTree>
 <ResultCollector testname="Tree"/><hashTree/>
</hashTree></hashTree></jmeterTestPlan>"#,
        )
        .unwrap()
    }

    fn id(p: &Plan, name: &str) -> NodeId {
        p.find_by_name(name).unwrap()
    }

    fn names(s: &TreeState, p: &Plan) -> Vec<String> {
        s.rows(p).iter().map(|r| format!("{}{}", " ".repeat(r.depth), p.node(r.id).unwrap().name)).collect()
    }

    #[test]
    fn collapsed_tree_shows_only_roots() {
        let p = plan();
        let s = TreeState::default();
        assert_eq!(names(&s, &p), ["Shop"]);
    }

    #[test]
    fn expanded_to_depth() {
        let p = plan();
        let s = TreeState::expanded_to_depth(&p, 2);
        assert_eq!(names(&s, &p), ["Shop", " Users", "  Loop", "  Think", " Tree"]);
        let s = TreeState::expanded_to_depth(&p, 0);
        assert_eq!(names(&s, &p), ["Shop"]);
        let s = TreeState::expanded_to_depth(&p, 9);
        assert_eq!(s.rows(&p).len(), p.len());
    }

    #[test]
    fn toggle_and_row_flags() {
        let p = plan();
        let mut s = TreeState::default();
        s.toggle(&p, id(&p, "Shop"));
        let rows = s.rows(&p);
        assert_eq!(rows.len(), 3);
        assert!(rows[0].expanded && rows[0].has_children);
        assert!(!rows[1].expanded && rows[1].has_children, "Users has children but is collapsed");
        assert!(!rows[2].has_children, "Tree is a leaf");
        s.toggle(&p, id(&p, "Shop"));
        assert_eq!(s.rows(&p).len(), 1);
    }

    #[test]
    fn select_expands_ancestors() {
        let p = plan();
        let mut s = TreeState::default();
        s.select(&p, id(&p, "POST"));
        assert_eq!(s.selected(), Some(id(&p, "POST")));
        assert!(s.is_expanded(id(&p, "Loop")) && s.is_expanded(id(&p, "Users")) && s.is_expanded(id(&p, "Shop")));
        assert_eq!(s.row_index(&p, id(&p, "POST")), Some(4));
    }

    #[test]
    fn collapsing_an_ancestor_moves_the_selection_to_it() {
        let p = plan();
        let mut s = TreeState::default();
        s.select(&p, id(&p, "POST"));
        s.collapse(&p, id(&p, "Users"));
        assert_eq!(s.selected(), Some(id(&p, "Users")));
        // Collapsing the selected element itself keeps it selected.
        s.collapse(&p, id(&p, "Users"));
        assert_eq!(s.selected(), Some(id(&p, "Users")));
    }

    #[test]
    fn expand_all_and_collapse_all() {
        let p = plan();
        let mut s = TreeState::default();
        s.expand_all(&p);
        assert_eq!(s.rows(&p).len(), 7);
        s.select(&p, id(&p, "GET"));
        s.collapse_all(&p);
        assert_eq!(s.rows(&p).len(), 1);
        assert_eq!(s.selected(), Some(id(&p, "Shop")));
    }

    #[test]
    fn arrow_navigation_stops_at_the_ends() {
        let p = plan();
        let mut s = TreeState::expanded_to_depth(&p, 2);
        assert_eq!(s.move_selection(&p, Move::Down), Some(id(&p, "Shop")), "first Down selects the first row");
        s.move_selection(&p, Move::Down);
        assert_eq!(s.selected(), Some(id(&p, "Users")));
        s.move_selection(&p, Move::Last);
        assert_eq!(s.selected(), Some(id(&p, "Tree")));
        s.move_selection(&p, Move::Down);
        assert_eq!(s.selected(), Some(id(&p, "Tree")), "Down at the end stays put");
        s.move_selection(&p, Move::First);
        s.move_selection(&p, Move::Up);
        assert_eq!(s.selected(), Some(id(&p, "Shop")), "Up at the start stays put");
    }

    #[test]
    fn up_with_no_selection_picks_the_last_row_and_pages_clamp() {
        let p = plan();
        let mut s = TreeState::expanded_to_depth(&p, 9);
        s.move_selection(&p, Move::Up);
        assert_eq!(s.selected(), Some(id(&p, "Tree")));
        s.move_selection(&p, Move::PageUp(100));
        assert_eq!(s.selected(), Some(id(&p, "Shop")));
        s.move_selection(&p, Move::PageDown(3));
        assert_eq!(s.row_index(&p, s.selected().unwrap()), Some(3));
        s.move_selection(&p, Move::PageDown(100));
        assert_eq!(s.selected(), Some(id(&p, "Tree")));
    }

    #[test]
    fn left_and_right_keys() {
        let p = plan();
        let mut s = TreeState::default();
        s.select(&p, id(&p, "Shop"));
        s.key_right(&p); // expand
        assert!(s.is_expanded(id(&p, "Shop")));
        s.key_right(&p); // go to first child
        assert_eq!(s.selected(), Some(id(&p, "Users")));
        s.key_left(&p); // Users is collapsed, so go to the parent
        assert_eq!(s.selected(), Some(id(&p, "Shop")));
        s.key_left(&p); // collapse Shop
        assert!(!s.is_expanded(id(&p, "Shop")));
        s.key_left(&p); // root, collapsed: nothing happens
        assert_eq!(s.selected(), Some(id(&p, "Shop")));
        s.select(&p, id(&p, "Tree"));
        s.key_right(&p); // leaf: nothing happens
        assert_eq!(s.selected(), Some(id(&p, "Tree")));
    }

    #[test]
    fn selecting_a_missing_node_is_ignored() {
        let p = plan();
        let mut s = TreeState::default();
        s.select(&p, NodeId(999));
        assert_eq!(s.selected(), None);
        s.clear_selection();
        assert_eq!(s.move_selection(&Plan::default(), Move::Down), None);
    }
}
