//! Copying elements as text and pasting them: into the same plan, into another, next to or under the selection,
//! in one undo step, and what happens with text that is not elements, is hostile, or is huge.

use plan_core::{Category, EditablePlan, NodeId, Plan};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

fn fixture(name: &str) -> String {
    std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name)).unwrap()
}

fn open(name: &str) -> EditablePlan {
    EditablePlan::open(&fixture(name)).unwrap()
}

const SMALL: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<jmeterTestPlan version=\"1.2\" properties=\"5.0\" jmeter=\"5.6.3\">\n  <hashTree>\n    <TestPlan guiclass=\"TestPlanGui\" testclass=\"TestPlan\" testname=\"Shop\" enabled=\"true\"/>\n    <hashTree>\n      <ThreadGroup guiclass=\"ThreadGroupGui\" testclass=\"ThreadGroup\" testname=\"Users\" enabled=\"true\"/>\n      <hashTree>\n        <HTTPSamplerProxy guiclass=\"HttpTestSampleGui\" testclass=\"HTTPSamplerProxy\" testname=\"Home\" enabled=\"true\">\n          <stringProp name=\"HTTPSampler.path\">/home</stringProp>\n        </HTTPSamplerProxy>\n        <hashTree>\n          <ResponseAssertion guiclass=\"AssertionGui\" testclass=\"ResponseAssertion\" testname=\"Check\" enabled=\"true\"/>\n          <hashTree/>\n        </hashTree>\n        <HTTPSamplerProxy guiclass=\"HttpTestSampleGui\" testclass=\"HTTPSamplerProxy\" testname=\"Cart\" enabled=\"true\"/>\n        <hashTree/>\n      </hashTree>\n      <ConstantTimer guiclass=\"ConstantTimerGui\" testclass=\"ConstantTimer\" testname=\"Think\" enabled=\"true\"/>\n      <hashTree/>\n    </hashTree>\n  </hashTree>\n</jmeterTestPlan>\n";

fn small() -> EditablePlan {
    EditablePlan::open(SMALL).unwrap()
}

fn id(e: &EditablePlan, name: &str) -> NodeId {
    e.plan().find_by_name(name).unwrap_or_else(|| panic!("no {name}"))
}

fn names_under(e: &EditablePlan, parent: &str) -> Vec<String> {
    e.plan().node(id(e, parent)).unwrap().children.iter().map(|&c| e.plan().node(c).unwrap().name.clone()).collect()
}

fn all_names(e: &EditablePlan) -> Vec<String> {
    e.plan().iter().map(|n| n.name.clone()).collect()
}

fn subtree_size(p: &Plan, id: NodeId) -> usize {
    1 + p.node(id).unwrap().children.iter().map(|&c| subtree_size(p, c)).sum::<usize>()
}

/// A plan's shape as nested (tag, name, settings) so two subtrees can be compared whatever their ids.
fn shape(p: &Plan, id: NodeId) -> String {
    let n = p.node(id).unwrap();
    let props: Vec<String> = n.props.iter().map(|x| format!("{}={}", x.name, x.value)).collect();
    let kids: Vec<String> = n.children.iter().map(|&c| shape(p, c)).collect();
    format!("{}[{}]({})<{}>", n.tag, n.name, props.join(";"), kids.join(","))
}

// ---- copying ----

#[test]
fn a_copy_is_a_plan_file_that_holds_just_the_elements() {
    let e = small();
    let text = e.copy_text(&[id(&e, "Home")]).unwrap();
    assert!(EditablePlan::open(&text).is_err() || EditablePlan::open(&text).is_ok(), "reading it back never panics");
    assert!(text.starts_with("<?xml"), "{text}");
    assert!(text.contains("<jmeterTestPlan") && text.contains("testname=\"Home\"") && text.contains("testname=\"Check\""), "{text}");
    assert!(!text.contains("testname=\"Shop\"") && !text.contains("testname=\"Cart\""), "only what was copied: {text}");
}

#[test]
fn the_test_plan_cannot_be_copied_and_nothing_cannot_either() {
    let e = small();
    assert!(e.copy_text(&[id(&e, "Shop")]).is_err());
    assert!(e.copy_text(&[]).is_err());
    assert!(e.copy_text(&[NodeId(9999)]).is_err());
}

#[test]
fn copying_changes_nothing() {
    let e = small();
    let before = e.to_text();
    e.copy_text(&[id(&e, "Users")]).unwrap();
    assert_eq!(e.to_text(), before);
    assert!(!e.is_dirty());
}

#[test]
fn several_elements_copy_in_the_order_given() {
    let e = small();
    let text = e.copy_text(&[id(&e, "Cart"), id(&e, "Home")]).unwrap();
    let mut other = small();
    let users = id(&other, "Users");
    other.paste_text(users, 0, &text).unwrap();
    assert_eq!(names_under(&other, "Users")[..2], ["Cart", "Home"]);
}

// ---- pasting ----

#[test]
fn a_pasted_subtree_is_the_same_as_the_copied_one_and_the_original_is_untouched() {
    let src = small();
    let text = src.copy_text(&[id(&src, "Home")]).unwrap();
    let mut dst = small();
    let users = id(&dst, "Users");
    let made = dst.paste_text(users, 99, &text).unwrap();
    assert_eq!(made.len(), 1);
    assert_eq!(shape(dst.plan(), made[0]), shape(src.plan(), id(&src, "Home")));
    assert_eq!(names_under(&dst, "Users"), ["Home", "Cart", "Home"], "pasted last among its new siblings");
    assert!(dst.is_dirty());
}

#[test]
fn pasting_gives_every_new_element_its_own_id() {
    let mut e = small();
    let text = e.copy_text(&[id(&e, "Home")]).unwrap();
    let users = id(&e, "Users");
    let before: std::collections::HashSet<NodeId> = e.plan().iter().map(|n| n.id).collect();
    let made = e.paste_text(users, 0, &text).unwrap();
    for &m in &made {
        assert!(!before.contains(&m));
    }
    let ids: std::collections::HashSet<NodeId> = e.plan().iter().map(|n| n.id).collect();
    assert_eq!(ids.len(), e.plan().len(), "no id is used twice");
    assert_eq!(e.plan().len(), before.len() + 2, "the request and its check came");
}

#[test]
fn the_same_text_can_be_pasted_again_and_again() {
    let mut e = small();
    let text = e.copy_text(&[id(&e, "Home")]).unwrap();
    let users = id(&e, "Users");
    for _ in 0..3 {
        e.paste_text(users, 0, &text).unwrap();
    }
    assert_eq!(names_under(&e, "Users").iter().filter(|n| *n == "Home").count(), 4);
    assert!(Plan::from_jmx_str(&e.to_text()).is_ok());
}

#[test]
fn the_position_decides_where_it_goes() {
    let mut e = small();
    let text = e.copy_text(&[id(&e, "Cart")]).unwrap();
    let users = id(&e, "Users");
    e.paste_text(users, 0, &text).unwrap();
    assert_eq!(names_under(&e, "Users")[0], "Cart", "at the front");
    e.paste_text(users, 1, &text).unwrap();
    assert_eq!(names_under(&e, "Users")[..3], ["Cart", "Cart", "Home"]);
}

#[test]
fn a_paste_is_one_undo_step_and_redo_brings_it_back() {
    let mut e = small();
    let before = e.to_text();
    let text = e.copy_text(&[id(&e, "Home"), id(&e, "Cart")]).unwrap();
    let users = id(&e, "Users");
    let made = e.paste_text(users, 1, &text).unwrap();
    assert_eq!(made.len(), 2);
    let after = e.to_text();
    assert!(e.can_undo());
    let out = e.undo().unwrap().unwrap();
    assert_eq!(out.select, Some(users), "the element it went under is selected");
    assert_eq!(e.to_text(), before, "one undo takes both elements away, byte for byte");
    assert!(!e.can_undo() && !e.is_dirty());
    let out = e.redo().unwrap().unwrap();
    assert_eq!(out.select, Some(made[0]));
    assert_eq!(e.to_text(), after, "redo gives them back exactly");
    e.undo().unwrap();
    assert_eq!(e.to_text(), before);
}

#[test]
fn a_whole_plan_pasted_brings_what_is_inside_its_test_plan() {
    let mut e = small();
    let other = fixture("Test.jmx");
    let root = e.plan().roots()[0];
    let inside: Vec<String> = {
        let o = EditablePlan::open(&other).unwrap();
        let r = o.plan().roots()[0];
        o.plan().node(r).unwrap().children.iter().map(|&c| o.plan().node(c).unwrap().name.clone()).collect()
    };
    let made = e.paste_text(root, 0, &other).unwrap();
    assert_eq!(made.len(), inside.len());
    let got: Vec<String> = made.iter().map(|&m| e.plan().node(m).unwrap().name.clone()).collect();
    assert_eq!(got, inside, "its children, in order, and not its Test Plan");
    assert_eq!(all_names(&e).iter().filter(|n| n.as_str() == "Shop").count(), 1, "its Test Plan did not come along");
}

// ---- where an element fits ----

#[test]
fn paste_near_goes_under_a_container_and_next_to_a_leaf() {
    let mut e = small();
    let text = e.copy_text(&[id(&e, "Cart")]).unwrap();
    // A request under a thread group: it goes last among the group's children.
    let made = e.paste_near(id(&e, "Users"), &text).unwrap();
    assert_eq!(e.plan().node(made[0]).unwrap().parent, Some(id(&e, "Users")));
    assert_eq!(names_under(&e, "Users").last().unwrap(), "Cart");
    // With a request selected it cannot go under it, so it goes right after it.
    let made = e.paste_near(id(&e, "Home"), &text).unwrap();
    let users = e.plan().node(id(&e, "Users")).unwrap();
    let at = users.children.iter().position(|&c| c == id(&e, "Home")).unwrap();
    assert_eq!(users.children[at + 1], made[0]);
}

#[test]
fn paste_near_looks_further_up_when_it_does_not_fit_next_to_the_selection() {
    let mut e = small();
    let text = e.copy_text(&[id(&e, "Users")]).unwrap();
    // A thread group cannot go under a request, nor in a thread group: it goes after the group, under the plan.
    let made = e.paste_near(id(&e, "Home"), &text).unwrap();
    assert_eq!(e.plan().node(made[0]).unwrap().parent, Some(e.plan().roots()[0]));
    let top = names_under(&e, "Shop");
    assert_eq!(top[0], "Users");
    assert_eq!(top[1], "Users", "right after the group the paste started from");
}

#[test]
fn what_cannot_go_there_is_refused_and_changes_nothing() {
    let mut e = small();
    let text = e.copy_text(&[id(&e, "Home")]).unwrap();
    let before = e.to_text();
    // A request cannot go under another request, a timer or the Test Plan.
    for target in ["Cart", "Think", "Shop"] {
        let err = e.paste_text(id(&e, target), 0, &text).unwrap_err().to_string();
        assert!(err.contains("cannot go under"), "{target}: {err}");
    }
    assert_eq!(e.to_text(), before);
    assert!(!e.can_undo());
    // The same through paste_near when no ancestor takes it (the Test Plan is the top).
    let err = e.paste_near(id(&e, "Shop"), &text).unwrap_err().to_string();
    assert!(err.contains("cannot go under"), "{err}");
    assert_eq!(e.to_text(), before);
}

#[test]
fn a_mixed_paste_is_all_or_nothing() {
    let mut e = small();
    let text = e.copy_text(&[id(&e, "Think"), id(&e, "Home")]).unwrap();
    let before = e.to_text();
    // A timer may go under the Test Plan, a request may not.
    assert!(e.paste_text(e.plan().roots()[0], 0, &text).is_err());
    assert_eq!(e.to_text(), before, "the timer did not go in alone");
}

// ---- text that is not elements ----

#[test]
fn text_that_is_not_a_plan_is_refused_with_a_plain_message() {
    let mut e = small();
    let users = id(&e, "Users");
    let before = e.to_text();
    for junk in ["", "   ", "hello there", "{\"json\": true}", "<html><body>hi</body></html>", "<jmeterTestPlan/>", "<jmeterTestPlan><hashTree/></jmeterTestPlan>", "\u{0}\u{1}\u{2}", "<<<>>>", "<a>"] {
        let err = e.paste_text(users, 0, junk).unwrap_err().to_string();
        assert!(err.contains("not text copied from a JMeter plan") || err.contains("no plan elements"), "{junk:?}: {err}");
    }
    assert_eq!(e.to_text(), before);
    assert!(!e.can_undo());
}

#[test]
fn a_test_plan_with_nothing_in_it_is_refused() {
    let mut e = small();
    let users = id(&e, "Users");
    let empty = "<jmeterTestPlan><hashTree><TestPlan guiclass=\"TestPlanGui\" testclass=\"TestPlan\" testname=\"X\"/><hashTree/></hashTree></jmeterTestPlan>";
    assert!(e.paste_text(users, 0, empty).unwrap_err().to_string().contains("no plan elements"));
}

#[test]
fn hostile_texts_are_refused_or_read_safely() {
    let mut e = small();
    let users = id(&e, "Users");
    let before = e.to_text();
    let mut bomb = String::from("<?xml version=\"1.0\"?><!DOCTYPE lolz [<!ENTITY a0 \"lol\">");
    for i in 1..10 {
        bomb.push_str(&format!("<!ENTITY a{i} \"{}\">", format!("&a{};", i - 1).repeat(10)));
    }
    bomb.push_str("]><jmeterTestPlan><hashTree><ConstantTimer guiclass=\"ConstantTimerGui\" testclass=\"ConstantTimer\" testname=\"&a9;\"/><hashTree/></hashTree></jmeterTestPlan>");
    let external = "<?xml version=\"1.0\"?><!DOCTYPE p [<!ENTITY x SYSTEM \"file:///etc/passwd\">]><jmeterTestPlan><hashTree><ConstantTimer testname=\"&x;\"/><hashTree/></hashTree></jmeterTestPlan>";
    let deep = format!("{}{}", "<hashTree>".repeat(200_000), "</hashTree>".repeat(200_000));
    for (name, text) in [("entity bomb", bomb), ("external entity", external.to_string()), ("deep nesting", format!("<jmeterTestPlan>{deep}</jmeterTestPlan>"))] {
        let t = Instant::now();
        let r = e.paste_text(users, 0, &text);
        assert!(t.elapsed() < Duration::from_secs(10), "{name} took {:?}", t.elapsed());
        if let Ok(made) = r {
            // If it was read at all, it is text only: nothing was expanded or fetched.
            for m in made {
                let n = e.plan().node(m).unwrap();
                assert!(n.name.len() < 200 && !n.name.contains("root:"), "{name}: {}", n.name);
            }
            e.undo().unwrap();
        }
        assert_eq!(e.to_text(), before, "{name}");
    }
}

#[test]
fn an_element_with_markup_in_its_name_stays_one_element() {
    let mut e = small();
    let home = id(&e, "Home");
    e.rename(home, "a </stringProp><evil/> & ]]> b").unwrap();
    let text = e.copy_text(&[home]).unwrap();
    let users = id(&e, "Users");
    let made = e.paste_text(users, 0, &text).unwrap();
    assert_eq!(e.plan().node(made[0]).unwrap().name, "a </stringProp><evil/> & ]]> b");
    assert_eq!(e.plan().len(), 8, "the name did not add elements");
    assert!(Plan::from_jmx_str(&e.to_text()).is_ok());
}

#[test]
fn a_paste_with_too_many_elements_or_too_much_text_is_refused() {
    let mut e = small();
    let users = id(&e, "Users");
    let sampler = "<HTTPSamplerProxy guiclass=\"HttpTestSampleGui\" testclass=\"HTTPSamplerProxy\" testname=\"s\"/><hashTree/>";
    let many = format!("<jmeterTestPlan><hashTree>{}</hashTree></jmeterTestPlan>", sampler.repeat(plan_core::edit::MAX_PASTE_ELEMENTS + 1));
    let err = e.paste_text(users, 0, &many).unwrap_err().to_string();
    assert!(err.contains("at most"), "{err}");
    let huge = " ".repeat(plan_core::edit::MAX_PASTE_BYTES + 1);
    assert!(e.paste_text(users, 0, &huge).unwrap_err().to_string().contains("too big"));
}

#[test]
fn elements_of_plugins_are_copied_and_pasted_like_any_other() {
    let plan = "<jmeterTestPlan><hashTree><TestPlan guiclass=\"TestPlanGui\" testclass=\"TestPlan\" testname=\"P\"/><hashTree><ThreadGroup guiclass=\"ThreadGroupGui\" testclass=\"ThreadGroup\" testname=\"G\"/><hashTree><kg.apc.jmeter.samplers.Thing guiclass=\"kg.apc.jmeter.Gui\" testclass=\"kg.apc.jmeter.Thing\" testname=\"Plugin thing\"><stringProp name=\"x\">1</stringProp></kg.apc.jmeter.samplers.Thing><hashTree/></hashTree></hashTree></hashTree></jmeterTestPlan>";
    let mut e = EditablePlan::open(plan).unwrap();
    let text = e.copy_text(&[id(&e, "Plugin thing")]).unwrap();
    let g = id(&e, "G");
    let made = e.paste_text(g, 0, &text).unwrap();
    assert_eq!(e.plan().node(made[0]).unwrap().name, "Plugin thing");
    assert!(e.to_text().matches("kg.apc.jmeter.samplers.Thing").count() >= 4);
}

// ---- real plans ----

fn corpus_plans() -> Vec<(String, String)> {
    let mut out = Vec::new();
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    for entry in std::fs::read_dir(dir).unwrap().flatten() {
        let p = entry.path();
        if p.extension().is_some_and(|x| x == "jmx") {
            out.push((p.file_name().unwrap().to_string_lossy().to_string(), std::fs::read_to_string(&p).unwrap()));
        }
    }
    let home = std::env::var_os("JMETER_HOME").map(PathBuf::from).or_else(|| std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).map(|h| PathBuf::from(h).join("Downloads").join("apache-jmeter-5.6.3")));
    if let Some(h) = home {
        for sub in ["bin/templates", "extras"] {
            if let Ok(rd) = std::fs::read_dir(h.join(sub)) {
                for entry in rd.flatten() {
                    let p = entry.path();
                    if p.extension().is_some_and(|x| x == "jmx") && let Ok(t) = std::fs::read_to_string(&p) {
                        out.push((p.file_name().unwrap().to_string_lossy().to_string(), t));
                    }
                }
            }
        }
    }
    out
}

#[test]
fn copying_and_pasting_every_element_of_every_sample_plan_works_and_undoes_exactly() {
    let plans = corpus_plans();
    assert!(plans.len() >= 4);
    let (mut elements, mut pasted_ok, mut refused) = (0, 0, 0);
    for (name, text) in &plans {
        let Ok(mut e) = EditablePlan::open(text) else { continue };
        let original = e.to_text();
        let ids: Vec<NodeId> = e.plan().iter().map(|n| n.id).collect();
        for id in ids {
            let n = e.plan().node(id).unwrap();
            if n.category == Category::TestPlan {
                continue;
            }
            elements += 1;
            let size = subtree_size(e.plan(), id);
            let shape_before = shape(e.plan(), id);
            let clip = e.copy_text(&[id]).unwrap_or_else(|err| panic!("{name}: copy {}: {err:#}", n.name));
            let len = e.plan().len();
            match e.paste_near(id, &clip) {
                Ok(made) => {
                    pasted_ok += 1;
                    assert_eq!(e.plan().len(), len + size, "{name}: pasting {} brought the wrong number of elements", e.plan().node(made[0]).unwrap().name);
                    assert_eq!(shape(e.plan(), made[0]), shape_before, "{name}: the copy of an element differs from it");
                    assert!(Plan::from_jmx_str(&e.to_text()).is_ok(), "{name}: the result is not a plan");
                    e.undo().unwrap();
                    assert_eq!(e.to_text(), original, "{name}: undoing a paste of {} did not give the file back", e.plan().node(id).unwrap().name);
                }
                Err(_) => {
                    refused += 1;
                    assert_eq!(e.to_text(), original, "{name}: a refused paste changed the plan");
                }
            }
        }
    }
    eprintln!("{} plans, {elements} elements: {pasted_ok} pasted, {refused} refused", plans.len());
    assert!(pasted_ok > 50, "{pasted_ok}");
}

#[test]
fn a_copy_of_a_whole_thread_group_pasted_into_another_plan_runs_the_same_shape() {
    let a = open("build-adv-web-test-plan.jmx");
    let group = a.plan().iter().find(|n| n.category == Category::ThreadGroup).unwrap().id;
    let text = a.copy_text(&[group]).unwrap();
    let mut b = open("Test.jmx");
    let root = b.plan().roots()[0];
    let len = b.plan().len();
    let made = b.paste_text(root, 0, &text).unwrap();
    assert_eq!(shape(b.plan(), made[0]), shape(a.plan(), group));
    assert_eq!(b.plan().len(), len + subtree_size(a.plan(), group));
}

// ---- speed ----

#[test]
fn copying_and_pasting_a_big_subtree_is_quick() {
    let sampler = "<HTTPSamplerProxy guiclass=\"HttpTestSampleGui\" testclass=\"HTTPSamplerProxy\" testname=\"Request N\" enabled=\"true\"><stringProp name=\"HTTPSampler.path\">/item/N</stringProp></HTTPSamplerProxy><hashTree/>";
    let plan = format!(
        "<jmeterTestPlan><hashTree><TestPlan guiclass=\"TestPlanGui\" testclass=\"TestPlan\" testname=\"Big\"/><hashTree><ThreadGroup guiclass=\"ThreadGroupGui\" testclass=\"ThreadGroup\" testname=\"G\"/><hashTree>{}</hashTree></hashTree></hashTree></jmeterTestPlan>",
        sampler.repeat(5000)
    );
    let mut e = EditablePlan::open(&plan).unwrap();
    let g = id(&e, "G");
    let t = Instant::now();
    let text = e.copy_text(&[g]).unwrap();
    let copy_time = t.elapsed();
    let root = e.plan().roots()[0];
    let t = Instant::now();
    e.paste_text(root, 1, &text).unwrap();
    let paste_time = t.elapsed();
    assert_eq!(e.plan().len(), 5002 + 5001, "{}", e.plan().len());
    let t = Instant::now();
    e.undo().unwrap();
    e.redo().unwrap();
    let undo_time = t.elapsed();
    eprintln!("copy {copy_time:?}, paste {paste_time:?}, undo and redo {undo_time:?}");
    assert!(copy_time < Duration::from_secs(10) && paste_time < Duration::from_secs(20) && undo_time < Duration::from_secs(20));
}

// ---- damaged copies ----

#[test]
fn damaged_copies_are_refused_or_pasted_cleanly_and_never_change_a_plan_by_halves() {
    let src = open("build-adv-web-test-plan.jmx");
    let group = src.plan().iter().find(|n| n.category == Category::ThreadGroup).unwrap().id;
    let original = src.copy_text(&[group]).unwrap();
    let mut seed = 0x5EED_C0DE_1234_5678u64;
    let mut next = move |n: usize| {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((seed >> 33) as usize) % n.max(1)
    };
    let (mut pasted, mut refused) = (0, 0);
    for round in 0..1500 {
        let mut bytes = original.clone().into_bytes();
        for _ in 0..1 + next(4) {
            let at = next(bytes.len());
            match next(5) {
                0 => bytes[at] = next(256) as u8,
                1 => {
                    let end = (at + next(40)).min(bytes.len());
                    bytes.drain(at..end);
                }
                2 => {
                    let end = (at + next(60)).min(bytes.len());
                    let piece = bytes[at..end].to_vec();
                    bytes.splice(at..at, piece);
                }
                3 => bytes.truncate(at),
                _ => bytes.splice(at..at, b"<!-- & ]]> </hashTree><x>".iter().copied()).for_each(drop),
            }
        }
        let text = String::from_utf8_lossy(&bytes).into_owned();
        let mut dst = open("Test.jmx");
        let before = dst.to_text();
        let root = dst.plan().roots()[0];
        match dst.paste_near(root, &text) {
            Ok(made) => {
                pasted += 1;
                assert!(!made.is_empty());
                assert!(Plan::from_jmx_str(&dst.to_text()).is_ok(), "round {round}: the result is not a plan");
                dst.undo().unwrap();
                assert_eq!(dst.to_text(), before, "round {round}: undo did not give the plan back");
            }
            Err(_) => {
                refused += 1;
                let now = dst.to_text();
                if now != before {
                    let at = now.bytes().zip(before.bytes()).position(|(a, b)| a != b).unwrap_or(now.len().min(before.len()));
                    let from = at.saturating_sub(80);
                    panic!("round {round}: a refused paste changed the plan at byte {at}:
  was: {:?}
  now: {:?}
  text pasted: {:?}", &before[from..(at + 120).min(before.len())], &now[from..(at + 120).min(now.len())], &text[..text.len().min(300)]);
                }
            }
        }
    }
    eprintln!("{pasted} pasted, {refused} refused");
    assert!(pasted > 0 && refused > 0, "the damage should give both kinds of result");
}
