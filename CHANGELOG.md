# Changelog

All notable changes to MergeMint Contracts are documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
Versions follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [Unreleased]

Changes that have landed on `main` but are not yet associated with a tagged release.

### Added

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
