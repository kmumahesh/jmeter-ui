# Test timings

How long each kind of check took, one section per run of `tools\run-tests.ps1`, newest first. Times are wall-clock seconds on the developer's machine in a debug build; they show trends, not guarantees. The runs that start real JMeter processes are the slow ones.

## 2026-10-24 01:48 (after commit 5dc6719)

| Group | What | Seconds | Result |
|---|---|---:|---|
| Tests | jmeter_ui (unit tests), 39 tests | 0.2 | passed |
| Tests | editing, 39 tests | 77.9 | passed |
| Tests | fields, 18 tests | 0 | passed |
| Tests | forms, 37 tests | 15.9 | passed |
| Tests | runs, 35 tests | 19.1 | passed |
| Tests | shell, 53 tests | 7.5 | passed |
| Tests | jmx_model (unit tests), 23 tests | 0 | passed |
| Tests | runcopy, 5 tests | 0 | passed |
| Tests | security, 14 tests | 1.3 | passed |
| Tests | xml, 14 tests | 0 | passed |
| Tests | jvm_runtime (unit tests), 36 tests | 2.1 | passed |
| Tests | results_security, 14 tests | 4.5 | passed |
| Tests | run_jmeter, 22 tests | 14.9 | passed |
| Tests | plan_core (unit tests), 130 tests | 1.1 | passed |
| Tests | corpus, 6 tests | 0.6 | passed |
| Tests | derive_run, 3 tests | 11.5 | passed |
| Tests | edit_corpus, 2 tests | 2 | passed |
| Tests | jmeter_load, 1 tests | 4.4 | passed |
| Tests | security, 14 tests | 0.1 | passed |
| Tests | shell_core (unit tests), 89 tests | 0.5 | passed |
| Tests | settings_run, 6 tests | 0 | passed |
| Tests | all test programs, with building | 268.1 | passed |
| Lints | cargo clippy, warnings as errors | 34.2 | passed |
| Supply chain | cargo audit | 4 | passed |
| Supply chain | cargo deny check | 11.7 | passed |
| Real window | smoke.ps1 (61 of 61 checks passed. Screenshots: C:\code-work\jmeter-ui\target\ui-smoke) | 276.7 | passed |
| Total | everything above | 606 | passed |

