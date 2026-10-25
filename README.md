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
| New plan (File > New Plan…, Ctrl+N): start from an API test, a web site test or an empty plan, choose where to save it, and it opens ready to edit and run | ✅ |
| Guide (`docs/guide`) that shows how Apache JMeter is used and compares this app with it, task by task, with an example plan and web shop | ✅ |
| View Results Tree: what was sent and returned for each request, including failed ones (today a failed request shows only its code and message) | ⬜ |
| Aggregate Report with all percentiles and a Total row, and graphs over time for each request | ⬜ |
| Open saved result files (`.jtl` or CSV) from anywhere, compare two runs | ⬜ |
| Make and open the HTML dashboard report (the results saved by each run already work with JMeter's report generator) | ⬜ |
| Multi-line editor for scripts and request bodies, with syntax highlighting (a body is one line in a table cell today) | ⬜ |
| Copy, cut and paste elements (also between plans), merge a plan into another | ⬜ |
| Save a selection as a Test Fragment | ⬜ |
| Templates (File > Templates) and import from cURL | ⬜ |
| Search and replace across the plan | ⬜ |
| HTTP(S) Test Script Recorder (proxy, certificate, request filtering) | ⬜ |
| Expression tester (regex, JSON path, XPath, CSS) against a real response, variable values after a run, and the Function Helper dialog | ⬜ |
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
| The 62 real-window checks that drive the actual window | ✅ | ⬜ | ⬜ |
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
