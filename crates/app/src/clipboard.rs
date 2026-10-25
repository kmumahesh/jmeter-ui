//! Copying, cutting and pasting elements, saving a selection on its own, and merging a plan file into the plan.
//!
//! Elements travel as text: a plan file that holds just them. That is the clipboard format, so a copy can be pasted
//! into another plan in this window or another, kept in a file, or sent to a colleague.

use crate::shell::{Severity, Shell};
use anyhow::Context as _;
use gpui::{ClipboardItem, Context, PathPromptOptions, Window};
use plan_core::files;
use std::path::Path;

impl Shell {
    /// Put the selected element, with everything under it, on the clipboard.
    pub fn copy_selected(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(id) = self.selected_id() else { return };
        let Some(lp) = self.plan() else { return };
        let name = lp.plan.node(id).map(|n| n.name.clone()).unwrap_or_default();
        let count = subtree_len(&lp.plan, id);
        match lp.edit.copy_text(&[id]) {
            Ok(text) => {
                cx.write_to_clipboard(ClipboardItem::new_string(text));
                self.notify(format!("Copied {name}{}", if count > 1 { format!(" and the {} elements under it", count - 1) } else { String::new() }), Severity::Info, window, cx);
            }
            Err(e) => self.notify(format!("{e:#}"), Severity::Error, window, cx),
        }
    }

    /// Copy the selected element, then remove it. Undo brings it back.
    pub fn cut_selected(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(id) = self.selected_id() else { return };
        let Some(lp) = self.plan() else { return };
        let name = lp.plan.node(id).map(|n| n.name.clone()).unwrap_or_default();
        match lp.edit.copy_text(&[id]) {
            Ok(text) => {
                cx.write_to_clipboard(ClipboardItem::new_string(text));
                self.delete_selected(window, cx);
                self.notify(format!("Cut {name}. Paste puts it back, and so does Ctrl+Z."), Severity::Info, window, cx);
            }
            Err(e) => self.notify(format!("{e:#}"), Severity::Error, window, cx),
        }
    }

    /// Put the elements on the clipboard into the plan: under the selected element if they fit, else next to it.
    pub fn paste_clipboard(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) else {
            self.notify("The clipboard holds no text. Copy an element first.", Severity::Error, window, cx);
            return;
        };
        self.paste_text(&text, window, cx);
    }

    /// Paste plan text at the selection (or under the Test Plan when nothing is selected). Returns whether it worked.
    pub fn paste_text(&mut self, text: &str, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let sel = self.selected_id();
        let text = text.to_string();
        let mut made = 0;
        let done = self.edit_active(window, cx, |e, plan| {
            let at = sel.filter(|&s| plan.node(s).is_some()).or_else(|| plan.roots().first().copied()).context("there is no element to paste into")?;
            let ids = e.paste_near(at, &text)?;
            made = ids.len();
            Ok(ids.first().copied())
        });
        if done {
            self.notify(if made == 1 { "Pasted 1 element".to_string() } else { format!("Pasted {made} elements") }, Severity::Info, window, cx);
        }
        done
    }

    /// Ask where to save the selected element and what is under it, as a plan file of its own.
    pub fn save_selection_as(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let dir = self.new_plan_folder();
        let name = self.selected_id().and_then(|id| self.plan().and_then(|lp| lp.plan.node(id))).map(|n| file_name_for(&n.name)).unwrap_or_else(|| "selection".to_string());
        let rx = cx.prompt_for_new_path(&dir, Some(&format!("{name}.jmx")));
        cx.spawn_in(window, async move |this, cx| {
            if let Ok(Ok(Some(path))) = rx.await {
                this.update_in(cx, |this, window, cx| {
                    this.save_selection_to(&path, window, cx);
                })
                .ok();
            }
        })
        .detach();
    }

    /// Write the selected element and what is under it to `path` (`.jmx` is added when there is no extension).
    pub fn save_selection_to(&mut self, path: &Path, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let Some(id) = self.selected_id() else { return false };
        let Some(lp) = self.plan() else { return false };
        let mut path = path.to_path_buf();
        if path.extension().is_none() {
            path.set_extension("jmx");
        }
        if path.is_dir() {
            self.notify(format!("{} is a folder. Pick a file name.", path.display()), Severity::Error, window, cx);
            return false;
        }
        // Never over a plan that is open: saving a selection must not replace the whole plan.
        let abs = std::fs::canonicalize(&path).unwrap_or_else(|_| path.clone());
        if self.plans.iter().any(|p| p.path == abs) {
            self.notify("That plan is open in a tab. Pick another name.", Severity::Error, window, cx);
            return false;
        }
        let text = match lp.edit.copy_text(&[id]) {
            Ok(t) => t,
            Err(e) => {
                self.notify(format!("{e:#}"), Severity::Error, window, cx);
                return false;
            }
        };
        match files::save(&path, &text) {
            Ok(()) => {
                self.notify(format!("Saved the selection as {}", path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()), Severity::Info, window, cx);
                true
            }
            Err(e) => {
                self.notify(format!("Could not save {}: {e}", path.display()), Severity::Error, window, cx);
                false
            }
        }
    }

    /// Ask for a plan file whose elements are brought into this plan.
    pub fn merge_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let rx = cx.prompt_for_paths(PathPromptOptions { files: true, directories: false, multiple: false, prompt: Some("Merge JMeter Plan".into()) });
        cx.spawn_in(window, async move |this, cx| {
            if let Ok(Ok(Some(paths))) = rx.await
                && let Some(path) = paths.into_iter().next()
            {
                this.update_in(cx, |this, window, cx| {
                    this.merge_file(&path, window, cx);
                })
                .ok();
            }
        })
        .detach();
    }

    /// Bring the elements of a plan file (or of a saved selection) into this plan, as a paste does.
    pub fn merge_file(&mut self, path: &Path, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let text = match files::read_text(path) {
            Ok(t) => t,
            Err(e) => {
                self.notify(format!("Cannot read {}: {e}", path.display()), Severity::Error, window, cx);
                return false;
            }
        };
        self.paste_text(&text, window, cx)
    }
}

/// How many elements an element and everything under it make.
fn subtree_len(plan: &plan_core::Plan, id: plan_core::NodeId) -> usize {
    1 + plan.node(id).map_or(0, |n| n.children.iter().map(|&c| subtree_len(plan, c)).sum())
}

/// A file name made from an element's name: letters, digits, dashes and underscores, never empty.
fn file_name_for(name: &str) -> String {
    let cleaned: String = name.chars().map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '-' }).collect();
    let cleaned = cleaned.trim_matches('-').to_lowercase();
    if cleaned.is_empty() { "selection".to_string() } else { cleaned.chars().take(60).collect() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_names_are_made_from_element_names() {
        assert_eq!(file_name_for("Log in"), "log-in");
        assert_eq!(file_name_for("a/b\\c:d*e?"), "a-b-c-d-e");
        assert_eq!(file_name_for("***"), "selection");
        assert_eq!(file_name_for(""), "selection");
        assert_eq!(file_name_for(&"x".repeat(200)).len(), 60);
        assert_eq!(file_name_for("Caf\u{e9}"), "caf\u{e9}");
    }
}
