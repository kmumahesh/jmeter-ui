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
   - the workflows: every `uses:` in `.github/` is still pinned to a full commit hash, pending Dependabot updates to actions have been looked at, and the rules on `main` (required checks, signed commits) are still as written in `docs/DECISIONS.md`.
4. **Version and notes.** Set the version in `Cargo.toml`, move the `[Unreleased]` part of `CHANGELOG.md` under the new version with the date, and check that `README.md` (Features table, status) and `docs/PROGRESS.md` are current.
5. **Tag.** Create the signed tag on `main`: `git tag -s vX.Y.Z -m "..."`, then `git push origin vX.Y.Z`. Pushing the tag starts the release workflow (`.github/workflows/release.yml`); it refuses a tag that is not on `main`.
6. **Build.** The workflow builds with `--release --locked`, packs the program with the licence, README, changelog and third-party notices into `jmeter-ui-X.Y.Z-windows-x64.zip`, writes a list of dependencies (`jmeter-ui-X.Y.Z-sbom.cdx.json`, CycloneDX) and `SHA256SUMS.txt`, signs a build provenance statement for the files, and creates a **draft** GitHub release with all of them. A pull request that changes the workflow runs the same build as a dry run without releasing anything.
7. **Publish.** Check the draft release, add the notes from the changelog and the security review result, then publish it.
8. **After.** Start a new `[Unreleased]` section in `CHANGELOG.md`.

## Checking a download

- Compare the SHA-256 of the file with the line in `SHA256SUMS.txt` (`Get-FileHash file.zip` in PowerShell).
- Check that the file was built by this repository's workflow: `gh attestation verify jmeter-ui-X.Y.Z-windows-x64.zip --repo kmumahesh/jmeter-ui`.
- The SBOM lists every crate that went into the program with its version and licence.

The program is not signed with a Windows code-signing certificate yet, so Windows SmartScreen may warn when it is first run. That needs a certificate (a decision for later).

