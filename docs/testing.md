# Testing

This project has two shell scripts under `scripts/` in addition to `cargo test`:
`scripts/integration_test.sh` and `scripts/smoke_test.sh`. Both drive the compiled
contract through its actual CLI surface (`stellar contract invoke`) rather than
calling Rust functions directly, so they exercise the same path a real integrator
would use.

## `scripts/integration_test.sh`

**What it covers:** a full local end-to-end pass — starts a local Soroban sandbox,
builds the WASM, deploys a mock reward token and the MergeMint contract, then walks
through `create_bounty` → `claim_bounty` → `complete_bounty` → `update_contributor_metadata`,
asserting bounty count, assignee membership, contributor reputation/contribution
count, and final bounty status at each step.

**When to run it:** locally, before opening a PR that touches contract logic. It's
the fastest way to catch a regression in the bounty lifecycle without needing a
funded testnet account.

**Prerequisites:**
- `stellar-cli` installed (`cargo install stellar-cli --locked`)
- Docker installed and running (the script starts a local sandbox container)
- The `wasm32v1-none` Rust target installed (`rustup target add wasm32v1-none`) —
  see the note below on target mismatches

**Usage:**
```bash
./scripts/integration_test.sh
```
No environment variables are required; it manages its own local identities and
network.

## `scripts/smoke_test.sh`

**What it covers:** a minimal build → deploy → create → claim → complete pass
against a real network (testnet by default), intended as a quick post-deploy sanity
check rather than a full regression suite.

**When to run it:** after deploying to testnet (or another live network), to
confirm the deployed contract responds to the basic bounty lifecycle.

**Prerequisites:**
- `stellar-cli` installed
- A funded account on the target network (see the Getting Started guide for
  funding a testnet account via Friendbot)

**Required environment variables:**
| Variable  | Required | Default    | Description                                  |
|-----------|----------|------------|-----------------------------------------------|
| `ACCOUNT` | Yes      | —          | Funded account public key used as source, creator, and claimant |
| `NETWORK` | No       | `testnet`  | Network alias passed to `stellar` commands    |

**Usage:**
```bash
ACCOUNT=GXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX ./scripts/smoke_test.sh
```

> **Known drift:** `smoke_test.sh`'s `create_bounty` call currently uses `--reward`
> and `--deadline` flags, and its `claim_bounty`/`complete_bounty` calls use
> `--claimant`/`--creator`. The contract's actual current entrypoints
> (`src/contract/mutations.rs`) expect `--reward_amount`/`--reward_token`/
> `--min_reputation` for `create_bounty`, and `--contributor`/`--bounty_id` and
> `--verifier`/`--bounty_id` for `claim_bounty`/`complete_bounty` respectively —
> matching `integration_test.sh`, which is up to date. Until `smoke_test.sh` is
> fixed to match, expect it to fail with an unrecognized-argument error; use
> `integration_test.sh` as the reference for the current CLI shape in the
> meantime.

## Build target note

Both scripts (and `docs/getting-started.md`) currently look for the WASM binary
under `target/wasm32-unknown-unknown/release/`. CI (`.github/workflows/build.yml`,
`.github/workflows/interface-check.yml`) and `rust-toolchain.toml` both build with
`wasm32v1-none` instead, which places the binary under
`target/wasm32v1-none/release/`. If a script reports a missing WASM file, check
which target actually produced your build.
