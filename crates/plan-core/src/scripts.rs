//! Finding the places where a plan can run code, so the user is asked before running it for the first time.
//!
//! A plan can hold script elements (JSR223, BeanShell, BSF), an OS Process sampler, and script functions
//! such as `${__groovy(...)}`. All of these run with the user's own rights when the plan runs.

use crate::model::{NodeId, Plan};

/// One place where the plan can run code.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScriptUse {
    /// The element, or `None` for a function written inside a setting.
    pub node: Option<NodeId>,
    /// The element's name, or the function.
    pub name: String,
    /// What kind of code it can run.
    pub kind: String,
}

/// Functions that run code.
const FUNCTIONS: [&str; 6] = ["__groovy", "__beanshell", "__javascript", "__jexl2", "__jexl3", "__bsh"];

fn kind_of(tag: &str) -> Option<&'static str> {
    if tag.starts_with("JSR223") {
        Some("JSR223 script")
    } else if tag.starts_with("BeanShell") {
        Some("BeanShell script")
    } else if tag.starts_with("BSF") {
        Some("BSF script")
    } else if tag == "SystemSampler" {
        Some("OS Process (runs a program)")
    } else {
        None
    }
}

/// Every script element in the plan, and every script function found in the text of the file.
pub fn find(plan: &Plan, text: &str) -> Vec<ScriptUse> {
    let mut out: Vec<ScriptUse> = plan
        .iter()
        .filter_map(|n| kind_of(&n.tag).map(|k| ScriptUse { node: Some(n.id), name: n.name.clone(), kind: k.into() }))
        .collect();
    let lower = text.to_lowercase();
    for f in FUNCTIONS {
        // A call looks like `${__groovy(` or `${__groovy (`; also `__groovy(` inside another function.
        if lower.contains(&format!("{f}(")) || lower.contains(&format!("{f} (")) {
            out.push(ScriptUse { node: None, name: format!("${{{f}(...)}}"), kind: "script function".into() });
        }
    }
    out
}

/// A short sentence for the question before running.
pub fn describe(uses: &[ScriptUse]) -> String {
    let mut kinds: Vec<&str> = Vec::new();
    for u in uses {
        if !kinds.contains(&u.kind.as_str()) {
            kinds.push(&u.kind);
        }
    }
    let list = kinds.join(", ");
    format!("{} place{} can run code on this computer ({list}).", uses.len(), if uses.len() == 1 { "" } else { "s" })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::EditablePlan;

    fn plan_with(extra: &str) -> (Plan, String) {
        let text = format!(
            "<jmeterTestPlan><hashTree><TestPlan guiclass=\"TestPlanGui\" testclass=\"TestPlan\" testname=\"P\"/><hashTree><ThreadGroup guiclass=\"ThreadGroupGui\" testclass=\"ThreadGroup\" testname=\"G\"/><hashTree>{extra}</hashTree></hashTree></hashTree></jmeterTestPlan>"
        );
        (EditablePlan::open(&text).unwrap().plan().as_ref().clone(), text)
    }

    #[test]
    fn a_plain_plan_has_no_scripts() {
        let (p, t) = plan_with("<HTTPSamplerProxy guiclass=\"HttpTestSampleGui\" testclass=\"HTTPSamplerProxy\" testname=\"h\"/><hashTree/>");
        assert!(find(&p, &t).is_empty());
    }

    #[test]
    fn script_elements_are_found_by_kind() {
        let (p, t) = plan_with(
            "<JSR223Sampler guiclass=\"TestBeanGUI\" testclass=\"JSR223Sampler\" testname=\"js\"/><hashTree/>\
             <BeanShellPreProcessor guiclass=\"TestBeanGUI\" testclass=\"BeanShellPreProcessor\" testname=\"bs\"/><hashTree/>\
             <SystemSampler guiclass=\"SystemSamplerGui\" testclass=\"SystemSampler\" testname=\"os\"/><hashTree/>\
             <BSFAssertion guiclass=\"TestBeanGUI\" testclass=\"BSFAssertion\" testname=\"bsf\"/><hashTree/>\
             <JSR223PostProcessor guiclass=\"TestBeanGUI\" testclass=\"JSR223PostProcessor\" testname=\"pp\"/><hashTree/>",
        );
        let found = find(&p, &t);
        let names: Vec<_> = found.iter().map(|u| u.name.as_str()).collect();
        assert_eq!(names, ["js", "bs", "os", "bsf", "pp"]);
        assert!(found.iter().all(|u| u.node.is_some()));
        assert_eq!(found[2].kind, "OS Process (runs a program)");
    }

    #[test]
    fn script_functions_are_found_wherever_they_are_written() {
        for f in ["${__groovy(1+1)}", "${__BeanShell(x)}", "${__javaScript(1)}", "${__jexl3(2)}", "${__groovy (1)}", "${__time(,)}__GROOVY(1)"] {
            let (p, t) = plan_with(&format!("<HTTPSamplerProxy guiclass=\"HttpTestSampleGui\" testclass=\"HTTPSamplerProxy\" testname=\"h\"><stringProp name=\"HTTPSampler.path\">/x?v={f}</stringProp></HTTPSamplerProxy><hashTree/>"));
            let found = find(&p, &t);
            assert_eq!(found.len(), 1, "{f}: {found:?}");
            assert!(found[0].node.is_none() && found[0].kind == "script function");
        }
        let (p, t) = plan_with("<HTTPSamplerProxy guiclass=\"HttpTestSampleGui\" testclass=\"HTTPSamplerProxy\" testname=\"h\"><stringProp name=\"HTTPSampler.path\">/x?v=${__time(,)}${__Random(1,2)}</stringProp></HTTPSamplerProxy><hashTree/>");
        assert!(find(&p, &t).is_empty(), "harmless functions are not scripts");
    }

    #[test]
    fn the_question_names_how_many_places_and_what_kinds() {
        let (p, t) = plan_with("<JSR223Sampler guiclass=\"TestBeanGUI\" testclass=\"JSR223Sampler\" testname=\"js\"/><hashTree/><SystemSampler guiclass=\"SystemSamplerGui\" testclass=\"SystemSampler\" testname=\"os\"/><hashTree/>");
        let d = describe(&find(&p, &t));
        assert!(d.starts_with("2 places") && d.contains("JSR223 script") && d.contains("OS Process"), "{d}");
        assert!(describe(&[ScriptUse { node: None, name: "x".into(), kind: "k".into() }]).starts_with("1 place can"));
    }

    #[test]
    fn every_corpus_plan_is_scanned_without_trouble() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures");
        for e in std::fs::read_dir(dir).unwrap().flatten() {
            if e.path().extension().is_some_and(|x| x == "jmx") {
                let text = std::fs::read_to_string(e.path()).unwrap();
                let plan = Plan::from_jmx_str(&text).unwrap();
                let _ = find(&plan, &text);
            }
        }
    }
}
