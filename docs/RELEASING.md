# Releasing

How a release is made. Work through the list top to bottom and do not skip the review.

## Branches and tags

- `main` is always releasable. Changes reach it only through pull requests with a passing CI run.
- Versions follow Semantic Versioning (`MAJOR.MINOR.PATCH`). A release is a signed, annotated tag `vX.Y.Z` on `main`.
- A branch `release/X.Y` is cut from the tag only when an older release needs a fix; the fix is made on `main` first and brought across.

## Checklist

1. **Everything is merged and green.** CI on `main` passes, including the tests that start the real JMeter.
2. **Full local run.** `powershell -ExecutionPolicy Bypass -File tools\run-tests.ps1 -Smoke` passes (this includes the real-window checks, unless they already run in CI) and the timings are in `docs/TEST-TIMINGS.md`.
3. **Manual security review.** Go through these and write the result in the release notes:
   - the run-time warning before scripts run, and what it says;
   - how result files and `.jmx` files are read (sizes, damaged lines, nesting);
   - opening the HTML report, if it exists in this release (local file only);
   - the installer and signing, if there is one;
   - a fresh `cargo audit` and `cargo deny check`, and a look at the six ignored "unmaintained" advisories in `deny.toml`: are they still the only ones, and are the reasons still true?
   - new dependencies since the last release: who maintains them and what do they do?
4. **Version and notes.** Set the version in `Cargo.toml`, move the `[Unreleased]` part of `CHANGELOG.md` under the new version with the date, and check that `README.md` (Features table, status) and `docs/PROGRESS.md` are current.
5. **Build.** The release workflow builds with `--release --locked`, makes checksums (SHA-256) and a list of dependencies (SBOM), and attaches them to a draft GitHub release.
6. **Tag.** Create the signed tag `git tag -s vX.Y.Z -m "..."` and push it.
7. **Publish.** Check the draft release, add the notes from the changelog and the security review result, then publish it.
8. **After.** Start a new `[Unreleased]` section in `CHANGELOG.md`.
