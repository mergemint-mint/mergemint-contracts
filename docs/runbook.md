# Deployment Runbook

This runbook describes the hardened deployment flow for the Soroban contract and
its supporting release artifacts. All scripts under `scripts/` run with strict
shell safety (`set -euo pipefail`) and are expected to pass
`shellcheck -S warning`.

## Prerequisites

- Stellar CLI installed and on `PATH` (`stellar --version`).
- `shellcheck` installed for local verification.
- The following environment variables exported before running any deploy step:
  - `STELLAR_NETWORK` — target network (`testnet`, `futurenet`, or `mainnet`).
  - `STELLAR_NETWORK_PASSPHRASE` — must match the selected network; the deploy
    script aborts if the passphrase does not correspond to `STELLAR_NETWORK`.
  - `STELLAR_SOURCE_ACCOUNT` — funded identity used to sign the deploy.
  - `ADMIN_ADDRESS` — admin account initialized atomically after deploy.

## 1. Build the release WASM

```bash
./scripts/build-release-wasm.sh
```

The script builds the optimized contract and writes a `SHA256SUMS` file next to
the artifact. Record the printed WASM hash — it is required for verification in
the next step.

## 2. Verify the WASM checksum

Before deploying, confirm the artifact matches the recorded checksum:

```bash
sha256sum -c target/wasm32-unknown-unknown/release/SHA256SUMS
```

`scripts/deploy.sh` performs this verification automatically and refuses to
continue if the checksum does not match.

## 3. Deploy and initialize the admin

```bash
./scripts/deploy.sh
```

The deploy script:

1. Validates required environment variables and the network passphrase.
2. Verifies the SHA-256 checksum of the release WASM.
3. Runs `stellar contract deploy` and captures the contract ID.
4. Immediately invokes `init_admin` with `ADMIN_ADDRESS` to close the
   uninitialized-admin front-running window.
5. Calls `version()` on the deployed contract and asserts the expected version.
6. Prints the contract ID and WASM hash for the release record.

If any step fails the script exits non-zero and no partial state is left behind.

## 4. Smoke and integration tests

```bash
./scripts/smoke_test.sh
./scripts/integration_test.sh
```

Both harnesses run under `set -euo pipefail` and fail fast on the first error.
Run them against the freshly deployed contract before announcing a release.

## Rollback

If post-deploy validation fails, redeploy the previous known-good WASM using the
same flow. The `init_admin` step is idempotent-safe only for a fresh contract ID;
never reuse a contract ID whose admin was already initialized.
