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

/// The most bytes read from the results file in one call; a bigger backlog takes several calls.
pub const MAX_READ_PER_POLL: usize = 8 * 1024 * 1024;
/// A record longer than this is dropped (a damaged or hostile file).
pub const MAX_RECORD_BYTES: usize = 1024 * 1024;

/// Positions of the columns that are read, found once from the header row.
#[derive(Clone, Debug, Default)]
struct Columns {
    time_stamp: Option<usize>,
    elapsed: Option<usize>,
    label: Option<usize>,
    response_code: Option<usize>,
    response_message: Option<usize>,
    thread_name: Option<usize>,
    success: Option<usize>,
    bytes: Option<usize>,
    group_threads: Option<usize>,
    all_threads: Option<usize>,
    latency: Option<usize>,
    connect: Option<usize>,
}

impl Columns {
    /// The columns of a results file, or `None` when the first line is not the header of one (it must at
    /// least name `label` and `elapsed`).
    fn from_header(h: &[String]) -> Option<Columns> {
        let at = |name: &str| h.iter().position(|x| x == name);
        at("label")?;
        at("elapsed")?;
        Some(Columns {
            time_stamp: at("timeStamp"),
            elapsed: at("elapsed"),
            label: at("label"),
            response_code: at("responseCode"),
            response_message: at("responseMessage"),
            thread_name: at("threadName"),
            success: at("success"),
            bytes: at("bytes"),
            group_threads: at("grpThreads"),
            all_threads: at("allThreads"),
            latency: at("Latency"),
            connect: at("Connect"),
        })
    }

    /// The sample in a row, or `None` for a row that is not one: the file has a time stamp column and this
    /// row's value is not a number (a damaged line, or text that is not a results file at all).
    fn sample(&self, row: &[String]) -> Option<Sample> {
        let get = |i: Option<usize>| i.and_then(|i| row.get(i)).map(String::as_str).unwrap_or("");
        if self.time_stamp.is_some() && get(self.time_stamp).trim().parse::<u64>().is_err() {
            return None;
        }
        let num = |i: Option<usize>| get(i).trim().parse::<u64>().unwrap_or(0);
        Some(Sample {
            timestamp_ms: num(self.time_stamp),
            elapsed_ms: num(self.elapsed),
            label: get(self.label).to_string(),
            response_code: get(self.response_code).to_string(),
            response_message: get(self.response_message).to_string(),
            thread_name: get(self.thread_name).to_string(),
            success: get(self.success) == "true",
            bytes: num(self.bytes),
            group_threads: num(self.group_threads).min(u32::MAX as u64) as u32,
            all_threads: num(self.all_threads).min(u32::MAX as u64) as u32,
            latency_ms: num(self.latency),
            connect_ms: num(self.connect),
        })
    }
}

/// Reads new rows from a results file that is still being written.
///
/// Memory stays bounded whatever the file holds: at most [`MAX_READ_PER_POLL`] bytes are read per call and a
/// record longer than [`MAX_RECORD_BYTES`] is dropped. Rows that cannot be read are skipped and counted.
pub struct JtlTail {
    offset: u64,
    pending: Vec<u8>,
    columns: Option<Columns>,
    /// The first line has been read (whether or not it was a usable header).
    header_read: bool,
    skipped: u64,
    more: bool,
}

impl JtlTail {
    pub fn new() -> Self {
        JtlTail { offset: 0, pending: Vec::new(), columns: None, header_read: false, skipped: 0, more: false }
    }

    /// How many records were dropped because they were damaged or too long.
    pub fn skipped(&self) -> u64 {
        self.skipped
    }

    /// Whether the last call stopped at its read limit, so that calling again will find more at once.
    pub fn has_more(&self) -> bool {
        self.more
    }

    /// Read anything new since the last call and return the complete rows.
    pub fn poll(&mut self, path: &Path) -> Result<Vec<Sample>> {
        self.more = false;
        let Ok(mut f) = std::fs::File::open(path) else { return Ok(vec![]) };
        use std::io::Seek;
        let len = f.metadata().map(|m| m.len()).unwrap_or(0);
        if len < self.offset {
            // The file was replaced by a shorter one: start again.
            *self = JtlTail { skipped: self.skipped, ..JtlTail::new() };
        }
        f.seek(std::io::SeekFrom::Start(self.offset))?;
        let mut buf = Vec::new();
        f.take(MAX_READ_PER_POLL as u64).read_to_end(&mut buf)?;
        self.offset += buf.len() as u64;
        self.more = buf.len() == MAX_READ_PER_POLL;
        self.pending.extend_from_slice(&buf);

        // Keep only complete records. A record ends at a newline outside quotes.
        let mut out = Vec::new();
        let mut start = 0;
        let mut in_quotes = false;
        for i in 0..self.pending.len() {
            match self.pending[i] {
                b'"' => in_quotes = !in_quotes,
                b'\n' if !in_quotes => {
                    let rec = &self.pending[start..i];
                    start = i + 1;
                    let rec = rec.strip_suffix(b"\r").unwrap_or(rec);
                    if rec.is_empty() {
                        continue;
                    }
                    if rec.len() > MAX_RECORD_BYTES {
                        self.skipped += 1;
                        continue;
                    }
                    let row = split_record(rec);
                    if !self.header_read {
                        self.header_read = true;
                        self.columns = Columns::from_header(&row);
                        continue;
                    }
                    match &self.columns {
                        Some(c) => match c.sample(&row) {
                            Some(sample) => out.push(sample),
                            None => self.skipped += 1,
                        },
                        // Not a results file: its lines are not samples.
                        None => self.skipped += 1,
                    }
                }
                _ => {}
            }
        }
        self.pending.drain(..start);
        if self.pending.len() > MAX_RECORD_BYTES {
            // A record that never ends (an open quote, or no line break): drop it and look for the next line.
            self.pending.clear();
            self.skipped += 1;
        }
        Ok(out)
    }
}

impl Default for JtlTail {
    fn default() -> Self {
        Self::new()
    }
}

/// Split one CSV record into fields: `,` separates, `"` quotes, `""` inside quotes is one quote.
fn split_record(rec: &[u8]) -> Vec<String> {
    let mut fields = Vec::with_capacity(20);
    let mut cur: Vec<u8> = Vec::new();
    let mut quoted = false;
    let mut i = 0;
    while i < rec.len() {
        let c = rec[i];
        if quoted {
            if c == b'"' {
                if rec.get(i + 1) == Some(&b'"') {
                    cur.push(b'"');
                    i += 1;
                } else {
                    quoted = false;
                }
            } else {
                cur.push(c);
            }
        } else {
            match c {
                b'"' => quoted = true,
                b',' => fields.push(String::from_utf8_lossy(&std::mem::take(&mut cur)).into_owned()),
                _ => cur.push(c),
            }
        }
        i += 1;
    }
    fields.push(String::from_utf8_lossy(&cur).into_owned());
    fields
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
