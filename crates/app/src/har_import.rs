//! Import from a HAR file: choose a file saved from a browser's developer tools, choose what to bring in, and the requests
//! are added to the plan like a paste.

use crate::fields::TextField;
use crate::shell::{Dialog, Severity, Shell};
use gpui::{AppContext as _, Context, PathPromptOptions, Window};
use plan_core::har::{self, Options, Summary};
use plan_core::curl;
use std::path::{Path, PathBuf};
use std::rc::Rc;

impl Shell {
    /// Requests go under the selected element when it takes them; with the Test Plan itself selected, or nothing, they go
    /// under the first thread group, which is where a person importing a recording means them to be.
    pub(crate) fn aim_import_at_a_thread_group(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(lp) = self.plan() else { return };
        let selected = lp.tree.selected();
        let at_root = selected.is_none_or(|s| lp.plan.node(s).is_none_or(|n| n.parent.is_none()));
        if !at_root {
            return;
        }
        let first_group = lp.plan.iter().find(|n| n.category == plan_core::Category::ThreadGroup).map(|n| n.id);
        if let Some(id) = first_group {
            self.select_node(id, window, cx);
        }
    }

    /// Ask for a HAR file and open the dialog for it.
    pub fn open_har_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let rx = cx.prompt_for_paths(PathPromptOptions { files: true, directories: false, multiple: false, prompt: Some("Choose a HAR file".into()) });
        cx.spawn_in(window, async move |this, cx| {
            if let Ok(Ok(Some(paths))) = rx.await
                && let Some(path) = paths.into_iter().next()
            {
                this.update_in(cx, |this, window, cx| this.open_har_path(&path, window, cx)).ok();
            }
        })
        .detach();
    }

    /// Read the file in the background and show what it holds.
    pub fn open_har_path(&mut self, path: &Path, window: &mut Window, cx: &mut Context<Self>) {
        if self.plan().is_none() {
            self.notify("Open a plan first", Severity::Info, window, cx);
            return;
        }
        let path = path.to_path_buf();
        cx.spawn_in(window, async move |this, cx| {
            let file = path.clone();
            let read = cx.background_executor().spawn(async move { read_har(&file) }).await;
            this.update_in(cx, |this, window, cx| match read {
                Ok(text) => this.show_har_dialog(path, Rc::new(text), window, cx),
                Err(e) => this.notify(e, Severity::Error, window, cx),
            })
            .ok();
        })
        .detach();
    }

    fn show_har_dialog(&mut self, path: PathBuf, raw: Rc<String>, window: &mut Window, cx: &mut Context<Self>) {
        let hosts = cx.new(|cx| TextField::new(cx, "har-hosts", "All servers, or names separated by commas", ""));
        let sub = cx.subscribe_in(&hosts, window, |this, _, e: &crate::fields::TextFieldEvent, window, cx| {
            if matches!(e, crate::fields::TextFieldEvent::Submitted | crate::fields::TextFieldEvent::Blurred) {
                this.har_refresh(cx);
            }
            if *e == crate::fields::TextFieldEvent::Cancelled && matches!(this.dialog, Some(Dialog::Har { .. })) {
                this.dialog_cancel(cx);
                window.focus(&this.focus, cx);
            }
        });
        self._subscriptions.push(sub);
        self.dialog = Some(Dialog::Har { path, raw, skip_static: true, group_by_page: true, hosts, summary: None, error: None });
        self.har_refresh(cx);
        cx.notify();
    }

    /// The choices as they are in the dialog.
    fn har_options(&self, cx: &Context<Self>) -> Option<Options> {
        let Some(Dialog::Har { skip_static, group_by_page, hosts, .. }) = &self.dialog else { return None };
        Some(Options { skip_static: *skip_static, group_by_page: *group_by_page, hosts: hosts.read(cx).text().split(',').map(|h| h.trim().to_string()).filter(|h| !h.is_empty()).collect() })
    }

    /// Read the file again with the choices as they are now, and show what that gives.
    pub fn har_refresh(&mut self, cx: &mut Context<Self>) {
        let Some(options) = self.har_options(cx) else { return };
        let Some(Dialog::Har { raw, summary, error, .. }) = &mut self.dialog else { return };
        match har::parse(raw, &options) {
            Ok((_, s)) => {
                *summary = Some(s);
                *error = None;
            }
            Err(e) => {
                *error = Some(format!("{e:#}"));
                // Keep the counts of the file itself so the person can see why nothing is left.
                if let Ok((_, s)) = har::parse(raw, &Options { skip_static: false, group_by_page: false, hosts: Vec::new() }) {
                    *summary = Some(Summary { kept: 0, ..s });
                }
            }
        }
        cx.notify();
    }

    pub fn har_set(&mut self, skip_static: Option<bool>, group_by_page: Option<bool>, cx: &mut Context<Self>) {
        if let Some(Dialog::Har { skip_static: s, group_by_page: g, .. }) = &mut self.dialog {
            if let Some(v) = skip_static {
                *s = v;
            }
            if let Some(v) = group_by_page {
                *g = v;
            }
        }
        self.har_refresh(cx);
    }

    /// "Add to Plan": the requests go in like a paste. A problem stays in the dialog.
    pub fn dialog_har_add(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(options) = self.har_options(cx) else { return };
        let Some(Dialog::Har { raw, .. }) = self.dialog.clone() else { return };
        self.aim_import_at_a_thread_group(window, cx);
        let made = har::parse(&raw, &options).and_then(|(requests, _)| curl::plan_text(&requests).map(|t| (t, requests.len())));
        match made {
            Ok((text, count)) => {
                if self.paste_text(&text, window, cx) {
                    self.dialog = None;
                    self.notify(format!("Added {count} request{} from the HAR file", if count == 1 { "" } else { "s" }), Severity::Info, window, cx);
                } else if let Some(Dialog::Har { error, .. }) = &mut self.dialog {
                    *error = Some(self.notice.as_ref().map(|n| n.text.clone()).unwrap_or_else(|| "The requests could not be added here.".to_string()));
                }
            }
            Err(e) => {
                if let Some(Dialog::Har { error, .. }) = &mut self.dialog {
                    *error = Some(format!("{e:#}"));
                }
            }
        }
        cx.notify();
    }
}

/// Read a HAR file as text, refusing one that is too large, empty or not text.
fn read_har(path: &Path) -> Result<String, String> {
    let meta = std::fs::metadata(path).map_err(|e| format!("{} cannot be read: {e}", path.display()))?;
    if !meta.is_file() {
        return Err(format!("{} is not a file.", path.display()));
    }
    if meta.len() == 0 {
        return Err(format!("{} is empty.", path.display()));
    }
    if meta.len() as usize > har::MAX_BYTES {
        return Err(format!("{} is larger than {} MB.", path.display(), har::MAX_BYTES / (1024 * 1024)));
    }
    let bytes = std::fs::read(path).map_err(|e| format!("{} cannot be read: {e}", path.display()))?;
    String::from_utf8(bytes).map_err(|_| format!("{} is not text, so it is not a HAR file.", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn files_that_cannot_be_read_as_har_text_are_refused_with_a_reason() {
        let d = std::env::temp_dir().join(format!("jmeter-ui-har-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        assert!(read_har(&d.join("missing.har")).unwrap_err().contains("cannot be read"));
        assert!(read_har(&d).unwrap_err().contains("is not a file"));
        std::fs::write(d.join("empty.har"), "").unwrap();
        assert!(read_har(&d.join("empty.har")).unwrap_err().contains("is empty"));
        std::fs::write(d.join("bin.har"), [0xff, 0xfe, 0x00]).unwrap();
        assert!(read_har(&d.join("bin.har")).unwrap_err().contains("not text"));
        std::fs::write(d.join("ok.har"), "{}").unwrap();
        assert_eq!(read_har(&d.join("ok.har")).unwrap(), "{}");
        let _ = std::fs::remove_dir_all(d);
    }
}
