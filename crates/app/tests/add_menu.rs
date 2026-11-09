//! The "Add" list of the right-click menu in the tree: every row in it must add its element, to the element that was
//! right-clicked, wherever that element is and whatever else is selected.

mod common;

use common::*;
use gpui::TestAppContext;
use plan_core::catalog::Category;
use shell_core::menus::add_groups;

fn select(app: &mut App, name: &str) {
    let shell = app.shell.clone();
    let name = name.to_string();
    app.cx.update(|window, cx| {
        shell.update(cx, |s, cx| {
            let id = s.plan().unwrap().plan.find_by_name(&name).unwrap_or_else(|| panic!("no element {name}"));
            s.select_node(id, window, cx);
        })
    });
    app.cx.run_until_parked();
}

/// What the menu does when a row is chosen: add the element under the selected one.
fn choose(app: &mut App, tag: &'static str, gui: &'static str) {
    let shell = app.shell.clone();
    app.cx.update(|window, cx| shell.update(cx, |s, cx| s.add_element(tag, gui, window, cx)));
    app.cx.run_until_parked();
}

fn elements(app: &mut App) -> u64 {
    app.state()["elements"].as_u64().unwrap()
}

fn category_of(app: &mut App, name: &str) -> Category {
    let shell = app.shell.clone();
    let name = name.to_string();
    app.cx.update(|_, cx| {
        let s = shell.read(cx);
        let p = &s.plan().unwrap().plan;
        p.node(p.find_by_name(&name).unwrap()).unwrap().category
    })
}

#[gpui::test]
fn setup_and_teardown_thread_groups_are_added_from_the_menu_of_the_test_plan(cx: &mut gpui::TestAppContext) {
    let mut app = start(cx, Some(fixture(WEB)), None);
    let before = elements(&mut app);
    // Something else is selected when the Test Plan is right-clicked: the menu selects the Test Plan first.
    select(&mut app, "View Results Tree");
    select(&mut app, "build-adv-web-test-plan");
    let groups = add_groups(Some(Category::TestPlan));
    let thread = groups.iter().find(|g| g.category == Category::ThreadGroup).unwrap();
    for (i, label) in ["setUp Thread Group", "tearDown Thread Group"].into_iter().enumerate() {
        let t = thread.types.iter().find(|t| t.label == label).unwrap_or_else(|| panic!("{label} is not in the list"));
        choose(&mut app, t.tag, t.gui.unwrap_or_default());
        assert_eq!(elements(&mut app), before + 1 + i as u64, "{label} was not added");
        assert_eq!(app.selected().as_deref(), Some(label), "the new element is selected");
        assert!(app.children_of("build-adv-web-test-plan").iter().any(|n| n == label), "{label} is under the Test Plan");
        select(&mut app, "build-adv-web-test-plan");
    }
}

/// Every row of every Add list, for one element of each kind in the plan: the row adds exactly one element of that
/// type under the element, and Undo takes it away again. A row that did nothing would fail here.
#[gpui::test]
fn every_row_of_every_add_list_adds_its_element(cx: &mut TestAppContext) {
    let mut app = start(cx, Some(fixture(WEB)), None);
    let probes = [
        "build-adv-web-test-plan",    // Test Plan
        "JMeter Users",               // Thread Group (checked below)
        "Home",                       // a transaction or simple controller, whatever the plan has under that name
        "View Results Tree",          // Listener
    ];
    let mut tried = 0;
    let mut kinds = std::collections::HashSet::new();
    for name in probes {
        let exists = {
            let shell = app.shell.clone();
            let n = name.to_string();
            app.cx.update(|_, cx| shell.read(cx).plan().unwrap().plan.find_by_name(&n).is_some())
        };
        if !exists {
            continue;
        }
        select(&mut app, name);
        let category = category_of(&mut app, name);
        kinds.insert(category);
        let before = elements(&mut app);
        for g in add_groups(Some(category)) {
            for t in &g.types {
                select(&mut app, name);
                choose(&mut app, t.tag, t.gui.unwrap_or_default());
                assert_eq!(elements(&mut app), before + 1, "adding {} under {name} ({category:?}) did nothing", t.label);
                tried += 1;
                app.keys("ctrl-z");
                app.cx.run_until_parked();
                assert_eq!(elements(&mut app), before, "undo after adding {}", t.label);
            }
        }
    }
    assert!(kinds.len() >= 2, "the plan should have shown more than one kind of element: {kinds:?}");
    assert!(tried > 100, "only {tried} rows were tried");
}
