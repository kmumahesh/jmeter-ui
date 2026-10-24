//! Safety of what is read and written: hostile text going through the editing operations, files that are
//! too large, too deep, links, read-only targets and odd names.

use plan_core::edit::EditablePlan;
use plan_core::files;
use plan_core::forms::{RowShape, new_row};
use std::path::PathBuf;

const HOSTILE: &[&str] = &[
    "</stringProp><evil/>",
    "\"><evil a=\"",
    "' onload='x",
    "]]><![CDATA[",
    "<!-- c -->",
    "a & b &amp; c &#38; d",
    "line one\r\nline two\ttab\nthree",
    "<![CDATA[x]]>",
    "&lt;already&gt;",
    "\u{e9}\u{1F600}\u{202e}rtl",
    "${__groovy(Runtime.getRuntime().exec('calc'))}",
];

fn fixture() -> String {
    std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/Test.jmx")).unwrap()
}

fn dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("jmeter-ui-sec-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// A plan with a thread group and an HTTP request in it.
fn plan_with_request() -> (EditablePlan, plan_core::NodeId, plan_core::NodeId) {
    let mut e = EditablePlan::open(&fixture()).unwrap();
    let root = e.plan().roots()[0];
    let tg = e.add(root, "ThreadGroup", "ThreadGroupGui").unwrap();
    let req = e.add(tg, "HTTPSamplerProxy", "HttpTestSampleGui").unwrap();
    (e, tg, req)
}

#[test]
fn hostile_text_through_every_edit_keeps_the_plan_structure() {
    for h in HOSTILE {
        let (mut e, tg, req) = plan_with_request();
        let before = fixture();
        let count = e.plan().len();
        e.rename(req, h).unwrap();
        e.set_prop(req, "HTTPSampler.domain", "stringProp", h).unwrap();
        e.set_prop(req, "TestPlan.comments", "stringProp", h).unwrap();
        e.set_prop(tg, "ThreadGroup.num_threads", "stringProp", h).unwrap();
        e.add_row(req, "HTTPsampler.Arguments/Arguments.arguments", 0, new_row(RowShape::HttpArgument, &[h, h])).unwrap();
        let text = e.to_text();

        let again = EditablePlan::open(&text).unwrap_or_else(|err| panic!("{h:?} gives a broken plan: {err:#}"));
        assert_eq!(again.plan().len(), count, "{h:?} added or removed elements");
        let node = again.plan().node(req).or_else(|| again.plan().iter().find(|n| n.name == *h)).unwrap();
        assert_eq!(node.name, *h, "the name comes back as typed");
        assert_eq!(node.prop("HTTPSampler.domain"), Some(*h));
        assert_eq!(node.comment(), Some(*h));
        let cells = again.row_cells(node.id, "HTTPsampler.Arguments/Arguments.arguments", 0);
        assert!(cells.iter().any(|c| c.1 == *h), "{h:?} in the table: {cells:?}");

        while e.can_undo() {
            e.undo().unwrap();
        }
        assert_eq!(&e.to_text(), &before, "{h:?}: undoing every change gives the first text back");
    }
}

#[test]
fn a_plan_nested_as_deep_as_allowed_still_builds_and_edits() {
    // Alternating test elements and hash trees, as far down as the limit lets the file go.
    let levels = jmx_model::xml::MAX_DEPTH / 2 - 3;
    let mut s = String::from("<?xml version=\"1.0\"?><jmeterTestPlan version=\"1.2\"><hashTree><TestPlan testname=\"T\"/><hashTree>");
    for i in 0..levels {
        s.push_str(&format!("<GenericController testname=\"c{i}\"/><hashTree>"));
    }
    for _ in 0..levels {
        s.push_str("</hashTree>");
    }
    s.push_str("</hashTree></hashTree></jmeterTestPlan>");
    let mut e = EditablePlan::open(&s).expect("opens");
    assert_eq!(e.plan().len(), levels + 1);
    let deepest = e.plan().find_by_name(&format!("c{}", levels - 1)).unwrap();
    e.rename(deepest, "renamed").unwrap();
    assert!(e.to_text().contains("renamed"));
    e.undo().unwrap();
    assert_eq!(e.to_text(), s);
}

#[test]
fn a_plan_nested_past_the_limit_is_refused_with_a_message() {
    let n = jmx_model::xml::MAX_DEPTH + 1;
    let s = format!("{}{}", "<hashTree>".repeat(n), "</hashTree>".repeat(n));
    let err = EditablePlan::open(&s).err().expect("refused");
    assert!(format!("{err:#}").contains("nested"));
}

#[test]
fn a_file_over_the_size_limit_is_refused_without_reading_it() {
    let d = dir("big");
    let p = d.join("huge.jmx");
    let f = std::fs::File::create(&p).unwrap();
    f.set_len(files::MAX_PLAN_BYTES + 1).unwrap();
    drop(f);
    let err = files::read_text(&p).expect_err("too large");
    assert!(err.to_string().contains("not opened"), "{err}");
    assert!(plan_core::Plan::load(&p).is_err());
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn a_file_that_is_not_text_is_an_error_not_a_crash() {
    let d = dir("binary");
    let p = d.join("x.jmx");
    std::fs::write(&p, [0xffu8, 0xfe, 0x00, 0x01, 0x80, 0x81]).unwrap();
    assert!(plan_core::Plan::load(&p).is_err());
    assert!(files::read_text(&d.join("missing.jmx")).is_err());
    assert!(files::read_text(&d).is_err(), "a folder is not a plan");
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn saving_never_writes_through_a_leftover_temporary_file_to_another_file() {
    // A temporary file left behind that is another name for an important file (a hard link needs no
    // special rights, unlike a symbolic link).
    let d = dir("tmplink");
    let victim = d.join("victim.txt");
    std::fs::write(&victim, "precious").unwrap();
    let plan = d.join("p.jmx");
    std::fs::hard_link(&victim, d.join("p.jmx.tmp")).unwrap();
    files::save(&plan, "new plan").unwrap();
    assert_eq!(std::fs::read_to_string(&victim).unwrap(), "precious", "the other file is untouched");
    assert_eq!(std::fs::read_to_string(&plan).unwrap(), "new plan");
    assert!(!d.join("p.jmx.tmp").exists());
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn values_that_span_several_lines_survive_duplicate_move_and_add() {
    let body = "line one\r\n    indented two\n\n  three  ";
    let (mut e, tg, req) = plan_with_request();
    e.set_prop(req, "HTTPSampler.domain", "stringProp", body).unwrap();
    let copy = e.duplicate(req).unwrap();
    let other = e.add(tg, "TransactionController", "TransactionControllerGui").unwrap();
    e.move_to(copy, other, 0).unwrap();
    let again = EditablePlan::open(&e.to_text()).unwrap();
    let n = again.plan().iter().filter(|n| n.tag == "HTTPSamplerProxy" && n.prop("HTTPSampler.domain") == Some(body)).count();
    assert_eq!(n, 2,"the original and the moved copy both keep the text exactly");
}

#[test]
fn saving_a_plan_that_is_a_link_writes_through_the_link() {
    let d = dir("planlink");
    let real = d.join("real.jmx");
    std::fs::write(&real, "old").unwrap();
    let link = d.join("link.jmx");
    #[cfg(windows)]
    let r = std::os::windows::fs::symlink_file(&real, &link);
    #[cfg(unix)]
    let r = std::os::unix::fs::symlink(&real, &link);
    if r.is_err() {
        eprintln!("skipped: this account cannot make links");
        let _ = std::fs::remove_dir_all(d);
        return;
    }
    files::save(&link, "new").unwrap();
    assert_eq!(std::fs::read_to_string(&real).unwrap(), "new");
    assert!(std::fs::symlink_metadata(&link).unwrap().file_type().is_symlink(), "the link is still a link");
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn the_recovery_copy_is_written_next_to_this_plan_only() {
    let d = dir("bakonly");
    let other = d.join("other.jmx.bak");
    std::fs::write(&other, "someone else's copy").unwrap();
    let plan = d.join("shop.jmx");
    files::autosave(&plan, "mine").unwrap();
    assert_eq!(std::fs::read_to_string(d.join("shop.jmx.bak")).unwrap(), "mine");
    assert_eq!(std::fs::read_to_string(&other).unwrap(), "someone else's copy");
    // A name with dots, spaces and a path-like look stays one file name in the same folder.
    let odd = d.join("a b..c.jmx");
    files::autosave(&odd, "x").unwrap();
    assert!(d.join("a b..c.jmx.bak").exists());
    assert_eq!(files::recovery_path(&odd).parent(), odd.parent());
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn saving_to_a_read_only_file_fails_cleanly_and_leaves_no_temporary_file() {
    let d = dir("readonly");
    let p = d.join("locked.jmx");
    std::fs::write(&p, "original").unwrap();
    let mut perm = std::fs::metadata(&p).unwrap().permissions();
    perm.set_readonly(true);
    std::fs::set_permissions(&p, perm.clone()).unwrap();
    let r = files::save(&p, "changed");
    #[allow(clippy::permissions_set_readonly_false)]
    perm.set_readonly(false);
    let content = std::fs::read_to_string(&p).unwrap();
    std::fs::set_permissions(&p, perm).unwrap();
    // On some systems a rename over a read-only file works; either way nothing is half done.
    match r {
        Err(_) => assert_eq!(content, "original"),
        Ok(()) => assert_eq!(content, "changed"),
    }
    assert!(!d.join("locked.jmx.tmp").exists(), "no temporary file is left behind");
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn saving_into_a_missing_folder_is_an_error() {
    let d = dir("missingdir");
    let p = d.join("no").join("such").join("p.jmx");
    assert!(files::save(&p, "x").is_err());
    assert!(files::autosave(&p, "x").is_err());
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn a_very_long_path_gives_an_error_or_works_but_never_panics() {
    let d = dir("longpath");
    let p = d.join("x".repeat(300)).join("p.jmx");
    let r = files::save(&p, "x");
    if r.is_ok() {
        assert_eq!(std::fs::read_to_string(&p).unwrap(), "x");
    }
    let long_name = d.join(format!("{}.jmx", "n".repeat(300)));
    let _ = files::save(&long_name, "x");
    let _ = std::fs::remove_dir_all(d);
}

#[cfg(windows)]
#[test]
fn a_reserved_windows_device_name_is_never_reported_as_saved() {
    let d = dir("reserved");
    for name in ["NUL.jmx", "CON.jmx", "aux.jmx", "COM1.jmx", "LPT1.jmx"] {
        let p = d.join(name);
        // A save that says it worked must have made a file that holds the text.
        if files::save(&p, "plan text").is_ok() {
            assert_eq!(std::fs::read_to_string(&p).ok().as_deref(), Some("plan text"), "{name} was reported as saved but holds nothing");
        }
    }
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn a_recovery_copy_that_is_not_text_is_ignored() {
    let d = dir("badbak");
    let plan = d.join("p.jmx");
    std::fs::write(&plan, "<a/>").unwrap();
    std::fs::write(files::recovery_path(&plan), [0xffu8, 0xfe, 0xfd]).unwrap();
    assert!(files::find_recovery(&plan).is_none());
    let _ = std::fs::remove_dir_all(d);
}
