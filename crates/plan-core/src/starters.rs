//! Plans to start from: an empty one, one for an API and one for a web site.
//!
//! Each starter is built with the same editing steps a person would take, from an empty Test Plan, so what it
//! contains is exactly what JMeter writes for those elements, and it can be opened, edited and run like any other
//! plan. Nothing in it points at a real site: the server names are examples to replace.

use crate::edit::EditablePlan;
use crate::forms::{self, RowShape, new_row};
use crate::model::NodeId;
use anyhow::Result;
use jmx_model::xml::escape;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Starter {
    Empty,
    Api,
    Web,
    /// Log in, keep the token the server gives, and use it in the next request.
    Login,
    /// Requests that take their values from a CSV file, a row for each user and iteration.
    Data,
    /// A steady load for a set time, with a ramp up and a pause between requests.
    Load,
    /// A small steady load with a sudden burst of users on top of it.
    Spike,
}

impl Starter {
    pub const ALL: [Starter; 7] = [Starter::Api, Starter::Web, Starter::Login, Starter::Data, Starter::Load, Starter::Spike, Starter::Empty];

    /// The name of this starter's button in the New Plan question.
    pub fn button_id(self) -> &'static str {
        match self {
            Starter::Api => "dialog-new-api",
            Starter::Web => "dialog-new-web",
            Starter::Login => "dialog-new-login",
            Starter::Data => "dialog-new-data",
            Starter::Load => "dialog-new-load",
            Starter::Spike => "dialog-new-spike",
            Starter::Empty => "dialog-new-empty",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Starter::Api => "API test",
            Starter::Web => "Web site test",
            Starter::Login => "Login and reuse a token",
            Starter::Data => "Data-driven test (CSV)",
            Starter::Load => "Load test (ramp up, steady, time limit)",
            Starter::Spike => "Spike test",
            Starter::Empty => "Empty plan",
        }
    }

    /// One sentence on what the starter contains.
    pub fn description(self) -> &'static str {
        match self {
            Starter::Api => "server defaults, JSON headers, one request with a status check, and a pause between requests",
            Starter::Web => "server defaults, cookies, caching, one page request with a status check, and a random pause between requests",
            Starter::Login => "a CSV file of users, a login that keeps the token from the answer, a request that sends it, checks, and a pause between requests",
            Starter::Data => "a CSV file of search terms, a request that uses a term from each row, a check, and a pause between requests",
            Starter::Load => "50 users ramped up over a minute, running for ten minutes, a check, and a random pause between requests",
            Starter::Spike => "10 steady users for five minutes and 200 more that arrive after two minutes for one minute",
            Starter::Empty => "only the Test Plan",
        }
    }

    /// A name for the file, without the extension.
    pub fn file_stem(self) -> &'static str {
        match self {
            Starter::Api => "api-test",
            Starter::Web => "web-test",
            Starter::Login => "login-test",
            Starter::Data => "data-driven-test",
            Starter::Load => "load-test",
            Starter::Spike => "spike-test",
            Starter::Empty => "new-plan",
        }
    }
}

/// The longest name given to a new Test Plan.
pub const MAX_NAME: usize = 100;

/// What JMeter's File > New gives: a Test Plan and nothing else.
fn empty_plan(name: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<jmeterTestPlan version=\"1.2\" properties=\"5.0\" jmeter=\"5.6.3\">\n  <hashTree>\n    <TestPlan guiclass=\"TestPlanGui\" testclass=\"TestPlan\" testname=\"{}\" enabled=\"true\">\n      <boolProp name=\"TestPlan.functional_mode\">false</boolProp>\n      <boolProp name=\"TestPlan.serialize_threadgroups\">false</boolProp>\n      <elementProp name=\"TestPlan.user_defined_variables\" elementType=\"Arguments\" guiclass=\"ArgumentsPanel\" testclass=\"Arguments\" testname=\"User Defined Variables\" enabled=\"true\">\n        <collectionProp name=\"Arguments.arguments\"/>\n      </elementProp>\n    </TestPlan>\n    <hashTree/>\n  </hashTree>\n</jmeterTestPlan>\n",
        escape(name, true)
    )
}

/// A name that is safe to write into a plan: no control characters, no leading or trailing white space, at most
/// `MAX_NAME` characters, and never empty.
pub fn clean_name(name: &str) -> String {
    let cleaned: String = name.chars().filter(|c| !c.is_control()).collect();
    let cleaned: String = cleaned.trim().chars().take(MAX_NAME).collect();
    let cleaned = cleaned.trim().to_string();
    if cleaned.is_empty() { "Test Plan".to_string() } else { cleaned }
}

struct Build {
    e: EditablePlan,
    root: NodeId,
}

impl Build {
    fn add(&mut self, parent: NodeId, tag: &str, gui: &str, name: &str) -> Result<NodeId> {
        let id = self.e.add(parent, tag, gui)?;
        self.e.rename(id, name)?;
        Ok(id)
    }

    fn set(&mut self, id: NodeId, prop: &str, kind: &str, value: &str) -> Result<()> {
        self.e.set_prop(id, prop, kind, value)
    }

    fn defaults(&mut self, host: &str) -> Result<()> {
        let d = self.add(self.root, "ConfigTestElement", "HttpDefaultsGui", "HTTP Request Defaults")?;
        self.set(d, "HTTPSampler.protocol", "stringProp", "https")?;
        self.set(d, "HTTPSampler.domain", "stringProp", host)
    }

    fn headers(&mut self, rows: &[(&str, &str)]) -> Result<()> {
        let h = self.add(self.root, "HeaderManager", "HeaderPanel", "HTTP Header Manager")?;
        for (i, (name, value)) in rows.iter().enumerate() {
            self.e.add_row(h, "HeaderManager.headers", i, new_row(RowShape::Header, &[name, value]))?;
        }
        Ok(())
    }

    fn group(&mut self, name: &str, users: u32, ramp: u32, loops: u32) -> Result<NodeId> {
        let g = self.add(self.root, "ThreadGroup", "ThreadGroupGui", name)?;
        self.set(g, "ThreadGroup.num_threads", "stringProp", &users.to_string())?;
        self.set(g, "ThreadGroup.ramp_time", "stringProp", &ramp.to_string())?;
        self.set(g, "ThreadGroup.main_controller/LoopController.loops", "stringProp", &loops.to_string())?;
        Ok(g)
    }

    /// A thread group that runs for `duration` seconds after waiting `delay` seconds to start.
    fn timed_group(&mut self, name: &str, users: u32, ramp: u32, delay: u32, duration: u32) -> Result<NodeId> {
        let g = self.group(name, users, ramp, 1)?;
        self.set(g, "ThreadGroup.main_controller/LoopController.loops", "stringProp", "-1")?;
        self.set(g, "ThreadGroup.main_controller/LoopController.continue_forever", "boolProp", "false")?;
        self.set(g, "ThreadGroup.scheduler", "boolProp", "true")?;
        self.set(g, "ThreadGroup.duration", "stringProp", &duration.to_string())?;
        self.set(g, "ThreadGroup.delay", "stringProp", &delay.to_string())?;
        Ok(g)
    }

    /// A pause of 1 to 3 seconds before the requests of the group, chosen at random each time.
    fn think_time(&mut self, parent: NodeId) -> Result<()> {
        let t = self.add(parent, "UniformRandomTimer", "UniformRandomTimerGui", "Think time")?;
        self.set(t, "ConstantTimer.delay", "stringProp", "1000")?;
        self.set(t, "RandomTimer.range", "stringProp", "2000.0")
    }

    /// A CSV Data Set that reads `file` (next to the plan) into the variables named, a row for each user in turn.
    fn csv(&mut self, parent: NodeId, name: &str, file: &str, variables: &str) -> Result<()> {
        let c = self.add(parent, "CSVDataSet", "TestBeanGUI", name)?;
        self.set(c, "filename", "stringProp", file)?;
        self.set(c, "variableNames", "stringProp", variables)?;
        self.set(c, "ignoreFirstLine", "boolProp", "false")?;
        self.set(c, "delimiter", "stringProp", ",")?;
        self.set(c, "recycle", "boolProp", "true")?;
        self.set(c, "stopThread", "boolProp", "false")
    }

    /// A request that must answer 200 (the status check goes under it).
    fn status_check(&mut self, request: NodeId) -> Result<()> {
        let a = self.add(request, "ResponseAssertion", "AssertionGui", "Status is 200")?;
        self.set(a, "Assertion.test_field", "stringProp", "Assertion.response_code")?;
        self.set(a, "Assertion.test_type", "intProp", "8")?;
        self.e.add_row(a, "Asserion.test_strings", 0, new_row(RowShape::Text, &["200"]))
    }

    /// A GET request that must answer 200.
    fn checked_get(&mut self, group: NodeId, name: &str, path: &str) -> Result<()> {
        let r = self.add(group, "HTTPSamplerProxy", "HttpTestSampleGui", name)?;
        self.set(r, "HTTPSampler.method", "stringProp", "GET")?;
        self.set(r, "HTTPSampler.path", "stringProp", path)?;
        let a = self.add(r, "ResponseAssertion", "AssertionGui", "Status is 200")?;
        self.set(a, "Assertion.test_field", "stringProp", "Assertion.response_code")?;
        self.set(a, "Assertion.test_type", "intProp", "8")?;
        self.e.add_row(a, "Asserion.test_strings", 0, new_row(RowShape::Text, &["200"]))?;
        Ok(())
    }
}

/// The text of a new plan of the given kind, named `name` (cleaned first). The plan is clean: opening the text
/// gives a plan with nothing to save and nothing to undo.
pub fn new_plan_text(starter: Starter, name: &str) -> Result<String> {
    let name = clean_name(name);
    let e = EditablePlan::open(&empty_plan(&name))?;
    let root = e.plan().roots()[0];
    let mut b = Build { e, root };
    match starter {
        Starter::Empty => {}
        Starter::Api => {
            b.defaults("api.example.com")?;
            b.headers(&[("Accept", "application/json"), ("Content-Type", "application/json")])?;
            let g = b.group("Users", 5, 10, 10)?;
            b.checked_get(g, "Get items", "/items")?;
            let t = b.add(g, "ConstantTimer", "ConstantTimerGui", "Think time")?;
            b.set(t, "ConstantTimer.delay", "stringProp", "1000")?;
        }
        Starter::Login => {
            b.defaults("api.example.com")?;
            b.headers(&[("Accept", "application/json"), ("Content-Type", "application/json")])?;
            let g = b.group("Users", 10, 30, 5)?;
            b.csv(g, "Users file", "users.csv", "username,password")?;
            let login = b.add(g, "HTTPSamplerProxy", "HttpTestSampleGui", "Log in")?;
            b.set(login, "HTTPSampler.method", "stringProp", "POST")?;
            b.set(login, "HTTPSampler.path", "stringProp", "/auth/login")?;
            b.set(login, forms::RAW_BODY, "boolProp", "true")?;
            b.e.add_row(login, forms::PARAMETERS, 0, new_row(RowShape::HttpArgument, &["", "{\"username\":\"${username}\",\"password\":\"${password}\"}", "false", "false"]))?;
            b.status_check(login)?;
            let x = b.add(login, "JSONPostProcessor", "JSONPostProcessorGui", "Keep the token")?;
            b.set(x, "JSONPostProcessor.referenceNames", "stringProp", "token")?;
            b.set(x, "JSONPostProcessor.jsonPathExprs", "stringProp", "$.token")?;
            b.set(x, "JSONPostProcessor.match_numbers", "stringProp", "1")?;
            b.set(x, "JSONPostProcessor.defaultValues", "stringProp", "NO_TOKEN")?;
            let me = b.add(g, "HTTPSamplerProxy", "HttpTestSampleGui", "Get my profile")?;
            b.set(me, "HTTPSampler.method", "stringProp", "GET")?;
            b.set(me, "HTTPSampler.path", "stringProp", "/me")?;
            b.status_check(me)?;
            let h = b.add(me, "HeaderManager", "HeaderPanel", "Send the token")?;
            b.e.add_row(h, "HeaderManager.headers", 0, new_row(RowShape::Header, &["Authorization", "Bearer ${token}"]))?;
            b.think_time(g)?;
        }
        Starter::Data => {
            b.defaults("www.example.com")?;
            let g = b.group("Users", 10, 30, 20)?;
            b.csv(g, "Search terms", "terms.csv", "term")?;
            let r = b.add(g, "HTTPSamplerProxy", "HttpTestSampleGui", "Search")?;
            b.set(r, "HTTPSampler.method", "stringProp", "GET")?;
            b.set(r, "HTTPSampler.path", "stringProp", "/search?q=${term}")?;
            b.status_check(r)?;
            b.think_time(g)?;
        }
        Starter::Load => {
            b.defaults("api.example.com")?;
            b.headers(&[("Accept", "application/json")])?;
            let g = b.timed_group("Users", 50, 60, 0, 600)?;
            b.checked_get(g, "Get items", "/items")?;
            b.think_time(g)?;
        }
        Starter::Spike => {
            b.defaults("api.example.com")?;
            b.headers(&[("Accept", "application/json")])?;
            let base = b.timed_group("Steady users", 10, 10, 0, 300)?;
            b.checked_get(base, "Get items", "/items")?;
            b.think_time(base)?;
            let spike = b.timed_group("Spike", 200, 5, 120, 60)?;
            b.checked_get(spike, "Get items", "/items")?;
            b.think_time(spike)?;
        }
        Starter::Web => {
            b.defaults("www.example.com")?;
            b.add(b.root, "CookieManager", "CookiePanel", "HTTP Cookie Manager")?;
            b.add(b.root, "CacheManager", "CacheManagerGui", "HTTP Cache Manager")?;
            b.headers(&[("Accept-Language", "en-US,en;q=0.9")])?;
            let g = b.group("Visitors", 10, 30, 5)?;
            b.checked_get(g, "Home page", "/")?;
            let t = b.add(g, "UniformRandomTimer", "UniformRandomTimerGui", "Think time")?;
            b.set(t, "ConstantTimer.delay", "stringProp", "1000")?;
            b.set(t, "RandomTimer.range", "stringProp", "2000.0")?;
        }
    }
    Ok(b.e.to_text())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Plan;

    fn names(e: &EditablePlan) -> Vec<String> {
        e.plan().iter().map(|n| n.name.clone()).collect()
    }

    #[test]
    fn every_starter_is_a_plan_that_opens_clean_and_saves_back_unchanged() {
        for s in Starter::ALL {
            let text = new_plan_text(s, "My plan").unwrap();
            assert!(Plan::from_jmx_str(&text).is_ok(), "{s:?}");
            let e = EditablePlan::open(&text).unwrap();
            assert!(!e.is_dirty() && !e.can_undo(), "{s:?} opens with nothing to save");
            assert_eq!(e.to_text(), text, "{s:?} is saved back exactly as it was written");
            assert_eq!(e.plan().node(e.plan().roots()[0]).unwrap().name, "My plan");
        }
    }

    #[test]
    fn the_empty_plan_is_only_a_test_plan() {
        let e = EditablePlan::open(&new_plan_text(Starter::Empty, "Nothing").unwrap()).unwrap();
        assert_eq!(names(&e), ["Nothing"]);
    }

    #[test]
    fn the_api_starter_has_what_the_description_says() {
        let e = EditablePlan::open(&new_plan_text(Starter::Api, "API").unwrap()).unwrap();
        let p = e.plan();
        assert_eq!(names(&e), ["API", "HTTP Request Defaults", "HTTP Header Manager", "Users", "Get items", "Status is 200", "Think time"]);
        let users = p.find_by_name("Users").unwrap();
        assert_eq!(e.prop(users, "ThreadGroup.num_threads").as_deref(), Some("5"));
        assert_eq!(e.prop(users, "ThreadGroup.ramp_time").as_deref(), Some("10"));
        assert_eq!(e.prop(users, "ThreadGroup.main_controller/LoopController.loops").as_deref(), Some("10"));
        let defaults = p.find_by_name("HTTP Request Defaults").unwrap();
        assert_eq!(e.prop(defaults, "HTTPSampler.protocol").as_deref(), Some("https"));
        assert_eq!(e.prop(defaults, "HTTPSampler.domain").as_deref(), Some("api.example.com"));
        let req = p.find_by_name("Get items").unwrap();
        assert_eq!(e.prop(req, "HTTPSampler.path").as_deref(), Some("/items"));
        let headers = p.find_by_name("HTTP Header Manager").unwrap();
        assert_eq!(e.row_count(headers, "HeaderManager.headers"), 2);
        let check = p.find_by_name("Status is 200").unwrap();
        assert_eq!(e.prop(check, "Assertion.test_field").as_deref(), Some("Assertion.response_code"));
        assert_eq!(e.row_text(check, "Asserion.test_strings", 0), "200");
    }

    #[test]
    fn the_web_starter_has_what_the_description_says() {
        let e = EditablePlan::open(&new_plan_text(Starter::Web, "Site").unwrap()).unwrap();
        assert_eq!(names(&e), ["Site", "HTTP Request Defaults", "HTTP Cookie Manager", "HTTP Cache Manager", "HTTP Header Manager", "Visitors", "Home page", "Status is 200", "Think time"]);
        let timer = e.plan().find_by_name("Think time").unwrap();
        assert_eq!(e.prop(timer, "RandomTimer.range").as_deref(), Some("2000.0"));
    }

    #[test]
    fn starters_can_be_edited_further_like_any_plan() {
        for s in Starter::ALL {
            let mut e = EditablePlan::open(&new_plan_text(s, "X").unwrap()).unwrap();
            let root = e.plan().roots()[0];
            e.add(root, "ThreadGroup", "ThreadGroupGui").unwrap();
            e.undo().unwrap();
            assert_eq!(e.to_text(), new_plan_text(s, "X").unwrap(), "undo gives the starter back exactly");
        }
    }

    #[test]
    fn the_starters_start_without_warnings_or_undefined_variables() {
        for s in Starter::ALL {
            let e = EditablePlan::open(&new_plan_text(s, "X").unwrap()).unwrap();
            let problems = crate::problems::check(e.plan());
            if s != Starter::Empty {
                assert!(problems.iter().all(|p| p.level != crate::problems::Level::Warning), "{s:?} starts with a warning: {problems:?}");
            }
            assert!(crate::variables::check(&e).is_empty(), "{s:?}");
        }
    }

    // ---- names that come from a file name or from a person ----

    #[test]
    fn names_with_markup_are_written_as_text_not_as_xml() {
        for name in ["a<b>&c", "\"quoted\"", "<script>alert(1)</script>", "x]]>y", "&amp;", "a'b"] {
            let text = new_plan_text(Starter::Api, name).unwrap();
            let e = EditablePlan::open(&text).unwrap();
            assert_eq!(e.plan().node(e.plan().roots()[0]).unwrap().name, clean_name(name), "{name}");
            assert_eq!(e.plan().len(), 7, "the name did not change the shape of the plan: {name}");
            assert!(!text.contains("<script>"), "{name}");
        }
    }

    #[test]
    fn control_characters_are_dropped_and_blank_names_get_a_default() {
        assert_eq!(clean_name("a\u{0}b\nc\td"), "abcd");
        assert_eq!(clean_name("  padded  "), "padded");
        assert_eq!(clean_name(""), "Test Plan");
        assert_eq!(clean_name("\n\t\u{7}"), "Test Plan");
        assert_eq!(clean_name("   "), "Test Plan");
        let long = clean_name(&"n".repeat(5000));
        assert_eq!(long.chars().count(), MAX_NAME);
        let wide = clean_name(&"\u{1F600}".repeat(500));
        assert_eq!(wide.chars().count(), MAX_NAME, "counted in characters, not bytes");
    }

    #[test]
    fn building_a_starter_is_quick() {
        let start = std::time::Instant::now();
        for _ in 0..50 {
            for s in Starter::ALL {
                new_plan_text(s, "Timing").unwrap();
            }
        }
        assert!(start.elapsed() < std::time::Duration::from_secs(5), "150 starters took {:?}", start.elapsed());
    }

    #[test]
    fn labels_and_file_names_are_distinct_and_sensible() {
        let mut labels = std::collections::HashSet::new();
        let mut stems = std::collections::HashSet::new();
        for s in Starter::ALL {
            assert!(labels.insert(s.label()) && stems.insert(s.file_stem()));
            assert!(!s.description().is_empty());
            assert!(s.file_stem().chars().all(|c| c.is_ascii_lowercase() || c == '-'));
        }
    }
}
