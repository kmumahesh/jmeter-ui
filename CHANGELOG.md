# Changelog

All notable changes are listed here, newest first. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project follows [Semantic Versioning](https://semver.org/) once it has a first release.

## [Unreleased]

### Added
- Import from cURL: paste curl commands and get HTTP requests, with headers, bodies and form fields.
- Copy, cut and paste elements (also between plans), save a selection as a plan file, and merge a plan file into the open plan.
- Text area for request bodies and scripts, with line numbers and colours for JSON, XML and `${variables}`; an HTTP request shows it while it sends a body.
- New plan (File > New Plan…, Ctrl+N): start from an API test, a web site test or an empty plan.
- Open, edit and save `.jmx` plans: one tab per plan, a form for every element, add, delete, move, drag and drop, undo and redo, a recovery copy, Scope & Order and variable hints.
- Run plans with the real JMeter: Run, Validate and Send once, stopping gently, at once or by force, live numbers and charts, failed requests and a table per request.
- Run history: every run in its own folder under `runs`, next to the plan or in a folder chosen in Settings; the last runs of a plan are kept (10 by default) and earlier runs can be opened again.
- Safety: hostile `.jmx` and results files are handled, nothing typed into a path or setting reaches the JMeter command line, JMeter is ended when the app closes or crashes (Windows), and only folders the app made are removed.
- Automatic checks on GitHub: build, tests (including those that start the real JMeter), clippy, known advisories, licences and code sources.

### Changed
- Damaged-input tests and libFuzzer targets for the plan and results readers; a coverage report is made on every push to `main`.
- The build and all automated tests now run on Linux and macOS in CI.
- `main` requires a pull request, signed commits and passing checks; every workflow action is pinned to a commit hash.
- A release workflow: a signed tag on `main` builds the Windows package, a list of dependencies (SBOM), checksums and signed build provenance, and creates a draft release.

### Fixed
- On Linux and macOS a run ended by a kill is reported as ended (it was reported as still running).
- The `--state` file used by the real-window checks is written in one step, so it is never read half written.
- Run history is found for a plan however its path is spelled (short Windows names, `..` and slashes).
- A settings file saved with a byte order mark is read instead of being set aside.
- A plan file that starts with a byte order mark is read and saved back with it; one with damaged text after it used to crash the reader.
- Selecting an element with a comment or a table value of several lines no longer turns the lines into spaces when the form is saved, and undoing a paste no longer leaves the plan marked as unsaved.
- Control characters that a plan file cannot hold are no longer written to it.
