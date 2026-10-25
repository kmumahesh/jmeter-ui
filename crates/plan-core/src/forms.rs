//! What the editor shows for each kind of element: labelled fields for its settings.
//!
//! Elements without a hand-made form get a generic one built from the settings saved on them, so every element
//! can be edited. The names of the settings (`HTTPSampler.path` and so on) are the ones JMeter saves.

use crate::model::Node;
use jmx_model::plan::PropKind;

/// How a setting is edited.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Kind {
    Text,
    /// A whole number, or a `${variable}` or function.
    Number,
    /// A number that may have decimals, or a `${variable}`.
    Decimal,
    Bool,
    /// One of a fixed list of (label, saved value).
    Choice(&'static [(&'static str, &'static str)]),
    /// One of several flags that share an integer setting: (label, bit). Choosing one clears the others.
    BitChoice(&'static [(&'static str, i64)]),
    /// A single flag in an integer setting.
    BitFlag(i64),
    /// Text that may span lines, edited in a text area.
    Multiline,
    /// A collection or group of settings, shown as a short summary and not edited here.
    Summary,
}

#[derive(Copy, Clone, Debug)]
pub struct Field {
    pub label: &'static str,
    /// The setting's name. `a/b` is the setting `b` inside the element setting `a`.
    pub prop: &'static str,
    pub kind: Kind,
    /// What the element does when the setting is missing, shown as the value then.
    pub default: &'static str,
    pub help: &'static str,
    /// Shown only while this on/off setting of the element is in the given state.
    pub when: Option<(&'static str, bool)>,
}

#[derive(Copy, Clone, Debug)]
pub struct Column {
    pub label: &'static str,
    /// The cell's setting inside the row. Empty for a table of plain strings, where the row itself is the cell.
    pub prop: &'static str,
    pub kind: Kind,
}

/// The kind of row a table holds, which decides how a new row is written.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum RowShape {
    /// `Argument.name`, `Argument.value` and the `=` between them (user defined variables).
    Argument,
    /// The same, for HTTP request parameters (with encoding flags).
    HttpArgument,
    /// `Header.name` and `Header.value`.
    Header,
    /// Just a string, as in the patterns of a Response Assertion.
    Text,
}

#[derive(Copy, Clone, Debug)]
pub struct Table {
    pub title: &'static str,
    /// Where the table is, for example `HTTPsampler.Arguments/Arguments.arguments`.
    pub path: &'static str,
    pub columns: &'static [Column],
    pub shape: RowShape,
    pub add_label: &'static str,
    /// Shown only while this on/off setting of the element is in the given state.
    pub when: Option<(&'static str, bool)>,
}

#[derive(Copy, Clone, Debug)]
pub struct Section {
    pub title: &'static str,
    pub fields: &'static [Field],
    pub tables: &'static [Table],
}

const fn f(label: &'static str, prop: &'static str, kind: Kind) -> Field {
    Field { label, prop, kind, default: "", help: "", when: None }
}

const fn fd(label: &'static str, prop: &'static str, kind: Kind, default: &'static str) -> Field {
    Field { label, prop, kind, default, help: "", when: None }
}

/// A field that is shown only while an on/off setting of the element is in the given state.
const fn fw(label: &'static str, prop: &'static str, kind: Kind, when: (&'static str, bool)) -> Field {
    Field { label, prop, kind, default: "", help: "", when: Some(when) }
}

/// The on/off setting that makes an HTTP request send its parameters as one body.
pub const RAW_BODY: &str = "HTTPSampler.postBodyRaw";

/// The table of parameters of an HTTP request (or of its defaults).
pub const PARAMETERS: &str = "HTTPsampler.Arguments/Arguments.arguments";

/// The body of an HTTP request that sends one: the value of the first row of the parameters.
pub const BODY: &str = "HTTPsampler.Arguments/Arguments.arguments/#0/Argument.value";

/// Whether a field or table with this condition is shown for `node`.
pub fn shown(when: Option<(&str, bool)>, node: &Node) -> bool {
    match when {
        None => true,
        Some((prop, want)) => node.props.iter().find(|p| p.name == prop).is_some_and(|p| p.value == "true") == want,
    }
}

const fn col(label: &'static str, prop: &'static str, kind: Kind) -> Column {
    Column { label, prop, kind }
}

use Kind::*;

// ---- shared pieces ----

const ARGUMENT_COLUMNS: &[Column] = &[col("Name", "Argument.name", Text), col("Value", "Argument.value", Text)];

const HTTP_ARGUMENT_COLUMNS: &[Column] = &[
    col("Name", "Argument.name", Text),
    col("Value", "Argument.value", Text),
    col("URL encode", "HTTPArgument.always_encode", Bool),
    col("Include =", "HTTPArgument.use_equals", Bool),
];

const HEADER_COLUMNS: &[Column] = &[col("Name", "Header.name", Text), col("Value", "Header.value", Text)];

const SERVER_FIELDS: &[Field] = &[
    fd("Protocol", "HTTPSampler.protocol", Text, "http"),
    f("Server name or IP", "HTTPSampler.domain", Text),
    f("Port", "HTTPSampler.port", Number),
];

const SCOPE_CHOICES: &[(&str, &str)] =
    &[("Main sample only", ""), ("Sub-samples only", "children"), ("Main sample and sub-samples", "all"), ("A JMeter variable", "variable")];

const EXTRACTOR_SCOPE: &[Field] = &[fd("Apply to", "Sample.scope", Choice(SCOPE_CHOICES), ""), f("Variable name", "Scope.variable", Text)];

const ON_SAMPLE_ERROR: &[(&str, &str)] = &[
    ("Continue", "continue"),
    ("Start next thread loop", "startnextloop"),
    ("Stop thread", "stopthread"),
    ("Stop test", "stoptest"),
    ("Stop test now", "stoptestnow"),
];

const SCRIPT_LANGUAGES: &[(&str, &str)] = &[
    ("Groovy", "groovy"),
    ("Java", "java"),
    ("JavaScript", "javascript"),
    ("BeanShell", "beanshell"),
    ("Jexl3", "jexl3"),
];

const SCRIPT_FIELDS: &[Field] = &[
    fd("Language", "scriptLanguage", Choice(SCRIPT_LANGUAGES), "groovy"),
    f("Parameters", "parameters", Text),
    f("Script file", "filename", Text),
    fd("Cache compiled script", "cacheKey", Bool, "true"),
    f("Script", "script", Multiline),
];

// ---- elements ----

const TEST_PLAN: &[Section] = &[Section {
    title: "Test Plan",
    fields: &[
        f("Run thread groups consecutively", "TestPlan.serialize_threadgroups", Bool),
        f("Run tearDown thread groups after shutdown of main threads", "TestPlan.tearDown_on_shutdown", Bool),
        f("Functional test mode", "TestPlan.functional_mode", Bool),
        f("Add directory or jar to classpath", "TestPlan.user_define_classpath", Text),
    ],
    tables: &[Table {
        title: "User defined variables",
        path: "TestPlan.user_defined_variables/Arguments.arguments",
        columns: ARGUMENT_COLUMNS,
        shape: RowShape::Argument,
        add_label: "Add Variable",
        when: None,
    }],
}];

const THREAD_GROUP: &[Section] = &[
    Section {
        title: "Action to be taken after a sampler error",
        fields: &[fd("Action", "ThreadGroup.on_sample_error", Choice(ON_SAMPLE_ERROR), "continue")],
        tables: &[],
    },
    Section {
        title: "Thread properties",
        fields: &[
            fd("Number of threads (users)", "ThreadGroup.num_threads", Number, "1"),
            fd("Ramp-up period (seconds)", "ThreadGroup.ramp_time", Number, "1"),
            fd("Loop count (-1 for forever)", "ThreadGroup.main_controller/LoopController.loops", Number, "1"),
            fd("Same user on each iteration", "ThreadGroup.same_user_on_next_iteration", Bool, "true"),
            f("Delay thread creation until needed", "ThreadGroup.delayedStart", Bool),
            f("Specify thread lifetime", "ThreadGroup.scheduler", Bool),
            f("Duration (seconds)", "ThreadGroup.duration", Number),
            f("Startup delay (seconds)", "ThreadGroup.delay", Number),
        ],
        tables: &[],
    },
];

const HTTP_REQUEST: &[Section] = &[
    Section {
        title: "Web server",
        fields: SERVER_FIELDS,
        tables: &[],
    },
    Section {
        title: "Request",
        fields: &[
            fd("Method", "HTTPSampler.method", Choice(HTTP_METHODS), "GET"),
            f("Path", "HTTPSampler.path", Text),
            f("Content encoding", "HTTPSampler.contentEncoding", Text),
            fd("Redirect automatically", "HTTPSampler.auto_redirects", Bool, "false"),
            fd("Follow redirects", "HTTPSampler.follow_redirects", Bool, "true"),
            fd("Use KeepAlive", "HTTPSampler.use_keepalive", Bool, "true"),
            fd("Use multipart/form-data", "HTTPSampler.DO_MULTIPART_POST", Bool, "false"),
            fd("Browser-compatible headers", "HTTPSampler.BROWSER_COMPATIBLE_MULTIPART", Bool, "false"),
        ],
        tables: &[],
    },
    Section {
        title: "Timeouts (milliseconds)",
        fields: &[f("Connect", "HTTPSampler.connect_timeout", Number), f("Response", "HTTPSampler.response_timeout", Number)],
        tables: &[],
    },
    Section {
        title: "Parameters",
        fields: &[fd("Send the parameters as a body", RAW_BODY, Bool, "false"), fw("Body", BODY, Multiline, (RAW_BODY, true))],
        tables: &[Table {
            title: "Parameters sent with the request",
            path: "HTTPsampler.Arguments/Arguments.arguments",
            columns: HTTP_ARGUMENT_COLUMNS,
            shape: RowShape::HttpArgument,
            add_label: "Add Parameter",
            when: Some((RAW_BODY, false)),
        }],
    },
];

const HTTP_METHODS: &[(&str, &str)] = &[
    ("GET", "GET"),
    ("POST", "POST"),
    ("PUT", "PUT"),
    ("DELETE", "DELETE"),
    ("PATCH", "PATCH"),
    ("HEAD", "HEAD"),
    ("OPTIONS", "OPTIONS"),
    ("TRACE", "TRACE"),
];

const HTTP_DEFAULTS: &[Section] = &[
    Section { title: "Web server", fields: SERVER_FIELDS, tables: &[] },
    Section {
        title: "Request",
        fields: &[
            f("Path", "HTTPSampler.path", Text),
            f("Content encoding", "HTTPSampler.contentEncoding", Text),
            f("Implementation", "HTTPSampler.implementation", Choice(HTTP_IMPLEMENTATIONS)),
        ],
        tables: &[],
    },
    Section {
        title: "Timeouts (milliseconds)",
        fields: &[f("Connect", "HTTPSampler.connect_timeout", Number), f("Response", "HTTPSampler.response_timeout", Number)],
        tables: &[],
    },
    Section {
        title: "Parameters",
        fields: &[],
        tables: &[Table {
            title: "Parameters sent with every request",
            path: "HTTPsampler.Arguments/Arguments.arguments",
            columns: HTTP_ARGUMENT_COLUMNS,
            shape: RowShape::HttpArgument,
            add_label: "Add Parameter",
            when: None,
        }],
    },
];

const HTTP_IMPLEMENTATIONS: &[(&str, &str)] = &[("Java", "Java"), ("HttpClient4", "HttpClient4"), ("Default", "")];

const LOOP_CONTROLLER: &[Section] = &[Section { title: "Loop", fields: &[fd("Loop count (-1 for forever)", "LoopController.loops", Number, "1")], tables: &[] }];

const IF_CONTROLLER: &[Section] = &[Section {
    title: "If",
    fields: &[
        f("Condition", "IfController.condition", Text),
        fd("Interpret condition as a variable expression", "IfController.useExpression", Bool, "true"),
        fd("Evaluate for all children", "IfController.evaluateAll", Bool, "false"),
    ],
    tables: &[],
}];

const WHILE_CONTROLLER: &[Section] = &[Section { title: "While", fields: &[f("Condition (empty, or true, or false)", "WhileController.condition", Text)], tables: &[] }];

const FOREACH_CONTROLLER: &[Section] = &[Section {
    title: "ForEach",
    fields: &[
        f("Input variable prefix", "ForeachController.inputVal", Text),
        f("Start index for loop (exclusive)", "ForeachController.startIndex", Number),
        f("End index for loop (inclusive)", "ForeachController.endIndex", Number),
        f("Output variable name", "ForeachController.returnVal", Text),
        fd("Add an underscore before the number", "ForeachController.useSeparator", Bool, "true"),
    ],
    tables: &[],
}];

const TRANSACTION_CONTROLLER: &[Section] = &[Section {
    title: "Transaction",
    fields: &[
        f("Generate parent sample", "TransactionController.parent", Bool),
        f("Include duration of timer and pre-post processors in generated sample", "TransactionController.includeTimers", Bool),
    ],
    tables: &[],
}];

const RUNTIME_CONTROLLER: &[Section] = &[Section { title: "Runtime", fields: &[f("Runtime (seconds)", "RunTime.seconds", Number)], tables: &[] }];

const INCLUDE_CONTROLLER: &[Section] = &[Section { title: "Include", fields: &[f("Filename", "IncludeController.includepath", Text)], tables: &[] }];

const SWITCH_CONTROLLER: &[Section] = &[Section { title: "Switch", fields: &[f("Switch value", "SwitchController.value", Text)], tables: &[] }];

const CRITICAL_SECTION: &[Section] = &[Section { title: "Critical section", fields: &[f("Lock name", "CriticalSectionController.lockName", Text)], tables: &[] }];

const THROUGHPUT_CONTROLLER: &[Section] = &[Section {
    title: "Throughput",
    fields: &[
        fd("Execute", "ThroughputController.style", Choice(THROUGHPUT_STYLES), "0"),
        f("Throughput (executions)", "ThroughputController.maxThroughput", Number),
        f("Throughput (percent)", "ThroughputController.percentThroughput", Decimal),
        f("Per user", "ThroughputController.perThread", Bool),
    ],
    tables: &[],
}];

const THROUGHPUT_STYLES: &[(&str, &str)] = &[("Total executions", "0"), ("Percent executions", "1")];

const CONSTANT_TIMER: &[Section] = &[Section { title: "Delay", fields: &[fd("Thread delay (milliseconds)", "ConstantTimer.delay", Number, "300")], tables: &[] }];

const RANDOM_TIMER: &[Section] = &[Section {
    title: "Delay",
    fields: &[fd("Constant delay offset (milliseconds)", "ConstantTimer.delay", Number, "0"), fd("Random delay maximum (milliseconds)", "RandomTimer.range", Decimal, "100.0")],
    tables: &[],
}];

const GAUSSIAN_TIMER: &[Section] = &[Section {
    title: "Delay",
    fields: &[fd("Constant delay offset (milliseconds)", "ConstantTimer.delay", Number, "300"), fd("Deviation (milliseconds)", "RandomTimer.range", Decimal, "100.0")],
    tables: &[],
}];

const ASSERTION_FIELDS: &[(&str, &str)] = &[
    ("Text response", "Assertion.response_data"),
    ("Document (text)", "Assertion.response_data_as_document"),
    ("URL sampled", "Assertion.sample_label"),
    ("Response code", "Assertion.response_code"),
    ("Response message", "Assertion.response_message"),
    ("Response headers", "Assertion.response_headers"),
    ("Request headers", "Assertion.request_headers"),
    ("Request data", "Assertion.request_data"),
];

const ASSERTION_RULES: &[(&str, i64)] = &[("Contains", 2), ("Matches", 1), ("Equals", 8), ("Substring", 16)];

const RESPONSE_ASSERTION: &[Section] = &[
    Section {
        title: "Apply to",
        fields: &[fd("Apply to", "Assertion.scope", Choice(SCOPE_CHOICES), ""), f("Variable name", "Scope.variable", Text)],
        tables: &[],
    },
    Section {
        title: "Field to test",
        fields: &[
            fd("Test field", "Assertion.test_field", Choice(ASSERTION_FIELDS), "Assertion.response_data"),
            f("Ignore status", "Assertion.assume_success", Bool),
        ],
        tables: &[],
    },
    Section {
        title: "Pattern matching rules",
        fields: &[
            fd("Rule", "Assertion.test_type", BitChoice(ASSERTION_RULES), "16"),
            fd("Not", "Assertion.test_type", BitFlag(4), "16"),
            fd("Or", "Assertion.test_type", BitFlag(32), "16"),
        ],
        tables: &[Table {
            title: "Patterns to test",
            path: "Asserion.test_strings",
            columns: &[col("Pattern", "", Text)],
            shape: RowShape::Text,
            add_label: "Add Pattern",
            when: None,
        }],
    },
    Section { title: "Custom failure message", fields: &[f("Message", "Assertion.custom_message", Text)], tables: &[] },
];

const DURATION_ASSERTION: &[Section] = &[Section { title: "Duration", fields: &[f("Duration in milliseconds", "DurationAssertion.duration", Number)], tables: &[] }];

const SIZE_OPERATORS: &[(&str, &str)] = &[("=", "1"), ("!=", "2"), (">", "3"), ("<", "4"), (">=", "5"), ("<=", "6")];
const SIZE_FIELDS: &[(&str, &str)] = &[
    ("Full response", "SizeAssertion.response_network_size"),
    ("Response headers", "SizeAssertion.response_headers"),
    ("Response body", "SizeAssertion.response_data"),
    ("Response code", "SizeAssertion.response_code"),
    ("Response message", "SizeAssertion.response_message"),
];

const SIZE_ASSERTION: &[Section] = &[Section {
    title: "Size",
    fields: &[
        fd("Response size field to test", "Assertion.test_field", Choice(SIZE_FIELDS), "SizeAssertion.response_network_size"),
        f("Size in bytes", "SizeAssertion.size", Number),
        fd("Type of comparison", "SizeAssertion.operator", Choice(SIZE_OPERATORS), "1"),
    ],
    tables: &[],
}];

const JSON_ASSERTION: &[Section] = &[Section {
    title: "JSON path assertion",
    fields: &[
        f("JSON path", "JSON_PATH", Text),
        f("Validate against expected value", "JSONVALIDATION", Bool),
        f("Expected value", "EXPECTED_VALUE", Text),
        f("Expect null", "EXPECT_NULL", Bool),
        f("Invert assertion (will fail if above conditions met)", "INVERT", Bool),
        fd("Match as regular expression", "ISREGEX", Bool, "true"),
    ],
    tables: &[],
}];

const REGEX_USE: &[(&str, &str)] = &[
    ("Body", "false"),
    ("Body (unescaped)", "unescaped"),
    ("Body as a document", "as_document"),
    ("Response headers", "true"),
    ("Request headers", "request_headers"),
    ("URL", "URL"),
    ("Response code", "code"),
    ("Response message", "message"),
];

const REGEX_EXTRACTOR: &[Section] = &[
    Section { title: "Apply to", fields: EXTRACTOR_SCOPE, tables: &[] },
    Section {
        title: "Extract",
        fields: &[
            fd("Field to check", "RegexExtractor.useHeaders", Choice(REGEX_USE), "false"),
            f("Name of created variable", "RegexExtractor.refname", Text),
            f("Regular expression", "RegexExtractor.regex", Text),
            f("Template ($1$ for group 1)", "RegexExtractor.template", Text),
            f("Match number (0 for random, -1 for all)", "RegexExtractor.match_number", Number),
            f("Default value", "RegexExtractor.default", Text),
            f("Use empty default value", "RegexExtractor.default_empty_value", Bool),
        ],
        tables: &[],
    },
];

const JSON_EXTRACTOR: &[Section] = &[Section {
    title: "Extract",
    fields: &[
        f("Names of created variables", "JSONPostProcessor.referenceNames", Text),
        f("JSON path expressions", "JSONPostProcessor.jsonPathExprs", Text),
        f("Match numbers (0 for random)", "JSONPostProcessor.match_numbers", Text),
        f("Compute concatenation var (suffix _ALL)", "JSONPostProcessor.compute_concat", Bool),
        f("Default values", "JSONPostProcessor.defaultValues", Text),
    ],
    tables: &[],
}];

const BOUNDARY_EXTRACTOR: &[Section] = &[
    Section { title: "Apply to", fields: EXTRACTOR_SCOPE, tables: &[] },
    Section {
        title: "Extract",
        fields: &[
            fd("Field to check", "BoundaryExtractor.useHeaders", Choice(REGEX_USE), "false"),
            f("Name of created variable", "BoundaryExtractor.refname", Text),
            f("Left boundary", "BoundaryExtractor.lboundary", Text),
            f("Right boundary", "BoundaryExtractor.rboundary", Text),
            f("Match number (0 for random, -1 for all)", "BoundaryExtractor.match_number", Number),
            f("Default value", "BoundaryExtractor.default", Text),
            f("Use empty default value", "BoundaryExtractor.default_empty_value", Bool),
        ],
        tables: &[],
    },
];

const CSS_EXTRACTOR: &[Section] = &[
    Section { title: "Apply to", fields: EXTRACTOR_SCOPE, tables: &[] },
    Section {
        title: "Extract",
        fields: &[
            fd("Implementation", "HtmlExtractor.extractor_impl", Choice(CSS_IMPLEMENTATIONS), ""),
            f("Name of created variable", "HtmlExtractor.refname", Text),
            f("CSS selector expression", "HtmlExtractor.expr", Text),
            f("Attribute (empty for the text)", "HtmlExtractor.attribute", Text),
            f("Match number (0 for random, -1 for all)", "HtmlExtractor.match_number", Number),
            f("Default value", "HtmlExtractor.default", Text),
            f("Use empty default value", "HtmlExtractor.default_empty_value", Bool),
        ],
        tables: &[],
    },
];

const CSS_IMPLEMENTATIONS: &[(&str, &str)] = &[("Default", ""), ("JSOUP", "JSOUP"), ("JODD", "JODD")];

const XPATH_EXTRACTOR: &[Section] = &[
    Section { title: "Apply to", fields: EXTRACTOR_SCOPE, tables: &[] },
    Section {
        title: "Extract",
        fields: &[
            f("Use tidy (tolerant parser)", "XPathExtractor.tolerant", Bool),
            f("Name of created variable", "XPathExtractor.refname", Text),
            f("XPath query", "XPathExtractor.xpathQuery", Text),
            fd("Match number (0 for random, -1 for all)", "XPathExtractor.matchNumber", Number, "-1"),
            f("Default value", "XPathExtractor.default", Text),
            f("Validate XML", "XPathExtractor.validate", Bool),
            f("Namespace aware", "XPathExtractor.namespace", Bool),
        ],
        tables: &[],
    },
];

const SHARE_MODES: &[(&str, &str)] =
    &[("All threads", "shareMode.all"), ("Current thread group", "shareMode.group"), ("Current thread", "shareMode.thread")];

const CSV_DATA_SET: &[Section] = &[Section {
    title: "CSV data set",
    fields: &[
        f("Filename", "filename", Text),
        f("File encoding", "fileEncoding", Text),
        f("Variable names (comma-delimited)", "variableNames", Text),
        fd("Ignore first line", "ignoreFirstLine", Bool, "false"),
        fd("Delimiter (use \\t for tab)", "delimiter", Text, ","),
        fd("Allow quoted data", "quotedData", Bool, "false"),
        fd("Recycle on EOF", "recycle", Bool, "true"),
        fd("Stop thread on EOF", "stopThread", Bool, "false"),
        fd("Sharing mode", "shareMode", Choice(SHARE_MODES), "shareMode.all"),
    ],
    tables: &[],
}];

const HEADER_MANAGER: &[Section] = &[Section {
    title: "Headers",
    fields: &[],
    tables: &[Table { title: "Headers sent with every request", path: "HeaderManager.headers", columns: HEADER_COLUMNS, shape: RowShape::Header, add_label: "Add Header", when: None }],
}];

const USER_VARIABLES: &[Section] = &[Section {
    title: "User defined variables",
    fields: &[],
    tables: &[Table { title: "Variables", path: "Arguments.arguments", columns: ARGUMENT_COLUMNS, shape: RowShape::Argument, add_label: "Add Variable", when: None }],
}];

const COOKIE_MANAGER: &[Section] = &[Section {
    title: "Cookies",
    fields: &[
        f("Clear cookies each iteration", "CookieManager.clearEachIteration", Bool),
        f("Use thread group configuration to control cookie clearing", "CookieManager.controlledByThreadGroup", Bool),
    ],
    tables: &[],
}];

const CACHE_MANAGER: &[Section] = &[Section {
    title: "Cache",
    fields: &[
        f("Clear cache each iteration", "clearEachIteration", Bool),
        fd("Use Cache-Control/Expires header when processing GET requests", "useExpires", Bool, "true"),
        f("Max number of elements in cache", "maxSize", Number),
    ],
    tables: &[],
}];

const DEBUG_SAMPLER: &[Section] = &[Section {
    title: "Debug",
    fields: &[
        f("JMeter properties", "displayJMeterProperties", Bool),
        fd("JMeter variables", "displayJMeterVariables", Bool, "true"),
        f("System properties", "displaySystemProperties", Bool),
    ],
    tables: &[],
}];

const TEST_ACTION_TARGETS: &[(&str, &str)] = &[("Current thread", "0"), ("All threads", "2")];
const TEST_ACTIONS: &[(&str, &str)] = &[
    ("Stop", "0"),
    ("Pause", "1"),
    ("Stop now", "2"),
    ("Go to next loop iteration", "3"),
    ("Start next thread loop", "4"),
    ("Break current loop", "5"),
];

const TEST_ACTION: &[Section] = &[Section {
    title: "Action",
    fields: &[
        fd("Target", "ActionProcessor.target", Choice(TEST_ACTION_TARGETS), "0"),
        fd("Action", "ActionProcessor.action", Choice(TEST_ACTIONS), "1"),
        fd("Duration (milliseconds, for Pause)", "ActionProcessor.duration", Number, "0"),
    ],
    tables: &[],
}];

const SCRIPTED: &[Section] = &[Section { title: "Script", fields: SCRIPT_FIELDS, tables: &[] }];

const RESULT_FILE: &[Section] = &[Section {
    title: "Write results to file",
    fields: &[f("Filename", "filename", Text), f("Log/display only errors", "ResultCollector.error_logging", Bool)],
    tables: &[],
}];

const COMMENT_ONLY: &[Section] = &[];

/// The hand-made form for an element type, found by its XML tag and `guiclass`.
pub fn curated(tag: &str, guiclass: &str) -> Option<&'static [Section]> {
    Some(match (tag, guiclass) {
        ("TestPlan", _) => TEST_PLAN,
        ("ThreadGroup" | "SetupThreadGroup" | "PostThreadGroup", _) => THREAD_GROUP,
        ("HTTPSamplerProxy", "HttpTestSampleGui") => HTTP_REQUEST,
        ("ConfigTestElement", "HttpDefaultsGui") => HTTP_DEFAULTS,
        ("LoopController", _) => LOOP_CONTROLLER,
        ("IfController", _) => IF_CONTROLLER,
        ("WhileController", _) => WHILE_CONTROLLER,
        ("ForeachController", _) => FOREACH_CONTROLLER,
        ("TransactionController", _) => TRANSACTION_CONTROLLER,
        ("RunTime", _) => RUNTIME_CONTROLLER,
        ("IncludeController", _) => INCLUDE_CONTROLLER,
        ("SwitchController", _) => SWITCH_CONTROLLER,
        ("CriticalSectionController", _) => CRITICAL_SECTION,
        ("ThroughputController", _) => THROUGHPUT_CONTROLLER,
        ("OnceOnlyController" | "GenericController" | "RecordingController" | "TestFragmentController", _) => COMMENT_ONLY,
        ("ConstantTimer", _) => CONSTANT_TIMER,
        ("UniformRandomTimer" | "PoissonRandomTimer", _) => RANDOM_TIMER,
        ("GaussianRandomTimer", _) => GAUSSIAN_TIMER,
        ("ResponseAssertion", _) => RESPONSE_ASSERTION,
        ("DurationAssertion", _) => DURATION_ASSERTION,
        ("SizeAssertion", _) => SIZE_ASSERTION,
        ("JSONPathAssertion", _) => JSON_ASSERTION,
        ("RegexExtractor", _) => REGEX_EXTRACTOR,
        ("JSONPostProcessor", _) => JSON_EXTRACTOR,
        ("BoundaryExtractor", _) => BOUNDARY_EXTRACTOR,
        ("HtmlExtractor", _) => CSS_EXTRACTOR,
        ("XPathExtractor", _) => XPATH_EXTRACTOR,
        ("CSVDataSet", _) => CSV_DATA_SET,
        ("HeaderManager", _) => HEADER_MANAGER,
        ("Arguments", _) => USER_VARIABLES,
        ("CookieManager", _) => COOKIE_MANAGER,
        ("CacheManager", _) => CACHE_MANAGER,
        ("DebugSampler", _) => DEBUG_SAMPLER,
        ("TestAction", _) => TEST_ACTION,
        ("JSR223Sampler" | "JSR223PreProcessor" | "JSR223PostProcessor" | "JSR223Assertion" | "JSR223Timer" | "JSR223Listener", _) => SCRIPTED,
        ("ResultCollector", _) => RESULT_FILE,
        _ => return None,
    })
}

// ---- the generic form ----

/// A setting found on an element that has no hand-made form.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GenericField {
    pub prop: String,
    pub label: String,
    pub kind: Kind,
    /// A collection or nested group of settings, summarised and not editable here.
    pub summary_only: bool,
}

/// Fields for every simple setting saved on `node`, in file order.
pub fn generic_fields(node: &Node) -> Vec<GenericField> {
    node.props
        .iter()
        .map(|p| {
            let (kind, summary_only) = match p.kind {
                PropKind::Bool => (Bool, false),
                PropKind::Int | PropKind::Long => (Number, false),
                PropKind::Double => (Decimal, false),
                PropKind::String if p.value.contains('\n') => (Multiline, false),
                PropKind::String => (Text, false),
                PropKind::Collection | PropKind::Nested => (Summary, true),
            };
            GenericField { prop: p.name.clone(), label: label_for_prop(&p.name), kind, summary_only }
        })
        .filter(|f| f.prop != "TestPlan.comments")
        .collect()
}

/// A short, readable name for a setting that has no label of its own: `HTTPSampler.max_redirects` becomes
/// `Max redirects`.
pub fn label_for_prop(prop: &str) -> String {
    let last = prop.rsplit(['.', '/']).next().unwrap_or(prop);
    let words = last.replace('_', " ");
    let mut c = words.chars();
    match c.next() {
        Some(first) => first.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

/// The settings kept on every element, shown above the rest: the name is edited on the element itself.
pub const COMMENTS_PROP: &str = "TestPlan.comments";

#[cfg(test)]
mod tests {
    use super::*;
    use crate::templates;
    use std::collections::HashSet;

    /// Every field of a hand-made form must be a setting JMeter knows for that element: it is in the settings JMeter
    /// saves for a new element, or on the list of settings JMeter only saves once they are filled in.
    const OPTIONAL: &[&str] = &[
        "HTTPSampler.domain", "HTTPSampler.port", "HTTPSampler.protocol", "HTTPSampler.path", "HTTPSampler.contentEncoding", "HTTPSampler.connect_timeout", "HTTPSampler.response_timeout",
        "HTTPSampler.auto_redirects", "HTTPSampler.DO_MULTIPART_POST", "HTTPSampler.BROWSER_COMPATIBLE_MULTIPART", "HTTPSampler.implementation", "HTTPsampler.Arguments",
        "ThreadGroup.delayedStart", "ThreadGroup.scheduler", "ThreadGroup.duration", "ThreadGroup.delay", "ThreadGroup.main_controller",
        "TestPlan.serialize_threadgroups", "TestPlan.tearDown_on_shutdown", "TestPlan.functional_mode", "TestPlan.user_define_classpath", "TestPlan.user_defined_variables",
        "ForeachController.startIndex", "ForeachController.endIndex", "TransactionController.parent", "RunTime.seconds", "IncludeController.includepath", "SwitchController.value",
        "CriticalSectionController.lockName", "ThroughputController.percentThroughput", "Assertion.scope", "Scope.variable", "Sample.scope", "DurationAssertion.duration",
        "SizeAssertion.response_network_size", "JSONPostProcessor.compute_concat", "JSONPostProcessor.defaultValues", "JSONPostProcessor.referenceNames", "JSONPostProcessor.jsonPathExprs",
        "JSONPostProcessor.match_numbers", "HtmlExtractor.extractor_impl", "maxSize", "clearEachIteration", "CookieManager.clearEachIteration", "CookieManager.controlledByThreadGroup",
        "ResultCollector.error_logging", "filename", "Assertion.custom_message", "Assertion.test_strings", "Asserion.test_strings", "IfController.condition", "WhileController.condition",
        "HeaderManager.headers", "Arguments.arguments", "DEFAULT", "script", "parameters", "XPathExtractor.fragment", "Argument.value",
    ];

    fn template_props(tag: &str, gui: &str) -> HashSet<String> {
        let mut out = HashSet::new();
        for t in templates::all().iter().filter(|t| t.tag == tag && (gui.is_empty() || t.guiclass == gui)) {
            let pair = t.instantiate();
            collect(&pair.element, "", &mut out);
        }
        out
    }

    fn collect(e: &jmx_model::Element, prefix: &str, out: &mut HashSet<String>) {
        for c in e.elements() {
            if let Some(name) = c.attr("name") {
                let full = if prefix.is_empty() { name.clone() } else { format!("{prefix}/{name}") };
                out.insert(full.clone());
                collect(c, &full, out);
            }
        }
    }

    fn all_fields(sections: &[Section]) -> impl Iterator<Item = &Field> {
        sections.iter().flat_map(|s| s.fields.iter())
    }

    #[test]
    fn every_curated_form_names_settings_that_jmeter_knows() {
        let mut checked = 0;
        for t in templates::all() {
            let Some(sections) = curated(&t.tag, &t.guiclass) else { continue };
            let known = template_props(&t.tag, &t.guiclass);
            for fl in all_fields(sections) {
                checked += 1;
                let base = fl.prop.rsplit('/').next().unwrap_or(fl.prop);
                assert!(known.contains(fl.prop) || OPTIONAL.contains(&fl.prop) || OPTIONAL.contains(&base), "{}: {} is neither in JMeter's defaults nor a known optional setting", t.label, fl.prop);
            }
        }
        assert!(checked > 100, "{checked}");
    }

    #[test]
    fn forms_exist_for_the_everyday_elements() {
        for (tag, gui) in [
            ("ThreadGroup", "ThreadGroupGui"),
            ("HTTPSamplerProxy", "HttpTestSampleGui"),
            ("ConfigTestElement", "HttpDefaultsGui"),
            ("LoopController", "LoopControlPanel"),
            ("IfController", "IfControllerPanel"),
            ("ConstantTimer", "ConstantTimerGui"),
            ("ResponseAssertion", "AssertionGui"),
            ("RegexExtractor", "RegexExtractorGui"),
            ("JSONPostProcessor", "JSONPostProcessorGui"),
            ("CSVDataSet", "TestBeanGUI"),
            ("HeaderManager", "HeaderPanel"),
            ("Arguments", "ArgumentsPanel"),
        ] {
            assert!(curated(tag, gui).is_some(), "{tag}");
        }
    }

    #[test]
    fn curated_forms_cover_most_of_jmeters_menus() {
        let all = templates::all();
        let covered = all.iter().filter(|t| curated(&t.tag, &t.guiclass).is_some()).count();
        assert!(covered * 3 >= all.len(), "{covered} of {}", all.len());
    }

    #[test]
    fn field_properties_are_unique_within_a_form_except_for_flags_in_one_setting() {
        for t in templates::all() {
            let Some(sections) = curated(&t.tag, &t.guiclass) else { continue };
            let mut seen = HashSet::new();
            for fl in all_fields(sections) {
                if matches!(fl.kind, BitFlag(_) | BitChoice(_)) {
                    continue; // several flags share one integer setting
                }
                assert!(seen.insert(fl.prop), "{}: {} appears twice", t.label, fl.prop);
            }
        }
    }

    #[test]
    fn choices_include_the_default_and_have_unique_values() {
        for t in templates::all() {
            let Some(sections) = curated(&t.tag, &t.guiclass) else { continue };
            for fl in all_fields(sections) {
                if let Choice(opts) = fl.kind {
                    let mut seen = HashSet::new();
                    for (_, v) in opts {
                        assert!(seen.insert(*v), "{}: {} lists {v:?} twice", t.label, fl.label);
                    }
                    if !fl.default.is_empty() {
                        assert!(opts.iter().any(|(_, v)| *v == fl.default), "{}: {} has no option for its default {:?}", t.label, fl.label, fl.default);
                    }
                }
                if let BitChoice(opts) = fl.kind {
                    let bits: Vec<i64> = opts.iter().map(|(_, b)| *b).collect();
                    let unique: HashSet<_> = bits.iter().collect();
                    assert_eq!(bits.len(), unique.len());
                    assert!(bits.iter().all(|b| b.count_ones() == 1), "choices are single bits");
                }
            }
        }
    }

    #[test]
    fn every_table_path_exists_in_a_new_element_or_is_known() {
        for t in templates::all() {
            let Some(sections) = curated(&t.tag, &t.guiclass) else { continue };
            let known = template_props(&t.tag, &t.guiclass);
            for table in sections.iter().flat_map(|s| s.tables.iter()) {
                assert!(known.contains(table.path) || OPTIONAL.iter().any(|o| table.path.ends_with(o)), "{}: table {} is not on a new element", t.label, table.path);
            }
        }
    }

    #[test]
    fn the_generic_form_covers_every_simple_setting() {
        let plan = crate::Plan::from_jmx_str(
            "<jmeterTestPlan><hashTree><TestPlan testname=\"T\"/><hashTree><ThreadGroup testname=\"G\"/><hashTree><HTTPSamplerProxy guiclass=\"HttpTestSampleGui\" testname=\"S\"><stringProp name=\"HTTPSampler.path\">/a</stringProp><boolProp name=\"HTTPSampler.use_keepalive\">true</boolProp><intProp name=\"HTTPSampler.port\">80</intProp><doubleProp name=\"x.rate\">1.5</doubleProp><stringProp name=\"script\">line 1\nline 2</stringProp><collectionProp name=\"HTTPSampler.items\"/><stringProp name=\"TestPlan.comments\">hi</stringProp></HTTPSamplerProxy><hashTree/></hashTree></hashTree></hashTree></jmeterTestPlan>",
        )
        .unwrap();
        let n = plan.node(plan.find_by_name("S").unwrap()).unwrap();
        let g = generic_fields(n);
        let kinds: Vec<_> = g.iter().map(|f| (f.prop.as_str(), f.kind, f.summary_only)).collect();
        assert_eq!(
            kinds,
            [
                ("HTTPSampler.path", Text, false),
                ("HTTPSampler.use_keepalive", Bool, false),
                ("HTTPSampler.port", Number, false),
                ("x.rate", Decimal, false),
                ("script", Multiline, false),
                ("HTTPSampler.items", Summary, true),
            ],
            "the comments have their own box"
        );
        assert_eq!(g[0].label, "Path");
    }

    #[test]
    fn labels_are_made_readable() {
        assert_eq!(label_for_prop("HTTPSampler.max_redirects"), "Max redirects");
        assert_eq!(label_for_prop("a/b/Loops"), "Loops");
        assert_eq!(label_for_prop(""), "");
    }
}

// ---- new table rows ----

/// Java's `String.hashCode`, which JMeter uses to name the rows of a list of strings.
pub fn java_hash(s: &str) -> i32 {
    s.encode_utf16().fold(0i32, |h, u| h.wrapping_mul(31).wrapping_add(i32::from(u)))
}

/// A new table row of the given shape, with `cells` in the order of the table's columns (missing cells are
/// empty). The text is indented relative to its first line, to be inserted with `EditablePlan::add_row`.
pub fn new_row(shape: RowShape, cells: &[&str]) -> jmx_model::Element {
    use jmx_model::xml::escape;
    let cell = |i: usize| escape(cells.get(i).copied().unwrap_or(""), false);
    let name_attr = |i: usize| escape(cells.get(i).copied().unwrap_or(""), true);
    let xml = match shape {
        RowShape::Argument => format!(
            "<elementProp name=\"{}\" elementType=\"Argument\">\n  <stringProp name=\"Argument.name\">{}</stringProp>\n  <stringProp name=\"Argument.value\">{}</stringProp>\n  <stringProp name=\"Argument.metadata\">=</stringProp>\n</elementProp>",
            name_attr(0),
            cell(0),
            cell(1)
        ),
        RowShape::HttpArgument => format!(
            "<elementProp name=\"{}\" elementType=\"HTTPArgument\">\n  <boolProp name=\"HTTPArgument.always_encode\">{}</boolProp>\n  <stringProp name=\"Argument.value\">{}</stringProp>\n  <stringProp name=\"Argument.metadata\">=</stringProp>\n  <boolProp name=\"HTTPArgument.use_equals\">{}</boolProp>\n  <stringProp name=\"Argument.name\">{}</stringProp>\n</elementProp>",
            name_attr(0),
            cells.get(2).copied().unwrap_or("false"),
            cell(1),
            cells.get(3).copied().unwrap_or("true"),
            cell(0)
        ),
        RowShape::Header => format!(
            "<elementProp name=\"\" elementType=\"Header\">\n  <stringProp name=\"Header.name\">{}</stringProp>\n  <stringProp name=\"Header.value\">{}</stringProp>\n</elementProp>",
            cell(0),
            cell(1)
        ),
        RowShape::Text => format!("<stringProp name=\"{}\">{}</stringProp>", java_hash(cells.first().copied().unwrap_or("")), cell(0)),
    };
    jmx_model::xml::parse_fragment(&xml).expect("row templates are well formed")
}

#[cfg(test)]
mod row_tests {
    use super::*;
    use crate::EditablePlan;

    const PLAN: &str = "<jmeterTestPlan><hashTree>\n  <TestPlan testname=\"P\"/>\n  <hashTree>\n    <ThreadGroup testname=\"G\"/>\n    <hashTree/>\n  </hashTree>\n</hashTree></jmeterTestPlan>";

    fn with_sampler() -> (EditablePlan, crate::NodeId) {
        let mut e = EditablePlan::open(PLAN).unwrap();
        let g = e.plan().find_by_name("G").unwrap();
        let s = e.add(g, "HTTPSamplerProxy", "HttpTestSampleGui").unwrap();
        (e, s)
    }

    const PARAMS: &str = "HTTPsampler.Arguments/Arguments.arguments";

    #[test]
    fn java_hash_matches_java() {
        assert_eq!(java_hash(""), 0);
        assert_eq!(java_hash("a"), 97);
        assert_eq!(java_hash("hello"), 99162322);
        assert_eq!(java_hash("Hello World"), -862_545_276);
    }

    #[test]
    fn rows_of_every_shape_parse_and_escape() {
        for shape in [RowShape::Argument, RowShape::HttpArgument, RowShape::Header, RowShape::Text] {
            let r = new_row(shape, &["a & \"b\"", "<c>"]);
            assert!(!r.name.is_empty());
        }
        let r = new_row(RowShape::Text, &["x"]);
        assert_eq!(r.attr("name").as_deref(), Some(&*java_hash("x").to_string()));
        assert_eq!(r.text(), "x");
        let h = new_row(RowShape::HttpArgument, &["q", "v"]);
        assert_eq!(h.elements().filter(|c| c.attr("name").as_deref() == Some("HTTPArgument.use_equals")).count(), 1);
    }

    #[test]
    fn parameters_can_be_added_edited_removed_and_undone() {
        let (mut e, s) = with_sampler();
        let before = e.to_text();
        assert_eq!(e.row_count(s, PARAMS), 0);
        e.add_row(s, PARAMS, 0, new_row(RowShape::HttpArgument, &["id", "7"])).unwrap();
        e.add_row(s, PARAMS, 1, new_row(RowShape::HttpArgument, &["page", "2"])).unwrap();
        assert_eq!(e.row_count(s, PARAMS), 2);
        assert_eq!(e.row_cells(s, PARAMS, 1).iter().find(|(n, _)| n == "Argument.value").unwrap().1, "2");
        e.set_prop(s, &format!("{PARAMS}/#0/Argument.value"), "stringProp", "${id}").unwrap();
        assert_eq!(e.prop(s, &format!("{PARAMS}/#0/Argument.value")).as_deref(), Some("${id}"));
        e.remove_row(s, PARAMS, 0).unwrap();
        assert_eq!(e.row_count(s, PARAMS), 1);
        assert_eq!(e.row_cells(s, PARAMS, 0).iter().find(|(n, _)| n == "Argument.name").unwrap().1, "page");
        assert!(crate::Plan::from_jmx_str(&e.to_text()).is_ok());
        for _ in 0..4 {
            e.undo().unwrap();
        }
        assert_eq!(e.to_text(), before);
        while e.can_redo() {
            e.redo().unwrap();
        }
        assert_eq!(e.row_count(s, PARAMS), 1);
    }

    #[test]
    fn response_assertion_patterns_are_a_list_of_strings() {
        let (mut e, s) = with_sampler();
        let a = e.add(s, "ResponseAssertion", "AssertionGui").unwrap();
        let before = e.to_text();
        e.add_row(a, "Asserion.test_strings", 0, new_row(RowShape::Text, &["OK"])).unwrap();
        e.add_row(a, "Asserion.test_strings", 1, new_row(RowShape::Text, &["Welcome"])).unwrap();
        assert_eq!(e.row_text(a, "Asserion.test_strings", 1), "Welcome");
        e.replace_row(a, "Asserion.test_strings", 1, new_row(RowShape::Text, &["Hello"])).unwrap();
        assert_eq!(e.row_text(a, "Asserion.test_strings", 1), "Hello");
        e.undo().unwrap();
        assert_eq!(e.row_text(a, "Asserion.test_strings", 1), "Welcome", "the replace was one step");
        e.redo().unwrap();
        assert_eq!(e.row_text(a, "Asserion.test_strings", 1), "Hello");
        while e.can_undo() {
            e.undo().unwrap();
            if e.to_text() == before {
                break;
            }
        }
        assert_eq!(e.row_count(a, "Asserion.test_strings"), 0);
    }

    #[test]
    fn bad_tables_are_errors() {
        let (mut e, s) = with_sampler();
        assert!(e.add_row(s, "no/such/table", 0, new_row(RowShape::Header, &["a", "b"])).is_err());
        assert!(e.remove_row(s, PARAMS, 5).is_err());
        assert!(!e.can_undo() || e.row_count(s, PARAMS) == 0);
    }
}

#[cfg(test)]
mod body_tests {
    use super::*;
    use crate::EditablePlan;

    const PLAN: &str = "<jmeterTestPlan><hashTree>\n  <TestPlan testname=\"P\"/>\n  <hashTree>\n    <ThreadGroup testname=\"G\"/>\n    <hashTree/>\n  </hashTree>\n</hashTree></jmeterTestPlan>";

    fn request() -> (EditablePlan, crate::NodeId) {
        let mut e = EditablePlan::open(PLAN).unwrap();
        let g = e.plan().find_by_name("G").unwrap();
        let s = e.add(g, "HTTPSamplerProxy", "HttpTestSampleGui").unwrap();
        (e, s)
    }

    fn node(e: &EditablePlan, id: crate::NodeId) -> &Node {
        e.plan().node(id).unwrap()
    }

    #[test]
    fn the_body_and_the_parameters_table_show_in_turn() {
        let (mut e, s) = request();
        assert!(shown(Some((RAW_BODY, false)), node(&e, s)), "no body setting means the table");
        assert!(!shown(Some((RAW_BODY, true)), node(&e, s)));
        e.set_prop(s, RAW_BODY, "boolProp", "true").unwrap();
        assert!(shown(Some((RAW_BODY, true)), node(&e, s)));
        assert!(!shown(Some((RAW_BODY, false)), node(&e, s)));
        e.set_prop(s, RAW_BODY, "boolProp", "false").unwrap();
        assert!(shown(Some((RAW_BODY, false)), node(&e, s)));
        assert!(shown(None, node(&e, s)));
    }

    #[test]
    fn the_http_request_form_has_a_body_that_is_edited_as_text() {
        let sections = curated("HTTPSamplerProxy", "HttpTestSampleGui").unwrap();
        let body = sections.iter().flat_map(|s| s.fields.iter()).find(|f| f.prop == BODY).expect("a body field");
        assert_eq!(body.kind, Multiline);
        assert_eq!(body.when, Some((RAW_BODY, true)));
        let table = sections.iter().flat_map(|s| s.tables.iter()).find(|t| t.path == PARAMETERS).unwrap();
        assert_eq!(table.when, Some((RAW_BODY, false)));
    }

    #[test]
    fn the_body_path_reaches_the_value_of_the_first_row() {
        let (mut e, s) = request();
        assert_eq!(e.prop(s, BODY), None, "no row yet");
        assert!(e.set_prop(s, BODY, "stringProp", "x").is_err(), "a row has to be added first");
        e.add_row(s, PARAMETERS, 0, new_row(RowShape::HttpArgument, &["", "", "false", "false"])).unwrap();
        e.set_prop(s, BODY, "stringProp", "{\n  \"a\": 1\n}").unwrap();
        assert_eq!(e.prop(s, BODY).as_deref(), Some("{\n  \"a\": 1\n}"));
        let text = e.to_text();
        assert!(crate::Plan::from_jmx_str(&text).is_ok());
        let again = EditablePlan::open(&text).unwrap();
        assert_eq!(again.prop(again.plan().find_by_name("HTTP Request").unwrap(), BODY).as_deref(), Some("{\n  \"a\": 1\n}"), "the lines survive a save and a reload");
    }

    #[test]
    fn a_body_with_markup_stays_text() {
        let (mut e, s) = request();
        e.add_row(s, PARAMETERS, 0, new_row(RowShape::HttpArgument, &["", ""])).unwrap();
        let body = "<soap:Envelope a=\"1\">&amp;<![CDATA[x]]></soap:Envelope>\n</stringProp><evil/>";
        e.set_prop(s, BODY, "stringProp", body).unwrap();
        let again = EditablePlan::open(&e.to_text()).unwrap();
        let id = again.plan().find_by_name("HTTP Request").unwrap();
        assert_eq!(again.prop(id, BODY).as_deref(), Some(body));
        assert_eq!(again.plan().len(), 3, "the body did not add elements");
    }

    #[test]
    fn collections_and_groups_are_summaries_and_long_text_is_editable() {
        let plan = crate::Plan::from_jmx_str(
            "<jmeterTestPlan><hashTree><TestPlan testname=\"T\"/><hashTree><ThreadGroup testname=\"G\"/><hashTree><HTTPSamplerProxy guiclass=\"HttpTestSampleGui\" testname=\"S\"><stringProp name=\"script\">a\nb</stringProp><collectionProp name=\"c\"/><elementProp name=\"n\" elementType=\"X\"/></HTTPSamplerProxy><hashTree/></hashTree></hashTree></hashTree></jmeterTestPlan>",
        )
        .unwrap();
        let g = generic_fields(plan.node(plan.find_by_name("S").unwrap()).unwrap());
        let by = |p: &str| g.iter().find(|f| f.prop == p).unwrap();
        assert_eq!((by("script").kind, by("script").summary_only), (Multiline, false));
        assert_eq!((by("c").kind, by("c").summary_only), (Summary, true));
        assert_eq!((by("n").kind, by("n").summary_only), (Summary, true));
    }
}
