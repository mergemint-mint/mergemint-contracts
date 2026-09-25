#!/usr/bin/env bash
# SPDX-License-Identifier: MIT
#
# Usage: scripts/deploy.sh [NETWORK] [ACCOUNT]
#
# Optional environment variables:
#   WASM_TARGET       Rust target to build for (default: wasm32-unknown-unknown)
#   CONTRACT_ID_FILE  If set, the deployed contract ID is written to this file
set -euo pipefail

NETWORK="${1:-sepolia}"
ACCOUNT="${2:-default}"
WASM_TARGET="${WASM_TARGET:-wasm32-unknown-unknown}"
WASM_PATH="target/${WASM_TARGET}/release/mergemint_contracts.wasm"

echo "Building contract..."
cargo build --release --target "$WASM_TARGET"

echo "Deploying to $NETWORK..."
# `stellar contract deploy` prints the contract ID on stdout and progress on stderr.
CONTRACT_ID=$(stellar contract deploy \
  --wasm "$WASM_PATH" \
  --network "$NETWORK" \
  --source-account "$ACCOUNT")

echo "Contract ID: $CONTRACT_ID"
if [ -n "${CONTRACT_ID_FILE:-}" ]; then
  echo "$CONTRACT_ID" > "$CONTRACT_ID_FILE"
fi

echo "Deployment complete!"
