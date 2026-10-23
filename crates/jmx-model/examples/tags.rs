//! List every element tag found in plan trees under the given folders, with counts and an example guiclass.
use jmx_model::{Document, plan::plan_tree};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() { collect(&p, out) } else if p.extension().is_some_and(|x| x == "jmx") { out.push(p) }
    }
}

fn main() {
    let mut files = Vec::new();
    for a in std::env::args().skip(1) { collect(Path::new(&a), &mut files) }
    let mut tags: BTreeMap<(String, String), (usize, String)> = BTreeMap::new();
    for f in files {
        let Ok(src) = std::fs::read_to_string(&f) else { continue };
        let Ok(doc) = Document::parse(&src) else { continue };
        let Ok(nodes) = plan_tree(&doc) else { continue };
        for n in &nodes {
            n.visit(&mut |p| {
                let e = tags.entry((p.tag.clone(), p.testclass.clone())).or_insert((0, p.guiclass.clone()));
                e.0 += 1;
            });
        }
    }
    for ((tag, testclass), (n, gui)) in &tags {
        println!("{n:5}  {tag}  |  {testclass}  |  {gui}");
    }
}
