# Changelog

All notable changes to MergeMint Contracts are documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
Versions follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [Unreleased]

Changes that have landed on `main` but are not yet associated with a tagged release.

### Changed

- `resolve_dispute`, `get_bounties_by_status`, `get_status_count` — raw `Symbol` inputs (`resolution`, `status`) are now checked against a shared allow-list (`validation::validate_symbol`) and panic with `invalid symbol value for this field` on an unrecognised value, replacing the previous per-call ad-hoc checks.

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
