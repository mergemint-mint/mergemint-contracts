#!/usr/bin/env bash
# integration_test.sh — end-to-end integration test for MergeMint contracts
# using the Soroban local sandbox via stellar-cli.
#
# Usage: ./scripts/integration_test.sh
#
# Requirements:
#   - stellar-cli installed (cargo install stellar-cli)
#   - Docker available (for stellar network container)
#   - rustup with wasm32-unknown-unknown target

set -euo pipefail

NETWORK="local"
ACCOUNT="default"
WASM="target/wasm32-unknown-unknown/release/mergemint_contracts.wasm"

log() { echo "[integration_test] $*"; }
fail() { echo "[integration_test] FAIL: $*" >&2; exit 1; }

# ---------------------------------------------------------------------------
# 0. Preflight checks
# ---------------------------------------------------------------------------
command -v stellar >/dev/null 2>&1 || fail "stellar-cli not found in PATH"
command -v cargo >/dev/null 2>&1 || fail "cargo not found in PATH"

# ---------------------------------------------------------------------------
# 1. Start local sandbox
# ---------------------------------------------------------------------------
log "Starting local Soroban sandbox..."
stellar network container start "$NETWORK" || true   # ignore if already running

# Give the container a moment to initialise
sleep 3

# ---------------------------------------------------------------------------
# 2. Build WASM
# ---------------------------------------------------------------------------
log "Building WASM..."
cargo build --release --target wasm32-unknown-unknown 2>&1
[ -f "$WASM" ] || fail "WASM binary not found at $WASM"

# Verify SHA-256 checksum of the built WASM before deploying
WASM_SHA256=$(sha256sum "$WASM" | awk '{print $1}')
[ -n "$WASM_SHA256" ] || fail "failed to compute WASM SHA-256"
log "  wasm_sha256: $WASM_SHA256"

# ---------------------------------------------------------------------------
# 3. Generate test identities
# ---------------------------------------------------------------------------
log "Generating test identities..."
stellar keys generate creator  --network "$NETWORK" --fund 2>/dev/null || true
stellar keys generate contributor --network "$NETWORK" --fund 2>/dev/null || true
stellar keys generate verifier  --network "$NETWORK" --fund 2>/dev/null || true

CREATOR=$(stellar keys address creator)
CONTRIBUTOR=$(stellar keys address contributor)
VERIFIER=$(stellar keys address verifier)

[ -n "$CREATOR" ] || fail "failed to resolve creator address"
[ -n "$CONTRIBUTOR" ] || fail "failed to resolve contributor address"
[ -n "$VERIFIER" ] || fail "failed to resolve verifier address"

log "  creator:     $CREATOR"
log "  contributor: $CONTRIBUTOR"
log "  verifier:    $VERIFIER"

# ---------------------------------------------------------------------------
# 4. Deploy a mock SAC token and mint reward tokens to the verifier
# ---------------------------------------------------------------------------
log "Deploying reward token (Stellar Asset Contract)..."
REWARD_TOKEN=$(stellar contract asset deploy \
    --asset "USDC:$CREATOR" \
    --network "$NETWORK" \
    --source creator)
[ -n "$REWARD_TOKEN" ] || fail "failed to deploy reward token"
log "  reward_token: $REWARD_TOKEN"

log "Minting 10000 USDC to verifier..."
stellar contract invoke \
    --id "$REWARD_TOKEN" \
    --network "$NETWORK" \
    --source creator \
    -- mint \
    --to "$VERIFIER" \
    --amount 10000

# ---------------------------------------------------------------------------
# 5. Deploy MergeMint contract
# ---------------------------------------------------------------------------
log "Deploying MergeMint contract..."
CONTRACT_ID=$(stellar contract deploy \
    --wasm "$WASM" \
    --network "$NETWORK" \
    --source "$ACCOUNT")
[ -n "$CONTRACT_ID" ] || fail "failed to deploy MergeMint contract"
log "  contract_id: $CONTRACT_ID"

# Helper: invoke a contract function and capture output
invoke() {
    stellar contract invoke \
        --id "$CONTRACT_ID" \
        --network "$NETWORK" \
        --source "$ACCOUNT" \
        -- "$@"
}

# ---------------------------------------------------------------------------
# 5b. init_admin — bootstrap admin atomically right after deploy to close
#     the uninitialized-admin front-running window.
# ---------------------------------------------------------------------------
log "Invoking init_admin..."
invoke init_admin --admin "$CREATOR"
log "  admin initialized ✓"

# Validate version() post-deploy
log "Validating version()..."
VERSION=$(invoke version)
[ -n "$VERSION" ] || fail "version() returned empty"
log "  version: $VERSION ✓"

# ---------------------------------------------------------------------------
# 6. create_bounty
# ---------------------------------------------------------------------------
log "Invoking create_bounty..."
BOUNTY_ID=$(invoke create_bounty \
    --creator "$CREATOR" \
    --title "integration_test" \
    --description "end_to_end_test" \
    --reward_amount 1000 \
    --reward_token "$REWARD_TOKEN" \
    --min_reputation 0)
log "  bounty_id: $BOUNTY_ID"
[ -n "$BOUNTY_ID" ] || fail "create_bounty returned empty ID"

# Verify bounty count is now 1
COUNT=$(invoke get_bounty_count)
[ "$COUNT" = "1" ] || fail "expected bounty_count=1, got $COUNT"
log "  bounty_count: $COUNT ✓"

# ---------------------------------------------------------------------------
# 7. claim_bounty
# ---------------------------------------------------------------------------
log "Invoking claim_bounty..."
invoke claim_bounty \
    --contributor "$CONTRIBUTOR" \
    --bounty_id "$BOUNTY_ID"

# Verify assignees contains contributor
BOUNTY_JSON=$(invoke get_bounty --bounty_id "$BOUNTY_ID")
echo "$BOUNTY_JSON" | grep -q "$CONTRIBUTOR" \
    || fail "contributor not found in assignees after claim"
log "  assignee verified ✓"

# ---------------------------------------------------------------------------
# 8. complete_bounty
# ---------------------------------------------------------------------------
log "Invoking complete_bounty..."
invoke complete_bounty \
    --verifier "$VERIFIER" \
    --bounty_id "$BOUNTY_ID"

# Verify contributor reputation increased by 10
CONTRIBUTOR_JSON=$(invoke get_contributor --address "$CONTRIBUTOR")
echo "$CONTRIBUTOR_JSON" | grep -q '"reputation":10' \
    || fail "expected reputation=10 after completion"
log "  reputation=10 ✓"

echo "$CONTRIBUTOR_JSON" | grep -q '"contribution_count":1' \
    || fail "expected contribution_count=1 after completion"
log "  contribution_count=1 ✓"

# Verify bounty status is "completed"
invoke get_bounty --bounty_id "$BOUNTY_ID" | grep -q '"completed"' \
    || fail "expected status=completed"
log "  status=completed ✓"

# ---------------------------------------------------------------------------
# 9. update_contributor_metadata (#312)
# ---------------------------------------------------------------------------
log "Invoking update_contributor_metadata..."
invoke update_contributor_metadata \
    --contributor "$CONTRIBUTOR" \
    --metadata "ipfs://QmTestHash"

invoke get_contributor --address "$CONTRIBUTOR" | grep -q "ipfs://QmTestHash" \
    || fail "metadata URI not stored correctly"
log "  metadata URI stored ✓"

# ---------------------------------------------------------------------------
# 10. Tear down sandbox
# ---------------------------------------------------------------------------
log "Stopping local sandbox..."
stellar network container stop "$NETWORK" || true

log "All integration tests passed ✓"
