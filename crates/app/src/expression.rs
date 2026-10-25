//! Try an expression: a dialog to type a JSON Path, regular expression, XPath or CSS selector and some text, and see
//! what JMeter's own libraries find in it. The text can be a response from the request viewer.

use crate::fields::TextField;
use crate::shell::{Dialog, Shell};
use crate::textarea::TextArea;
use gpui::{AppContext as _, Context, Window};
use jvm_runtime::tester::{self, Kind};
use std::path::Path;

/// The expression an extractor already holds and what kind it is, so the dialog starts with it.
fn from_element(tag: &str) -> Option<(Kind, &'static str)> {
    Some(match tag {
        "JSONPostProcessor" => (Kind::JsonPath, "JSONPostProcessor.jsonPathExprs"),
        "RegexExtractor" => (Kind::Regex, "RegexExtractor.regex"),
        "XPathExtractor" => (Kind::XPath, "XPathExtractor.xpathQuery"),
        "XPath2Extractor" => (Kind::XPath, "XPathExtractor.xpathQuery"),
        "HtmlExtractor" => (Kind::Css, "HtmlExtractor.expr"),
        _ => return None,
    })
}

/// What a finished test shows: how many things were found, then each one.
pub fn describe(found: &tester::Found) -> (String, String) {
    let n = found.items.len();
    let status = match (n, found.cut) {
        (0, _) => "Nothing found".to_string(),
        (1, false) => "1 found".to_string(),
        (_, false) => format!("{n} found"),
        (_, true) => format!("{n} found (more were cut, or a long one was)"),
    };
    let text = found.items.iter().enumerate().map(|(i, s)| if n > 1 && !s.starts_with("Match ") { format!("{}: {s}", i + 1) } else { s.clone() }).collect::<Vec<_>>().join("\n");
    (status, text)
}

impl Shell {
    /// Open the dialog. It starts from the selected extractor's expression when there is one, and from the answer of the
    /// request shown in the request viewer when there is one.
    pub fn open_expression_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (mut kind, mut expr) = (Kind::JsonPath, String::new());
        if let Some(lp) = self.plan()
            && let Some(id) = lp.tree.selected()
            && let Some((k, prop)) = lp.plan.node(id).and_then(|n| from_element(&n.tag))
        {
            kind = k;
            expr = lp.edit.prop(id, prop).unwrap_or_default();
        }
        let sample = if self.bottom_tab == crate::shell::BottomTab::Requests { self.run.as_ref().and_then(|r| r.details.get(self.req_sel)).map(|d| d.response.clone()).unwrap_or_default() } else { String::new() };
        self.open_expression_with(kind, &expr, &sample, window, cx);
    }

    pub fn open_expression_with(&mut self, kind: Kind, expr: &str, sample: &str, window: &mut Window, cx: &mut Context<Self>) {
        let field = cx.new(|cx| TextField::new(cx, "expr-text", "Type the expression here", expr));
        let sample = cx.new(|cx| TextArea::new(cx, "expr-sample", "Paste the text to try it on, for example a response", sample).rows(8));
        let result = cx.new(|cx| TextArea::new(cx, "expr-result", "", "").rows(6).read_only(true));
        let sub = cx.subscribe_in(&field, window, |this, _, e: &crate::fields::TextFieldEvent, window, cx| {
            match e {
                crate::fields::TextFieldEvent::Submitted => this.expression_try(window, cx),
                crate::fields::TextFieldEvent::Cancelled if matches!(this.dialog, Some(Dialog::Expression { .. })) => {
                    this.dialog_cancel(cx);
                    window.focus(&this.focus, cx);
                }
                _ => {}
            }
        });
        self._subscriptions.push(sub);
        field.update(cx, |f, cx| f.focus(window, cx));
        self.dialog = Some(Dialog::Expression { kind, expr: field, sample, result, status: None, error: false, busy: false });
        cx.notify();
    }

    /// Choose what kind of expression it is.
    pub fn expression_kind(&mut self, kind: Kind, cx: &mut Context<Self>) {
        if let Some(Dialog::Expression { kind: k, .. }) = &mut self.dialog {
            *k = kind;
        }
        cx.notify();
    }

    /// Run the expression on the text, in the background.
    pub fn expression_try(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let Some(Dialog::Expression { kind, expr, sample, result, busy, .. }) = self.dialog.clone() else { return };
        if busy {
            return;
        }
        let (kind_now, expression, text) = (kind, expr.read(cx).text().to_string(), sample.read(cx).text().to_string());
        let java = match jvm_runtime::locate::find_java(self.settings.java_path.as_deref().map(Path::new)) {
            Ok(j) => j.path,
            Err(e) => return self.expression_done(Err(format!("{e:#}")), cx),
        };
        let Some(home) = self.settings.jmeter_home.clone().map(std::path::PathBuf::from) else {
            return self.expression_done(Err("No JMeter folder is chosen in Settings".into()), cx);
        };
        if let Some(Dialog::Expression { busy, status, .. }) = &mut self.dialog {
            *busy = true;
            *status = Some("Trying\u{2026}".into());
        }
        result.update(cx, |a, cx| a.set_text("", cx));
        cx.spawn(async move |this, cx| {
            let outcome = cx.background_executor().spawn(async move { tester::evaluate(&tester::Tools { java: &java, jmeter_home: &home }, kind_now, &expression, &text, tester::DEFAULT_TIMEOUT) }).await;
            this.update(cx, |this, cx| this.expression_done(outcome, cx)).ok();
        })
        .detach();
        cx.notify();
    }

    fn expression_done(&mut self, outcome: Result<tester::Found, String>, cx: &mut Context<Self>) {
        let Some(Dialog::Expression { result, busy, status, error, .. }) = &mut self.dialog else { return };
        *busy = false;
        match outcome {
            Ok(found) => {
                let (s, text) = describe(&found);
                *status = Some(s);
                *error = false;
                result.update(cx, |a, cx| a.set_text(&text, cx));
            }
            Err(e) => {
                *status = Some(e);
                *error = true;
            }
        }
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jvm_runtime::tester::Found;

    #[test]
    fn extractors_start_the_dialog_with_their_expression() {
        assert_eq!(from_element("RegexExtractor").map(|x| x.0), Some(Kind::Regex));
        assert_eq!(from_element("JSONPostProcessor").map(|x| x.0), Some(Kind::JsonPath));
        assert_eq!(from_element("XPath2Extractor").map(|x| x.0), Some(Kind::XPath));
        assert_eq!(from_element("HtmlExtractor").map(|x| x.0), Some(Kind::Css));
        assert!(from_element("HTTPSamplerProxy").is_none());
    }

    #[test]
    fn what_was_found_is_counted_and_listed() {
        let f = |items: &[&str], cut| Found { items: items.iter().map(|s| s.to_string()).collect(), cut };
        assert_eq!(describe(&f(&[], false)), ("Nothing found".into(), String::new()));
        assert_eq!(describe(&f(&["a"], false)), ("1 found".into(), "a".into()));
        assert_eq!(describe(&f(&["a", "b"], false)), ("2 found".into(), "1: a\n2: b".into()));
        assert!(describe(&f(&["a", "b"], true)).0.contains("cut"));
        assert_eq!(describe(&f(&["Match 1: x", "Match 2: y"], false)).1, "Match 1: x\nMatch 2: y");
    }
}
