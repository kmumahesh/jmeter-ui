//! What the command palette shows for a query.
//!
//! A prefix picks a mode: `>` commands, `@` elements in the plan, `+` add an element. With no prefix
//! the palette mixes commands, recent plans and matching elements.

use crate::actions::{Action, AppContext, Availability};
use plan_core::catalog::{self, AddableType, Category};
use plan_core::fuzzy::Fuzzy;
use plan_core::rules;
use plan_core::{NodeId, Plan};

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Mode {
    Everything,
    Commands,
    Elements,
    Add,
}

impl Mode {
    pub fn prefix(self) -> &'static str {
        match self {
            Mode::Everything => "",
            Mode::Commands => ">",
            Mode::Elements => "@",
            Mode::Add => "+",
        }
    }

    pub fn placeholder(self) -> &'static str {
        match self {
            Mode::Everything => "Search commands and elements. Type > for commands, @ for elements, + to add.",
            Mode::Commands => "Run a command",
            Mode::Elements => "Go to element",
            Mode::Add => "Add element under the selection",
        }
    }
}

/// Split the typed text into a mode and the search text.
pub fn parse_query(raw: &str) -> (Mode, &str) {
    let t = raw.trim_start();
    match t.chars().next() {
        Some('>') => (Mode::Commands, t[1..].trim_start()),
        Some('@') => (Mode::Elements, t[1..].trim_start()),
        Some('+') => (Mode::Add, t[1..].trim_start()),
        _ => (Mode::Everything, t.trim_end()),
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Target {
    Action(Action),
    Goto(NodeId),
    Add(AddableType),
    OpenRecent(String),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    pub target: Target,
    pub title: String,
    /// Smaller text after the title, for example the element type or a file path.
    pub detail: String,
    pub group: String,
    pub shortcut: Option<&'static str>,
    pub category: Option<Category>,
    pub disabled: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Ranked {
    pub item: Item,
    pub score: u32,
    /// Matched character positions in the title, for highlighting.
    pub positions: Vec<u32>,
}

/// Everything the palette needs to know about the app.
pub struct Input<'a> {
    pub context: AppContext,
    pub plan: Option<&'a Plan>,
    /// The element selected in the tree; adding goes under it.
    pub selected: Option<NodeId>,
    pub recent_files: &'a [String],
}

/// Results for `raw`, best first, at most `limit`.
pub fn results(input: &Input, raw: &str, limit: usize) -> Vec<Ranked> {
    let (mode, query) = parse_query(raw);
    let mut items: Vec<Item> = Vec::new();
    match mode {
        Mode::Commands => items.extend(action_items(input)),
        Mode::Elements => items.extend(element_items(input)),
        Mode::Add => items.extend(add_items(input, query)),
        Mode::Everything => {
            items.extend(action_items(input));
            if query.is_empty() {
                items.extend(recent_items(input).into_iter().take(5));
            } else {
                items.extend(recent_items(input));
                items.extend(element_items(input));
            }
        }
    }
    let mut fuzzy = Fuzzy::new();
    let titles: Vec<&str> = items.iter().map(|i| i.title.as_str()).collect();
    let ranked = fuzzy.rank(query, &titles);
    let mut out: Vec<Ranked> = ranked.into_iter().map(|m| Ranked { item: items[m.index].clone(), score: m.score, positions: m.positions }).collect();
    // Available items first, then an exact title, then titles that start with what was typed; within each
    // group keep the match order (already best first).
    let q = query.trim().to_lowercase();
    out.sort_by_key(|r| {
        let t = r.item.title.to_lowercase();
        let closeness = if q.is_empty() { 2 } else if t == q { 0 } else if t.starts_with(&q) { 1 } else { 2 };
        (r.item.disabled.is_some(), closeness)
    });
    if mode == Mode::Add && query.is_empty() {
        // Without a query, JMeter's quick-add list comes first.
        out.sort_by_key(|r| match &r.item.target {
            Target::Add(t) => (r.item.disabled.is_some(), catalog::quick_rank(t).unwrap_or(usize::MAX)),
            _ => (true, usize::MAX),
        });
    }
    out.truncate(limit);
    out
}

fn action_items(input: &Input) -> Vec<Item> {
    Action::ALL
        .iter()
        .map(|&a| Item {
            target: Target::Action(a),
            title: a.title().to_string(),
            detail: String::new(),
            group: a.group().label().to_string(),
            shortcut: a.shortcut(),
            category: None,
            disabled: match a.availability(&input.context) {
                Availability::Enabled => None,
                Availability::Disabled(why) => Some(why.to_string()),
            },
        })
        .collect()
}

fn recent_items(input: &Input) -> Vec<Item> {
    input
        .recent_files
        .iter()
        .map(|p| {
            let name = p.rsplit(['/', '\\']).next().unwrap_or(p).to_string();
            Item { target: Target::OpenRecent(p.clone()), title: name, detail: p.clone(), group: "Recent plans".into(), shortcut: None, category: None, disabled: None }
        })
        .collect()
}

fn element_items(input: &Input) -> Vec<Item> {
    let Some(plan) = input.plan else { return vec![] };
    plan.iter()
        .map(|n| Item {
            target: Target::Goto(n.id),
            title: n.name.clone(),
            detail: n.type_label.clone(),
            group: "Elements".into(),
            shortcut: None,
            category: Some(n.category),
            disabled: None,
        })
        .collect()
}

/// Types that can be added under the selection. With a query, types that do not fit are listed too,
/// marked unavailable with the reason.
fn add_items(input: &Input, query: &str) -> Vec<Item> {
    let Some(plan) = input.plan else { return vec![] };
    let parent = input.selected.and_then(|id| plan.node(id)).or_else(|| plan.roots().first().and_then(|&r| plan.node(r)));
    let Some(parent) = parent else { return vec![] };
    let mut out = Vec::new();
    for t in catalog::addable() {
        let fits = rules::can_add(parent.category, t.category);
        let disabled = if !input.context.can_edit {
            Some("Open a plan to add elements".to_string())
        } else if !fits {
            Some(format!("Cannot go under {} \"{}\"", parent.category.label().to_lowercase(), parent.name))
        } else {
            None
        };
        if !fits && query.is_empty() {
            continue;
        }
        out.push(Item { target: Target::Add(t.clone()), title: t.label.to_string(), detail: t.category.label().to_string(), group: t.category.plural().to_string(), shortcut: None, category: Some(t.category), disabled });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const PLAN: &str = r#"<jmeterTestPlan><hashTree>
<TestPlan testname="Shop"/><hashTree>
 <ThreadGroup testname="Checkout Users"/><hashTree>
  <HTTPSamplerProxy testname="POST /api/login"/><hashTree/>
  <JSONPostProcessor testname="token"/><hashTree/>
 </hashTree>
</hashTree></hashTree></jmeterTestPlan>"#;

    fn plan() -> Plan {
        Plan::from_jmx_str(PLAN).unwrap()
    }

    fn ctx() -> AppContext {
        AppContext { has_plan: true, tab_count: 2, can_edit: true, ..Default::default() }
    }

    fn input<'a>(plan: Option<&'a Plan>, selected: Option<NodeId>, recent: &'a [String], context: AppContext) -> Input<'a> {
        Input { context, plan, selected, recent_files: recent }
    }

    fn titles(r: &[Ranked]) -> Vec<&str> {
        r.iter().map(|r| r.item.title.as_str()).collect()
    }

    #[test]
    fn prefixes_pick_the_mode() {
        assert_eq!(parse_query(""), (Mode::Everything, ""));
        assert_eq!(parse_query("login"), (Mode::Everything, "login"));
        assert_eq!(parse_query("> split"), (Mode::Commands, "split"));
        assert_eq!(parse_query("  @tok"), (Mode::Elements, "tok"));
        assert_eq!(parse_query("+ext"), (Mode::Add, "ext"));
        assert_eq!(parse_query(">"), (Mode::Commands, ""));
        for m in [Mode::Everything, Mode::Commands, Mode::Elements, Mode::Add] {
            assert!(!m.placeholder().is_empty());
        }
    }

    #[test]
    fn commands_mode_lists_only_actions() {
        let p = plan();
        let r = results(&input(Some(&p), None, &[], ctx()), ">tab", 50);
        assert!(r.iter().all(|r| matches!(r.item.target, Target::Action(_))));
        let t = titles(&r);
        assert!(t.contains(&"Close Tab") && t.contains(&"Next Tab"), "{t:?}");
    }

    #[test]
    fn unavailable_commands_sink_to_the_bottom_with_a_reason() {
        let p = plan();
        let c = AppContext { has_plan: true, tab_count: 0, ..Default::default() };
        let r = results(&input(Some(&p), None, &[], c), ">", 100);
        let first_disabled = r.iter().position(|r| r.item.disabled.is_some()).unwrap();
        assert!(r[first_disabled..].iter().all(|r| r.item.disabled.is_some()), "all disabled items come last");
        let close = r.iter().find(|r| r.item.target == Target::Action(Action::CloseTab)).unwrap();
        assert_eq!(close.item.disabled.as_deref(), Some("No tab is open"));
    }

    #[test]
    fn elements_mode_finds_by_name_and_carries_the_type() {
        let p = plan();
        let r = results(&input(Some(&p), None, &[], ctx()), "@login", 10);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].item.detail, "HTTP Request");
        assert_eq!(r[0].item.category, Some(Category::Sampler));
        assert!(matches!(r[0].item.target, Target::Goto(_)));
        assert!(!r[0].positions.is_empty());
    }

    #[test]
    fn elements_mode_without_a_query_lists_the_tree_in_order() {
        let p = plan();
        let r = results(&input(Some(&p), None, &[], ctx()), "@", 10);
        assert_eq!(titles(&r), ["Shop", "Checkout Users", "POST /api/login", "token"]);
        assert!(results(&input(None, None, &[], AppContext::default()), "@", 10).is_empty());
    }

    #[test]
    fn plain_queries_mix_commands_recents_and_elements() {
        let p = plan();
        let recent = vec!["C:/plans/checkout.jmx".to_string()];
        let r = results(&input(Some(&p), None, &recent, ctx()), "check", 10);
        let kinds: Vec<_> = r.iter().map(|r| std::mem::discriminant(&r.item.target)).collect();
        assert!(r.iter().any(|r| matches!(r.item.target, Target::OpenRecent(_))));
        assert!(r.iter().any(|r| matches!(r.item.target, Target::Goto(_))), "{kinds:?}");
    }

    #[test]
    fn an_empty_query_shows_commands_and_a_few_recents_but_no_elements() {
        let p = plan();
        let recent: Vec<String> = (0..8).map(|i| format!("C:/p/{i}.jmx")).collect();
        let r = results(&input(Some(&p), None, &recent, ctx()), "", 100);
        assert_eq!(r.iter().filter(|r| matches!(r.item.target, Target::OpenRecent(_))).count(), 5);
        assert!(!r.iter().any(|r| matches!(r.item.target, Target::Goto(_))));
        assert!(r.iter().any(|r| matches!(r.item.target, Target::Action(_))));
    }

    #[test]
    fn recent_items_use_the_file_name_as_the_title() {
        let recent = vec!["C:\\work\\Shop Test.jmx".to_string()];
        let r = results(&input(None, None, &recent, AppContext::default()), "shop", 10);
        assert_eq!(r[0].item.title, "Shop Test.jmx");
        assert_eq!(r[0].item.detail, "C:\\work\\Shop Test.jmx");
    }

    #[test]
    fn add_mode_offers_only_what_fits_under_the_selection() {
        let p = plan();
        let sampler = p.find_by_name("POST /api/login").unwrap();
        let r = results(&input(Some(&p), Some(sampler), &[], ctx()), "+", 200);
        assert!(!r.is_empty());
        for x in &r {
            let c = x.item.category.unwrap();
            assert!(rules::can_add(Category::Sampler, c), "{:?} cannot go under a sampler", x.item.title);
            assert!(x.item.disabled.is_none());
        }
        assert!(!titles(&r).contains(&"Loop Controller"));
        assert!(titles(&r).contains(&"JSON Extractor"));
    }

    #[test]
    fn add_mode_with_a_query_also_lists_what_does_not_fit_as_unavailable() {
        let p = plan();
        let sampler = p.find_by_name("POST /api/login").unwrap();
        let r = results(&input(Some(&p), Some(sampler), &[], ctx()), "+loop", 20);
        let lc = r.iter().find(|r| r.item.title == "Loop Controller").expect("listed");
        let why = lc.item.disabled.as_deref().unwrap();
        assert!(why.contains("Cannot go under") && why.contains("POST /api/login"), "{why}");
        // Available matches come before unavailable ones.
        let first_dis = r.iter().position(|r| r.item.disabled.is_some());
        if let Some(i) = first_dis {
            assert!(r[i..].iter().all(|r| r.item.disabled.is_some()));
        }
    }

    #[test]
    fn add_mode_quick_list_comes_first_without_a_query() {
        let p = plan();
        let tg = p.find_by_name("Checkout Users").unwrap();
        let r = results(&input(Some(&p), Some(tg), &[], ctx()), "+", 200);
        let t = titles(&r);
        assert_eq!(&t[..4], ["HTTP Request", "Regular Expression Extractor", "Response Assertion", "Constant Timer"], "{:?}", &t[..6]);
        assert!(!t.contains(&"Thread Group"), "thread groups only go under the plan");
    }

    #[test]
    fn add_mode_without_a_selection_uses_the_test_plan() {
        let p = plan();
        let r = results(&input(Some(&p), None, &[], ctx()), "+", 200);
        assert_eq!(r[0].item.title, "Thread Group", "first quick-add entry that fits the plan");
        assert!(!titles(&r).contains(&"HTTP Request"), "samplers do not go directly under the plan");
    }

    #[test]
    fn add_mode_under_a_leaf_offers_nothing() {
        let p = plan();
        let tok = p.find_by_name("token").unwrap();
        assert!(results(&input(Some(&p), Some(tok), &[], ctx()), "+", 50).is_empty());
    }

    #[test]
    fn add_items_are_unavailable_until_editing_exists() {
        let p = plan();
        let c = AppContext { can_edit: false, ..ctx() };
        let r = results(&input(Some(&p), None, &[], c), "+", 5);
        assert!(!r.is_empty());
        assert!(r.iter().all(|r| r.item.disabled.as_deref() == Some("Open a plan to add elements")));
    }

    #[test]
    fn limit_is_applied_after_ranking() {
        let p = plan();
        let r = results(&input(Some(&p), None, &[], ctx()), ">", 3);
        assert_eq!(r.len(), 3);
        let best = results(&input(Some(&p), None, &[], ctx()), ">expand", 50);
        assert!(best[0].item.title.contains("Expand"));
    }

    #[test]
    fn shortcuts_are_included_for_commands() {
        let p = plan();
        let r = results(&input(Some(&p), None, &[], ctx()), ">open plan", 5);
        assert_eq!(r[0].item.shortcut, Some("Ctrl+O"));
    }
}
