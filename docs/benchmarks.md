# Benchmarks

Performance notes for the MergeMint contract. Instruction counts are measured in two ways:

1. **Unit tests** — `env.budget().cpu_instruction_count()` in `src/test.rs` (see `benchmark_*` tests) measures CPU instructions consumed in the Soroban simulator during `cargo test`.
2. **On-chain simulation** — `simulateTransaction` RPC call returns `cost.cpuInsns` for real network measurements.

---

## CPU Instruction Baselines (Issue #289)

Baseline measurements captured by the `benchmark_*` tests in `src/test.rs` using `env.budget().cpu_instruction_count()`. Run `cargo test benchmark` to reproduce.

| Function | CPU Instructions | Soft Limit |
|----------|-----------------|------------|
| `create_bounty` | — | 1,000,000 |
| `claim_bounty` | — | 1,000,000 |
| `complete_bounty` | — | 1,000,000 |
| `get_bounty` | — | 500,000 |
| `get_contributor` | — | 500,000 |
| `get_bounty_count` | — | 500,000 |

> Values are populated by running `cargo test benchmark -- --nocapture` and reading the printed output.

---

## `complete_bounty` — storage read/write restructuring

**Branch:** `perf/complete-bounty-batch-writes`  
**Commit:** `perf: restructure complete_bounty to batch storage reads and writes`

### Change

Before, `complete_bounty` interleaved storage reads and writes with the token transfer:

```
read  bounty
            transfer tokens   (external call)
read  contributor
write contributor
```

After, all reads happen before the external call and all writes happen after:

```
read  bounty
read  contributor
            transfer tokens   (external call)
write contributor
```

The number of storage operations is unchanged (2 reads, 1 write). The improvement comes from access pattern locality: both ledger entries are fetched before the host executes the cross-contract token transfer, so the host can load them in the same scheduling window rather than suspending between the external call and the second read. This also eliminates the window between the external call and the second storage read where a reentrant call could observe stale contributor state.

### Instruction counts

Instruction counts are not yet captured here. To measure:

```bash
# Build
cargo build --release --target wasm32-unknown-unknown

# Deploy to testnet, then invoke complete_bounty via Stellar CLI
# and inspect the simulateTransaction response:
stellar contract invoke \
  --id <CONTRACT_ID> \
  --network testnet \
  --source-account <ACCOUNT> \
  -- complete_bounty \
  --verifier <VERIFIER> \
  --bounty_id <BOUNTY_ID>
```

The `simulateTransaction` RPC response includes:

```json
{
  "cost": {
    "cpuInsns": "<before>",
    "memBytes": "<before>"
  }
}
```

Update this table once measurements are taken against both the old and new WASM:

| Version | `cpuInsns` | `memBytes` |
|---------|-----------|------------|
| Before  | —         | —          |
| After   | —         | —          |
| Delta   | —         | —          |

---

## mergemint-backend HTTP load baseline

Baseline concurrent-request latency and error rate for `mergemint-backend`'s HTTP routes, captured with `scripts/load_test.sh`. Complements `scripts/smoke_test.sh` (single happy-path request) by exercising the server under concurrency.

**Environment:** `cargo run --bin mergemint-backend` (debug build, in-memory store, no seeded records — every request resolves to a 404, so these numbers reflect HTTP/middleware overhead rather than business logic).

**Command:** `REQUESTS=200 CONCURRENCY=20 ./scripts/load_test.sh`

| Route | Requests | Errors | Error rate | min (ms) | avg (ms) | p95 (ms) | max (ms) |
|-------|---------:|-------:|-----------:|---------:|---------:|---------:|---------:|
| `POST /tx/self-claim` | 200 | 0 | 0.00% | 1.09 | 26.28 | 93.34 | 260.28 |
| `POST /tx/resolve-dispute` | 200 | 0 | 0.00% | 1.56 | 36.90 | 170.46 | 261.56 |

> Reproduce with `cargo build --bin mergemint-backend && ./target/debug/mergemint-backend &` then run the command above. See `scripts/load_test.sh` for `HOST`/`CONCURRENCY`/`REQUESTS` overrides.
