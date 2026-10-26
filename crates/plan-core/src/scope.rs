//! Which elements apply to which, and in what order JMeter runs them.
//!
//! Everything that is not a sampler or a controller applies to the samplers in its scope: the samplers that
//! sit under the element's parent (including those inside sibling controllers), or just the sampler it is
//! placed under. These rules follow `TestCompiler.saveSamplerConfigs` in JMeter, which decides the order.

use crate::catalog::Category;
use crate::model::{NodeId, Plan};

/// The steps of running one sampler, in the order JMeter takes them.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Phase {
    /// Settings merged into the sampler before it runs. The nearest element wins where two set the same thing.
    Config,
    PreProcessor,
    /// All timers apply and their delays add up.
    Timer,
    Sampler,
    PostProcessor,
    Assertion,
    Listener,
}

impl Phase {
    pub fn label(self) -> &'static str {
        match self {
            Phase::Config => "Configuration",
            Phase::PreProcessor => "Pre-processors",
            Phase::Timer => "Timers",
            Phase::Sampler => "The sampler",
            Phase::PostProcessor => "Post-processors",
            Phase::Assertion => "Assertions",
            Phase::Listener => "Listeners",
        }
    }

    /// A short note on how the elements of this step combine.
    pub fn note(self) -> &'static str {
        match self {
            Phase::Config => "the nearest wins",
            Phase::PreProcessor => "outer first",
            Phase::Timer => "delays add up",
            Phase::Sampler => "",
            Phase::PostProcessor => "outer first",
            Phase::Assertion => "outer first",
            Phase::Listener => "inner first",
        }
    }

    fn of(category: Category) -> Option<Phase> {
        Some(match category {
            Category::Config => Phase::Config,
            Category::PreProcessor => Phase::PreProcessor,
            Category::Timer => Phase::Timer,
            Category::Sampler => Phase::Sampler,
            Category::PostProcessor => Phase::PostProcessor,
            Category::Assertion => Phase::Assertion,
            Category::Listener => Phase::Listener,
            _ => return None,
        })
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub node: NodeId,
    pub phase: Phase,
    /// The element it sits under, which gives it its reach.
    pub from: NodeId,
    /// Whether it takes part: it and everything above it are switched on.
    pub active: bool,
}

/// Is `id` and everything above it switched on?
fn active(plan: &Plan, id: NodeId) -> bool {
    plan.path_to(id).iter().all(|&n| plan.node(n).is_some_and(|n| n.enabled))
}

/// Under a Test Fragment nothing runs by itself.
fn in_fragment(plan: &Plan, id: NodeId) -> bool {
    plan.path_to(id).iter().any(|&n| plan.node(n).is_some_and(|n| n.category == Category::Fragment))
}

/// Everything that takes part in running `sampler`, in the order JMeter runs it. Elements that are switched
/// off are listed too, marked inactive.
pub fn for_sampler(plan: &Plan, sampler: NodeId) -> Vec<Entry> {
    let Some(s) = plan.node(sampler) else { return vec![] };
    if s.category != Category::Sampler || in_fragment(plan, sampler) {
        return vec![];
    }
    // From the sampler itself outwards: its own children first, then those of each ancestor.
    let path = plan.path_to(sampler);
    let mut levels: Vec<NodeId> = Vec::new();
    levels.push(sampler);
    levels.extend(path[..path.len() - 1].iter().rev().copied());
    let mut by_phase: std::collections::BTreeMap<Phase, Vec<Entry>> = Default::default();
    for &holder in &levels {
        let Some(h) = plan.node(holder) else { continue };
        for &c in &h.children {
            let Some(cn) = plan.node(c) else { continue };
            let Some(phase) = Phase::of(cn.category) else { continue };
            if phase == Phase::Sampler {
                continue;
            }
            by_phase.entry(phase).or_default().push(Entry { node: c, phase, from: holder, active: active(plan, c) });
        }
    }
    let mut out = Vec::new();
    for phase in [Phase::Config, Phase::PreProcessor, Phase::Timer, Phase::Sampler, Phase::PostProcessor, Phase::Assertion, Phase::Listener] {
        if phase == Phase::Sampler {
            out.push(Entry { node: sampler, phase, from: s.parent.unwrap_or(sampler), active: active(plan, sampler) });
            continue;
        }
        let mut entries = by_phase.remove(&phase).unwrap_or_default();
        // `entries` is innermost level first, in tree order within a level. Configuration and listeners are
        // used in that order; pre-processors, post-processors and assertions run outermost level first.
        if matches!(phase, Phase::PreProcessor | Phase::PostProcessor | Phase::Assertion) {
            entries = regroup_outer_first(plan, entries);
        }
        out.extend(entries);
    }
    out
}

/// The settings of an HTTP Request that HTTP Request Defaults can supply.
pub const HTTP_DEFAULTED: [&str; 7] = ["HTTPSampler.domain", "HTTPSampler.port", "HTTPSampler.protocol", "HTTPSampler.path", "HTTPSampler.contentEncoding", "HTTPSampler.connect_timeout", "HTTPSampler.response_timeout"];

/// What an HTTP Request would use for the setting `prop` when it leaves it empty: the value in the nearest HTTP Request
/// Defaults that takes part in running it and has one, and the name of that element.
pub fn inherited_http(edit: &crate::edit::EditablePlan, sampler: NodeId, prop: &str) -> Option<(String, String)> {
    if !HTTP_DEFAULTED.contains(&prop) {
        return None;
    }
    let plan = edit.plan();
    for e in for_sampler(plan, sampler) {
        if e.phase != Phase::Config || !e.active {
            continue;
        }
        let Some(n) = plan.node(e.node) else { continue };
        if n.type_label != "HTTP Request Defaults" {
            continue;
        }
        if let Some(v) = edit.prop(e.node, prop).filter(|v| !v.trim().is_empty()) {
            return Some((v, n.name.clone()));
        }
    }
    None
}

/// Put entries listed innermost level first into outermost level first, keeping tree order within a level.
fn regroup_outer_first(plan: &Plan, entries: Vec<Entry>) -> Vec<Entry> {
    let mut levels: Vec<(NodeId, Vec<Entry>)> = Vec::new();
    for e in entries {
        match levels.iter_mut().find(|(h, _)| *h == e.from) {
            Some((_, v)) => v.push(e),
            None => levels.push((e.from, vec![e])),
        }
    }
    levels.reverse();
    let _ = plan;
    levels.into_iter().flat_map(|(_, v)| v).collect()
}

/// The samplers an element applies to. For a sampler that is itself; for a controller or thread group, every sampler
/// below it. For anything else, the samplers in its scope.
pub fn applies_to(plan: &Plan, id: NodeId) -> Vec<NodeId> {
    let Some(n) = plan.node(id) else { return vec![] };
    match n.category {
        Category::Sampler => vec![id],
        c if Phase::of(c).is_some() => {
            let Some(parent) = n.parent.and_then(|p| plan.node(p)) else { return vec![] };
            if in_fragment(plan, id) {
                return vec![];
            }
            if parent.category == Category::Sampler {
                return vec![parent.id];
            }
            samplers_below(plan, parent.id)
        }
        _ => vec![],
    }
}

/// Every sampler at or below `id`, in tree order.
pub fn samplers_below(plan: &Plan, id: NodeId) -> Vec<NodeId> {
    let mut out = Vec::new();
    let mut stack = vec![id];
    while let Some(n) = stack.pop() {
        let Some(node) = plan.node(n) else { continue };
        if node.category == Category::Sampler {
            out.push(n);
        }
        stack.extend(node.children.iter().rev());
    }
    out
}

/// The elements directly under `id` that apply to everything below it (configuration, timers, processors,
/// assertions and listeners), for a controller or thread group.
pub fn shared_by_children(plan: &Plan, id: NodeId) -> Vec<Entry> {
    let Some(n) = plan.node(id) else { return vec![] };
    if matches!(n.category, Category::Sampler) {
        return vec![];
    }
    n.children
        .iter()
        .filter_map(|&c| {
            let cn = plan.node(c)?;
            let phase = Phase::of(cn.category).filter(|p| *p != Phase::Sampler)?;
            Some(Entry { node: c, phase, from: id, active: active(plan, c) })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test Plan with a config at the top, and a thread group holding a timer, a controller with a pre-processor
    /// and two samplers (one with its own assertion), and a listener at the end.
    const PLAN: &str = r#"<jmeterTestPlan><hashTree>
<TestPlan testname="Plan"/><hashTree>
 <ConfigTestElement guiclass="HttpDefaultsGui" testname="Defaults Top"/><hashTree/>
 <ResponseAssertion guiclass="AssertionGui" testname="Assert Top"/><hashTree/>
 <ThreadGroup testname="Users"/><hashTree>
  <ConfigTestElement guiclass="HttpDefaultsGui" testname="Defaults Group"/><hashTree/>
  <ConstantTimer testname="Timer Group"/><hashTree/>
  <JSR223PreProcessor guiclass="TestBeanGUI" testname="Pre Group"/><hashTree/>
  <TransactionController testname="Checkout"/><hashTree>
   <JSR223PreProcessor guiclass="TestBeanGUI" testname="Pre Checkout"/><hashTree/>
   <ResponseAssertion guiclass="AssertionGui" testname="Assert Checkout"/><hashTree/>
   <HTTPSamplerProxy guiclass="HttpTestSampleGui" testname="Pay"/><hashTree>
    <ResponseAssertion guiclass="AssertionGui" testname="Assert Pay"/><hashTree/>
    <ConfigTestElement guiclass="HttpDefaultsGui" testname="Defaults Pay"/><hashTree/>
    <JSONPostProcessor testname="Extract"/><hashTree/>
   </hashTree>
   <HTTPSamplerProxy guiclass="HttpTestSampleGui" testname="Receipt"/><hashTree/>
  </hashTree>
  <ResultCollector guiclass="ViewResultsFullVisualizer" testname="Tree Group"/><hashTree/>
 </hashTree>
 <ResultCollector guiclass="SummaryReport" testname="Summary Top"/><hashTree/>
</hashTree></hashTree></jmeterTestPlan>"#;

    fn plan() -> Plan {
        Plan::from_jmx_str(PLAN).unwrap()
    }

    fn id(p: &Plan, n: &str) -> NodeId {
        p.find_by_name(n).unwrap_or_else(|| panic!("no {n}"))
    }

    fn names(p: &Plan, v: &[Entry]) -> Vec<String> {
        v.iter().map(|e| p.node(e.node).unwrap().name.clone()).collect()
    }

    #[test]
    fn a_sampler_gets_its_steps_in_jmeters_order() {
        let p = plan();
        let e = for_sampler(&p, id(&p, "Pay"));
        let phases: Vec<Phase> = e.iter().map(|e| e.phase).collect();
        let mut sorted = phases.clone();
        sorted.sort();
        assert_eq!(phases, sorted, "the steps come in order");
        assert_eq!(names(&p, &e), ["Defaults Pay", "Defaults Group", "Defaults Top", "Pre Group", "Pre Checkout", "Timer Group", "Pay", "Extract", "Assert Top", "Assert Checkout", "Assert Pay", "Tree Group", "Summary Top"]);
    }

    #[test]
    fn configuration_is_nearest_first_and_the_others_follow_their_note() {
        let p = plan();
        let e = for_sampler(&p, id(&p, "Pay"));
        let in_phase = |ph| names(&p, &e.iter().copied().filter(|x| x.phase == ph).collect::<Vec<_>>());
        assert_eq!(in_phase(Phase::Config), ["Defaults Pay", "Defaults Group", "Defaults Top"], "the nearest wins, so it comes first");
        assert_eq!(in_phase(Phase::PreProcessor), ["Pre Group", "Pre Checkout"], "outer first");
        assert_eq!(in_phase(Phase::Assertion), ["Assert Top", "Assert Checkout", "Assert Pay"], "outer first");
        assert_eq!(in_phase(Phase::Listener), ["Tree Group", "Summary Top"], "inner first");
    }

    #[test]
    fn an_element_under_a_sibling_sampler_does_not_apply() {
        let p = plan();
        let e = for_sampler(&p, id(&p, "Receipt"));
        let n = names(&p, &e);
        assert!(!n.contains(&"Assert Pay".to_string()) && !n.contains(&"Extract".to_string()) && !n.contains(&"Defaults Pay".to_string()), "{n:?}");
        assert!(n.contains(&"Assert Checkout".to_string()));
    }

    #[test]
    fn something_under_a_sampler_applies_to_that_sampler_only() {
        let p = plan();
        assert_eq!(applies_to(&p, id(&p, "Assert Pay")), [id(&p, "Pay")]);
        assert_eq!(applies_to(&p, id(&p, "Extract")), [id(&p, "Pay")]);
    }

    #[test]
    fn something_under_a_controller_applies_to_every_sampler_inside_its_parent() {
        let p = plan();
        let pay = id(&p, "Pay");
        let receipt = id(&p, "Receipt");
        assert_eq!(applies_to(&p, id(&p, "Assert Checkout")), [pay, receipt]);
        assert_eq!(applies_to(&p, id(&p, "Timer Group")), [pay, receipt], "samplers inside the sibling controller too");
        assert_eq!(applies_to(&p, id(&p, "Summary Top")), [pay, receipt], "a listener at the top covers the whole plan");
    }

    #[test]
    fn a_sampler_applies_to_itself_and_a_controller_to_nothing_in_particular() {
        let p = plan();
        assert_eq!(applies_to(&p, id(&p, "Pay")), [id(&p, "Pay")]);
        assert!(applies_to(&p, id(&p, "Checkout")).is_empty());
        assert!(applies_to(&p, id(&p, "Users")).is_empty());
    }

    #[test]
    fn a_controller_lists_what_it_shares_with_everything_inside() {
        let p = plan();
        let shared = shared_by_children(&p, id(&p, "Checkout"));
        assert_eq!(names(&p, &shared), ["Pre Checkout", "Assert Checkout"]);
        assert!(shared.iter().all(|e| e.from == id(&p, "Checkout")));
        assert!(shared_by_children(&p, id(&p, "Pay")).is_empty(), "a sampler shares nothing");
    }

    #[test]
    fn switched_off_elements_are_listed_but_inactive() {
        let src = PLAN.replace("testname=\"Assert Top\"", "testname=\"Assert Top\" enabled=\"false\"").replace("testname=\"Checkout\"", "testname=\"Checkout\" enabled=\"false\"");
        let p = Plan::from_jmx_str(&src).unwrap();
        let e = for_sampler(&p, id(&p, "Pay"));
        let top = e.iter().find(|x| p.node(x.node).unwrap().name == "Assert Top").unwrap();
        assert!(!top.active);
        let pay = e.iter().find(|x| x.phase == Phase::Sampler).unwrap();
        assert!(!pay.active, "a switched-off controller switches off what is inside it");
        let pre = e.iter().find(|x| p.node(x.node).unwrap().name == "Pre Group").unwrap();
        assert!(pre.active);
    }

    #[test]
    fn nothing_runs_inside_a_test_fragment() {
        let src = "<jmeterTestPlan><hashTree><TestPlan testname=\"P\"/><hashTree><TestFragmentController testname=\"F\"/><hashTree><ResponseAssertion guiclass=\"AssertionGui\" testname=\"A\"/><hashTree/><HTTPSamplerProxy guiclass=\"HttpTestSampleGui\" testname=\"S\"/><hashTree/></hashTree></hashTree></hashTree></jmeterTestPlan>";
        let p = Plan::from_jmx_str(src).unwrap();
        assert!(applies_to(&p, id(&p, "A")).is_empty());
    }

    #[test]
    fn non_samplers_have_no_run_order() {
        let p = plan();
        assert!(for_sampler(&p, id(&p, "Checkout")).is_empty());
        assert!(for_sampler(&p, NodeId(9999)).is_empty());
    }
}
