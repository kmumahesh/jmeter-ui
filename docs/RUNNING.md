# Running plans and run history

JMeter UI does not run tests itself. It starts the real JMeter in non-GUI mode (`java -jar ApacheJMeter.jar -n`), reads what JMeter writes while it works, and shows it.

## Before the first run

Settings > Runtime must name a JMeter folder (the one that holds `bin/ApacheJMeter.jar`), and Java must be found (the Java setting, `JAVA_HOME` or `PATH`). Until both are found, Run, Validate and Send once say so and do not start.

## The three kinds of run

| Command | Shortcut | What it runs |
|---|---|---|
| Run Plan | F5 | The whole plan, exactly as saved. |
| Validate Plan | Ctrl+F5 | The whole plan with one user going through it once: every thread group gets one thread, one iteration, no schedule and no start delay, and timers and Backend Listeners are left out. This follows JMeter's own rules for checking a plan. |
| Send Selected Request Once | F6, or right-click a request > Send once | Only the selected request, together with what applies to it (configuration, pre-processors, post-processors and assertions, in the order JMeter takes them), in one thread group with one user and one iteration. Timers and listeners are left out. |

Validate and Send once work on a copy of the plan made for the run, so the plan you edit is never changed. Relative file names in the copy (CSV data files, script files, included plans) are made absolute so that JMeter finds them next to the original plan.

**A run always uses the saved file.** If the plan has unsaved changes, the app asks "Save before running?"; answer "Save and Run" or Cancel. Nothing is saved without your answer.

**Plans that contain scripts.** A plan with JSR223, BeanShell or BSF elements, an OS Process sampler, or a script function such as `${__groovy(...)}` can run code on your computer. The first time such a plan is run, the app asks. The answer is remembered for that plan file (by its path); the question is not asked again for it. If the file is later changed by someone else to hold new scripts, it is not asked again. Choosing Cancel remembers nothing.

## While it runs

The lower panel opens on the Run tab and shows the run:

- the state (Starting, Running, Stopping, Finished, Stopped, Failed), the time, and the folder of the run's files;
- samples, requests per second, average and 95th percentile response time, errors and active users;
- four charts: response time, requests per second, errors, users;
- three lists: the latest failed requests (with code and message), a table with one line per request (samples, average, minimum, maximum, 90th, 95th and 99th percentile, error rate), and the lines JMeter printed.

Percentiles use the nearest-rank method. The numbers are read from the results file as JMeter writes it, so they do not depend on how often JMeter prints its own summary.

**Stop** (Shift+F5) asks JMeter to let the running iterations finish. **Stop now** (Ctrl+Shift+F5) interrupts requests in progress. If a stop takes longer than 15 seconds, an "End JMeter" button appears that ends the process by force. A run that was stopped is kept in the history as Stopped, with the numbers up to that point.

Only one run goes at a time. Quitting during a run asks first; "Stop and Quit" ends the run and keeps its results so far.

## Run history

Every run, of any kind, gets its own folder:

    <place>/runs/<plan name>-<date>-<time>/
        run.json          what the run was, when it started and ended, how it ended, headline numbers
        results.jtl       every sample, as JMeter wrote it
        jmeter.log        JMeter's log
        plan-as-run.jmx   a copy of the plan that was run (for Run)
        plan.jmx          the plan made for the run (for Validate and Send once)

### Where the runs folder goes (Settings > Run history)

- **Same folder as the .jmx file** (the default): `runs` is a folder next to each plan.
- **Custom folder**: choose a folder with the "Choose folder..." button. The path cannot be typed. What you choose is the **parent**: the app creates and uses a folder named `runs` inside it, so you never choose the name of the `runs` folder itself. All plans share that one `runs` folder, so everything can be cleaned up in one place. Runs of different plans are told apart by the plan name in the folder name and by `run.json`.

If "Custom folder" is selected but no folder was chosen, the runs go next to the `.jmx` file until one is chosen.

**Before every run** the app checks the place for the plan that is open. If a chosen folder is missing, is not a folder, or cannot be written to, the run does not start and the app asks: "Use the .jmx Folder", "Choose Folder...", or Cancel. The folder used is shown in the Run tab for every run.

### How many runs are kept

Settings > Run history > "Keep the last runs of each plan" (default 10, from 1 to 1,000). After each run, the oldest runs of that plan beyond the limit are removed **with everything in their folders**; the runs that are kept keep all their files (results, log, plan copy, reports). The count is per plan, so a shared folder keeps 10 runs of each plan. Lowering the number removes the older runs of the open plan at once. A run that never recorded an end (the app was closed during it) is kept for a day and then treated like any other old run.

Only folders that hold this app's own `run.json` are ever listed or removed. A folder you made by hand inside `runs` is never touched.

### Looking back

The History tab of the lower panel lists the runs of the open plan, newest first, with start time, kind, how it ended, length, samples, error rate, average and 95th percentile. Click a run to see its numbers and charts again (read from its `results.jtl`). "Folder" opens the run's folder in the file manager. "Remove" asks first and removes that one run.

### Clearing the shared folder

"Clear Runs Folder..." (Run menu, Settings, command palette) empties the `runs` folder chosen in Settings, after a confirmation that says how many runs it will remove. It is only available when a custom folder is chosen: the `runs` folders next to `.jmx` files are never cleared this way. It removes only run folders made by this app, never follows links out of the folder, and leaves anything else in it alone.

## What the app does to keep this safe

- JMeter is started with each argument passed on its own, never through a shell; paths with spaces, quotes, `&`, `%` and symbols work. Java options are limited to memory and `-D` settings, property names to letters, digits, `.`, `_` and `-`, and a script (`.bat`, `.cmd`, `.ps1`...) is never accepted as the Java program.
- The port that stops a run is used by one run only and answers messages from this computer only.
- JMeter is ended when the app ends, including when the app crashes (Windows job object).
- The results file is read in bounded steps; damaged, endless or huge lines are skipped and counted, never loaded whole.
- JMeter output is bounded: if the window cannot keep up, extra lines are dropped and counted, and the run itself is never held up.
- On Windows, Java reads paths in the system code page, so a plan in a folder whose name has letters outside it (for example Chinese) may not be found by JMeter. The run header warns about this.

## Not built (yet)

- Running several copies of a plan at once: not wanted.
- Pinning runs so they are never removed, and comparing two runs (planned with the results viewer).
- Opening JMeter's HTML dashboard report (planned with the results viewer).
- **A project concept.** Today run history belongs to each `.jmx` file (or one shared folder) and settings are global. A "project" could group several plans with their run history, data files, shared settings (JMeter and Java paths, run location, environments) and notes in one place. This is an idea for later, to be looked at after the results viewer; nothing has been designed yet.
