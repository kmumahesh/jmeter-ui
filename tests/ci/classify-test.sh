#!/usr/bin/env bash
# Checks the rule that decides whether a change needs the build and tests (.github/actions/changes/classify.sh).
# Run: bash tests/ci/classify-test.sh
set -u
here=$(cd "$(dirname "$0")" && pwd)
classify="$here/../../.github/actions/changes/classify.sh"
failed=0

expect() { # expect <code|docs> <name> [<name>...]
  want=$1; shift
  got=$(printf '%s\n' "$@" | bash "$classify")
  if [ "$got" = "$want" ]; then echo "  [PASS] $want: $*"; else echo "  [FAIL] wanted $want, got $got: $*"; failed=1; fi
}

echo "Documentation only"
expect docs README.md
expect docs README.md CHANGELOG.md SECURITY.md
expect docs docs/guide/index.html docs/guide/img/a.jpg docs/guide/demo/shop-demo.jmx
expect docs docs/DECISIONS.md .gitignore LICENSE
expect docs .github/PULL_REQUEST_TEMPLATE.md .github/ISSUE_TEMPLATE/bug.yml .github/CODEOWNERS

echo "Code, or anything that could change the build"
expect code crates/app/src/shell.rs
expect code README.md crates/app/src/shell.rs
expect code docs/guide/index.html Cargo.toml
expect code Cargo.lock
expect code deny.toml
expect code rust-toolchain.toml
expect code .github/workflows/ci.yml
expect code .github/actions/setup-jmeter/action.yml
expect code .github/dependabot.yml
expect code tests/ui/smoke.ps1
expect code tools/run-tests.ps1
expect code crates/jmx-model/README.md
expect code fuzz/Cargo.toml
expect code third_party/zed/PROVENANCE.md
expect code .gitattributes

echo "Look-alikes and edge cases"
expect code docs.rs
expect code "docsx/readme.txt"
expect code README.md.rs
expect code NOTES.txt
expect code ""

[ "$failed" -eq 0 ] && echo "All checks passed." || { echo "Some checks failed."; exit 1; }
