//! What an HTTP Request would take from HTTP Request Defaults when it leaves a setting empty.

use plan_core::scope::inherited_http;
use plan_core::{EditablePlan, NodeId};

fn plan(domain_outer: &str, domain_inner: &str, inner_enabled: bool, request_domain: &str) -> (EditablePlan, NodeId) {
    let text = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<jmeterTestPlan version="1.2" properties="5.0" jmeter="5.6.3"><hashTree>
<TestPlan guiclass="TestPlanGui" testclass="TestPlan" testname="Shop" enabled="true"/>
<hashTree>
  <ConfigTestElement guiclass="HttpDefaultsGui" testclass="ConfigTestElement" testname="Outer defaults" enabled="true">
    <stringProp name="HTTPSampler.domain">{domain_outer}</stringProp>
    <stringProp name="HTTPSampler.port">8443</stringProp>
    <stringProp name="HTTPSampler.protocol">https</stringProp>
  </ConfigTestElement>
  <hashTree/>
  <ThreadGroup guiclass="ThreadGroupGui" testclass="ThreadGroup" testname="Users" enabled="true"/>
  <hashTree>
    <ConfigTestElement guiclass="HttpDefaultsGui" testclass="ConfigTestElement" testname="Inner defaults" enabled="{inner_enabled}">
      <stringProp name="HTTPSampler.domain">{domain_inner}</stringProp>
    </ConfigTestElement>
    <hashTree/>
    <HTTPSamplerProxy guiclass="HttpTestSampleGui" testclass="HTTPSamplerProxy" testname="Home" enabled="true">
      <stringProp name="HTTPSampler.domain">{request_domain}</stringProp>
      <stringProp name="HTTPSampler.path">/</stringProp>
    </HTTPSamplerProxy>
    <hashTree/>
  </hashTree>
</hashTree></hashTree></jmeterTestPlan>"#
    );
    let e = EditablePlan::open(&text).unwrap();
    let id = e.plan().find_by_name("Home").unwrap();
    (e, id)
}

#[test]
fn the_nearest_defaults_with_a_value_supply_it_and_are_named() {
    let (e, id) = plan("outer.example.com", "inner.example.com", true, "");
    assert_eq!(inherited_http(&e, id, "HTTPSampler.domain"), Some(("inner.example.com".into(), "Inner defaults".into())));
    // A setting only the outer one has comes from the outer one.
    assert_eq!(inherited_http(&e, id, "HTTPSampler.port"), Some(("8443".into(), "Outer defaults".into())));
    assert_eq!(inherited_http(&e, id, "HTTPSampler.protocol"), Some(("https".into(), "Outer defaults".into())));
}

#[test]
fn defaults_that_are_switched_off_or_empty_supply_nothing() {
    let (e, id) = plan("outer.example.com", "inner.example.com", false, "");
    assert_eq!(inherited_http(&e, id, "HTTPSampler.domain").map(|x| x.0).as_deref(), Some("outer.example.com"), "the switched-off one is skipped");
    let (e, id) = plan("", "", true, "");
    assert_eq!(inherited_http(&e, id, "HTTPSampler.domain"), None);
    assert_eq!(inherited_http(&e, id, "HTTPSampler.path"), None);
}

#[test]
fn only_the_settings_defaults_can_supply_are_looked_up() {
    let (e, id) = plan("outer.example.com", "inner.example.com", true, "");
    for prop in ["HTTPSampler.method", "HTTPSampler.follow_redirects", "TestPlan.comments", "", "../../x", "HTTPSampler.domain\u{0}"] {
        assert_eq!(inherited_http(&e, id, prop), None, "{prop:?}");
    }
}

#[test]
fn something_that_is_not_a_request_or_has_no_defaults_gets_nothing() {
    let (e, _) = plan("outer.example.com", "inner.example.com", true, "");
    let group = e.plan().find_by_name("Users").unwrap();
    assert_eq!(inherited_http(&e, group, "HTTPSampler.domain"), None, "a thread group is not a request");
}

#[test]
fn the_real_plan_from_the_jmeter_samples_works() {
    let text = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/build-adv-web-test-plan.jmx")).unwrap();
    let e = EditablePlan::open(&text).unwrap();
    let mut found = 0;
    for n in e.plan().iter() {
        if n.tag == "HTTPSamplerProxy" {
            // Never panics, whatever the plan holds.
            if inherited_http(&e, n.id, "HTTPSampler.domain").is_some() {
                found += 1;
            }
        }
    }
    eprintln!("{found} requests take their server from defaults");
}
