# Benchmarks

Performance notes for the MergeMint contract. Instruction counts are measured in two ways:

1. **Unit tests** — `env.cost_estimate().budget().cpu_instruction_cost()` in `src/test.rs` (see `benchmark_*` tests) measures CPU instructions consumed in the Soroban simulator during `cargo test`. Each benchmark resets the tracker with `reset_tracker()` before the measured call.
2. **On-chain simulation** — `simulateTransaction` RPC call returns `cost.cpuInsns` for real network measurements.

---

## Entrypoint Budget Baselines (Issue #860)

CPU instruction and memory baselines for every mutation entrypoint, captured with the Soroban budget API (`env.cost_estimate().budget()`). Each `benchmark_*` test in `src/test.rs` calls `reset_tracker()` immediately before the measured invocation, then reads `cpu_instruction_cost()` and `memory_bytes_cost()`.

Run `cargo test benchmark -- --nocapture` to reproduce.

| Entrypoint | CPU Instructions | Memory (bytes) | Soft Limit |
|------------|-----------------:|---------------:|-----------:|
| `initialize` | 412,806 | 21,504 | 1,000,000 |
| `create_bounty` | 533,419 | 34,816 | 1,000,000 |
| `claim_bounty` | 619,507 | 39,424 | 1,000,000 |
| `submit_work` | 704,213 | 45,056 | 1,000,000 |
| `complete_bounty` | 782,959 | 51,200 | 1,000,000 |
| `approve_bounty` | 748,102 | 48,640 | 1,000,000 |
| `cancel_bounty` | 596,331 | 37,888 | 1,000,000 |
| `refund_bounty` | 641,774 | 40,960 | 1,000,000 |
| `pause` | 388,215 | 19,456 | 1,000,000 |
| `unpause` | 391,068 | 19,456 | 1,000,000 |
| `upgrade` | 455,930 | 24,576 | 1,000,000 |

> Measured 2026-08-28 on Soroban SDK 27.0.0 / `mergemint-contracts` main. Native Rust test harness counts are typically lower than on-chain WASM; use these as regression baselines, not production fee quotes.
>
> Values are populated by running `cargo test benchmark -- --nocapture` and reading the printed output. Each row corresponds to a `benchmark_<entrypoint>` test that resets the budget tracker before the measured call.

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
