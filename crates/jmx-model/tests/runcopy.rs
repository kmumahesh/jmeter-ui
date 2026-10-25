//! Tests for the edits applied to a per-run copy of a plan.

use jmx_model::plan::plan_tree;
use jmx_model::runcopy::{Collector, inject_collector, set_loops};
use jmx_model::Document;

const PLAN: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<jmeterTestPlan version="1.2" properties="5.0" jmeter="5.6.3">
  <hashTree>
    <TestPlan guiclass="TestPlanGui" testclass="TestPlan" testname="P" enabled="true"/>
    <hashTree>
      <ThreadGroup guiclass="ThreadGroupGui" testclass="ThreadGroup" testname="G" enabled="true">
        <elementProp name="ThreadGroup.main_controller" elementType="LoopController">
          <stringProp name="LoopController.loops">5</stringProp>
        </elementProp>
      </ThreadGroup>
      <hashTree/>
    </hashTree>
  </hashTree>
</jmeterTestPlan>"#;

#[test]
fn injects_one_collector_under_the_test_plan() {
    let mut d = Document::parse(PLAN).unwrap();
    inject_collector(&mut d, &Collector::XmlErrors { path: "C:\\runs\\1\\errors.xml" }).unwrap();
    let out = d.write();
    let nodes = plan_tree(&Document::parse(&out).expect("still valid XML")).unwrap();
    let plan_children: Vec<_> = nodes[0].children.iter().map(|c| c.tag.as_str()).collect();
    assert_eq!(plan_children, ["ThreadGroup", "ResultCollector"], "added after the existing children");
    let rc = &nodes[0].children[1];
    assert!(rc.props.iter().any(|p| p.name == "filename" && p.value == "C:\\runs\\1\\errors.xml"));
    assert!(rc.props.iter().any(|p| p.name == "ResultCollector.error_logging" && p.value == "true"));
    assert!(out.contains("<xml>true</xml>") && out.contains("<responseData>true</responseData>") && out.contains("<url>true</url>"));
}

#[test]
fn the_path_is_escaped() {
    let mut d = Document::parse(PLAN).unwrap();
    inject_collector(&mut d, &Collector::XmlErrors { path: "C:\\a&b\\<x>.xml" }).unwrap();
    let out = d.write();
    assert!(out.contains("C:\\a&amp;b\\&lt;x&gt;.xml"));
    let doc = Document::parse(&out).unwrap();
    let nodes = plan_tree(&doc).unwrap();
    let f = nodes[0].children[1].props.iter().find(|p| p.name == "filename").unwrap();
    assert_eq!(f.value, "C:\\a&b\\<x>.xml");
}

#[test]
fn everything_before_the_insertion_point_is_unchanged() {
    let mut d = Document::parse(PLAN).unwrap();
    inject_collector(&mut d, &Collector::XmlErrors { path: "x" }).unwrap();
    let cut = PLAN.find("</hashTree>\n  </hashTree>").unwrap();
    assert!(d.write().starts_with(&PLAN[..cut]));
}

#[test]
fn rejects_documents_without_a_test_plan() {
    let mut d = Document::parse("<jmeterTestPlan><hashTree/></jmeterTestPlan>").unwrap();
    assert!(inject_collector(&mut d, &Collector::XmlErrors { path: "x" }).is_err());
    let mut d = Document::parse("<other/>").unwrap();
    assert!(inject_collector(&mut d, &Collector::XmlErrors { path: "x" }).is_err());
}

#[test]
fn sets_loops_everywhere_and_counts() {
    let mut d = Document::parse(PLAN).unwrap();
    assert_eq!(set_loops(&mut d, -1), 1);
    assert!(d.write().contains("<stringProp name=\"LoopController.loops\">-1</stringProp>"));
    let mut none = Document::parse("<a/>").unwrap();
    assert_eq!(set_loops(&mut none, 3), 0);
}

#[test]
fn a_listener_with_a_script_goes_last_under_the_test_plan_and_the_script_comes_back_exactly() {
    use jmx_model::runcopy::inject_script_listener;
    let script = "try {\n  def a = [] << 'x' && \"<tag attr='1'>\" ; if (1 < 2 && 3 > 2) { a << \"]]>\" }\n} catch (Throwable t) { }\n// caf\u{e9} \u{1F600}\n";
    let mut d = Document::parse(PLAN).unwrap();
    inject_script_listener(&mut d, "JMeter UI \"details\" <1>", script).unwrap();
    let out = d.write();
    let again = Document::parse(&out).expect("still valid XML");
    let nodes = plan_tree(&again).unwrap();
    let kids: Vec<_> = nodes[0].children.iter().map(|c| c.tag.as_str()).collect();
    assert_eq!(kids, ["ThreadGroup", "JSR223Listener"]);
    let listener = &nodes[0].children[1];
    assert_eq!(listener.testname, "JMeter UI \"details\" <1>");
    let got = listener.props.iter().find(|p| p.name == "script").unwrap();
    assert_eq!(got.value, script, "the script is stored as text and read back unchanged");
    assert!(listener.props.iter().any(|p| p.name == "scriptLanguage" && p.value == "groovy"));
    assert!(!PLAN.is_empty() && out.starts_with(&PLAN[..PLAN.find("</hashTree>\n  </hashTree>").unwrap()]), "what was there is unchanged");
}

#[test]
fn a_listener_cannot_be_added_to_a_document_without_a_test_plan() {
    use jmx_model::runcopy::inject_script_listener;
    assert!(inject_script_listener(&mut Document::parse("<jmeterTestPlan><hashTree/></jmeterTestPlan>").unwrap(), "x", "y").is_err());
}
