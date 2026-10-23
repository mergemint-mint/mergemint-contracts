#!/usr/bin/env bash
# Scans staged changes for secrets with gitleaks (#1117).
#
# Runs from the pre-commit hook installed by scripts/install-hooks.sh, and can
# also be run by hand. It is optional: if gitleaks is not installed, it prints
# a note and lets the commit through. CI runs the same scan either way.
#
# Install gitleaks: https://github.com/gitleaks/gitleaks#installing
# Skip once:        SKIP_GITLEAKS=1 git commit ...
# Hit a false positive? Add a narrow allowlist entry to .gitleaks.toml.
set -euo pipefail

cd "$(git rev-parse --show-toplevel)"

if [ "${SKIP_GITLEAKS:-}" = "1" ]; then
  echo "▶ gitleaks skipped (SKIP_GITLEAKS=1)"
  exit 0
fi

if ! command -v gitleaks > /dev/null 2>&1; then
  echo "▶ gitleaks not installed; skipping the secret scan (CI still runs it)."
  echo "  Install: https://github.com/gitleaks/gitleaks#installing"
  exit 0
fi

echo "▶ gitleaks (staged changes)"
if ! gitleaks git --pre-commit --staged --config .gitleaks.toml --redact --no-banner .; then
  echo ""
  echo "gitleaks found what looks like a secret in the staged changes."
  echo "Remove it from the commit. If it was real, rotate it: it may already be"
  echo "in your local history or a pushed branch."
  echo "If it is a false positive, add a narrow allowlist entry to .gitleaks.toml."
  exit 1
fi
