# Contributing

Thanks for helping. This is a Rust desktop app (GPUI) that edits and runs Apache JMeter plans. Read `README.md` first, then `docs/DECISIONS.md` for why things are the way they are.

## Setup

- Rust (the version in `rust-toolchain.toml`). Windows 10 or 11 is the main platform; Linux and macOS build and pass the tests in CI (see the README for what is not finished there).
- Java 17 and Apache JMeter 5.6.3 to run the tests that start the real JMeter (set `JMETER_HOME`). Without them those tests skip themselves.
- Optional: `cargo install cargo-audit cargo-deny`.

## Working on a change

1. Branch from `main` (`feature/...`, `fix/...`, `docs/...`). Do not commit to `main` directly.
2. Keep the change small and focused.
3. Add tests with the change (changes to the plan or results readers also run the fuzz targets in CI; you can run them locally with `cargo +nightly fuzz run <target>` on Linux): logic tests, a test through the window, and, where it matters, security and performance tests. Hostile input (bad files, odd paths) is tested on purpose, not only the happy path.
4. Run `powershell -ExecutionPolicy Bypass -File tools\run-tests.ps1` (add `-Smoke` for the real-window checks). It runs the tests, clippy, `cargo audit` and `cargo deny`, and records how long each took in `docs/TEST-TIMINGS.md`. (On GitHub, a pull request that changes only documentation skips the build, tests and scans, so it is checked in seconds; anything else runs everything.)
5. Update the docs that go with it:
   - the Features table in `README.md` (done rows at the top, to do below);
   - `docs/TEST-SCENARIOS.md` (regenerate the list with `python tools/gen-test-doc.py`, write the summary by hand);
   - `docs/DECISIONS.md` for a design decision, and `docs/PROGRESS.md`;
   - `CHANGELOG.md` for anything a user would notice.
6. Open a pull request. It needs a passing CI run to merge.

## Style

- Write code that reads like the code around it: same naming, same amount of comments.
- `cargo clippy -- -D warnings` must be clean.
- No milestone names or decision numbers in code, docs or commit messages. Describe the work in plain words.
- Commit messages: a short subject line in the imperative, then a paragraph on why.

## Commits

Commits should be signed (SSH or GPG). Pull requests are merged only when CI passes.

## Licence

By contributing you agree that your work is released under GPL-3.0-or-later, the licence of this project.
