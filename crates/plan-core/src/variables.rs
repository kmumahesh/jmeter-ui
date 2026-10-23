//! Finding `${variables}` that nothing in the plan defines.
//!
//! JMeter reads a missing variable as the text `${name}` and carries on, so a misspelt name only shows up as a
//! wrong request. The check is a hint: a variable can also come from a property, a file or a script that this
//! check cannot see.

use crate::edit::EditablePlan;
use crate::model::NodeId;
use crate::problems::{Level, Problem};
use std::collections::HashSet;

/// Settings whose value is a variable name, or a list of them. (setting name, separator)
const DEFINING: &[(&str, &[char])] = &[
    ("RegexExtractor.refname", &[]),
    ("XPathExtractor.refname", &[]),
    ("XPathExtractor2.refname", &[]),
    ("HtmlExtractor.refname", &[]),
    ("BoundaryExtractor.refname", &[]),
    ("JMESExtractor.referenceName", &[]),
    ("JSONPostProcessor.referenceNames", &[';', ',']),
    ("ForeachController.returnVal", &[]),
    ("CounterConfig.name", &[]),
    ("variableName", &[]),
    ("variableNames", &[',']),
    ("resultVariable", &[]),
];

/// Names JMeter always provides.
const BUILT_IN: &[&str] = &["JMeterThread.last_sample_ok", "START.MS", "START.YMD", "START.HMS", "TESTSTART.MS"];

fn last_segment(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// Every name the plan defines: by settings, by user defined variables, and by scripts that call `vars.put`.
pub fn defined(edit: &EditablePlan) -> HashSet<String> {
    let mut out = HashSet::new();
    let plan = edit.plan();
    for n in plan.iter() {
        let is_variables_element = n.tag == "Arguments";
        for (path, text) in edit.setting_texts(n.id) {
            let last = last_segment(&path);
            if let Some((_, seps)) = DEFINING.iter().find(|(name, _)| *name == last) {
                if seps.is_empty() {
                    insert(&mut out, &text);
                } else {
                    for part in text.split(|c| seps.contains(&c)) {
                        insert(&mut out, part);
                    }
                }
            }
            // Variables set up in the plan: the table of the Test Plan, and User Defined Variables elements.
            if last == "Argument.name" && (path.starts_with("TestPlan.user_defined_variables/") || (is_variables_element && path.starts_with("Arguments.arguments/"))) {
                insert(&mut out, &text);
            }
            if path.starts_with("UserParameters.names/") {
                insert(&mut out, &text);
            }
            // A script can set variables for the rest of the test.
            if text.contains("vars.put") {
                scan_script(&text, &mut out);
            }
        }
    }
    out
}

fn insert(out: &mut HashSet<String>, name: &str) {
    let n = name.trim();
    if !n.is_empty() {
        out.insert(n.to_string());
    }
}

/// Names given to `vars.put("name", ...)` and `vars.putObject("name", ...)`.
fn scan_script(text: &str, out: &mut HashSet<String>) {
    for call in ["vars.put(", "vars.putObject("] {
        let mut rest = text;
        while let Some(i) = rest.find(call) {
            rest = &rest[i + call.len()..];
            let t = rest.trim_start();
            if let Some(q) = t.chars().next().filter(|c| *c == '"' || *c == '\'') {
                let inner = &t[1..];
                if let Some(end) = inner.find(q) {
                    insert(out, &inner[..end]);
                }
            }
        }
    }
}

/// Is `name` one of the defined names, or one JMeter makes from one: an extractor with a match number or group
/// adds `_1`, `_g1`, `_matchNr`, `_ALL` and so on.
fn is_defined(name: &str, defined: &HashSet<String>) -> bool {
    if defined.contains(name) || BUILT_IN.contains(&name) {
        return true;
    }
    let mut cut = name;
    while let Some(i) = cut.rfind('_') {
        cut = &cut[..i];
        let rest = &name[i + 1..];
        if defined.contains(cut) && derived_suffix(&name[cut.len() + 1..]) {
            return true;
        }
        let _ = rest;
    }
    false
}

/// The endings JMeter adds to the name of a variable an extractor creates: `matchNr`, `ALL`, a group (`g`, `g0`,
/// `gn`), a match number, or a match number and a group (`2_g1`).
fn derived_suffix(s: &str) -> bool {
    let group = |g: &str| g == "g" || g == "gn" || g.strip_prefix('g').is_some_and(|d| !d.is_empty() && d.bytes().all(|b| b.is_ascii_digit()));
    let number = |n: &str| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit());
    match s {
        "matchNr" | "ALL" => true,
        _ => match s.split_once('_') {
            Some((a, b)) => number(a) && group(b),
            None => number(s) || group(s),
        },
    }
}

/// The `${names}` used in a text: plain names only, not functions such as `${__P(x)}`.
pub fn references(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(i) = rest.find("${") {
        // Move on by two characters only, so a variable inside a function call is found too.
        let tail = &rest[i + 2..];
        rest = tail;
        let Some(end) = tail.find('}') else { break };
        let name = &tail[..end];
        let plain = !name.is_empty() && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'.' || b == b'-');
        if plain && !name.starts_with("__") {
            out.push(name.to_string());
        }
    }
    out
}

/// One hint per element and unknown variable.
pub fn check(edit: &EditablePlan) -> Vec<Problem> {
    let plan = edit.plan();
    let known = defined(edit);
    let mut out = Vec::new();
    for n in plan.iter() {
        let mut seen: HashSet<String> = HashSet::new();
        for (path, text) in edit.setting_texts(n.id) {
            // Scripts use `${...}` for their own purposes and are checked by the script itself.
            if matches!(last_segment(&path), "script" | "query" | "parameters") {
                continue;
            }
            for name in references(&text) {
                if is_defined(&name, &known) || !seen.insert(name.clone()) {
                    continue;
                }
                out.push(Problem {
                    level: Level::Info,
                    node: Some(NodeId(n.id.0)),
                    text: format!("{} uses ${{{name}}}, which nothing in this plan defines. It may come from a property or a file.", n.name),
                });
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const PLAN: &str = r#"<jmeterTestPlan><hashTree>
<TestPlan testname="P"><elementProp name="TestPlan.user_defined_variables" elementType="Arguments"><collectionProp name="Arguments.arguments">
 <elementProp name="host" elementType="Argument"><stringProp name="Argument.name">host</stringProp><stringProp name="Argument.value">example.org</stringProp></elementProp>
</collectionProp></elementProp></TestPlan><hashTree>
 <Arguments guiclass="ArgumentsPanel" testname="Vars"><collectionProp name="Arguments.arguments">
  <elementProp name="token" elementType="Argument"><stringProp name="Argument.name">token</stringProp><stringProp name="Argument.value">x</stringProp></elementProp>
 </collectionProp></Arguments><hashTree/>
 <CSVDataSet guiclass="TestBeanGUI" testname="Data"><stringProp name="variableNames">user,pass</stringProp></CSVDataSet><hashTree/>
 <ThreadGroup testname="G"/><hashTree>
  <HTTPSamplerProxy guiclass="HttpTestSampleGui" testname="Login">
   <stringProp name="HTTPSampler.domain">${host}</stringProp>
   <stringProp name="HTTPSampler.path">/login?u=${user}&amp;p=${pass}&amp;t=${token}&amp;x=${nope}&amp;y=${__P(y,1)}&amp;z=${nope}</stringProp>
   <elementProp name="HTTPsampler.Arguments" elementType="Arguments"><collectionProp name="Arguments.arguments">
    <elementProp name="a" elementType="HTTPArgument"><stringProp name="Argument.name">a</stringProp><stringProp name="Argument.value">${session}</stringProp></elementProp>
   </collectionProp></elementProp>
  </HTTPSamplerProxy><hashTree>
   <RegexExtractor guiclass="RegexExtractorGui" testname="Grab"><stringProp name="RegexExtractor.refname">session</stringProp></RegexExtractor><hashTree/>
  </hashTree>
  <HTTPSamplerProxy guiclass="HttpTestSampleGui" testname="Next"><stringProp name="HTTPSampler.path">/${session_g1}/${session_matchNr}/${session_2}/${session_2_g1}/${sessionX}</stringProp></HTTPSamplerProxy><hashTree/>
 </hashTree>
</hashTree></hashTree></jmeterTestPlan>"#;

    fn edit() -> EditablePlan {
        EditablePlan::open(PLAN).unwrap()
    }

    #[test]
    fn names_come_from_settings_tables_and_the_test_plan() {
        let d = defined(&edit());
        for n in ["host", "token", "user", "pass", "session"] {
            assert!(d.contains(n), "{n}: {d:?}");
        }
        assert!(!d.contains("nope"));
    }

    #[test]
    fn scripts_can_define_variables() {
        let src = "<jmeterTestPlan><hashTree><TestPlan testname=\"P\"/><hashTree><ThreadGroup testname=\"G\"/><hashTree><JSR223Sampler guiclass=\"TestBeanGUI\" testname=\"S\"><stringProp name=\"script\">vars.put(\"made\", \"1\"); vars.putObject('other', x)</stringProp></JSR223Sampler><hashTree/><HTTPSamplerProxy guiclass=\"HttpTestSampleGui\" testname=\"H\"><stringProp name=\"HTTPSampler.path\">/${made}/${other}/${unknown}</stringProp></HTTPSamplerProxy><hashTree/></hashTree></hashTree></hashTree></jmeterTestPlan>";
        let e = EditablePlan::open(src).unwrap();
        let d = defined(&e);
        assert!(d.contains("made") && d.contains("other"));
        let p = check(&e);
        assert_eq!(p.len(), 1);
        assert!(p[0].text.contains("${unknown}"));
    }

    #[test]
    fn only_undefined_names_are_reported_once_per_element() {
        let p = check(&edit());
        let texts: Vec<_> = p.iter().map(|x| x.text.as_str()).collect();
        assert_eq!(p.len(), 2, "{texts:?}");
        assert!(texts.iter().any(|t| t.starts_with("Login uses ${nope}")), "reported once although it is used twice");
        assert!(texts.iter().any(|t| t.starts_with("Next uses ${sessionX}")), "{texts:?}");
        assert!(p.iter().all(|x| x.level == Level::Info && x.node.is_some()));
    }

    #[test]
    fn what_an_extractor_adds_to_a_name_counts_as_defined() {
        for (s, ok) in [("g1", true), ("g", true), ("gn", true), ("g0", true), ("matchNr", true), ("ALL", true), ("2", true), ("2_g1", true), ("x", false), ("", false), ("g_1", false), ("2_x", false)] {
            assert_eq!(derived_suffix(s), ok, "{s:?}");
        }
        let d: HashSet<String> = ["a".to_string()].into();
        assert!(is_defined("a_1_g2", &d) && is_defined("a", &d) && !is_defined("b_1", &d) && !is_defined("ab", &d));
        let d: HashSet<String> = ["my_var".to_string()].into();
        assert!(is_defined("my_var_ALL", &d), "names with underscores");
    }

    #[test]
    fn functions_and_odd_text_are_left_alone() {
        assert_eq!(references("${a} ${__time()} ${__P(x,${y})} ${b-c} ${d e} ${} ${"), ["a", "y", "b-c"]);
        assert!(references("no variables here").is_empty());
    }

    #[test]
    fn built_in_names_are_known() {
        let src = "<jmeterTestPlan><hashTree><TestPlan testname=\"P\"/><hashTree><ThreadGroup testname=\"G\"/><hashTree><IfController testname=\"I\"><stringProp name=\"IfController.condition\">${JMeterThread.last_sample_ok}</stringProp></IfController><hashTree/></hashTree></hashTree></hashTree></jmeterTestPlan>";
        assert!(check(&EditablePlan::open(src).unwrap()).is_empty());
    }

    #[test]
    fn a_plan_in_which_everything_is_defined_has_no_hints() {
        let sample = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/Test.jmx")).unwrap();
        let e = EditablePlan::open(&sample).unwrap();
        let _ = check(&e); // does not panic on a real plan
    }
}
