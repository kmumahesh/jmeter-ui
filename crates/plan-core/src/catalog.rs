//! What kind of element each XML tag is: its category, display name and icon.
//!
//! The table covers the elements shipped with JMeter 5.6.3. Plugin elements (saved with a full class
//! name as their tag) are classified from the end of the class name.

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Category {
    TestPlan,
    ThreadGroup,
    Sampler,
    Controller,
    Config,
    Timer,
    PreProcessor,
    PostProcessor,
    Assertion,
    Listener,
    /// Test Fragment: a controller that only runs when referenced.
    Fragment,
    /// The WorkBench: a scratch area outside the test that can hold anything except thread groups.
    WorkBench,
    /// Elements that live outside the test: the recorder, mirror server, property display.
    NonTest,
    Unknown,
}

impl Category {
    pub const ALL: [Category; 14] = [
        Category::TestPlan,
        Category::ThreadGroup,
        Category::Sampler,
        Category::Controller,
        Category::Config,
        Category::Timer,
        Category::PreProcessor,
        Category::PostProcessor,
        Category::Assertion,
        Category::Listener,
        Category::Fragment,
        Category::WorkBench,
        Category::NonTest,
        Category::Unknown,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Category::TestPlan => "Test Plan",
            Category::ThreadGroup => "Thread Group",
            Category::Sampler => "Sampler",
            Category::Controller => "Logic Controller",
            Category::Config => "Config Element",
            Category::Timer => "Timer",
            Category::PreProcessor => "Pre-Processor",
            Category::PostProcessor => "Post-Processor",
            Category::Assertion => "Assertion",
            Category::Listener => "Listener",
            Category::Fragment => "Test Fragment",
            Category::WorkBench => "WorkBench",
            Category::NonTest => "Non-Test Element",
            Category::Unknown => "Unknown",
        }
    }

    /// Plural heading used in menus and the palette.
    pub fn plural(self) -> &'static str {
        match self {
            Category::TestPlan => "Test Plans",
            Category::ThreadGroup => "Thread Groups",
            Category::Sampler => "Samplers",
            Category::Controller => "Logic Controllers",
            Category::Config => "Config Elements",
            Category::Timer => "Timers",
            Category::PreProcessor => "Pre-Processors",
            Category::PostProcessor => "Post-Processors",
            Category::Assertion => "Assertions",
            Category::Listener => "Listeners",
            Category::Fragment => "Test Fragments",
            Category::WorkBench => "WorkBench",
            Category::NonTest => "Non-Test Elements",
            Category::Unknown => "Other",
        }
    }

    /// Asset path of the icon, relative to the assets folder.
    pub fn icon(self) -> &'static str {
        match self {
            Category::TestPlan => "icons/jm/flask.svg",
            Category::ThreadGroup => "icons/jm/users.svg",
            Category::Sampler => "icons/jm/globe.svg",
            Category::Controller => "icons/jm/repeat.svg",
            Category::Config => "icons/jm/sliders.svg",
            Category::Timer => "icons/jm/clock.svg",
            Category::PreProcessor => "icons/jm/zap.svg",
            Category::PostProcessor => "icons/jm/filter.svg",
            Category::Assertion => "icons/jm/check.svg",
            Category::Listener => "icons/jm/chart.svg",
            Category::Fragment => "icons/jm/puzzle.svg",
            Category::WorkBench => "icons/jm/wrench.svg",
            Category::NonTest => "icons/jm/wrench.svg",
            Category::Unknown => "icons/jm/file.svg",
        }
    }
}

use Category::*;

/// (tag, display name, category). Sorted by tag for binary search; checked by a test.
static TABLE: &[(&str, &str, Category)] = &[
    ("AccessLogSampler", "Access Log Sampler", Sampler),
    ("AjpSampler", "AJP/1.3 Sampler", Sampler),
    ("AnchorModifier", "HTML Link Parser", PreProcessor),
    ("Arguments", "User Defined Variables", Config),
    ("AuthManager", "HTTP Authorization Manager", Config),
    ("BSFAssertion", "BSF Assertion", Assertion),
    ("BSFListener", "BSF Listener", Listener),
    ("BSFPostProcessor", "BSF PostProcessor", PostProcessor),
    ("BSFPreProcessor", "BSF PreProcessor", PreProcessor),
    ("BSFSampler", "BSF Sampler", Sampler),
    ("BSFTimer", "BSF Timer", Timer),
    ("BackendListener", "Backend Listener", Listener),
    ("BeanShellAssertion", "BeanShell Assertion", Assertion),
    ("BeanShellListener", "BeanShell Listener", Listener),
    ("BeanShellPostProcessor", "BeanShell PostProcessor", PostProcessor),
    ("BeanShellPreProcessor", "BeanShell PreProcessor", PreProcessor),
    ("BeanShellSampler", "BeanShell Sampler", Sampler),
    ("BeanShellTimer", "BeanShell Timer", Timer),
    ("CSVDataSet", "CSV Data Set Config", Config),
    ("CacheManager", "HTTP Cache Manager", Config),
    ("CompareAssertion", "Compare Assertion", Assertion),
    ("ConfigTestElement", "Config Element", Config),
    ("ConstantThroughputTimer", "Constant Throughput Timer", Timer),
    ("ConstantTimer", "Constant Timer", Timer),
    ("CookieManager", "HTTP Cookie Manager", Config),
    ("CounterConfig", "Counter", Config),
    ("CriticalSectionController", "Critical Section Controller", Controller),
    ("DNSCacheManager", "DNS Cache Manager", Config),
    ("DebugPostProcessor", "Debug PostProcessor", PostProcessor),
    ("DebugSampler", "Debug Sampler", Sampler),
    ("DurationAssertion", "Duration Assertion", Assertion),
    ("FTPSampler", "FTP Request", Sampler),
    ("ForeachController", "ForEach Controller", Controller),
    ("GaussianRandomTimer", "Gaussian Random Timer", Timer),
    ("GenericController", "Simple Controller", Controller),
    ("HTMLAssertion", "HTML Assertion", Assertion),
    ("HTTPSampler", "HTTP Request", Sampler),
    ("HTTPSampler2", "HTTP Request", Sampler),
    ("HTTPSamplerProxy", "HTTP Request", Sampler),
    ("HeaderManager", "HTTP Header Manager", Config),
    ("HtmlExtractor", "CSS Selector Extractor", PostProcessor),
    ("HttpMirrorControl", "HTTP Mirror Server", NonTest),
    ("IfController", "If Controller", Controller),
    ("IncludeController", "Include Controller", Controller),
    ("InterleaveControl", "Interleave Controller", Controller),
    ("JDBCDataSource", "JDBC Connection Configuration", Config),
    ("JDBCPostProcessor", "JDBC PostProcessor", PostProcessor),
    ("JDBCPreProcessor", "JDBC PreProcessor", PreProcessor),
    ("JDBCSampler", "JDBC Request", Sampler),
    ("JMSSampler", "JMS Point-to-Point", Sampler),
    ("JSONPostProcessor", "JSON Extractor", PostProcessor),
    ("JSR223Assertion", "JSR223 Assertion", Assertion),
    ("JSR223Listener", "JSR223 Listener", Listener),
    ("JSR223PostProcessor", "JSR223 PostProcessor", PostProcessor),
    ("JSR223PreProcessor", "JSR223 PreProcessor", PreProcessor),
    ("JSR223Sampler", "JSR223 Sampler", Sampler),
    ("JSR223Timer", "JSR223 Timer", Timer),
    ("JUnitSampler", "JUnit Request", Sampler),
    ("JavaConfig", "Java Request Defaults", Config),
    ("JavaSampler", "Java Request", Sampler),
    ("KeystoreConfig", "Keystore Configuration", Config),
    ("LDAPExtSampler", "LDAP Extended Request", Sampler),
    ("LDAPSampler", "LDAP Request", Sampler),
    ("LoopController", "Loop Controller", Controller),
    ("MD5HexAssertion", "MD5Hex Assertion", Assertion),
    ("MailReaderSampler", "Mail Reader Sampler", Sampler),
    ("MailerResultCollector", "Mailer Visualizer", Listener),
    ("ModuleController", "Module Controller", Controller),
    ("MongoScriptSampler", "MongoDB Script", Sampler),
    ("MongoSourceElement", "MongoDB Source Config", Config),
    ("OnceOnlyController", "Once Only Controller", Controller),
    ("ParamModifier", "HTML Parameter Mask", PreProcessor),
    ("PoissonRandomTimer", "Poisson Random Timer", Timer),
    ("PostThreadGroup", "tearDown Thread Group", ThreadGroup),
    ("ProxyControl", "HTTP(S) Test Script Recorder", NonTest),
    ("PublisherSampler", "JMS Publisher", Sampler),
    ("RandomController", "Random Controller", Controller),
    ("RandomOrderController", "Random Order Controller", Controller),
    ("RandomVariableConfig", "Random Variable", Config),
    ("RecordingController", "Recording Controller", Controller),
    ("RegExUserParameters", "RegEx User Parameters", PreProcessor),
    ("RegexExtractor", "Regular Expression Extractor", PostProcessor),
    ("ResponseAssertion", "Response Assertion", Assertion),
    ("ResultAction", "Result Status Action Handler", PostProcessor),
    ("ResultCollector", "Listener", Listener),
    ("ResultSaver", "Save Responses to a file", Listener),
    ("RunTime", "Runtime Controller", Controller),
    ("SMIMEAssertion", "SMIME Assertion", Assertion),
    ("SampleTimeout", "Sample Timeout", PreProcessor),
    ("SetupThreadGroup", "setUp Thread Group", ThreadGroup),
    ("SizeAssertion", "Size Assertion", Assertion),
    ("SmtpSampler", "SMTP Sampler", Sampler),
    ("SoapSampler", "SOAP/XML-RPC Request", Sampler),
    ("SubscriberSampler", "JMS Subscriber", Sampler),
    ("Summariser", "Generate Summary Results", Listener),
    ("SwitchController", "Switch Controller", Controller),
    ("SyncTimer", "Synchronizing Timer", Timer),
    ("SystemSampler", "OS Process Sampler", Sampler),
    ("TCPSampler", "TCP Sampler", Sampler),
    ("TestAction", "Flow Control Action", Sampler),
    ("TestFragmentController", "Test Fragment", Fragment),
    ("TestPlan", "Test Plan", TestPlan),
    ("ThreadGroup", "Thread Group", ThreadGroup),
    ("ThroughputController", "Throughput Controller", Controller),
    ("TransactionController", "Transaction Controller", Controller),
    ("URLRewritingModifier", "HTTP URL Re-writing Modifier", PreProcessor),
    ("UniformRandomTimer", "Uniform Random Timer", Timer),
    ("UserParameterModifier", "HTML Parameter Mask", PreProcessor),
    ("UserParameters", "User Parameters", PreProcessor),
    ("WebServiceSampler", "Web Service(SOAP) Request", Sampler),
    ("WhileController", "While Controller", Controller),
    ("WorkBench", "WorkBench", WorkBench),
    ("XMLAssertion", "XML Assertion", Assertion),
    ("XMLSchemaAssertion", "XML Schema Assertion", Assertion),
    ("XPathAssertion", "XPath Assertion", Assertion),
    ("XPathExtractor", "XPath Extractor", PostProcessor),
];

/// Display names for the several listeners that share the `ResultCollector` tag, by `guiclass`.
static LISTENER_GUI_NAMES: &[(&str, &str)] = &[
    ("AssertionVisualizer", "Assertion Results"),
    ("AggregateGraphVisualizer", "Aggregate Graph"),
    ("StatVisualizer", "Aggregate Report"),
    ("GraphVisualizer", "Graph Results"),
    ("RespTimeGraphVisualizer", "Response Time Graph"),
    ("SimpleDataWriter", "Simple Data Writer"),
    ("SummaryReport", "Summary Report"),
    ("TableVisualizer", "View Results in Table"),
    ("ViewResultsFullVisualizer", "View Results Tree"),
    ("ComparisonVisualizer", "Comparison Assertion Visualizer"),
    ("DistributionGraphVisualizer", "Distribution Graph"),
];

/// Display names for `ConfigTestElement` variants, by `guiclass`.
static CONFIG_GUI_NAMES: &[(&str, &str)] = &[
    ("HttpDefaultsGui", "HTTP Request Defaults"),
    ("FtpConfigGui", "FTP Request Defaults"),
    ("LoginConfigGui", "Login Config Element"),
    ("LdapConfigGui", "LDAP Request Defaults"),
    ("LdapExtConfigGui", "LDAP Extended Request Defaults"),
    ("SimpleConfigGui", "Simple Config Element"),
    ("TCPConfigGui", "TCP Sampler Config"),
];

/// Display name and category for an element, given its XML tag and `guiclass`.
pub fn describe(tag: &str, guiclass: &str) -> (String, Category) {
    if let Ok(i) = TABLE.binary_search_by(|(t, _, _)| (*t).cmp(tag)) {
        let (_, label, category) = TABLE[i];
        // The property display shares the ConfigTestElement class but lives outside the test.
        if tag == "ConfigTestElement" && guiclass == "PropertyControlGui" {
            return ("Property Display".to_string(), NonTest);
        }
        let specific = match tag {
            "ResultCollector" => LISTENER_GUI_NAMES.iter().find(|(g, _)| *g == guiclass).map(|(_, n)| *n),
            "ConfigTestElement" => CONFIG_GUI_NAMES.iter().find(|(g, _)| *g == guiclass).map(|(_, n)| *n),
            _ => None,
        };
        return (specific.unwrap_or(label).to_string(), category);
    }
    // Elements newer than the table: JMeter's own add menus know them.
    if let Some(t) = crate::templates::find(tag, guiclass) {
        return (t.label.clone(), t.category);
    }
    if tag.contains('.') {
        return (class_label(tag), classify_plugin(tag));
    }
    (humanize(tag), Unknown)
}

pub fn category_of(tag: &str, guiclass: &str) -> Category {
    describe(tag, guiclass).1
}

/// Every core tag the table knows, for completeness checks.
pub fn known_tags() -> impl Iterator<Item = &'static str> {
    TABLE.iter().map(|(t, _, _)| *t)
}

/// Guess a plugin element's category from the end of its class name.
fn classify_plugin(class: &str) -> Category {
    let simple = class.rsplit('.').next().unwrap_or(class);
    let ends = |s: &str| simple.ends_with(s);
    if ends("ThreadGroup") {
        ThreadGroup
    } else if ends("Sampler") {
        Sampler
    } else if ends("Controller") {
        Controller
    } else if ends("Timer") {
        Timer
    } else if ends("PreProcessor") {
        PreProcessor
    } else if ends("PostProcessor") || ends("Extractor") {
        PostProcessor
    } else if ends("Assertion") {
        Assertion
    } else if ends("Collector") || ends("Listener") || ends("Writer") || ends("Logger") || ends("Uploader") || ends("AutoStop") {
        Listener
    } else if ends("Config") || ends("DataSet") || ends("Manager") || ends("Source") {
        Config
    } else {
        Unknown
    }
}

/// `kg.apc.jmeter.threads.UltimateThreadGroup` becomes `Ultimate Thread Group`.
pub fn class_label(class: &str) -> String {
    humanize(class.rsplit('.').next().unwrap_or(class))
}

/// `ForeachController` becomes `Foreach Controller`; `HTTPSampler` becomes `HTTP Sampler`.
pub fn humanize(name: &str) -> String {
    let chars: Vec<char> = name.chars().collect();
    let mut out = String::with_capacity(name.len() + 4);
    for (i, &c) in chars.iter().enumerate() {
        if i > 0 {
            let prev = chars[i - 1];
            let next = chars.get(i + 1).copied();
            let boundary = c.is_uppercase() && (prev.is_lowercase() || prev.is_ascii_digit() || (prev.is_uppercase() && next.is_some_and(char::is_lowercase)));
            if boundary {
                out.push(' ');
            }
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_is_sorted_and_unique() {
        for w in TABLE.windows(2) {
            assert!(w[0].0 < w[1].0, "{} must come before {}", w[0].0, w[1].0);
        }
    }

    #[test]
    fn core_elements() {
        assert_eq!(category_of("HTTPSamplerProxy", "HttpTestSampleGui"), Sampler);
        assert_eq!(category_of("ThreadGroup", ""), ThreadGroup);
        assert_eq!(category_of("SetupThreadGroup", ""), ThreadGroup);
        assert_eq!(category_of("IfController", ""), Controller);
        assert_eq!(category_of("TestFragmentController", ""), Fragment);
        assert_eq!(category_of("WorkBench", ""), WorkBench);
        assert_eq!(category_of("ProxyControl", ""), NonTest);
        assert_eq!(describe("ConfigTestElement", "PropertyControlGui"), ("Property Display".to_string(), NonTest));
        assert_eq!(category_of("JSONPostProcessor", ""), PostProcessor);
        assert_eq!(category_of("ResponseAssertion", ""), Assertion);
        assert_eq!(category_of("ConstantTimer", ""), Timer);
        assert_eq!(category_of("CSVDataSet", ""), Config);
    }

    #[test]
    fn shared_tags_use_the_gui_class() {
        assert_eq!(describe("ResultCollector", "ViewResultsFullVisualizer").0, "View Results Tree");
        assert_eq!(describe("ResultCollector", "StatVisualizer").0, "Aggregate Report");
        assert_eq!(describe("ResultCollector", "SomethingNew").0, "Listener");
        assert_eq!(describe("ConfigTestElement", "HttpDefaultsGui").0, "HTTP Request Defaults");
        assert_eq!(describe("ConfigTestElement", "LoginConfigGui").0, "Login Config Element");
    }

    #[test]
    fn plugin_classes_are_guessed_from_their_names() {
        assert_eq!(category_of("kg.apc.jmeter.threads.UltimateThreadGroup", ""), ThreadGroup);
        assert_eq!(category_of("kg.apc.jmeter.samplers.DummySampler", ""), Sampler);
        assert_eq!(category_of("kg.apc.jmeter.control.ParameterizedController", ""), Controller);
        assert_eq!(category_of("kg.apc.jmeter.timers.VariableThroughputTimer", ""), Timer);
        assert_eq!(category_of("kg.apc.jmeter.perfmon.PerfMonCollector", ""), Listener);
        assert_eq!(category_of("kg.apc.jmeter.reporters.AutoStop", ""), Listener);
        assert_eq!(category_of("com.example.Mystery", ""), Unknown);
        assert_eq!(describe("kg.apc.jmeter.threads.UltimateThreadGroup", "").0, "Ultimate Thread Group");
    }

    #[test]
    fn unknown_core_tags_get_a_readable_name() {
        assert_eq!(describe("SomeNewThing", "").0, "Some New Thing");
        assert_eq!(describe("SomeNewThing", "").1, Unknown);
    }

    #[test]
    fn humanize_handles_acronyms() {
        assert_eq!(humanize("HTTPSampler"), "HTTP Sampler");
        assert_eq!(humanize("JSR223Timer"), "JSR223 Timer");
        assert_eq!(humanize("ForeachController"), "Foreach Controller");
        assert_eq!(humanize("TCP"), "TCP");
    }

    #[test]
    fn every_category_has_an_icon_and_labels() {
        for c in Category::ALL {
            assert!(c.icon().starts_with("icons/jm/") && c.icon().ends_with(".svg"));
            assert!(!c.label().is_empty() && !c.plural().is_empty());
        }
    }
}

// ---- Element types that can be added from the palette ----

/// One entry in the "Add element" list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AddableType {
    pub tag: &'static str,
    /// The `guiclass` attribute: with the tag it identifies the type (several listeners share a tag).
    pub gui: Option<&'static str>,
    pub label: &'static str,
    pub category: Category,
}

/// JMeter's own quick-add list (`gui.quick_0` to `gui.quick_9` in `jmeter.properties`), as (tag, gui).
pub const QUICK_ADD: &[(&str, Option<&str>)] = &[
    ("ThreadGroup", Some("ThreadGroupGui")),
    ("HTTPSamplerProxy", Some("HttpTestSampleGui")),
    ("RegexExtractor", Some("RegexExtractorGui")),
    ("ResponseAssertion", Some("AssertionGui")),
    ("ConstantTimer", Some("ConstantTimerGui")),
    ("TestAction", Some("TestActionGui")),
    ("JSR223PostProcessor", Some("TestBeanGUI")),
    ("JSR223PreProcessor", Some("TestBeanGUI")),
    ("DebugSampler", Some("TestBeanGUI")),
    ("ResultCollector", Some("ViewResultsFullVisualizer")),
];

/// Every element type offered when adding: JMeter's own "Add" menus, sorted by category then name.
pub fn addable() -> Vec<AddableType> {
    let mut out: Vec<AddableType> =
        crate::templates::all().iter().map(|t| AddableType { tag: &t.tag, gui: Some(&t.guiclass), label: &t.label, category: t.category }).collect();
    out.sort_by(|a, b| a.category.cmp(&b.category).then(a.label.cmp(b.label)));
    out
}

/// Position in JMeter's quick-add list, if the type is on it.
pub fn quick_rank(t: &AddableType) -> Option<usize> {
    QUICK_ADD.iter().position(|(tag, gui)| *tag == t.tag && *gui == t.gui)
}

#[cfg(test)]
mod addable_tests {
    use super::*;

    #[test]
    fn list_is_complete_and_unique() {
        let all = addable();
        assert!(all.len() > 90, "{}", all.len());
        let mut seen = std::collections::HashSet::new();
        for t in &all {
            assert!(seen.insert((t.tag, t.gui)), "duplicate {t:?}");
            assert_ne!(t.category, Category::Unknown);
            assert!(t.category != TestPlan && t.category != WorkBench, "{t:?}");
        }
    }

    #[test]
    fn variants_are_expanded() {
        let all = addable();
        assert!(all.iter().any(|t| t.label == "View Results Tree" && t.gui == Some("ViewResultsFullVisualizer")));
        assert!(all.iter().any(|t| t.label == "HTTP Request Defaults" && t.category == Category::Config));
        assert!(!all.iter().any(|t| t.tag == "ResultCollector" && t.gui.is_none()));
        assert!(!all.iter().any(|t| t.tag == "HTTPSampler"), "legacy tag is hidden");
    }

    #[test]
    fn every_quick_add_entry_exists() {
        let all = addable();
        for (tag, gui) in QUICK_ADD {
            assert!(all.iter().any(|t| t.tag == *tag && t.gui == *gui), "{tag} {gui:?}");
        }
        let http = all.iter().find(|t| t.tag == "HTTPSamplerProxy" && t.gui == Some("HttpTestSampleGui")).unwrap();
        assert_eq!(quick_rank(http), Some(1));
        let ex = all.iter().find(|t| t.tag == "XPathExtractor").unwrap();
        assert_eq!(quick_rank(ex), None);
    }

    #[test]
    fn sorted_by_category_then_name() {
        let all = addable();
        for w in all.windows(2) {
            assert!((w[0].category, w[0].label) <= (w[1].category, w[1].label), "{:?} {:?}", w[0], w[1]);
        }
    }
}

#[cfg(test)]
mod template_fallback_tests {
    use super::*;

    #[test]
    fn elements_missing_from_the_table_are_found_through_jmeters_menus() {
        let (label, cat) = describe("OpenModelThreadGroup", "OpenModelThreadGroupGui");
        assert_eq!((label.as_str(), cat), ("Open Model Thread Group", ThreadGroup));
    }

    #[test]
    fn every_menu_element_is_classified_the_same_way_by_both_sources() {
        for t in crate::templates::all() {
            assert_eq!(describe(&t.tag, &t.guiclass).1, t.category, "{}", t.label);
        }
    }
}
