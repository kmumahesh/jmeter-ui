//! What was sent and what came back, for the requests of a run: kept by a small listener that runs inside JMeter,
//! and read back here.
//!
//! The listener (`SCRIPT`) is added to the copy of the plan that JMeter is given. For each request it keeps, it writes
//! one line of JSON to a file in the run folder, until a limit is reached, so the file stays small however long the
//! run is. Long texts are cut. Whatever the listener meets, it never makes a request fail.
//!
//! The file is read like any file from outside: its size, its lines, and every text in it are limited, and a line that
//! cannot be read is skipped and counted.

use anyhow::Result;
use serde::Deserialize;
use std::io::Read;
use std::path::Path;

/// Which requests keep their details.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
pub enum Mode {
    Off,
    /// Failed requests only.
    #[default]
    Failed,
    /// Every request, up to the limit.
    All,
}

impl Mode {
    pub fn word(self) -> &'static str {
        match self {
            Mode::Off => "off",
            Mode::Failed => "failed",
            Mode::All => "all",
        }
    }
}

/// How many requests keep their details in one run.
pub const MAX_KEPT: u32 = 500;

/// The longest text kept for a body or a header block, in characters; longer texts are cut.
pub const MAX_TEXT_CHARS: usize = 64 * 1024;

/// The property names the listener reads (set with `-J`).
pub const PROP_MODE: &str = "jmeterui.details";
pub const PROP_FILE: &str = "jmeterui.details.file";
pub const PROP_MAX: &str = "jmeterui.details.max";

/// The properties to give JMeter so the listener keeps details in `file`.
pub fn props(mode: Mode, file: &Path, max: u32) -> Vec<(String, String)> {
    vec![(PROP_MODE.into(), mode.word().into()), (PROP_FILE.into(), file.to_string_lossy().into_owned()), (PROP_MAX.into(), max.min(MAX_KEPT).to_string())]
}

/// The listener's script (Groovy, which JMeter brings with it).
pub const SCRIPT: &str = r#"// Added by JMeter UI to the copy of the plan that runs. It keeps what was sent and received for some requests, so that
// they can be looked at afterwards. It never changes a request and never lets a problem of its own fail one.
try {
    def mode = props.get("jmeterui.details")
    if (mode == null || mode == "off") return
    if (mode == "failed" && prev.isSuccessful()) return
    def file = props.get("jmeterui.details.file")
    if (file == null || file.isEmpty()) return
    int max = Integer.parseInt(props.get("jmeterui.details.max") ?: "500")
    final int limit = 65536
    boolean cut = false
    def text = { String s ->
        if (s == null) return ""
        if (s.length() > limit) { cut = true; return s.substring(0, limit) }
        return s
    }
    String body = ""
    byte[] data = prev.getResponseData()
    // JMeter's own guess at the data type misses some binary types, so the first bytes are looked at too.
    boolean binary = "bin".equals(prev.getDataType())
    if (!binary && data != null) {
        int look = Math.min(data.length, 1024)
        int odd = 0
        for (int i = 0; i < look; i++) {
            int b = data[i] & 0xff
            if (b == 0) { binary = true; break }
            if (b < 9 || (b > 13 && b < 32)) odd++
        }
        if (odd * 10 > look) binary = true
    }
    if (!binary && data != null && data.length > 0) {
        int n = Math.min(data.length, limit * 2)
        if (n < data.length) cut = true
        body = text(new String(data, 0, n, prev.getDataEncodingWithDefault()))
    }
    def assertions = []
    prev.getAssertionResults().each { a ->
        assertions << [name: a.getName(), failure: a.isFailure(), error: a.isError(), message: text(a.getFailureMessage())]
    }
    def subs = []
    prev.getSubResults().take(50).each { s ->
        subs << [label: s.getSampleLabel(), code: s.getResponseCode(), ok: s.isSuccessful(), ms: s.getTime()]
    }
    def entry = [
        ts: prev.getTimeStamp(), elapsed: prev.getTime(), latency: prev.getLatency(), connect: prev.getConnectTime(),
        label: prev.getSampleLabel(), code: prev.getResponseCode(), message: prev.getResponseMessage(), thread: prev.getThreadName(),
        success: prev.isSuccessful(), bytes: prev.getBytesAsLong(), sent: prev.getSentBytes(), url: prev.getUrlAsString(),
        request: text(prev.getSamplerData()), request_headers: text(prev.getRequestHeaders()), response_headers: text(prev.getResponseHeaders()),
        response: body, binary: binary, truncated: cut, assertions: assertions, sub_results: subs
    ]
    String line = groovy.json.JsonOutput.toJson(entry)
    synchronized (props) {
        int n = Integer.parseInt(props.get("jmeterui.details.count") ?: "0")
        if (n >= max) return
        props.put("jmeterui.details.count", String.valueOf(n + 1))
        new File(file).withWriterAppend("UTF-8") { w -> w.write(line); w.write("\n") }
    }
} catch (Throwable t) {
    // Nothing here may fail a request.
}
"#;

/// One assertion that was checked on a request.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct AssertionDetail {
    pub name: String,
    pub failure: bool,
    pub error: bool,
    pub message: String,
}

/// A request made as part of another (a redirect, an embedded resource).
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct SubDetail {
    pub label: String,
    pub code: String,
    pub ok: bool,
    pub ms: u64,
}

/// What was kept for one request.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Detail {
    /// When the request started, in milliseconds since 1970 (the same stamp as in the results file).
    pub ts: i64,
    pub elapsed: u64,
    pub latency: u64,
    pub connect: u64,
    pub label: String,
    pub code: String,
    pub message: String,
    pub thread: String,
    pub success: bool,
    pub bytes: u64,
    pub sent: u64,
    pub url: String,
    /// JMeter's description of what was sent: the request line, the body, and the cookies.
    pub request: String,
    pub request_headers: String,
    pub response_headers: String,
    pub response: String,
    /// The response was not text, so it is not kept.
    pub binary: bool,
    /// A text was cut at the limit.
    pub truncated: bool,
    pub assertions: Vec<AssertionDetail>,
    pub sub_results: Vec<SubDetail>,
}

impl Detail {
    /// The first line of what was sent: `POST http://host/path`.
    pub fn request_line(&self) -> &str {
        self.request.lines().next().unwrap_or("").trim()
    }

    /// The body that was sent, if there was one. JMeter writes it after a line such as `POST data:`.
    pub fn request_body(&self) -> Option<&str> {
        let at = self.request.find(" data:\n")?;
        let rest = &self.request[at + " data:\n".len()..];
        let end = ["\n\nCookie Data:", "\n\n[no cookies]", "\n\nCookie data:"].iter().filter_map(|m| rest.find(m)).min().unwrap_or(rest.len());
        let body = &rest[..end];
        // A request with no body has the marker and nothing after it.
        (!body.trim().is_empty()).then_some(body)
    }

    /// What was sent, as text to read: the request line, the headers, a blank line and the body.
    pub fn request_text(&self) -> String {
        let mut out = String::new();
        let line = self.request_line();
        out.push_str(if line.is_empty() { &self.url } else { line });
        out.push('\n');
        let headers = self.request_headers.trim_end();
        if !headers.is_empty() {
            out.push_str(headers);
            out.push('\n');
        }
        if let Some(body) = self.request_body() {
            out.push('\n');
            out.push_str(body);
        }
        out
    }

    /// What came back, as text to read: the status line and headers, a blank line and the body.
    pub fn response_text(&self) -> String {
        let mut out = String::new();
        let headers = self.response_headers.trim_end();
        if headers.is_empty() {
            out.push_str(&format!("{} {}", self.code, self.message));
        } else {
            out.push_str(headers);
        }
        out.push('\n');
        if self.binary {
            out.push_str("\n(The response is not text, so it is not shown.)");
        } else if !self.response.is_empty() {
            out.push('\n');
            out.push_str(&self.response);
        }
        out
    }
}

/// The most that is read from the file in one call.
pub const MAX_READ_PER_POLL: usize = 16 * 1024 * 1024;

/// The most characters read for any text in a line, whatever the line says.
pub const MAX_FIELD_CHARS: usize = 2 * MAX_TEXT_CHARS;

/// A line longer than this is dropped.
pub const MAX_LINE_BYTES: usize = 2 * 1024 * 1024;

/// The most details kept in memory.
pub const MAX_ITEMS: usize = 2000;

fn clip(s: &mut String) {
    if s.chars().count() > MAX_FIELD_CHARS {
        *s = s.chars().take(MAX_FIELD_CHARS).collect();
    }
}

fn clip_all(d: &mut Detail) {
    for s in [&mut d.label, &mut d.code, &mut d.message, &mut d.thread, &mut d.url, &mut d.request, &mut d.request_headers, &mut d.response_headers, &mut d.response] {
        clip(s);
    }
    d.assertions.truncate(200);
    for a in &mut d.assertions {
        clip(&mut a.name);
        clip(&mut a.message);
    }
    d.sub_results.truncate(50);
    for s in &mut d.sub_results {
        clip(&mut s.label);
        clip(&mut s.code);
    }
}

/// Reads new lines from the details file of a run that may still be going.
pub struct DetailsTail {
    offset: u64,
    pending: Vec<u8>,
    skipped: u64,
    more: bool,
    kept: usize,
}

impl DetailsTail {
    pub fn new() -> DetailsTail {
        DetailsTail { offset: 0, pending: Vec::new(), skipped: 0, more: false, kept: 0 }
    }

    /// Lines that could not be read, or were too long.
    pub fn skipped(&self) -> u64 {
        self.skipped
    }

    /// Whether the last call stopped at its read limit, so that calling again will find more at once.
    pub fn has_more(&self) -> bool {
        self.more
    }

    /// Read anything new since the last call and return the complete lines that could be read.
    pub fn poll(&mut self, path: &Path) -> Result<Vec<Detail>> {
        self.more = false;
        let Ok(mut f) = std::fs::File::open(path) else { return Ok(vec![]) };
        use std::io::Seek;
        let len = f.metadata().map(|m| m.len()).unwrap_or(0);
        if len < self.offset {
            *self = DetailsTail { skipped: self.skipped, ..DetailsTail::new() };
        }
        if self.kept >= MAX_ITEMS {
            return Ok(vec![]);
        }
        f.seek(std::io::SeekFrom::Start(self.offset))?;
        let mut buf = Vec::new();
        f.take(MAX_READ_PER_POLL as u64).read_to_end(&mut buf)?;
        self.offset += buf.len() as u64;
        self.more = buf.len() == MAX_READ_PER_POLL;
        self.pending.extend_from_slice(&buf);
        let mut out = Vec::new();
        let mut start = 0;
        for i in 0..self.pending.len() {
            if self.pending[i] != b'\n' {
                continue;
            }
            let line = &self.pending[start..i];
            start = i + 1;
            let line = line.strip_suffix(b"\r").unwrap_or(line);
            if line.is_empty() {
                continue;
            }
            // Only a JSON object is a detail (serde would also read an array as the fields in order).
            if line.len() > MAX_LINE_BYTES || self.kept + out.len() >= MAX_ITEMS || line.first() != Some(&b'{') {
                self.skipped += 1;
                continue;
            }
            match serde_json::from_slice::<Detail>(line) {
                Ok(mut d) => {
                    clip_all(&mut d);
                    out.push(d);
                }
                Err(_) => self.skipped += 1,
            }
        }
        self.pending.drain(..start);
        if self.pending.len() > MAX_LINE_BYTES {
            self.pending.clear();
            self.skipped += 1;
        }
        self.kept += out.len();
        Ok(out)
    }
}

impl Default for DetailsTail {
    fn default() -> Self {
        Self::new()
    }
}

/// Read the whole details file of a finished run.
pub fn read_all(path: &Path) -> (Vec<Detail>, u64) {
    let mut tail = DetailsTail::new();
    let mut all = Vec::new();
    loop {
        match tail.poll(path) {
            Ok(mut items) => all.append(&mut items),
            Err(_) => break,
        }
        if !tail.has_more() {
            break;
        }
    }
    (all, tail.skipped())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("jmeter-ui-details-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d.join("details.jsonl")
    }

    const LINE: &str = r#"{"ts":1792950293417,"elapsed":57,"latency":51,"connect":29,"label":"Log in","code":"500","message":"Internal Server Error","thread":"Shoppers 1-1","success":false,"bytes":4,"sent":211,"url":"http://127.0.0.1:8089/login","request":"POST http://127.0.0.1:8089/login\n\nPOST data:\n{\"user\":\"demo\"}\n\n[no cookies]\n","request_headers":"Content-Type: application/json\nX-Demo: 1\n","response_headers":"HTTP/1.1 500 Internal Server Error\nContent-Length: 4\n","response":"boom","binary":false,"truncated":false,"assertions":[{"name":"Status is 200","failure":true,"error":false,"message":"Test failed: code expected to equal /\n\n500"}],"sub_results":[]}"#;

    #[test]
    fn a_line_from_the_listener_is_read_whole() {
        let p = temp("one");
        std::fs::write(&p, format!("{LINE}\n")).unwrap();
        let (items, skipped) = read_all(&p);
        assert_eq!(skipped, 0);
        let d = &items[0];
        assert_eq!((d.ts, d.elapsed, d.code.as_str(), d.success, d.label.as_str()), (1792950293417, 57, "500", false, "Log in"));
        assert_eq!(d.assertions.len(), 1);
        assert!(d.assertions[0].failure && d.assertions[0].message.contains("expected to equal"));
        assert_eq!(d.response, "boom");
    }

    #[test]
    fn the_request_and_the_response_are_laid_out_to_read() {
        let p = temp("text");
        std::fs::write(&p, format!("{LINE}\n")).unwrap();
        let d = &read_all(&p).0[0];
        assert_eq!(d.request_line(), "POST http://127.0.0.1:8089/login");
        assert_eq!(d.request_body(), Some("{\"user\":\"demo\"}"));
        assert_eq!(d.request_text(), "POST http://127.0.0.1:8089/login\nContent-Type: application/json\nX-Demo: 1\n\n{\"user\":\"demo\"}");
        assert_eq!(d.response_text(), "HTTP/1.1 500 Internal Server Error\nContent-Length: 4\n\nboom");
    }

    #[test]
    fn a_get_has_no_body_and_a_binary_response_is_not_shown() {
        let d = Detail { request: "GET http://x/a\n\n[no cookies]\n".into(), url: "http://x/a".into(), code: "200".into(), message: "OK".into(), binary: true, ..Default::default() };
        assert_eq!(d.request_body(), None);
        assert_eq!(d.request_text(), "GET http://x/a\n");
        assert!(d.response_text().starts_with("200 OK\n") && d.response_text().contains("not text"));
        let empty = Detail::default();
        assert_eq!(empty.request_text(), "\n");
        assert_eq!(empty.response_text(), " \n");
    }

    #[test]
    fn a_body_that_looks_like_the_markers_is_cut_at_the_first_real_one() {
        let d = Detail { request: "POST http://x\n\nPOST data:\nline\n\n[no cookies]\n".into(), ..Default::default() };
        assert_eq!(d.request_body(), Some("line"));
        let d = Detail { request: "POST http://x\n\nPOST data:\na b\n\nCookie Data:\nsid=1\n".into(), ..Default::default() };
        assert_eq!(d.request_body(), Some("a b"));
        let d = Detail { request: "PUT http://x\n\nPUT data:\nno end marker".into(), ..Default::default() };
        assert_eq!(d.request_body(), Some("no end marker"));
    }

    #[test]
    fn lines_are_read_as_they_arrive_and_a_half_line_waits() {
        let p = temp("tail");
        let mut tail = DetailsTail::new();
        assert!(tail.poll(&p).unwrap().is_empty(), "no file yet");
        let (a, b) = LINE.split_at(100);
        std::fs::write(&p, a).unwrap();
        assert!(tail.poll(&p).unwrap().is_empty());
        std::fs::write(&p, format!("{a}{b}\n{LINE}")).unwrap();
        assert_eq!(tail.poll(&p).unwrap().len(), 1, "the first line is whole now; the second has no end yet");
        let mut f = std::fs::OpenOptions::new().append(true).open(&p).unwrap();
        std::io::Write::write_all(&mut f, b"\n").unwrap();
        assert_eq!(tail.poll(&p).unwrap().len(), 1);
        assert!(tail.poll(&p).unwrap().is_empty(), "nothing new");
        assert_eq!(tail.skipped(), 0);
    }

    #[test]
    fn damaged_lines_are_skipped_and_counted_and_the_rest_is_read() {
        let p = temp("damaged");
        let text = format!("not json\n{LINE}\n{{\"ts\": \"wrong type\"}}\n\n[1,2]\n{{\"label\":\"ok\"}}\n\u{0}\u{1}\u{2}\n{LINE}\n");
        std::fs::write(&p, text).unwrap();
        let (items, skipped) = read_all(&p);
        assert_eq!(items.len(), 3, "{items:?}");
        assert_eq!(items[1].label, "ok");
        assert_eq!(skipped, 4);
    }

    #[test]
    fn missing_fields_are_empty_and_unknown_ones_are_ignored() {
        let p = temp("fields");
        std::fs::write(&p, "{\"label\":\"x\",\"surprise\":[1,2,3],\"code\":\"200\"}\n").unwrap();
        let d = &read_all(&p).0[0];
        assert_eq!((d.label.as_str(), d.code.as_str(), d.elapsed, d.response.as_str()), ("x", "200", 0, ""));
    }

    #[test]
    fn texts_in_a_line_are_limited_whatever_the_file_says() {
        let p = temp("clip");
        let huge = "x".repeat(MAX_FIELD_CHARS * 3);
        let many: Vec<String> = (0..500).map(|i| format!("{{\"name\":\"a{i}\",\"failure\":true,\"message\":\"m\"}}")).collect();
        std::fs::write(&p, format!("{{\"label\":\"{huge}\",\"response\":\"{huge}\",\"assertions\":[{}]}}\n", many.join(","))).unwrap();
        let d = &read_all(&p).0[0];
        assert_eq!(d.label.chars().count(), MAX_FIELD_CHARS);
        assert_eq!(d.response.chars().count(), MAX_FIELD_CHARS);
        assert_eq!(d.assertions.len(), 200);
    }

    #[test]
    fn a_line_that_is_too_long_or_never_ends_is_dropped_without_using_the_memory() {
        let p = temp("long");
        let mut text = String::from("{\"label\":\"");
        text.push_str(&"y".repeat(MAX_LINE_BYTES + 10));
        text.push_str("\"}\n");
        text.push_str(LINE);
        text.push('\n');
        std::fs::write(&p, text).unwrap();
        let (items, skipped) = read_all(&p);
        assert_eq!((items.len(), skipped), (1, 1));
        // And one that has no end at all.
        std::fs::write(&p, "z".repeat(MAX_LINE_BYTES * 2)).unwrap();
        let mut tail = DetailsTail::new();
        assert!(tail.poll(&p).unwrap().is_empty());
        assert_eq!(tail.skipped(), 1);
    }

    #[test]
    fn no_more_than_the_limit_is_kept_in_memory() {
        let p = temp("many");
        let text = format!("{LINE}\n").repeat(MAX_ITEMS + 500);
        std::fs::write(&p, text).unwrap();
        let (items, skipped) = read_all(&p);
        assert_eq!(items.len(), MAX_ITEMS);
        assert_eq!(skipped, 500);
    }

    #[test]
    fn a_shorter_file_starts_the_reading_again() {
        let p = temp("replaced");
        std::fs::write(&p, format!("{LINE}\n{LINE}\n")).unwrap();
        let mut tail = DetailsTail::new();
        assert_eq!(tail.poll(&p).unwrap().len(), 2);
        std::fs::write(&p, format!("{LINE}\n")).unwrap();
        assert_eq!(tail.poll(&p).unwrap().len(), 1);
    }

    #[test]
    fn the_properties_name_the_mode_the_file_and_a_limit_that_cannot_be_raised() {
        let p = props(Mode::All, Path::new("C:/runs/1/details.jsonl"), 100_000);
        assert_eq!(p[0], ("jmeterui.details".to_string(), "all".to_string()));
        assert_eq!(p[1].1, "C:/runs/1/details.jsonl");
        assert_eq!(p[2].1, MAX_KEPT.to_string());
        for (k, _) in &p {
            assert!(crate::run::valid_property_name(k), "{k}");
        }
        assert_eq!((Mode::Off.word(), Mode::Failed.word(), Mode::All.word()), ("off", "failed", "all"));
        assert_eq!(Mode::default(), Mode::Failed);
    }

    #[test]
    fn the_script_keeps_to_what_it_promises() {
        assert!(SCRIPT.contains("catch (Throwable"), "a problem in the listener must never fail a request");
        assert!(SCRIPT.contains("synchronized (props)"), "lines from several threads must not mix");
        assert!(SCRIPT.contains("jmeterui.details.max"));
        assert!(!SCRIPT.contains("Runtime.getRuntime") && !SCRIPT.contains(".execute()") && !SCRIPT.contains("ProcessBuilder"), "the script runs nothing");
        assert!(!SCRIPT.contains("http://") && !SCRIPT.contains("https://") && !SCRIPT.contains(".openConnection"), "the script contacts nothing");
        assert!(SCRIPT.len() < 6000);
    }
}
