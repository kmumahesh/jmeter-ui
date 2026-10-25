# Progress

## Spikes (finished 2026-10-18)
All five passed. Details are in the spike sections of the plan page and `docs/DECISIONS.md`. Spike code stays in `spikes/` and `crates/jvm-runtime/examples/`.

## UI kit and shell (finished 2026-10-18)
Browse plans in a coloured tree, command palette, tabs, panels, themes, settings. 267 automated tests and 26 real-window checks at the time. Details in `docs/DECISIONS.md`. The shell was first built with one tab per clicked element and a split editor; both were replaced before editing was added.

## Design changes before editing (decided 2026-10-23, applied the same day)
One tab per plan, the tree selection decides what the editor shows, no split editor, search only in the top box, the inspector hidden by default and cut down to Scope & Order and Can add.

## Editing a plan (finished 2026-10-23)

What exists, all in `crates/`:

| Crate | What it holds | Tests |
|-------|---------------|-------|
| `jmx-model` | XML tree that keeps unchanged text byte for byte; element ids; add, remove, move, copy, settings (also nested ones), table rows | 42 |
| `plan-core` | Element catalog and the 127 templates generated from JMeter, add-where rules, plan model with stable ids, editing with undo and redo, forms, scope and order, variable check, saving and the recovery copy, fuzzy search, plan notes | 125 (9 of them check against real plans and real JMeter) |
| `shell-core` | Settings, layout (one tab per plan), text buffer, number rules, actions, palette logic | 87 |
| `jvm-runtime` | Java and JMeter discovery, run supervisor, results parsing | 6 |
| `app` | The window: theme, fields, combo box, palette, tree with drag and drop and right-click menu, tabs, forms and tables, inspector, results panel, dialogs, status bar, settings | 162 |
| `tools/jmeter-catalog` | The script that makes the templates by running JMeter itself | checked by the `plan-core` tests |

Total: **422 automated tests, all passing** (458 after the security tests below), plus **50 real-window checks** in `tests/ui/smoke.ps1`.

### What the user can do now
- Open plans, one tab each (the same plan twice just shows its tab). The tree on the left and the editor in the middle work together: click or arrow to an element and the editor shows its settings, never a new tab.
- Edit any element. Forms are hand-made for about 40 element types (Test Plan, thread groups, HTTP Request and defaults, main controllers, timers, assertions, extractors, CSV Data Set, header, cookie and cache managers, user variables, debug and flow-control samplers, script elements, result files) and generated from the saved settings for every other element. Boxes save on Enter or when left; numbers accept `${variables}`; checkboxes, drop-downs, flag groups, and tables (HTTP parameters, headers, variables, assertion patterns) work.
- Add elements from the palette (`+`), the inspector's Can add list, or the right-click menu, under the selected element, following JMeter's rules. Delete, duplicate, rename (F2 or the name box), enable or disable, move (Alt+Up and Alt+Down, drag and drop, menu). Undo and redo (Ctrl+Z, Ctrl+Y).
- Save with Ctrl+S, Save As; a recovery copy `name.jmx.bak` is written three seconds after a change, offered back after a crash, removed on save. Closing a plan, closing everything, quitting and reloading ask before unsaved work is lost.
- Inspector (Ctrl+Alt+B): Scope & Order for the selected element, and Can add.
- Problems panel: plan notes, and variables nothing defines.

### How it was tested
- `jmx-model` and `plan-core`: unit tests for every editing operation; a check that random edits on all 177 sample plans (4,833 edits) leave a valid plan and undo to the identical file; a plan with one of each of the 126 element types written by this code loads in real JMeter without errors (`tests/jmeter_load.rs`; skipped when Java or JMeter is missing); every setting named by a hand-made form exists in JMeter's own defaults; scope and order rules follow `TestCompiler`.
- Whole-window tests (`crates/app/tests/editing.rs`, `forms.rs`, `shell.rs`, `fields.rs`): add, delete, move, drag and drop, undo, save, recovery copy, questions before closing, forms, tables, shortcuts, focus.
- Real window (`tests/ui/smoke.ps1`): types in a form, clicks a checkbox, saves, adds and deletes, drags a row, opens the right-click menu, kills the app to leave a recovery copy and recovers it, closes the window with unsaved work.
- Performance: editing a 20,000-element plan takes about 30 ms per change in a release build (the plan is read again after each change). Whole-window timing was only measured in a debug build, about 1.5 s per interaction at that size; the test hooks for finding elements exist only in debug builds, so the release build was not driven.

### Found while testing (all fixed)
- A form lost the keyboard focus when saving a setting the element had not had before, so the next shortcut or keystroke went nowhere. The form now keeps the focus by name.
- Undoing a change to a setting whose value had odd text (`&gt;`, single quotes in a start tag, a number setting turned into a string) did not give back the same bytes; steps now keep the original text.
- Open Model Thread Group was missing from the element table and showed as "Unknown".
- Typing `${variable}` into a number setting would have saved text JMeter cannot read as a number; it now becomes a string setting.
- Focus events only reach a window that counts as active, which the headless tests did not set up.

### Not done
- Multi-line text (scripts, request bodies) is shown but cannot be edited here yet; it needs a multi-line editor.
- About 90 element types have only the generated list of settings, not a hand-made form with labels.
- Copy and paste of elements between plans, saving a selection as a fragment.
- Running plans and results (done later, see below).
- Untested: IME input, screen readers, high-DPI scaling, macOS and Linux. The plan is read again in full after each change, which is fine at 20,000 elements but is the first thing to make incremental if plans get much larger.

## Security testing (done 2026-10-24)
Added 29 automated tests (458 in all): hostile input and escaping in `jmx-model` (14), hostile text through every edit and file safety in `plan-core` (14), a hostile plan opened in the window (1). Two real defects were found and fixed: a deeply nested file overflowed the stack, and re-indenting changed multi-line values when a row was added or an element duplicated or moved. `cargo audit` shows no vulnerabilities (six unmaintained dependencies of GPUI, listed in `deny.toml`), `cargo deny check` passes, clippy is clean with warnings as errors, and the checks run on GitHub for every change.

## Run and watch (finished 2026-10-24)

Decisions made with the user are in `docs/DECISIONS.md` (running plans, and how running works); how it works for the user is in `docs/RUNNING.md`.

What exists:

| Part | What it does |
|---|---|
| `jvm-runtime::run` | Starts JMeter in non-GUI mode with each argument passed on its own, checks everything first (Java options, property names, paths, scripts as the Java program), reads output with bounded memory, stops it gracefully, at once or by force, ends it when the handle is dropped and, on Windows, when the app itself ends (job object). |
| `jvm-runtime::results`, `live` | Reads the results file while JMeter writes it (bounded steps, damaged or endless lines skipped and counted, not-a-results-file refused) and keeps the live numbers: totals, exact percentiles, a time series that merges when long, per-request tables, the latest failures. |
| `jvm-runtime::runs` | Run folders (`<place>/runs/<plan>-<date>-<time>/`), `run.json`, listing, keeping the last N, clearing the shared folder, checking the place before a run. Only folders holding this app's own `run.json` are ever listed or removed; links are never followed. |
| `plan-core::derive`, `scripts` | The plans made for Validate (one user, once, no timers) and Send once (one request with what applies to it), with relative file names made absolute; finding script elements and script functions. |
| `app` | Run, Stop, Stop now, Validate, Send once (F5, Shift+F5, Ctrl+Shift+F5, Ctrl+F5, F6, tree menu), the questions before a run (save, runs place, scripts), the Run tab (numbers, four charts, errors, per-request table, JMeter output), the History tab, Settings > Run history, Clear Runs Folder, status bar, quit during a run. |

Tests: **607 automated tests, all passing** (jmx-model 58, plan-core 157, shell-core 95, jvm-runtime 75, app 222), plus **61 real-window checks**. About 60 of the automated tests start the real JMeter against a small local web server. Every kind of test is now timed by `tools\run-tests.ps1` and recorded in `docs/TEST-TIMINGS.md`.

### How it was tested
- Security: arguments (paths with spaces, quotes, `&`, `%` and symbols still run; a plan named like an option; property names, values and Java options that could add to the command line are refused; scripts never used as Java), the stop port (a stop message sent to this machine's network address is ignored by JMeter), JMeter ended on kill, on drop and when the app crashes, a flood of output (150,000 lines and one 3 MB line) bounded without blocking the run, hostile results files (giant, endless, unclosed quote, binary junk, a 200 MB file read in steps, text that is not a results file), run folders (names that try to leave the folder, damaged or foreign `run.json`, links, removing only this app's folders, refusing to clear a folder not named `runs`), the script question.
- Functional: real runs through the window (F5 to the end with exact counts, Stop, Stop now, force, Validate, Send once with a request inside a looping controller, the custom folder, the last N kept with their files, a plan JMeter cannot read), the questions before a run, the History tab, Settings, and the same in the real window (`smoke.ps1` section 11).
- Performance: 1,000,000 samples into the live numbers in about 2 s (debug build); 500,000 rows of results read in about 4 s (debug); a 200 MB file read in 26 steps in about 3 s; 200,000 rows of a stored run read and shown in about 5 s; a run with 8,000 samples at full speed with the slowest redraw about 1 s (debug); JMeter start to first sample about 6 to 8 s (JMeter's own start-up).

### Found while testing (all fixed)
- A real-window check found that a settings file saved with a byte order mark (what Windows PowerShell and Notepad write) was treated as damaged and set aside; it is read now.
- The derived plan for Send once kept the request's own timers and listeners; they are removed.
- The stored-run reader accepted any text as a results file and counted its lines as samples; a file must now have a header naming `label` and `elapsed`, and lines with a damaged time stamp are skipped.
- The first GitHub run failed on 8 history tests: the runner's temporary folder has a short Windows name (`RUNNER~1`), and runs were matched to a plan by comparing the path as text, so a plan reached by the short name found no runs. Runs are now matched by the real path (a unit test and the app tests run under a short-name folder). The same problem could hit a plan opened through a shortcut or a different spelling of its path.
- The real-window checks (61) and the tests that start the real JMeter (about 60) now run on GitHub for every pull request and push to `main`. Getting the window checks to run on a shared runner took three changes: raise the screen to 1920x1080 (the runner starts at 1024x768, where a click meant for an empty title bar hit the Close button), let the script find JMeter through `JMETER_HOME`, and install Java and a checksum-verified JMeter in the workflow.
- Fuzzing and coverage added: repeatable damage tests for plans and results files in the normal test run, libFuzzer targets for the same readers on GitHub, and a coverage run on every push to `main`. The damage tests accepted every accepted plan back byte for byte on the first run, so they found nothing new.
- Linux and macOS: the app builds and the automated tests pass there in CI (with Java and JMeter installed). What it found: `wait` reported a process killed by a signal as "still running" (a real bug on Linux and macOS, fixed: it now returns 128 plus the signal); the tests pressed Ctrl for text shortcuts that the window binds to the platform key (Cmd on macOS; 18 tests); a process check read `/proc`, which macOS has not; and the crash test left the orphaned JMeter running off Windows. Known limits off Windows: JMeter is not ended if the app crashes (only when it closes normally), global shortcuts use Ctrl on macOS, and the window itself has only been run in tests there, not looked at.
- A release workflow was added (`.github/workflows/release.yml`): a signed tag on `main` builds the package, an SBOM, checksums and signed build provenance and creates a draft release; a pull request that changes it runs the build as a dry run. Not done: a Windows code-signing certificate and an installer.
- A real-window run on GitHub failed once with "Invalid JSON primitive": the checks read the `--state` file while the app was halfway through writing it. The app now writes it in one step (a test reads it continuously while it is rewritten; it failed before the change) and the script reads it again if it catches the file mid-change.
- Java on Windows reads command-line paths in the system code page, so a plan in a folder with Chinese letters is not found by JMeter. The run header warns about it (a limit of JMeter, not fixable here).
- `canonicalize` gives `\\?\C:\...` paths on Windows, which Java does not understand; the prefix is removed before a path reaches JMeter.

### Not done
- Result listeners as in JMeter (View Results Tree with request and response), the Aggregate Report as its own view, and opening the HTML dashboard report (planned with the results viewer).
- Comparing two runs and pinning runs (planned with the results viewer).
- Running several copies of a plan: dropped by the user.
- The project concept (idea for later, in `docs/RUNNING.md`).
- Untested: running with a JMeter other than 5.6.3, runs longer than a few minutes, macOS and Linux (the job object that ends JMeter when the app crashes is Windows-only; elsewhere JMeter is ended when the app ends cleanly).

## Repository and CI practice (done 2026-10-25)

| Part | State |
|---|---|
| Branch rules | `main` protected (pull request, five required checks, signed commits, no force-push, no bypass); see `docs/DECISIONS.md` |
| Checks on every change | build, tests and clippy on Windows; build and tests on Linux and macOS (with real JMeter); advisories, licences and sources; code scanning; the 62 real-window checks; fuzzing and coverage when relevant |
| Release | signed tag on `main` builds the package, SBOM, checksums and build provenance as a draft release (`docs/RELEASING.md`) |
| Repository files | `SECURITY.md` (private reporting is on), `CONTRIBUTING.md`, `CODE_OF_CONDUCT.md`, `CHANGELOG.md`, templates, `CODEOWNERS`, pinned Rust toolchain |
| Supply chain | actions pinned to commit hashes, tools pinned by version, JMeter download checked by checksum, Dependabot for crates and actions |
| Not done | Windows code-signing certificate and installer; telemetry and crash reports (left for a later release); screen-reader accessibility; Cmd shortcuts on macOS; JMeter ended on a crash off Windows |

## Guide and comparison with Apache JMeter (done 2026-10-25)

`docs/guide/index.html` is a single-page guide with a sidebar, contents panel and search. It explains the performance test types, shows how Apache JMeter 5.6.3 is used for recording, scripting, correlating, running (locally and distributed) and reporting, and records the same tasks tried in this app with the same example plan (`docs/guide/demo`).

Found while trying the tasks (not fixed yet; details and suggested fixes are in the guide):
- Opening a plan of about 15,000 requests takes 11 seconds and one of 40,000 does not finish in a minute, with the window not responding.
- The Users tile reads 1 after a finished run.
- The per-request table is cut off and has no Total row.
- Smaller points: inherited settings are invisible in the request form, "No matches" in the palette does not point to add mode, long element names are cut without an ellipsis, the form keeps its scroll position for the next element.
- Missing compared with JMeter: recorder, templates, new plan, copy and paste, multi-line editor, expression tester, request and response viewer, opening result files, HTML report, remote testing.

## Getting the app ready for testers (started 2026-10-25)

Goal: performance engineers on Windows can finish an HTTP(S) API test, then a web test, inside the app. A build goes to testers when the list below is done. The order follows a tester's day: get a script in, edit it, find out why requests fail, share a result.

| # | Part | State |
|---|---|---|
| 1 | New plan from a starter (API, web site, empty) | done |
| 2 | Multi-line editor for request bodies and scripts | to do |
| 3 | Copy, cut and paste of elements, also between plans | to do |
| 4 | Import from cURL | to do |
| 5 | Request and response viewer, including failed requests | to do |
| 6 | Expression tester, variable values after a run | to do |
| 7 | Open result files, make and open the HTML report | to do |
| 8 | File pickers for CSV data, properties and `-J` overrides | to do |
| 9 | The small fixes found in the guide | to do |
| 10 | HTTP(S) Test Script Recorder | to do |
| 11 | Templates, Add Think Times to children | to do |
| 12 | Aggregate Report and graphs for each request | to do |
| 13 | Compare two runs | to do |
| 14 | Open large plans: measure the size that opens within 5 seconds, keep it as a limit | to do |

Each part is its own pull request with logic, window, security and speed tests.

## Next
Results (the Samples Explorer with request and response detail, comparing two runs, the generated HTML report, native versions of the dashboard graphs).

**Security work that goes with the results viewer:** opening the HTML report only as a local file, with its own tests.

**Before every release:** a manual security review : the run-time warning, report opening, installer and signing, and a fresh `cargo audit` and `cargo deny check`.
