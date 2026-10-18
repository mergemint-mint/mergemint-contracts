# Stellar CLI Troubleshooting Guide

This guide covers common errors encountered when developing, deploying, and interacting with MergeMint contracts using the **Stellar CLI** on the Soroban Testnet, along with exact diagnostic commands and fixes.

> **Tested CLI Version:** `stellar-cli 23.0.1` (pinned per `CONTRIBUTING.md`)  
> **Toolchain Target:** `wasm32v1-none` (pinned in `rust-toolchain.toml`) / `wasm32-unknown-unknown`  
> **Network:** Soroban Testnet (`https://soroban-testnet.stellar.org:443`)  
> **Passphrase:** `Test SDF Network ; September 2015`

---

## Quick Diagnostic Commands

Run these first to verify your local CLI environment:

```bash
# 1. Check Stellar CLI version (recommended: 23.0.1)
stellar --version

# 2. List configured networks and confirm testnet is present
stellar network ls

# 3. Verify RPC endpoint connectivity
curl -s -X POST https://soroban-testnet.stellar.org:443 \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":1,"method":"getHealth"}'
# Expected response: {"jsonrpc":"2.0","id":1,"result":{"status":"healthy"}}

# 4. Check account address and fund on testnet
stellar keys address <ACCOUNT_NAME>
stellar keys fund <ACCOUNT_NAME> --network testnet
```

---

## Common Errors & Solutions

### 1. Unfunded or Missing Account (`Account not found`)

#### Exact Error Output
```
error: account not found: GXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX
```
or
```
HostError: Error(Auth, InvalidAction)
Transaction failed: tx_bad_auth
```

#### Root Cause
The keypair was created locally via `stellar keys generate`, but has never been funded on the testnet ledger. Unfunded accounts do not exist on the Stellar network.

#### Fix
Fund the account using Friendbot:
```bash
# Using CLI
stellar keys fund <ACCOUNT_NAME> --network testnet

# Or directly via Friendbot HTTP API
export ADDR=$(stellar keys address <ACCOUNT_NAME>)
curl -s "https://friendbot.stellar.org?addr=$ADDR"
```

---

### 2. Network Passphrase or RPC Mismatch (`BadSignature`)

#### Exact Error Output
```
error: transaction rejected: BadSignature
```
or
```
error: send transaction failed: transaction simulation failed: HostError: Error(Context, InvalidAction)
```

#### Root Cause
Your transaction was signed with the Public Mainnet passphrase (`Public Global Stellar Network ; September 2015`) or a local standalone passphrase, while submitting to the Testnet RPC.

#### Fix
Ensure the global testnet network configuration matches the SDF Testnet passphrase:
```bash
stellar network add \
  --global testnet \
  --rpc-url https://soroban-testnet.stellar.org:443 \
  --network-passphrase "Test SDF Network ; September 2015"
```
Always append `--network testnet` to your CLI commands.

---

### 3. Contract State or Instance Expired (`TTL Expiry`)

#### Exact Error Output
```
HostError: Error(Storage, Expired)
error: entry expired: contract instance or persistent entry has exceeded its lifetime
```

#### Root Cause
Soroban uses state archival to manage blockchain state growth. Inactive contracts or temporary entries on Testnet expire once their Time-To-Live (TTL) ledger count elapses.

#### Diagnostic Command
```bash
stellar contract inspect \
  --id <CONTRACT_ID> \
  --network testnet
```

#### Fix
Extend the contract instance TTL:
```bash
stellar contract extend \
  --id <CONTRACT_ID> \
  --durability instance \
  --ledgers-to-extend 500000 \
  --source-account creator \
  --network testnet
```

---

### 4. Escrow Token Payout Failure (`HostError: Error(Contract, #5)` or Token Trap)

#### Exact Error Output
```
error: transaction simulation failed: HostError: Error(Contract, #5)
  Caused by: token contract transfer trapped
```
or invocation fails during `approve_completion` / `complete_bounty`.

#### Root Cause
MergeMint contracts do not automatically mint tokens; payouts are made from the **contract's own token balance**. If the contract balance is lower than the bounty reward, the SEP-41 token transfer aborts with `InsufficientBalance` (#5) or upstream token trap.

#### Fix
Query the contract balance and fund it before completing the bounty:
```bash
# Check contract balance
stellar contract invoke \
  --id <TOKEN_ID> \
  --network testnet \
  --source-account creator \
  -- balance \
  --id <CONTRACT_ID>

# Top up the contract balance
stellar contract invoke \
  --id <TOKEN_ID> \
  --network testnet \
  --source-account creator \
  -- transfer \
  --from $CREATOR_ADDR \
  --to $CONTRACT_ID \
  --amount 10000000
```

---

### 5. Authorization Mismatch (`Unauthorized` / Code 1)

#### Exact Error Output
```
HostError: Error(Auth, InvalidAction)
ContractError: Unauthorized (#1)
```

#### Root Cause
The `--source-account` specified does not match the address passed to the contract function requiring auth:
* `create_bounty` requires auth from `--creator`
* `claim_bounty` requires auth from `--contributor`
* `approve_completion` requires auth from `--verifier`

#### Fix
Verify your source account corresponds to the authorizer argument:
```bash
# Incorrect: contributor trying to authorize create_bounty
stellar contract invoke --id $CONTRACT_ID --source-account contributor -- create_bounty --creator $CREATOR_ADDR ...

# Correct: creator signs create_bounty
stellar contract invoke --id $CONTRACT_ID --source-account creator -- create_bounty --creator $CREATOR_ADDR ...
```

---

### 6. Complex Parameter Syntax / Quoting Issues

#### Exact Error Output
```
error: invalid value for '--tags <tags>': invalid json: expected string
```
or
```
error: value does not match regex pattern for Symbol
```

#### Root Cause
Bash shell expansion stripping quotes around JSON arrays or strings with special characters.

#### Syntax Reference Table
| Soroban Type | CLI Syntax Example | Notes |
|---|---|---|
| `Symbol` | `--title "DocsTutorial"` | Alphanumeric, max 32 chars |
| `String` | `--description "Detailed explanation here"` | Full text string |
| `Vec<Symbol>` | `--tags '["docs", "bug"]'` | Strict JSON array |
| `Option<Vec<Address>>` | `--required_verifiers '["GABC...", "GDEF..."]'` | Omit flag for `None` |
| `Option<u32>` | `--deadline 50000000` | **Omit flag entirely for `None`**; passing `0` sets `Some(0)` |
| `Vec<Milestone>` | `--milestones '[]'` | JSON array of milestone objects |

---

### 7. Contract Compilation Target Mismatch (`wasm32v1-none`)

#### Exact Error Output
```
error[E0463]: can't find crate for `core`
  | target wasm32v1-none not found
```

#### Fix
Install the pinned WASM target specified in `rust-toolchain.toml`:
```bash
rustup target add wasm32v1-none
# or fallback
rustup target add wasm32-unknown-unknown

cargo build --release --target wasm32v1-none
```

---

## Related Documentation

* [Getting Started on Testnet](getting-started.md)
* [Contract Function Reference](contract-reference.md)
* [Security Model](security.md)
