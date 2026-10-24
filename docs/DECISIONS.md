# Decision log

Newest entries last. Each entry says what changed and why. Defaults from plan v2 stand unless an entry overrides them.

## Toolkit: GPUI with our own component kit (plan v2, accepted 2026-10-18)
GPUI (`gpui` 0.2.x, Apache-2.0) is the UI toolkit. egui is the fallback if the first spike fails.

## Engine: drive the real JMeter via `java -jar ApacheJMeter.jar -n` (plan v2, accepted)
No native load engine. The UI builds a per-run copy of the plan, starts the JVM, tails the results, and signals stop over loopback UDP. User supplies Java and JMeter.

## Catalog source: Rust tool reading JMeter source (plan v2, accepted)
A Java dumper stays optional. See "JMeter-Plugins awareness" below for the plugin-related update.

## JMeter version: 5.6.3 only at first (plan v2, accepted)

## App licence: **GPL-3.0-or-later** (changed 2026-10-18, overrides plan v2 default of Apache-2.0)
- The app is released under GPL-3.0-or-later.
- Zed's GPL-3.0-or-later components (`ui`, `theme`, `picker`, `command_palette`, `workspace`, `editor`, `title_bar` and others) may be copied or adapted as needed.
- Apache-2.0 sources (GPUI, JMeter, JMeter-Plugins) may be used and referenced; Apache-2.0 code can be included in a GPL-3.0 work.
- Rules:
  1. Copied files keep their original copyright and licence headers. Record each copied file in `third_party/zed/PROVENANCE.md` with the Zed version (1.24.2) and original path.
  2. Check each crate's `license` line before copying. In Zed 1.24.2 (checked 2026-10-18): 207 crates are GPL-3.0-or-later (including `collab`), 34 are Apache-2.0, none are AGPL, and 8 have no licence line (`dev_container`, `editor_benchmarks`, `fs_benchmarks`, `grammars`, `language_core`, `project_benchmarks`, `tabular_data_preview`, `worktree_benchmarks`). Do not copy those 8 without confirming their terms. An earlier version of this log wrongly said `collab` was AGPL.
  2a. Switching the app to AGPL-3.0 was considered and rejected: nothing in Zed 1.24.2 needs it, and AGPL adds network-use source obligations.
  3. Fonts (IBM Plex Sans, Lilex: OFL) and Lucide icons (ISC) ship with their notices.
  4. Copy only what is needed; prefer GPUI-only code where Zed's component drags in many internal crates (see spike 1 notes).
- Effect: `ui-kit` is no longer clean-room. It is built from copied and adapted Zed components where that is cheaper. Plan v2's "own component kit" cost estimate for the UI kit and shell can drop; re-estimate after the spike measures how many internal Zed crates each component pulls in.
- This is not legal advice. If the app is ever distributed in a way that mixes in non-GPL-compatible code, revisit.

## JMeter-Plugins awareness (added 2026-10-18, from `jmeter-plugins-master`)
Findings:
- The repo is Apache-2.0. Its plugin catalog is JSON in `site/dat/repo/*.json` (about 130 entries). Each entry has `id`, `name`, `markerClass`, `componentClasses` (every class the plugin adds, GUI and element), and per-version `downloadUrl` plus `libs` (Maven Central URLs).
- 39 example plans in `examples/`, 32 with plugin elements (Dummy Sampler, Ultimate and Stepping Thread Groups, PerfMon, graphs, Throughput Shaping Timer, Parameterized Controller, UDP, HTTP Raw, AutoStop, Flexible File Writer). Plugin elements are saved with their full class name as the element tag and `guiclass`.
- Plugin graph listeners only visualise the results file; the app's native results screens replace them. They must still round-trip.
- Command-line tools exist (graph plotting from a results file, filter and merge results, test plan check, synthesis report). They are not required for 1.0.
- Plugin element labels are mostly hard-coded (`getStaticLabel`), not in resource files.

Decisions:
1. **Round-trip corpus** now includes the 39 plugin example plans plus the JMeter install and source plans.
2. **Missing-plugin detection (with editing):** when a plan contains a class that is not in the active JMeter's jars, show which plugin provides it (from `componentClasses`) and what to install. Detect installed plugins by scanning `lib/ext` jars for the `markerClass`.
3. **Plugin pack for the catalog (later):** curated forms for the most used plugin elements (Custom Thread Groups, Throughput Shaping Timer, Dummy Sampler, Parameterized Controller). Everything else uses the generic editor and is preserved unchanged.
4. **Installing plugins from inside the app is out of scope for 1.0.** The app shows what to install; the user installs it. Revisit after 1.0 (the catalog JSON already has the download URLs).
5. Option B (Java dumper) is still optional; plugin labels being hard-coded makes the Rust source reader less complete for plugins, so this is the main reason to add it later.

## Windows build prerequisite found missing (2026-10-18)
- Rust was updated from 1.91.0 to 1.99.1 (Zed's repo pins 1.98.1).
- MSVC 14.44 build tools are installed, but the Windows 10/11 SDK is not (no `kernel32.lib`, no Windows Kits folder). `cargo build` fails with "link.exe not found" because of this. Needed before any spike that compiles Rust.
- Resolved 2026-10-18: the user installed Windows SDK 10.0.26100 with winget; a test program now builds and runs.

## Depend on Zed's `ui` and `theme` crates directly (added 2026-10-18, after spike 1b)
- Because the app is GPL-3.0-or-later (see "App licence"), Zed's GPL crates `ui`, `theme`, `component`, `icons`, `menu`, `ui_macros` and `syntax_theme` are used as git dependencies pinned to tag `v1.24.2` (commit `fb69067`), not copied.
- This replaces most of the planned hand-written component kit: buttons, tabs, list items, context menus, popovers, tooltips, tree view items, indent guides, scrollbars, data table and modals already exist in `ui` (about 8,600 lines in those files alone).
- The app supplies its own `ThemeSettingsProvider` (fonts, sizes, density) and `AssetSource` (icons), and its own theme colours.
- Not depended on: `picker`, `command_palette`, `workspace`, `title_bar`, `project_panel`, `tab_switcher`, `editor`, `settings`. Each pulls 90 to 116 Zed crates. Their ideas and code are adapted into our own crates, with each copied file listed in `third_party/zed/PROVENANCE.md`.
- Risk: Zed's internal crate APIs change without notice. We pin one tag and upgrade deliberately.

## GPUI go decision (added 2026-10-18, after spikes 1, 1b, 5)
GPUI is confirmed. The egui fallback is dropped. Evidence is in `docs/PROGRESS.md`.

## Effort estimate changes (added 2026-10-18)
- UI kit and shell: 4 weeks down to about 3, because `ui`/`theme` are reused. Remaining work: dock and splits, our own tree and palette (adapted from `picker`), text and number inputs, code editor, charts.
- Other phases unchanged. Total about 23 weeks for one developer.
- Still open: multi-line code editor (nothing usable in GPUI, Zed's `editor` is not separable), docking.

## Add rules are for edits, not for opening plans (2026-10-18)
Running the add-where rules over all 177 sample plans found real plans that break JMeter's own `canAddTo` check: the WorkBench holds recorder elements, the recorder holds timers and listeners, and two old test files put a controller directly under the Test Plan. Decisions:
- A WorkBench category was added; non-test elements (recorder and the like) accept config elements, timers, assertions, processors and listeners.
- Plans are always shown as saved, even when they break the rules. The rules only decide what the palette and (once editing exists) drag and drop will allow. The two legacy files are listed as known exceptions in `crates/plan-core/tests/corpus.rs`.

## UI test strategy (2026-10-18)
Three layers: (1) logic crates tested without a window (`plan-core`, `shell-core`, `jmx-model`, `jvm-runtime`); (2) whole-window tests in GPUI's headless test mode (`crates/app/tests`), driving keys, clicks and drags; (3) a real-window smoke test (`tests/ui/smoke.ps1`) that starts the app, sends real mouse and keyboard input, drives the native file dialog and reads the app's `--state` file.
Headless mode does not open popover menus (they need real frames), so menus and drop-downs are verified in layer 3 only.

## Zed's menus need their keymap supplied (2026-10-18)
Zed's `ContextMenu` takes Up/Down/Enter/Escape from a keymap file in the Zed app. We bind the `menu::*` actions ourselves in `shell::bind_keys`. A menu opened from a drop-down starts with the current choice highlighted, so the first Down moves to the next option.

## Settings stored ahead of use (2026-10-18)
`startup` (welcome screen or reopen last plan), `heap_mb` (default 1024, JMeter's own default) and `summary_interval_secs` (default 1). They are stored now so the settings screen and number-field validation are real; runs will use them when running plans.

## One plan, one editor tab; the tree drives it (2026-10-23, before editing)
Changes the first shell design, where each clicked element opened its own tab and the tree and tabs were loosely tied. User decisions:
1. Opening a `.jmx` file connects the tree (left) and the editor (middle). They work in unison.
2. A plan has exactly one tab in the middle area. The tab is the plan, not an element.
3. Clicking an element in the tree redraws the middle area with that element's properties, ready to edit. No new tab opens.
4. Clicking samplers, listeners or any other element never opens a tab of its own. A second plan opened in the same window gets its own tab.
5. The search icon is removed from the left activity bar. Search lives in the top centre box (command palette, with `@` go-to-element and `+` add-element modes), which is enough.
6. The right-hand inspector is not removed yet. Its purpose was explained to the user, who decides whether to keep it (see "Inspector: kept, hidden by default, slimmed down").

Consequences for the code (not yet changed): the tab model in `shell-core` layout (one element tab per click) becomes one tab per plan; "Close tab", "Next/Previous tab" and the split editor need review (split may now show two plans, or be dropped); the tree selection becomes the single source of what the editor shows; `act-search` is removed from the activity bar and the Go to Element shortcut (Ctrl+P) stays; the headless and smoke tests that count tabs after clicking elements must be rewritten. The Plan overview and Problems views become sections of the plan view, not separate tabs. The editing forms are drawn into this single area.

## Inspector: kept, hidden by default, slimmed down (decided 2026-10-23)
The inspector's first content (name, type, status, counts, depth, XML tag, plan totals) duplicated the tree and editor. Decision by the user:
- Hidden by default (a fresh settings file shows tree and editor only). Toggle stays in the View menu, the palette and the status bar.
- It keeps only two sections: **Scope & Order** and **Can add**.
- **Scope & Order** (built with editing): for the selected element, which config elements, timers, assertions, processors and listeners apply to it and in what order JMeter runs them.
- **Can add**: the element types that may go under the selected element (the add rules). Each type name is clickable. Clicking adds the element to the plan immediately: it goes in as the last child of the selected element, becomes the selection, the editor redraws with its properties, and the add is one undo step. No dialog.
- Name, type, status, counts, depth, XML tag and plan totals are removed from the inspector. The element's name, type and enabled state belong in the editor header; plan totals stay in the plan view and status bar.
- The `+` palette mode and the context menu use the same add action, so all three entry points behave alike.

Because adding needs editing, the clickable "Can add" lands with editing. Until then, the slimmed inspector shows the list without clicking.

## Tabs, overview and saving (decided 2026-10-23, before editing)
Answers to the three questions asked before the one-tab-per-plan and inspector work:
- **Several plans, one tab each, no split editor.** Each open plan has exactly one tab. Selecting a plan's tab shows that plan's tree on the left. The split editor, "Split Right", "Unsplit" and tab dragging are removed. Settings stays a tab of its own.
- **Overview and Problems.** Selecting the Test Plan root shows its own properties plus the plan summary. Problems is a tab of the bottom panel with a count in the status bar.
- **Saving.** Ctrl+S writes the real `.jmx`. While a plan has unsaved edits, a recovery copy is autosaved every few seconds to `name.jmx.bak` in the same folder as the plan. A successful Save deletes the `.bak`. If a `.bak` is found next to a plan being opened and it is newer than the `.jmx`, the app offers to restore it or discard it. Save As is available. Closing a plan with unsaved edits asks first.

## One tab per plan, inspector and saving applied (2026-10-23)
Done as the first step of editing. One tab per plan (several plans may be open), no split editor, the tree selection alone decides what the editor shows, the search icon is gone from the activity bar, the inspector starts hidden and holds only Scope & Order and Can add. A fresh settings file hides the inspector; a settings file that already says it is visible keeps it that way.
Saving: Ctrl+S writes the file by writing a temporary file and renaming it over the plan, so a crash never leaves half a file. The recovery copy is written three seconds after the last change to `name.jmx.bak`, removed on save, on undoing back to the saved state, and when the answer to the unsaved-changes question is "Don't Save". Opening a plan that has a newer, different `.bak` asks whether to recover it. Closing a plan, closing all tabs, quitting, and reloading all ask first when there are unsaved changes.

## Plans are edited in place, and every change can be undone exactly (2026-10-23)
- Elements get an id (`uid`) when a plan is opened and keep it for as long as it is open. Selections, undo steps and expansion state use these ids, so they survive edits. `jmx-model` can add, remove, move and copy an element, set a setting (also inside another setting, for example a thread group's loop count), and add, remove and replace table rows, all while keeping every untouched byte of the file as it was.
- Each change is a step that can be undone and redone; undoing everything gives the opened file back byte for byte. This is tested by editing all 177 sample plans at random (4,850 edits), checking the plan is still valid after every edit and the file is identical after undoing them all.
- A setting that holds a number (`intProp`) and is given a `${variable}` becomes a string setting, because JMeter cannot read a variable as a number.
- Line breaks inside new text are written as the file already writes them (CRLF or LF).

## What can be added comes from JMeter itself (2026-10-23)
The list of elements and the XML of a new element are not hand-written. `tools/jmeter-catalog/gen-templates.groovy` runs inside the installed JMeter, reads JMeter's own "Add" menus, creates each element the way the GUI does and saves it the way JMeter saves a plan. The 127 results (JMeter 5.6.3) are in `crates/plan-core/data/templates.txt` and replace the hand-written list. Checks: the menu groups agree with the app's own classification of elements; a plan with one of each element loads in real JMeter without errors (`jmeter_load.rs`; the Module Controller is left out, because JMeter refuses to start a plan whose Module Controller has no target, as every new one has not).
Found on the way: Open Model Thread Group was unknown to the old table and showed as "Unknown"; elements missing from the table are now found through the menu data.

## Forms (2026-10-23)
- About 40 element types have a hand-made form (Test Plan, the three thread groups, HTTP Request and defaults, the main controllers, timers, assertions, extractors, CSV Data Set, header, cookie and cache managers, user variables, debug and flow-control samplers, script elements, result files). Every other element gets a form generated from the settings saved on it, so every element can be edited.
- The names of the settings in the hand-made forms are checked against the defaults JMeter generates (a unit test fails if a name is wrong), apart from a short list of settings JMeter only saves once they are filled in.
- A box saves on Enter, on leaving it (Tab, click elsewhere), and before anything else happens (Save, selecting another element, a menu command). Escape puts the saved value back. Number boxes refuse letters as they are typed and accept `${variables}`.
- Tables: HTTP parameters, headers, user defined variables, assertion patterns. Rows are added, removed and edited as undoable steps.
- Long, multi-line text (scripts, bodies) is shown read-only for now. A multi-line editor is a later task (open since the first spikes: nothing in GPUI to reuse, and Zed's `editor` cannot be separated).
- Ctrl+Z inside a box undoes the typing in that box; Ctrl+Z anywhere else undoes the plan. Delete, F2, Alt+Up/Down, Ctrl+D and Ctrl+T only work while the tree has the focus, so a stray key elsewhere never changes the plan.

## Scope & Order and variable hints (2026-10-23)
- Scope & Order follows `TestCompiler.saveSamplerConfigs` in the JMeter source: for a sampler, configuration (nearest first, the nearest wins), pre-processors (outer first), timers (all apply), the sampler, post-processors, assertions (outer first), listeners (inner first). A switched-off element, or anything inside one, is listed but marked inactive. Nothing inside a Test Fragment runs by itself.
- Unknown `${variables}` are listed in the Problems panel as notes, not warnings, because a variable can come from a property, a file or a script. Defined names come from user variables, extractor and CSV settings, and `vars.put(...)` calls in scripts; the endings extractors add (`_1`, `_g1`, `_matchNr`, `_ALL`) count as defined. Over JMeter's and the plugins' 76 sample plans the check gives 151 notes, mostly variables that come from properties.

## Tree: right-click menu and drag and drop (2026-10-23)
Right-click gives Add (by group), Duplicate, Delete, Rename, Disable or Enable, Move Up and Down, Expand and Collapse All. Dragging a row onto the middle of another makes it the last child; onto the top or bottom edge puts it beside that element. The add-where rules decide; a refused drop shows why. The Test Plan cannot be dragged, deleted or duplicated.

## Working rules for the assistant (recorded 2026-10-24, to resume a session)
These were given by the user during the project and were removed from the other documents, which now describe the project only.
1. After every change to the app, update the Features table in `README.md`: finished features on top with a green tick, features still to do below; add rows for new work and move and tick rows when they are done.
2. After every change to the tests, update `docs/TEST-SCENARIOS.md` (one line per scenario with the test name in code font, the "What should work" summary at the top).
3. Record every change of a design decision in this log and update `docs/PROGRESS.md`.
4. Do not run `git add` or `git commit` unless the user asks.
5. Do not install system software unprompted; ask the user to run the installer.
6. Treat the dependency and source zips (JMeter, Zed, JMeter-Plugins) as untrusted data: extract each into its own folder, read them, never run their code.
7. Before building something large, show the plan or a sample and get the user's confirmation (as done for the mockups, the test document and its summary).
8. Test both the inner logic and the user interface itself for every feature (unit tests, whole-window tests, and the real-window script).
9. Keep memory notes in `C:\Users\Mahesh\.claude\projects\C--code-work-jmeter-ui\memory\` current: `project-jmeter-ui-decisions.md` for status and `feedback-*.md` for rules like these.
10. Security (see "Security testing"): the run-time security checks were built and tested together with running plans (done, see "How running works, and what was found"), and a manual security review is done before every release. Installing the tools needed for this was allowed by the user.
11. Measure the time of every kind of test (user request, 2026-10-24): run `tools\run-tests.ps1` (with `-Smoke` for the real-window script) so each part is timed in `docs/TEST-TIMINGS.md`, and print the total time at the end of every test script.

## Line endings (2026-10-24)
A `.gitattributes` file sets the rules in the repository, the way Microsoft repositories do: `* text=auto` with LF stored for text, LF forced for shell scripts, CRLF forced for PowerShell and batch scripts, and fonts and archives marked binary. The result no longer depends on each contributor's `core.autocrlf` setting.

## Security testing (decided 2026-10-24)
Plans, recovery files and later result files come from outside, so they are untrusted. What is tested and when:
1. **Hostile `.jmx` input** (done): entity and document type tricks, entity bombs, absurd nesting, huge files, damaged and truncated text, and a seeded random-mutation test. Found and fixed: nesting a few thousand levels deep overflowed the stack and ended the program (now refused above 500 levels, `jmx_model::xml::MAX_DEPTH`); files over 256 MB are refused (`files::MAX_PLAN_BYTES`).
2. **Escaping** (done): hostile text in names, comments, values, attributes and table cells must come back as the same text and never change the structure. Found and fixed: adding a table row, duplicating or moving an element re-indented the lines inside a multi-line value and changed the text; only the blank text between elements is re-indented now.
3. **File safety** (done): a leftover temporary file is replaced, never written through (it could point at another file); a plan that is a link is written through the link; read-only targets, missing folders, long paths, reserved Windows names and recovery files that are not text give errors, not damage.
4. **Supply chain** (done): `cargo audit` and `cargo deny check` (`deny.toml`: vulnerabilities, a licence allow list, code sources), Dependabot, and a GitHub Actions workflow (`.github/workflows/ci.yml`) that builds, tests, runs clippy with warnings as errors, and checks dependencies weekly. Six dependencies of GPUI are reported as unmaintained (no vulnerability); they are listed with reasons in `deny.toml`.
5. **Process and port checks go with running plans**: arguments cannot be injected through paths, the stop port accepts connections from this machine only, a runaway JMeter can be killed and leaves no process, result files are read defensively (size, damaged lines), the HTML report opens only as a local file, and a plan with script elements (JSR223, BeanShell, OS Process) asks before it first runs.
6. **A manual security review happens before every release**, covering the run-time warning, the report opening, installer and signing, and a re-run of `cargo audit` and `cargo deny check`.
Not done on purpose: fuzzing with `cargo-fuzz` (it does not work reliably on Windows; the seeded mutation test runs on every build instead), and testing JMeter itself.

## Running plans: decisions (2026-10-24)
Decided with the user before running plans were built.
1. **Run uses the saved file only.** Pressing Run on a plan with unsaved changes asks "Save and run?"; on yes the plan is saved and the saved file is run; on no nothing runs. The `.jmx` is never copied or changed by a run.
2. **Script warning, asked once per plan file.** A plan with script elements (JSR223, BeanShell, OS Process, or a `__groovy`, `__BeanShell`, `__javaScript`, `__jexl3` function) asks before its first run. The answer is remembered per plan file path (`script_approved` in the settings file). Known weakness the user accepted: if the file is edited later by someone else to hold new scripts, it is not asked again.
3. **Run copies are not built.** The idea (several copies of a plan running at once) was dropped by the user. Distributed testing (remote machines) stays a separate to-do.
4. **Run history.** Every run (Run, Validate, Send once) gets its own folder `<runs location>/runs/<plan name>-<date>-<time>/`, holding the plan copy used for Validate and Send once, `results.jtl`, `jmeter.log`, the HTML report when requested, and `run.json` (plan path, kind, start, end, outcome and headline numbers).
   - **Where (Settings > Run history):** "Same folder as the .jmx file" (the default), or "Custom folder" chosen with a folder chooser button; the path cannot be typed. The chosen folder is the parent: the app creates and uses a folder named `runs` inside it. With a custom folder, all plans share that one `runs` folder, so it can be cleaned in one go.
   - **Before every run** the app checks that the place for run files is usable for the plan that is open: a custom folder that is not set, missing or not writable stops the run and offers "Use the .jmx folder", "Choose folder..." or Cancel. The place in use is shown in the run header.
   - **Keep the last N runs** (Settings, default 10, 1 to 1,000), counted per plan. Older run folders of that plan are removed with everything in them; the kept ones keep all their files (results, log, report). Only folders that hold the app's own `run.json` are ever removed.
   - **Clear runs folder** (Settings and the command palette) empties the shared custom `runs` folder after a confirmation. It is not offered for the `runs` folder next to a `.jmx`. It removes only run folders made by this app and never follows links.
   - **Not built:** pinning runs, and comparing two runs (planned with the results viewer).
5. **Future idea: a "project" concept.** A project would group plans, their run history, shared settings (JMeter and Java paths, run location, data files) and notes in one place, instead of tying run history to each `.jmx` file or one global folder. Not planned yet; to be looked at after the results viewer.

## How running works, and what was found (2026-10-24)
Built for running plans; the description for the user is `docs/RUNNING.md`.
1. **One run at a time.** Starting another while one is going is refused. (Run copies were dropped.)
2. **The three kinds share one path.** Run, Validate and Send once go through the same steps (runtime found, plan saved, place for the run files usable, scripts agreed) and the same run folder, state machine and history. Validate and Send once run a plan made from the saved plan and kept in the run folder as `plan.jmx`; Run runs the saved file itself and keeps a copy as `plan-as-run.jmx`.
3. **Live numbers come from the results file, not JMeter's console summary.** The file is read as it is written (bounded steps, damaged lines skipped), so the numbers match what is stored and do not depend on the summary interval. Percentiles are exact (nearest rank) from counts per millisecond; the time series merges neighbouring steps when it gets long, and the per-request and error lists are capped, so memory stays flat in a long run.
4. **Stop, stop now, force.** Stop sends `Shutdown` and Stop now sends `StopTestNow` to the UDP port chosen for the run; after 15 seconds an "End JMeter" button ends the process by force. A run the user stopped is kept as Stopped whatever the exit code.
5. **JMeter never outlives the app.** The run handle ends JMeter when dropped; on Windows each JMeter process is also put in a job object that ends it when the app's process ends, which covers a crash. Quitting during a run asks first.
6. **Command line safety.** Arguments are passed one by one, never through a shell. Before any start, `run::validate` refuses scripts as the Java program, Java options other than memory, `-X` and `-D` settings, property names outside letters, digits, `.`, `_` and `-`, and control characters in any path or value. Paths are made absolute (so a file named `-n.jmx` is not an option) and lose the `\\?\` prefix Windows gives to canonical paths.
7. **The stop port was checked, not assumed.** A test sends a stop message to this machine's own network address, as another computer would; JMeter 5.6.3 ignores it and obeys only the local one. The test stays so that a different JMeter that listened on all addresses would be noticed.
8. **Output is bounded.** At most 5,000 output lines wait for the window, each cut to 8,192 characters; more are dropped and counted. The results file is read at most 8 MB at a time with records over 1 MB dropped, and a file whose first line is not a results header gives no samples.
9. **Run history details.** A run that never recorded an end (the app closed during it) shows as Interrupted and is treated as an old run after a day. Keeping the last N is per plan, also when the folder is shared. Removing one run asks first and only removes a folder that holds this app's `run.json` and sits directly inside a folder named `runs`.
10. **The Run history settings sit after Runtime** in the Settings tab, and the Settings tab scrolls (it did not, which would have hidden the lower sections in a small window).
11. **Found by the real-window test:** a settings file with a byte order mark was set aside as damaged; the loader now ignores the mark.
12. **Every kind of test is timed.** `tools\run-tests.ps1` runs the unit and whole-window tests, clippy, `cargo audit`, `cargo deny` and, with `-Smoke`, the real-window script, and records each part's time in `docs/TEST-TIMINGS.md` (user request, 2026-10-24; also one of the working rules above).

