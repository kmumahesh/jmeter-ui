//! The lower panel while running and looking back: the live numbers and charts of a run, and the list of
//! earlier runs of the open plan.

use crate::assets::MONO_FONT;
use crate::chart::line_chart;
use crate::running::{Phase, RunSession};
use crate::shell::{BottomTab, RunDetail, Shell};
use crate::theme::{self, Palette, with_alpha};
use gpui::{AnyElement, Context, FontWeight, IntoElement, div, prelude::*, px};
use jvm_runtime::runs::Outcome;
use ui::{Button, ButtonCommon, ButtonSize, ButtonStyle, Clickable, Disableable};

/// Rows shown in the label table and the error list (the numbers above still count everything).
const MAX_ROWS: usize = 200;

/// `1:05` or `1:02:03` for a number of seconds.
pub fn clock(secs: u64) -> String {
    if secs >= 3600 { format!("{}:{:02}:{:02}", secs / 3600, secs % 3600 / 60, secs % 60) } else { format!("{}:{:02}", secs / 60, secs % 60) }
}

/// A count with thousands separators: `1,234,567`.
pub fn thousands(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// A rate with one decimal for small numbers: `0.4`, `12.5`, `1,234`.
pub fn rate(r: f64) -> String {
    if !r.is_finite() {
        "-".into()
    } else if r >= 100.0 {
        thousands(r.round() as u64)
    } else {
        format!("{r:.1}")
    }
}

fn phase_color(phase: Phase, p: &Palette) -> gpui::Hsla {
    match phase {
        Phase::Starting | Phase::Running | Phase::Stopping | Phase::Loading => p.accent,
        Phase::Finished => p.ok,
        Phase::Stopped => p.warn,
        Phase::Failed => p.err,
    }
}

impl Shell {
    /// The Run tab of the lower panel.
    pub(crate) fn run_view(&self, cx: &mut Context<Self>) -> AnyElement {
        let p = theme::palette(cx);
        let Some(run) = &self.run else {
            return div()
                .p(px(16.))
                .flex()
                .flex_col()
                .gap(px(4.))
                .text_color(p.muted)
                .child(div().text_color(p.fg).child("No run yet"))
                .child("Run the plan with F5. The numbers and charts of the run appear here, and every run is kept in the History tab.")
                .into_any_element();
        };
        let t = run.live.total();
        let step = run.live.step_secs().max(1) as f32;
        let series = run.live.series();
        let kpi = |id: &'static str, label: &'static str, value: String, color: gpui::Hsla| {
            div()
                .id(id)
                .debug_selector(move || id.to_string())
                .flex_1()
                .min_w(px(90.))
                .px(px(10.))
                .py(px(6.))
                .rounded(px(6.))
                .bg(p.panel2)
                .child(div().text_size(px(10.5)).font_family(MONO_FONT).text_color(p.muted).child(label))
                .child(div().text_size(px(17.)).font_weight(FontWeight::SEMIBOLD).text_color(color).child(value))
        };
        let chart = |id: &'static str, title: &'static str, now: String, values: Vec<f32>, color: gpui::Hsla| {
            div()
                .id(id)
                .debug_selector(move || id.to_string())
                .flex_1()
                .min_w(px(120.))
                .flex()
                .flex_col()
                .gap(px(2.))
                .child(div().flex().justify_between().text_size(px(10.5)).font_family(MONO_FONT).text_color(p.muted).child(title).child(div().text_color(p.fg).child(now)))
                .child(line_chart(values, color, with_alpha(p.line, 0.7)))
        };
        let avg_series: Vec<f32> = series.iter().map(|x| x.avg_ms() as f32).collect();
        let rate_series: Vec<f32> = series.iter().map(|x| x.samples as f32 / step).collect();
        let err_series: Vec<f32> = series.iter().map(|x| x.errors as f32).collect();
        let user_series: Vec<f32> = series.iter().map(|x| x.threads as f32).collect();
        let err_color = if t.errors > 0 { p.err } else { p.fg };

        let detail_tab = |id: &'static str, label: String, which: RunDetail, cx: &mut Context<Self>| {
            let active = self.run_detail == which;
            div()
                .id(id)
                .debug_selector(move || id.to_string())
                .px(px(10.))
                .py(px(4.))
                .text_size(px(12.))
                .cursor_pointer()
                .when(active, |d| d.text_color(p.fg).border_b_2().border_color(p.accent))
                .when(!active, |d| d.text_color(p.muted).hover(|d| d.text_color(p.fg)))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.run_detail = which;
                    cx.notify();
                }))
                .child(label)
        };
        let detail: AnyElement = match self.run_detail {
            RunDetail::Errors => self.run_errors(run, &p),
            RunDetail::Labels => self.run_labels(run, &p),
            RunDetail::Log => self.run_log(run, &p),
        };
        let elapsed = run.elapsed().as_secs();
        let active = run.phase.active();

        div()
            .id("run-view")
            .size_full()
            .flex()
            .flex_col()
            .gap(px(8.))
            .p(px(12.))
            // Header.
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .child(div().size(px(9.)).rounded_full().bg(phase_color(run.phase, &p)))
                    .child(div().id("run-phase").debug_selector(|| "run-phase".to_string()).font_weight(FontWeight::SEMIBOLD).child(format!("{} \u{b7} {}", run.kind.label(), run.phase.label())))
                    .child(div().text_color(p.muted).child(format!("{} \u{b7} {}", run.plan_name, clock(elapsed))))
                    .child(div().flex_1())
                    .when(active, |d| {
                        d.child(div().debug_selector(|| "run-stop".to_string()).child(Button::new("run-stop", "Stop").style(ButtonStyle::Subtle).size(ButtonSize::Compact).disabled(run.phase == Phase::Stopping).on_click(cx.listener(|this, _, w, cx| this.stop_run(false, w, cx)))))
                            .child(div().debug_selector(|| "run-stop-now".to_string()).child(Button::new("run-stop-now", "Stop now").style(ButtonStyle::Subtle).size(ButtonSize::Compact).on_click(cx.listener(|this, _, w, cx| this.stop_run(true, w, cx)))))
                    })
                    .when(run.can_kill(), |d| d.child(div().debug_selector(|| "run-kill".to_string()).child(Button::new("run-kill", "End JMeter").style(ButtonStyle::Tinted(ui::TintColor::Error)).size(ButtonSize::Compact).on_click(cx.listener(|this, _, _, cx| this.kill_run(cx))))))
                    .child(div().debug_selector(|| "run-open-folder".to_string()).child(Button::new("run-open-folder", "Open folder").style(ButtonStyle::Subtle).size(ButtonSize::Compact).on_click(cx.listener(|this, _, w, cx| this.open_runs_folder(w, cx))))),
            )
            .when_some(run.error.clone(), |d, e| d.child(div().id("run-error").debug_selector(|| "run-error".to_string()).text_color(p.err).child(e)))
            .child(
                div()
                    .id("run-files")
                    .debug_selector(|| "run-files".to_string())
                    .font_family(MONO_FONT)
                    .text_size(px(11.))
                    .text_color(p.muted)
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .child(format!("Run files: {}", run.dir.display()))
                    .children(run.notes.iter().map(|n| div().child(format!("\u{b7} {n}")))),
            )
            // Numbers.
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(px(8.))
                    .child(kpi("run-kpi-samples", "SAMPLES", thousands(t.count), p.fg))
                    .child(kpi("run-kpi-rate", "PER SECOND", rate(run.live.throughput()), p.fg))
                    .child(kpi("run-kpi-avg", "AVERAGE MS", thousands(t.avg_ms()), p.fg))
                    .child(kpi("run-kpi-p95", "95TH MS", thousands(t.percentile(95.0)), p.fg))
                    .child(kpi("run-kpi-errors", "ERRORS", format!("{} ({:.1}%)", thousands(t.errors), t.error_pct()), err_color))
                    .child(kpi("run-kpi-users", "USERS", thousands(run.live.threads() as u64), p.fg)),
            )
            // Charts.
            .child(
                div()
                    .flex()
                    .gap(px(14.))
                    .child(chart("run-chart-response", "RESPONSE MS", format!("{}", t.avg_ms()), avg_series, p.accent))
                    .child(chart("run-chart-rate", "PER SECOND", rate(run.live.throughput()), rate_series, p.ok))
                    .child(chart("run-chart-errors", "ERRORS / STEP", thousands(t.errors), err_series, p.err))
                    .child(chart("run-chart-users", "USERS", thousands(run.live.threads() as u64), user_series, p.warn)),
            )
            // Details.
            .child(
                div()
                    .flex()
                    .border_b_1()
                    .border_color(p.line)
                    .child(detail_tab("run-tab-errors", format!("Errors ({})", run.live.errors().count()), RunDetail::Errors, cx))
                    .child(detail_tab("run-tab-labels", format!("Requests ({})", run.live.labels().len()), RunDetail::Labels, cx))
                    .child(detail_tab("run-tab-log", format!("JMeter output ({})", run.log.len()), RunDetail::Log, cx)),
            )
            .child(div().id("run-detail").flex_1().min_h(px(60.)).overflow_y_scroll().child(detail))
            .into_any_element()
    }

    fn run_errors(&self, run: &RunSession, p: &Palette) -> AnyElement {
        let rows: Vec<_> = run.live.errors().collect();
        if rows.is_empty() {
            return div().py(px(6.)).text_color(p.muted).child(if run.phase.active() { "No failed requests so far." } else { "No failed requests." }).into_any_element();
        }
        div()
            .flex()
            .flex_col()
            .font_family(MONO_FONT)
            .text_size(px(11.5))
            .children(rows.iter().rev().take(MAX_ROWS).map(|e| {
                div()
                    .flex()
                    .gap(px(10.))
                    .py(px(1.))
                    .child(div().w(px(46.)).text_color(p.err).child(e.code.clone()))
                    .child(div().w(px(180.)).overflow_hidden().whitespace_nowrap().child(e.label.clone()))
                    .child(div().flex_1().overflow_hidden().whitespace_nowrap().text_color(p.muted).child(e.message.clone()))
                    .child(div().w(px(60.)).text_color(p.muted).child(format!("{} ms", e.elapsed_ms)))
            }))
            .into_any_element()
    }

    fn run_labels(&self, run: &RunSession, p: &Palette) -> AnyElement {
        let labels = run.live.labels();
        if labels.is_empty() {
            return div().py(px(6.)).text_color(p.muted).child("No requests yet.").into_any_element();
        }
        let cell = |w: f32, text: String, color: gpui::Hsla| div().w(px(w)).text_color(color).child(text);
        let head = div()
            .flex()
            .gap(px(8.))
            .text_color(p.muted)
            .child(div().flex_1().child("Request"))
            .child(cell(70., "Samples".into(), p.muted))
            .child(cell(60., "Avg".into(), p.muted))
            .child(cell(60., "Min".into(), p.muted))
            .child(cell(60., "Max".into(), p.muted))
            .child(cell(60., "90%".into(), p.muted))
            .child(cell(60., "95%".into(), p.muted))
            .child(cell(60., "99%".into(), p.muted))
            .child(cell(70., "Errors".into(), p.muted));
        div()
            .flex()
            .flex_col()
            .font_family(MONO_FONT)
            .text_size(px(11.5))
            .child(head)
            .children(labels.iter().take(MAX_ROWS).map(|(name, s)| {
                div()
                    .flex()
                    .gap(px(8.))
                    .py(px(1.))
                    .child(div().flex_1().overflow_hidden().whitespace_nowrap().child(name.to_string()))
                    .child(cell(70., thousands(s.count), p.fg))
                    .child(cell(60., thousands(s.avg_ms()), p.fg))
                    .child(cell(60., thousands(s.min_ms), p.fg))
                    .child(cell(60., thousands(s.max_ms), p.fg))
                    .child(cell(60., thousands(s.percentile(90.0)), p.fg))
                    .child(cell(60., thousands(s.percentile(95.0)), p.fg))
                    .child(cell(60., thousands(s.percentile(99.0)), p.fg))
                    .child(cell(70., format!("{:.1}%", s.error_pct()), if s.errors > 0 { p.err } else { p.fg }))
            }))
            .when(labels.len() > MAX_ROWS, |d| d.child(div().text_color(p.muted).child(format!("{} more requests are not listed", labels.len() - MAX_ROWS))))
            .into_any_element()
    }

    fn run_log(&self, run: &RunSession, p: &Palette) -> AnyElement {
        if run.log.is_empty() {
            return div().py(px(6.)).text_color(p.muted).child("JMeter has printed nothing yet.").into_any_element();
        }
        div().flex().flex_col().font_family(MONO_FONT).text_size(px(11.)).text_color(p.muted).children(run.log.iter().map(|l| div().child(l.clone()))).into_any_element()
    }

    /// The History tab of the lower panel: the earlier runs of the open plan.
    pub(crate) fn history_view(&self, cx: &mut Context<Self>) -> AnyElement {
        let p = theme::palette(cx);
        let Some(root) = self.runs_root_for_active() else {
            return div().p(px(16.)).text_color(p.muted).child("Open a plan to see its runs.").into_any_element();
        };
        let current = self.run.as_ref().map(|r| r.dir.clone());
        let keep = self.settings.runs_keep;
        let rows = self.history_entries().iter().enumerate().map(|(i, e)| {
            let m = &e.meta;
            let dir = e.dir.clone();
            let (dir_view, dir_open, dir_del) = (dir.clone(), dir.clone(), dir.clone());
            let live_now = current.as_ref() == Some(&e.dir) && self.run_in_progress();
            let (label, color) = match m.outcome {
                Outcome::Running if live_now => ("Running", p.accent),
                Outcome::Running => ("Interrupted", p.warn),
                Outcome::Finished => ("Finished", p.ok),
                Outcome::Stopped => ("Stopped", p.warn),
                Outcome::Failed => ("Failed", p.err),
            };
            let err_pct = if m.samples == 0 { 0.0 } else { m.errors as f64 * 100.0 / m.samples as f64 };
            let shown = current.as_ref() == Some(&e.dir);
            div()
                .id(("history-row", i))
                .debug_selector(move || format!("history-row-{i}"))
                .flex()
                .items_center()
                .gap(px(10.))
                .px(px(12.))
                .py(px(4.))
                .text_size(px(12.))
                .cursor_pointer()
                .when(shown, |d| d.bg(with_alpha(p.accent, 0.12)))
                .hover(|d| d.bg(p.panel2))
                .on_click(cx.listener(move |this, _, w, cx| this.view_run(&dir_view, w, cx)))
                .child(div().w(px(140.)).font_family(MONO_FONT).text_color(p.muted).child(m.started.clone()))
                .child(div().w(px(70.)).child(m.kind.label()))
                .child(div().w(px(80.)).text_color(color).child(label))
                .child(div().w(px(60.)).text_color(p.muted).child(m.duration_secs().map(|s| clock(s as u64)).unwrap_or_else(|| "-".into())))
                .child(div().w(px(90.)).child(format!("{} samples", thousands(m.samples))))
                .child(div().w(px(70.)).text_color(if m.errors > 0 { p.err } else { p.fg }).child(format!("{err_pct:.1}% err")))
                .child(div().w(px(70.)).child(format!("avg {}", thousands(m.avg_ms))))
                .child(div().w(px(70.)).child(format!("p95 {}", thousands(m.p95_ms))))
                .child(div().flex_1())
                .child(
                    div()
                        .debug_selector(move || format!("history-folder-{i}"))
                        .child(Button::new(("history-folder", i), "Folder").style(ButtonStyle::Subtle).size(ButtonSize::Compact).on_click(cx.listener(move |_, _, _, cx| {
                            crate::run_ctl::open_in_file_manager(&dir_open);
                            cx.stop_propagation();
                        }))),
                )
                .child(
                    div()
                        .debug_selector(move || format!("history-delete-{i}"))
                        .child(Button::new(("history-delete", i), "Remove").style(ButtonStyle::Subtle).size(ButtonSize::Compact).disabled(live_now).on_click(cx.listener(move |this, _, _, cx| {
                            this.request_delete_run(&dir_del, cx);
                            cx.stop_propagation();
                        }))),
                )
        });
        div()
            .id("history-view")
            .size_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .px(px(12.))
                    .py(px(6.))
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .border_b_1()
                    .border_color(p.line)
                    .child(div().id("history-root").debug_selector(|| "history-root".to_string()).flex_1().overflow_hidden().whitespace_nowrap().font_family(MONO_FONT).text_size(px(11.)).text_color(p.muted).child(format!("{}  \u{b7}  the last {keep} runs of this plan are kept", root.display())))
                    .child(div().debug_selector(|| "history-open-folder".to_string()).child(Button::new("history-open-folder", "Open folder").style(ButtonStyle::Subtle).size(ButtonSize::Compact).on_click(cx.listener(|this, _, w, cx| this.open_runs_folder(w, cx))))),
            )
            .child(
                div()
                    .id("history-list")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .when(self.history_entries().is_empty(), |d| d.child(div().p(px(16.)).text_color(p.muted).child("No runs of this plan yet. Press F5 to run it; every run is kept here with its results and log.")))
                    .children(rows),
            )
            .into_any_element()
    }

    /// Whether the lower panel is showing the run tab (for tests).
    pub fn showing_run_tab(&self) -> bool {
        self.bottom_tab == BottomTab::Run
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_clock_shows_minutes_and_seconds_and_hours_when_needed() {
        assert_eq!((clock(0), clock(5), clock(65), clock(3599), clock(3600), clock(3723)), ("0:00".into(), "0:05".into(), "1:05".into(), "59:59".into(), "1:00:00".into(), "1:02:03".into()));
    }

    #[test]
    fn counts_get_thousands_separators() {
        assert_eq!((thousands(0), thousands(999), thousands(1000), thousands(1234567), thousands(100000)), ("0".into(), "999".into(), "1,000".into(), "1,234,567".into(), "100,000".into()));
    }

    #[test]
    fn rates_are_short() {
        assert_eq!((rate(0.0), rate(0.44), rate(12.55), rate(99.94), rate(1234.4), rate(f64::INFINITY)), ("0.0".into(), "0.4".into(), "12.6".into(), "99.9".into(), "1,234".into(), "-".into()));
    }
}
