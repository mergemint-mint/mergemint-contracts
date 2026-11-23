// SPDX-License-Identifier: MIT
use soroban_sdk::{contractevent, Address, BytesN, Env, Symbol};

use crate::types::BountyId;

/// Emitted when a new bounty is created.
///
/// Topics: `(bounty_created, creator)`
/// Data: `(bounty_id, reward)`
#[contractevent(topics = ["bounty_created"], data_format = "vec")]
pub struct BountyCreated {
    #[topic]
    pub creator: Address,
    pub bounty_id: BountyId,
    pub reward: i128,
}

pub fn emit_bounty_created(env: &Env, bounty_id: &BountyId, creator: &Address, reward: &i128) {
    BountyCreated {
        creator: creator.clone(),
        bounty_id: bounty_id.clone(),
        reward: *reward,
    }
    .publish(env);
}

/// Topics: `(bounty_claimed, contributor)`
/// Data: `bounty_id`
#[contractevent(topics = ["bounty_claimed"], data_format = "single-value")]
pub struct BountyClaimed {
    #[topic]
    pub contributor: Address,
    pub bounty_id: BountyId,
}

pub fn emit_bounty_claimed(env: &Env, bounty_id: &BountyId, contributor: &Address) {
    BountyClaimed {
        contributor: contributor.clone(),
        bounty_id: bounty_id.clone(),
    }
    .publish(env);
}

/// Topics: `(bounty_completed, contributor)`
/// Data: `bounty_id`
#[contractevent(topics = ["bounty_completed"], data_format = "single-value")]
pub struct BountyCompleted {
    #[topic]
    pub contributor: Address,
    pub bounty_id: BountyId,
}

pub fn emit_bounty_completed(env: &Env, bounty_id: &BountyId, contributor: &Address) {
    BountyCompleted {
        contributor: contributor.clone(),
        bounty_id: bounty_id.clone(),
    }
    .publish(env);
}

/// Topics: `(reward_paid, contributor)`
/// Data: `(bounty_id, amount)`
#[contractevent(topics = ["reward_paid"], data_format = "vec")]
pub struct RewardPaid {
    #[topic]
    pub contributor: Address,
    pub bounty_id: BountyId,
    pub amount: i128,
}

pub fn emit_reward_paid(env: &Env, bounty_id: &BountyId, contributor: &Address, amount: &i128) {
    RewardPaid {
        contributor: contributor.clone(),
        bounty_id: bounty_id.clone(),
        amount: *amount,
    }
    .publish(env);
}

/// Topics: `(bounty_disputed, caller)`
/// Data: `bounty_id`
#[contractevent(topics = ["bounty_disputed"], data_format = "single-value")]
pub struct BountyDisputed {
    #[topic]
    pub caller: Address,
    pub bounty_id: BountyId,
}

pub fn emit_bounty_disputed(env: &Env, bounty_id: &BountyId, caller: &Address) {
    BountyDisputed {
        caller: caller.clone(),
        bounty_id: bounty_id.clone(),
    }
    .publish(env);
}

/// Topics: `(bounty_unclaimed, contributor)`
/// Data: `bounty_id`
#[contractevent(topics = ["bounty_unclaimed"], data_format = "single-value")]
pub struct BountyUnclaimed {
    #[topic]
    pub contributor: Address,
    pub bounty_id: BountyId,
}

pub fn emit_bounty_unclaimed(env: &Env, bounty_id: &BountyId, contributor: &Address) {
    BountyUnclaimed {
        contributor: contributor.clone(),
        bounty_id: bounty_id.clone(),
    }
    .publish(env);
}

/// Topics: `(dispute_evidence_added, caller)`
/// Data: `(bounty_id, hash)`
#[contractevent(topics = ["dispute_evidence_added"], data_format = "vec")]
pub struct DisputeEvidenceAdded {
    #[topic]
    pub caller: Address,
    pub bounty_id: BountyId,
    pub hash: BytesN<32>,
}

pub fn emit_dispute_evidence_added(
    env: &Env,
    bounty_id: &BountyId,
    caller: &Address,
    hash: &BytesN<32>,
) {
    DisputeEvidenceAdded {
        caller: caller.clone(),
        bounty_id: bounty_id.clone(),
        hash: hash.clone(),
    }
    .publish(env);
}

/// Topics: `(bounty_cancelled, creator)`
/// Data: `bounty_id`
#[contractevent(topics = ["bounty_cancelled"], data_format = "single-value")]
pub struct BountyCancelled {
    #[topic]
    pub creator: Address,
    pub bounty_id: BountyId,
}

pub fn emit_bounty_cancelled(env: &Env, bounty_id: &BountyId, creator: &Address) {
    BountyCancelled {
        creator: creator.clone(),
        bounty_id: bounty_id.clone(),
    }
    .publish(env);
}

/// Topics: `(bounty_expired, creator)`
/// Data: `bounty_id`
#[contractevent(topics = ["bounty_expired"], data_format = "single-value")]
pub struct BountyExpired {
    #[topic]
    pub creator: Address,
    pub bounty_id: BountyId,
}

pub fn emit_bounty_expired(env: &Env, bounty_id: &BountyId, creator: &Address) {
    BountyExpired {
        creator: creator.clone(),
        bounty_id: bounty_id.clone(),
    }
    .publish(env);
}

/// Topics: `(bounty_reopened, creator)`
/// Data: `(bounty_id, new_deadline)`
#[contractevent(topics = ["bounty_reopened"], data_format = "vec")]
pub struct BountyReopened {
    #[topic]
    pub creator: Address,
    pub bounty_id: BountyId,
    pub new_deadline: u32,
}

pub fn emit_bounty_reopened(env: &Env, bounty_id: &BountyId, creator: &Address, new_deadline: u32) {
    BountyReopened {
        creator: creator.clone(),
        bounty_id: bounty_id.clone(),
        new_deadline,
    }
    .publish(env);
}

/// Topics: `(bounty_reassigned, old_assignee)`
/// Data: `(bounty_id, new_assignee)`
#[contractevent(topics = ["bounty_reassigned"], data_format = "vec")]
pub struct BountyReassigned {
    #[topic]
    pub old_assignee: Address,
    pub bounty_id: BountyId,
    pub new_assignee: Address,
}

pub fn emit_bounty_reassigned(
    env: &Env,
    bounty_id: &BountyId,
    old_assignee: &Address,
    new_assignee: &Address,
) {
    BountyReassigned {
        old_assignee: old_assignee.clone(),
        bounty_id: bounty_id.clone(),
        new_assignee: new_assignee.clone(),
    }
    .publish(env);
}

/// Topics: `(bounty_sponsored, sponsor)`
/// Data: `(bounty_id, amount, new_reward_amount)`
#[contractevent(topics = ["bounty_sponsored"], data_format = "vec")]
pub struct BountySponsored {
    #[topic]
    pub sponsor: Address,
    pub bounty_id: BountyId,
    pub amount: i128,
    pub new_reward_amount: i128,
}

pub fn emit_bounty_sponsored(
    env: &Env,
    bounty_id: &BountyId,
    sponsor: &Address,
    amount: &i128,
    new_reward_amount: &i128,
) {
    BountySponsored {
        sponsor: sponsor.clone(),
        bounty_id: bounty_id.clone(),
        amount: *amount,
        new_reward_amount: *new_reward_amount,
    }
    .publish(env);
}

/// Topics: `(work_submitted, assignee)`
/// Data: `(bounty_id, submission_hash)`
#[contractevent(topics = ["work_submitted"], data_format = "vec")]
pub struct WorkSubmitted {
    #[topic]
    pub assignee: Address,
    pub bounty_id: BountyId,
    pub submission_hash: BytesN<32>,
}

pub fn emit_work_submitted(
    env: &Env,
    bounty_id: &BountyId,
    assignee: &Address,
    submission_hash: &BytesN<32>,
) {
    WorkSubmitted {
        assignee: assignee.clone(),
        bounty_id: bounty_id.clone(),
        submission_hash: submission_hash.clone(),
    }
    .publish(env);
}

/// Topics: `(deadline_extended, creator)`
/// Data: `(bounty_id, new_deadline)`
#[contractevent(topics = ["deadline_extended"], data_format = "vec")]
pub struct DeadlineExtended {
    #[topic]
    pub creator: Address,
    pub bounty_id: BountyId,
    pub new_deadline: u32,
}

pub fn emit_deadline_extended(
    env: &Env,
    bounty_id: &BountyId,
    creator: &Address,
    new_deadline: u32,
) {
    DeadlineExtended {
        creator: creator.clone(),
        bounty_id: bounty_id.clone(),
        new_deadline,
    }
    .publish(env);
}

/// Topics: `(bounty_topped_up, creator)`
/// Data: `(bounty_id, amount, new_reward)`
#[contractevent(topics = ["bounty_topped_up"], data_format = "vec")]
pub struct BountyToppedUp {
    #[topic]
    pub creator: Address,
    pub bounty_id: BountyId,
    pub amount: i128,
    pub new_reward: i128,
}

pub fn emit_bounty_topped_up(
    env: &Env,
    bounty_id: &BountyId,
    creator: &Address,
    amount: &i128,
    new_reward: &i128,
) {
    BountyToppedUp {
        creator: creator.clone(),
        bounty_id: bounty_id.clone(),
        amount: *amount,
        new_reward: *new_reward,
    }
    .publish(env);
}

/// Topics: `(approval_recorded, verifier)`
/// Data: `(bounty_id, approval_count)`
#[contractevent(topics = ["approval_recorded"], data_format = "vec")]
pub struct ApprovalRecorded {
    #[topic]
    pub verifier: Address,
    pub bounty_id: BountyId,
    pub approval_count: u32,
}

pub fn emit_approval_recorded(
    env: &Env,
    bounty_id: &BountyId,
    verifier: &Address,
    approval_count: u32,
) {
    ApprovalRecorded {
        verifier: verifier.clone(),
        bounty_id: bounty_id.clone(),
        approval_count,
    }
    .publish(env);
}

/// Topics: `(dispute_resolved, arbitrator)`
/// Data: `(bounty_id, resolution)`
#[contractevent(topics = ["dispute_resolved"], data_format = "vec")]
pub struct DisputeResolved {
    #[topic]
    pub arbitrator: Address,
    pub bounty_id: BountyId,
    pub resolution: Symbol,
}

pub fn emit_dispute_resolved(
    env: &Env,
    bounty_id: &BountyId,
    arbitrator: &Address,
    resolution: &Symbol,
) {
    DisputeResolved {
        arbitrator: arbitrator.clone(),
        bounty_id: bounty_id.clone(),
        resolution: resolution.clone(),
    }
    .publish(env);
}

/// Topics: `(dispute_appealed, caller)`
/// Data: `bounty_id`
#[contractevent(topics = ["dispute_appealed"], data_format = "single-value")]
pub struct DisputeAppealed {
    #[topic]
    pub caller: Address,
    pub bounty_id: BountyId,
}

pub fn emit_dispute_appealed(env: &Env, bounty_id: &BountyId, caller: &Address) {
    DisputeAppealed {
        caller: caller.clone(),
        bounty_id: bounty_id.clone(),
    }
    .publish(env);
}

/// Topics: `(dispute_finalized, arbitrator)`
/// Data: `bounty_id`
#[contractevent(topics = ["dispute_finalized"], data_format = "single-value")]
pub struct DisputeFinalized {
    #[topic]
    pub arbitrator: Address,
    pub bounty_id: BountyId,
}

pub fn emit_dispute_finalized(env: &Env, bounty_id: &BountyId, arbitrator: &Address) {
    DisputeFinalized {
        arbitrator: arbitrator.clone(),
        bounty_id: bounty_id.clone(),
    }
    .publish(env);
}

/// Topics: `(milestone_completed, milestone_index)`
/// Data: `(bounty_id, amount)`
#[contractevent(topics = ["milestone_completed"], data_format = "vec")]
pub struct MilestoneCompleted {
    #[topic]
    pub milestone_index: u32,
    pub bounty_id: BountyId,
    pub amount: i128,
}

pub fn emit_milestone_completed(
    env: &Env,
    bounty_id: &BountyId,
    milestone_index: u32,
    amount: &i128,
) {
    MilestoneCompleted {
        milestone_index,
        bounty_id: bounty_id.clone(),
        amount: *amount,
    }
    .publish(env);
}

/// Topics: `(contributor_metadata_updated, contributor)`
/// Data: `metadata`
///
/// The payload type must match `Contributor.metadata` so indexers can
/// reconcile the emitted value with the stored value.
#[contractevent(topics = ["contributor_metadata_updated"], data_format = "single-value")]
pub struct ContributorMetadataUpdated {
    #[topic]
    pub contributor: Address,
    pub metadata: BytesN<32>,
}

pub fn emit_contributor_metadata_updated(env: &Env, contributor: &Address, metadata: &BytesN<32>) {
    ContributorMetadataUpdated {
        contributor: contributor.clone(),
        metadata: metadata.clone(),
    }
    .publish(env);
}

/// Topics: `(bounty_archived,)`
/// Data: `bounty_id`
#[contractevent(topics = ["bounty_archived"], data_format = "single-value")]
pub struct BountyArchived {
    pub bounty_id: BountyId,
}

pub fn emit_bounty_archived(env: &Env, bounty_id: &BountyId) {
    BountyArchived {
        bounty_id: bounty_id.clone(),
    }
    .publish(env);
}

/// Emit a `reputation_changed` event whenever a contributor's reputation
/// score is updated.
///
/// `reason` is one of the documented symbols in `docs/event-schema.md`
/// (e.g. `"completion"`, `"penalty"`, `"decay"`).
///
/// Topics: `(reputation_changed, contributor)`
/// Data: `(old_reputation, new_reputation, reason)`
#[contractevent(topics = ["reputation_changed"], data_format = "vec")]
pub struct ReputationChanged {
    #[topic]
    pub contributor: Address,
    pub old_reputation: u32,
    pub new_reputation: u32,
    pub reason: Symbol,
}

pub fn emit_reputation_changed(
    env: &Env,
    contributor: &Address,
    old_reputation: u32,
    new_reputation: u32,
    reason: Symbol,
) {
    ReputationChanged {
        contributor: contributor.clone(),
        old_reputation,
        new_reputation,
        reason,
    }
    .publish(env);
}

/// Emitted when a stale claim is released back to `Open` via
/// `release_stale_claim`.
///
/// Topics: `(claim_released, released_by)`
/// Data: `bounty_id`
#[contractevent(topics = ["claim_released"], data_format = "single-value")]
pub struct ClaimReleased {
    #[topic]
    pub released_by: Address,
    pub bounty_id: BountyId,
}

pub fn emit_claim_released(env: &Env, bounty_id: &BountyId, released_by: &Address) {
    ClaimReleased {
        released_by: released_by.clone(),
        bounty_id: bounty_id.clone(),
    }
    .publish(env);
}

/// Emitted when the admin adds a token to the reward-token allowlist.
///
/// Topics: `(reward_token_added, admin)`
/// Data: `token`
#[contractevent(topics = ["reward_token_added"], data_format = "single-value")]
pub struct RewardTokenAdded {
    #[topic]
    pub admin: Address,
    pub token: Address,
}

pub fn emit_reward_token_added(env: &Env, admin: &Address, token: &Address) {
    RewardTokenAdded {
        admin: admin.clone(),
        token: token.clone(),
    }
    .publish(env);
}

/// Emitted when the admin removes a token from the reward-token allowlist.
///
/// Topics: `(reward_token_removed, admin)`
/// Data: `token`
#[contractevent(topics = ["reward_token_removed"], data_format = "single-value")]
pub struct RewardTokenRemoved {
    #[topic]
    pub admin: Address,
    pub token: Address,
}

pub fn emit_reward_token_removed(env: &Env, admin: &Address, token: &Address) {
    RewardTokenRemoved {
        admin: admin.clone(),
        token: token.clone(),
    }
    .publish(env);
}

/// Shared `(config_changed, key)` topic for every admin configuration change,
/// so an indexer can build an audit trail of admin actions from chain data.
///
/// Topics: `(config_changed, key)`
/// Data: `(old_value, new_value)`
#[contractevent(topics = ["config_changed"], data_format = "vec")]
pub struct ConfigChangedU32 {
    #[topic]
    pub key: Symbol,
    pub old_value: u32,
    pub new_value: u32,
}

pub fn emit_config_changed_u32(env: &Env, key: &str, old_value: u32, new_value: u32) {
    ConfigChangedU32 {
        key: Symbol::new(env, key),
        old_value,
        new_value,
    }
    .publish(env);
}

/// Topics: `(config_changed, min_reward)`
/// Data: `(token, old_value, new_value)`
#[contractevent(topics = ["config_changed"], data_format = "vec")]
pub struct ConfigChangedMinReward {
    #[topic]
    pub key: Symbol,
    pub token: Address,
    pub old_value: i128,
    pub new_value: i128,
}

pub fn emit_config_changed_min_reward(
    env: &Env,
    token: &Address,
    old_value: i128,
    new_value: i128,
) {
    ConfigChangedMinReward {
        key: Symbol::new(env, "min_reward"),
        token: token.clone(),
        old_value,
        new_value,
    }
    .publish(env);
}

/// Topics: `(config_changed, key)`
/// Data: `(old_value, new_value)`
#[contractevent(topics = ["config_changed"], data_format = "vec")]
pub struct ConfigChangedAddress {
    #[topic]
    pub key: Symbol,
    pub old_value: Option<Address>,
    pub new_value: Address,
}

pub fn emit_config_changed_address(
    env: &Env,
    key: &str,
    old_value: Option<Address>,
    new_value: &Address,
) {
    ConfigChangedAddress {
        key: Symbol::new(env, key),
        old_value,
        new_value: new_value.clone(),
    }
    .publish(env);
}

/// Topics: `(ownership_transfer_started, current_owner)`
/// Data: `(bounty_id, new_owner)`
#[contractevent(topics = ["ownership_transfer_started"], data_format = "vec")]
pub struct OwnershipTransferStarted {
    #[topic]
    pub current_owner: Address,
    pub bounty_id: BountyId,
    pub new_owner: Address,
}

/// `config_changed` / `arbitrator` with `(old, new)`; `None` means the
/// creator-as-arbitrator fallback is in effect (#1195).
pub fn emit_config_changed_arbitrator(
    env: &Env,
    old_value: Option<Address>,
    new_value: Option<Address>,
) {
    env.events()
        .publish(config_changed_topic(env, "arbitrator"), (old_value, new_value));
}

pub fn emit_ownership_transfer_started(
    env: &Env,
    bounty_id: &BountyId,
    current_owner: &Address,
    new_owner: &Address,
) {
    OwnershipTransferStarted {
        current_owner: current_owner.clone(),
        bounty_id: bounty_id.clone(),
        new_owner: new_owner.clone(),
    }
    .publish(env);
}

/// Topics: `(ownership_transferred, new_owner)`
/// Data: `(bounty_id, previous_owner)`
#[contractevent(topics = ["ownership_transferred"], data_format = "vec")]
pub struct OwnershipTransferred {
    #[topic]
    pub new_owner: Address,
    pub bounty_id: BountyId,
    pub previous_owner: Address,
}

/// Emitted by `cancel_bounty_ownership_transfer` when a creator revokes a
/// pending ownership transfer (#1193).
pub fn emit_ownership_transfer_cancelled(
    env: &Env,
    bounty_id: &BountyId,
    owner: &Address,
    revoked_owner: &Address,
) {
    let topic = Symbol::new(env, "ownership_transfer_cancelled");
    env.events().publish(
        (topic, owner.clone()),
        (bounty_id.clone(), revoked_owner.clone()),
    );
}

pub fn emit_ownership_transferred(
    env: &Env,
    bounty_id: &BountyId,
    previous_owner: &Address,
    new_owner: &Address,
) {
    OwnershipTransferred {
        new_owner: new_owner.clone(),
        bounty_id: bounty_id.clone(),
        previous_owner: previous_owner.clone(),
    }
    .publish(env);
}

/// Emitted when the admin pauses the contract.
///
/// Topics: `(contract_paused,)`
/// Data: `admin`
#[contractevent(topics = ["contract_paused"], data_format = "single-value")]
pub struct ContractPaused {
    pub admin: Address,
}

pub fn emit_contract_paused(env: &Env, admin: &Address) {
    ContractPaused {
        admin: admin.clone(),
    }
    .publish(env);
}

/// Emitted when the admin unpauses the contract.
///
/// Topics: `(contract_unpaused,)`
/// Data: `admin`
#[contractevent(topics = ["contract_unpaused"], data_format = "single-value")]
pub struct ContractUnpaused {
    pub admin: Address,
}

pub fn emit_contract_unpaused(env: &Env, admin: &Address) {
    ContractUnpaused {
        admin: admin.clone(),
    }
    .publish(env);
}
