# JMeter UI

A desktop front end for Apache JMeter written in Rust with GPUI. It edits `.jmx` plans and runs JMeter in non-GUI mode; it does not reimplement the load engine.

Status: open, edit, save, run and watch plans, with run history. The results viewer comes next. See `docs/PROGRESS.md`, `docs/DECISIONS.md` and, for running plans and run history, `docs/RUNNING.md`.

Guide: `docs/guide/index.html` (open it in a browser) explains how Apache JMeter is used and how this app compares, step by step.

Contributing: `CONTRIBUTING.md`. Security reports: `SECURITY.md`. Changes by version: `CHANGELOG.md`. How a release is made: `docs/RELEASING.md`.

Licence: GPL-3.0-or-later (`LICENSE`). Third-party code is listed in `third_party/zed/PROVENANCE.md`.

## Features

This app replaces the JMeter UI only; the test engine is the real JMeter, started in non-GUI mode.

| Feature | Done |
|---|:---:|
| Open `.jmx` test plans (file dialog, command line, recent list, last session), several plans at once, one tab each | ✅ |
| Test plan tree with element icons and colours, expand and collapse, keyboard navigation | ✅ |
| Editor that follows the tree selection (one tab per plan, no tab per element) | ✅ |
| Command palette: commands, go to element, add element, recent plans | ✅ |
| Edit every element in a form (hand-made for about 40 element types, generated from the saved settings for the rest) | ✅ |
| Edit tables: HTTP parameters, headers, user defined variables, assertion patterns | ✅ |
| Add any element JMeter offers in its Add menus (127 types), following JMeter's rules for what fits where | ✅ |
| Delete, duplicate, rename, comments, enable or disable, move up and down | ✅ |
| Drag and drop in the tree, right-click menu | ✅ |
| Undo and redo for every change | ✅ |
| Save, Save As, reload from disk; untouched parts of a file are saved exactly as they were | ✅ |
| Recovery copy (`name.jmx.bak`) while there are unsaved changes, offered back after a crash | ✅ |
| Unsaved-changes marks (also while typing) and questions before closing, quitting or reloading | ✅ |
| Scope & Order: what applies to a sampler and in what order JMeter runs it | ✅ |
| Problems panel: plan notes and variables nothing defines | ✅ |
| Plugin elements are kept and shown (generic form) | ✅ |
| Dark, light and follow-system themes, resizable panels, remembered window size | ✅ |
| Settings: Java and JMeter locations (checked), heap size, summary interval, startup behaviour | ✅ |
| Settings for run history: next to the `.jmx` file or a chosen folder (chooser button, not typed), how many runs to keep | ✅ |
| Run a plan in non-GUI mode with F5: it asks to save first and runs the saved file; stop it gracefully, at once, or by force | ✅ |
| Validate a plan (one user, once, no timers) and send one selected request once | ✅ |
| Live results while running: samples, throughput, average and 95th percentile, errors, users, charts, failed requests and a table per request | ✅ |
| Run history: every run in its own folder under `runs`, next to the `.jmx` or in a chosen folder, the last 10 kept per plan, earlier runs viewable again, a command to clear the shared folder | ✅ |
| Warning before the first run of a plan that contains scripts (asked once per plan file) | ✅ |
| Safe running: nothing typed anywhere can be added to the JMeter command line, the stop port answers this machine only, JMeter never outlives the app (even after a crash), runaway output and hostile result files are bounded | ✅ |
| Safe with hostile files: plans are read without ever fetching or reading other files, absurd nesting and sizes are refused, text typed into any box stays text, saving never writes to a file it should not | ✅ |
| Automatic checks on GitHub for every change: build, tests, lints, known vulnerabilities, licences, code sources (a change to documentation only skips the build and tests, so it is checked in seconds) | ✅ |
| Builds and passes all the automated tests on Windows, Linux and macOS for every change; the real-window checks run on Windows | ✅ |
| Release build: a signed version tag builds the Windows package, a list of dependencies (SBOM), checksums and signed build provenance, as a draft release | ✅ |
| Import from cURL (Ctrl+Shift+I): paste one or more `curl` commands, for example from a browser's "Copy as cURL", and each becomes an HTTP request with its headers, body or form fields; what was left out is written in the request's comment; the command is only read, never run | ✅ |
| Copy, cut and paste elements (Ctrl+C, Ctrl+X, Ctrl+V and the menus) with everything under them, inside a plan or between plans; a paste goes where JMeter's rules allow and is one undo step; save a selection as a plan file of its own; merge a plan file into the open plan | ✅ |
| Text area for request bodies and scripts: line numbers, colours for JSON, XML and `${variables}`, indenting, scrolling and undo; an HTTP request shows the body editor while it sends a body and the parameters table otherwise | ✅ |
| New plan (File > New Plan…, Ctrl+N): start from an API test, a web site test or an empty plan, choose where to save it, and it opens ready to edit and run | ✅ |
| Guide (`docs/guide`) that shows how Apache JMeter is used and compares this app with it, task by task, with an example plan and web shop | ✅ |
| Request viewer (Ctrl+Shift+R): what was sent and what came back for each request — request line, headers and body, status, response headers and body, assertion results and the requests made as part of it. Failed requests are kept by default, or choose all requests or none in Settings; a run keeps at most 500. Send once opens it on the answer. Works on stored runs too | ✅ |
| Aggregate Report with all percentiles and a Total row, and graphs over time for each request | ⬜ |
| Open a saved results file (`.jtl` or CSV, Ctrl+Shift+O) from anywhere and look at its numbers, charts, errors and requests like a run. A file with 300,000 rows opens in seconds while the window stays usable | ✅ |
| HTML report (Ctrl+Shift+H or the *Make report* button): made with JMeter's own report generator from the results of the run you are looking at, in the run folder (or next to a results file you opened), and opened in the browser; asking again opens the same report | ✅ |
| Compare two runs | ⬜ |
| Make and open the HTML dashboard report (the results saved by each run already work with JMeter's report generator) | ⬜ |
| Save a selection as a Test Fragment (wrapped in one, kept in the plan) | ⬜ |
| Templates (File > Templates) | ⬜ |
| Search and replace across the plan | ⬜ |
| HTTP(S) Test Script Recorder (proxy, certificate, request filtering) | ⬜ |
| Try an expression (Ctrl+Shift+T, or Tools): type a JSON Path, regular expression, XPath or CSS selector and some text, for example a response from the request viewer, and see what JMeter's own libraries find in it. An extractor you have selected fills in its own expression. Stops after 15 seconds, which also shows an expression that is too slow | ✅ |
| Variable values after a run: the request viewer lists the variables of the thread as they were after each request, so you can see what an extractor took | ✅ |
| Function Helper dialog (build a `${__function()}` call and see what it gives) | ⬜ |
| Distributed testing: remote hosts, remote start, stop and exit, key store set-up | ⬜ |
| Backend Listener setup | ⬜ |
| Properties editor (`user.properties`, `-J` options per run), log viewer, Start no pauses, Add Think Times to children | ⬜ |
| Hand-made forms for the other element types (about 90 have the generated form) | ⬜ |
| Keep the window responsive when opening very large plans (15,000 requests take 11 seconds, 40,000 do not finish in a minute) | ⬜ |
| Small fixes found in the guide: the Users tile after a run, the cut-off per-request table, inherited settings shown in the request form, a palette hint for add mode, long names with an ellipsis, the form scrolling back to the top | ⬜ |
| File pickers for CSV, include and script files | ⬜ |
| Opening the HTML dashboard report only as a local file, with its own security check (arrives with the report viewer) | ⬜ |
| Security review before every release (see `docs/PROGRESS.md`) | ⬜ |
| Project concept: group plans, their run history and shared settings (a future idea, not planned yet) | ⬜ |
| Telemetry and crash reporting (opt-in, planned for a later release) | ⬜ |
| Keyboard shortcut editor, accessibility pass, installer | ⬜ |
| Close the gaps listed under Platform support (Linux and macOS) | ⬜ |

## Platform support

The app builds and passes all automated tests on Windows, Linux and macOS for every change. These are the differences today.

| Area | Windows | Linux | macOS |
|---|:---:|:---:|:---:|
| Build and automated tests (about 600, including runs of the real JMeter) | ✅ | ✅ | ✅ |
| The 80 real-window checks that drive the actual window | ✅ | ⬜ | ⬜ |
| Used by hand | ✅ | ⬜ | ⬜ |
| JMeter ended when the app crashes | ✅ | ⬜ | ⬜ |
| JMeter ended when the app closes normally | ✅ | ✅ | ✅ |
| Text-field shortcuts use the platform key (Cmd on macOS) | ✅ | ✅ | ✅ |
| Menu and global shortcuts use the platform key (they use Ctrl on macOS today) | ✅ | ✅ | ⬜ |
| Installer and code signing | ⬜ | ⬜ | ⬜ |

## Prerequisites

**Windows:** Rust stable, Visual Studio 2022 C++ build tools, and the Windows 10/11 SDK. Java 8 or newer and a JMeter 5.6.3 install to run plans.

**Linux:** the system libraries listed in the "System libraries for the window toolkit" step of `.github/workflows/ci.yml`. **macOS:** the Xcode command line tools. On both, the app builds and passes its automated tests, but the window itself has only been exercised by those tests so far; JMeter is ended when the app closes normally, not when it crashes.

## Run
    cargo run -p jmeter-ui -- path\to\plan.jmx
    cargo run -p jmeter-ui -- --theme light --settings my-settings.json --state state.json

## Test
    cargo test --workspace --exclude gpui-spike --exclude zed-ui-spike
    powershell -ExecutionPolicy Bypass -File tools\run-tests.ps1 [-Smoke]    # everything, with each part timed in docs\TEST-TIMINGS.md
    powershell -ExecutionPolicy Bypass -File tests\ui\smoke.ps1      # real-window test; build first with cargo build -p jmeter-ui

Dependency checks (install once with `cargo install --locked cargo-audit cargo-deny`):

    cargo audit
    cargo deny check

The tests that run the real JMeter against a small local web server (about 60 of them, each run taking 5 to 10 seconds) skip themselves when Java or JMeter is missing.

Set `JMX_CORPUS` to a list of folders of `.jmx` files to run the catalog, add-rule and random-edit checks against more plans. Set `JMETER_HOME` to use a JMeter install other than `~/Downloads/apache-jmeter-5.6.3` for the test that loads a plan with every element in real JMeter.

## Layout
- `crates/jmx-model`: read, edit and write `.jmx` without disturbing unchanged text
- `crates/plan-core`: element kinds and templates, what can go where, the plan tree, editing and undo, forms, scope, variable checks, saving
- `crates/shell-core`: settings, layout, text editing, palette logic
- `crates/jvm-runtime`: finds Java and JMeter, runs plans, reads results
- `crates/app`: the window
- `tools/jmeter-catalog`: regenerates the element templates from a JMeter install
- `spikes/`: the first experiments
