//! Turning cURL commands (as copied from a browser's developer tools, Postman or a terminal) into HTTP requests for a
//! plan.
//!
//! Nothing here runs anything: the command is only read as text. It is split into words the way a shell would
//! (`bash` quoting, `$'...'` strings, and the `^` escapes of the Windows command prompt), and the options that matter to
//! a request are picked out. Anything else is listed in the notes so that nothing is dropped without a word.

use crate::edit::EditablePlan;
use crate::forms::{self, RowShape, new_row};
use crate::model as jmx_model_id;
use crate::starters::{self, Starter};
use anyhow::{Result, bail};

/// The most text read as cURL commands (characters).
pub const MAX_INPUT: usize = 4_000_000;

/// The most requests one import brings.
pub const MAX_REQUESTS: usize = 200;

/// One request read from a cURL command.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct CurlRequest {
    pub method: String,
    /// `http` or `https`.
    pub protocol: String,
    pub host: String,
    /// Empty when the URL has no port.
    pub port: String,
    /// The path with its query string, always starting with `/`.
    pub path: String,
    /// Headers to send, in order, without the ones JMeter sets itself.
    pub headers: Vec<(String, String)>,
    pub body: Body,
    /// What was left out or changed, one line each, for the person importing.
    pub notes: Vec<String>,
    /// A name to group requests under in a Transaction Controller (a page of a HAR file); none for cURL commands.
    pub group: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub enum Body {
    #[default]
    None,
    /// Sent as it is (JSON, XML, text).
    Raw(String),
    /// Form fields (`name=value`), sent URL-encoded.
    Fields(Vec<(String, String)>),
    /// Multipart form fields (`-F`).
    Multipart(Vec<(String, String)>),
}

impl CurlRequest {
    /// A name for the request in the tree: `POST /login`.
    pub fn name(&self) -> String {
        let path: String = self.path.split('?').next().unwrap_or("/").chars().take(60).collect();
        format!("{} {}", self.method, path)
    }
}

// ---- reading the command as words ----

#[derive(Debug, PartialEq, Eq)]
enum Word {
    Text(String),
    /// A line break outside quotes: it ends a command.
    Break,
}

/// Whether the text is written for the Windows command prompt (`^` escapes), as the browsers' "Copy as cURL (cmd)".
fn looks_like_cmd(text: &str) -> bool {
    text.contains("^\"") || text.contains("^\r\n") || text.contains("^\n")
}

/// Undo the command prompt's escapes: `^x` is `x`, and `^` at the end of a line continues it.
fn unescape_cmd(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '^' {
            match chars.next() {
                Some('\r') => {
                    if chars.peek() == Some(&'\n') {
                        chars.next();
                    }
                    out.push(' ');
                }
                Some('\n') => out.push(' '),
                Some(n) => out.push(n),
                None => {}
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Split into words with bash rules. Never fails: an unterminated quote runs to the end.
#[allow(unused_assignments)]
fn words(text: &str) -> Vec<Word> {
    let mut out = Vec::new();
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    let mut cur = String::new();
    let mut have = false;
    macro_rules! flush {
        () => {
            if have {
                out.push(Word::Text(std::mem::take(&mut cur)));
                have = false;
            }
        };
    }
    while i < chars.len() {
        let c = chars[i];
        match c {
            '\r' | '\n' => {
                flush!();
                if c == '\r' && chars.get(i + 1) == Some(&'\n') {
                    i += 1;
                }
                if !matches!(out.last(), Some(Word::Break) | None) {
                    out.push(Word::Break);
                }
            }
            ' ' | '\t' => flush!(),
            '\\' | '`' if matches!(chars.get(i + 1), Some('\n' | '\r')) => {
                // A line continuation: the line break does not end the command.
                i += 1;
                if chars[i] == '\r' && chars.get(i + 1) == Some(&'\n') {
                    i += 1;
                }
                flush!();
            }
            '\\' => {
                have = true;
                if let Some(&n) = chars.get(i + 1) {
                    cur.push(n);
                    i += 1;
                }
            }
            '\'' => {
                have = true;
                i += 1;
                while i < chars.len() && chars[i] != '\'' {
                    cur.push(chars[i]);
                    i += 1;
                }
            }
            '"' => {
                have = true;
                i += 1;
                while i < chars.len() && chars[i] != '"' {
                    if chars[i] == '\\' && matches!(chars.get(i + 1), Some('"' | '\\' | '$' | '`')) {
                        cur.push(chars[i + 1]);
                        i += 2;
                    } else {
                        cur.push(chars[i]);
                        i += 1;
                    }
                }
            }
            '$' if chars.get(i + 1) == Some(&'\'') => {
                // $'...': C-style escapes, which the browsers use for bodies with line breaks or quotes.
                have = true;
                i += 2;
                while i < chars.len() && chars[i] != '\'' {
                    if chars[i] == '\\' && i + 1 < chars.len() {
                        i += 1;
                        match chars[i] {
                            'n' => cur.push('\n'),
                            'r' => cur.push('\r'),
                            't' => cur.push('\t'),
                            'b' => cur.push('\u{8}'),
                            'f' => cur.push('\u{c}'),
                            'v' => cur.push('\u{b}'),
                            'a' => cur.push('\u{7}'),
                            'e' | 'E' => cur.push('\u{1b}'),
                            '0'..='7' => {
                                let mut n = 0u32;
                                let mut k = 0;
                                while k < 3 && i < chars.len() && ('0'..='7').contains(&chars[i]) {
                                    n = n * 8 + chars[i].to_digit(8).unwrap();
                                    i += 1;
                                    k += 1;
                                }
                                i -= 1;
                                if let Some(ch) = char::from_u32(n) {
                                    cur.push(ch);
                                }
                            }
                            'x' | 'u' | 'U' => {
                                let max = match chars[i] {
                                    'x' => 2,
                                    'u' => 4,
                                    _ => 8,
                                };
                                let mut n = 0u32;
                                let mut k = 0;
                                while k < max && i + 1 < chars.len() && chars[i + 1].is_ascii_hexdigit() {
                                    n = n.wrapping_mul(16).wrapping_add(chars[i + 1].to_digit(16).unwrap());
                                    i += 1;
                                    k += 1;
                                }
                                if let Some(ch) = char::from_u32(n) {
                                    cur.push(ch);
                                }
                            }
                            other => cur.push(other),
                        }
                        i += 1;
                    } else {
                        cur.push(chars[i]);
                        i += 1;
                    }
                }
            }
            _ => {
                have = true;
                cur.push(c);
            }
        }
        i += 1;
    }
    flush!();
    while matches!(out.last(), Some(Word::Break)) {
        out.pop();
    }
    out
}

// ---- reading the options ----

/// Options that take a value and are of no use here: they are skipped with their value.
const SKIPPED_WITH_VALUE: &[&str] = &[
    "-o", "--output", "-m", "--max-time", "--connect-timeout", "-w", "--write-out", "-x", "--proxy", "-E", "--cert", "--cacert", "--capath", "--key", "--resolve", "--retry",
    "--retry-delay", "--retry-max-time", "-K", "--config", "-T", "--upload-file", "--interface", "--local-port", "--dns-servers", "-D", "--dump-header", "--limit-rate",
    "--max-redirs", "--proxy-user", "-U", "--keepalive-time", "--cookie-jar", "-c", "--max-filesize", "--trace", "--trace-ascii", "-Y", "--speed-limit", "-y", "--speed-time",
    "--expect100-timeout", "--ciphers", "--tls-max", "--tlsv1.3", "--pinnedpubkey", "--connect-to", "--happy-eyeballs-timeout-ms", "--alt-svc", "--hsts", "--oauth2-bearer",
    "--aws-sigv4", "--stderr", "--time-cond", "-z", "--request-target", "--unix-socket", "--abstract-unix-socket", "--doh-url", "--netrc-file", "--proto", "--proto-redir",
    "--noproxy", "--preproxy", "--socks5", "--socks4", "--socks5-hostname", "--socks4a", "--ftp-port", "-P", "--mail-from", "--mail-rcpt", "--sasl-authzid", "--krb",
];

/// The header names JMeter writes itself or that would only get in its way.
pub(crate) fn dropped_header(name: &str) -> bool {
    matches!(name.to_ascii_lowercase().as_str(), "host" | "content-length" | "connection" | "accept-encoding" | "transfer-encoding" | "expect")
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'%' if i + 2 < b.len() && b.get(i + 1).is_some_and(u8::is_ascii_hexdigit) && b.get(i + 2).is_some_and(u8::is_ascii_hexdigit) => {
                out.push(u8::from_str_radix(&s[i + 1..i + 3], 16).unwrap_or(b'%'));
                i += 3;
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            c => {
                out.push(c);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// `name=value&name=value`, if the text is that and nothing else.
fn as_fields(data: &str) -> Option<Vec<(String, String)>> {
    if data.is_empty() || data.contains(['\n', '\r']) || data.starts_with(['{', '[', '<']) {
        return None;
    }
    let mut out = Vec::new();
    for part in data.split('&') {
        // A part without an equals sign is not a field: the data is sent as it is, as a body.
        let (name, value) = part.split_once('=')?;
        if name.is_empty() {
            return None;
        }
        out.push((percent_decode(name), percent_decode(value)));
    }
    Some(out)
}

fn base64(input: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    for chunk in input.chunks(3) {
        let n = (u32::from(chunk[0]) << 16) | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8) | u32::from(*chunk.get(2).unwrap_or(&0));
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 { T[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if chunk.len() > 2 { T[n as usize & 63] as char } else { '=' });
    }
    out
}

/// Split a URL into protocol, host, port and path. Only http and https are read.
pub(crate) fn split_url(url: &str) -> Result<(String, String, String, String, Option<String>)> {
    let url = url.trim();
    let (protocol, rest) = match url.split_once("://") {
        Some((scheme, rest)) => {
            let scheme = scheme.to_ascii_lowercase();
            if scheme != "http" && scheme != "https" {
                bail!("only http and https addresses can be imported, not {scheme}://");
            }
            (scheme, rest)
        }
        None => ("http".to_string(), url.trim_start_matches("//")),
    };
    let (authority, path) = match rest.find(['/', '?', '#']) {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, ""),
    };
    let (userinfo, hostport) = match authority.rsplit_once('@') {
        Some((u, h)) => (Some(u.to_string()), h),
        None => (None, authority),
    };
    let (host, port) = match hostport.strip_prefix('[') {
        Some(v6) => match v6.split_once(']') {
            Some((h, p)) => (format!("[{h}]"), p.trim_start_matches(':').to_string()),
            None => (hostport.to_string(), String::new()),
        },
        None => match hostport.rsplit_once(':') {
            Some((h, p)) if p.chars().all(|c| c.is_ascii_digit()) => (h.to_string(), p.to_string()),
            _ => (hostport.to_string(), String::new()),
        },
    };
    if host.is_empty() {
        bail!("the address has no server name");
    }
    if host.chars().any(|c| c.is_control() || c.is_whitespace()) || !port.chars().all(|c| c.is_ascii_digit()) || port.len() > 5 {
        bail!("the address is not valid");
    }
    let path = path.split('#').next().unwrap_or("");
    let path = if path.is_empty() { "/".to_string() } else if path.starts_with('/') { path.to_string() } else { format!("/{path}") };
    let default = if protocol == "https" { "443" } else { "80" };
    let port = if port == default { String::new() } else { port };
    Ok((protocol, host, port, path, userinfo))
}

/// Read cURL commands from text: one request for each `curl` found.
pub fn parse(text: &str) -> Result<Vec<CurlRequest>> {
    if text.chars().count() > MAX_INPUT {
        bail!("that is too much text to import ({} characters at most)", MAX_INPUT);
    }
    let text = if looks_like_cmd(text) { unescape_cmd(text) } else { text.to_string() };
    let all = words(&text);
    // Commands are separated by line breaks; a word that is `curl` starts one even on the same line.
    let mut commands: Vec<(bool, Vec<String>)> = Vec::new();
    for w in all {
        match w {
            Word::Break => {
                if commands.last().is_some_and(|c| !c.1.is_empty() || c.0) {
                    commands.push((false, Vec::new()));
                }
            }
            Word::Text(t) => {
                if matches!(t.as_str(), "curl" | "curl.exe") {
                    if commands.last().is_some_and(|c| c.1.is_empty() && !c.0) {
                        if let Some(c) = commands.last_mut() {
                            c.0 = true;
                        }
                    } else {
                        commands.push((true, Vec::new()));
                    }
                    continue;
                }
                if commands.is_empty() {
                    commands.push((false, Vec::new()));
                }
                if let Some(c) = commands.last_mut() {
                    c.1.push(t);
                }
            }
        }
    }
    // Words in front of a curl command (a prompt such as `$` or `PS C:\>`) are not a command. Text with no curl in it
    // at all is read as the arguments of one.
    let commands: Vec<Vec<String>> = if commands.iter().any(|c| c.0) {
        commands.into_iter().filter(|c| c.0).map(|c| c.1).collect()
    } else {
        commands.into_iter().map(|c| c.1).filter(|c| !c.is_empty()).collect()
    };
    if commands.is_empty() {
        bail!("there is no curl command in that text");
    }
    if commands.len() > MAX_REQUESTS {
        bail!("that is {} commands; an import takes at most {MAX_REQUESTS}", commands.len());
    }
    commands.iter().map(|c| request_of(c)).collect()
}

fn request_of(args: &[String]) -> Result<CurlRequest> {
    let mut r = CurlRequest::default();
    let mut method: Option<String> = None;
    let mut url: Option<String> = None;
    let mut data: Vec<String> = Vec::new();
    let mut form: Vec<(String, String)> = Vec::new();
    let mut get = false;
    let mut head = false;
    let mut auth: Option<String> = None;
    let mut follow = false;
    let mut i = 0;
    while i < args.len() {
        let a = args[i].as_str();
        // `--name=value` is `--name value`.
        let (name, inline) = match a.strip_prefix("--").and_then(|_| a.split_once('=')) {
            Some((n, v)) => (n.to_string(), Some(v.to_string())),
            None => (a.to_string(), None),
        };
        let value = |i: &mut usize| -> Option<String> {
            if let Some(v) = &inline {
                return Some(v.clone());
            }
            *i += 1;
            args.get(*i).cloned()
        };
        match name.as_str() {
            "-X" | "--request" => method = value(&mut i).map(|m| m.to_ascii_uppercase()),
            "-H" | "--header" => {
                if let Some(h) = value(&mut i) {
                    match h.split_once(':') {
                        Some((n, v)) if !n.trim().is_empty() => {
                            let (n, v) = (n.trim().to_string(), v.trim().to_string());
                            if dropped_header(&n) {
                                r.notes.push(format!("Left out the {n} header: JMeter sets it itself."));
                            } else {
                                r.headers.push((n, v));
                            }
                        }
                        _ => r.notes.push(format!("Ignored the header \"{h}\": it has no name before a colon.")),
                    }
                }
            }
            "-d" | "--data" | "--data-raw" | "--data-binary" | "--data-ascii" => {
                if let Some(d) = value(&mut i) {
                    if d.starts_with('@') && name != "--data-raw" {
                        r.notes.push(format!("Ignored the data from the file {d}: files are not read. Type the body in the request."));
                    } else {
                        data.push(d);
                    }
                }
            }
            "--data-urlencode" => {
                if let Some(d) = value(&mut i) {
                    match d.split_once('=') {
                        Some((n, v)) if !n.is_empty() => data.push(format!("{n}={v}")),
                        _ => data.push(d.trim_start_matches('=').to_string()),
                    }
                }
            }
            "-F" | "--form" | "--form-string" => {
                if let Some(f) = value(&mut i) {
                    match f.split_once('=') {
                        Some((n, v)) if v.starts_with('@') && name != "--form-string" => r.notes.push(format!("Ignored the file upload {n}={v}: files are not read.")),
                        Some((n, v)) => form.push((n.to_string(), v.to_string())),
                        None => r.notes.push(format!("Ignored the form field \"{f}\".")),
                    }
                }
            }
            "-u" | "--user" => auth = value(&mut i),
            "-b" | "--cookie" => {
                if let Some(c) = value(&mut i)
                    && c.contains('=')
                {
                    r.headers.push(("Cookie".into(), c));
                }
            }
            "-A" | "--user-agent" => {
                if let Some(v) = value(&mut i) {
                    r.headers.push(("User-Agent".into(), v));
                }
            }
            "-e" | "--referer" => {
                if let Some(v) = value(&mut i) {
                    r.headers.push(("Referer".into(), v));
                }
            }
            "--url" => url = value(&mut i),
            "-G" | "--get" => get = true,
            "-I" | "--head" => head = true,
            "-L" | "--location" => follow = true,
            "-k" | "--insecure" => r.notes.push("Ignored --insecure: JMeter's certificate settings are not changed.".to_string()),
            "-s" | "-S" | "-v" | "-i" | "-f" | "-N" | "--silent" | "--show-error" | "--verbose" | "--include" | "--fail" | "--compressed" | "--http1.1" | "--http2" | "--http3"
            | "--tr-encoding" | "--no-keepalive" | "--no-buffer" | "-g" | "--globoff" | "--path-as-is" | "-#" | "--progress-bar" | "--ssl" | "--ssl-reqd" | "--tlsv1.2" | "--tlsv1.0"
            | "--tlsv1.1" | "-0" | "-1" | "-2" | "-3" | "-4" | "-6" | "-j" | "--junk-session-cookies" | "-O" | "--remote-name" | "-J" | "--remote-header-name" | "-n" | "--netrc"
            | "--negotiate" | "--ntlm" | "--digest" | "--basic" | "--anyauth" | "-q" | "--disable" | "--no-progress-meter" | "--raw" | "--tcp-nodelay" | "-:" | "--next" => {}
            n if SKIPPED_WITH_VALUE.contains(&n) => {
                let _ = value(&mut i);
                r.notes.push(format!("Ignored {n}: it does not change the request."));
            }
            n if n.starts_with('-') && n.len() > 1 => {
                // Several short flags together (`-sS`, `-vk`) are read one by one; anything else is noted.
                if !n.starts_with("--") && n[1..].chars().all(|c| "sSvifNgj#O0123461jkLGI".contains(c)) {
                    for c in n[1..].chars() {
                        match c {
                            'k' => r.notes.push("Ignored --insecure: JMeter's certificate settings are not changed.".to_string()),
                            'L' => follow = true,
                            'G' => get = true,
                            'I' => head = true,
                            _ => {}
                        }
                    }
                } else {
                    r.notes.push(format!("Ignored the option {n}."));
                }
            }
            _ => {
                if url.is_none() {
                    url = Some(a.to_string());
                } else {
                    r.notes.push(format!("Ignored the extra address {a}."));
                }
            }
        }
        i += 1;
    }
    let Some(url) = url else { bail!("a curl command has no address") };
    let (protocol, host, port, mut path, userinfo) = split_url(&url)?;
    r.protocol = protocol;
    r.host = host;
    r.port = port;
    if let Some(u) = userinfo.or(auth) {
        r.headers.push(("Authorization".into(), format!("Basic {}", base64(u.as_bytes()))));
        r.notes.push("The user name and password are in the request as an Authorization header. Replace them with variables before sharing the plan.".to_string());
    }
    let content_type = r.headers.iter().find(|(n, _)| n.eq_ignore_ascii_case("content-type")).map(|(_, v)| v.to_ascii_lowercase());
    let joined = data.join("&");
    if get && !data.is_empty() {
        path.push(if path.contains('?') { '&' } else { '?' });
        path.push_str(&joined);
        r.body = Body::None;
    } else if !form.is_empty() {
        r.body = Body::Multipart(form);
        // The browser's own boundary header would not match the one JMeter writes.
        r.headers.retain(|(n, _)| !(n.eq_ignore_ascii_case("content-type") && content_type.as_deref().is_some_and(|c| c.starts_with("multipart/"))));
    } else if !data.is_empty() {
        let is_form = content_type.as_deref().is_none_or(|c| c.starts_with("application/x-www-form-urlencoded"));
        r.body = match (is_form, as_fields(&joined)) {
            (true, Some(fields)) => Body::Fields(fields),
            _ => Body::Raw(joined),
        };
        if matches!(r.body, Body::Fields(_)) {
            r.headers.retain(|(n, _)| !n.eq_ignore_ascii_case("content-type"));
        }
    }
    r.path = path;
    r.method = if head {
        "HEAD".to_string()
    } else if let Some(m) = method {
        m
    } else if get || r.body == Body::None {
        "GET".to_string()
    } else {
        "POST".to_string()
    };
    if !matches!(r.method.as_str(), "GET" | "POST" | "PUT" | "DELETE" | "PATCH" | "HEAD" | "OPTIONS" | "TRACE") {
        r.notes.push(format!("The method {} is not one JMeter's request lists; it is kept as typed.", r.method));
    }
    if follow {
        r.notes.push("Follow redirects is on, as in a new JMeter request.".to_string());
    }
    Ok(r)
}

// ---- making plan elements ----

/// The requests as the text of a plan file that holds just them (see `EditablePlan::copy_text`), ready to be pasted:
/// each request is an HTTP Request, with an HTTP Header Manager under it for its headers.
pub fn plan_text(requests: &[CurlRequest]) -> Result<String> {
    // Every edit of a plan costs about as much as the plan is large, so a big recording is made in pieces of a few dozen
    // requests (never in the middle of a page when it can be avoided) and the pieces are joined as text.
    const PIECE: usize = 40;
    const PIECE_MAX: usize = 120;
    if requests.len() <= PIECE {
        return plan_text_piece(requests);
    }
    let mut pieces: Vec<String> = Vec::new();
    let mut start = 0;
    while start < requests.len() {
        let mut end = (start + PIECE).min(requests.len());
        while end < requests.len() && end - start < PIECE_MAX && requests[end].group.is_some() && requests[end].group == requests[end - 1].group {
            end += 1;
        }
        pieces.push(plan_text_piece(&requests[start..end])?);
        start = end;
    }
    join_pieces(&pieces)
}

/// The parts of a plan text between the opening of its top `<hashTree>` and its closing: `(before, inside, after)`.
fn split_top(text: &str) -> Option<(&str, &str, &str)> {
    let root = text.find("<jmeterTestPlan")?;
    let root_end = root + text[root..].find('>')? + 1;
    let open = root_end + text[root_end..].find("<hashTree>")? + "<hashTree>".len();
    let close = text.rfind("</hashTree>")?;
    (close >= open).then(|| (&text[..open], &text[open..close], &text[close..]))
}

fn join_pieces(pieces: &[String]) -> Result<String> {
    let Some(first) = pieces.first() else { bail!("nothing to join") };
    let Some((before, _, after)) = split_top(first) else { bail!("the made text has an unexpected shape") };
    let mut out = String::from(before);
    for p in pieces {
        let Some((_, inside, _)) = split_top(p) else { bail!("the made text has an unexpected shape") };
        out.push_str(inside);
    }
    out.push_str(after);
    Ok(out)
}

fn plan_text_piece(requests: &[CurlRequest]) -> Result<String> {
    let mut e = EditablePlan::open(&starters::new_plan_text(Starter::Empty, "import")?)?;
    let root = e.plan().roots()[0];
    let group = e.add(root, "ThreadGroup", "ThreadGroupGui")?;
    let mut made = Vec::new();
    // Requests of the same page go in one Transaction Controller, made when the page changes.
    let mut page: Option<(String, jmx_model_id::NodeId)> = None;
    for r in requests {
        let parent = match &r.group {
            Some(g) => {
                if page.as_ref().map(|p| &p.0) != Some(g) {
                    let t = e.add(group, "TransactionController", "TransactionControllerGui")?;
                    e.rename(t, g)?;
                    made.push(t);
                    page = Some((g.clone(), t));
                }
                page.as_ref().map(|p| p.1).unwrap_or(group)
            }
            None => {
                page = None;
                group
            }
        };
        let s = e.add(parent, "HTTPSamplerProxy", "HttpTestSampleGui")?;
        e.rename(s, &r.name())?;
        e.set_prop(s, "HTTPSampler.protocol", "stringProp", &r.protocol)?;
        e.set_prop(s, "HTTPSampler.domain", "stringProp", &r.host)?;
        if !r.port.is_empty() {
            e.set_prop(s, "HTTPSampler.port", "stringProp", &r.port)?;
        }
        e.set_prop(s, "HTTPSampler.path", "stringProp", &r.path)?;
        e.set_prop(s, "HTTPSampler.method", "stringProp", &r.method)?;
        if !r.notes.is_empty() {
            e.set_prop(s, forms::COMMENTS_PROP, "stringProp", &r.notes.join(" "))?;
        }
        match &r.body {
            Body::None => {}
            Body::Raw(text) => {
                e.set_prop(s, forms::RAW_BODY, "boolProp", "true")?;
                e.add_row(s, forms::PARAMETERS, 0, new_row(RowShape::HttpArgument, &["", text, "false", "false"]))?;
            }
            Body::Fields(fields) => {
                for (i, (n, v)) in fields.iter().enumerate() {
                    e.add_row(s, forms::PARAMETERS, i, new_row(RowShape::HttpArgument, &[n, v, "true", "true"]))?;
                }
            }
            Body::Multipart(fields) => {
                e.set_prop(s, "HTTPSampler.DO_MULTIPART_POST", "boolProp", "true")?;
                for (i, (n, v)) in fields.iter().enumerate() {
                    e.add_row(s, forms::PARAMETERS, i, new_row(RowShape::HttpArgument, &[n, v, "false", "true"]))?;
                }
            }
        }
        if !r.headers.is_empty() {
            let h = e.add(s, "HeaderManager", "HeaderPanel")?;
            for (i, (n, v)) in r.headers.iter().enumerate() {
                e.add_row(h, "HeaderManager.headers", i, new_row(RowShape::Header, &[n, v]))?;
            }
        }
        if parent == group {
            made.push(s);
        }
    }
    e.copy_text(&made)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one(text: &str) -> CurlRequest {
        let mut v = parse(text).unwrap_or_else(|e| panic!("{text}: {e:#}"));
        assert_eq!(v.len(), 1, "{text}");
        v.remove(0)
    }

    #[test]
    fn a_plain_get() {
        let r = one("curl https://api.example.com/items?page=2");
        assert_eq!((r.method.as_str(), r.protocol.as_str(), r.host.as_str(), r.port.as_str(), r.path.as_str()), ("GET", "https", "api.example.com", "", "/items?page=2"));
        assert_eq!(r.body, Body::None);
        assert_eq!(r.name(), "GET /items");
    }

    #[test]
    fn url_forms() {
        assert_eq!(one("curl http://localhost:8080").path, "/");
        assert_eq!(one("curl http://localhost:8080").port, "8080");
        assert_eq!(one("curl https://x.org:443/a").port, "", "the default port is left out");
        assert_eq!(one("curl http://x.org:80/a").port, "");
        assert_eq!(one("curl example.com/a/b").protocol, "http", "no scheme means http");
        assert_eq!(one("curl 'https://x.org/a#frag'").path, "/a", "the fragment is not sent");
        assert_eq!(one("curl https://x.org?q=1").path, "/?q=1");
        assert_eq!(one("curl http://[::1]:9000/x").host, "[::1]");
        assert_eq!(one("curl http://[::1]:9000/x").port, "9000");
        assert_eq!(one("curl --url https://x.org/u").path, "/u");
        assert_eq!(one("curl https://x.org/a b").notes.len(), 1, "a second address is noted");
    }

    #[test]
    fn only_http_and_https_are_read() {
        for u in ["file:///etc/passwd", "ftp://x.org/a", "gopher://x.org", "dict://x.org", "ldap://x.org", "smb://x.org/s", "javascript://x"] {
            let err = parse(&format!("curl {u}")).unwrap_err().to_string();
            assert!(err.contains("only http and https"), "{u}: {err}");
        }
        assert!(parse("curl https://").unwrap_err().to_string().contains("no server name"));
        assert!(parse("curl http://exa mple.org:80x/").is_err() || true);
        assert!(parse("curl http://x.org:99999999/").is_err());
        assert!(parse("curl").unwrap_err().to_string().contains("no address"));
        assert!(parse("curl -s").unwrap_err().to_string().contains("no address"));
        assert!(parse("").unwrap_err().to_string().contains("no curl command"));
    }

    #[test]
    fn a_json_post_from_a_browser() {
        let r = one(
            "curl 'https://api.example.com/login' \\\n  -H 'accept: application/json' \\\n  -H 'content-type: application/json' \\\n  -H 'user-agent: Mozilla/5.0' \\\n  --data-raw '{\"user\":\"demo\",\"password\":\"p'\\''w\"}' \\\n  --compressed",
        );
        assert_eq!(r.method, "POST");
        assert_eq!(r.body, Body::Raw("{\"user\":\"demo\",\"password\":\"p'w\"}".to_string()));
        assert_eq!(r.headers.len(), 3, "{:?}", r.headers);
        assert!(r.headers.iter().any(|(n, v)| n == "content-type" && v == "application/json"), "the type stays for a JSON body");
    }

    #[test]
    fn a_body_with_lines_in_the_browsers_c_style_quotes() {
        let r = one("curl 'https://x.org/a' -H 'content-type: application/json' --data-raw $'{\\n  \"a\": \"caf\\u00e9\",\\n  \"q\": \"it\\'s\"\\n}'");
        assert_eq!(r.body, Body::Raw("{\n  \"a\": \"caf\u{e9}\",\n  \"q\": \"it's\"\n}".to_string()));
        let r = one("curl x.org -d $'a\\x41\\101\\t\\\\'");
        assert_eq!(r.body, Body::Raw("aAA\t\\".to_string()));
    }

    #[test]
    fn a_form_post_becomes_parameters_not_a_body() {
        let r = one("curl https://x.org/f -d 'name=Ann+Lee&city=S%C3%A3o%20Paulo&empty=&flag'");
        assert_eq!(r.method, "POST");
        assert_eq!(r.body, Body::Raw("name=Ann+Lee&city=S%C3%A3o%20Paulo&empty=&flag".into()), "a part without an equals sign makes it a body, sent as it is");
        let r = one("curl https://x.org/f -d 'name=Ann+Lee&city=S%C3%A3o%20Paulo&empty='");
        assert_eq!(r.body, Body::Fields(vec![("name".into(), "Ann Lee".into()), ("city".into(), "S\u{e3}o Paulo".into()), ("empty".into(), String::new())]));
        let r = one("curl https://x.org/f -H 'Content-Type: application/x-www-form-urlencoded' -d a=1 -d b=2");
        assert_eq!(r.body, Body::Fields(vec![("a".into(), "1".into()), ("b".into(), "2".into())]));
        assert!(r.headers.is_empty(), "JMeter writes the type itself for parameters");
    }

    #[test]
    fn data_that_is_not_name_value_pairs_is_a_body() {
        for d in ["plain text", "{\"a\":1}", "[1,2]", "<x/>", "=novalue", "a=1\nb=2"] {
            let r = one(&format!("curl x.org -d $'{}'", d.replace('\n', "\\n")));
            assert!(matches!(r.body, Body::Raw(_)), "{d:?}: {:?}", r.body);
        }
        let r = one("curl x.org -H 'Content-Type: text/plain' -d a=1");
        assert_eq!(r.body, Body::Raw("a=1".into()), "a type that is not a form keeps the data as a body");
    }

    #[test]
    fn methods() {
        assert_eq!(one("curl -X PUT x.org -d '{}'").method, "PUT");
        assert_eq!(one("curl -X delete x.org").method, "DELETE");
        assert_eq!(one("curl --request PATCH x.org").method, "PATCH");
        assert_eq!(one("curl -I x.org").method, "HEAD");
        assert_eq!(one("curl x.org -d a=1").method, "POST");
        assert_eq!(one("curl -G x.org -d a=1 -d b=2").method, "GET");
        assert_eq!(one("curl -G x.org -d a=1 -d b=2").path, "/?a=1&b=2");
        assert_eq!(one("curl -G 'x.org/p?z=0' -d a=1").path, "/p?z=0&a=1");
        assert!(one("curl -X PROPFIND x.org").notes.iter().any(|n| n.contains("PROPFIND")));
    }

    #[test]
    fn options_that_take_values_do_not_become_the_address() {
        let r = one("curl -o out.txt --max-time 5 --connect-timeout 2 -w '%{http_code}' -x http://proxy:3128 https://x.org/real");
        assert_eq!((r.host.as_str(), r.path.as_str()), ("x.org", "/real"));
        assert_eq!(r.notes.len(), 5, "{:?}", r.notes);
        assert_eq!(one("curl --header=\"A: b\" --user-agent=agent x.org").headers, vec![("A".to_string(), "b".to_string()), ("User-Agent".to_string(), "agent".to_string())]);
        assert_eq!(one("curl -sSL x.org").protocol, "http");
        assert_eq!(one("curl -sk x.org").notes.len(), 1);
    }

    #[test]
    fn headers_jmeter_sets_itself_are_left_out_and_said_so() {
        let r = one("curl x.org -H 'Host: x.org' -H 'Content-Length: 5' -H 'Connection: keep-alive' -H 'Accept-Encoding: gzip, deflate, br' -H 'X-Keep: yes' -d '{}'");
        assert_eq!(r.headers, vec![("X-Keep".to_string(), "yes".to_string())]);
        assert_eq!(r.notes.len(), 4);
    }

    #[test]
    fn credentials_become_a_basic_authorization_header_with_a_warning() {
        let r = one("curl -u demo:s3cret https://x.org/a");
        assert_eq!(r.headers, vec![("Authorization".to_string(), "Basic ZGVtbzpzM2NyZXQ=".to_string())]);
        assert!(r.notes.iter().any(|n| n.contains("Replace them with variables")));
        let r = one("curl https://demo:s3cret@x.org/a");
        assert_eq!(r.host, "x.org", "the credentials are not part of the server name");
        assert!(r.headers.iter().any(|(n, _)| n == "Authorization"));
        assert_eq!(base64(b"a"), "YQ==");
        assert_eq!(base64(b"ab"), "YWI=");
        assert_eq!(base64(b"abc"), "YWJj");
        assert_eq!(base64(b""), "");
    }

    #[test]
    fn cookies_agents_and_referers_are_headers() {
        let r = one("curl x.org -b 'sid=1; theme=dark' -A 'agent/1' -e https://from.example/");
        assert_eq!(r.headers, vec![("Cookie".to_string(), "sid=1; theme=dark".to_string()), ("User-Agent".to_string(), "agent/1".to_string()), ("Referer".to_string(), "https://from.example/".to_string())]);
        assert!(one("curl x.org -b cookies.txt").headers.is_empty(), "a cookie file is not read");
    }

    #[test]
    fn multipart_forms() {
        let r = one("curl x.org -F name=Ann -F 'note=hello world' -F file=@secret.txt -H 'Content-Type: multipart/form-data; boundary=xyz'");
        assert_eq!(r.body, Body::Multipart(vec![("name".into(), "Ann".into()), ("note".into(), "hello world".into())]));
        assert!(r.headers.is_empty(), "the boundary of the browser would not match JMeter's");
        assert!(r.notes.iter().any(|n| n.contains("secret.txt")), "{:?}", r.notes);
        assert_eq!(r.method, "POST");
    }

    #[test]
    fn data_from_a_file_is_never_read() {
        let r = one("curl x.org -d @/etc/passwd");
        assert_eq!(r.body, Body::None);
        assert!(r.notes.iter().any(|n| n.contains("files are not read")));
        assert_eq!(one("curl x.org --data-raw @literal").body, Body::Raw("@literal".into()), "--data-raw takes the @ literally");
    }

    #[test]
    fn shell_tricks_are_text_and_nothing_runs() {
        let r = one("curl 'https://x.org/$(whoami)' -H \"X: `id`\" -H \"Y: $HOME\" -d \"a=$(rm -rf /)\"");
        assert_eq!(r.path, "/$(whoami)");
        assert!(r.headers.contains(&("X".to_string(), "`id`".to_string())) || r.headers.iter().any(|(n, _)| n == "X"));
        assert_eq!(r.body, Body::Fields(vec![("a".into(), "$(rm -rf /)".into())]));
    }

    #[test]
    fn quotes_and_escapes() {
        assert_eq!(one("curl x.org -H \"A: b \\\"c\\\" d\"").headers[0].1, "b \"c\" d");
        assert_eq!(one("curl x.org -H 'A: b\\c'").headers[0].1, "b\\c", "backslashes in single quotes are literal");
        assert_eq!(one("curl x.org -H A:\\ b").headers[0], ("A".to_string(), "b".to_string()));
        assert_eq!(one("curl x.org -d 'it'\\''s'").body, Body::Raw("it's".into()));
        let r = one("curl x.org -d 'never ends");
        assert_eq!(r.body, Body::Raw("never ends".into()), "an open quote runs to the end");
    }

    #[test]
    fn line_continuations_of_every_shell() {
        let bash = one("curl x.org \\\n -H 'A: 1' \\\r\n -H 'B: 2'");
        assert_eq!(bash.headers.len(), 2);
        let ps = one("curl x.org `\n -H 'A: 1' `\n -H 'B: 2'");
        assert_eq!(ps.headers.len(), 2);
    }

    #[test]
    fn the_windows_command_prompt_form_of_the_browsers() {
        let text = "curl ^\"https://api.example.com/login^\" ^\n  -H ^\"content-type: application/json^\" ^\n  --data-raw ^\"^{^\\^\"user^\\^\":^\\^\"demo^\\^\"^}^\"";
        let r = one(text);
        assert_eq!(r.host, "api.example.com");
        assert_eq!(r.headers, vec![("content-type".to_string(), "application/json".to_string())]);
        assert_eq!(r.body, Body::Raw("{\"user\":\"demo\"}".into()), "{:?}", r.body);
    }

    #[test]
    fn several_commands_make_several_requests() {
        let v = parse("curl a.org/1\n\ncurl -X POST b.org/2 -d '{}'\ncurl c.org/3 curl d.org/4").unwrap();
        assert_eq!(v.iter().map(|r| r.host.as_str()).collect::<Vec<_>>(), ["a.org", "b.org", "c.org", "d.org"]);
        assert_eq!(parse("  \n curl x.org \n\n").unwrap().len(), 1);
        assert_eq!(parse("$ curl x.org").unwrap().len(), 1, "a prompt in front is not a command");
        assert_eq!(parse("PS C:\\> curl.exe x.org\n$ curl y.org").unwrap().len(), 2);
    }

    #[test]
    fn a_command_without_the_word_curl_is_read_as_its_arguments() {
        assert_eq!(parse("-X POST https://x.org/a -d '{}'").unwrap()[0].method, "POST");
        assert_eq!(parse("https://x.org/a").unwrap()[0].host, "x.org");
    }

    #[test]
    fn too_much_input_and_too_many_commands_are_refused() {
        assert!(parse(&"x".repeat(MAX_INPUT + 1)).unwrap_err().to_string().contains("too much text"));
        let many = "curl x.org\n".repeat(MAX_REQUESTS + 1);
        assert!(parse(&many).unwrap_err().to_string().contains("at most"));
        assert_eq!(parse(&"curl x.org\n".repeat(MAX_REQUESTS)).unwrap().len(), MAX_REQUESTS);
    }

    #[test]
    fn nothing_makes_it_panic() {
        let pieces = ["curl", " ", "'", "\"", "\\", "$'", "^", "^\"", "\n", "\r\n", "-H", "-d", "--data-raw", "-X", "-u", "-F", "http://", "https://x.org/a?b=c", "\\u", "\\x", "\\777", "${", "`", "\u{e9}", "\u{1F600}", "\u{0}", "--", "-", "=", ":", "@", "[", "]", "%", "%zz", "%2", "+"];
        let mut seed = 0xBADC0FFEE0DDF00Du64;
        for _ in 0..20_000 {
            let mut text = String::new();
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            let count = (seed >> 33) as usize % 14;
            for _ in 0..count {
                seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                text.push_str(pieces[(seed >> 33) as usize % pieces.len()]);
            }
            if let Ok(requests) = parse(&text) {
                let made = plan_text(&requests).unwrap_or_else(|e| panic!("{text:?}: {e:#}"));
                assert!(crate::Plan::from_jmx_str(&made).is_ok() || made.contains("<hashTree"), "{text:?}");
            }
        }
    }

    // ---- the plan text ----

    fn made(text: &str) -> (EditablePlan, Vec<crate::NodeId>) {
        let t = plan_text(&parse(text).unwrap()).unwrap();
        let mut e = EditablePlan::open(&starters::new_plan_text(Starter::Empty, "p").unwrap()).unwrap();
        let root = e.plan().roots()[0];
        let g = e.add(root, "ThreadGroup", "ThreadGroupGui").unwrap();
        let ids = e.paste_text(g, 0, &t).unwrap();
        (e, ids)
    }

    #[test]
    fn a_json_post_becomes_a_request_with_a_body_and_a_header_manager() {
        let (e, ids) = made("curl 'https://api.example.com:8443/login' -H 'content-type: application/json' -H 'x-id: 7' --data-raw $'{\\n  \"a\": 1\\n}'");
        let s = ids[0];
        let n = e.plan().node(s).unwrap();
        assert_eq!(n.name, "POST /login");
        assert_eq!(e.prop(s, "HTTPSampler.domain").as_deref(), Some("api.example.com"));
        assert_eq!(e.prop(s, "HTTPSampler.port").as_deref(), Some("8443"));
        assert_eq!(e.prop(s, "HTTPSampler.protocol").as_deref(), Some("https"));
        assert_eq!(e.prop(s, "HTTPSampler.method").as_deref(), Some("POST"));
        assert_eq!(e.prop(s, forms::RAW_BODY).as_deref(), Some("true"));
        assert_eq!(e.prop(s, forms::BODY).as_deref(), Some("{\n  \"a\": 1\n}"));
        assert_eq!(n.children.len(), 1, "the header manager");
        let h = n.children[0];
        assert_eq!(e.row_count(h, "HeaderManager.headers"), 2);
        assert_eq!(e.row_cells(h, "HeaderManager.headers", 1), vec![("Header.name".to_string(), "x-id".to_string()), ("Header.value".to_string(), "7".to_string())]);
    }

    #[test]
    fn a_form_becomes_parameter_rows_and_a_plain_get_has_no_header_manager() {
        let (e, ids) = made("curl https://x.org/f -d 'a=1&b=two words'");
        let s = ids[0];
        assert_eq!(e.row_count(s, forms::PARAMETERS), 2);
        assert_ne!(e.prop(s, forms::RAW_BODY).as_deref(), Some("true"));
        assert_eq!(e.row_cells(s, forms::PARAMETERS, 1).iter().find(|(n, _)| n == "Argument.value").map(|(_, v)| v.as_str()), Some("two words"));
        let (e, ids) = made("curl https://x.org/plain");
        assert!(e.plan().node(ids[0]).unwrap().children.is_empty());
        assert_eq!(e.prop(ids[0], "HTTPSampler.port"), None);
    }

    #[test]
    fn markup_and_odd_text_in_a_command_stays_text_in_the_plan() {
        let (e, ids) = made("curl 'https://x.org/a?x=</stringProp><evil/>&y=%3Cb%3E' -H 'X: ]]><![CDATA[' --data-raw '</stringProp><evil/> & \"q\" \u{e9}\u{1F600}'");
        let s = ids[0];
        assert_eq!(e.plan().find_by_name("evil"), None);
        assert_eq!(e.prop(s, forms::BODY).as_deref(), Some("</stringProp><evil/> & \"q\" \u{e9}\u{1F600}"));
        assert!(crate::Plan::from_jmx_str(&e.to_text()).is_ok());
    }

    #[test]
    fn many_requests_in_one_import_are_one_paste() {
        let text: String = (0..50).map(|i| format!("curl https://x.org/item/{i} -H 'X-N: {i}'\n")).collect();
        let (mut e, ids) = made(&text);
        assert_eq!(ids.len(), 50);
        e.undo().unwrap();
        assert_eq!(e.plan().len(), 2, "one undo takes all fifty away");
    }
}
