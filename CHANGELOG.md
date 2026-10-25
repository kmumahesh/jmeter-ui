# Changelog

All notable changes are listed here, newest first. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project follows [Semantic Versioning](https://semver.org/) once it has a first release.

## [Unreleased]

### Added
- Open, edit and save `.jmx` plans: one tab per plan, a form for every element, add, delete, move, drag and drop, undo and redo, a recovery copy, Scope & Order and variable hints.
- Run plans with the real JMeter: Run, Validate and Send once, stopping gently, at once or by force, live numbers and charts, failed requests and a table per request.
- Run history: every run in its own folder under `runs`, next to the plan or in a folder chosen in Settings; the last runs of a plan are kept (10 by default) and earlier runs can be opened again.
- Safety: hostile `.jmx` and results files are handled, nothing typed into a path or setting reaches the JMeter command line, JMeter is ended when the app closes or crashes (Windows), and only folders the app made are removed.
- Automatic checks on GitHub: build, tests (including those that start the real JMeter), clippy, known advisories, licences and code sources.

### Changed
- Damaged-input tests and libFuzzer targets for the plan and results readers; a coverage report is made on every push to `main`.
- A release workflow: a signed tag on `main` builds the Windows package, a list of dependencies (SBOM), checksums and signed build provenance, and creates a draft release.

### Fixed
- The `--state` file used by the real-window checks is written in one step, so it is never read half written.
- Run history is found for a plan however its path is spelled (short Windows names, `..` and slashes).
- A settings file saved with a byte order mark is read instead of being set aside.
