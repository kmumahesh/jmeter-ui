//! What the right-click menu on an element in the tree holds, as plain rows, so the order and the separators can be
//! checked without a window. The window turns each row into a menu entry.

use crate::actions::Action;
use plan_core::catalog::{self, AddableType, Category};
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

/// One group of the "Add" list: a kind of element and the types of that kind that can go under the element.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AddGroup {
    pub category: Category,
    pub types: Vec<AddableType>,
}

/// What the "Add" list of an element of this category holds: the same types the Inspector lists, grouped by kind in
/// the order of the kinds, each group sorted by name. A kind with nothing to add is left out; an unknown element
/// has no list. The list is one flat list with the kind as a heading, not a menu inside a menu: a pointer that
/// travels to a row of a menu inside a menu crosses the other kinds on the way and opens one of them instead.
pub fn add_groups(category: Option<Category>) -> Vec<AddGroup> {
    let Some(parent) = category else { return Vec::new() };
    let all = catalog::addable();
    rules::allowed_children(parent)
        .into_iter()
        .map(|c| AddGroup { category: c, types: all.iter().filter(|t| t.category == c).cloned().collect() })
        .filter(|g| !g.types.is_empty())
        .collect()
}

/// One entry of the Recent Plans submenu.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecentEntry {
    /// What the menu shows: the file name, with its folder after it when two plans have the same file name.
    pub label: String,
    /// The full path, shown beside the entry and opened when it is chosen.
    pub path: String,
    /// The file is gone, so the entry is shown but cannot be chosen.
    pub missing: bool,
}

/// The entries of the Recent Plans submenu, newest first, at most `limit`. A file name that appears twice is shown with its
/// folder so the two can be told apart. `exists` says whether a file is still there.
pub fn recent_entries(recent: &[String], limit: usize, exists: impl Fn(&str) -> bool) -> Vec<RecentEntry> {
    // The last part of the path; a path that ends in a separator uses the part before it, and one that is only separators
    // is shown as it is.
    let name_of = |p: &str| {
        let trimmed = p.trim_end_matches(['/', '\\']);
        let last = trimmed.rsplit(['/', '\\']).next().unwrap_or(trimmed);
        if last.is_empty() { p.to_string() } else { last.to_string() }
    };
    let shown: Vec<&String> = recent.iter().filter(|p| !p.trim().is_empty()).take(limit).collect();
    shown
        .iter()
        .map(|p| {
            let name = name_of(p);
            let twins = shown.iter().filter(|q| name_of(q).eq_ignore_ascii_case(&name)).count();
            let label = if twins > 1 {
                // Split on either kind of separator so a Windows path is read the same on every system.
                let trimmed = p.trim_end_matches(['/', '\\']);
                let folder = trimmed.rfind(['/', '\\']).map(|i| trimmed[..i].to_string()).unwrap_or_default();
                if folder.is_empty() { name } else { format!("{name}  ({folder})") }
            } else {
                name
            };
            RecentEntry { label, path: (*p).clone(), missing: !exists(p) }
        })
        .collect()
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
    fn recent_plans_are_listed_newest_first_by_file_name() {
        let r = vec!["C:\\work\\shop.jmx".to_string(), "C:\\work\\api.jmx".to_string()];
        let e = recent_entries(&r, 10, |_| true);
        assert_eq!(e.iter().map(|x| x.label.as_str()).collect::<Vec<_>>(), ["shop.jmx", "api.jmx"]);
        assert_eq!(e[0].path, "C:\\work\\shop.jmx");
        assert!(e.iter().all(|x| !x.missing));
    }

    #[test]
    fn two_plans_with_one_file_name_show_their_folders() {
        let r = vec!["C:\\a\\test.jmx".to_string(), "C:\\b\\test.jmx".to_string(), "C:\\a\\other.jmx".to_string()];
        let e = recent_entries(&r, 10, |_| true);
        assert_ne!(e[0].label, e[1].label, "{e:?}");
        assert!(e[0].label.contains("test.jmx") && e[0].label.contains('a'));
        assert_eq!(e[2].label, "other.jmx", "a name that is not repeated stays short");
    }

    #[test]
    fn the_list_is_capped_and_a_missing_file_is_marked_not_dropped() {
        let r: Vec<String> = (0..25).map(|i| format!("C:\\p\\{i}.jmx")).collect();
        assert_eq!(recent_entries(&r, 10, |_| true).len(), 10);
        let e = recent_entries(&r[..3], 10, |p| !p.ends_with("1.jmx"));
        assert_eq!(e.iter().map(|x| x.missing).collect::<Vec<_>>(), [false, true, false]);
    }

    #[test]
    fn odd_entries_never_break_the_list() {
        let r = vec![String::new(), "   ".to_string(), "no-folder.jmx".to_string(), "C:/slashes/forward.jmx".to_string(), "trailing\\".to_string(), "\u{202e}evil.jmx".to_string()];
        let e = recent_entries(&r, 10, |_| true);
        assert!(e.iter().all(|x| !x.label.is_empty()));
        assert!(e.iter().any(|x| x.label == "forward.jmx" && x.path == "C:/slashes/forward.jmx"));
        assert!(recent_entries(&[], 10, |_| true).is_empty());
        assert!(recent_entries(&r, 0, |_| true).is_empty());
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
    fn the_add_list_holds_exactly_what_can_be_added() {
        for parent in Category::ALL {
            let groups = add_groups(Some(parent));
            let listed: Vec<_> = groups.iter().flat_map(|g| g.types.iter()).collect();
            let expected: Vec<_> = catalog::addable().into_iter().filter(|t| rules::can_add(parent, t.category)).collect();
            assert_eq!(listed.len(), expected.len(), "{parent:?}");
            for t in &expected {
                assert!(listed.contains(&t), "{parent:?} is missing {t:?}");
            }
            for g in &groups {
                assert!(!g.types.is_empty() && g.types.iter().all(|t| t.category == g.category), "{parent:?} {:?}", g.category);
                assert!(g.types.windows(2).all(|w| w[0].label <= w[1].label), "{parent:?} {:?} is not sorted", g.category);
            }
            assert_eq!(!groups.is_empty(), !rules::allowed_children(parent).is_empty(), "{parent:?}");
        }
        assert!(add_groups(None).is_empty());
    }

    #[test]
    fn a_test_plan_can_be_given_a_setup_and_a_teardown_thread_group() {
        let groups = add_groups(Some(Category::TestPlan));
        let thread = groups.iter().find(|g| g.category == Category::ThreadGroup).expect("thread groups");
        let labels: Vec<_> = thread.types.iter().map(|t| t.label).collect();
        for want in ["Thread Group", "setUp Thread Group", "tearDown Thread Group"] {
            assert!(labels.contains(&want), "{labels:?}");
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
