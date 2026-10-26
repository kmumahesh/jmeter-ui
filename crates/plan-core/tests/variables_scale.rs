//! The check for variables nothing defines on big plans: it was slow with the square of the plan (21 seconds for 8,000
//! requests in a test build), and must stay in step with the plan. Also that reading every element's settings in one
//! walk gives what asking for each element gives.

use plan_core::{EditablePlan, variables};
use std::time::{Duration, Instant};

fn plan_text(requests: usize, with_variable: bool) -> String {
    let mut s = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?><jmeterTestPlan version=\"1.2\" properties=\"5.0\" jmeter=\"5.6.3\"><hashTree><TestPlan guiclass=\"TestPlanGui\" testclass=\"TestPlan\" testname=\"Big\" enabled=\"true\"/><hashTree>");
    let groups = (requests / 100).max(1);
    for g in 0..groups {
        s.push_str(&format!("<ThreadGroup guiclass=\"ThreadGroupGui\" testclass=\"ThreadGroup\" testname=\"G{g}\" enabled=\"true\"/><hashTree>"));
        for r in 0..(requests / groups) {
            let var = if with_variable { "?id=${id}" } else { "" };
            s.push_str(&format!("<HTTPSamplerProxy guiclass=\"HttpTestSampleGui\" testclass=\"HTTPSamplerProxy\" testname=\"GET /g{g}/r{r}\" enabled=\"true\"><stringProp name=\"HTTPSampler.path\">/g{g}/r{r}{var}</stringProp></HTTPSamplerProxy><hashTree/>"));
        }
        s.push_str("</hashTree>");
    }
    s.push_str("</hashTree></hashTree></jmeterTestPlan>");
    s
}

#[test]
fn the_check_for_undefined_variables_grows_with_the_plan_and_not_with_its_square() {
    let time = |n: usize| {
        let e = EditablePlan::open(&plan_text(n, true)).unwrap();
        let t = Instant::now();
        let notes = variables::check(&e);
        assert_eq!(notes.len(), n, "one note for each request that uses ${{id}}");
        t.elapsed()
    };
    let (small, big) = (time(5_000).max(Duration::from_millis(20)), time(40_000));
    eprintln!("5,000 requests {small:?}, 40,000 requests {big:?}");
    // Eight times the plan: about eight times the time. Growth with the square would be sixty-four.
    assert!(big.as_secs_f64() < small.as_secs_f64() * 24.0, "{small:?} for 5,000 and {big:?} for 40,000");
    assert!(big < Duration::from_secs(10), "{big:?}");
}

#[test]
fn defined_variables_of_a_big_plan_are_found_quickly_too() {
    let mut text = plan_text(20_000, true);
    // One extractor defines `id` at the top: no request is reported.
    let extractor = "<RegexExtractor guiclass=\"RegexExtractorGui\" testclass=\"RegexExtractor\" testname=\"Take id\" enabled=\"true\"><stringProp name=\"RegexExtractor.refname\">id</stringProp></RegexExtractor><hashTree/>";
    let at = text.find("<ThreadGroup").unwrap();
    text.insert_str(at, extractor);
    let e = EditablePlan::open(&text).unwrap();
    let t = Instant::now();
    assert!(variables::check(&e).is_empty());
    assert!(variables::defined(&e).contains("id"));
    assert!(t.elapsed() < Duration::from_secs(5), "{:?}", t.elapsed());
}

#[test]
fn one_walk_gives_the_settings_of_every_element_as_asking_for_each_does() {
    for file in ["build-adv-web-test-plan.jmx", "Test.jmx", "PluginComplexExample.jmx"] {
        let text = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(file)).unwrap();
        let e = EditablePlan::open(&text).unwrap();
        let all = e.all_setting_texts();
        assert_eq!(all.len(), e.plan().len(), "{file}: an entry for every element");
        for (id, settings) in &all {
            assert_eq!(*settings, e.setting_texts(*id), "{file}: {:?}", e.plan().node(*id).map(|n| n.name.clone()));
        }
    }
}
