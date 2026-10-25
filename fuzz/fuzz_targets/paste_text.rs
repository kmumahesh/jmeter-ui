#![no_main]
//! Any text as something pasted into a plan: it never crashes or hangs, a refused paste changes nothing, and an
//! accepted one leaves a plan that opens and that one undo takes back exactly.

use libfuzzer_sys::fuzz_target;
use plan_core::starters::{self, Starter};
use plan_core::{EditablePlan, Plan};

fuzz_target!(|data: &[u8]| {
    let text = String::from_utf8_lossy(data);
    let mut plan = EditablePlan::open(&starters::new_plan_text(Starter::Api, "fuzz").unwrap()).unwrap();
    let before = plan.to_text();
    let at = plan.plan().find_by_name("Get items").unwrap();
    match plan.paste_near(at, &text) {
        Ok(_) => {
            assert!(Plan::from_jmx_str(&plan.to_text()).is_ok());
            plan.undo().unwrap();
            assert_eq!(plan.to_text(), before);
        }
        Err(_) => assert_eq!(plan.to_text(), before),
    }
});
