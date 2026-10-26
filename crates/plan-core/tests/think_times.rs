//! Adding a pause before every request under an element: what gets one, what does not, one step to undo, and limits.

use plan_core::edit::MAX_THINK_MS;
use plan_core::{EditablePlan, NodeId};

const PLAN: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<jmeterTestPlan version="1.2" properties="5.0" jmeter="5.6.3"><hashTree>
<TestPlan guiclass="TestPlanGui" testclass="TestPlan" testname="Shop" enabled="true"/>
<hashTree>
  <ThreadGroup guiclass="ThreadGroupGui" testclass="ThreadGroup" testname="Users" enabled="true"/>
  <hashTree>
    <HTTPSamplerProxy guiclass="HttpTestSampleGui" testclass="HTTPSamplerProxy" testname="Home" enabled="true"/><hashTree/>
    <TransactionController guiclass="TransactionControllerGui" testclass="TransactionController" testname="Checkout" enabled="true"/>
    <hashTree>
      <HTTPSamplerProxy guiclass="HttpTestSampleGui" testclass="HTTPSamplerProxy" testname="Cart" enabled="true"/><hashTree/>
      <HTTPSamplerProxy guiclass="HttpTestSampleGui" testclass="HTTPSamplerProxy" testname="Pay" enabled="true"/>
      <hashTree>
        <ConstantTimer guiclass="ConstantTimerGui" testclass="ConstantTimer" testname="Own pause" enabled="true"/><hashTree/>
      </hashTree>
      <HTTPSamplerProxy guiclass="HttpTestSampleGui" testclass="HTTPSamplerProxy" testname="Off" enabled="false"/><hashTree/>
    </hashTree>
  </hashTree>
</hashTree></hashTree></jmeterTestPlan>"#;

fn open() -> EditablePlan {
    EditablePlan::open(PLAN).unwrap()
}

fn id(e: &EditablePlan, name: &str) -> NodeId {
    e.plan().find_by_name(name).unwrap()
}

fn timers_under(e: &EditablePlan, name: &str) -> Vec<String> {
    let n = e.plan().node(id(e, name)).unwrap();
    n.children.iter().filter_map(|c| e.plan().node(*c)).filter(|c| c.name == "Think time").map(|c| c.tag.clone()).collect()
}

#[test]
fn every_enabled_request_without_a_pause_gets_one_and_the_others_are_left_alone() {
    let mut e = open();
    let added = e.add_think_times(id(&e, "Users"), 1000, 3000).unwrap();
    assert_eq!(added, 2, "Home and Cart; Pay has its own pause and Off is switched off");
    assert_eq!(timers_under(&e, "Home"), ["UniformRandomTimer"]);
    assert_eq!(timers_under(&e, "Cart"), ["UniformRandomTimer"]);
    assert!(timers_under(&e, "Pay").is_empty() && timers_under(&e, "Off").is_empty());
    let t = e.plan().node(id(&e, "Home")).unwrap().children[0];
    assert_eq!((e.prop(t, "ConstantTimer.delay").as_deref(), e.prop(t, "RandomTimer.range").as_deref()), (Some("1000"), Some("2000.0")));
    // The saved plan reads back the same.
    let text = e.to_text();
    assert_eq!(EditablePlan::open(&text).unwrap().to_text(), text);
}

#[test]
fn it_is_one_step_to_undo_and_redo() {
    let mut e = open();
    let before = e.to_text();
    e.add_think_times(id(&e, "Users"), 500, 1500).unwrap();
    let after = e.to_text();
    assert_ne!(before, after);
    e.undo().unwrap();
    assert_eq!(e.to_text(), before, "one undo takes all of them away");
    assert!(!e.can_undo() || e.to_text() == before);
    e.redo().unwrap();
    assert_eq!(e.to_text(), after);
}

#[test]
fn only_what_is_under_the_chosen_element_is_touched() {
    let mut e = open();
    assert_eq!(e.add_think_times(id(&e, "Checkout"), 100, 200).unwrap(), 1);
    assert!(timers_under(&e, "Home").is_empty(), "Home is not under Checkout");
    assert_eq!(timers_under(&e, "Cart").len(), 1);
    // A request itself can be chosen.
    assert_eq!(e.add_think_times(id(&e, "Home"), 100, 200).unwrap(), 1);
    // The Test Plan reaches everything below it, so only what is left gets one.
    let mut e = open();
    assert_eq!(e.add_think_times(id(&e, "Shop"), 100, 200).unwrap(), 2);
}

#[test]
fn the_same_range_makes_a_constant_timer() {
    let mut e = open();
    e.add_think_times(id(&e, "Home"), 2500, 2500).unwrap();
    assert_eq!(timers_under(&e, "Home"), ["ConstantTimer"]);
    let t = e.plan().node(id(&e, "Home")).unwrap().children[0];
    assert_eq!(e.prop(t, "ConstantTimer.delay").as_deref(), Some("2500"));
}

#[test]
fn asking_again_or_with_nothing_to_do_or_a_wrong_range_says_why_and_changes_nothing() {
    let mut e = open();
    e.add_think_times(id(&e, "Users"), 1000, 2000).unwrap();
    let text = e.to_text();
    assert!(e.add_think_times(id(&e, "Users"), 1000, 2000).unwrap_err().to_string().contains("already has a pause"));
    assert!(e.add_think_times(id(&e, "Pay"), 1, 2).unwrap_err().to_string().contains("already has a pause"));
    let mut f = open();
    let before = f.to_text();
    assert!(f.add_think_times(id(&f, "Home"), 3000, 1000).unwrap_err().to_string().contains("shortest pause"));
    assert!(f.add_think_times(id(&f, "Home"), 0, MAX_THINK_MS + 1).unwrap_err().to_string().contains("not allowed"));
    assert!(f.add_think_times(NodeId(999_999), 1, 2).is_err());
    assert_eq!(f.to_text(), before, "nothing changed");
    assert!(!f.is_dirty());
    assert_eq!(e.to_text(), text);
    // The longest allowed pause and a zero pause are fine.
    assert!(f.add_think_times(id(&f, "Home"), 0, MAX_THINK_MS).is_ok());
}

#[test]
fn a_big_plan_gets_its_pauses_quickly() {
    let mut text = String::from(r#"<?xml version="1.0" encoding="UTF-8"?><jmeterTestPlan version="1.2" properties="5.0" jmeter="5.6.3"><hashTree><TestPlan guiclass="TestPlanGui" testclass="TestPlan" testname="Big" enabled="true"/><hashTree><ThreadGroup guiclass="ThreadGroupGui" testclass="ThreadGroup" testname="G" enabled="true"/><hashTree>"#);
    for i in 0..3000 {
        text.push_str(&format!(r#"<HTTPSamplerProxy guiclass="HttpTestSampleGui" testclass="HTTPSamplerProxy" testname="R{i}" enabled="true"/><hashTree/>"#));
    }
    text.push_str("</hashTree></hashTree></hashTree></jmeterTestPlan>");
    let mut e = EditablePlan::open(&text).unwrap();
    let g = e.plan().find_by_name("G").unwrap();
    let t = std::time::Instant::now();
    assert_eq!(e.add_think_times(g, 1000, 3000).unwrap(), 3000);
    eprintln!("3000 pauses added in {:?}", t.elapsed());
    assert!(t.elapsed() < std::time::Duration::from_secs(20), "{:?}", t.elapsed());
    let t = std::time::Instant::now();
    e.undo().unwrap();
    assert!(t.elapsed() < std::time::Duration::from_secs(20), "undo took {:?}", t.elapsed());
    assert_eq!(e.plan().iter().count(), 3002);
}
