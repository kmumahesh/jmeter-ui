//! The Aggregate Report: the per-request table of a run as a CSV for a spreadsheet, and choosing one request for the charts.

use crate::shell::{Severity, Shell};
use gpui::{ClipboardItem, Context, Window};
use jvm_runtime::live::{LiveStats, Stats};

/// A CSV field: quoted when it has a comma, quote or line break, and a leading `=`, `+`, `-`, `@`, tab or carriage return
/// is defused with a quote mark so that a spreadsheet never takes a request name for a formula.
pub fn csv_field(text: &str) -> String {
    let text: String = text.chars().map(|c| if c == '\n' || c == '\r' { ' ' } else { c }).collect();
    let text = if text.starts_with(['=', '+', '-', '@', '\t']) { format!("'{text}") } else { text };
    if text.contains([',', '"']) { format!("\"{}\"", text.replace('"', "\"\"")) } else { text }
}

fn row(name: &str, s: &Stats) -> String {
    format!(
        "{},{},{},{},{},{},{},{},{},{:.2},{:.2}%,{:.2},{:.2}",
        csv_field(name),
        s.count,
        s.avg_ms(),
        s.median_ms(),
        s.percentile(90.0),
        s.percentile(95.0),
        s.percentile(99.0),
        s.min_ms,
        s.max_ms,
        s.std_dev_ms(),
        s.error_pct(),
        s.throughput(),
        s.received_kb_per_sec()
    )
}

/// The table of a run as CSV, with the column names of JMeter's Aggregate Report and a TOTAL row at the end.
pub fn csv(live: &LiveStats) -> String {
    let mut out = String::from("Label,# Samples,Average,Median,90% Line,95% Line,99% Line,Min,Max,Std. Dev.,Error %,Throughput,Received KB/sec\n");
    for (name, s) in live.labels() {
        out.push_str(&row(name, s));
        out.push('\n');
    }
    out.push_str(&row("TOTAL", live.total()));
    out.push('\n');
    out
}

impl Shell {
    /// Choose the request the charts show, or all of them with `None`.
    pub fn chart_label(&mut self, label: Option<String>, cx: &mut Context<Self>) {
        self.run_label = label;
        cx.notify();
    }

    /// Put the table on the clipboard as CSV.
    pub fn copy_aggregate_csv(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(run) = &self.run else { return };
        let text = csv(&run.live);
        let rows = run.live.labels().len();
        cx.write_to_clipboard(ClipboardItem::new_string(text));
        self.notify(format!("Copied {rows} request{} as CSV", if rows == 1 { "" } else { "s" }), Severity::Info, window, cx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jvm_runtime::results::Sample;

    fn sample(label: &str, ts: u64, ms: u64, ok: bool) -> Sample {
        Sample { timestamp_ms: ts, elapsed_ms: ms, label: label.into(), response_code: "200".into(), response_message: "OK".into(), thread_name: "t".into(), success: ok, bytes: 2048, group_threads: 1, all_threads: 1, latency_ms: 1, connect_ms: 1 }
    }

    #[test]
    fn fields_with_commas_quotes_and_lines_are_quoted() {
        assert_eq!(csv_field("plain"), "plain");
        assert_eq!(csv_field("a,b"), "\"a,b\"");
        assert_eq!(csv_field("say \"hi\""), "\"say \"\"hi\"\"\"");
        assert_eq!(csv_field("two\nlines\r\nhere"), "two lines  here");
    }

    #[test]
    fn a_name_that_a_spreadsheet_would_run_as_a_formula_is_defused() {
        for evil in ["=cmd|' /C calc'!A0", "+1+1", "-2+3", "@SUM(1,1)", "\t=1"] {
            let f = csv_field(evil);
            assert!(f.starts_with('\'') || f.starts_with("\"'"), "{evil:?} became {f:?}");
        }
        assert_eq!(csv_field("GET /a-b"), "GET /a-b", "a dash inside a name is no problem");
    }

    #[test]
    fn the_table_has_jmeters_columns_a_row_for_each_request_and_a_total() {
        let mut l = LiveStats::new();
        for i in 0..10u64 {
            l.add(&sample("Home", 1_000 + i * 100, 100 + i * 10, true));
            l.add(&sample("Search, \"big\"", 1_000 + i * 100, 200, i != 3));
        }
        let text = csv(&l);
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 4, "{text}");
        assert!(lines[0].starts_with("Label,# Samples,Average,Median,90% Line,95% Line,99% Line,Min,Max,Std. Dev.,Error %,Throughput,Received KB/sec"));
        assert!(lines[1].starts_with("Home,10,145,"), "{}", lines[1]);
        assert!(lines[2].starts_with("\"Search, \"\"big\"\"\",10,200,200,"), "{}", lines[2]);
        assert!(lines[2].contains(",10.00%,"), "{}", lines[2]);
        assert!(lines[3].starts_with("TOTAL,20,"), "{}", lines[3]);
        // Every row has the same number of columns once quoted fields are taken whole.
        assert_eq!(lines[1].matches(',').count(), 12);
        assert_eq!(lines[3].matches(',').count(), 12);
    }

    #[test]
    fn an_empty_run_is_only_the_header_and_a_zero_total() {
        let text = csv(&LiveStats::new());
        assert_eq!(text.lines().count(), 2);
        assert!(text.lines().nth(1).unwrap().starts_with("TOTAL,0,0,0,"));
    }
}
