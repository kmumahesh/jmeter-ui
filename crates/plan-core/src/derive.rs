//! Plans made from a plan for a quick check: Validate (the whole plan with one user going through once) and
//! Send once (one request with what applies to it). The original is never changed; the new plan is written
//! into the run folder.
//!
//! Relative file names (CSV data, script files, included plans) are made absolute, because JMeter looks for
//! them next to the plan it was given, and the new plan is not in the folder of the original.

use crate::catalog::Category;
use crate::edit::EditablePlan;
use crate::model::NodeId;
use crate::scope::{self, Phase};
use anyhow::{Result, bail};
use std::path::Path;

/// A derived plan and what was changed in it, for the run header.
#[derive(Debug, Clone)]
pub struct Derived {
    pub text: String,
    pub notes: Vec<String>,
}

const LIMITED_GROUPS: [&str; 3] = ["ThreadGroup", "SetupThreadGroup", "PostThreadGroup"];

/// One user, one iteration, no schedule and no start delay.
fn limit_group(f: &mut EditablePlan, id: NodeId) -> Result<Option<String>> {
    let Some(n) = f.plan().node(id) else { return Ok(None) };
    let (tag, name) = (n.tag.clone(), n.name.clone());
    if !LIMITED_GROUPS.contains(&tag.as_str()) {
        return Ok(Some(format!("{name} ({tag}) could not be limited to one user and runs as it is")));
    }
    f.set_prop(id, "ThreadGroup.num_threads", "stringProp", "1")?;
    f.set_prop(id, "ThreadGroup.ramp_time", "stringProp", "1")?;
    f.set_prop(id, "ThreadGroup.scheduler", "boolProp", "false")?;
    f.set_prop(id, "ThreadGroup.duration", "stringProp", "")?;
    f.set_prop(id, "ThreadGroup.delay", "stringProp", "")?;
    // A group saved without its loop controller loops once anyway, so a missing one is not an error.
    let _ = f.set_prop(id, "ThreadGroup.main_controller/LoopController.loops", "stringProp", "1");
    let _ = f.set_prop(id, "ThreadGroup.main_controller/LoopController.continue_forever", "boolProp", "false");
    Ok(None)
}

/// Settings that name a file, by element tag: JMeter looks for relative names next to the plan.
const FILE_SETTINGS: [(&str, &str); 4] = [("CSVDataSet", "filename"), ("IncludeController", "IncludeController.includepath"), ("JSR223Sampler", "filename"), ("BeanShellSampler", "filename")];

fn is_relative_file(v: &str) -> bool {
    let v = v.trim();
    !v.is_empty() && !v.contains("${") && !v.starts_with(['/', '\\']) && !v.contains(':')
}

/// Make the file names in the plan absolute, relative to `plan_dir` (the folder of the original plan).
fn absolute_files(f: &mut EditablePlan, plan_dir: &Path, notes: &mut Vec<String>) -> Result<()> {
    let targets: Vec<(NodeId, &str, String)> = f
        .plan()
        .iter()
        .flat_map(|n| FILE_SETTINGS.iter().filter(|(tag, _)| *tag == n.tag).map(move |(_, prop)| (n.id, *prop)))
        .filter_map(|(id, prop)| f.prop(id, prop).map(|v| (id, prop, v)))
        .collect();
    let mut changed = 0;
    for (id, prop, value) in targets {
        if is_relative_file(&value) {
            let abs = plan_dir.join(value.trim());
            f.set_prop(id, prop, "stringProp", &abs.to_string_lossy())?;
            changed += 1;
        }
    }
    if changed > 0 {
        notes.push(format!("{changed} file name{} made absolute so JMeter finds {} next to the original plan", if changed == 1 { "" } else { "s" }, if changed == 1 { "it" } else { "them" }));
    }
    Ok(())
}

/// The plan for Validate: JMeter's own rules for checking a plan. Every thread group runs one user once
/// without a schedule or delay, and timers and Backend Listeners are left out.
pub fn validate_plan(e: &EditablePlan, plan_dir: &Path) -> Result<Derived> {
    let mut f = e.fork();
    let mut notes = Vec::new();
    let ids: Vec<NodeId> = f.plan().iter().map(|n| n.id).collect();
    let mut removed = 0;
    for id in &ids {
        if let Some(n) = f.plan().node(*id)
            && (n.category == Category::Timer || n.tag == "BackendListener")
        {
            f.remove(*id)?;
            removed += 1;
        }
    }
    if removed > 0 {
        notes.push(format!("{removed} timer{} and Backend Listener{} left out", if removed == 1 { "" } else { "s" }, if removed == 1 { "" } else { "s" }));
    }
    let groups: Vec<NodeId> = f.plan().iter().filter(|n| n.category == Category::ThreadGroup).map(|n| n.id).collect();
    if groups.is_empty() {
        bail!("The plan has no thread group, so there is nothing to validate");
    }
    for g in groups {
        notes.extend(limit_group(&mut f, g)?);
    }
    notes.insert(0, "one user, one iteration, no schedule or start delay".into());
    absolute_files(&mut f, plan_dir, &mut notes)?;
    Ok(Derived { text: f.to_text(), notes })
}

/// The plan for Send once: the sampler with what applies to it (configuration, pre-processors,
/// post-processors and assertions, in the order JMeter takes them), in one thread group that runs one user once.
/// Timers and listeners are left out so the request goes at once and nothing else is written.
pub fn send_once_plan(e: &EditablePlan, sampler: NodeId, plan_dir: &Path) -> Result<Derived> {
    let plan = e.plan().clone();
    let Some(s) = plan.node(sampler) else { bail!("That element is no longer in the plan") };
    if s.category != Category::Sampler {
        bail!("{} is not a sampler", s.name);
    }
    let entries = scope::for_sampler(&plan, sampler);
    if entries.is_empty() {
        bail!("{} is under a Test Fragment, which only runs when a Module or Include Controller refers to it", s.name);
    }
    if let Some(off) = plan.path_to(sampler).iter().find_map(|&n| plan.node(n).filter(|n| !n.enabled)) {
        bail!("{} is switched off", off.name);
    }
    let Some(group) = plan.path_to(sampler).iter().rev().copied().find(|&n| plan.node(n).is_some_and(|n| n.category == Category::ThreadGroup)) else {
        bail!("{} is not inside a thread group", s.name)
    };
    let keep: Vec<NodeId> = entries.iter().filter(|x| x.active && !matches!(x.phase, Phase::Timer | Phase::Listener)).map(|x| x.node).collect();
    let included = keep.len() - 1;
    let sampler_name = s.name.clone();

    let mut f = e.fork();
    // Put the request and what applies to it directly under the thread group, in the order JMeter takes them.
    for (i, id) in keep.iter().enumerate() {
        f.move_to(*id, group, i)?;
    }
    let extras: Vec<NodeId> = f.plan().node(group).map(|g| g.children[keep.len()..].to_vec()).unwrap_or_default();
    for id in extras {
        f.remove(id)?;
    }
    // What was under the request itself and does not apply (its timers and listeners) goes too.
    let own: Vec<NodeId> = f.plan().node(sampler).map(|n| n.children.clone()).unwrap_or_default();
    for id in own {
        f.remove(id)?;
    }
    let root = f.plan().roots()[0];
    let others: Vec<NodeId> = f.plan().node(root).map(|r| r.children.iter().copied().filter(|&c| c != group).collect()).unwrap_or_default();
    for id in others {
        f.remove(id)?;
    }
    let mut notes = vec![format!("only \"{sampler_name}\" is sent, with {included} element{} that apply to it", if included == 1 { "" } else { "s" })];
    notes.extend(limit_group(&mut f, group)?);
    absolute_files(&mut f, plan_dir, &mut notes)?;
    Ok(Derived { text: f.to_text(), notes })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(e: &EditablePlan, parent: &str) -> Vec<String> {
        let p = e.plan();
        p.node(p.find_by_name(parent).unwrap()).unwrap().children.iter().map(|&c| p.node(c).unwrap().name.clone()).collect()
    }

    /// A plan: defaults at the top, a thread group with a Loop Controller that holds the request.
    const PLAN: &str = r#"<jmeterTestPlan><hashTree>
<TestPlan guiclass="TestPlanGui" testclass="TestPlan" testname="Shop"/>
<hashTree>
  <ConfigTestElement guiclass="HttpDefaultsGui" testclass="ConfigTestElement" testname="Defaults"/><hashTree/>
  <ThreadGroup guiclass="ThreadGroupGui" testclass="ThreadGroup" testname="Users">
    <stringProp name="ThreadGroup.num_threads">50</stringProp>
    <boolProp name="ThreadGroup.scheduler">true</boolProp>
    <stringProp name="ThreadGroup.duration">600</stringProp>
    <stringProp name="ThreadGroup.delay">30</stringProp>
    <elementProp name="ThreadGroup.main_controller" elementType="LoopController" guiclass="LoopControlPanel" testclass="LoopController" testname="Loop Controller">
      <boolProp name="LoopController.continue_forever">true</boolProp>
      <stringProp name="LoopController.loops">-1</stringProp>
    </elementProp>
  </ThreadGroup>
  <hashTree>
    <CSVDataSet guiclass="TestBeanGUI" testclass="CSVDataSet" testname="Users file"><stringProp name="filename">users.csv</stringProp></CSVDataSet><hashTree/>
    <ConstantTimer guiclass="ConstantTimerGui" testclass="ConstantTimer" testname="Think"/><hashTree/>
    <LoopController guiclass="LoopControlPanel" testclass="LoopController" testname="Repeat"><stringProp name="LoopController.loops">10</stringProp></LoopController>
    <hashTree>
      <HeaderManager guiclass="HeaderPanel" testclass="HeaderManager" testname="Headers"/><hashTree/>
      <HTTPSamplerProxy guiclass="HttpTestSampleGui" testclass="HTTPSamplerProxy" testname="Login"><stringProp name="HTTPSampler.path">/login</stringProp></HTTPSamplerProxy>
      <hashTree>
        <ResponseAssertion guiclass="AssertionGui" testclass="ResponseAssertion" testname="Is 200"/><hashTree/>
        <ResultCollector guiclass="ViewResultsFullVisualizer" testclass="ResultCollector" testname="Tree"/><hashTree/>
        <ConstantTimer guiclass="ConstantTimerGui" testclass="ConstantTimer" testname="Own timer"/><hashTree/>
      </hashTree>
      <HTTPSamplerProxy guiclass="HttpTestSampleGui" testclass="HTTPSamplerProxy" testname="Cart"/><hashTree/>
    </hashTree>
    <UniformRandomTimer guiclass="UniformRandomTimerGui" testclass="UniformRandomTimer" testname="Random wait"/><hashTree/>
    <BackendListener guiclass="BackendListenerGui" testclass="BackendListener" testname="Graphite"/><hashTree/>
  </hashTree>
  <ThreadGroup guiclass="ThreadGroupGui" testclass="ThreadGroup" testname="Other users"/><hashTree/>
</hashTree>
</hashTree></jmeterTestPlan>"#;

    fn open() -> EditablePlan {
        EditablePlan::open(PLAN).unwrap()
    }

    #[test]
    fn validate_limits_every_group_and_drops_timers_and_backend_listeners() {
        let e = open();
        let d = validate_plan(&e, Path::new("C:/work/shop")).unwrap();
        let v = EditablePlan::open(&d.text).unwrap();
        let g = v.plan().find_by_name("Users").unwrap();
        for (prop, want) in [("ThreadGroup.num_threads", "1"), ("ThreadGroup.scheduler", "false"), ("ThreadGroup.duration", ""), ("ThreadGroup.delay", "")] {
            assert_eq!(v.prop(g, prop).as_deref(), Some(want), "{prop}");
        }
        assert!(d.text.contains("<stringProp name=\"LoopController.loops\">1</stringProp>"), "the group loops once:\n{}", d.text);
        assert!(!v.plan().iter().any(|n| matches!(n.category, Category::Timer) || n.tag == "BackendListener"), "timers and Backend Listeners are gone");
        assert_eq!(names(&v, "Users"), ["Users file", "Repeat"], "the rest stays");
        assert!(d.notes[0].contains("one user") && d.notes.iter().any(|n| n.contains("4 timers and Backend Listeners")), "{:?}", d.notes);
        // The original plan is untouched.
        assert_eq!(e.to_text(), PLAN);
    }

    #[test]
    fn validate_keeps_every_group_and_every_sampler() {
        let d = validate_plan(&open(), Path::new(".")).unwrap();
        let v = EditablePlan::open(&d.text).unwrap();
        assert_eq!(v.plan().iter().filter(|n| n.category == Category::ThreadGroup).count(), 2);
        assert_eq!(v.plan().iter().filter(|n| n.category == Category::Sampler).count(), 2);
        let other = v.plan().find_by_name("Other users").unwrap();
        assert_eq!(v.prop(other, "ThreadGroup.num_threads").as_deref(), Some("1"), "a group that had no settings gets them");
    }

    #[test]
    fn validate_needs_a_thread_group() {
        let e = EditablePlan::open("<jmeterTestPlan><hashTree><TestPlan guiclass=\"TestPlanGui\" testclass=\"TestPlan\" testname=\"P\"/><hashTree/></hashTree></jmeterTestPlan>").unwrap();
        assert!(validate_plan(&e, Path::new(".")).unwrap_err().to_string().contains("no thread group"));
    }

    #[test]
    fn send_once_keeps_the_request_and_what_applies_to_it_in_jmeters_order() {
        let e = open();
        let login = e.plan().find_by_name("Login").unwrap();
        let d = send_once_plan(&e, login, Path::new("C:/work/shop")).unwrap();
        let v = EditablePlan::open(&d.text).unwrap();
        // Configuration nearest first, then the request, then its assertions. Timers and listeners are left out.
        assert_eq!(names(&v, "Users"), ["Headers", "Users file", "Defaults", "Login", "Is 200"]);
        let root = v.plan().roots()[0];
        assert_eq!(v.plan().node(root).unwrap().children.len(), 1, "the other thread group and the top-level defaults are not duplicated");
        for gone in ["Own timer", "Think", "Repeat", "testname=\"Tree\""] {
            assert!(!d.text.contains(gone), "{gone} is still in:
{}", d.text);
        }
        let g = v.plan().find_by_name("Users").unwrap();
        assert_eq!(v.prop(g, "ThreadGroup.num_threads").as_deref(), Some("1"));
        assert!(d.notes[0].contains("\"Login\"") && d.notes[0].contains("4 elements"), "{:?}", d.notes);
        assert_eq!(e.to_text(), PLAN, "the original is untouched");
    }

    #[test]
    fn send_once_can_send_a_request_with_nothing_else_applying() {
        let e = open();
        let cart = e.plan().find_by_name("Cart").unwrap();
        let d = send_once_plan(&e, cart, Path::new(".")).unwrap();
        let v = EditablePlan::open(&d.text).unwrap();
        assert_eq!(names(&v, "Users"), ["Headers", "Users file", "Defaults", "Cart"]);
    }

    #[test]
    fn send_once_refuses_what_is_not_a_plain_request() {
        let e = open();
        let repeat = e.plan().find_by_name("Repeat").unwrap();
        assert!(send_once_plan(&e, repeat, Path::new(".")).unwrap_err().to_string().contains("not a sampler"));
        let mut e2 = open();
        let cart = e2.plan().find_by_name("Cart").unwrap();
        e2.set_enabled(cart, false).unwrap();
        assert!(send_once_plan(&e2, cart, Path::new(".")).unwrap_err().to_string().contains("switched off"));
        let mut e3 = open();
        let g = e3.plan().find_by_name("Users").unwrap();
        e3.set_enabled(g, false).unwrap();
        let login = e3.plan().find_by_name("Login").unwrap();
        assert!(send_once_plan(&e3, login, Path::new(".")).unwrap_err().to_string().contains("switched off"));
    }

    #[test]
    fn send_once_skips_requests_under_a_test_fragment() {
        let text = PLAN.replace("<LoopController guiclass=\"LoopControlPanel\" testclass=\"LoopController\" testname=\"Repeat\">", "<TestFragmentController guiclass=\"TestFragmentControllerGui\" testclass=\"TestFragmentController\" testname=\"Frag\"><stringProp name=\"x\">1</stringProp></TestFragmentController><hashTree><LoopController guiclass=\"LoopControlPanel\" testclass=\"LoopController\" testname=\"Repeat\">").replace("<HTTPSamplerProxy guiclass=\"HttpTestSampleGui\" testclass=\"HTTPSamplerProxy\" testname=\"Cart\"/><hashTree/>\n    </hashTree>", "<HTTPSamplerProxy guiclass=\"HttpTestSampleGui\" testclass=\"HTTPSamplerProxy\" testname=\"Cart\"/><hashTree/>\n    </hashTree></hashTree>");
        let e = EditablePlan::open(&text).unwrap();
        let login = e.plan().find_by_name("Login").unwrap();
        assert!(send_once_plan(&e, login, Path::new(".")).unwrap_err().to_string().contains("Test Fragment"));
    }

    #[test]
    fn relative_file_names_become_absolute_and_others_are_left_alone() {
        let e = open();
        let login = e.plan().find_by_name("Login").unwrap();
        let d = send_once_plan(&e, login, Path::new("C:/work/shop")).unwrap();
        let v = EditablePlan::open(&d.text).unwrap();
        let csv = v.plan().find_by_name("Users file").unwrap();
        assert_eq!(v.prop(csv, "filename").unwrap().replace('\\', "/"), "C:/work/shop/users.csv");
        assert!(d.notes.iter().any(|n| n.contains("1 file name made absolute")));

        for (value, stays) in [("C:/data/x.csv", true), ("/data/x.csv", true), ("\\\\server\\share\\x.csv", true), ("${__P(f)}.csv", true), ("", true), ("sub/x.csv", false)] {
            let text = PLAN.replace(">users.csv<", &format!(">{value}<"));
            let e = EditablePlan::open(&text).unwrap();
            let login = e.plan().find_by_name("Login").unwrap();
            let d = send_once_plan(&e, login, Path::new("D:/base")).unwrap();
            let v = EditablePlan::open(&d.text).unwrap();
            let got = v.prop(v.plan().find_by_name("Users file").unwrap(), "filename").unwrap();
            assert_eq!(got == value, stays, "{value:?} became {got:?}");
        }
    }

    #[test]
    fn hostile_text_in_names_and_files_survives_the_derived_plan() {
        let name = "</testname><evil/>&\"'";
        let mut e = open();
        let login = e.plan().find_by_name("Login").unwrap();
        e.rename(login, name).unwrap();
        let d = send_once_plan(&e, login, Path::new("D:/a & b/\"q\"")).unwrap();
        let v = EditablePlan::open(&d.text).unwrap();
        assert!(v.plan().find_by_name(name).is_some());
        assert_eq!(v.prop(v.plan().find_by_name("Users file").unwrap(), "filename").unwrap().replace('\\', "/"), "D:/a & b/\"q\"/users.csv");
    }
}
