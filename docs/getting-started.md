# Getting Started with MergeMint Contracts

## Prerequisites

- Rust (stable): `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`
- Stellar CLI: `cargo install stellar-cli`
- WASM target: `rustup target add wasm32-unknown-unknown`

## Setup

1. **Generate testnet account:**
   ```bash
   stellar keys generate testnet-account
   ```

2. **Fund with Friendbot:**
   ```bash
   stellar account fund testnet-account --network testnet
   ```

3. **Build contract:**
   ```bash
   cargo build --release --target wasm32-unknown-unknown
   ```

## Deploy to Testnet

```bash
stellar contract deploy \
  --wasm target/wasm32-unknown-unknown/release/mergemint_contracts.wasm \
  --network testnet \
  --source-account testnet-account
```

Save the contract ID from output.

## Test Contract

> **Note:** Soroban contract tests run against the compiled WASM. Build the contract before running tests so the test harness picks up the latest binary.

```bash
cargo build --release --target wasm32-unknown-unknown
cargo test
```

## Deploy Example Bounty

```bash
stellar contract invoke \
  --id <CONTRACT_ID> \
  --network testnet \
  --source-account testnet-account \
  -- create_bounty \
  --creator testnet-account \
  --title "Fix_bug" \
  --description "Fix_auth_issue" \
  --reward_amount 1000000 \
  --reward_token <USDC_ADDRESS> \
  --min_reputation 0
```

Replace `<CONTRACT_ID>` with your deployed contract ID and `<USDC_ADDRESS>` with actual token address.

---

## Sub-projects

This repository contains more than just the Soroban contract. Two additional sub-projects live alongside it, each with their own dependencies and setup instructions:

- **`mergemint-frontend/`** — React/TypeScript frontend. See `mergemint-frontend/package.json` for scripts (`npm install && npm run dev`).
- **`mergemint-backend/`** — Rust/Axum backend service. See `mergemint-backend/Cargo.toml` and its own `README` or `CHANGELOG.md` for setup.

---

## Full Stack (Docker)

A `docker-compose.yml` at the repo root orchestrates the contract node, backend, and frontend together. To bring up the full stack locally:

```bash
docker compose up --build
```

This is the recommended approach for integration testing and local end-to-end development. Refer to `docker-compose.yml` for service names, ports, and environment variable overrides.

A `Makefile` is also provided with convenience targets — run `make help` (or just `make`) to see available commands.
