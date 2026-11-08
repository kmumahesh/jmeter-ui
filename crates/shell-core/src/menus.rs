//! What the right-click menu on an element in the tree holds, as plain rows, so the order and the separators can be
//! checked without a window. The window turns each row into a menu entry.

use crate::actions::Action;
use plan_core::catalog::Category;
use plan_core::rules;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Row {
    /// The "Add" submenu with what can go under the element.
    Add,
    /// A command. Enable and Disable are one row (`ToggleElementEnabled`); the window names it.
    Command(Action),
    /// Send only this request once (requests only).
    SendOnce,
    Separator,
}

/// The rows of the menu for an element of this category (`None` when it is not known). A separator is never first, last
/// or next to another separator.
pub fn tree_menu_rows(category: Option<Category>) -> Vec<Row> {
    let mut rows = Vec::new();
    if category.is_some_and(|c| !rules::allowed_children(c).is_empty()) {
        rows.push(Row::Add);
        rows.push(Row::Separator);
    }
    rows.extend([Row::Command(Action::CutElement), Row::Command(Action::CopyElement), Row::Command(Action::PasteElement), Row::Separator]);
    rows.extend([Row::Command(Action::DuplicateElement), Row::Command(Action::DeleteElement), Row::Command(Action::RenameElement), Row::Command(Action::ToggleElementEnabled), Row::Separator]);
    if category == Some(Category::Sampler) {
        rows.push(Row::SendOnce);
    }
    rows.extend([Row::Command(Action::MoveElementUp), Row::Command(Action::MoveElementDown), Row::Separator]);
    rows.extend([Row::Command(Action::ExpandAll), Row::Command(Action::CollapseAll)]);
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds() -> Vec<Option<Category>> {
        let mut v: Vec<Option<Category>> = Category::ALL.iter().map(|&c| Some(c)).collect();
        v.push(None);
        v
    }

    #[test]
    fn a_separator_is_never_first_last_or_next_to_another() {
        for c in kinds() {
            let rows = tree_menu_rows(c);
            assert_ne!(rows.first(), Some(&Row::Separator), "{c:?} starts with a separator");
            assert_ne!(rows.last(), Some(&Row::Separator), "{c:?} ends with a separator");
            for pair in rows.windows(2) {
                assert!(!(pair[0] == Row::Separator && pair[1] == Row::Separator), "{c:?} has two separators in a row: {rows:?}");
            }
        }
    }

    #[test]
    fn a_request_has_one_separator_after_enable_and_disable() {
        let rows = tree_menu_rows(Some(Category::Sampler));
        let at = rows.iter().position(|r| *r == Row::Command(Action::ToggleElementEnabled)).unwrap();
        assert_eq!(rows[at + 1], Row::Separator);
        assert_eq!(rows[at + 2], Row::SendOnce, "exactly one separator between Disable and Send once");
    }

    #[test]
    fn send_once_is_only_for_requests_and_add_only_where_something_can_go() {
        for c in kinds() {
            let rows = tree_menu_rows(c);
            assert_eq!(rows.contains(&Row::SendOnce), c == Some(Category::Sampler), "{c:?}");
            let takes_children = c.is_some_and(|c| !rules::allowed_children(c).is_empty());
            assert_eq!(rows.contains(&Row::Add), takes_children, "{c:?}");
        }
    }

    #[test]
    fn every_kind_of_element_has_the_same_commands_in_the_same_order() {
        let commands = |c| tree_menu_rows(c).into_iter().filter_map(|r| if let Row::Command(a) = r { Some(a) } else { None }).collect::<Vec<_>>();
        let first = commands(Some(Category::TestPlan));
        for c in kinds() {
            assert_eq!(commands(c), first, "{c:?}");
        }
        assert_eq!(first.len(), 11);
    }
}
