//! Run the variable check over every plan under the given folders and print what it finds.
use plan_core::{EditablePlan, variables};
use std::path::{Path, PathBuf};

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

fn main() {
    let mut files = Vec::new();
    for a in std::env::args().skip(1) {
        collect(Path::new(&a), &mut files);
    }
    files.sort();
    let (mut plans, mut hints) = (0, 0);
    for f in &files {
        let Ok(src) = std::fs::read_to_string(f) else { continue };
        let Ok(e) = EditablePlan::open(&src) else { continue };
        let p = variables::check(&e);
        plans += 1;
        hints += p.len();
        for h in p.iter().take(4) {
            println!("{}: {}", f.file_name().unwrap().to_string_lossy(), h.text);
        }
    }
    println!("{plans} plans, {hints} hints");
}
