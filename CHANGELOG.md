# Changelog

All notable changes are listed here, newest first. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project follows [Semantic Versioning](https://semver.org/) once it has a first release.

## [Unreleased]

### Added
- New Plan templates for load testing: login and reuse a token, data-driven (CSV), load test with a ramp and a time limit, and spike test.
- Add Think Times to Children (Edit menu): a pause before every request under the selected element, in one step.
- Import from a HAR file (File menu): the requests of a browser recording become HTTP Requests, grouped by page, with static files, preflights and cookies left out. Importing cURL commands or a recording with the Test Plan selected now puts the requests under the first thread group.
- Run properties (Ctrl+Shift+J): `name=value` lines kept for a plan and given to its runs as `-J` options, with loading from a `.properties` file.
- Browse buttons for the file settings of a CSV Data Set, an Include Controller and script elements.
- Open a JMeter results file (Ctrl+Shift+O) and look at it like a run.
- HTML report (Ctrl+Shift+H): made with JMeter's report generator from the results you are looking at, then opened in the browser.
- Try an expression (Ctrl+Shift+T): a JSON Path, regular expression, XPath or CSS selector tried on some text with JMeter's own libraries; an extractor fills in its own expression and a response from the request viewer fills in the text.
- The request viewer lists the variables of the thread after each request.
- Escape closes a question (except the one about recovering unsaved changes).
- Request viewer (Ctrl+Shift+R, or the Requests tab): what was sent and what came back for each request, including failed ones, with the checks that ran. Settings choose whether failed requests, all requests or none are kept.
- Import from cURL: paste curl commands and get HTTP requests, with headers, bodies and form fields.
- Copy, cut and paste elements (also between plans), save a selection as a plan file, and merge a plan file into the open plan.
- Text area for request bodies and scripts, with line numbers and colours for JSON, XML and `${variables}`; an HTTP request shows it while it sends a body.
- New plan (File > New Plan…, Ctrl+N): start from an API test, a web site test or an empty plan.
- Open, edit and save `.jmx` plans: one tab per plan, a form for every element, add, delete, move, drag and drop, undo and redo, a recovery copy, Scope & Order and variable hints.
- Run plans with the real JMeter: Run, Validate and Send once, stopping gently, at once or by force, live numbers and charts, failed requests and a table per request.
- Run history: every run in its own folder under `runs`, next to the plan or in a folder chosen in Settings; the last runs of a plan are kept (10 by default) and earlier runs can be opened again.
- Safety: hostile `.jmx` and results files are handled, nothing typed into a path or setting reaches the JMeter command line, JMeter is ended when the app closes or crashes (Windows), and only folders the app made are removed.
- Automatic checks on GitHub: build, tests (including those that start the real JMeter), clippy, known advisories, licences and code sources.

### Changed
- Damaged-input tests and libFuzzer targets for the plan and results readers; a coverage report is made on every push to `main`.
- The build and all automated tests now run on Linux and macOS in CI.
- `main` requires a pull request, signed commits and passing checks; every workflow action is pinned to a commit hash.
- A release workflow: a signed tag on `main` builds the Windows package, a list of dependencies (SBOM), checksums and signed build provenance, and creates a draft release.

### Fixed
- After a run, the Users tile shows the most users the run had instead of the last count (which was 1 at the end).
- The per-request table ends with a Total row and scrolls sideways in a narrow window instead of cutting columns.
- An empty setting of an HTTP Request (server, port, protocol, path, encoding, timeouts) shows the value it takes from HTTP Request Defaults, and which element it comes from.
- The command palette says how to add an element when nothing matches; long element names end in an ellipsis; another element opens at the top of its form.
- A text box hint written over several lines made the window stop with an error; only its first line is shown now.
- Escape in a text box of a question (cURL import, run properties, expression) closes the question.
- On Linux and macOS a run ended by a kill is reported as ended (it was reported as still running).
- The `--state` file used by the real-window checks is written in one step, so it is never read half written.
- Run history is found for a plan however its path is spelled (short Windows names, `..` and slashes).
- A settings file saved with a byte order mark is read instead of being set aside.
- A plan file that starts with a byte order mark is read and saved back with it; one with damaged text after it used to crash the reader.
- Selecting an element with a comment or a table value of several lines no longer turns the lines into spaces when the form is saved, and undoing a paste no longer leaves the plan marked as unsaved.
- Control characters that a plan file cannot hold are no longer written to it.
