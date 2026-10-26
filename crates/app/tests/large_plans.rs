//! Big plans: how many requests open within five seconds, that working with them stays quick, and that nothing in the
//! window grows faster than the plan does. A plan of 80,000 requests opened in about five seconds when this was measured
//! in a test build; 60,000 is the size the app calls comfortable, and the check here keeps 40,000 well inside the limit on
//! every kind of machine the automated checks run on.

mod common;

use common::*;
use gpui::TestAppContext;
use std::time::{Duration, Instant};

/// The most time a plan of the "comfortable" size may take to open.
const OPEN_LIMIT: Duration = Duration::from_secs(5);

fn plan_text(requests: usize) -> String {
    let mut s = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?><jmeterTestPlan version=\"1.2\" properties=\"5.0\" jmeter=\"5.6.3\"><hashTree><TestPlan guiclass=\"TestPlanGui\" testclass=\"TestPlan\" testname=\"Big\" enabled=\"true\"/><hashTree>");
    let groups = (requests / 100).max(1);
    for g in 0..groups {
        s.push_str(&format!("<ThreadGroup guiclass=\"ThreadGroupGui\" testclass=\"ThreadGroup\" testname=\"G{g}\" enabled=\"true\"/><hashTree>"));
        for r in 0..(requests / groups) {
            // A variable in every request: the check for undefined variables has the most to do.
            s.push_str(&format!("<HTTPSamplerProxy guiclass=\"HttpTestSampleGui\" testclass=\"HTTPSamplerProxy\" testname=\"GET /g{g}/r{r}\" enabled=\"true\"><stringProp name=\"HTTPSampler.domain\">example.com</stringProp><stringProp name=\"HTTPSampler.path\">/g{g}/r{r}?id=${{id}}</stringProp><stringProp name=\"HTTPSampler.method\">GET</stringProp></HTTPSamplerProxy><hashTree/>"));
        }
        s.push_str("</hashTree>");
    }
    s.push_str("</hashTree></hashTree></jmeterTestPlan>");
    s
}

struct Timings {
    open: Duration,
    problems: Duration,
    goto: Duration,
    rename: Duration,
    save: Duration,
}

fn measure(cx: &mut TestAppContext, requests: usize, name: &str) -> Timings {
    let dir = temp_dir(name);
    let plan = dir.join("big.jmx");
    std::fs::write(&plan, plan_text(requests)).unwrap();
    let t = Instant::now();
    let mut app = start(cx, Some(plan.clone()), None);
    let open = t.elapsed();
    assert_eq!(app.state()["elements"].as_u64(), Some(requests as u64 + requests as u64 / 100 + 1), "{requests} requests, their groups and the plan");
    app.keys("ctrl-j");
    // The notes about undefined variables are worked out when the Problems list is drawn: the part that grew with the square.
    let t = Instant::now();
    app.click("bottom-problems");
    let problems = t.elapsed();
    assert!(app.state()["problems"].as_array().unwrap().len() >= requests, "every request uses a variable nothing defines");
    let t = Instant::now();
    app.keys("ctrl-p");
    app.input("g7 r42");
    app.keys("enter");
    let goto = t.elapsed();
    assert!(app.selected().is_some_and(|n| n.contains("r42")), "{:?}", app.selected());
    let t = Instant::now();
    app.keys("f2");
    app.input("Renamed");
    app.keys("enter");
    let rename = t.elapsed();
    let t = Instant::now();
    let shell = app.shell.clone();
    app.cx.update(|window, cx| shell.update(cx, |sh, cx| sh.save_active(window, cx)));
    let save = t.elapsed();
    assert!(std::fs::read_to_string(&plan).unwrap().contains("Renamed"));
    eprintln!("{requests} requests: open {open:?}, problems {problems:?}, go to {goto:?}, rename {rename:?}, save {save:?}");
    Timings { open, problems, goto, rename, save }
}

#[gpui::test]
fn a_plan_of_forty_thousand_requests_opens_and_edits_well_inside_five_seconds(cx: &mut TestAppContext) {
    let t = measure(cx, 40_000, "large-40k");
    assert!(t.open < OPEN_LIMIT, "opening took {:?}", t.open);
    for (what, d) in [("the Problems list", t.problems), ("going to an element", t.goto), ("renaming", t.rename), ("saving", t.save)] {
        assert!(d < OPEN_LIMIT, "{what} took {d:?}");
    }
}

#[gpui::test]
fn the_time_grows_with_the_plan_and_not_with_its_square(cx: &mut TestAppContext) {
    let small = measure(cx, 5_000, "large-5k");
    let big = measure(cx, 20_000, "large-20k");
    // Four times the plan may take about four times as long; sixteen times would be growth with the square. Small
    // timings are noisy, so the allowance is generous and the floors keep a few milliseconds from deciding.
    let floor = Duration::from_millis(150);
    for (what, a, b) in [("opening", small.open, big.open), ("the Problems list", small.problems, big.problems), ("going to an element", small.goto, big.goto), ("renaming", small.rename, big.rename)] {
        let (a, b) = (a.max(floor), b.max(floor));
        assert!(b.as_secs_f64() < a.as_secs_f64() * 10.0, "{what}: {a:?} for 5,000 and {b:?} for 20,000 requests");
    }
}

#[gpui::test]
fn a_plan_over_the_comfortable_size_opens_and_says_it_will_be_slow(cx: &mut TestAppContext) {
    let dir = temp_dir("large-over");
    let plan = dir.join("huge.jmx");
    std::fs::write(&plan, plan_text(plan_core::COMFORTABLE_PLAN_ELEMENTS + 1_000)).unwrap();
    let mut app = start(cx, Some(plan), None);
    let notice = app.state()["notice"].as_str().unwrap_or("").to_string();
    assert!(notice.contains("open and edit slowly") && notice.contains("60000"), "{notice}");
    // A plan of ordinary size says nothing of the kind.
    let small = dir.join("small.jmx");
    std::fs::write(&small, plan_text(1_000)).unwrap();
    app.open_path(&small);
    let notice = app.state()["notice"].as_str().unwrap_or("").to_string();
    assert!(notice.starts_with("Opened small.jmx") && !notice.contains("slowly"), "{notice}");
}
