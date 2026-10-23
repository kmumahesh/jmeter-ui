//! Edit real plans at random and check the guarantees that matter: the text stays a valid plan after every
//! step, undoing everything gives the original file back byte for byte, and redoing gives the edits back.
//!
//! Set `JMX_CORPUS` to a list of folders (separated like PATH); without it the JMeter install in
//! `~/Downloads` is used, and the test is skipped if that is not there.

use plan_core::{EditablePlan, NodeId, Plan, catalog};
use std::path::{Path, PathBuf};

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

/// A small deterministic random number generator, so a failure can be repeated.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        self.0 >> 33
    }

    fn below(&mut self, n: usize) -> usize {
        if n == 0 { 0 } else { (self.next() % n as u64) as usize }
    }
}

fn all_ids(p: &Plan) -> Vec<NodeId> {
    p.iter().map(|n| n.id).collect()
}

/// Do one random edit. Returns false if the chosen edit was not allowed (which is fine).
fn random_edit(e: &mut EditablePlan, rng: &mut Rng, adds: &[catalog::AddableType]) -> bool {
    let ids = all_ids(e.plan());
    if ids.is_empty() {
        return false;
    }
    let pick = ids[rng.below(ids.len())];
    match rng.below(7) {
        0 | 1 => {
            let t = &adds[rng.below(adds.len())];
            e.add(pick, t.tag, t.gui.unwrap()).is_ok()
        }
        2 => e.remove(pick).is_ok(),
        3 => e.duplicate(pick).is_ok(),
        4 => {
            let target = ids[rng.below(ids.len())];
            let n = e.plan().node(target).map_or(0, |n| n.children.len());
            e.move_to(pick, target, rng.below(n + 1)).is_ok()
        }
        5 => e.rename(pick, &format!("n{} & <x> \"q\"", rng.below(1000))).is_ok(),
        _ => e.set_enabled(pick, rng.below(2) == 0).is_ok(),
    }
}

#[test]
fn random_edits_on_real_plans_stay_valid_and_undo_exactly() {
    let mut files = Vec::new();
    for d in corpus_dirs() {
        collect(&d, &mut files);
    }
    files.sort();
    if files.is_empty() {
        eprintln!("skipped: no sample plans found (set JMX_CORPUS)");
        return;
    }
    let adds = catalog::addable();
    let (mut edited, mut ops_done) = (0, 0);
    for (n, f) in files.iter().enumerate() {
        let Ok(src) = std::fs::read_to_string(f) else { continue };
        let Ok(mut e) = EditablePlan::open(&src) else { continue };
        assert_eq!(e.to_text(), src, "{} opens unchanged", f.display());
        let mut rng = Rng(0x9E3779B97F4A7C15 ^ n as u64);
        let mut done = 0;
        for _ in 0..40 {
            if random_edit(&mut e, &mut rng, &adds) {
                done += 1;
                let text = e.to_text();
                let again = Plan::from_jmx_str(&text).unwrap_or_else(|err| panic!("{}: not a valid plan after edit {done}: {err:#}", f.display()));
                assert_eq!(again.len(), e.plan().len(), "{}: the plan and the text agree after edit {done}", f.display());
            }
        }
        if done == 0 {
            continue;
        }
        edited += 1;
        ops_done += done;
        let result = e.to_text();
        while e.can_undo() {
            e.undo().unwrap();
        }
        assert_eq!(e.to_text(), src, "{}: undo of {done} edits is not byte-identical", f.display());
        assert!(!e.is_dirty());
        while e.can_redo() {
            e.redo().unwrap();
        }
        assert_eq!(e.to_text(), result, "{}: redo of {done} edits differs", f.display());
    }
    eprintln!("{edited} of {} plans edited, {ops_done} edits, all undone exactly", files.len());
    assert!(edited >= files.len() / 2, "too few plans accepted edits: {edited} of {}", files.len());
}

#[test]
fn editing_a_large_plan_is_fast_enough() {
    let mut s = String::from("<jmeterTestPlan><hashTree><TestPlan testname=\"Big\"/><hashTree>");
    for g in 0..200 {
        s.push_str(&format!("<ThreadGroup testname=\"G{g}\"/><hashTree>"));
        for r in 0..100 {
            s.push_str(&format!("<HTTPSamplerProxy testname=\"GET /g{g}/r{r}\"/><hashTree/>"));
        }
        s.push_str("</hashTree>");
    }
    s.push_str("</hashTree></hashTree></jmeterTestPlan>");
    let mut e = EditablePlan::open(&s).unwrap();
    let g = e.plan().find_by_name("G150").unwrap();
    let r = e.plan().find_by_name("GET /g150/r42").unwrap();
    let t = std::time::Instant::now();
    e.rename(r, "renamed").unwrap();
    let rename = t.elapsed();
    let t = std::time::Instant::now();
    e.add(g, "ConstantTimer", "ConstantTimerGui").unwrap();
    let add = t.elapsed();
    let t = std::time::Instant::now();
    e.undo().unwrap();
    e.undo().unwrap();
    let undo = t.elapsed();
    eprintln!("20k elements: rename {rename:?}, add {add:?}, two undos {undo:?}");
    // Debug builds on a slow machine; the point is that an edit is not a full re-read in disguise.
    assert!(rename.as_millis() < 1500, "rename took {rename:?}");
    assert!(add.as_millis() < 1500, "add took {add:?}");
    assert_eq!(e.to_text(), s);
}
