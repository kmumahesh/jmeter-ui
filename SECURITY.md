# Security policy

## Reporting a vulnerability

Please do not open a public issue for a security problem.

Use GitHub's private reporting: open the **Security** tab of this repository and choose **Report a vulnerability**. Say what you found, how to reproduce it and which version you used. You will get an answer within 7 days. If the report is accepted, a fix is prepared in private and released with a note in `CHANGELOG.md`.

## What counts

The app opens `.jmx` files, starts the real JMeter, reads its result files and removes its own run folders. Reports are welcome about, for example:

- a `.jmx` file, results file or setting that makes the app read or write files it should not, run something it should not, or crash;
- anything typed into a path, property or setting that reaches the JMeter command line;
- the stop port answering other machines;
- a run folder removal that touches anything the app did not make.

Problems in Apache JMeter or Java themselves belong to those projects.

## Supported versions

There is no release yet. Once there is, only the newest release is supported.

## What is already checked

See `docs/DECISIONS.md` (Security testing) and `docs/TEST-SCENARIOS.md` for the hostile-input, command-line and file-safety tests, and `.github/workflows/` for the automatic checks (dependency advisories, licences, code scanning). A manual security review is done before every release (`docs/RELEASING.md`).
