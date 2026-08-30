# Benchmarks

Performance notes for the MergeMint contract. Instruction counts are measured in two ways:

1. **Unit tests** — `env.cost_estimate().budget().cpu_instruction_cost()` in `src/test.rs` (see `benchmark_*` tests) measures CPU instructions consumed in the Soroban simulator during `cargo test`. Each benchmark resets the tracker with `reset_tracker()` before the measured call.
2. **On-chain simulation** — `simulateTransaction` RPC call returns `cost.cpuInsns` for real network measurements.

---

## CPU Instruction Baselines (Issue #289)

Baseline measurements captured by the `benchmark_*` tests in `src/test.rs` using `env.cost_estimate().budget().cpu_instruction_cost()`. Run `cargo test benchmark -- --nocapture` to reproduce.

| Function | CPU Instructions | Soft Limit |
|----------|-----------------|------------|
| `create_bounty` | 533,419 | 1,000,000 |
| `claim_bounty` | 619,507 | 1,000,000 |
| `complete_bounty` | 782,959 | 1,000,000 |
| `get_bounty` | 90,368 | 500,000 |
| `get_contributor` | 68,465 | 500,000 |
| `get_bounty_count` | 42,974 | 500,000 |

> Measured 2026-08-28 on Soroban SDK 27.0.0 / `mergemint-contracts` main. Native Rust test harness counts are typically lower than on-chain WASM; use these as regression baselines, not production fee quotes.

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
