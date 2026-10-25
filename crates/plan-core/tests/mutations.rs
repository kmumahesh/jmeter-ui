//! Damaged plans at the level above the XML reader: whatever the reader accepts must load as a plan that can be
//! walked, described and checked without a crash.

#[path = "../../jmx-model/tests/mutate/mod.rs"]
mod mutate;

use mutate::{Rng, mutate};
use plan_core::Plan;
use std::time::{Duration, Instant};

const SEEDS: [&str; 3] = [
    include_str!("fixtures/Test.jmx"),
    include_str!("fixtures/build-adv-web-test-plan.jmx"),
    include_str!("fixtures/PluginComplexExample.jmx"),
];

#[test]
fn damaged_plans_load_or_are_refused_and_a_loaded_plan_can_always_be_walked() {
    let started = Instant::now();
    let mut loaded = 0;
    for (n, seed) in SEEDS.iter().enumerate() {
        let mut rng = Rng(0xA076_1D64_78BD_642F ^ (n as u64 + 1));
        for _ in 0..300 {
            let text = String::from_utf8_lossy(&mutate(&mut rng, seed.as_bytes())).into_owned();
            let Ok(plan) = Plan::from_jmx_str(&text) else { continue };
            loaded += 1;
            for node in plan.iter() {
                let id = node.id;
                let _ = plan.breadcrumb(id);
                let _ = plan.path_to(id);
                let _ = plan.depth(id);
                let _ = plan.descendant_count(id);
            }
            assert!(plan.len() >= plan.roots().len());
        }
    }
    eprintln!("{loaded} damaged plans still loaded, in {:?}", started.elapsed());
    assert!(loaded > 0);
    assert!(started.elapsed() < Duration::from_secs(120));
}
