//! Turning a HAR file (what a browser saves with "Save all as HAR" in its developer tools) into HTTP requests for a plan:
//! a recording of a real visit, including HTTPS, without a proxy or a certificate.
//!
//! Nothing here fetches or runs anything: the file is only read as text. Images, styles, scripts and fonts are left out
//! unless asked for, a browser's preflight checks are left out, and everything that was left out or changed is counted so
//! the person importing knows.

use crate::curl::{Body, CurlRequest};
use anyhow::{Result, bail};
use serde::Deserialize;

/// The largest HAR file read, in bytes.
pub const MAX_BYTES: usize = 64 * 1024 * 1024;
/// The most requests one import brings.
pub const MAX_REQUESTS: usize = 2000;
/// The longest request body kept, in characters.
pub const MAX_BODY_CHARS: usize = 1_000_000;
/// The longest header value kept, in characters.
const MAX_HEADER_CHARS: usize = 8000;
/// The most headers kept for a request.
const MAX_HEADERS: usize = 100;

/// What to bring in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Options {
    /// Leave out images, styles, scripts, fonts and media.
    pub skip_static: bool,
    /// Group the requests by the page they belong to, in a Transaction Controller for each page.
    pub group_by_page: bool,
    /// Only requests to these hosts (all when empty), compared without case.
    pub hosts: Vec<String>,
}

impl Default for Options {
    fn default() -> Self {
        Options { skip_static: true, group_by_page: true, hosts: Vec::new() }
    }
}

/// What a read found, for the dialog.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Summary {
    /// Requests in the file.
    pub total: usize,
    /// Requests that will be brought in.
    pub kept: usize,
    pub skipped_static: usize,
    pub skipped_preflight: usize,
    pub skipped_other_hosts: usize,
    /// Not http or https (data:, blob:, websockets).
    pub skipped_not_http: usize,
    /// Hosts and how many requests went to each, most first.
    pub hosts: Vec<(String, usize)>,
}

impl Summary {
    /// One sentence about what was left out, or an empty text when nothing was.
    pub fn left_out(&self) -> String {
        let mut parts = Vec::new();
        for (n, what) in [(self.skipped_static, "images, styles, scripts and fonts"), (self.skipped_preflight, "preflight checks"), (self.skipped_other_hosts, "requests to other servers"), (self.skipped_not_http, "addresses that are not http or https")] {
            if n > 0 {
                parts.push(format!("{n} {what}"));
            }
        }
        if parts.is_empty() { String::new() } else { format!("Left out: {}.", parts.join(", ")) }
    }
}

// ---- the file ----

#[derive(Deserialize, Default)]
#[serde(default)]
struct Har {
    log: Log,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Log {
    pages: Vec<Page>,
    entries: Vec<Entry>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Page {
    id: String,
    title: String,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Entry {
    pageref: Option<String>,
    #[serde(rename = "_resourceType")]
    resource_type: Option<String>,
    request: Req,
    response: Resp,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Req {
    method: String,
    url: String,
    headers: Vec<Pair>,
    #[serde(rename = "postData")]
    post_data: Option<Post>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Resp {
    content: Content,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Content {
    #[serde(rename = "mimeType")]
    mime_type: String,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Pair {
    name: String,
    value: String,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Post {
    #[serde(rename = "mimeType")]
    mime_type: String,
    text: Option<String>,
    params: Vec<Pair>,
}

fn is_static(e: &Entry) -> bool {
    if let Some(t) = &e.resource_type {
        return matches!(t.as_str(), "script" | "stylesheet" | "image" | "font" | "media" | "texttrack" | "manifest");
    }
    let mime = e.response.content.mime_type.to_ascii_lowercase();
    if mime.starts_with("image/") || mime.starts_with("font/") || mime.starts_with("audio/") || mime.starts_with("video/") || mime.contains("javascript") || mime.contains("ecmascript") || mime == "text/css" || mime.contains("font-woff") || mime.contains("x-font") {
        return true;
    }
    let path = e.request.url.split(['?', '#']).next().unwrap_or("").to_ascii_lowercase();
    [".png", ".jpg", ".jpeg", ".gif", ".svg", ".ico", ".webp", ".avif", ".css", ".js", ".mjs", ".woff", ".woff2", ".ttf", ".otf", ".eot", ".map", ".mp4", ".mp3"].iter().any(|x| path.ends_with(x))
}

fn is_preflight(e: &Entry) -> bool {
    e.request.method.eq_ignore_ascii_case("OPTIONS") && e.request.headers.iter().any(|h| h.name.eq_ignore_ascii_case("access-control-request-method"))
}

/// The headers worth sending: not the ones JMeter writes itself, not a browser's `:authority` style pseudo headers, and
/// not cookies (a Cookie Manager does that), cut to a sensible size.
fn headers_of(h: &[Pair]) -> Vec<(String, String)> {
    h.iter()
        .filter(|p| !p.name.starts_with(':') && !p.name.is_empty() && !p.name.chars().any(|c| c.is_control() || c.is_whitespace()) && !crate::curl::dropped_header(&p.name) && !p.name.eq_ignore_ascii_case("cookie"))
        .take(MAX_HEADERS)
        .map(|p| (p.name.clone(), p.value.chars().filter(|c| !c.is_control()).take(MAX_HEADER_CHARS).collect()))
        .collect()
}

fn body_of(p: &Option<Post>, notes: &mut Vec<String>) -> Body {
    let Some(p) = p else { return Body::None };
    if let Some(text) = p.text.as_ref().filter(|t| !t.is_empty()) {
        let form = p.mime_type.to_ascii_lowercase().starts_with("application/x-www-form-urlencoded");
        if form && !p.params.is_empty() {
            return Body::Fields(p.params.iter().map(|x| (x.name.clone(), x.value.clone())).collect());
        }
        if text.chars().count() > MAX_BODY_CHARS {
            notes.push(format!("The body was longer than {MAX_BODY_CHARS} characters and was cut."));
            return Body::Raw(text.chars().take(MAX_BODY_CHARS).collect());
        }
        return Body::Raw(text.clone());
    }
    if !p.params.is_empty() {
        if p.mime_type.to_ascii_lowercase().starts_with("multipart/") {
            notes.push("Files in the form were not kept; only its fields.".to_string());
            return Body::Multipart(p.params.iter().map(|x| (x.name.clone(), x.value.clone())).collect());
        }
        return Body::Fields(p.params.iter().map(|x| (x.name.clone(), x.value.clone())).collect());
    }
    Body::None
}

/// Read a HAR file: the requests that match the options, and what was left out. A file that is not a HAR file is an error
/// with a sentence.
pub fn parse(text: &str, options: &Options) -> Result<(Vec<CurlRequest>, Summary)> {
    if text.len() > MAX_BYTES {
        bail!("The file is larger than {} MB.", MAX_BYTES / (1024 * 1024));
    }
    let har: Har = serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|e| anyhow::anyhow!("This is not a HAR file: {e}"))?;
    if har.log.entries.is_empty() {
        bail!("This HAR file has no requests in it.");
    }
    let hosts: Vec<String> = options.hosts.iter().map(|h| h.trim().to_ascii_lowercase()).filter(|h| !h.is_empty()).collect();
    let mut summary = Summary { total: har.log.entries.len(), ..Default::default() };
    let mut counts: Vec<(String, usize)> = Vec::new();
    let mut out: Vec<CurlRequest> = Vec::new();
    for e in &har.log.entries {
        let (protocol, host, port, path, _userinfo) = match crate::curl::split_url(&e.request.url) {
            Ok(x) if e.request.url.contains("://") => x,
            _ => {
                summary.skipped_not_http += 1;
                continue;
            }
        };
        let lower = host.to_ascii_lowercase();
        match counts.iter_mut().find(|(h, _)| *h == lower) {
            Some((_, n)) => *n += 1,
            None => counts.push((lower, 1)),
        }
        if is_preflight(e) {
            summary.skipped_preflight += 1;
            continue;
        }
        if options.skip_static && is_static(e) {
            summary.skipped_static += 1;
            continue;
        }
        if !hosts.is_empty() && !hosts.iter().any(|h| *h == host.to_ascii_lowercase()) {
            summary.skipped_other_hosts += 1;
            continue;
        }
        if out.len() >= MAX_REQUESTS {
            bail!("More than {MAX_REQUESTS} requests would be imported. Skip the static files or choose a host to bring in fewer.");
        }
        let mut notes = Vec::new();
        let method = if e.request.method.is_empty() { "GET".to_string() } else { e.request.method.to_ascii_uppercase() };
        let body = body_of(&e.request.post_data, &mut notes);
        let group = if options.group_by_page {
            e.pageref.as_ref().map(|id| har.log.pages.iter().find(|p| p.id == *id).map(|p| if p.title.trim().is_empty() { p.id.clone() } else { p.title.chars().filter(|c| !c.is_control()).take(80).collect() }).unwrap_or_else(|| id.clone()))
        } else {
            None
        };
        out.push(CurlRequest { method, protocol, host, port, path, headers: headers_of(&e.request.headers), body, notes, group });
    }
    counts.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    summary.hosts = counts;
    summary.kept = out.len();
    if out.is_empty() {
        bail!("Nothing is left to import with these choices.{}", if summary.left_out().is_empty() { String::new() } else { format!(" {}", summary.left_out()) });
    }
    Ok((out, summary))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn har(entries: &str) -> String {
        format!(r#"{{"log":{{"version":"1.2","pages":[{{"id":"page_1","title":"Home page"}},{{"id":"page_2","title":""}}],"entries":[{entries}]}}}}"#)
    }

    fn entry(method: &str, url: &str, mime: &str, extra: &str) -> String {
        format!(r#"{{"pageref":"page_1","request":{{"method":"{method}","url":"{url}","headers":[{{"name":":authority","value":"x"}},{{"name":"Host","value":"x"}},{{"name":"Accept","value":"*/*"}},{{"name":"Cookie","value":"sid=1"}}]{extra}}},"response":{{"content":{{"mimeType":"{mime}"}}}}}}"#)
    }

    #[test]
    fn requests_are_read_with_their_headers_but_not_cookies_or_pseudo_headers() {
        let text = har(&entry("get", "https://shop.example.com/items?page=2", "application/json", ""));
        let (r, s) = parse(&text, &Options::default()).unwrap();
        assert_eq!(r.len(), 1);
        assert_eq!((r[0].method.as_str(), r[0].protocol.as_str(), r[0].host.as_str(), r[0].path.as_str()), ("GET", "https", "shop.example.com", "/items?page=2"));
        assert_eq!(r[0].headers, vec![("Accept".to_string(), "*/*".to_string())]);
        assert_eq!(r[0].group.as_deref(), Some("Home page"));
        assert_eq!((s.total, s.kept), (1, 1));
        assert_eq!(s.left_out(), "");
    }

    #[test]
    fn static_files_and_preflights_are_left_out_and_counted() {
        let text = har(&[
            entry("GET", "https://x.org/api", "application/json", ""),
            entry("GET", "https://x.org/logo.png", "image/png", ""),
            entry("GET", "https://x.org/app.js?v=3", "application/javascript", ""),
            entry("GET", "https://x.org/style", "text/css", ""),
            entry("GET", "https://x.org/font", "font/woff2", ""),
            entry("GET", "https://x.org/noext.js?x=1", "", ""),
            r#"{"request":{"method":"OPTIONS","url":"https://x.org/api","headers":[{"name":"Access-Control-Request-Method","value":"POST"}]},"response":{}}"#.to_string(),
            entry("GET", "data:image/png;base64,AAAA", "image/png", ""),
            entry("GET", "wss://x.org/socket", "", ""),
        ]
        .join(","));
        let (r, s) = parse(&text, &Options::default()).unwrap();
        assert_eq!(r.len(), 1, "{r:?}");
        assert_eq!((s.total, s.kept, s.skipped_static, s.skipped_preflight, s.skipped_not_http), (9, 1, 5, 1, 2));
        assert!(s.left_out().contains("5 images, styles, scripts and fonts") && s.left_out().contains("1 preflight checks"));
        // With static files wanted they are all there.
        let (r, _) = parse(&text, &Options { skip_static: false, ..Default::default() }).unwrap();
        assert_eq!(r.len(), 6);
    }

    #[test]
    fn chromes_resource_type_decides_when_it_is_there() {
        let text = har(r#"{"_resourceType":"xhr","request":{"method":"GET","url":"https://x.org/data.js"},"response":{"content":{"mimeType":"application/javascript"}}},{"_resourceType":"script","request":{"method":"GET","url":"https://x.org/api"},"response":{"content":{"mimeType":"application/json"}}}"#);
        let (r, _) = parse(&text, &Options::default()).unwrap();
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].path, "/data.js", "a request the page made with xhr is kept even if it asked for a .js file");
    }

    #[test]
    fn hosts_can_be_chosen_and_are_listed_most_used_first() {
        let text = har(&[entry("GET", "https://a.org/1", "text/html", ""), entry("GET", "https://B.org/2", "text/html", ""), entry("GET", "https://b.org/3", "text/html", "")].join(","));
        let (_, s) = parse(&text, &Options::default()).unwrap();
        assert_eq!(s.hosts, vec![("b.org".to_string(), 2), ("a.org".to_string(), 1)], "hosts are counted without case, most used first");
        let (r, s) = parse(&text, &Options { hosts: vec![" A.org ".into()], ..Default::default() }).unwrap();
        assert_eq!((r.len(), s.skipped_other_hosts), (1, 2));
        assert!(parse(&text, &Options { hosts: vec!["nothing.org".into()], ..Default::default() }).unwrap_err().to_string().contains("Nothing is left"));
    }

    #[test]
    fn bodies_become_a_body_fields_or_a_multipart_form() {
        let json = entry("POST", "https://x.org/login", "text/plain", r#","postData":{"mimeType":"application/json","text":"{\"u\":\"demo\"}"}"#);
        let form = entry("POST", "https://x.org/f", "text/plain", r#","postData":{"mimeType":"application/x-www-form-urlencoded","text":"a=1&b=2","params":[{"name":"a","value":"1"},{"name":"b","value":"two words"}]}"#);
        let multi = entry("POST", "https://x.org/m", "text/plain", r#","postData":{"mimeType":"multipart/form-data; boundary=x","params":[{"name":"a","value":"1"},{"name":"file","value":"","fileName":"x.png"}]}"#);
        let (r, _) = parse(&har(&[json, form, multi].join(",")), &Options::default()).unwrap();
        assert_eq!(r[0].body, Body::Raw("{\"u\":\"demo\"}".into()));
        assert_eq!(r[1].body, Body::Fields(vec![("a".into(), "1".into()), ("b".into(), "two words".into())]));
        assert!(matches!(&r[2].body, Body::Multipart(f) if f.len() == 2));
        assert!(r[2].notes.iter().any(|n| n.contains("Files in the form")));
    }

    #[test]
    fn pages_become_groups_only_when_asked_and_a_missing_title_uses_the_id() {
        let a = entry("GET", "https://x.org/a", "text/html", "");
        let b = a.replace("page_1", "page_2");
        let c = a.replace("page_1", "page_9");
        let d = r#"{"request":{"method":"GET","url":"https://x.org/d"},"response":{}}"#.to_string();
        let text = har(&[a, b, c, d].join(","));
        let (r, _) = parse(&text, &Options::default()).unwrap();
        assert_eq!(r.iter().map(|x| x.group.clone()).collect::<Vec<_>>(), vec![Some("Home page".to_string()), Some("page_2".to_string()), Some("page_9".to_string()), None]);
        let (r, _) = parse(&text, &Options { group_by_page: false, ..Default::default() }).unwrap();
        assert!(r.iter().all(|x| x.group.is_none()));
    }

    #[test]
    fn what_is_not_a_har_file_gets_a_sentence() {
        for (text, want) in [("", "not a HAR file"), ("not json", "not a HAR file"), ("[1,2]", "not a HAR file"), ("{}", "no requests"), (r#"{"log":{"entries":[]}}"#, "no requests"), (r#"{"log":{"entries":[{"request":{"url":"ftp://x.org/a"}}]}}"#, "Nothing is left")] {
            let e = parse(text, &Options::default()).unwrap_err().to_string();
            assert!(e.contains(want), "{text:?}: {e}");
        }
        assert!(parse(&"x".repeat(MAX_BYTES + 1), &Options::default()).unwrap_err().to_string().contains("larger than"));
    }

    #[test]
    fn hostile_values_are_cut_or_dropped_and_never_break_the_read() {
        let long = "v".repeat(MAX_HEADER_CHARS * 2);
        let e = format!(r#"{{"request":{{"method":"G\u0000ET","url":"https://x.org/a\u0007b","headers":[{{"name":"X-Long","value":"{long}"}},{{"name":"bad name","value":"1"}},{{"name":"X-Ctl","value":"a\u0000b\nc"}}]}},"response":{{}}}}"#);
        let (r, _) = parse(&har(&e), &Options::default()).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(r[0].headers.len(), 2, "a name with a space is dropped");
        assert_eq!(r[0].headers[0].1.chars().count(), MAX_HEADER_CHARS);
        assert_eq!(r[0].headers[1].1, "abc", "control characters are removed from values");
        // Too many requests are refused with a way out.
        let many: String = (0..=MAX_REQUESTS).map(|i| entry("GET", &format!("https://x.org/{i}"), "text/html", "")).collect::<Vec<_>>().join(",");
        assert!(parse(&har(&many), &Options::default()).unwrap_err().to_string().contains("More than"));
        // Deeply nested JSON is refused by the reader, not by a crash.
        let deep = format!("{}{}", "[".repeat(5000), "]".repeat(5000));
        assert!(parse(&deep, &Options::default()).is_err());
    }
}
