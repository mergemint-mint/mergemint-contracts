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

## Bounty archiving storage footprint

Approximate per-bounty persistent-storage footprint before and after `archive_bounty` compacts a finished bounty into a `BountySummary`. Based on the field layout of `Bounty`/`BountyMeta` (title, description, tags, milestones, verifier list, assignees) versus `BountySummary` (id, creator, assignee, status, reward).

| Record | Fields retained | Approx. XDR size |
|--------|------------------|------------------:|
| `Bounty` + `BountyMeta` (pre-archive) | creator, reward_amount, reward_token, assignees, max_assignees, status, min_reputation, deadline, required_verifiers, approval_threshold, tags, milestones, title, description | ~300–900+ bytes (variable: scales with tags/milestones/verifiers/description length) |
| `BountySummary` (post-archive) | id, creator, assignee, status, reward | ~90–110 bytes (fixed-ish; one optional `Address`) |

Archiving a finished bounty also removes its `Bounty(id)` and `BountyMeta(id)` persistent entries entirely, so those stop being extended by `extend_ttl` on every read — the dominant long-run rent cost for bounties nobody touches again. The exact bytes-saved figure depends on the bounty's tag/milestone/verifier/description sizes; this table gives the field-level shape rather than a single measured number (no `cargo` run was performed to produce an exact benchmark).

---

## SSE stream load (Issue #1120)

Load baseline for `GET /bounties/stream`, measured with
[`scripts/k6/sse.js`](../scripts/k6/sse.js). The scenario ramps up long-lived
SSE subscribers, publishes `bounty_updated` events at a fixed rate, and
records how long each event takes to reach each client. Setup and metric
definitions are in [`scripts/k6/README.md`](../scripts/k6/README.md#sse-scenarios-ssejs-sse-limitjs).

**Environment:** 2026-10-20, branch `security/webhook-ssrf-sse-limits-gitleaks-k6`.
Release build of `mergemint-backend` run directly (`cargo run --release`), k6 v2.3.0 + xk6-sse v0.2.0. Backend and
k6 shared one 2-vCPU / 8 GB dev container, so the latency figures include k6's
own CPU contention. Connection caps were turned off
(`SSE_MAX_CONNECTIONS_PER_IP=0 SSE_MAX_CONNECTIONS=0`) because every client
came from one IP. Keep-alive interval 15 s (default), `TRIGGER_RPS=2`,
`SSE_HOLD_SECONDS=30`, `RAMP=30s`.

| Clients | Steady state | Events triggered | Events delivered | Dropped connections | Connect p95 | Latency median | Latency p95 | Latency p99 | Latency max |
| ------: | -----------: | ---------------: | ---------------: | ------------------: | ----------: | -------------: | ----------: | ----------: | ----------: |
| 200     | 2 min        | 240              | 47,999 / 48,000  | 0 (988 streams)     | 2 ms        | 4 ms           | 7 ms        | 15 ms       | 29 ms       |
| 1,000   | 1 min        | 121              | 121,000 / 121,000 | 0 (2,987 streams)  | 24 ms       | 21 ms          | 34 ms       | 55 ms       | 155 ms      |

Latency is measured from just before the trigger request is sent, so it
includes the trigger's HTTP round trip (4 to 7 ms on average here) as well as the
broadcast fan-out. In the 200-client run one event was missed by a client
that was reconnecting between hold windows. That is expected: the stream does
not replay missed events.

The k6 thresholds (`sse_event_latency` p95 < 250 ms, p99 < 500 ms) leave a
wide margin over these numbers. Tighten them once there is a baseline from
production-like hardware with k6 on a separate machine.

Reproduce:

```bash
scripts/k6/build-k6-sse.sh
(cd mergemint-backend && SSE_MAX_CONNECTIONS_PER_IP=0 SSE_MAX_CONNECTIONS=0 cargo run --release)
API_PREFIX= scripts/k6/run.sh sse
API_PREFIX= SSE_CLIENTS=1000 DURATION=1m scripts/k6/run.sh sse
```

### Connection caps (Issue #1118)

`scripts/k6/sse-limit.js` against the same build with default caps (32 per IP,
1,024 total): 40 streams opened at once from one IP gave **32 accepted and 8
refused with `429` + `Retry-After`**, and nothing else. `/metrics` showed
`mergemint_sse_active_connections 32` during the hold and `0` afterwards, with
`mergemint_sse_rejected_connections_total{reason="per_ip"} 8`.
