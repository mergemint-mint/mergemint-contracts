# mergemint-backend

## Backend reliability fixes (fix/backend-reliability-batch)

Four issues covering pool exhaustion, input validation, pagination limits, and CORS were addressed in a single branch, landed as four focused commits plus one prerequisite CI fix. All changes pass `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and `cargo test` (58 tests, 0 failures).

1. **DB connection pool exhaustion test** (`03baf85`) — Added `DbPool`, a capacity-bounded pool over `SharedDb` backed by a `tokio::sync::Semaphore`. `acquire()` waits up to a bounded timeout for a free connection and returns `PoolExhausted` instead of hanging. Covered by tests for saturation-then-bounded-error and permit release on drop.

2. **Malformed assignee validation in `list_bounties_by_assignee`** (`6cabd45`) — `routes/bounties.rs` was dead code: unwired from `routes/mod.rs` and referencing an abandoned DB shape from before `SharedDb` replaced it. Rewired it against the real `AppState`, added `list_bounties_by_creator`/`list_bounties_by_assignee` query helpers, and mounted `GET /bounties` and `GET /bounties/assignee/:address` in `main.rs`. A syntactically invalid address now returns 400; a well-formed but unknown address returns an empty page, not a 500.

3. **Pagination limit enforcement in `list_bounties`** (`145239b`) — `list_bounties` was already clamping `limit` to `MAX_LIST_LIMIT` (100) but the behavior was untested. Added tests asserting an oversized `limit=10000` request is clamped to `MAX_LIST_LIMIT`, and that an in-bounds limit is honored as-is.

4. **CORS allow-list** (`ce442f5`) — `main.rs` had no explicit CORS layer at all. Added a `CorsLayer` driven by `CORS_ALLOWED_ORIGINS`, a comma-separated allow-list read from configuration. An empty/unset value logs a warning and permits no cross-origin access, mirroring the existing `warn_if_reward_token_allowlist_empty` pattern. Covered by tests for an allowed origin, a disallowed origin, and an empty allow-list.

Also included as a prerequisite:

- **Clippy lint fix** (`08188e5`) — `cargo clippy --all-targets -- -D warnings` was failing on `main` under a newer clippy version (`clippy::cloned_ref_to_slice_refs` on `&[event_name.clone()]` in `indexer.rs`). Swapped in `std::slice::from_ref` to unblock the CI gate required by every backend PR.
