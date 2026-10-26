//! Comparing two runs of a plan: choose a baseline in the History tab, then another run to compare with it. The numbers of
//! both runs are read in the background and shown side by side, request by request, with the worst changes first.

use crate::aggregate::csv_field;
use crate::assets::MONO_FONT;
use crate::shell::{BottomTab, Severity, Shell};
use crate::theme;
use crate::views_run::{rate, thousands};
use gpui::{AnyElement, ClipboardItem, Context, FontWeight, IntoElement, Window, div, prelude::*, px};
use jvm_runtime::compare::{Comparison, Metrics, Row, Verdict, change_pct};
use jvm_runtime::live::LiveStats;
use jvm_runtime::runs::RunEntry;
use std::path::Path;
use ui::{Button, ButtonCommon, ButtonSize, ButtonStyle, Clickable};

/// Rows drawn in the comparison (the copy has every request).
const MAX_ROWS: usize = 300;

/// A comparison of two runs, ready to show.
pub struct CompareData {
    pub base_title: String,
    pub cand_title: String,
    pub cmp: Comparison,
    /// Rows in name order instead of worst first.
    pub by_name: bool,
}

fn title(e: &RunEntry) -> String {
    format!("{} ({} samples)", e.meta.started, thousands(e.meta.samples))
}

impl Shell {
    /// "Compare" on a run in the History tab: the first one chosen is the baseline, a second one is compared with it.
    /// Choosing the baseline again takes it back.
    pub fn compare_pick(&mut self, dir: &Path, window: &mut Window, cx: &mut Context<Self>) {
        let Some(base) = self.compare_base.clone() else {
            self.compare_base = Some(dir.to_path_buf());
            cx.notify();
            return;
        };
        if base == dir {
            self.compare_base = None;
            cx.notify();
            return;
        }
        let (Some(b), Some(c)) = (self.history.iter().find(|e| e.dir == base).cloned(), self.history.iter().find(|e| e.dir == dir).cloned()) else {
            self.compare_base = None;
            self.notify("The baseline run is gone. Choose it again.", Severity::Info, window, cx);
            return;
        };
        // The earlier run is the baseline whichever was chosen first.
        let (b, c) = if b.meta.started_ms <= c.meta.started_ms { (b, c) } else { (c, b) };
        self.show_comparison(b, c, window, cx);
    }

    /// Read both runs in the background and show how the second differs from the first.
    pub fn show_comparison(&mut self, base: RunEntry, cand: RunEntry, window: &mut Window, cx: &mut Context<Self>) {
        let (bt, ct) = (title(&base), title(&cand));
        let (bf, cf) = (base.dir.join("results.jtl"), cand.dir.join("results.jtl"));
        self.compare_base = None;
        self.compare = None;
        self.compare_loading = true;
        self.bottom_tab = BottomTab::Compare;
        cx.spawn_in(window, async move |this, cx| {
            let read = cx
                .background_executor()
                .spawn(async move {
                    // A run whose results are gone would compare as "everything is new", which says nothing true.
                    if !bf.is_file() {
                        return Err("The baseline run could not be read: its results file is gone.".to_string());
                    }
                    if !cf.is_file() {
                        return Err("The other run could not be read: its results file is gone.".to_string());
                    }
                    let a = LiveStats::from_file(&bf).map(|x| x.0).map_err(|e| format!("The baseline run could not be read: {e:#}"))?;
                    let b = LiveStats::from_file(&cf).map(|x| x.0).map_err(|e| format!("The other run could not be read: {e:#}"))?;
                    Ok::<Comparison, String>(Comparison::of(&a, &b))
                })
                .await;
            this.update_in(cx, |this, window, cx| {
                this.compare_loading = false;
                match read {
                    Ok(cmp) => this.compare = Some(CompareData { base_title: bt, cand_title: ct, cmp, by_name: false }),
                    Err(e) => this.notify(e, Severity::Error, window, cx),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    pub fn compare_sort(&mut self, by_name: bool, cx: &mut Context<Self>) {
        if let Some(c) = &mut self.compare {
            c.by_name = by_name;
        }
        cx.notify();
    }

    /// Put the comparison on the clipboard as CSV.
    pub fn copy_comparison_csv(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(c) = &self.compare else { return };
        let text = comparison_csv(&c.cmp);
        let rows = c.cmp.rows.len();
        cx.write_to_clipboard(ClipboardItem::new_string(text));
        self.notify(format!("Copied {rows} request{} as CSV", if rows == 1 { "" } else { "s" }), Severity::Info, window, cx);
    }

    // ---- drawing ----

    pub(crate) fn compare_view(&self, cx: &mut Context<Self>) -> AnyElement {
        let p = theme::palette(cx);
        if self.compare_loading {
            return div().p(px(16.)).text_color(p.muted).child("Reading both runs\u{2026}").into_any_element();
        }
        let Some(c) = &self.compare else {
            return div()
                .id("compare-empty")
                .debug_selector(|| "compare-empty".to_string())
                .p(px(16.))
                .flex()
                .flex_col()
                .gap(px(4.))
                .text_color(p.muted)
                .child(div().text_color(p.fg).child("Nothing to compare yet"))
                .child("In the History tab, press Compare on the run to use as the baseline, then Compare on the run to check against it.")
                .into_any_element();
        };
        let rows: Vec<&Row> = if c.by_name { c.cmp.rows.iter().collect() } else { c.cmp.worst_first() };
        let cell = |w: f32, text: String, color: gpui::Hsla| div().w(px(w)).flex_none().text_color(color).child(text);
        let head = div()
            .flex()
            .gap(px(8.))
            .text_color(p.muted)
            .child(div().flex_1().min_w(px(160.)).child("Request"))
            .child(cell(150., "Average ms".into(), p.muted))
            .child(cell(150., "95th ms".into(), p.muted))
            .child(cell(130., "Errors".into(), p.muted))
            .child(cell(150., "Per second".into(), p.muted))
            .child(cell(80., "Result".into(), p.muted));
        let verdict_text = |v: Verdict| match v {
            Verdict::Worse => ("Worse", p.err),
            Verdict::Better => ("Better", p.ok),
            Verdict::Same => ("Same", p.muted),
            Verdict::New => ("New", p.accent),
            Verdict::Gone => ("Gone", p.warn),
        };
        let line = |idx: usize, r: &Row, bold: bool| {
            let (word, color) = verdict_text(r.verdict());
            let pair = |f: &dyn Fn(&Metrics) -> (String, f64, f64)| -> (String, Option<f64>) {
                match (&r.base, &r.cand) {
                    (Some(b), Some(c)) => {
                        let (_, bv, cv) = f(b);
                        let (text, _, _) = f(c);
                        (format!("{} \u{2192} {}", f(b).0, text), change_pct(bv, cv))
                    }
                    (None, Some(c)) => (format!("\u{2192} {}", f(c).0), None),
                    (Some(b), None) => (format!("{} \u{2192}", f(b).0), None),
                    (None, None) => ("-".into(), None),
                }
            };
            let avg = pair(&|m| (thousands(m.avg_ms), m.avg_ms as f64, m.avg_ms as f64));
            let p95 = pair(&|m| (thousands(m.p95_ms), m.p95_ms as f64, m.p95_ms as f64));
            let err = pair(&|m| (format!("{:.1}%", m.error_pct), m.error_pct, m.error_pct));
            let per = pair(&|m| (rate(m.per_sec), m.per_sec, m.per_sec));
            let with_change = |(text, ch): (String, Option<f64>)| match ch {
                Some(c) if c.abs() >= 0.05 => format!("{text}  {:+.0}%", c),
                _ => text,
            };
            div()
                .id(("cmp-row", idx))
                .debug_selector(move || format!("cmp-row-{idx}"))
                .flex()
                .gap(px(8.))
                .py(px(1.))
                .when(bold, |d| d.font_weight(FontWeight::SEMIBOLD).border_t_1().border_color(p.line))
                .child(div().flex_1().min_w(px(160.)).overflow_hidden().text_ellipsis().whitespace_nowrap().child(r.label.clone()))
                .child(cell(150., with_change(avg), p.fg))
                .child(cell(150., with_change(p95), if matches!(r.verdict(), Verdict::Worse) { p.err } else { p.fg }))
                .child(cell(130., err.0, p.fg))
                .child(cell(150., with_change(per), p.fg))
                .child(cell(80., word.to_string(), color))
        };
        let (worse, better, newer, gone) = (c.cmp.count(Verdict::Worse), c.cmp.count(Verdict::Better), c.cmp.count(Verdict::New), c.cmp.count(Verdict::Gone));
        let summary_color = if worse > 0 { p.err } else { p.ok };
        div()
            .id("compare-view")
            .debug_selector(|| "compare-view".to_string())
            .size_full()
            .flex()
            .flex_col()
            .gap(px(8.))
            .p(px(12.))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap(px(12.))
                    .child(div().text_size(px(12.)).child(format!("Baseline: {}", c.base_title)))
                    .child(div().text_size(px(12.)).child(format!("Compared with: {}", c.cand_title)))
                    .child(div().flex_1())
                    .child(div().debug_selector(|| "compare-sort".to_string()).child(Button::new("compare-sort", if c.by_name { "Worst first" } else { "Name order" }).style(ButtonStyle::Subtle).size(ButtonSize::Compact).on_click(cx.listener({
                        let by_name = c.by_name;
                        move |this, _, _, cx| this.compare_sort(!by_name, cx)
                    }))))
                    .child(div().debug_selector(|| "compare-copy".to_string()).child(Button::new("compare-copy", "Copy as CSV").style(ButtonStyle::Subtle).size(ButtonSize::Compact).on_click(cx.listener(|this, _, w, cx| this.copy_comparison_csv(w, cx))))),
            )
            .child(
                div()
                    .debug_selector(|| "compare-summary".to_string())
                    .text_size(px(12.5))
                    .text_color(summary_color)
                    .child(format!("{worse} worse, {better} better, {newer} new, {gone} gone, out of {} requests. A change counts when the 95th percentile moves by {}% and {} ms, or the error rate by {} points.", c.cmp.rows.len(), (jvm_runtime::compare::SHARE * 100.0) as u32, jvm_runtime::compare::MIN_MS, jvm_runtime::compare::ERROR_POINTS)),
            )
            .child(
                div().id("compare-table").flex_1().min_h_0().overflow_scroll().child(
                    div()
                        .flex()
                        .flex_col()
                        .min_w(px(900.))
                        .font_family(MONO_FONT)
                        .text_size(px(11.5))
                        .child(head)
                        .children(rows.iter().take(MAX_ROWS).enumerate().map(|(i, r)| line(i, r, false)))
                        .child(line(usize::MAX, &c.cmp.total, true))
                        .when(rows.len() > MAX_ROWS, |d| d.child(div().text_color(p.muted).child(format!("{} more requests are not listed; the copy has them all", rows.len() - MAX_ROWS)))),
                ),
            )
            .into_any_element()
    }
}

/// The comparison as CSV, with the baseline and the other run side by side and the change for each number.
pub fn comparison_csv(c: &Comparison) -> String {
    let mut out = String::from("Label,Baseline samples,Samples,Baseline avg,Avg,Avg change %,Baseline 95%,95%,95% change %,Baseline error %,Error %,Baseline per sec,Per sec,Per sec change %,Result\n");
    let pct = |b: f64, c: f64| change_pct(b, c).map(|v| format!("{v:.1}")).unwrap_or_default();
    let mut line = |r: &Row, name: &str| {
        let (b, k) = (r.base.as_ref(), r.cand.as_ref());
        let n = |m: Option<&Metrics>, f: &dyn Fn(&Metrics) -> String| m.map(f).unwrap_or_default();
        let result = match r.verdict() {
            Verdict::Worse => "Worse",
            Verdict::Better => "Better",
            Verdict::Same => "Same",
            Verdict::New => "New",
            Verdict::Gone => "Gone",
        };
        out.push_str(&format!(
            "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}\n",
            csv_field(name),
            n(b, &|m| m.count.to_string()),
            n(k, &|m| m.count.to_string()),
            n(b, &|m| m.avg_ms.to_string()),
            n(k, &|m| m.avg_ms.to_string()),
            match (b, k) { (Some(b), Some(k)) => pct(b.avg_ms as f64, k.avg_ms as f64), _ => String::new() },
            n(b, &|m| m.p95_ms.to_string()),
            n(k, &|m| m.p95_ms.to_string()),
            match (b, k) { (Some(b), Some(k)) => pct(b.p95_ms as f64, k.p95_ms as f64), _ => String::new() },
            n(b, &|m| format!("{:.2}", m.error_pct)),
            n(k, &|m| format!("{:.2}", m.error_pct)),
            n(b, &|m| format!("{:.2}", m.per_sec)),
            n(k, &|m| format!("{:.2}", m.per_sec)),
            match (b, k) { (Some(b), Some(k)) => pct(b.per_sec, k.per_sec), _ => String::new() },
            result
        ));
    };
    for r in c.worst_first() {
        line(r, &r.label);
    }
    line(&c.total, "TOTAL");
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use jvm_runtime::results::Sample;

    fn run(rows: &[(&str, u64, usize)]) -> LiveStats {
        let mut l = LiveStats::new();
        let mut t = 1_000;
        for (label, ms, n) in rows {
            for _ in 0..*n {
                l.add(&Sample { timestamp_ms: t, elapsed_ms: *ms, label: (*label).into(), response_code: "200".into(), response_message: "OK".into(), thread_name: "t".into(), success: true, bytes: 10, group_threads: 1, all_threads: 1, latency_ms: 1, connect_ms: 1 });
                t += 10;
            }
        }
        l
    }

    #[test]
    fn the_csv_lists_the_worst_first_with_changes_and_a_total() {
        let c = Comparison::of(&run(&[("A", 100, 20), ("B", 100, 20), ("Old", 5, 5)]), &run(&[("A", 100, 20), ("B", 300, 20), ("=New", 5, 5)]));
        let text = comparison_csv(&c);
        let lines: Vec<&str> = text.lines().collect();
        assert!(lines[0].starts_with("Label,Baseline samples,Samples,"));
        assert!(lines[1].starts_with("B,20,20,100,300,200.0,"), "{}", lines[1]);
        assert!(lines[1].ends_with(",Worse"));
        assert!(text.contains("'=New,,5,"), "a name that starts with = is defused and has no baseline numbers: {text}");
        assert!(lines.last().unwrap().starts_with("TOTAL,"));
        assert_eq!(lines.len(), 6, "a header, four requests and the total: {text}");
    }
}
