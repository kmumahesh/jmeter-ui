//! A HAR file becomes plan elements that paste into a plan: grouped by page, with headers and bodies, and always
//! text that can be pasted and saved.

use plan_core::har::{self, Options};
use plan_core::starters::{self, Starter};
use plan_core::{EditablePlan, curl};

fn entry(page: &str, method: &str, url: &str, extra: &str) -> String {
    format!(r#"{{"pageref":"{page}","request":{{"method":"{method}","url":"{url}","headers":[{{"name":"Accept","value":"application/json"}}]{extra}}},"response":{{"content":{{"mimeType":"application/json"}}}}}}"#)
}

fn shop_har() -> String {
    let entries = [
        entry("p1", "GET", "https://shop.example.com/", ""),
        entry("p1", "GET", "https://shop.example.com/api/items?page=1", ""),
        entry("p2", "POST", "https://shop.example.com/api/login", r#","postData":{"mimeType":"application/json","text":"{\"user\":\"demo\"}"}"#),
        entry("p2", "GET", "https://shop.example.com/api/cart", ""),
    ]
    .join(",");
    format!(r#"{{"log":{{"pages":[{{"id":"p1","title":"Home"}},{{"id":"p2","title":"Sign in & buy"}}],"entries":[{entries}]}}}}"#)
}

fn pasted(text: &str) -> EditablePlan {
    let mut plan = EditablePlan::open(&starters::new_plan_text(Starter::Api, "t").unwrap()).unwrap();
    let group = plan.plan().find_by_name("Users").unwrap();
    plan.paste_text(group, 0, text).expect("the text can be pasted");
    // And the plan can be saved and read again.
    EditablePlan::open(&plan.to_text()).expect("the saved plan reads back");
    plan
}

#[test]
fn pages_become_transaction_controllers_holding_their_requests() {
    let (reqs, _) = har::parse(&shop_har(), &Options::default()).unwrap();
    let plan = pasted(&curl::plan_text(&reqs).unwrap());
    let home = plan.plan().find_by_name("Home").expect("a controller for the first page");
    let buy = plan.plan().find_by_name("Sign in & buy").expect("the second page, named with its own characters");
    let kids = |id| plan.plan().node(id).unwrap().children.iter().map(|c| plan.plan().node(*c).unwrap().name.clone()).collect::<Vec<_>>();
    assert_eq!(plan.plan().node(home).unwrap().tag, "TransactionController");
    assert_eq!(kids(home), ["GET /", "GET /api/items"]);
    assert_eq!(kids(buy), ["POST /api/login", "GET /api/cart"]);
    let login = plan.plan().find_by_name("POST /api/login").unwrap();
    assert_eq!(plan.prop(login, "HTTPSampler.domain").as_deref(), Some("shop.example.com"));
    assert_eq!(plan.prop(login, "HTTPSampler.protocol").as_deref(), Some("https"));
    assert_eq!(plan.prop(login, "HTTPSampler.postBodyRaw").as_deref(), Some("true"));
}

#[test]
fn without_grouping_the_requests_sit_directly_under_the_group() {
    let (reqs, _) = har::parse(&shop_har(), &Options { group_by_page: false, ..Default::default() }).unwrap();
    let plan = pasted(&curl::plan_text(&reqs).unwrap());
    assert!(plan.plan().find_by_name("Home").is_none());
    let group = plan.plan().find_by_name("Users").unwrap();
    let samplers = plan.plan().node(group).unwrap().children.iter().filter(|c| plan.plan().node(**c).unwrap().tag == "HTTPSamplerProxy").count();
    assert!(samplers >= 4, "{samplers}");
}

#[test]
fn a_page_that_comes_back_later_starts_a_new_controller_and_requests_without_a_page_stay_loose() {
    let text = format!(
        r#"{{"log":{{"pages":[{{"id":"a","title":"A"}},{{"id":"b","title":"B"}}],"entries":[{},{},{},{}]}}}}"#,
        entry("a", "GET", "https://x.org/1", ""),
        entry("b", "GET", "https://x.org/2", ""),
        entry("a", "GET", "https://x.org/3", ""),
        r#"{"request":{"method":"GET","url":"https://x.org/4"},"response":{}}"#
    );
    let (reqs, _) = har::parse(&text, &Options::default()).unwrap();
    let plan = pasted(&curl::plan_text(&reqs).unwrap());
    let controllers = plan.plan().iter().filter(|n| n.tag == "TransactionController").count();
    assert_eq!(controllers, 3, "A, B and A again, in the order they happened");
    assert!(plan.plan().find_by_name("GET /4").is_some());
}

#[test]
fn a_big_recording_pastes_quickly() {
    let entries: Vec<String> = (0..1500).map(|i| entry(&format!("p{}", i / 10), "GET", &format!("https://x.org/api/{i}?q=1"), "")).collect();
    let pages: Vec<String> = (0..150).map(|i| format!(r#"{{"id":"p{i}","title":"Page {i}"}}"#)).collect();
    let text = format!(r#"{{"log":{{"pages":[{}],"entries":[{}]}}}}"#, pages.join(","), entries.join(","));
    let t = std::time::Instant::now();
    let (reqs, s) = har::parse(&text, &Options::default()).unwrap();
    assert_eq!(s.kept, 1500);
    let t1 = std::time::Instant::now();
    let made = curl::plan_text(&reqs).unwrap();
    eprintln!("plan_text {:?} parse {:?}", t1.elapsed(), t1.duration_since(t));
    let t2 = std::time::Instant::now();
    let plan = pasted(&made);
    eprintln!("paste+reopen {:?}", t2.elapsed());
    eprintln!("1500 requests in 150 pages, read and pasted in {:?}", t.elapsed());
    assert!(plan.plan().iter().count() > 1650);
    assert!(t.elapsed() < std::time::Duration::from_secs(30), "{:?}", t.elapsed());
}

#[test]
fn hostile_har_files_never_make_text_that_cannot_be_pasted() {
    let odd = ["</testname><evil/>", "a\"b'c&d<e>", "\u{1F600} caf\u{e9}", "line\nbreak", "tab\there"];
    for (i, title) in odd.iter().enumerate() {
        let text = format!(
            r#"{{"log":{{"pages":[{{"id":"p","title":{}}}],"entries":[{{"pageref":"p","request":{{"method":"POST","url":"https://x.org/a{}?x=<y>&z=\"q\"","headers":[{{"name":"X-{}","value":{}}}],"postData":{{"mimeType":"text/plain","text":{}}}}},"response":{{}}}}]}}}}"#,
            serde_json::to_string(title).unwrap(),
            i,
            i,
            serde_json::to_string(title).unwrap(),
            serde_json::to_string(title).unwrap()
        );
        let (reqs, _) = har::parse(&text, &Options::default()).unwrap_or_else(|e| panic!("{title:?}: {e}"));
        let _ = pasted(&curl::plan_text(&reqs).unwrap_or_else(|e| panic!("{title:?}: {e:#}")));
    }
}
