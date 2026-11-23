#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
#
# Usage: scripts/deploy.sh [NETWORK] [ACCOUNT]
#
# Optional environment variables:
#   WASM_TARGET       Rust target to build for (default: wasm32-unknown-unknown)
#   CONTRACT_ID_FILE  If set, the deployed contract ID is written to this file
#   ADMIN             Admin address passed to init_admin (default: $ACCOUNT)
#   WASM_SHA256       Expected SHA-256 of the built WASM; deploy aborts on mismatch
#   NETWORK_PASSPHRASE  Expected network passphrase; deploy aborts on mismatch
set -euo pipefail

NETWORK="${1:-sepolia}"
ACCOUNT="${2:-default}"
WASM_TARGET="${WASM_TARGET:-wasm32-unknown-unknown}"
WASM_PATH="target/${WASM_TARGET}/release/mergemint_contracts.wasm"
ADMIN="${ADMIN:-$ACCOUNT}"

# --- Explicit env-var checks -------------------------------------------------
if ! command -v stellar >/dev/null 2>&1; then
  echo "error: 'stellar' CLI not found in PATH" >&2
  exit 1
fi
if ! command -v cargo >/dev/null 2>&1; then
  echo "error: 'cargo' not found in PATH" >&2
  exit 1
fi
if ! command -v sha256sum >/dev/null 2>&1; then
  echo "error: 'sha256sum' not found in PATH" >&2
  exit 1
fi

# --- Network-passphrase guard ------------------------------------------------
if [ -n "${NETWORK_PASSPHRASE:-}" ]; then
  ACTUAL_PASSPHRASE=$(stellar network passphrase --network "$NETWORK")
  if [ "$ACTUAL_PASSPHRASE" != "$NETWORK_PASSPHRASE" ]; then
    echo "error: network passphrase mismatch for '$NETWORK'" >&2
    echo "  expected: $NETWORK_PASSPHRASE" >&2
    echo "  actual:   $ACTUAL_PASSPHRASE" >&2
    exit 1
  fi
fi

echo "Building contract..."
cargo build --release --target "$WASM_TARGET"

if [ ! -f "$WASM_PATH" ]; then
  echo "error: built WASM not found at $WASM_PATH" >&2
  exit 1
fi

# --- SHA-256 WASM checksum verification --------------------------------------
WASM_SHA256_ACTUAL=$(sha256sum "$WASM_PATH" | awk '{print $1}')
echo "WASM SHA-256: $WASM_SHA256_ACTUAL"
if [ -n "${WASM_SHA256:-}" ]; then
  if [ "$WASM_SHA256_ACTUAL" != "$WASM_SHA256" ]; then
    echo "error: WASM checksum mismatch" >&2
    echo "  expected: $WASM_SHA256" >&2
    echo "  actual:   $WASM_SHA256_ACTUAL" >&2
    exit 1
  fi
  echo "WASM checksum verified."
fi

echo "Deploying to $NETWORK..."
# `stellar contract deploy` prints the contract ID on stdout and progress on stderr.
CONTRACT_ID=$(stellar contract deploy \
  --wasm "$WASM_PATH" \
  --network "$NETWORK" \
  --source-account "$ACCOUNT")

echo "Contract ID: $CONTRACT_ID"

# --- Atomic init_admin bootstrap ---------------------------------------------
# Invoke init_admin immediately after deploy to close the uninitialized-admin
# front-running window.
echo "Initializing admin ($ADMIN)..."
stellar contract invoke \
  --id "$CONTRACT_ID" \
  --network "$NETWORK" \
  --source-account "$ACCOUNT" \
  -- init_admin \
  --admin "$ADMIN"

# --- Post-deploy version() validation ----------------------------------------
DEPLOYED_VERSION=$(stellar contract invoke \
  --id "$CONTRACT_ID" \
  --network "$NETWORK" \
  --source-account "$ACCOUNT" \
  -- version)
echo "Deployed version: $DEPLOYED_VERSION"

if [ -n "${CONTRACT_ID_FILE:-}" ]; then
  echo "$CONTRACT_ID" > "$CONTRACT_ID_FILE"
fi

echo "Deployment complete!"
