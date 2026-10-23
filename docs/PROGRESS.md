# Progress

## M0 spikes (finished 2026-10-18)
All five passed. Details are in the spike sections of the plan page and `docs/DECISIONS.md` (D7 to D10). Spike code stays in `spikes/` and `crates/jvm-runtime/examples/`.

## M1: UI kit and shell (finished 2026-10-18)
Browse plans in a coloured tree, command palette, tabs, panels, themes, settings. 267 automated tests and 26 real-window checks at the time. Details in `docs/DECISIONS.md` D11 to D14. M1 was built with one tab per clicked element and a split editor; both were replaced before M2 (D15, D17, D18).

## Design changes before M2 (decided 2026-10-23, applied the same day)
One tab per plan, the tree selection decides what the editor shows, no split editor, search only in the top box, the inspector hidden by default and cut down to Scope & Order and Can add (D15 to D18).

## M2: edit a plan (finished 2026-10-23)

What exists, all in `crates/`:

| Crate | What it holds | Tests |
|-------|---------------|-------|
| `jmx-model` | XML tree that keeps unchanged text byte for byte; element ids; add, remove, move, copy, settings (also nested ones), table rows | 42 |
| `plan-core` | Element catalog and the 127 templates generated from JMeter, add-where rules, plan model with stable ids, editing with undo and redo, forms, scope and order, variable check, saving and the recovery copy, fuzzy search, plan notes | 125 (9 of them check against real plans and real JMeter) |
| `shell-core` | Settings, layout (one tab per plan), text buffer, number rules, actions, palette logic | 87 |
| `jvm-runtime` | Java and JMeter discovery, run supervisor, results parsing | 6 |
| `app` | The window: theme, fields, combo box, palette, tree with drag and drop and right-click menu, tabs, forms and tables, inspector, results panel, dialogs, status bar, settings | 162 |
| `tools/jmeter-catalog` | The script that makes the templates by running JMeter itself | checked by the `plan-core` tests |

Total: **422 automated tests, all passing**, plus **50 real-window checks** in `tests/ui/smoke.ps1`.

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
- Running plans and results (M3 onwards).
- Untested: IME input, screen readers, high-DPI scaling, macOS and Linux. The plan is read again in full after each change, which is fine at 20,000 elements but is the first thing to make incremental if plans get much larger.

## Next
M3: run and watch (runtime screen, run copies, validate, live numbers and charts, stop controls).
