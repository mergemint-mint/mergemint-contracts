# Event Schema

Contract events emitted for indexer integration.

## Events

### bounty_created
- **Topics**: `(Symbol("bounty_created"), creator_address)`
- **Data**: `(bounty_id, reward_amount)`
- **Trigger**: New bounty created
- **Purpose**: Notify indexer of new bounty

### bounty_claimed
- **Topics**: `(Symbol("bounty_claimed"), contributor_address)`
- **Data**: `bounty_id`
- **Trigger**: Contributor claims bounty
- **Purpose**: Notify indexer of bounty assignment

### bounty_disputed
- **Topics**: `(Symbol("bounty_disputed"), caller_address)`
- **Data**: `bounty_id`
- **Trigger**: Dispute raised on bounty
- **Purpose**: Notify indexer of dispute

### bounty_completed
- **Topics**: `(Symbol("bounty_completed"), contributor_address)`
- **Data**: `bounty_id`
- **Trigger**: Bounty completed and reward paid
- **Purpose**: Notify indexer of completion

### reward_paid
- **Topics**: `(Symbol("reward_paid"), contributor_address)`
- **Data**: `(bounty_id, amount)`
- **Trigger**: Token transfer confirmed
- **Purpose**: Confirm payment to indexer

### bounty_cancelled
- **Topics**: `(Symbol("bounty_cancelled"), creator_address)`
- **Data**: `bounty_id`
- **Trigger**: Creator cancels bounty
- **Purpose**: Notify indexer of bounty cancellation

### bounty_expired
- **Topics**: `(Symbol("bounty_expired"), creator_address)`
- **Data**: `bounty_id`
- **Trigger**: Bounty reaches deadline without completion
- **Purpose**: Notify indexer of bounty expiration

### bounty_reassigned
- **Topics**: `(Symbol("bounty_reassigned"), old_assignee_address)`
- **Data**: `(bounty_id, new_assignee_address)`
- **Trigger**: Creator reassigns a claimed bounty via `reassign_bounty`
- **Purpose**: Notify indexer that the assignee on an in-progress bounty changed

### bounty_sponsored
- **Topics**: `(Symbol("bounty_sponsored"), sponsor_address)`
- **Data**: `(bounty_id, amount, new_reward_amount)`
- **Trigger**: A third party co-funds a bounty via `sponsor_bounty`
- **Purpose**: Notify indexer of additional escrowed funds and the sponsor's contribution

### work_submitted
- **Topics**: `(Symbol("work_submitted"), assignee_address)`
- **Data**: `(bounty_id, submission_hash)`
- **Trigger**: Assignee submits work via `submit_work`
- **Purpose**: Notify indexer/verifiers that a bounty is ready for review

### approval_recorded
- **Topics**: `(Symbol("approval_recorded"), verifier_address)`
- **Data**: `(bounty_id, approval_count)`
- **Trigger**: Verifier approves completion
- **Purpose**: Notify indexer of verification progress

### dispute_resolved
- **Topics**: `(Symbol("dispute_resolved"), arbitrator_address)`
- **Data**: `(bounty_id, resolution)`
- **Trigger**: Arbitrator resolves dispute
- **Purpose**: Notify indexer of dispute outcome

### milestone_completed
- **Topics**: `(Symbol("milestone_completed"), milestone_index)`
- **Data**: `(bounty_id, amount)`
- **Trigger**: A milestone is completed and its staged reward paid
- **Purpose**: Notify indexer of milestone-level progress

### contributor_metadata_updated
- **Topics**: `(Symbol("contributor_metadata_updated"), contributor_address)`
- **Data**: `metadata`
- **Trigger**: Contributor metadata is updated via `update_contributor_metadata`
- **Purpose**: Notify indexer of contributor profile changes so cached profiles stay fresh

### bounty_reopened
- **Topics**: `(Symbol("bounty_reopened"), creator_address)`
- **Data**: `(bounty_id, new_deadline)`
- **Trigger**: Bounty creator reopens an expired bounty with a new deadline
- **Purpose**: Notify indexer of bounty reactivation and updated deadline

### bounty_archived
- **Topics**: `(Symbol("bounty_archived"),)`
- **Data**: `bounty_id`
- **Trigger**: `archive_bounty` compacts a finished bounty into a `BountySummary`
- **Purpose**: Tell the indexer the full bounty record is gone so it can rely on `get_bounty_summary` / the degenerate `get_bounty` fallback for this id going forward

### reputation_changed
- **Topics**: `(Symbol("reputation_changed"), contributor_address)`
- **Data**: `(old_reputation, new_reputation, reason)`
- **Trigger**: Emitted every time a contributor's reputation score is written via the
  `set_reputation` helper in `src/contract/mutations.rs`
- **Reasons** (`reason` symbol):
  - `"completion"` — reward applied when a bounty assignee is paid out on completion
    (including arbitrator-resolved completion)
  - `"penalty"` — [reputation penalty](glossary.md#reputation-penalty) applied to each
    assignee when `release_stale_claim` releases a [stale claim](glossary.md#stale-claim)
  - `"decay"` — reserved for future time-based reputation decay
- **Purpose**: Let the leaderboard and profile pages show why a contributor's score moved.
  Unlike the bounty-scoped events above, this event is keyed by `contributor_address`
  and carries no `bounty_id`.

### config_changed
- **Topics**: `(Symbol("config_changed"), Symbol(<key>))`
- **Data**: `(old_value, new_value)`; for `min_reward`, `(token_address, old_value, new_value)`
- **Trigger**: Every admin configuration change:

  | key                  | value type                     | emitted by               |
  |----------------------|--------------------------------|--------------------------|
  | `creator_open_limit` | `u32`                          | `set_creator_open_limit` |
  | `claim_cooldown`     | `u32`                          | `set_claim_cooldown`     |
  | `claim_window`       | `u32`                          | `set_claim_window`       |
  | `reputation_penalty` | `u32`                          | `set_reputation_penalty` |
  | `appeal_window`      | `u32`                          | `set_appeal_window`      |
  | `min_reward`         | `i128` (per token)             | `set_min_reward`         |
  | `admin`              | `Option<Address>` -> `Address` | `accept_admin` (completes `transfer_admin`) |
- **Purpose**: Let the indexer build a full audit trail of admin actions from chain data alone

### ownership_transfer_started
- **Topics**: `(Symbol("ownership_transfer_started"), current_owner_address)`
- **Data**: `(bounty_id, new_owner_address)`
- **Trigger**: Creator proposes a new owner via `transfer_bounty_ownership`
- **Purpose**: Notify indexer of a pending ownership change

### ownership_transferred
- **Topics**: `(Symbol("ownership_transferred"), new_owner_address)`
- **Data**: `(bounty_id, previous_owner_address)`
- **Trigger**: Proposed owner accepts via `accept_bounty_ownership`
- **Purpose**: Notify indexer that the bounty's creator changed

## Queries

### get_contributor_bounty_history
- **Params**: `(contributor_address, offset: u32, limit: u32)`
- **Returns**: `Vec<BountyId>` — the contributor's bounties, newest first
- **Paging**: `offset` skips the most recent `offset` entries; `limit` caps the page size and is clamped to `MAX_HISTORY_PAGE_SIZE`
- **Purpose**: Bounded, paginated lookup of a contributor's bounty history for profile pages and SDK consumers
