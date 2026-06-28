# Changelog

All notable changes to MergeMint Contracts will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
This project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

<!-- Add entries here as changes are made. Categories: Added, Changed, Deprecated, Removed, Fixed, Security. -->

---

## [0.1.0] — Initial release

### Added

#### Contract Functions

All state-mutating functions enforce caller authentication via `require_auth()`.

| Function | Auth | Description |
|---|---|---|
| `create_bounty(creator, title, description, reward_amount, reward_token, min_reputation, deadline)` | `creator` | Create a new open bounty. Returns a 32-byte bounty ID. |
| `claim_bounty(contributor, bounty_id)` | `contributor` | Claim an open bounty. Sets status to `in_progress`. |
| `complete_bounty(verifier, bounty_id)` | `verifier` | Mark a bounty complete and distribute the reward to all assignees proportionally. |
| `raise_dispute(caller, bounty_id)` | `caller` | Raise a dispute on an in-progress bounty. Only the creator or an assignee may call this. |
| `cancel_bounty(caller, bounty_id)` | `caller` | Cancel an open bounty. Only the creator may call this. |
| `expire_bounty(caller, bounty_id)` | `caller` | Permissionlessly expire an open bounty whose deadline has passed. |
| `update_contributor_metadata(contributor, metadata)` | `contributor` | Set or update the contributor's off-chain metadata URI. |
| `get_bounty(bounty_id)` | — | Read-only. Returns `Option<Bounty>`. |
| `get_bounty_meta(bounty_id)` | — | Read-only. Returns `Option<BountyMeta>`. |
| `get_contributor(address)` | — | Read-only. Returns `Option<Contributor>`. |
| `get_bounty_count()` | — | Read-only. Returns the total number of bounties ever created as `u64`. |
| `get_bounties_by_status(status)` | — | Read-only. Returns `Vec<BytesN<32>>` of bounty IDs in the given status bucket. |
| `get_open_bounties()` | — | Read-only. Returns `Vec<BytesN<32>>` of currently open bounty IDs. |

#### Data Types

**`Bounty`** (`#[contracttype]` struct)

| Field | Type | Description |
|---|---|---|
| `creator` | `Address` | Wallet that created the bounty. Only this address may cancel it. |
| `reward_amount` | `i128` | Total reward in raw token units (not human-readable decimals). |
| `reward_token` | `Address` | Contract address of the Soroban token used to pay the reward. |
| `assignees` | `Vec<(Address, u32)>` | Claimed contributors paired with their basis-point share (shares sum to 10 000). |
| `max_assignees` | `u32` | Maximum number of contributors allowed to claim. Defaults to `1`. |
| `status` | `Symbol` | Lifecycle state: `"open"`, `"in_progress"`, `"completed"`, `"disputed"`, `"cancelled"`. |
| `min_reputation` | `u32` | Minimum contributor reputation required to claim. `0` means open to all. |
| `deadline` | `Option<u32>` | Optional ledger sequence number after which the bounty cannot be claimed. |

**`BountyMeta`** (`#[contracttype]` struct, stored in temporary storage)

| Field | Type | Description |
|---|---|---|
| `title` | `Symbol` | Short human-readable title (max 32 chars, Soroban `Symbol` limit). |
| `description` | `Symbol` | Longer description of the work required. |

**`Contributor`** (`#[contracttype]` struct)

| Field | Type | Description |
|---|---|---|
| `address` | `Address` | Wallet address of the contributor. |
| `reputation` | `u32` | Reputation score. Incremented by `+10` per completed bounty. Never decreases. |
| `total_earned` | `i128` | Cumulative raw token units earned across all completed bounties. |
| `contribution_count` | `u32` | Total number of successfully completed bounties. |
| `active_claims` | `u32` | Number of claimed but not yet completed bounties. Capped at `1` per contributor. |
| `metadata` | `Option<Symbol>` | Optional URI pointing to an off-chain profile document (e.g. IPFS link). |

**`DataKey`** (`#[contracttype]` enum, storage keys)

| Variant | Description |
|---|---|
| `BountyCount` | Singleton `u64` counter of total bounties created. |
| `Bounty(BytesN<32>)` | Persistent storage for a `Bounty` struct keyed by ID. |
| `BountyMeta(BytesN<32>)` | Temporary storage for a `BountyMeta` struct keyed by bounty ID. |
| `Contributor(Address)` | Persistent storage for a `Contributor` profile keyed by wallet address. |
| `StatusIndex(Symbol)` | Persistent storage for a `Vec<BytesN<32>>` of bounty IDs in a given status. |
| `OpenBounties` | Persistent storage for a `Vec<BytesN<32>>` of open bounty IDs (redundant index). |

#### Events

All events are published via `env.events().publish()`.

| Event topic | Payload | Emitted by |
|---|---|---|
| `bounty_created` | `(bounty_id, reward_amount)` | `create_bounty` |
| `bounty_claimed` | `bounty_id` | `claim_bounty` |
| `bounty_completed` | `bounty_id` | `complete_bounty` |
| `reward_paid` | `(bounty_id, amount)` | `complete_bounty` (once per assignee) |
| `bounty_disputed` | `bounty_id` | `raise_dispute` |
| `bounty_cancelled` | `bounty_id` | `cancel_bounty` |
| `bounty_expired` | `bounty_id` | `expire_bounty` |
| `bounty_updated` | `bounty_id` | Reserved — emitted by `emit_bounty_updated` (not yet wired to a handler). |

#### Bounty ID Generation

Bounty IDs are 32-byte values (`BytesN<32>`). The last 8 bytes encode the
monotonically increasing bounty count as a big-endian `u64`. The first 24
bytes are zero-padded in this release. IDs are deterministic and unique
within a single contract deployment.

### Known Limitations

- **No on-chain escrow.** `reward_amount` and `reward_token` are recorded but
  the contract does not hold or transfer tokens during `create_bounty` or
  `cancel_bounty`. Token transfer only occurs in `complete_bounty` (pulled
  from the verifier's balance). Escrow support is planned for a future release.
- **`BountyMeta` uses temporary storage.** Title and description are stored in
  Soroban temporary storage, which is subject to expiry. Long-lived bounties
  may lose their metadata if the TTL is not extended.
- **Active claims capped at 1.** A contributor may hold at most one active
  claim at a time. Attempting a second simultaneous claim panics with
  `"contributor already has an active claim"`.
- **`max_assignees` is always 1.** The field exists and the multi-assignee
  proportional payout logic is implemented, but `create_bounty` hardcodes
  `max_assignees: 1`. There is no public API to create a multi-assignee bounty.
- **`min_reputation` is accepted but the enforcement path has a bug.** The
  reputation check in `claim_bounty` constructs a second `Contributor` default
  without the `active_claims` and `metadata` fields present in `0.1.0`, which
  will cause a compile error if that branch is reached. This will be fixed in a
  future release.
- **No dispute resolution flow.** `raise_dispute` sets status to `"disputed"`
  but there is no `resolve_dispute` function. Disputed bounties cannot
  currently be completed or cancelled.

[Unreleased]: https://github.com/mergemint-mint/mergemint-contracts/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/mergemint-mint/mergemint-contracts/releases/tag/v0.1.0
