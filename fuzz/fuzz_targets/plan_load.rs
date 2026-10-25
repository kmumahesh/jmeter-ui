#![no_main]
//! Any bytes as a plan: loading never crashes, and a loaded plan can always be walked and described.

use libfuzzer_sys::fuzz_target;
use plan_core::Plan;

fuzz_target!(|data: &[u8]| {
    let text = String::from_utf8_lossy(data);
    if let Ok(plan) = Plan::from_jmx_str(&text) {
        for node in plan.iter() {
            let _ = plan.breadcrumb(node.id);
            let _ = plan.path_to(node.id);
            let _ = plan.depth(node.id);
            let _ = plan.descendant_count(node.id);
        }
    }
});
