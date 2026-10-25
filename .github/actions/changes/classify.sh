#!/usr/bin/env bash
# Reads changed file names (one per line) on standard input and prints `code` or `docs`.
#
# `docs` only when every file is documentation: anything under docs/, a Markdown file in the top folder,
# LICENSE, .gitignore, and the issue and pull request templates. Everything else counts as code, including a
# Markdown file inside a crate (it can be pulled into the build), a workflow, and an empty list.
set -u

docs_pattern='^(docs/|[^/]+\.md$|LICENSE$|\.gitignore$|\.github/(ISSUE_TEMPLATE/|PULL_REQUEST_TEMPLATE\.md$|CODEOWNERS$))'

files=$(cat)
if [ -z "$files" ]; then
  echo code
  exit 0
fi

if printf '%s\n' "$files" | grep -qvE "$docs_pattern"; then
  echo code
else
  echo docs
fi
