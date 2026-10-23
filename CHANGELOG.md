# Changelog

All notable changes to MergeMint Contracts are documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
Versions follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [1.1.0](https://github.com/mergemint-mint/mergemint-contracts/compare/contract-v1.0.0...contract-v1.1.0) (2026-10-23)


### Added

* add glossary, mdBook docs, CSP, and security headers ([4b11961](https://github.com/mergemint-mint/mergemint-contracts/commit/4b11961a971dac7dfbb403509f39feb169cbe2e7))
* **backend:** add indexer reliability features ([f3f158b](https://github.com/mergemint-mint/mergemint-contracts/commit/f3f158b23ba68690c93ad8852040ca4ce513bd12))
* **backend:** resolve transaction and webhook issues ([3f84bbc](https://github.com/mergemint-mint/mergemint-contracts/commit/3f84bbcdff5273f2a2aaf9b0daa3c1176774288c))
* **frontend:** add content security policy ([99e7140](https://github.com/mergemint-mint/mergemint-contracts/commit/99e7140a8146321b1200ee2dd9e044ef723d2707)), closes [#1114](https://github.com/mergemint-mint/mergemint-contracts/issues/1114)


### Fixed

* **contract:** remove duplicate code reintroduced by [#1153](https://github.com/mergemint-mint/mergemint-contracts/issues/1153) merge ([2502239](https://github.com/mergemint-mint/mergemint-contracts/commit/250223975a31e8a73f478c88cccde40256245e69))

## [Unreleased]

Changes that have landed on `main` but are not yet associated with a tagged release.

### Changed

- `resolve_dispute` now validates its `resolution` argument (`"complete"` / `"cancel"`) through the shared `symbols::validate_symbol` allow-list, panicking with `resolution must be 'complete' or 'cancel'` on an unrecognised value (#651).

### Added

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
