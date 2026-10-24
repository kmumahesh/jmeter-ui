# JMeter UI

A desktop front end for Apache JMeter written in Rust with GPUI. It edits `.jmx` plans and runs JMeter in non-GUI mode; it does not reimplement the load engine.

Status: open, edit, save, run and watch plans, with run history. The results viewer comes next. See `docs/PROGRESS.md`, `docs/DECISIONS.md` and, for running plans and run history, `docs/RUNNING.md`.

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
| Automatic checks on GitHub for every change: build, tests, lints, known vulnerabilities, licences, code sources | ✅ |
| Result listeners as in JMeter: View Results Tree with request and response, Aggregate Report, graphs over time | ⬜ |
| Open saved result files (`.jtl`), compare two runs | ⬜ |
| Open the generated HTML dashboard report | ⬜ |
| Multi-line editor for scripts and request bodies, with syntax highlighting | ⬜ |
| Copy, cut and paste elements (also between plans), merge a plan into another | ⬜ |
| Save a selection as a Test Fragment | ⬜ |
| Templates (File > Templates) | ⬜ |
| Search and replace across the plan | ⬜ |
| HTTP(S) Test Script Recorder | ⬜ |
| Function Helper dialog and expression tester (regex, JSON path, XPath) | ⬜ |
| Backend Listener setup, distributed testing (remote start) | ⬜ |
| Properties editor (`user.properties`, `-J` options), log viewer | ⬜ |
| Hand-made forms for the other element types (about 90 have the generated form) | ⬜ |
| File pickers for CSV, include and script files | ⬜ |
| Opening the HTML dashboard report only as a local file, with its own security check (arrives with the report viewer) | ⬜ |
| Security review before every release (see `docs/PROGRESS.md`) | ⬜ |
| Project concept: group plans, their run history and shared settings (a future idea, not planned yet) | ⬜ |
| Keyboard shortcut editor, accessibility pass, installer | ⬜ |
| macOS and Linux builds | ⬜ |

## Prerequisites (Windows)
Rust stable, Visual Studio 2022 C++ build tools, and the Windows 10/11 SDK. Java 8 or newer and a JMeter 5.6.3 install to run plans.

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
