//! Things worth telling the user about a plan before it is run.

use crate::catalog::Category;
use crate::model::{NodeId, Plan};
use std::collections::BTreeMap;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Level {
    Info,
    Warning,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Problem {
    pub level: Level,
    pub text: String,
    /// The element to show when the problem is selected, if it is about one element.
    pub node: Option<NodeId>,
}

/// Check a plan. Warnings come first, then notes; each group keeps plan order.
pub fn check(plan: &Plan) -> Vec<Problem> {
    let mut out = Vec::new();
    let groups: Vec<_> = plan.iter().filter(|n| n.category == Category::ThreadGroup).collect();
    if !plan.is_empty() && groups.is_empty() {
        out.push(Problem { level: Level::Warning, text: "The plan has no Thread Group, so nothing will run".into(), node: plan.roots().first().copied() });
    }
    for g in &groups {
        let has_requests = descendants(plan, g.id).any(|n| matches!(n.category, Category::Sampler));
        if g.enabled && !has_requests {
            out.push(Problem { level: Level::Warning, text: format!("Thread Group \"{}\" has no requests", g.name), node: Some(g.id) });
        }
        if !g.enabled {
            out.push(Problem { level: Level::Info, text: format!("Thread Group \"{}\" is disabled and will not run", g.name), node: Some(g.id) });
        }
    }
    let mut unknown: BTreeMap<&str, (usize, NodeId)> = BTreeMap::new();
    let mut plugins: BTreeMap<&str, (usize, NodeId)> = BTreeMap::new();
    for n in plan.iter() {
        if n.tag.contains('.') {
            let e = plugins.entry(n.tag.as_str()).or_insert((0, n.id));
            e.0 += 1;
        } else if n.category == Category::Unknown {
            let e = unknown.entry(n.tag.as_str()).or_insert((0, n.id));
            e.0 += 1;
        }
    }
    for (tag, (count, id)) in unknown {
        out.push(Problem { level: Level::Warning, text: format!("{count} element{} of an unrecognised type ({tag}); they are kept but cannot be edited here", plural(count)), node: Some(id) });
    }
    for (tag, (count, id)) in plugins {
        out.push(Problem {
            level: Level::Info,
            text: format!("{count} element{} need the plugin class {tag}; JMeter must have it installed to run this plan", plural(count)),
            node: Some(id),
        });
    }
    let disabled = plan.iter().filter(|n| !n.enabled && n.category != Category::ThreadGroup).count();
    if disabled > 0 {
        out.push(Problem { level: Level::Info, text: format!("{disabled} element{} disabled", if disabled == 1 { " is" } else { "s are" }), node: None });
    }
    out.sort_by_key(|p| p.level == Level::Info);
    out
}

fn plural(n: usize) -> &'static str {
    if n == 1 { "" } else { "s" }
}

fn descendants(plan: &Plan, id: NodeId) -> impl Iterator<Item = &crate::model::Node> {
    let mut stack: Vec<NodeId> = plan.node(id).map(|n| n.children.clone()).unwrap_or_default();
    std::iter::from_fn(move || {
        let next = stack.pop()?;
        let n = plan.node(next)?;
        stack.extend(n.children.iter().copied());
        Some(n)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plan(inner: &str) -> Plan {
        Plan::from_jmx_str(&format!("<jmeterTestPlan><hashTree><TestPlan testname=\"P\"/><hashTree>{inner}</hashTree></hashTree></jmeterTestPlan>")).unwrap()
    }

    fn texts(p: &[Problem]) -> Vec<&str> {
        p.iter().map(|p| p.text.as_str()).collect()
    }

    #[test]
    fn a_healthy_plan_has_no_problems() {
        let p = plan(r#"<ThreadGroup testname="G"/><hashTree><HTTPSamplerProxy testname="S"/><hashTree/></hashTree>"#);
        assert!(check(&p).is_empty());
    }

    #[test]
    fn no_thread_group() {
        let p = plan(r#"<ConstantTimer testname="T"/><hashTree/>"#);
        let r = check(&p);
        assert_eq!(texts(&r), ["The plan has no Thread Group, so nothing will run"]);
        assert_eq!(r[0].level, Level::Warning);
    }

    #[test]
    fn thread_group_without_requests() {
        let p = plan(r#"<ThreadGroup testname="Empty"/><hashTree><ConstantTimer testname="T"/><hashTree/></hashTree>"#);
        assert_eq!(texts(&check(&p)), ["Thread Group \"Empty\" has no requests"]);
    }

    #[test]
    fn requests_nested_in_controllers_count() {
        let p = plan(r#"<ThreadGroup testname="G"/><hashTree><LoopController testname="L"/><hashTree><IfController testname="I"/><hashTree><HTTPSamplerProxy testname="S"/><hashTree/></hashTree></hashTree></hashTree>"#);
        assert!(check(&p).is_empty());
    }

    #[test]
    fn disabled_thread_group_is_a_note_not_a_warning() {
        let p = plan(r#"<ThreadGroup testname="Off" enabled="false"/><hashTree/>"#);
        let r = check(&p);
        assert_eq!(texts(&r), ["Thread Group \"Off\" is disabled and will not run"]);
        assert_eq!(r[0].level, Level::Info);
    }

    #[test]
    fn plugin_and_unknown_elements_are_grouped_by_class() {
        let p = plan(
            r#"<ThreadGroup testname="G"/><hashTree><HTTPSamplerProxy testname="S"/><hashTree/>
<kg.apc.jmeter.samplers.DummySampler testname="a"/><hashTree/><kg.apc.jmeter.samplers.DummySampler testname="b"/><hashTree/>
<MysteryThing testname="m"/><hashTree/></hashTree>"#,
        );
        let r = check(&p);
        let t = texts(&r);
        assert_eq!(t.len(), 2, "{t:?}");
        assert!(t[0].contains("unrecognised type (MysteryThing)"), "warnings come first: {t:?}");
        assert!(t[1].starts_with("2 elements need the plugin class kg.apc.jmeter.samplers.DummySampler"), "{t:?}");
        assert_eq!(r[1].level, Level::Info);
        assert!(r[1].node.is_some());
    }

    #[test]
    fn disabled_elements_are_counted() {
        let p = plan(r#"<ThreadGroup testname="G"/><hashTree><HTTPSamplerProxy testname="S"/><hashTree/><HTTPSamplerProxy testname="D" enabled="false"/><hashTree/></hashTree>"#);
        assert_eq!(texts(&check(&p)), ["1 element is disabled"]);
        let p2 = plan(r#"<ThreadGroup testname="G"/><hashTree><HTTPSamplerProxy testname="S"/><hashTree/><ConstantTimer testname="a" enabled="false"/><hashTree/><ConstantTimer testname="b" enabled="false"/><hashTree/></hashTree>"#);
        assert_eq!(texts(&check(&p2)), ["2 elements are disabled"]);
    }

    #[test]
    fn empty_plan_gives_nothing() {
        assert!(check(&Plan::default()).is_empty());
    }
}
