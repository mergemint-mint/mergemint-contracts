# Glossary

Terms used across the MergeMint docs, defined by what the contract actually
does. When a doc and this page disagree, the code in `src/contract/` wins and
this page should be fixed. See the [contract reference](contract-reference.md)
for full function signatures and errors.

[Admin](#admin) · [Appeal window](#appeal-window) ·
[Approval threshold](#approval-threshold) · [Arbitrator](#arbitrator) ·
[Assignee](#assignee) · [Bounty](#bounty) · [Claim](#claim) ·
[Claim window](#claim-window) · [Contributor](#contributor) ·
[Creator](#creator) · [Deadline](#deadline) · [Dispute](#dispute) ·
[Milestone](#milestone) · [Reputation](#reputation) ·
[Reputation penalty](#reputation-penalty) ·
[Required verifiers](#required-verifiers) · [Reward token](#reward-token) ·
[Share](#share) · [Sponsor](#sponsor) · [Stale claim](#stale-claim) ·
[Status](#status) · [Submission](#submission) · [Top-up](#top-up) ·
[Verifier](#verifier)

---

### Admin

The contract-wide administrator. It is set once with `init_admin`, can be
handed over with the two-step `transfer_admin` / `accept_admin`, and is the
only address allowed to change contract settings such as the
[claim window](#claim-window), [reputation penalty](#reputation-penalty),
[appeal window](#appeal-window) and reward-token allowlist. The admin has
**no** say in individual bounties: it is not the [arbitrator](#arbitrator).

### Appeal window

The number of ledgers after an [arbitrator](#arbitrator)'s decision during
which the creator or an assignee may call `appeal_dispute`. Set by the
[admin](#admin) with `set_appeal_window`. With the default of `0`,
`resolve_dispute` settles immediately. Otherwise the decision is stored and
only takes effect when someone calls `finalize_dispute` after the window. Each
dispute can be appealed once.

### Approval threshold

How many [required verifiers](#required-verifiers) must call
`approve_completion` before a bounty completes. Only used when
`required_verifiers` is set. `0` is treated as `1`, and it can never exceed the
number of required verifiers.

### Arbitrator

The address that settles a [dispute](#dispute) with `resolve_dispute`. The
contract requires the arbitrator to be the bounty's [creator](#creator); there
is no independent arbitrator or admin role. The arbitrator must also meet the
bounty's `min_reputation`. Resolving with `complete` pays the assignees from
the arbitrator's own wallet; resolving with `cancel` refunds the creator from
the contract.

### Assignee

A [contributor](#contributor) who has [claimed](#claim) a bounty and is
listed in `Bounty.assignees`, together with their [share](#share). A bounty
can have up to `max_assignees` assignees. An assignee can never be the
creator, and can never act as the verifier of the same bounty.

### Bounty

A unit of paid work recorded on-chain: a reward amount in a
[reward token](#reward-token), a [creator](#creator), its
[assignees](#assignee), a [status](#status) and optional
[milestones](#milestone), [deadline](#deadline) and
[required verifiers](#required-verifiers). It is identified by a `BountyId`
(32 bytes).

### Claim

Calling `claim_bounty` to become an [assignee](#assignee). A claim needs the
bounty to be `open` (or `in_progress` with free assignee slots), the deadline
not passed, enough [reputation](#reputation), and no other active claim by the
same contributor. Unclaiming (`unclaim_bounty`) releases the slot. A claim
that sits idle too long becomes a [stale claim](#stale-claim).

### Claim window

The number of ledgers after the first claim on a bounty before that claim
counts as a [stale claim](#stale-claim). Defaults to `120960` ledgers (about
seven days) and can be changed by the [admin](#admin) with `set_claim_window`.

### Contributor

Any address with a contributor profile (`Contributor`), which stores its
[reputation](#reputation), `total_earned`, `contribution_count`,
`active_claims` and optional metadata. A profile is created the first time an
address claims a bounty, is paid or sets its metadata.

### Creator

The address that called `create_bounty`, also called the bounty owner. It can
cancel an `open` bounty, extend its deadline, reassign it, edit its tags, and
is the [arbitrator](#arbitrator) of its disputes. Ownership can be moved with
`transfer_bounty_ownership` / `accept_bounty_ownership`.

### Deadline

An optional ledger sequence on a bounty (and optionally on each
[milestone](#milestone)). After it, the bounty can no longer be claimed, and
anyone can call `expire_bounty` on an `open` bounty to refund the creator.

### Dispute

A bounty in the `disputed` [status](#status), entered when its creator or an
assignee calls `raise_dispute`. Either party can attach evidence hashes with
`add_dispute_evidence`. It ends when the [arbitrator](#arbitrator)'s decision
is applied, possibly after an [appeal window](#appeal-window).

### Milestone

An optional staged payout inside a bounty. The milestone rewards must add up
exactly to the bounty's reward. A [verifier](#verifier) pays one milestone at a
time with `complete_milestone`, which pays every assignee their
[share](#share) of it without changing the bounty status. The bounty can only
be completed once all milestones are completed.

### Reputation

A contributor's score (`Contributor.reputation`). It goes up by `10` for each
payout (milestone or full completion) and down by the
[reputation penalty](#reputation-penalty) when a
[stale claim](#stale-claim) is released. It never drops below `0`. Bounties can
require a minimum reputation (`min_reputation`) to claim. Every change emits a
`reputation_changed` event with the reason `completion` or `penalty`.

### Reputation penalty

The reputation taken from each assignee when their claim is released by
`release_stale_claim`. Defaults to `10` and can be changed by the
[admin](#admin) with `set_reputation_penalty`. It is the only way reputation
goes down.

### Required verifiers

An optional allow-list of addresses set at creation. When present, completion
goes through `approve_completion` and needs the
[approval threshold](#approval-threshold) to be reached. When absent, any
single [verifier](#verifier) can complete the bounty.

### Reward token

The SEP-41 token contract a bounty pays in. If the [admin](#admin) has
allow-listed any token, only allow-listed tokens can be used. A per-token
minimum reward can also be set.

### Share

An assignee's portion of the reward, in basis points (`10000` = 100%). The
reward is split equally between `max_assignees`, and the first claimant gets
any remainder.

### Sponsor

An address that adds funds to someone else's bounty with `sponsor_bounty`. See
[top-up](#top-up).

### Stale claim

A claim on an `in_progress` bounty that is older than the
[claim window](#claim-window). Anyone can call `release_stale_claim` on it:
every assignee loses the [reputation penalty](#reputation-penalty) and their
active-claim slot, and the bounty goes back to `open`.

### Status

Where a bounty is in its lifecycle: `open`, `in_progress`, `submitted`,
`disputed`, `completed`, `cancelled` or `expired`. See the
[lifecycle diagram](architecture.md#lifecycle-diagram-mermaid).

### Submission

The work hash an assignee records with `submit_work`. It moves the bounty from
`in_progress` to `submitted`.

### Top-up

Adding more reward to an existing bounty. On-chain this is `sponsor_bounty`:
the [sponsor](#sponsor) transfers the tokens into the contract,
`reward_amount` grows by that amount, and the contribution is recorded so it
can be refunded if the bounty is cancelled or expires. Top-ups are accepted
while the bounty is `open`, `in_progress` or `submitted`. The SDK's `topUp`
helper and some older docs call the entry point `top_up`.

### Verifier

The address that confirms work and triggers payment, through
`complete_bounty`, `complete_milestone` or `approve_completion`. Any address
other than an assignee can be the verifier, unless the bounty has
[required verifiers](#required-verifiers), in which case only those addresses
can approve.
