#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
#
# Prints the Markdown files covered by the docs spell and link checks, one per
# line: everything under docs/ and security/, every README, and CONTRIBUTING.md.
# Used by .github/workflows/docs-checks.yml and `make docs-check`.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"

git ls-files -- \
  'docs/*.md' \
  'security/*.md' \
  'README*.md' \
  '**/README*.md' \
  'CONTRIBUTING.md' | sort -u
