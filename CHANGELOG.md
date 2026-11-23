# Changelog

All notable changes to MergeMint Contracts are documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
Versions follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [1.1.0](https://github.com/mergemint-mint/mergemint-contracts/compare/contract-v1.0.0...contract-v1.1.0) (2026-11-23)


### Added

* add glossary, mdBook docs, CSP, and security headers ([4b11961](https://github.com/mergemint-mint/mergemint-contracts/commit/4b11961a971dac7dfbb403509f39feb169cbe2e7))
* **backend:** add indexer reliability features ([f3f158b](https://github.com/mergemint-mint/mergemint-contracts/commit/f3f158b23ba68690c93ad8852040ca4ce513bd12))
* **backend:** resolve transaction and webhook issues ([3f84bbc](https://github.com/mergemint-mint/mergemint-contracts/commit/3f84bbcdff5273f2a2aaf9b0daa3c1176774288c))
* **bounty:** add gas-bounded chunked execution to BountyRefresh ([#1250](https://github.com/mergemint-mint/mergemint-contracts/issues/1250)) ([8582c21](https://github.com/mergemint-mint/mergemint-contracts/commit/8582c217056aa73f5fc799ddc0f32ca8bdc19d52))
* **bounty:** use Ownable2Step and pause-guard createBatch/finalizeBatch ([#1253](https://github.com/mergemint-mint/mergemint-contracts/issues/1253)) ([f5489a7](https://github.com/mergemint-mint/mergemint-contracts/commit/f5489a7783ccdeb92914d00d34d0c7875c1c069a))
* **bounty:** wire _executeRefresh to IBountyManager.updateContributorMetrics ([#1248](https://github.com/mergemint-mint/mergemint-contracts/issues/1248)) ([a3f744a](https://github.com/mergemint-mint/mergemint-contracts/commit/a3f744adf7ebc8a24d048c2909bc96bb2083cce5))
* **contract:** validate milestones on bounty creation ([953c449](https://github.com/mergemint-mint/mergemint-contracts/commit/953c44980744438fbfdbc7775e14aea1875e3a18))
* **contract:** validate milestones on bounty creation ([bf9d1b5](https://github.com/mergemint-mint/mergemint-contracts/commit/bf9d1b53a10a38ea0180f7cb5cc06ada86cb2c2d))
* **frontend:** add content security policy ([99e7140](https://github.com/mergemint-mint/mergemint-contracts/commit/99e7140a8146321b1200ee2dd9e044ef723d2707)), closes [#1114](https://github.com/mergemint-mint/mergemint-contracts/issues/1114)
* **frontend:** harden service worker, api timeouts/idempotency, csp, ci pins, and status a11y ([a347efb](https://github.com/mergemint-mint/mergemint-contracts/commit/a347efbfe2541eb7dfb88072a46151c0ebbf5928))
* **frontend:** harden service worker, api timeouts/idempotency, csp, ci pins, and status a11y ([7d20413](https://github.com/mergemint-mint/mergemint-contracts/commit/7d204136380907fb7c91cd8f3a49ada08e600e6e))


### Fixed

* [#1266](https://github.com/mergemint-mint/mergemint-contracts/issues/1266) Add jittered exponential backoff, circuit breaking, and dead- ([54f12e6](https://github.com/mergemint-mint/mergemint-contracts/commit/54f12e628890de032892c8d2b9e166e6eea8e1c6))
* [#1267](https://github.com/mergemint-mint/mergemint-contracts/issues/1267) Enforce webhook secret entropy, HTTPS-only production URLs, a ([64ad72f](https://github.com/mergemint-mint/mergemint-contracts/commit/64ad72f145cb0a1761cdbfd3098b61f4350db9f2))
* [#1268](https://github.com/mergemint-mint/mergemint-contracts/issues/1268) Harden `webhooks/ssrf.rs` with destination-port allowlisting, ([f3a45cc](https://github.com/mergemint-mint/mergemint-contracts/commit/f3a45ccedf04445d5e77377530173273ca324e69))
* [#1269](https://github.com/mergemint-mint/mergemint-contracts/issues/1269) Add authentication and authorization middleware to webhook ma ([6c7fc09](https://github.com/mergemint-mint/mergemint-contracts/commit/6c7fc09cb57ebee38a458ed6a40882f16cde0a0c))
* [#1274](https://github.com/mergemint-mint/mergemint-contracts/issues/1274) Reconcile `description` length validation parity between `mer ([e1e743d](https://github.com/mergemint-mint/mergemint-contracts/commit/e1e743d4929eb1e524cb741b900370730f08dea9))
* [#1284](https://github.com/mergemint-mint/mergemint-contracts/issues/1284) Fix `useBounty` infinite resubscription and unwired `hash` st ([43a81eb](https://github.com/mergemint-mint/mergemint-contracts/commit/43a81eb6f03750587570152c2a059bd3118124d0))
* [#1285](https://github.com/mergemint-mint/mergemint-contracts/issues/1285) Align `react/src/index.ts` `useBounties` pagination with curs ([6a5526d](https://github.com/mergemint-mint/mergemint-contracts/commit/6a5526dd928bf80dae3ab5ed2b2c9717161e41ea))
* [#1286](https://github.com/mergemint-mint/mergemint-contracts/issues/1286) Harden `cli/src/cli.ts` with real transaction signing, strict ([2ac7f6a](https://github.com/mergemint-mint/mergemint-contracts/commit/2ac7f6aff0f210501d15a5d5e8771177c116cccc))
* [#1294](https://github.com/mergemint-mint/mergemint-contracts/issues/1294) Consolidate or deprecate duplicate `mergemint-frontend/` pack ([f4e5876](https://github.com/mergemint-mint/mergemint-contracts/commit/f4e5876186ac54e470aeeef0f78b3c74534022c4))
* [#1296](https://github.com/mergemint-mint/mergemint-contracts/issues/1296) Harden `scripts/deploy.sh`, `scripts/build-release-wasm.sh`, ([54bdbbb](https://github.com/mergemint-mint/mergemint-contracts/commit/54bdbbbbd9712237e466d84fda113778a961a860))
* address [#1294](https://github.com/mergemint-mint/mergemint-contracts/issues/1294), [#1295](https://github.com/mergemint-mint/mergemint-contracts/issues/1295), [#1296](https://github.com/mergemint-mint/mergemint-contracts/issues/1296), [#1297](https://github.com/mergemint-mint/mergemint-contracts/issues/1297) ([c2dc292](https://github.com/mergemint-mint/mergemint-contracts/commit/c2dc29273640de7e4b3fb42aeb14826e64af5cef))
* **bounty:** bound getTask and paginate getContributorTasks ([#1252](https://github.com/mergemint-mint/mergemint-contracts/issues/1252)) ([d8bfb4a](https://github.com/mergemint-mint/mergemint-contracts/commit/d8bfb4ac68bd15fb74ae27a3de9a8ca38fbc0b70))
* **bounty:** harden BountyRefresh ([#1250](https://github.com/mergemint-mint/mergemint-contracts/issues/1250), [#1251](https://github.com/mergemint-mint/mergemint-contracts/issues/1251), [#1252](https://github.com/mergemint-mint/mergemint-contracts/issues/1252), [#1253](https://github.com/mergemint-mint/mergemint-contracts/issues/1253)) ([c55dd6b](https://github.com/mergemint-mint/mergemint-contracts/commit/c55dd6b5f98d952178206fd9f23525101ed88319))
* **bounty:** isolate custom-error and panic reverts in BountyRefresh._processRefreshTask ([#1251](https://github.com/mergemint-mint/mergemint-contracts/issues/1251)) ([1162724](https://github.com/mergemint-mint/mergemint-contracts/commit/1162724fbdaaba3ee10575c6d844bd62f97f9503))
* **bounty:** pin solidity pragma and drop unused index param ([#1254](https://github.com/mergemint-mint/mergemint-contracts/issues/1254)) ([0aabc7b](https://github.com/mergemint-mint/mergemint-contracts/commit/0aabc7bf7e420daa52543586a7846726c3363933))
* **bounty:** validate contributors and bountyIds in createBatch ([#1249](https://github.com/mergemint-mint/mergemint-contracts/issues/1249)) ([aa998a4](https://github.com/mergemint-mint/mergemint-contracts/commit/aa998a410c73e255f725df9a4f89d32d100f19ca))
* **contract:** eliminate integer overflow and rounding dust loss in distribute_payout ([b87306f](https://github.com/mergemint-mint/mergemint-contracts/commit/b87306faaf4ce5c3cc097e68dbec5c465c8d9744))
* **contract:** eliminate integer overflow and rounding dust loss in distribute_payout ([6d6d697](https://github.com/mergemint-mint/mergemint-contracts/commit/6d6d697b340faa50612c447fcfa7b9db4b627d84))
* **contract:** remove duplicate code reintroduced by [#1153](https://github.com/mergemint-mint/mergemint-contracts/issues/1153) merge ([2502239](https://github.com/mergemint-mint/mergemint-contracts/commit/250223975a31e8a73f478c88cccde40256245e69))
* refresh validation, manager wiring, ownership race tests, WASM gate ([#1246](https://github.com/mergemint-mint/mergemint-contracts/issues/1246) [#1247](https://github.com/mergemint-mint/mergemint-contracts/issues/1247) [#1248](https://github.com/mergemint-mint/mergemint-contracts/issues/1248) [#1249](https://github.com/mergemint-mint/mergemint-contracts/issues/1249)) ([3363cdd](https://github.com/mergemint-mint/mergemint-contracts/commit/3363cdd5e6795541a42bb34fe8411437a288f5e9))
* **scripts:** harden batchRefresh retries, input validation and gas ([#1256](https://github.com/mergemint-mint/mergemint-contracts/issues/1256)) ([1de6bb6](https://github.com/mergemint-mint/mergemint-contracts/commit/1de6bb6ae8352a1b3d286639aa828603d82b9d0e))

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
