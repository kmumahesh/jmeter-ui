//! Import from cURL: a dialog to paste commands into, and what happens when they are added to the plan.

use crate::shell::{Dialog, Severity, Shell};
use crate::textarea::TextArea;
use gpui::{AppContext as _, Context, Window};
use plan_core::curl;

/// Whether text looks like a cURL command, to fill the dialog from the clipboard.
fn looks_like_curl(text: &str) -> bool {
    let t = text.trim_start();
    t.starts_with("curl ") || t.starts_with("curl.exe ") || t.starts_with("curl\n") || t.starts_with("curl\\")
}

impl Shell {
    /// Escape in a text box of a question closes the question.
    pub(crate) fn close_dialog_on_escape(&mut self, area: &gpui::Entity<TextArea>, window: &mut Window, cx: &mut Context<Self>) {
        let sub = cx.subscribe_in(area, window, |this, _, e: &crate::textarea::TextAreaEvent, window, cx| {
            if *e == crate::textarea::TextAreaEvent::Escaped && matches!(this.dialog, Some(Dialog::Curl { .. } | Dialog::RunProps { .. } | Dialog::Expression { .. })) {
                this.dialog_cancel(cx);
                window.focus(&this.focus, cx);
            }
        });
        self._subscriptions.push(sub);
    }

    /// Open the dialog. If the clipboard holds a curl command it is filled in already.
    pub fn open_curl_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let clip = cx.read_from_clipboard().and_then(|c| c.text()).filter(|t| looks_like_curl(t) && t.len() <= curl::MAX_INPUT).unwrap_or_default();
        let area = cx.new(|cx| TextArea::new(cx, "curl-text", "curl 'https://api.example.com/items' -H 'accept: application/json'", &clip).rows(9));
        area.update(cx, |a, cx| a.focus(window, cx));
        self.close_dialog_on_escape(&area, window, cx);
        self.dialog = Some(Dialog::Curl { area, error: None });
        cx.notify();
    }

    /// "Add to Plan": read the commands and put the requests in the plan like a paste. A problem stays in the dialog.
    pub fn dialog_curl_add(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(Dialog::Curl { area, .. }) = self.dialog.clone() else { return };
        let text = area.read(cx).text().to_string();
        match self.add_curl(&text, window, cx) {
            Ok(count) => {
                self.dialog = None;
                self.notify(if count == 1 { "Added 1 request from cURL".to_string() } else { format!("Added {count} requests from cURL") }, Severity::Info, window, cx);
            }
            Err(message) => {
                self.dialog = Some(Dialog::Curl { area, error: Some(message) });
            }
        }
        cx.notify();
    }

    /// Put the requests of some cURL commands in the plan, under the selected element or next to it. Returns how many,
    /// or what is wrong in words for the person who pasted them.
    pub fn add_curl(&mut self, text: &str, window: &mut Window, cx: &mut Context<Self>) -> Result<usize, String> {
        let requests = curl::parse(text).map_err(|e| format!("{e:#}"))?;
        let plan_text = curl::plan_text(&requests).map_err(|e| format!("Could not make the requests: {e:#}"))?;
        if self.paste_text(&plan_text, window, cx) { Ok(requests.len()) } else { Err(self.notice.as_ref().map(|n| n.text.clone()).unwrap_or_else(|| "The requests could not be added here.".to_string())) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clipboard_text_that_looks_like_curl_is_recognised() {
        assert!(looks_like_curl("curl https://x.org"));
        assert!(looks_like_curl("  \n curl 'https://x.org'"));
        assert!(looks_like_curl("curl.exe -X POST x"));
        assert!(looks_like_curl("curl\n  -H x"));
        assert!(!looks_like_curl("curling is a sport"));
        assert!(!looks_like_curl("hello"));
        assert!(!looks_like_curl(""));
    }
}
