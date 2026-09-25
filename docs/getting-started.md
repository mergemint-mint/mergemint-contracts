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

`docker-compose.yml` at the repo root runs the whole local stack with one command. The only prerequisite is Docker with Compose v2 (`docker compose version`).

```bash
git clone https://github.com/mergemint-mint/mergemint-contracts.git
cd mergemint-contracts
docker compose up --build --wait
```

`--wait` returns once every service reports healthy (the first run takes a few minutes while the Rust backend compiles and the Stellar network boots). Drop `--wait` to stay attached to the logs instead.

| Service    | What it runs                                                      | URL on your machine                 | Healthcheck                              |
|------------|-------------------------------------------------------------------|-------------------------------------|------------------------------------------|
| `stellar`  | `stellar/quickstart --local`: standalone network with core, Horizon, Soroban RPC and Friendbot | http://localhost:8000 (RPC at `/rpc`, Friendbot at `/friendbot`) | RPC `getHealth` returns `healthy`        |
| `postgres` | Postgres 16                                                       | `localhost:5432` (user/password/db: `mergemint`) | `pg_isready`                   |
| `backend`  | `mergemint-backend` (distroless image, non-root)                  | http://localhost:8080 (`/health`)   | `mergemint-backend healthcheck` → `/health` |
| `frontend` | Vite dev server for `frontend/`                                   | http://localhost:5173               | HTTP GET `/`                             |

Services start in dependency order: `backend` waits for `postgres` and `stellar` to be healthy, and `frontend` waits for `backend`.

Useful commands:

```bash
docker compose ps                  # status and health of each service
docker compose logs -f backend     # follow one service's logs
docker compose down                # stop the stack
docker compose down -v             # stop and wipe the Postgres volume
```

### Deploying the contract to the local network

The local network uses the passphrase `Standalone Network ; February 2017`. Register it with the Stellar CLI, fund an account through the local Friendbot, then deploy:

```bash
stellar network add local \
  --rpc-url http://localhost:8000/rpc \
  --network-passphrase "Standalone Network ; February 2017"
stellar keys generate local-account --network local --fund
cargo build --release --target wasm32v1-none
stellar contract deploy \
  --wasm target/wasm32v1-none/release/mergemint_contracts.wasm \
  --network local \
  --source-account local-account
```

### Overrides

Every host port and credential can be overridden through environment variables or a `.env` file next to `docker-compose.yml`, for example if a port is already in use:

| Variable | Default | Purpose |
|---|---|---|
| `STELLAR_PORT` / `POSTGRES_PORT` / `BACKEND_PORT` / `FRONTEND_PORT` | `8000` / `5432` / `8080` / `5173` | Host ports |
| `POSTGRES_USER` / `POSTGRES_PASSWORD` / `POSTGRES_DB` | `mergemint` | Database credentials (also used to build the backend's `DATABASE_URL`) |
| `CORS_ALLOWED_ORIGINS` | `http://localhost:5173` | Origins allowed to call the backend |
| `ALLOWLISTED_REWARD_TOKENS` | _(empty)_ | Reward tokens accepted by the backend |
| `VITE_API_BASE_URL` | `http://localhost:8080` | Backend URL used by the browser |
| `STELLAR_QUICKSTART_TAG` | `latest` | `stellar/quickstart` image tag |

If you change `BACKEND_PORT` or `FRONTEND_PORT`, update `VITE_API_BASE_URL` or `CORS_ALLOWED_ORIGINS` to match.

A `Makefile` is also provided with convenience targets — run `make help` (or just `make`) to see available commands.
