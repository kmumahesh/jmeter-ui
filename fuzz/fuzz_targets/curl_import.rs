#![no_main]
//! Any text as cURL commands: reading never crashes or hangs, and what is read always makes plan text that can be
//! pasted under a thread group.

use libfuzzer_sys::fuzz_target;
use plan_core::starters::{self, Starter};
use plan_core::{EditablePlan, curl};

fuzz_target!(|data: &[u8]| {
    let text = String::from_utf8_lossy(data);
    if let Ok(requests) = curl::parse(&text) {
        let made = curl::plan_text(&requests).expect("requests always make plan text");
        let mut plan = EditablePlan::open(&starters::new_plan_text(Starter::Api, "fuzz").unwrap()).unwrap();
        let group = plan.plan().find_by_name("Users").unwrap();
        plan.paste_text(group, 0, &made).expect("the text that was made can be pasted");
    }
});
