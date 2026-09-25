# Contract Function Reference

This page lists every public entry point on `MergeMintContract`, with its
parameters, auth requirements, errors and events. It was checked against:

- `src/contract/mutations.rs`: state-changing entry points
- `src/contract/queries.rs`: read-only entry points
- `src/errors.rs`: `ContractError` variants and their panic messages
- `src/events.rs`: event topics and payloads
- `src/types.rs`: `Bounty`, `BountyId`, `BountyMeta`, `Contributor`, `Milestone`

For which status transitions each mutation allows, see the
[lifecycle diagram](architecture.md#lifecycle-diagram-mermaid).

> **Keeping this page in sync:** when you add, remove, or change the
> signature, guards or events of a `pub fn` in `src/contract/`, update this
> page in the same PR.

---

## Conventions

**Errors.** The contract fails by panicking through `errors::fail`, so a
client sees a failed invocation whose diagnostic contains the panic message.
Errors are listed **in the order they are checked**, and the first failing
guard wins. `→ "…"` is the exact message from `errors::message`.

**Auth.** `x.require_auth()` means the transaction must carry a valid Soroban
authorization entry for address `x`. Unless noted otherwise, it is the first
statement of the function.

**Events.** Every event is published as `env.events().publish(topics, data)`.
`Symbol` topics are the literal event names shown.

**Token transfers.** Token calls go through the SEP-41 `TokenClient`. A failure
inside the token contract (insufficient balance, missing authorization) aborts
the whole invocation, which rolls back every state change and event. These
failures are not `ContractError`s and are listed as *token error*.

**Escrow funding.** `create_bounty` does **not** pull tokens into the
contract. Payouts from `complete_bounty`, `complete_milestone` and
`approve_completion`, and refunds from `cancel_bounty`, `expire_bounty` and
`resolve_dispute("cancel")`, are paid from the **contract's own balance**. The
contract therefore has to be funded separately for those calls to succeed.
`resolve_dispute("complete")` is the exception: the arbitrator's wallet pays.

### Shared types

| Type | Shape |
|---|---|
| `BountyId` | `BountyId(BytesN<32>)`. The big-endian bounty counter in the last 8 bytes, zeros elsewhere. |
| `Bounty` | `creator`, `reward_amount: i128`, `reward_token: Address`, `assignees: Vec<(Address, u32 /* share in basis points */)>`, `max_assignees: u32`, `status: Symbol`, `min_reputation: u32`, `deadline: Option<u32> /* ledger sequence */`, `required_verifiers: Option<Vec<Address>>`, `approval_threshold: u32`, `tags: Vec<Symbol>`, `milestones: Vec<Milestone>` |
| `BountyMeta` | `title: Symbol`, `description: String` (kept in **temporary** storage) |
| `Contributor` | `address`, `reputation: u32`, `total_earned: i128`, `contribution_count: u32`, `active_claims: u32`, `metadata: Option<Symbol>` |
| `Milestone` | `description: Symbol`, `reward: i128`, `completed: bool` |

Allowed symbol values (`src/symbols.rs`):

- **status**: `open`, `in_progress`, `completed`, `cancelled`, `disputed`
- **tag**: `bug`, `docs`, `feature`, `security`, `test`, `refactor`, `design`, `chore`, `perf`, `other`
- **resolution**: `complete`, `cancel`

---

## Summary

| Function | Kind | Auth | Status change | Events |
|---|---|---|---|---|
| [`create_bounty`](#create_bounty) | mutation | `creator` | — → `open` | `bounty_created` |
| [`claim_bounty`](#claim_bounty) | mutation | `contributor` | `open`/`in_progress` → `in_progress` | `bounty_claimed` |
| [`complete_milestone`](#complete_milestone) | mutation | `verifier` | none | `reward_paid`×N, `milestone_completed` |
| [`complete_bounty`](#complete_bounty) | mutation | `verifier` | `in_progress` → `completed` | `reward_paid`×N, `bounty_completed` |
| [`approve_completion`](#approve_completion) | mutation | `verifier` | → `completed` once the threshold is met | `approval_recorded`, then `reward_paid`×N, `bounty_completed` |
| [`raise_dispute`](#raise_dispute) | mutation | `caller` | `open`/`in_progress` → `disputed` | `bounty_disputed` |
| [`resolve_dispute`](#resolve_dispute) | mutation | `arbitrator` | `disputed` → `completed`/`cancelled` | `reward_paid`×N, `dispute_resolved` |
| [`update_contributor_metadata`](#update_contributor_metadata) | mutation | `contributor` | n/a | none |
| [`cancel_bounty`](#cancel_bounty) | mutation | `caller` (creator) | `open` → `cancelled` | `bounty_cancelled` |
| [`expire_bounty`](#expire_bounty) | mutation | `caller` (anyone) | `open` → `cancelled` | `bounty_expired` |
| [`get_bounty`](#get_bounty) | query | none | — | — |
| [`get_bounties`](#get_bounties) | query | none | — | — |
| [`get_bounty_metas`](#get_bounty_metas) | query | none | — | — |
| [`get_bounty_count`](#get_bounty_count) | query | none | — | — |
| [`get_contributor`](#get_contributor) | query | none | — | — |
| [`get_bounties_by_status`](#get_bounties_by_status) | query | none | — | — |
| [`get_status_count`](#get_status_count) | query | none | — | — |
| [`get_open_bounties`](#get_open_bounties) | query | none | — | — |
| [`get_open_bounties_count`](#get_open_bounties_count) | query | none | — | — |
| [`get_open_bounties_paged`](#get_open_bounties_paged) | query (legacy) | none | — | — |
| [`get_bounties_by_tag`](#get_bounties_by_tag) | query | none | — | — |
| [`get_contributor_active_bounty`](#get_contributor_active_bounty) | query | none | — | — |
| [`get_contributor_bounty_history`](#get_contributor_bounty_history) | query | none | — | — |
| [`get_bounties_by_creator`](#get_bounties_by_creator) | query | none | — | — |

---

## Mutations

### `create_bounty`

```rust
fn create_bounty(
    env: Env,
    creator: Address,
    title: Symbol,
    description: String,
    reward_amount: i128,
    reward_token: Address,
    min_reputation: u32,
    deadline: Option<u32>,
    tags: Vec<Symbol>,
    max_assignees: u32,
    required_verifiers: Option<Vec<Address>>,
    approval_threshold: u32,
    milestones: Vec<Milestone>,
) -> BountyId
```

Creates a bounty in status `open` and returns its ID.

| Param | Description |
|---|---|
| `creator` | Owner of the bounty. The only address that may cancel it or arbitrate its disputes. |
| `title` | Short title (Soroban `Symbol`, max 32 chars). Stored in `BountyMeta`. |
| `description` | Longer description. Stored in `BountyMeta` (temporary storage). |
| `reward_amount` | Reward in raw token units. Must be `>= 100` (`MIN_REWARD_AMOUNT`). |
| `reward_token` | SEP-41 token contract. Probed with `balance()` to confirm it is a token. |
| `min_reputation` | Minimum `Contributor.reputation` needed to claim. `0` means no minimum. |
| `deadline` | Optional ledger sequence. Claims are rejected after it, and it enables `expire_bounty`. |
| `tags` | At most 5, each from the tag allow-list. |
| `max_assignees` | Maximum number of claimants, `>= 1`. The reward is split equally in basis points, and the first claimant gets the remainder. |
| `required_verifiers` | Optional allow-list for `approve_completion`. `None` means single-verifier completion. |
| `approval_threshold` | Approvals needed when `required_verifiers` is set. Must be `<= required_verifiers.len()`. `0` is treated as `1`. |
| `milestones` | Optional staged payouts. When non-empty, the rewards must sum exactly to `reward_amount`. |

**Auth:** `creator.require_auth()`. This runs **after** the argument
validation below, not first. It is the one exception to the rule in
[security.md](security.md#require_auth-placement-audit).

**Errors (in order):**

1. `RewardMustBePositive` → `"reward_amount must be positive"` when `reward_amount <= 0`
2. `RewardBelowMinimum` → `"reward_amount is below the minimum allowed"` when `reward_amount < 100`
3. `TooManyTags` → `"too many tags"` when `tags.len() > 5`
4. `InvalidTag` → `"invalid bounty tag"` when a tag is not on the allow-list
5. `MaxAssigneesMustBePositive` → `"max_assignees must be at least 1"`
6. `ApprovalThresholdExceedsVerifiers` → `"approval_threshold cannot exceed the number of required_verifiers"`
7. `InvalidRewardToken` → `"invalid reward_token address"` when the `balance()` probe fails
8. `RewardAmountOverflow` → `"reward amount arithmetic overflow"` when summing milestone rewards overflows
9. `MilestoneRewardsMismatch` → `"milestone rewards do not sum to reward_amount"`
10. `BountyDeadlinePassed` → `"bounty deadline passed"` when `deadline` is already behind the current ledger

**Events:** `bounty_created`. Topics `("bounty_created", creator)`, data `(bounty_id, reward_amount)`.

**Storage effects:** stores the `Bounty` and `BountyMeta`, increments
`BountyCount`, and adds the bounty to the `open` status index, the creator's
bounty list and the open-bounties index.

---

### `claim_bounty`

```rust
fn claim_bounty(env: Env, contributor: Address, bounty_id: BountyId)
```

Adds `contributor` to `assignees` and moves the bounty to `in_progress`.
Multi-assignee bounties stay claimable in `in_progress` until
`max_assignees` is reached.

| Param | Description |
|---|---|
| `contributor` | Address claiming the bounty. |
| `bounty_id` | Bounty to claim. |

**Auth:** `contributor.require_auth()`.

**Errors (in order):**

1. `BountyNotFound` → `"bounty not found"`
2. `AlreadyClaimed` → `"bounty already claimed by contributor"` when `contributor` is already an assignee
3. `BountyNotOpen` → `"bounty not open"` when the status is not `open` or `in_progress`
4. `CreatorCannotClaim` → `"creator cannot claim"`
5. `BountyAlreadyAssigned` → `"bounty already assigned"` when `assignees.len() >= max_assignees`
6. `ContributorHasActiveClaim` → `"contributor already has an active claim"` when `active_claims >= 1`
7. `BountyDeadlinePassed` → `"bounty deadline passed"`
8. `ReputationTooLow` → `"contributor reputation is too low"`

**Events:** `bounty_claimed`. Topics `("bounty_claimed", contributor)`, data `bounty_id`.

**Storage effects:** increments the contributor's `active_claims`, moves the
bounty between status indexes and removes it from the open-bounties index.

---

### `complete_milestone`

```rust
fn complete_milestone(env: Env, verifier: Address, bounty_id: BountyId, milestone_index: u32)
```

Pays out one milestone to all assignees in proportion to their share, and
marks the milestone `completed`. **Does not change the bounty status.**

| Param | Description |
|---|---|
| `verifier` | Address approving the milestone. It must not be an assignee. |
| `bounty_id` | Bounty containing the milestone. |
| `milestone_index` | Zero-based index into `bounty.milestones`. |

**Auth:** `verifier.require_auth()`.

**Errors (in order):**

1. `BountyNotFound` → `"bounty not found"`
2. `BountyNotInProgress` → `"bounty is not in progress"`
3. `BountyHasNoAssignee` → `"bounty has no assignee"`
4. `InvalidMilestoneIndex` → `"invalid milestone index"`
5. `MilestoneAlreadyCompleted` → `"milestone is already completed"`
6. `VerifierCannotBeAssignee` → `"verifier cannot be the assignee"`
7. *token error* when the contract balance cannot cover the payout

**Events:** one `reward_paid` per assignee (topics `("reward_paid", assignee)`,
data `(bounty_id, payout)`), then `milestone_completed` (topics
`("milestone_completed", milestone_index)`, data `(bounty_id, milestone.reward)`).

**Side effects:** transfers `milestone.reward × share_bp / 10_000` to each
assignee **from the contract**. Each assignee gets `reputation += 10`,
`total_earned += payout`, `contribution_count += 1`, and `active_claims`
decremented.

---

### `complete_bounty`

```rust
fn complete_bounty(env: Env, verifier: Address, bounty_id: BountyId)
```

Moves an `in_progress` bounty to `completed` and pays the full reward to the
assignees.

| Param | Description |
|---|---|
| `verifier` | Address confirming the work. It must not be an assignee. |
| `bounty_id` | Bounty to complete. |

**Auth:** `verifier.require_auth()`.

**Errors (in order):**

1. `BountyNotFound` → `"bounty not found"`
2. `BountyIsDisputed` → `"bounty is disputed"`
3. `BountyNotInProgress` → `"bounty is not in progress"`
4. `BountyHasNoAssignee` → `"bounty has no assignee"`
5. `VerifierCannotBeAssignee` → `"verifier cannot be the assignee"`
6. `NotAllMilestonesCompleted` → `"not all milestones are completed"` (milestone bounties only)
7. *token error* (non-milestone bounties) when the contract balance cannot cover the payout

**Events:**

- Non-milestone bounty: one `reward_paid` per assignee, then `bounty_completed`.
- Milestone bounty: only `bounty_completed`, because the payouts already happened in `complete_milestone`.

`bounty_completed` has topics `("bounty_completed", primary_assignee)` and
data `bounty_id`, where `primary_assignee` is `assignees[0]`.

**Side effects:** the status is written **before** the token transfers
(checks-effects-interactions). The contributor updates are the same as for
`complete_milestone`.

---

### `approve_completion`

```rust
fn approve_completion(env: Env, verifier: Address, bounty_id: BountyId)
```

Records one approval for a multi-verifier bounty. When the number of unique
approvals reaches `approval_threshold` (`0` counts as `1`), it pays out and
marks the bounty `completed`. If `required_verifiers` is `None`, it behaves
like `complete_bounty` and completes immediately.

| Param | Description |
|---|---|
| `verifier` | Approving address. It must be in `required_verifiers` when that is set, and must not be an assignee. |
| `bounty_id` | Bounty being approved. |

**Auth:** `verifier.require_auth()`.

**Errors (in order):**

1. `BountyNotFound` → `"bounty not found"`
2. `BountyHasNoAssignee` → `"bounty has no assignee"`
3. `VerifierCannotBeAssignee` → `"verifier cannot be the assignee"`
4. When `required_verifiers` is `None`: `BountyNotInProgress`, then `NotAllMilestonesCompleted` (same as `complete_bounty`)
5. `VerifierNotAuthorized` → `"verifier is not in the required verifiers list"`
6. `AlreadyApproved` → `"verifier has already approved this bounty"`
7. `NotAllMilestonesCompleted` → `"not all milestones are completed"` (on reaching the threshold, milestone bounties only)
8. *token error* when the threshold is reached on a non-milestone bounty

> ⚠️ **No status guard on the multi-verifier path.** When `required_verifiers`
> is set, the bounty's `status` is never checked. See the
> [lifecycle diagram notes](architecture.md#invalid-transition-reference) and
> [threat-model.md](threat-model.md).

**Events:** `approval_recorded` (topics `("approval_recorded", verifier)`,
data `(bounty_id, approval_count)`). Once the threshold is met, it also emits
`reward_paid` per assignee (non-milestone only) and `bounty_completed`.

---

### `raise_dispute`

```rust
fn raise_dispute(env: Env, caller: Address, bounty_id: BountyId)
```

Moves an `open` or `in_progress` bounty to `disputed`.

| Param | Description |
|---|---|
| `caller` | The bounty creator or one of its assignees. |
| `bounty_id` | Bounty to dispute. |

**Auth:** `caller.require_auth()`.

**Errors (in order):**

1. `BountyNotFound` → `"bounty not found"`
2. `BountyIsDisputed` → `"bounty is disputed"` when it is already disputed
3. `BountyNotDisputed` → `"bounty is not in disputed status"` when the status is `completed` or `cancelled` (the message is misleading here, but this is the error returned)
4. `OnlyCreatorOrAssigneeCanDispute` → `"only creator or assignee can raise dispute"`

**Events:** `bounty_disputed`. Topics `("bounty_disputed", caller)`, data `bounty_id`.

---

### `resolve_dispute`

```rust
fn resolve_dispute(env: Env, arbitrator: Address, bounty_id: BountyId, resolution: Symbol)
```

Settles a `disputed` bounty. `resolution = "complete"` pays the assignees and
moves the bounty to `completed`. `resolution = "cancel"` refunds the creator
and moves it to `cancelled`.

| Param | Description |
|---|---|
| `arbitrator` | Must equal `bounty.creator`. There is no separate admin role. |
| `bounty_id` | Disputed bounty. |
| `resolution` | `complete` or `cancel`. |

**Auth:** `arbitrator.require_auth()`.

**Errors (in order):**

1. `BountyNotFound` → `"bounty not found"`
2. `BountyNotDisputed` → `"bounty is not in disputed status"`
3. `NotArbitrator` → `"caller is not authorized to resolve this dispute"`
4. `InvalidResolution` → `"resolution must be 'complete' or 'cancel'"`
5. `ReputationTooLow` → `"contributor reputation is too low"` when the arbitrator's reputation is below `bounty.min_reputation`
6. On `"complete"`: `BountyHasNoAssignee`, then `NotAllMilestonesCompleted` (milestone bounties)
7. *token error*. On `"complete"`, the **arbitrator's** wallet pays each assignee. On `"cancel"`, the **contract** refunds `reward_amount` to the creator.

**Events:** on `"complete"` for a non-milestone bounty, one `reward_paid` per
assignee. In every case it then emits `dispute_resolved` (topics
`("dispute_resolved", arbitrator)`, data `(bounty_id, resolution)`).
`bounty_completed` / `bounty_cancelled` are **not** emitted, so indexers must
read the outcome from `dispute_resolved`.

---

### `update_contributor_metadata`

```rust
fn update_contributor_metadata(env: Env, contributor: Address, metadata: Symbol)
```

Sets `Contributor.metadata` (for example a short IPFS CID or handle). If no
profile exists yet, one is created.

| Param | Description |
|---|---|
| `contributor` | Profile owner. |
| `metadata` | New value. It must not be empty. |

**Auth:** `contributor.require_auth()`.

**Errors:** `MetadataEmpty` → `"metadata must not be empty"`.

**Events:** none.

---

### `cancel_bounty`

```rust
fn cancel_bounty(env: Env, caller: Address, bounty_id: BountyId)
```

The creator cancels an `open` bounty, and the contract refunds `reward_amount`
to them.

| Param | Description |
|---|---|
| `caller` | Must equal `bounty.creator`. |
| `bounty_id` | Bounty to cancel. |

**Auth:** `caller.require_auth()`.

**Errors (in order):**

1. `BountyNotFound` → `"bounty not found"`
2. `NotBountyCreator` → `"not bounty creator"`
3. `BountyNotOpen` → `"bounty not open"` for any status other than `open`
4. *token error* when the contract balance cannot cover the refund

**Events:** `bounty_cancelled`. Topics `("bounty_cancelled", caller)`, data `bounty_id`.

---

### `expire_bounty`

```rust
fn expire_bounty(env: Env, caller: Address, bounty_id: BountyId)
```

Permissionless expiry of an `open` bounty whose deadline has passed. The
contract refunds `reward_amount` to the **creator**, not to the caller.

| Param | Description |
|---|---|
| `caller` | Any address. It only has to sign. |
| `bounty_id` | Bounty to expire. |

**Auth:** `caller.require_auth()`.

**Errors (in order):**

1. `BountyNotFound` → `"bounty not found"`
2. `BountyNoDeadline` → `"bounty has no deadline"`
3. `DeadlineNotPassed` → `"deadline has not passed"` when `ledger.sequence() <= deadline`
4. `BountyNotOpen` → `"bounty not open"`
5. *token error* when the contract balance cannot cover the refund

**Events:** `bounty_expired`. Topics `("bounty_expired", creator)`, data `bounty_id`.

---

## Queries

None of the queries require auth or emit events. They read persistent
storage, which extends the entry's TTL as described in
[architecture.md](architecture.md#storage-rent-and-ttl-management).

Paginated queries take `cursor: Option<u32>` (a zero-based offset, where
`None` means `0`) and `limit: u32`. A `limit` of `0` or above `50` is
clamped to `50` (`MAX_LIMIT`). They return `(Vec<BountyId>, Option<u32>)`,
where the second element is the next cursor, or `None` once the list is
exhausted.

| Function | Signature | Returns | Errors |
|---|---|---|---|
| <a id="get_bounty"></a>`get_bounty` | `(bounty_id: BountyId) -> Option<Bounty>` | The bounty, or `None` for IDs that were never allocated or have been pruned. | none |
| <a id="get_bounties"></a>`get_bounties` | `(ids: Vec<BountyId>) -> Vec<Option<Bounty>>` | One entry per input ID, in order. | none |
| <a id="get_bounty_metas"></a>`get_bounty_metas` | `(ids: Vec<BountyId>) -> Vec<Option<BountyMeta>>` | Title and description per ID. `None` once the temporary entry has expired. | none |
| <a id="get_bounty_count"></a>`get_bounty_count` | `() -> u64` | Total number of bounties ever created (monotonic). | none |
| <a id="get_contributor"></a>`get_contributor` | `(address: Address) -> Option<Contributor>` | Profile, or `None` if the address has never interacted with the contract. | none |
| <a id="get_bounties_by_status"></a>`get_bounties_by_status` | `(status: Symbol, cursor: Option<u32>, limit: u32) -> (Vec<BountyId>, Option<u32>)` | One page of IDs in `status`. | `InvalidStatus` → `"invalid bounty status"` |
| <a id="get_status_count"></a>`get_status_count` | `(status: Symbol) -> u32` | Number of bounties currently in `status`. | `InvalidStatus` |
| <a id="get_open_bounties"></a>`get_open_bounties` | `(cursor: Option<u32>, limit: u32) -> (Vec<BountyId>, Option<u32>)` | One page of the open-bounties index. | none |
| <a id="get_open_bounties_count"></a>`get_open_bounties_count` | `() -> u32` | Size of the open-bounties index. | none |
| <a id="get_open_bounties_paged"></a>`get_open_bounties_paged` | `(offset: u32, limit: u32) -> Vec<BountyId>` | Legacy offset/limit wrapper around `get_open_bounties`. Prefer the cursor version. | none |
| <a id="get_bounties_by_tag"></a>`get_bounties_by_tag` | `(tag: Symbol) -> Vec<BountyId>` | **Open** bounties carrying `tag`. Unpaginated, O(open bounties). | `InvalidTag` → `"invalid bounty tag"` |
| <a id="get_contributor_active_bounty"></a>`get_contributor_active_bounty` | `(address: Address) -> Option<BountyId>` | The first `in_progress` bounty that lists `address` as an assignee. | none |
| <a id="get_contributor_bounty_history"></a>`get_contributor_bounty_history` | `(address: Address) -> Vec<BountyId>` | Bounties where `address` was an assignee when they reached `completed`/`cancelled`. | none |
| <a id="get_bounties_by_creator"></a>`get_bounties_by_creator` | `(creator: Address, cursor: Option<u32>, limit: u32) -> (Vec<BountyId>, Option<u32>)` | One page of the bounties created by `creator`, in creation order. | none |

---

## Event index

| Event (topic 0) | Topic 1 | Data | Emitted by |
|---|---|---|---|
| `bounty_created` | `creator` | `(BountyId, i128 reward_amount)` | `create_bounty` |
| `bounty_claimed` | `contributor` | `BountyId` | `claim_bounty` |
| `reward_paid` | `assignee` | `(BountyId, i128 payout)` | `complete_milestone`, `complete_bounty`, `approve_completion`, `resolve_dispute("complete")` |
| `milestone_completed` | `milestone_index: u32` | `(BountyId, i128 milestone_reward)` | `complete_milestone` |
| `bounty_completed` | `assignees[0]` | `BountyId` | `complete_bounty`, `approve_completion` |
| `approval_recorded` | `verifier` | `(BountyId, u32 approval_count)` | `approve_completion` (multi-verifier path) |
| `bounty_disputed` | `caller` | `BountyId` | `raise_dispute` |
| `dispute_resolved` | `arbitrator` | `(BountyId, Symbol resolution)` | `resolve_dispute` |
| `bounty_cancelled` | `creator` (the caller) | `BountyId` | `cancel_bounty` |
| `bounty_expired` | `creator` | `BountyId` | `expire_bounty` |

See also [event-schema.md](event-schema.md) for how the backend indexer
consumes these events.

## Error index

Every `ContractError` variant and the entry points that can return it.

| Variant | Message | Returned by |
|---|---|---|
| `BountyNotFound` | bounty not found | all bounty mutations |
| `BountyAlreadyAssigned` | bounty already assigned | `claim_bounty` |
| `AlreadyClaimed` | bounty already claimed by contributor | `claim_bounty` |
| `BountyNotOpen` | bounty not open | `claim_bounty`, `cancel_bounty`, `expire_bounty` |
| `BountyNotInProgress` | bounty is not in progress | `complete_bounty`, `complete_milestone`, `approve_completion` |
| `BountyHasNoAssignee` | bounty has no assignee | `complete_bounty`, `complete_milestone`, `approve_completion`, `resolve_dispute` |
| `RewardMustBePositive` | reward_amount must be positive | `create_bounty` |
| `RewardBelowMinimum` | reward_amount is below the minimum allowed | `create_bounty` |
| `NotBountyCreator` | not bounty creator | `cancel_bounty` |
| `VerifierCannotBeAssignee` | verifier cannot be the assignee | `complete_bounty`, `complete_milestone`, `approve_completion` |
| `CreatorCannotClaim` | creator cannot claim | `claim_bounty` |
| `ContributorHasActiveClaim` | contributor already has an active claim | `claim_bounty` |
| `BountyIsDisputed` | bounty is disputed | `complete_bounty`, `raise_dispute` |
| `BountyDeadlinePassed` | bounty deadline passed | `create_bounty`, `claim_bounty` |
| `BountyNoDeadline` | bounty has no deadline | `expire_bounty` |
| `DeadlineNotPassed` | deadline has not passed | `expire_bounty` |
| `ReputationTooLow` | contributor reputation is too low | `claim_bounty`, `resolve_dispute` |
| `TooManyTags` | too many tags | `create_bounty` |
| `MaxAssigneesMustBePositive` | max_assignees must be at least 1 | `create_bounty` |
| `OnlyCreatorOrAssigneeCanDispute` | only creator or assignee can raise dispute | `raise_dispute` |
| `VerifierNotAuthorized` | verifier is not in the required verifiers list | `approve_completion` |
| `AlreadyApproved` | verifier has already approved this bounty | `approve_completion` |
| `BountyNotDisputed` | bounty is not in disputed status | `raise_dispute`, `resolve_dispute` |
| `NotArbitrator` | caller is not authorized to resolve this dispute | `resolve_dispute` |
| `ApprovalThresholdExceedsVerifiers` | approval_threshold cannot exceed the number of required_verifiers | `create_bounty` |
| `InvalidRewardToken` | invalid reward_token address | `create_bounty` |
| `InvalidStatus` | invalid bounty status | `get_bounties_by_status`, `get_status_count` |
| `InvalidTag` | invalid bounty tag | `create_bounty`, `get_bounties_by_tag` |
| `InvalidResolution` | resolution must be 'complete' or 'cancel' | `resolve_dispute` |
| `MilestoneAlreadyCompleted` | milestone is already completed | `complete_milestone` |
| `NotAllMilestonesCompleted` | not all milestones are completed | `complete_bounty`, `approve_completion`, `resolve_dispute` |
| `InvalidMilestoneIndex` | invalid milestone index | `complete_milestone` |
| `MilestoneRewardsMismatch` | milestone rewards do not sum to reward_amount | `create_bounty` |
| `RewardAmountOverflow` | reward amount arithmetic overflow | `create_bounty` |
| `MetadataEmpty` | metadata must not be empty | `update_contributor_metadata` |
