//! Parsing what a run produces: summary lines, results-file rows and XML error samples.

use anyhow::{Context, Result, bail};
use jmx_model::{Document, Element};
use std::io::Read;
use std::path::Path;

/// One `summary +` (interval) or `summary =` (cumulative) line printed by JMeter.
#[derive(Debug, Clone, PartialEq)]
pub struct Summary {
    pub cumulative: bool,
    pub samples: u64,
    pub rate_per_sec: f64,
    pub avg_ms: u64,
    pub min_ms: u64,
    pub max_ms: u64,
    pub errors: u64,
    pub error_pct: f64,
    /// Only present on interval lines.
    pub active: Option<u32>,
    pub started: Option<u32>,
    pub finished: Option<u32>,
}

/// Parse e.g. `summary +      1 in 00:00:00 =    2.4/s Avg:   353 Min:   353 Max:   353 Err:     0 (0.00%) Active: 2 Started: 2 Finished: 0`.
pub fn parse_summary(line: &str) -> Option<Summary> {
    let mut t = line.split_whitespace();
    t.next()?; // summariser name
    let cumulative = match t.next()? {
        "+" => false,
        "=" => true,
        _ => return None,
    };
    let samples = t.next()?.parse().ok()?;
    if t.next()? != "in" {
        return None;
    }
    t.next()?; // elapsed hh:mm:ss
    if t.next()? != "=" {
        return None;
    }
    let rate: f64 = t.next()?.trim_end_matches("/s").parse().unwrap_or(f64::INFINITY); // "******" when elapsed is 0
    let mut s = Summary {
        cumulative,
        samples,
        rate_per_sec: rate,
        avg_ms: 0,
        min_ms: 0,
        max_ms: 0,
        errors: 0,
        error_pct: 0.0,
        active: None,
        started: None,
        finished: None,
    };
    while let Some(key) = t.next() {
        match key {
            "Avg:" => s.avg_ms = t.next()?.parse().ok()?,
            "Min:" => s.min_ms = t.next()?.parse().ok()?,
            "Max:" => s.max_ms = t.next()?.parse().ok()?,
            "Err:" => {
                s.errors = t.next()?.parse().ok()?;
                s.error_pct = t.next()?.trim_matches(['(', ')', '%']).parse().ok()?;
            }
            "Active:" => s.active = t.next()?.parse().ok(),
            "Started:" => s.started = t.next()?.parse().ok(),
            "Finished:" => s.finished = t.next()?.parse().ok(),
            _ => {}
        }
    }
    Some(s)
}

/// The line JMeter prints with the stop-signal port it actually bound.
pub fn parse_stop_port(line: &str) -> Option<u16> {
    line.contains("Waiting for possible Shutdown").then(|| line.rsplit(' ').next()?.trim().parse().ok()).flatten()
}

/// One row of the results file (JTL, CSV).
#[derive(Debug, Clone)]
pub struct Sample {
    pub timestamp_ms: u64,
    pub elapsed_ms: u64,
    pub label: String,
    pub response_code: String,
    pub response_message: String,
    pub thread_name: String,
    pub success: bool,
    pub bytes: u64,
    pub group_threads: u32,
    pub all_threads: u32,
    pub latency_ms: u64,
    pub connect_ms: u64,
}

/// Reads new rows from a results file that is still being written.
pub struct JtlTail {
    offset: u64,
    pending: String,
    header: Option<Vec<String>>,
}

impl JtlTail {
    pub fn new() -> Self {
        JtlTail { offset: 0, pending: String::new(), header: None }
    }

    /// Read anything new since the last call and return the complete rows.
    pub fn poll(&mut self, path: &Path) -> Result<Vec<Sample>> {
        let Ok(mut f) = std::fs::File::open(path) else { return Ok(vec![]) };
        use std::io::Seek;
        f.seek(std::io::SeekFrom::Start(self.offset))?;
        let mut buf = Vec::new();
        f.read_to_end(&mut buf)?;
        self.offset += buf.len() as u64;
        self.pending.push_str(&String::from_utf8_lossy(&buf));
        // Keep only complete records. A record ends at a newline outside quotes.
        let mut records = Vec::new();
        let mut start = 0;
        let mut in_quotes = false;
        for (i, c) in self.pending.char_indices() {
            match c {
                '"' => in_quotes = !in_quotes,
                '\n' if !in_quotes => {
                    records.push(self.pending[start..i].trim_end_matches('\r').to_string());
                    start = i + 1;
                }
                _ => {}
            }
        }
        self.pending.drain(..start);
        let mut out = Vec::new();
        for rec in records {
            if rec.is_empty() {
                continue;
            }
            let mut rdr = csv::ReaderBuilder::new().has_headers(false).flexible(true).from_reader(rec.as_bytes());
            let Some(row) = rdr.records().next() else { continue };
            let row = row?;
            match &self.header {
                None => self.header = Some(row.iter().map(str::to_string).collect()),
                Some(h) => out.push(sample_from_row(h, &row)?),
            }
        }
        Ok(out)
    }
}

impl Default for JtlTail {
    fn default() -> Self {
        Self::new()
    }
}

fn sample_from_row(header: &[String], row: &csv::StringRecord) -> Result<Sample> {
    let get = |name: &str| header.iter().position(|h| h == name).and_then(|i| row.get(i)).unwrap_or("");
    let num = |name: &str| get(name).parse::<u64>().unwrap_or(0);
    Ok(Sample {
        timestamp_ms: num("timeStamp"),
        elapsed_ms: num("elapsed"),
        label: get("label").to_string(),
        response_code: get("responseCode").to_string(),
        response_message: get("responseMessage").to_string(),
        thread_name: get("threadName").to_string(),
        success: get("success") == "true",
        bytes: num("bytes"),
        group_threads: num("grpThreads") as u32,
        all_threads: num("allThreads") as u32,
        latency_ms: num("Latency"),
        connect_ms: num("Connect"),
    })
}

/// A failed sample from the injected XML collector, with request and response detail.
#[derive(Debug, Clone, Default)]
pub struct ErrorSample {
    pub label: String,
    pub response_code: String,
    pub response_message: String,
    pub thread_name: String,
    pub elapsed_ms: u64,
    pub url: String,
    pub request_headers: String,
    pub response_headers: String,
    pub response_body: String,
    pub assertion_failures: Vec<String>,
}

/// Read the XML file written by the injected collector once the run has finished.
pub fn read_error_samples(path: &Path) -> Result<Vec<ErrorSample>> {
    let src = std::fs::read_to_string(path).with_context(|| format!("cannot read {}", path.display()))?;
    let doc = Document::parse(&src)?;
    if doc.root.name != "testResults" {
        bail!("root element is <{}>, expected <testResults>", doc.root.name);
    }
    let mut out = Vec::new();
    for s in doc.root.elements().filter(|e| e.name == "httpSample" || e.name == "sample") {
        out.push(error_sample(s));
    }
    Ok(out)
}

fn child_text(e: &Element, name: &str) -> String {
    e.elements().find(|c| c.name == name).map(Element::text).unwrap_or_default()
}

fn error_sample(s: &Element) -> ErrorSample {
    ErrorSample {
        label: s.attr("lb").unwrap_or_default(),
        response_code: s.attr("rc").unwrap_or_default(),
        response_message: s.attr("rm").unwrap_or_default(),
        thread_name: s.attr("tn").unwrap_or_default(),
        elapsed_ms: s.attr("t").and_then(|v| v.parse().ok()).unwrap_or(0),
        url: child_text(s, "java.net.URL"),
        request_headers: child_text(s, "requestHeader"),
        response_headers: child_text(s, "responseHeader"),
        response_body: child_text(s, "responseData"),
        assertion_failures: s
            .elements()
            .filter(|c| c.name == "assertionResult" && child_text(c, "failure") == "true")
            .map(|c| format!("{}: {}", child_text(c, "name"), child_text(c, "failureMessage")))
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summary_interval_line() {
        let s = parse_summary("summary +     29 in 00:00:02 =   12.1/s Avg:   216 Min:   103 Max:   339 Err:     2 (6.90%) Active: 0 Started: 3 Finished: 3").unwrap();
        assert!(!s.cumulative);
        assert_eq!((s.samples, s.avg_ms, s.min_ms, s.max_ms, s.errors), (29, 216, 103, 339, 2));
        assert_eq!(s.error_pct, 6.9);
        assert_eq!((s.active, s.started, s.finished), (Some(0), Some(3), Some(3)));
    }

    #[test]
    fn summary_cumulative_line_has_no_threads() {
        let s = parse_summary("summary =     30 in 00:00:03 =   10.7/s Avg:   221 Min:   103 Max:   353 Err:     2 (6.67%)").unwrap();
        assert!(s.cumulative);
        assert_eq!(s.active, None);
    }

    #[test]
    fn summary_rate_when_elapsed_is_zero() {
        let s = parse_summary("summary +      1 in 00:00:00 = ******/s Avg:     5 Min:     5 Max:     5 Err:     0 (0.00%) Active: 1 Started: 1 Finished: 0").unwrap();
        assert!(s.rate_per_sec.is_infinite());
    }

    #[test]
    fn stop_port_line() {
        assert_eq!(parse_stop_port("Waiting for possible Shutdown/StopTestNow/HeapDump/ThreadDump message on port 4446"), Some(4446));
        assert_eq!(parse_stop_port("something else"), None);
    }

    #[test]
    fn jtl_tail_handles_partial_lines_and_quotes() {
        let dir = std::env::temp_dir().join(format!("jtltail-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("r.jtl");
        std::fs::write(&p, "timeStamp,elapsed,label,responseCode,responseMessage,threadName,success,bytes\n1,10,\"a,b\",200,OK,t-1,true,5\n2,20,c,5").unwrap();
        let mut t = JtlTail::new();
        let first = t.poll(&p).unwrap();
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].label, "a,b");
        use std::io::Write;
        std::fs::OpenOptions::new().append(true).open(&p).unwrap().write_all(b"00,OK,t-2,false,6\n").unwrap();
        let second = t.poll(&p).unwrap();
        assert_eq!(second.len(), 1);
        assert_eq!(second[0].response_code, "500");
        assert!(!second[0].success);
    }
}
