//! Properties for a run: `name=value` lines that are given to JMeter as `-J` options, so a plan can read them with
//! `${__P(name)}`. The text may come from a person typing or from a `.properties` file, so it is checked before it is
//! used: every name and value, how many there are, and the names the app sets itself.

use crate::run::valid_property_name;

/// The most properties for one plan.
pub const MAX_PROPERTIES: usize = 100;
/// The longest value, in bytes.
pub const MAX_VALUE_BYTES: usize = 1000;
/// The longest text of properties, in bytes.
pub const MAX_TEXT_BYTES: usize = 64 * 1024;

/// Names the app sets for every run. Setting them from here could break the run or its results.
pub const RESERVED: &[&str] = &[
    "summariser.interval",
    "summariser.name",
    "jmeter.save.saveservice.autoflush",
    "jmeterengine.nongui.port",
    "jmeterengine.nongui.maxport",
];

fn reserved(name: &str) -> bool {
    RESERVED.contains(&name) || name.starts_with("jmeterui.")
}

/// Read properties from text: one `name=value` (or `name: value`) on each line; empty lines and lines starting with `#`
/// or `!` are skipped. The first problem is returned as a sentence naming the line.
pub fn parse(text: &str) -> Result<Vec<(String, String)>, String> {
    if text.len() > MAX_TEXT_BYTES {
        return Err(format!("The properties are longer than {} KB.", MAX_TEXT_BYTES / 1024));
    }
    let mut out: Vec<(String, String)> = Vec::new();
    for (i, raw) in text.lines().enumerate() {
        let n = i + 1;
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with('!') {
            continue;
        }
        let Some(at) = line.find(['=', ':']) else {
            return Err(format!("Line {n}: write it as name=value."));
        };
        let (name, value) = (line[..at].trim(), line[at + 1..].trim());
        if name.is_empty() {
            return Err(format!("Line {n}: there is no name before the {}.", &line[at..at + 1]));
        }
        if !valid_property_name(name) || name.starts_with(['-', '.']) {
            return Err(format!("Line {n}: {name:?} is not a usable name. Use letters, digits, dots, dashes and underscores."));
        }
        if reserved(name) {
            return Err(format!("Line {n}: {name} is set by the app and cannot be changed here."));
        }
        if value.len() > MAX_VALUE_BYTES {
            return Err(format!("Line {n}: the value of {name} is longer than {MAX_VALUE_BYTES} characters."));
        }
        if value.chars().any(|c| c.is_control()) {
            return Err(format!("Line {n}: the value of {name} has a control character."));
        }
        if out.iter().any(|(k, _)| k == name) {
            return Err(format!("Line {n}: {name} is already set above."));
        }
        if out.len() >= MAX_PROPERTIES {
            return Err(format!("More than {MAX_PROPERTIES} properties."));
        }
        out.push((name.to_string(), value.to_string()));
    }
    Ok(out)
}

/// A sentence for the run header: how many properties were set and their names (never the values, which may be secrets).
pub fn note(props: &[(String, String)]) -> Option<String> {
    if props.is_empty() {
        return None;
    }
    let names: Vec<&str> = props.iter().take(8).map(|(k, _)| k.as_str()).collect();
    let more = props.len().saturating_sub(names.len());
    Some(format!("{} propert{} set for this run: {}{}", props.len(), if props.len() == 1 { "y" } else { "ies" }, names.join(", "), if more > 0 { format!(" and {more} more") } else { String::new() }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pairs(v: &[(&str, &str)]) -> Vec<(String, String)> {
        v.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect()
    }

    #[test]
    fn lines_of_name_and_value_are_read_and_the_rest_is_skipped() {
        let text = "# the server\nhost = shop.example.com\n\n! another comment\nport: 8443\nname.with-dash_and.dots=v\nempty=\nurl=http://x/y?a=b&c=d\n  indented = yes  \n";
        assert_eq!(parse(text).unwrap(), pairs(&[("host", "shop.example.com"), ("port", "8443"), ("name.with-dash_and.dots", "v"), ("empty", ""), ("url", "http://x/y?a=b&c=d"), ("indented", "yes")]));
        assert_eq!(parse("").unwrap(), pairs(&[]));
        assert_eq!(parse("# only a comment").unwrap(), pairs(&[]));
        assert_eq!(parse("a=1\r\nb=2\r\n").unwrap(), pairs(&[("a", "1"), ("b", "2")]));
    }

    #[test]
    fn every_kind_of_wrong_line_says_which_line_and_why() {
        let cases: &[(&str, &str)] = &[
            ("a=1\nnonsense", "Line 2: write it as name=value"),
            ("=value", "Line 1: there is no name"),
            ("bad name=1", "Line 1:"),
            ("-Dx=1", "Line 1:"),
            ("a;b=1", "Line 1:"),
            ("a\u{0}b=1", "Line 1:"),
            ("summariser.interval=5", "set by the app"),
            ("jmeter.save.saveservice.autoflush=false", "set by the app"),
            ("jmeterengine.nongui.port=1", "set by the app"),
            ("jmeterui.details=all", "set by the app"),
            ("a=1\na=2", "Line 2: a is already set above"),
        ];
        for (text, want) in cases {
            let e = parse(text).unwrap_err();
            assert!(e.contains(want), "{text:?} gave {e}");
        }
    }

    #[test]
    fn values_are_limited_and_cannot_hold_control_characters() {
        assert!(parse(&format!("a={}", "x".repeat(MAX_VALUE_BYTES))).is_ok());
        assert!(parse(&format!("a={}", "x".repeat(MAX_VALUE_BYTES + 1))).unwrap_err().contains("longer than"));
        assert!(parse("a=one\u{7}two").unwrap_err().contains("control character"));
        assert!(parse("a=caf\u{e9} \u{1F600}").is_ok(), "text that is not English is a value like any other");
        let many: String = (0..=MAX_PROPERTIES).map(|i| format!("p{i}=1\n")).collect();
        assert!(parse(&many).unwrap_err().contains("More than"));
        let fine: String = (0..MAX_PROPERTIES).map(|i| format!("p{i}=1\n")).collect();
        assert_eq!(parse(&fine).unwrap().len(), MAX_PROPERTIES);
        assert!(parse(&"a=1\n".repeat(MAX_TEXT_BYTES)).unwrap_err().contains("longer than"));
    }

    #[test]
    fn the_note_names_the_properties_and_never_shows_values() {
        assert_eq!(note(&[]), None);
        let n = note(&pairs(&[("password", "hunter2")])).unwrap();
        assert!(n.contains("password") && !n.contains("hunter2"), "{n}");
        assert!(n.starts_with("1 property set"));
        let many: Vec<(String, String)> = (0..20).map(|i| (format!("p{i}"), "secret".into())).collect();
        let n = note(&many).unwrap();
        assert!(n.contains("and 12 more") && !n.contains("secret"), "{n}");
    }
}
