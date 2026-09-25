#!/bin/bash
set -euo pipefail

# Generate TypeScript bindings from contract spec
echo "Generating TS bindings from contract spec..."

mkdir -p sdk/src/generated

# Generate bindings for each contract
for contract in compliance asset_token registry dividend; do
  echo "Generating bindings for $contract..."
  stellar contract bindings typescript \
    --contract-id "C_${contract}" \
    --output-dir "sdk/src/generated/$contract" \
    2>/dev/null || echo "Note: Ensure contract ABIs are available"
done

echo "Bindings generated in sdk/src/generated/"
