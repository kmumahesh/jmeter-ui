//! Add Think Times to Children: a pause before every request under the selected element, with the shortest and longest
//! pause chosen in a small question.

use crate::fields::TextField;
use crate::shell::{Dialog, Severity, Shell};
use anyhow::Context as _;
use gpui::{AppContext as _, Context, Window};
use plan_core::edit::MAX_THINK_MS;
use shell_core::number::NumberSpec;

/// The shortest and longest pause the question starts with, in milliseconds.
pub const DEFAULT_MIN_MS: &str = "1000";
pub const DEFAULT_MAX_MS: &str = "3000";

fn spec() -> NumberSpec {
    NumberSpec::whole(0, MAX_THINK_MS as i64)
}

/// A pause in milliseconds from the text of a box, or what is wrong with it in words.
pub fn pause_ms(label: &str, text: &str) -> Result<u32, String> {
    let t = text.trim();
    if t.is_empty() {
        return Err(format!("Write the {label} pause in milliseconds."));
    }
    let n: u32 = t.parse().map_err(|_| format!("The {label} pause must be a whole number of milliseconds."))?;
    if n > MAX_THINK_MS {
        return Err(format!("The {label} pause cannot be longer than {} minutes.", MAX_THINK_MS / 60_000));
    }
    Ok(n)
}

impl Shell {
    /// Whether the selection can hold requests, so that think times can be added under it.
    pub(crate) fn selection_can_take_think_times(&self) -> bool {
        use plan_core::Category::*;
        self.plan().zip(self.selected_id()).and_then(|(lp, id)| lp.plan.node(id)).is_some_and(|n| matches!(n.category, TestPlan | ThreadGroup | Controller | Sampler))
    }

    /// Open the question for the selected element.
    pub fn open_think_times_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(target) = self.selected_id().filter(|_| self.selection_can_take_think_times()) else {
            self.notify("Select a thread group, a controller or a request first", Severity::Info, window, cx);
            return;
        };
        let min = cx.new(|cx| TextField::new(cx, "think-min", "1000", DEFAULT_MIN_MS).number(spec()));
        let max = cx.new(|cx| TextField::new(cx, "think-max", "3000", DEFAULT_MAX_MS).number(spec()));
        for f in [&min, &max] {
            let sub = cx.subscribe_in(f, window, |this, _, e: &crate::fields::TextFieldEvent, window, cx| match e {
                crate::fields::TextFieldEvent::Submitted => this.dialog_think_add(window, cx),
                crate::fields::TextFieldEvent::Cancelled if matches!(this.dialog, Some(Dialog::ThinkTimes { .. })) => {
                    this.dialog_cancel(cx);
                    window.focus(&this.focus, cx);
                }
                _ => {}
            });
            self._subscriptions.push(sub);
        }
        min.update(cx, |f, cx| f.focus(window, cx));
        self.dialog = Some(Dialog::ThinkTimes { target, min, max, error: None });
        cx.notify();
    }

    /// "Add Think Times": check the two pauses and add them. A problem stays in the question.
    pub fn dialog_think_add(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(Dialog::ThinkTimes { target, min, max, .. }) = self.dialog.clone() else { return };
        let (min_text, max_text) = (min.read(cx).text().to_string(), max.read(cx).text().to_string());
        let pauses = pause_ms("shortest", &min_text).and_then(|a| pause_ms("longest", &max_text).map(|b| (a, b)));
        let (a, b) = match pauses {
            Ok(x) => x,
            Err(e) => {
                self.dialog = Some(Dialog::ThinkTimes { target, min, max, error: Some(e) });
                cx.notify();
                return;
            }
        };
        let mut added = 0;
        let mut failure: Option<String> = None;
        let done = self.edit_active(window, cx, |e, plan| {
            plan.node(target).context("the element is gone")?;
            match e.add_think_times(target, a, b) {
                Ok(n) => {
                    added = n;
                    Ok(Some(target))
                }
                Err(err) => {
                    failure = Some(format!("{err:#}"));
                    Err(err)
                }
            }
        });
        if done {
            self.dialog = None;
            window.focus(&self.focus, cx);
            let range = if a == b { format!("{a} ms") } else { format!("{a} to {b} ms") };
            self.notify(format!("Added {added} think time{} ({range})", if added == 1 { "" } else { "s" }), Severity::Info, window, cx);
        } else {
            let message = failure.unwrap_or_else(|| self.notice.as_ref().map(|n| n.text.clone()).unwrap_or_else(|| "The think times could not be added.".into()));
            self.dialog = Some(Dialog::ThinkTimes { target, min, max, error: Some(message) });
        }
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pauses_are_whole_milliseconds_within_the_limit() {
        assert_eq!(pause_ms("shortest", " 1500 "), Ok(1500));
        assert_eq!(pause_ms("shortest", "0"), Ok(0));
        assert_eq!(pause_ms("longest", &MAX_THINK_MS.to_string()), Ok(MAX_THINK_MS));
        for bad in ["", "  ", "abc", "-1", "1.5", "1e3", "${pause}", "99999999999999999999"] {
            assert!(pause_ms("longest", bad).is_err(), "{bad:?}");
        }
        assert!(pause_ms("longest", &(MAX_THINK_MS + 1).to_string()).unwrap_err().contains("60 minutes"));
    }
}
