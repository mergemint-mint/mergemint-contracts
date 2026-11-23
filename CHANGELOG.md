# Changelog

All notable changes to MergeMint Contracts are documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
Versions follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [Unreleased]

Changes that have landed on `main` but are not yet associated with a tagged release.

### Changed

- `resolve_dispute` now validates its `resolution` argument (`"complete"` / `"cancel"`) through the shared `symbols::validate_symbol` allow-list, panicking with `resolution must be 'complete' or 'cancel'` on an unrecognised value (#651).
- `BountySummary` now carries `reward_token: Address`. `get_bounty` returns the bounty's true reward token for archived bounties instead of substituting `env.current_contract_address()` as a placeholder, and `archive_bounty` populates it from the live `Bounty` record. This is a schema change to the archived summary payload: off-chain consumers that assumed the placeholder contract address must read the real token address, and any cached summaries serialized before this change should be re-fetched (#1220).
- `update_contributor_metadata` now emits the `contributor_metadata_updated` event on every successful update, with the payload aligned to the stored `Contributor.metadata` type (`BytesN<32>`) and documented in `docs/event-schema.md` (#1221).
- `src/events.rs` migrated from the deprecated `env.events().publish(...)` API to Soroban SDK 29's typed `#[contractevent]` macros, preserving wire-compatible topics and data. The blanket `#![allow(dead_code, deprecated, unused_imports)]` in `src/lib.rs` was removed and the resulting dead-code warnings cleaned up (#1222).

### Added

- `migrate_legacy_indexes`: admin-only, bounded and idempotent migration entrypoint that drains the legacy `DataKey::StatusIndex(Symbol)` and `DataKey::OpenBounties` storage blobs into `StatusIndexPage` / `OpenBountiesPage` shards, deduplicating entries and deleting the legacy keys once fully migrated so the work fits within a single transaction's CPU/write budget (#1219).

- `sdk`: typed `reopenBounty`, `reassignBounty` and `submitWork` methods wrapping the new lifecycle entrypoints, with `ReopenBountyParams` / `ReassignBountyParams` / `SubmitWorkParams` types, `ReopenNotAllowedError` / `SubmissionNotFoundError` typed errors, and recipe examples `11-reopen-bounty.ts`, `12-reassign-bounty.ts`, `13-submit-work.ts` (#1085).
- `BountyRefresh`: failed refresh tasks are now retried up to `MAX_TASK_RETRIES` (3) times, each retry emitting `TaskRetried(batchId, taskId, attempt)`. `_executeRefresh` is now restricted to self-calls (`onlySelf`); previously its `onlyOwner` guard made every self-invoked task fail.
- `sdk`: optional `retry: { attempts, backoffMs }` constructor option that retries
  every Soroban RPC round-trip with exponential backoff (#665).
- `sdk`: JSDoc on every public `MergeMintSDK` method, documenting parameters,
  return values, and thrown errors (#663).
- CI: `SDK CI` workflow gating `sdk/` on `tsc --noEmit` and the jest suite (#667).
- CI: `SDK CI` job requiring a `CHANGELOG.md` entry whenever `sdk/package.json`
  changes version (#664).

### Fixed

- `sdk`: removed an orphaned `CreateBountyParams` fragment left in
  `sdk/src/index.ts` by an earlier merge, which made the module unparseable.

---

## [0.1.0] — 2024-01-01

### Added

- `create_bounty` — creates a new bounty with reward token, amount, and optional deadline.
- `claim_bounty` — allows a contributor to claim an open bounty (enforces min reputation and deadline).
- `complete_bounty` — distributes reward to assignees proportionally by basis-point share.
- `cancel_bounty` — cancels an open or in-progress bounty and refunds the creator.
- `raise_dispute` — transitions a bounty into the `disputed` state.
- `update_contributor_metadata` — lets a contributor update their off-chain profile URI.
- `get_bounty` / `get_contributor` — read-only accessors for off-chain consumers.
- `Bounty` struct with fields: `creator`, `reward_amount`, `reward_token`, `assignees`, `max_assignees`, `status`, `min_reputation`, `deadline`.
- `Contributor` struct with fields: `address`, `reputation`, `total_earned`, `contribution_count`, `active_claims`, `metadata`.
- `DataKey` enum covering `BountyCount`, `Bounty`, `BountyMeta`, `Contributor`, `StatusIndex`, `OpenBounties`.
- Status index tracking bounty lifecycle transitions via `StatusIndex(Symbol)`.
- Open bounties index via `OpenBounties` persistent storage key.
- Multi-assignee support with configurable `max_assignees` and basis-point share splitting.

---

[Unreleased]: https://github.com/mergemint-mint/mergemint-contracts/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/mergemint-mint/mergemint-contracts/releases/tag/v0.1.0

## [Unreleased]
- Reconcile duplicate backend directories (#668): the orphaned TypeScript `backend/` (no Dockerfile, unreferenced by the build) has been removed; `docker-compose.yml` now builds the canonical Rust `mergemint-backend/`, and the doc reference in `docs/shared-type-generation.md` was updated.
