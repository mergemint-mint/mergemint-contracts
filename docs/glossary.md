# Glossary

Definitions for terms used across the MergeMint contract, backend, SDK and
documentation. Terms are listed alphabetically; each heading doubles as an
HTML anchor so other docs can link directly to a definition (e.g.
`[assignee](glossary.md#assignee)`).

---

## Arbitrator

An address authorised to resolve a [dispute](#dispute) by calling
`resolve_dispute`. The arbitrator decides whether the bounty reward should
be paid to the [assignee](#assignee) or refunded to the [creator](#creator).

After the arbitrator records their decision the contract stores a
[`PendingResolution`](../src/types.rs) entry. The resolution is not
finalised immediately: it sits behind an [appeal window](#appeal-window)
so the losing party has an opportunity to call `appeal_dispute`.

See [`docs/security.md`](security.md) for threat-model notes on arbitrator
selection and dispute collusion.

---

## Appeal window

A configurable number of ledgers — set by the admin via `set_appeal_window`
and stored as `DataKey::AppealWindow` — that must elapse after an
[arbitrator](#arbitrator) records their decision before `finalize_dispute`
can execute the payout. Setting the appeal window to `0` restores
pre-appeal, immediate-finalisation behaviour.

---

## Assignee

A contributor address listed in `Bounty.assignees`. An assignee is created
when a [contributor](#contributor) calls `claim_bounty` and is accepted onto
the bounty. Each entry in the `Vec<(Address, u32)>` is a pair of the
assignee's address and the ledger sequence at which they claimed.

A bounty can have at most `max_assignees` concurrent assignees. When the
bounty completes, each assignee receives a proportional share of the reward
and their reputation score is incremented.

---

## Bounty

The primary work unit on MergeMint. A `Bounty` struct records the creator,
reward token and amount, status, deadlines, tags, [milestones](#milestone),
and [assignees](#assignee). Bounties are stored under `DataKey::Bounty(id)`
with additional metadata (title, description) in `DataKey::BountyMeta(id)`.

Bounty lifecycle: `open` → `in_progress` → `submitted` → `completed`
(or `cancelled` / `disputed`). See [`docs/architecture.md`](architecture.md)
for the full state machine diagram.

---

## Claim

The act of a [contributor](#contributor) calling `claim_bounty` to become
an [assignee](#assignee). A claim is subject to:

- `min_reputation` — the contributor must meet the bounty's minimum reputation threshold.
- `claim_cooldown` — a contributor who recently unclaimed the same bounty must wait.
- `claim_window` — if no progress is submitted within the window, the claim becomes stale and can be released by `release_stale_claim`.

---

## Claim window

An admin-configured number of ledgers (`DataKey::ClaimWindow`) after which
an in-progress claim with no work submission becomes *stale*. Any caller
can then invoke `release_stale_claim` to free the assignee slot. A
[reputation penalty](#reputation-penalty) is applied to the assignee whose
claim is released.

---

## Contributor

Any Stellar address that interacts with bounties. A `Contributor` record
tracks `reputation`, `total_earned`, `contribution_count`, `active_claims`,
and optional metadata. The record is created lazily on first interaction
and stored under `DataKey::Contributor(address)`.

---

## Creator

The Stellar address that called `create_bounty`. The creator funds the
reward (in the no-escrow model, by holding the reward tokens and paying
them at completion time). The creator can cancel the bounty while it is
still `open`, and can propose new ownership via
`transfer_bounty_ownership`.

---

## Dispute

A state transition triggered by calling `raise_dispute` on an
`in_progress` bounty. Once disputed, the bounty enters the `"disputed"`
status and normal completion is blocked until an [arbitrator](#arbitrator)
calls `resolve_dispute`.

Evidence hashes can be attached by calling `add_dispute_evidence`; they
are stored under `DataKey::DisputeEvidence(bounty_id)`.

---

## Escrow

The planned future model where the contract itself holds the reward tokens
on behalf of the creator. In the current *no-escrow* model the verifier
holds the tokens and pushes them directly at completion time. The escrow
implementation plan is documented in
[`docs/escrow-implementation-plan.md`](escrow-implementation-plan.md).

---

## Milestone

An optional staged-payout entry in `Bounty.milestones`. Each `Milestone`
has a `description`, a `reward` amount (in the same token as the parent
bounty), a `completed` flag, and an optional per-milestone `deadline`
ledger sequence. When all milestones are completed the bounty can be
finalised.

Milestones must be declared at creation time, their deadlines must be
non-decreasing, and no individual deadline may exceed the parent bounty
deadline.

---

## Reputation

An unsigned 32-bit score (`Contributor.reputation`) that summarises a
contributor's track record. Reputation increases by a fixed amount when a
bounty completes with the contributor as an assignee. It is used as the
gate for `min_reputation` checks on new bounties and decremented by the
[reputation penalty](#reputation-penalty) when a stale claim is released.

---

## Reputation penalty

An admin-configured deduction (`DataKey::ReputationPenalty`, a `u32`)
subtracted from an [assignee's](#assignee) reputation score when their
stale claim is released by `release_stale_claim`. Defaults to `0` (no
penalty) until configured by an admin call to `set_reputation_penalty`.

---

## Sponsor / Top-up

A third party can add funds to an existing bounty by calling
`sponsor_bounty`. The contributed `(address, amount)` pairs are stored
under `DataKey::Sponsors(bounty_id)` and are refunded pro rata if the
bounty is cancelled or expires.

The action of adding funds to an open bounty is also referred to as a
**top-up** in the frontend and SDK.

---

## Verifier

The Stellar address that calls `complete_bounty` (single-verifier flow) or
`approve_completion` (multi-verifier flow). In the no-escrow model the
verifier holds the reward tokens and pushes them to each
[assignee](#assignee) during completion.

A verifier **cannot** be one of the bounty's assignees; the contract
enforces this guard to prevent self-verification.

When `Bounty.required_verifiers` is set, only addresses in that list may
call `approve_completion`, and at least `approval_threshold` approvals
must accumulate before the bounty auto-completes.
