//! Trying expressions with the real JMeter libraries: what each kind finds, what is said when an expression or the
//! text is wrong, and what cannot go wrong however hostile either is. Tests that need Java and JMeter skip themselves
//! when they are not installed.

mod common;

use common::*;
use jvm_runtime::tester::{self, Found, Kind, Tools};
use std::time::{Duration, Instant};

const T: Duration = Duration::from_secs(60);

/// Tests may run together, except the one that counts the folders left behind, which needs the others to be idle.
static BUSY: std::sync::RwLock<()> = std::sync::RwLock::new(());

fn try_it(env: &Env, kind: Kind, expr: &str, sample: &str) -> Result<Found, String> {
    let _together = BUSY.read().unwrap_or_else(|e| e.into_inner());
    tester::evaluate(&Tools { java: &env.java.path, jmeter_home: &env.home }, kind, expr, sample, T)
}

fn items(env: &Env, kind: Kind, expr: &str, sample: &str) -> Vec<String> {
    try_it(env, kind, expr, sample).unwrap_or_else(|e| panic!("{kind:?} {expr:?}: {e}")).items
}

#[test]
fn json_path_finds_values_lists_and_nothing() {
    let Some(env) = env() else { return };
    let json = r#"{"store":{"book":[{"title":"A","price":8.5},{"title":"B","price":12}],"open":true},"name":"café 😀"}"#;
    assert_eq!(items(&env, Kind::JsonPath, "$.store.book[*].title", json), ["A", "B"]);
    assert_eq!(items(&env, Kind::JsonPath, "$.store.open", json), ["true"]);
    assert_eq!(items(&env, Kind::JsonPath, "$.store.book[0].price", json), ["8.5"]);
    assert_eq!(items(&env, Kind::JsonPath, "$.name", json), ["caf\u{e9} \u{1F600}"], "text that is not English comes through whole");
    assert_eq!(items(&env, Kind::JsonPath, "$.store.book[?(@.price > 10)].title", json), ["B"]);
    assert!(items(&env, Kind::JsonPath, "$.nothing.here", json).is_empty(), "no match is an empty answer, not an error");
    let obj = items(&env, Kind::JsonPath, "$.store.book[0]", json);
    assert!(obj[0].contains("\"title\":\"A\""), "{obj:?}");
}

#[test]
fn json_path_says_what_is_wrong() {
    let Some(env) = env() else { return };
    let e = try_it(&env, Kind::JsonPath, "not a path", r#"{"a":1}"#).unwrap_err();
    assert!(!e.is_empty(), "{e}");
    assert!(try_it(&env, Kind::JsonPath, "$.a[?(@.x", r#"{"a":1}"#).is_err());
    for text in ["this is not json", "", "42", "\"just a string\""] {
        let e = try_it(&env, Kind::JsonPath, "$.a", text).unwrap_err();
        assert!(!e.is_empty(), "{text:?}");
    }
    let e = try_it(&env, Kind::JsonPath, "$.a", "42").unwrap_err();
    assert!(e.contains("not a JSON object or array"), "{e}");
}

#[test]
fn a_regular_expression_shows_each_match_with_its_groups() {
    let Some(env) = env() else { return };
    let text = "id=17 name=ann\nid=42 name=bo";
    let found = items(&env, Kind::Regex, r"id=(\d+) name=(\w+)", text);
    assert_eq!(found.len(), 2);
    assert!(found[0].starts_with("Match 1: id=17 name=ann") && found[0].contains("group 1: 17") && found[0].contains("group 2: ann"), "{found:?}");
    assert!(found[1].contains("group 1: 42"), "{found:?}");
    assert!(items(&env, Kind::Regex, "zzz", text).is_empty());
    assert_eq!(items(&env, Kind::Regex, r"café", "un caf\u{e9}").len(), 1, "text that is not English is matched");
    assert!(try_it(&env, Kind::Regex, "(unclosed", text).unwrap_err().to_lowercase().contains("unclosed"));
}

#[test]
fn xpath_finds_nodes_and_values() {
    let Some(env) = env() else { return };
    let xml = "<shop><item id=\"1\">Pen</item><item id=\"2\">Ink &amp; paper</item></shop>";
    assert_eq!(items(&env, Kind::XPath, "//item", xml), ["Pen", "Ink & paper"]);
    assert_eq!(items(&env, Kind::XPath, "//item[@id='2']/@id", xml), ["2"]);
    assert_eq!(items(&env, Kind::XPath, "count(//item)", xml), ["2"], "a number, not nodes");
    assert!(items(&env, Kind::XPath, "//nothing", xml).is_empty());
    assert!(try_it(&env, Kind::XPath, "//item", "<shop><item></shop>").unwrap_err().contains("not well-formed"));
    assert!(try_it(&env, Kind::XPath, "//[", xml).is_err());
}

#[test]
fn a_css_selector_finds_elements_in_html_that_is_not_tidy() {
    let Some(env) = env() else { return };
    let html = "<html><body><p class=a>one<p class=a>two<div id=x><a href='/go'>go</a></div>";
    assert_eq!(items(&env, Kind::Css, "p.a", html).len(), 2);
    let a = items(&env, Kind::Css, "#x a", html);
    assert_eq!(a.len(), 1);
    assert!(a[0].contains("href=\"/go\""), "{a:?}");
    assert!(items(&env, Kind::Css, "table", html).is_empty());
    assert!(try_it(&env, Kind::Css, "p[", html).is_err());
}

// ---- what must not go wrong ----

#[test]
fn a_document_cannot_read_files_or_fetch_anything() {
    let Some(env) = env() else { return };
    let dir = temp("tester-xxe");
    let secret = dir.join("secret.txt");
    std::fs::write(&secret, "TOP-SECRET-CONTENT").unwrap();
    let uri = format!("file:///{}", secret.to_string_lossy().replace('\\', "/"));
    let server = Server::start();
    for system in [uri, format!("http://127.0.0.1:{}/xxe", server.port)] {
        let xml = format!("<?xml version=\"1.0\"?><!DOCTYPE r [<!ENTITY e SYSTEM \"{system}\">]><r>&e;</r>");
        let result = try_it(&env, Kind::XPath, "/r", &xml);
        let shown = format!("{result:?}");
        assert!(!shown.contains("TOP-SECRET-CONTENT"), "the file was read: {shown}");
        assert!(result.is_err(), "a DOCTYPE is refused: {shown}");
    }
    assert!(server.paths.lock().unwrap().is_empty(), "nothing was fetched: {:?}", server.paths.lock().unwrap());
    // The same with the entity bomb.
    let bomb = "<?xml version=\"1.0\"?><!DOCTYPE l [<!ENTITY a \"aaaaaaaaaa\"><!ENTITY b \"&a;&a;&a;&a;&a;&a;&a;&a;&a;&a;\"><!ENTITY c \"&b;&b;&b;&b;&b;&b;&b;&b;&b;&b;\">]><l>&c;&c;&c;&c;&c;</l>";
    let t = Instant::now();
    assert!(try_it(&env, Kind::XPath, "/l", bomb).is_err());
    assert!(t.elapsed() < Duration::from_secs(30));
}

#[test]
fn a_regular_expression_that_never_finishes_is_stopped_and_the_tester_still_works() {
    let Some(env) = env() else { return };
    let _together = BUSY.read().unwrap_or_else(|e| e.into_inner());
    let tools = Tools { java: &env.java.path, jmeter_home: &env.home };
    // Starting Java alone takes longer than this, so the time limit always applies.
    let t = Instant::now();
    let e = tester::evaluate(&tools, Kind::Regex, "a", "a", Duration::from_millis(100)).unwrap_err();
    assert!(e.contains("Stopped after 0.1 seconds"), "{e}");
    assert!(t.elapsed() < Duration::from_secs(10), "{:?}", t.elapsed());
    // An expression that takes very long on a text is stopped the same way.
    let nasty = "x".repeat(900_000);
    let t = Instant::now();
    let r = tester::evaluate(&tools, Kind::Regex, "(x+x+)+z", &nasty, Duration::from_secs(8));
    assert!(t.elapsed() < Duration::from_secs(25), "{:?}", t.elapsed());
    if let Err(e) = &r {
        assert!(!e.is_empty());
    }
    assert_eq!(tester::evaluate(&tools, Kind::Regex, "a", "a", T).unwrap().items.len(), 1, "and it still works afterwards");
}

#[test]
fn hostile_expressions_and_texts_are_only_text() {
    let Some(env) = env() else { return };
    // Nothing in them reaches a command line: they are data on the standard input.
    let odd = "\"; calc & echo \u{0} \n\r\t `$(whoami)` %PATH% \u{1F600} </r>";
    let plain = odd.replace('\u{0}', "");
    let f = try_it(&env, Kind::Regex, &regex_escape(&plain), &plain).unwrap();
    assert_eq!(f.items.len(), 1);
    assert_eq!(try_it(&env, Kind::Regex, "calc", odd).unwrap().items.len(), 1, "a NUL in the text is only a character");
    assert!(try_it(&env, Kind::JsonPath, odd, "{}").is_err());
    // A flood of matches is cut.
    let many = "x".repeat(5000);
    let f = try_it(&env, Kind::Regex, "x", &many).unwrap();
    assert_eq!(f.items.len(), 200);
    assert!(f.cut);
    // A very long match is cut.
    let f = try_it(&env, Kind::Regex, "x+", &many).unwrap();
    assert!(f.items[0].chars().count() <= 2100 && f.cut);
    // The answer is whole text, whatever it is made of.
    let f = try_it(&env, Kind::Regex, r"[\s\S]+", "line1\nline2\t\"q\" \\ \u{1F600}").unwrap();
    assert!(f.items[0].contains("line1\nline2\t\"q\" \\ \u{1F600}"), "{:?}", f.items);
}

fn regex_escape(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        if c == '\u{0}' {
            continue;
        }
        if "\\.+*?()|[]{}^$".contains(c) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

fn count_tester_dirs() -> usize {
    let prefix = format!("jmeter-ui-tester-{}-", std::process::id());
    std::fs::read_dir(std::env::temp_dir()).unwrap().flatten().filter(|e| e.file_name().to_string_lossy().starts_with(&prefix)).count()
}

#[test]
fn nothing_is_left_behind_and_it_is_quick_enough_to_use() {
    let Some(env) = env() else { return };
    let _alone = BUSY.write().unwrap_or_else(|e| e.into_inner());
    let tools = Tools { java: &env.java.path, jmeter_home: &env.home };
    let t = Instant::now();
    assert_eq!(tester::evaluate(&tools, Kind::JsonPath, "$.a", r#"{"a":"v"}"#, T).unwrap().items, ["v"]);
    let first = t.elapsed();
    let t = Instant::now();
    assert_eq!(tester::evaluate(&tools, Kind::JsonPath, "$.a", r#"{"a":"w"}"#, T).unwrap().items, ["w"]);
    eprintln!("an expression test: first {first:?}, next {:?}", t.elapsed());
    assert!(first < Duration::from_secs(20) && t.elapsed() < Duration::from_secs(20));
    assert_eq!(count_tester_dirs(), 0, "the folder made for the test is removed");
}

#[test]
fn many_tests_at_once_do_not_get_in_each_others_way() {
    let Some(env) = env() else { return };
    let env = std::sync::Arc::new(env);
    let handles: Vec<_> = (0..4)
        .map(|i| {
            let env = env.clone();
            std::thread::spawn(move || {
                let sample = format!("{{\"n\":{i}}}");
                items(&env, Kind::JsonPath, "$.n", &sample)
            })
        })
        .collect();
    let mut got: Vec<String> = handles.into_iter().flat_map(|h| h.join().unwrap()).collect();
    got.sort();
    assert_eq!(got, ["0", "1", "2", "3"]);
}
