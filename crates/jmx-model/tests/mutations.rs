//! Damaged plans: real plans with random damage must never crash or hang the reader, and whatever it accepts is
//! saved back exactly as it came.

#[path = "mutate/mod.rs"]
mod mutate;

use jmx_model::xml::{Document, escape, parse_fragment, unescape};
use mutate::{Rng, mutate};
use std::time::{Duration, Instant};

const SEEDS: [&str; 3] = [
    include_str!("../../plan-core/tests/fixtures/Test.jmx"),
    include_str!("../../plan-core/tests/fixtures/build-adv-web-test-plan.jmx"),
    include_str!("../../plan-core/tests/fixtures/PluginComplexExample.jmx"),
];

#[test]
fn damaged_plans_never_crash_the_reader_and_what_it_accepts_is_saved_back_exactly() {
    let started = Instant::now();
    let (mut accepted, mut refused) = (0, 0);
    for (n, seed) in SEEDS.iter().enumerate() {
        let mut rng = Rng(0x9E37_79B9_7F4A_7C15 ^ (n as u64 + 1));
        for i in 0..400 {
            let text = String::from_utf8_lossy(&mutate(&mut rng, seed.as_bytes())).into_owned();
            match Document::parse(&text) {
                Ok(doc) => {
                    accepted += 1;
                    assert_eq!(doc.write(), text, "seed {n}, round {i}: an accepted file must be saved exactly as it came");
                }
                Err(_) => refused += 1,
            }
        }
    }
    eprintln!("accepted {accepted}, refused {refused}, in {:?}", started.elapsed());
    assert!(accepted > 0 && refused > 0, "the damage should produce both kinds of result");
    assert!(started.elapsed() < Duration::from_secs(120), "damaged input must not make the reader slow");
}

#[test]
fn damaged_fragments_and_text_never_crash_and_escaping_round_trips() {
    let mut rng = Rng(0xD1B5_4A32_D192_ED03);
    for i in 0..3000 {
        let bytes = mutate(&mut rng, SEEDS[i % 3].as_bytes().get(..400).unwrap_or(b"<a b='c'>text &amp; more</a>"));
        let text = String::from_utf8_lossy(&bytes).into_owned();
        let _ = parse_fragment(&text);
        assert_eq!(unescape(&escape(&text, false)), text, "round {i}: text");
        assert_eq!(unescape(&escape(&text, true)), text, "round {i}: attribute");
        let _ = unescape(&text); // any text, including broken references, is accepted
    }
}
