//! Try an expression on some text: a JSON Path, a regular expression, an XPath or a CSS selector, evaluated by the same
//! libraries JMeter uses, so the answer is what JMeter would take from that text.
//!
//! A small Java program (`SOURCE`) is run with JMeter's libraries on the class path. What to evaluate goes in through
//! its standard input (never the command line), the answer comes back as one line of JSON, and the process is ended
//! when it takes too long, which also catches a regular expression that would never finish.

use serde::Deserialize;
use std::io::{Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// What kind of expression to try.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    JsonPath,
    Regex,
    XPath,
    Css,
}

impl Kind {
    pub const ALL: [Kind; 4] = [Kind::JsonPath, Kind::Regex, Kind::XPath, Kind::Css];

    pub fn word(self) -> &'static str {
        match self {
            Kind::JsonPath => "jsonpath",
            Kind::Regex => "regex",
            Kind::XPath => "xpath",
            Kind::Css => "css",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Kind::JsonPath => "JSON Path",
            Kind::Regex => "Regular expression",
            Kind::XPath => "XPath",
            Kind::Css => "CSS selector",
        }
    }
}

/// The longest expression, in characters.
pub const MAX_EXPRESSION_CHARS: usize = 10_000;
/// The largest text to try an expression on, in bytes.
pub const MAX_SAMPLE_BYTES: usize = 1024 * 1024;
/// How long the answer may take before the program is ended.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(15);
/// The most of the answer that is read, in bytes.
const MAX_ANSWER_BYTES: u64 = 4 * 1024 * 1024;

/// What an expression found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Found {
    /// One text for each thing found: a value, a match with its groups, a node.
    pub items: Vec<String>,
    /// More than were shown, or a text was cut.
    pub cut: bool,
}

#[derive(Deserialize)]
struct Answer {
    ok: bool,
    #[serde(default)]
    error: String,
    #[serde(default)]
    items: Vec<String>,
    #[serde(default)]
    cut: bool,
}

/// Where Java and JMeter are.
pub struct Tools<'a> {
    pub java: &'a Path,
    pub jmeter_home: &'a Path,
}

fn plain(s: &str) -> bool {
    !s.chars().any(|c| c.is_control())
}

static COUNTER: AtomicU64 = AtomicU64::new(0);

/// Try `expression` on `sample`. The error is a sentence for the person who wrote the expression: what is wrong with
/// it, with the text, or why the tools could not run.
pub fn evaluate(tools: &Tools, kind: Kind, expression: &str, sample: &str, timeout: Duration) -> Result<Found, String> {
    if expression.trim().is_empty() {
        return Err("Write an expression first.".into());
    }
    if expression.chars().count() > MAX_EXPRESSION_CHARS {
        return Err(format!("The expression is longer than {MAX_EXPRESSION_CHARS} characters."));
    }
    if sample.len() > MAX_SAMPLE_BYTES {
        return Err(format!("The text is larger than {} MB.", MAX_SAMPLE_BYTES / (1024 * 1024)));
    }
    let ext = tools.java.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    if matches!(ext.as_str(), "bat" | "cmd" | "com" | "ps1" | "vbs" | "js") || !tools.java.is_file() {
        return Err("Java was not found. Check the Runtime section in Settings.".into());
    }
    let lib = tools.jmeter_home.join("lib");
    if !tools.jmeter_home.join("bin").join("ApacheJMeter.jar").is_file() || !lib.is_dir() {
        return Err("JMeter was not found. Check the Runtime section in Settings.".into());
    }
    let (java, lib_text) = (tools.java.to_string_lossy().into_owned(), lib.to_string_lossy().into_owned());
    if !plain(&java) || !plain(&lib_text) {
        return Err("The Java or JMeter path has control characters.".into());
    }

    let dir = std::env::temp_dir().join(format!("jmeter-ui-tester-{}-{}", std::process::id(), COUNTER.fetch_add(1, Ordering::Relaxed)));
    std::fs::create_dir_all(&dir).map_err(|e| format!("Could not make a folder for the test: {e}"))?;
    let result = run(&java, &lib_text, &dir, kind, expression, sample, timeout);
    let _ = std::fs::remove_dir_all(&dir);
    result
}

fn run(java: &str, lib: &str, dir: &Path, kind: Kind, expression: &str, sample: &str, timeout: Duration) -> Result<Found, String> {
    std::fs::write(dir.join("Tester.java"), SOURCE).map_err(|e| format!("Could not write the test program: {e}"))?;
    let sep = if cfg!(windows) { ';' } else { ':' };
    let class_path = format!("{lib}{}*{sep}{lib}{}ext{}*", std::path::MAIN_SEPARATOR, std::path::MAIN_SEPARATOR, std::path::MAIN_SEPARATOR);
    let mut cmd = Command::new(java);
    cmd.args(["-Xmx256m", "-XX:+UseSerialGC", "-XX:TieredStopAtLevel=1", "-Djava.awt.headless=true", "-cp"])
        .arg(&class_path)
        .arg("Tester.java")
        .current_dir(dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    crate::locate::quiet(&mut cmd);
    let mut child = cmd.spawn().map_err(|e| format!("Could not start Java: {e}"))?;
    let _tied = crate::job::tie_to_this_process(&child);

    // What to evaluate goes in as length-prefixed texts, so nothing in them is ever taken for a command.
    let mut input = Vec::new();
    for field in [kind.word(), expression, sample] {
        input.extend_from_slice(&(field.len() as u32).to_be_bytes());
        input.extend_from_slice(field.as_bytes());
    }
    let mut stdin = child.stdin.take().ok_or("Could not talk to Java")?;
    let writer = std::thread::spawn(move || {
        let _ = stdin.write_all(&input);
    });
    let mut stdout = child.stdout.take().ok_or("Could not read from Java")?;
    let reader = std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = (&mut stdout).take(MAX_ANSWER_BYTES).read_to_end(&mut buf);
        buf
    });
    let mut stderr = child.stderr.take().ok_or("Could not read from Java")?;
    let err_reader = std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = (&mut stderr).take(64 * 1024).read_to_end(&mut buf);
        buf
    });

    let start = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break Some(s),
            Ok(None) if start.elapsed() >= timeout => break None,
            Ok(None) => std::thread::sleep(Duration::from_millis(20)),
            Err(e) => return Err(format!("Lost track of Java: {e}")),
        }
    };
    if status.is_none() {
        let _ = child.kill();
        let _ = child.wait();
    }
    let _ = writer.join();
    let out = reader.join().unwrap_or_default();
    let err = err_reader.join().unwrap_or_default();
    if status.is_none() {
        return Err(format!("Stopped after {} seconds. An expression that takes this long on this text would slow a test down too.", timeout.as_secs_f32()));
    }
    let text = String::from_utf8_lossy(&out);
    let line = text.lines().rev().find(|l| l.starts_with('{'));
    match line.and_then(|l| serde_json::from_str::<Answer>(l).ok()) {
        Some(a) if a.ok => Ok(Found { items: a.items, cut: a.cut }),
        Some(a) => Err(a.error),
        None => {
            let e = String::from_utf8_lossy(&err);
            let first = e.lines().find(|l| !l.trim().is_empty()).unwrap_or("no answer");
            if first.contains("invalid source release") || first.contains("source-file mode") || first.contains("Unrecognized option") {
                Err("Trying expressions needs Java 11 or newer.".into())
            } else {
                Err(format!("Java did not give an answer: {}", first.chars().take(300).collect::<String>()))
            }
        }
    }
}

/// The program that evaluates an expression. It uses the libraries that come with JMeter: Jayway JSON Path, `java.util.regex`,
/// the XML parser and XPath of Java, and jsoup.
pub const SOURCE: &str = r####"import java.io.*;
import java.nio.charset.StandardCharsets;
import java.util.*;
import java.util.regex.*;

public class Tester {
    static final int MAX_ITEMS = 200;
    static final int MAX_CHARS = 2000;
    static boolean cut = false;

    static String field(DataInputStream in) throws IOException {
        int n = in.readInt();
        if (n < 0 || n > 16 * 1024 * 1024) throw new IOException("bad length");
        byte[] b = new byte[n];
        in.readFully(b);
        return new String(b, StandardCharsets.UTF_8);
    }

    static String clip(String s) {
        if (s == null) return "null";
        if (s.length() > MAX_CHARS) { cut = true; return s.substring(0, MAX_CHARS); }
        return s;
    }

    static void add(List<String> out, String s) {
        if (out.size() >= MAX_ITEMS) { cut = true; return; }
        out.add(clip(s));
    }

    static String json(String s) {
        StringBuilder b = new StringBuilder("\"");
        for (int i = 0; i < s.length(); i++) {
            char c = s.charAt(i);
            switch (c) {
                case '"': b.append("\\\""); break;
                case '\\': b.append("\\\\"); break;
                case '\n': b.append("\\n"); break;
                case '\r': b.append("\\r"); break;
                case '\t': b.append("\\t"); break;
                default:
                    if (c < 0x20) b.append(String.format("\\u%04x", (int) c)); else b.append(c);
            }
        }
        return b.append('"').toString();
    }

    static String show(Object o) {
        if (o == null) return "null";
        if (o instanceof String) return (String) o;
        if (o instanceof Map || o instanceof List) return net.minidev.json.JSONValue.toJSONString(o);
        return String.valueOf(o);
    }

    static void jsonPath(String expr, String sample, List<String> out) throws Exception {
        Object doc = new net.minidev.json.parser.JSONParser(net.minidev.json.parser.JSONParser.MODE_PERMISSIVE).parse(sample);
        if (!(doc instanceof Map) && !(doc instanceof List)) throw new IllegalArgumentException("The text is not a JSON object or array.");
        Object r;
        try {
            r = com.jayway.jsonpath.JsonPath.read(doc, expr);
        } catch (com.jayway.jsonpath.PathNotFoundException e) {
            return;
        }
        if (r instanceof List) {
            for (Object o : (List<?>) r) add(out, show(o));
        } else {
            add(out, show(r));
        }
    }

    static void regex(String expr, String sample, List<String> out) {
        Matcher m = Pattern.compile(expr).matcher(sample);
        int n = 0;
        while (m.find()) {
            n++;
            if (out.size() >= MAX_ITEMS) { cut = true; break; }
            StringBuilder b = new StringBuilder("Match " + n + ": " + m.group());
            for (int g = 1; g <= m.groupCount(); g++) b.append("\n  group ").append(g).append(": ").append(m.group(g));
            add(out, b.toString());
        }
    }

    static void xpath(String expr, String sample, List<String> out) throws Exception {
        javax.xml.parsers.DocumentBuilderFactory f = javax.xml.parsers.DocumentBuilderFactory.newInstance();
        // A document may not bring its own definitions or fetch anything: no DOCTYPE at all.
        f.setFeature("http://apache.org/xml/features/disallow-doctype-decl", true);
        f.setXIncludeAware(false);
        f.setExpandEntityReferences(false);
        org.w3c.dom.Document d = f.newDocumentBuilder().parse(new org.xml.sax.InputSource(new StringReader(sample)));
        javax.xml.xpath.XPathExpression e = javax.xml.xpath.XPathFactory.newInstance().newXPath().compile(expr);
        Object nodes;
        try {
            nodes = e.evaluate(d, javax.xml.xpath.XPathConstants.NODESET);
        } catch (javax.xml.xpath.XPathExpressionException ex) {
            // Not a path to nodes (count(...), string(...), a sum): the value itself.
            add(out, e.evaluate(d));
            return;
        }
        org.w3c.dom.NodeList nl = (org.w3c.dom.NodeList) nodes;
        for (int i = 0; i < nl.getLength(); i++) add(out, nl.item(i).getTextContent());
    }

    static void css(String expr, String sample, List<String> out) {
        for (org.jsoup.nodes.Element el : org.jsoup.Jsoup.parse(sample).select(expr)) add(out, el.outerHtml());
    }

    public static void main(String[] args) throws Exception {
        PrintStream stdout = new PrintStream(new FileOutputStream(FileDescriptor.out), false, "UTF-8");
        String kind, expr, sample;
        try {
            DataInputStream in = new DataInputStream(new BufferedInputStream(System.in));
            kind = field(in);
            expr = field(in);
            sample = field(in);
        } catch (Throwable t) {
            stdout.println("{\"ok\":false,\"error\":" + json("The test could not read what to try.") + "}");
            stdout.flush();
            return;
        }
        List<String> out = new ArrayList<>();
        String error = null;
        try {
            switch (kind) {
                case "jsonpath": jsonPath(expr, sample, out); break;
                case "regex": regex(expr, sample, out); break;
                case "xpath": xpath(expr, sample, out); break;
                case "css": css(expr, sample, out); break;
                default: throw new IllegalArgumentException("Unknown kind of expression");
            }
        } catch (Throwable t) {
            String m = t.getMessage();
            if (m == null || m.isEmpty()) m = t.getClass().getSimpleName();
            if (m.length() > 600) m = m.substring(0, 600);
            error = m;
            if (t instanceof org.xml.sax.SAXParseException) error = "The text is not well-formed XML (or has a DOCTYPE, which is not allowed here): " + m;
        }
        StringBuilder b = new StringBuilder();
        if (error != null) {
            b.append("{\"ok\":false,\"error\":").append(json(error)).append('}');
        } else {
            b.append("{\"ok\":true,\"cut\":").append(cut).append(",\"items\":[");
            for (int i = 0; i < out.size(); i++) {
                if (i > 0) b.append(',');
                b.append(json(out.get(i)));
            }
            b.append("]}");
        }
        stdout.println(b);
        stdout.flush();
    }
}
"####;

#[cfg(test)]
mod tests {
    use super::*;

    fn tools<'a>() -> Tools<'a> {
        Tools { java: Path::new("no-such-java.exe"), jmeter_home: Path::new("no-such-jmeter") }
    }

    #[test]
    fn what_cannot_be_tried_is_refused_before_anything_starts() {
        let t = tools();
        let d = Duration::from_secs(1);
        assert!(evaluate(&t, Kind::Regex, "", "x", d).unwrap_err().contains("Write an expression"));
        assert!(evaluate(&t, Kind::Regex, "   ", "x", d).unwrap_err().contains("Write an expression"));
        assert!(evaluate(&t, Kind::Regex, &"a".repeat(MAX_EXPRESSION_CHARS + 1), "x", d).unwrap_err().contains("longer than"));
        assert!(evaluate(&t, Kind::Regex, "a", &"a".repeat(MAX_SAMPLE_BYTES + 1), d).unwrap_err().contains("larger than"));
        assert!(evaluate(&t, Kind::Regex, "a", "a", d).unwrap_err().contains("Java was not found"));
        for script in ["x.bat", "x.cmd", "x.ps1", "x.js"] {
            let t = Tools { java: Path::new(script), jmeter_home: Path::new("x") };
            assert!(evaluate(&t, Kind::Regex, "a", "a", d).unwrap_err().contains("Java was not found"), "{script}");
        }
    }

    #[test]
    fn every_kind_has_a_word_and_a_label() {
        let words: std::collections::HashSet<_> = Kind::ALL.iter().map(|k| k.word()).collect();
        assert_eq!(words.len(), 4);
        assert!(Kind::ALL.iter().all(|k| !k.label().is_empty()));
    }

    #[test]
    fn the_program_reads_only_its_input_and_fetches_nothing() {
        // The checks the program makes for itself, read from its text.
        assert!(SOURCE.contains("disallow-doctype-decl"));
        assert!(!SOURCE.contains("Runtime.getRuntime"));
        assert!(!SOURCE.contains("ProcessBuilder"));
        assert!(!SOURCE.contains("URL("));
        assert!(!SOURCE.contains("Socket"));
    }
}
