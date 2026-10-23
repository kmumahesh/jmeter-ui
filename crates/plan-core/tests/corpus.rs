//! Tests against real plans: committed fixtures, and (when available) every plan on this machine.
//!
//! Set `JMX_CORPUS` to a list of folders (separated like PATH) to check a larger set. Without it the
//! large test looks for the JMeter install in `~/Downloads` and skips if it is not there.

use plan_core::{Category, Plan, TreeState, catalog, rules};
use std::path::{Path, PathBuf};

fn fixture(name: &str) -> Plan {
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name);
    Plan::load(&p).unwrap_or_else(|e| panic!("{e:#}"))
}

#[test]
fn simple_jmeter_sample() {
    let p = fixture("Test.jmx");
    let s = p.summary();
    assert_eq!(p.roots().len(), 1);
    let root = p.node(p.roots()[0]).unwrap();
    assert_eq!(root.category, Category::TestPlan);
    assert_eq!(root.name, "Test");
    assert_eq!(s.unknown, 0);
    let tg = p.find_by_name("Thread Group").unwrap();
    assert_eq!(p.node(tg).unwrap().category, Category::ThreadGroup);
    // The thread group holds a counter and the Java samplers.
    let kinds: Vec<_> = p.node(tg).unwrap().children.iter().map(|&c| p.node(c).unwrap().category).collect();
    assert!(kinds.contains(&Category::Config) && kinds.contains(&Category::Sampler), "{kinds:?}");
}

#[test]
fn web_test_template_has_the_usual_structure() {
    let p = fixture("build-adv-web-test-plan.jmx");
    let s = p.summary();
    assert_eq!(s.unknown, 0, "all elements in the template are known");
    assert!(s.total > 20);
    let count = |c: Category| s.by_category.iter().find(|(k, _)| *k == c).map_or(0, |(_, n)| *n);
    assert!(count(Category::Sampler) >= 5);
    assert!(count(Category::Config) >= 4);
    assert_eq!(count(Category::ThreadGroup), 1);
    assert!(p.find_by_name("JMeter Users").is_some());
    // Disabled elements are flagged (the template has a disabled Login request).
    assert!(s.disabled >= 1);
    // The tree starts collapsed and expands on demand.
    let mut t = TreeState::default();
    assert_eq!(t.rows(&p).len(), 1);
    t.expand_all(&p);
    assert_eq!(t.rows(&p).len(), p.len());
}

#[test]
fn plugin_plan_keeps_plugin_elements() {
    let p = fixture("PluginComplexExample.jmx");
    let s = p.summary();
    assert!(s.plugin >= 5, "expected plugin elements, found {}", s.plugin);
    assert_eq!(s.unknown, 0, "plugin elements here all end in a recognised suffix");
    assert!(p.iter().any(|n| n.tag.starts_with("kg.apc.") && n.category == Category::Sampler));
}

#[test]
fn every_real_parent_child_pair_obeys_the_rules() {
    let dirs = corpus_dirs();
    if dirs.is_empty() {
        eprintln!("skipped: no sample plans found (set JMX_CORPUS)");
        return;
    }
    let mut files = Vec::new();
    for d in &dirs {
        collect(d, &mut files);
    }
    assert!(!files.is_empty());
    let mut violations: Vec<String> = Vec::new();
    let mut unknown_core: std::collections::BTreeSet<String> = Default::default();
    let (mut plans, mut elements) = (0, 0);
    for f in &files {
        let plan = match Plan::load(f) {
            Ok(p) => p,
            Err(e) => panic!("{e:#}"),
        };
        plans += 1;
        elements += plan.len();
        for n in plan.iter() {
            if n.category == Category::Unknown && !n.tag.contains('.') {
                unknown_core.insert(n.tag.clone());
            }
            if let Some(pid) = n.parent {
                let parent = plan.node(pid).unwrap();
                let legacy = LEGACY_EXCEPTIONS.iter().any(|x| f.to_string_lossy().ends_with(x));
                if !legacy && !rules::can_add(parent.category, n.category) {
                    violations.push(format!("{}: {:?} '{}' under {:?} '{}'", f.display(), n.category, n.name, parent.category, parent.name));
                }
            }
        }
    }
    eprintln!("checked {plans} plans, {elements} elements");
    assert!(unknown_core.is_empty(), "core tags missing from the catalog: {unknown_core:?}");
    assert!(violations.is_empty(), "{} rule violations, first few:\n{}", violations.len(), violations.iter().take(8).cloned().collect::<Vec<_>>().join("\n"));
}

#[test]
fn every_core_tag_in_the_catalog_has_a_label_and_icon() {
    for tag in catalog::known_tags() {
        let (label, cat) = catalog::describe(tag, "");
        assert!(!label.is_empty(), "{tag}");
        assert_ne!(cat, Category::Unknown, "{tag}");
        assert!(cat.icon().ends_with(".svg"));
    }
}

/// JMeter loads these old test files even though its own add rules would reject their layout
/// (a controller directly under the Test Plan; a recorder under a thread group). Opening a plan must
/// never be blocked by the rules, which only govern edits.
const LEGACY_EXCEPTIONS: &[&str] = &["Modification Manager.jmx", "ProxyServerTestPlan.jmx"];

fn corpus_dirs() -> Vec<PathBuf> {
    if let Some(v) = std::env::var_os("JMX_CORPUS") {
        return std::env::split_paths(&v).filter(|p| p.is_dir()).collect();
    }
    let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).map(PathBuf::from);
    home.map(|h| h.join("Downloads").join("apache-jmeter-5.6.3")).filter(|p| p.is_dir()).into_iter().collect()
}

fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            collect(&p, out)
        } else if p.extension().is_some_and(|x| x == "jmx") {
            out.push(p)
        }
    }
}

#[test]
fn a_very_large_plan_loads_and_expands_quickly() {
    // 200 thread groups with 100 requests each: 20,200 elements.
    let mut s = String::from("<jmeterTestPlan><hashTree><TestPlan testname=\"Big\"/><hashTree>");
    for g in 0..200 {
        s.push_str(&format!("<ThreadGroup testname=\"G{g}\"/><hashTree>"));
        for r in 0..100 {
            s.push_str(&format!("<HTTPSamplerProxy testname=\"GET /g{g}/r{r}\"><stringProp name=\"HTTPSampler.path\">/g{g}/r{r}</stringProp></HTTPSamplerProxy><hashTree/>"));
        }
        s.push_str("</hashTree>");
    }
    s.push_str("</hashTree></hashTree></jmeterTestPlan>");
    let t = std::time::Instant::now();
    let plan = Plan::from_jmx_str(&s).unwrap();
    let load = t.elapsed();
    assert_eq!(plan.len(), 20_201);
    let t = std::time::Instant::now();
    let mut tree = TreeState::default();
    tree.expand_all(&plan);
    let rows = tree.rows(&plan);
    let expand = t.elapsed();
    assert_eq!(rows.len(), 20_201);
    let t = std::time::Instant::now();
    let hits = plan_core::search::search(&plan, "g150 r42", 20);
    let search = t.elapsed();
    assert!(!hits.is_empty());
    let t = std::time::Instant::now();
    let problems = plan_core::problems::check(&plan);
    let check = t.elapsed();
    assert!(problems.is_empty());
    eprintln!("load {load:?}, expand {expand:?}, search {search:?}, problems {check:?}");
    // Generous limits for a debug build on a slow machine; the point is to catch accidental O(n^2).
    assert!(load.as_secs_f32() < 3.0, "load took {load:?}");
    assert!(expand.as_secs_f32() < 2.0, "expand took {expand:?}");
    assert!(search.as_secs_f32() < 2.0, "search took {search:?}");
    assert!(check.as_secs_f32() < 3.0, "problems took {check:?}");
}
