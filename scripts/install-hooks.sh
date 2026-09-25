#!/usr/bin/env bash
# Installs the repository's git pre-commit hook.
#
# The hook runs the same format and lint checks as CI, but only for the parts
# of the repo touched by the commit:
#
#   *.rs in the contract crate (repo root)     cargo fmt --check, cargo clippy
#   *.rs in mergemint-backend/                 cargo fmt --check, cargo clippy
#   JS/TS sources                              eslint
#   JS/TS/JSON/CSS/Markdown/YAML               prettier --check
#
# Usage:  ./scripts/install-hooks.sh
# Bypass: git commit --no-verify   (CI still runs the same checks)
set -euo pipefail

HOOK_DIR="$(git rev-parse --git-path hooks)"
PRE_COMMIT="$HOOK_DIR/pre-commit"

mkdir -p "$HOOK_DIR"

cat > "$PRE_COMMIT" << 'EOF'
#!/usr/bin/env bash
# Installed by scripts/install-hooks.sh — re-run that script to update.
set -euo pipefail

cd "$(git rev-parse --show-toplevel)"

fail() {
  echo ""
  echo "ERROR: $1"
  echo "Fix the issues above and re-stage your changes (or bypass with 'git commit --no-verify')."
  exit 1
}

# Staged files that still exist (added, copied, modified, renamed).
mapfile -t STAGED < <(git diff --cached --name-only --diff-filter=ACMR)
if [ "${#STAGED[@]}" -eq 0 ]; then
  exit 0
fi

# ── Rust ─────────────────────────────────────────────────────────────────────

ROOT_RS=false
BACKEND_RS=false
for f in "${STAGED[@]}"; do
  case "$f" in
    mergemint-backend/*.rs | mergemint-backend/Cargo.toml) BACKEND_RS=true ;;
    *.rs | Cargo.toml) ROOT_RS=true ;;
  esac
done

if $ROOT_RS; then
  echo "▶ cargo fmt --check (contract)"
  cargo fmt --check || fail "Formatting check failed. Run 'cargo fmt' to fix."

  echo "▶ cargo clippy -- -D warnings (contract)"
  cargo clippy -q -- -D warnings || fail "Clippy found warnings (treated as errors)."
fi

if $BACKEND_RS; then
  echo "▶ cargo fmt --check (mergemint-backend)"
  (cd mergemint-backend && cargo fmt --check) ||
    fail "Formatting check failed. Run 'cargo fmt' in mergemint-backend/ to fix."

  echo "▶ cargo clippy --all-targets --all-features -- -D warnings (mergemint-backend)"
  (cd mergemint-backend && cargo clippy -q --all-targets --all-features -- -D warnings) ||
    fail "Clippy found warnings in mergemint-backend (treated as errors)."
fi

# ── JavaScript / TypeScript ─────────────────────────────────────────────────

ESLINT_FILES=()
PRETTIER_FILES=()
for f in "${STAGED[@]}"; do
  case "$f" in
    */node_modules/* | node_modules/* | */dist/* | target/* | sdk/generated/*) continue ;;
  esac
  case "$f" in
    *.js | *.jsx | *.mjs | *.cjs | *.ts | *.tsx)
      ESLINT_FILES+=("$f")
      PRETTIER_FILES+=("$f")
      ;;
    *.json | *.css | *.md | *.yml | *.yaml)
      PRETTIER_FILES+=("$f")
      ;;
  esac
done

if [ "${#PRETTIER_FILES[@]}" -gt 0 ]; then
  # Use the versions pinned in the root package.json so results match CI.
  run_js_tool() {
    local tool="$1"
    shift
    if [ ! -x "node_modules/.bin/$tool" ]; then
      fail "$tool is not installed. Install Node.js 20+ and run 'npm install' at the repo root."
    fi
    "node_modules/.bin/$tool" "$@"
  }

  echo "▶ prettier --check (${#PRETTIER_FILES[@]} file(s))"
  run_js_tool prettier --check --ignore-unknown "${PRETTIER_FILES[@]}" ||
    fail "Prettier found unformatted files. Run 'npx prettier --write ${PRETTIER_FILES[*]}' to fix."

  if [ "${#ESLINT_FILES[@]}" -gt 0 ]; then
    echo "▶ eslint (${#ESLINT_FILES[@]} file(s))"
    run_js_tool eslint --max-warnings=0 --no-warn-ignored "${ESLINT_FILES[@]}" ||
      fail "ESLint reported problems. Run 'npx eslint --fix ${ESLINT_FILES[*]}' to fix what it can."
  fi
fi

echo "✔ Pre-commit checks passed."
EOF

chmod +x "$PRE_COMMIT"
echo "Pre-commit hook installed at $PRE_COMMIT"

if [ ! -x node_modules/.bin/eslint ] || [ ! -x node_modules/.bin/prettier ]; then
  echo "Note: run 'npm install' at the repo root so the hook can run eslint and prettier."
fi
