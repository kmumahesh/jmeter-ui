//! Round-trip every .jmx under the given folders: parse, write, compare bytes, build the plan tree.
use jmx_model::{Document, plan::plan_tree};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            collect(&p, out);
        } else if p.extension().is_some_and(|x| x == "jmx") {
            out.push(p);
        }
    }
}

fn main() {
    let mut files = Vec::new();
    for a in std::env::args().skip(1) {
        collect(Path::new(&a), &mut files);
    }
    files.sort();
    let t0 = Instant::now();
    let (mut ok, mut bytes_diff, mut parse_fail, mut plan_fail) = (0, 0, 0, 0);
    let mut elements = 0usize;
    let mut plugin_tags: BTreeMap<String, usize> = BTreeMap::new();
    let mut total_bytes = 0usize;
    for f in &files {
        let raw = match std::fs::read(f) {
            Ok(b) => b,
            Err(e) => {
                println!("READ FAIL {}: {e}", f.display());
                continue;
            }
        };
        total_bytes += raw.len();
        let Ok(src) = String::from_utf8(raw) else {
            println!("NOT UTF-8 {}", f.display());
            parse_fail += 1;
            continue;
        };
        match Document::parse(&src) {
            Err(e) => {
                println!("PARSE FAIL {}: {e:#}", f.display());
                parse_fail += 1
            }
            Ok(doc) => {
                if doc.write() != src {
                    println!("BYTE DIFF {}", f.display());
                    bytes_diff += 1
                } else {
                    ok += 1
                }
                match plan_tree(&doc) {
                    Ok(nodes) => {
                        for n in &nodes {
                            n.visit(&mut |p| {
                                elements += 1;
                                if p.is_plugin_class() {
                                    *plugin_tags.entry(p.tag.clone()).or_default() += 1
                                }
                            })
                        }
                    }
                    Err(e) => {
                        println!("PLAN FAIL {}: {e:#}", f.display());
                        plan_fail += 1
                    }
                }
            }
        }
    }
    println!(
        "\nfiles: {}  byte-identical: {ok}  byte-diff: {bytes_diff}  parse-fail: {parse_fail}  plan-tree-fail: {plan_fail}",
        files.len()
    );
    println!("elements in plan trees: {elements}  total size: {} KB  time: {:?}", total_bytes / 1024, t0.elapsed());
    println!("plugin/custom class tags found: {} distinct", plugin_tags.len());
    let mut v: Vec<_> = plugin_tags.into_iter().collect();
    v.sort_by(|a, b| b.1.cmp(&a.1));
    for (t, n) in v.iter().take(12) {
        println!("  {n:4}  {t}")
    }
}
