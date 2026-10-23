// SPDX-License-Identifier: MIT
use soroban_sdk::{Address, BytesN, Env, Symbol};

use crate::types::BountyId;

pub fn emit_bounty_created(env: &Env, bounty_id: &BountyId, creator: &Address, reward: &i128) {
    let topic = Symbol::new(env, "bounty_created");
    env.events()
        .publish((topic, creator.clone()), (bounty_id.clone(), *reward));
}

pub fn emit_bounty_claimed(env: &Env, bounty_id: &BountyId, contributor: &Address) {
    let topic = Symbol::new(env, "bounty_claimed");
    env.events()
        .publish((topic, contributor.clone()), bounty_id.clone());
}

pub fn emit_bounty_completed(env: &Env, bounty_id: &BountyId, contributor: &Address) {
    let topic = Symbol::new(env, "bounty_completed");
    env.events()
        .publish((topic, contributor.clone()), bounty_id.clone());
}

pub fn emit_reward_paid(env: &Env, bounty_id: &BountyId, contributor: &Address, amount: &i128) {
    let topic = Symbol::new(env, "reward_paid");
    env.events()
        .publish((topic, contributor.clone()), (bounty_id.clone(), *amount));
}

pub fn emit_bounty_disputed(env: &Env, bounty_id: &BountyId, caller: &Address) {
    let topic = Symbol::new(env, "bounty_disputed");
    env.events()
        .publish((topic, caller.clone()), bounty_id.clone());
}

pub fn emit_bounty_unclaimed(env: &Env, bounty_id: &BountyId, contributor: &Address) {
    let topic = Symbol::new(env, "bounty_unclaimed");
    env.events()
        .publish((topic, contributor.clone()), bounty_id.clone());
}

pub fn emit_dispute_evidence_added(
    env: &Env,
    bounty_id: &BountyId,
    caller: &Address,
    hash: &BytesN<32>,
) {
    let topic = Symbol::new(env, "dispute_evidence_added");
    env.events()
        .publish((topic, caller.clone()), (bounty_id.clone(), hash.clone()));
}

pub fn emit_bounty_cancelled(env: &Env, bounty_id: &BountyId, creator: &Address) {
    let topic = Symbol::new(env, "bounty_cancelled");
    env.events()
        .publish((topic, creator.clone()), bounty_id.clone());
}

pub fn emit_bounty_expired(env: &Env, bounty_id: &BountyId, creator: &Address) {
    let topic = Symbol::new(env, "bounty_expired");
    env.events()
        .publish((topic, creator.clone()), bounty_id.clone());
}

pub fn emit_bounty_reopened(env: &Env, bounty_id: &BountyId, creator: &Address, new_deadline: u32) {
    let topic = Symbol::new(env, "bounty_reopened");
    env.events()
        .publish((topic, creator.clone()), (bounty_id.clone(), new_deadline));
}

pub fn emit_bounty_reassigned(
    env: &Env,
    bounty_id: &BountyId,
    old_assignee: &Address,
    new_assignee: &Address,
) {
    let topic = Symbol::new(env, "bounty_reassigned");
    env.events().publish(
        (topic, old_assignee.clone()),
        (bounty_id.clone(), new_assignee.clone()),
    );
}

pub fn emit_bounty_sponsored(
    env: &Env,
    bounty_id: &BountyId,
    sponsor: &Address,
    amount: &i128,
    new_reward_amount: &i128,
) {
    let topic = Symbol::new(env, "bounty_sponsored");
    env.events().publish(
        (topic, sponsor.clone()),
        (bounty_id.clone(), *amount, *new_reward_amount),
    );
}

pub fn emit_work_submitted(
    env: &Env,
    bounty_id: &BountyId,
    assignee: &Address,
    submission_hash: &BytesN<32>,
) {
    let topic = Symbol::new(env, "work_submitted");
    env.events().publish(
        (topic, assignee.clone()),
        (bounty_id.clone(), submission_hash.clone()),
    );
}

pub fn emit_deadline_extended(
    env: &Env,
    bounty_id: &BountyId,
    creator: &Address,
    new_deadline: u32,
) {
    let topic = Symbol::new(env, "deadline_extended");
    env.events()
        .publish((topic, creator.clone()), (bounty_id.clone(), new_deadline));
}

pub fn emit_bounty_topped_up(
    env: &Env,
    bounty_id: &BountyId,
    creator: &Address,
    amount: &i128,
    new_reward: &i128,
) {
    let topic = Symbol::new(env, "bounty_topped_up");
    env.events().publish(
        (topic, creator.clone()),
        (bounty_id.clone(), *amount, *new_reward),
    );
}

pub fn emit_approval_recorded(
    env: &Env,
    bounty_id: &BountyId,
    verifier: &Address,
    approval_count: u32,
) {
    let topic = Symbol::new(env, "approval_recorded");
    env.events().publish(
        (topic, verifier.clone()),
        (bounty_id.clone(), approval_count),
    );
}

pub fn emit_dispute_resolved(
    env: &Env,
    bounty_id: &BountyId,
    arbitrator: &Address,
    resolution: &Symbol,
) {
    let topic = Symbol::new(env, "dispute_resolved");
    env.events().publish(
        (topic, arbitrator.clone()),
        (bounty_id.clone(), resolution.clone()),
    );
}

pub fn emit_dispute_appealed(env: &Env, bounty_id: &BountyId, caller: &Address) {
    let topic = Symbol::new(env, "dispute_appealed");
    env.events()
        .publish((topic, caller.clone()), bounty_id.clone());
}

pub fn emit_dispute_finalized(env: &Env, bounty_id: &BountyId, arbitrator: &Address) {
    let topic = Symbol::new(env, "dispute_finalized");
    env.events()
        .publish((topic, arbitrator.clone()), bounty_id.clone());
}

pub fn emit_milestone_completed(
    env: &Env,
    bounty_id: &BountyId,
    milestone_index: u32,
    amount: &i128,
) {
    let topic = Symbol::new(env, "milestone_completed");
    env.events()
        .publish((topic, milestone_index), (bounty_id.clone(), *amount));
}

pub fn emit_contributor_metadata_updated(env: &Env, contributor: &Address, metadata: &BytesN<32>) {
    let topic = Symbol::new(env, "contributor_metadata_updated");
    env.events()
        .publish((topic, contributor.clone()), metadata.clone());
}

/// Emit a `reputation_changed` event whenever a contributor's reputation
/// score is updated.
///
/// `reason` is one of the documented symbols in `docs/event-schema.md`
/// (e.g. `"completion"`, `"penalty"`, `"decay"`).
pub fn emit_bounty_archived(env: &Env, bounty_id: &BountyId) {
    let topic = Symbol::new(env, "bounty_archived");
    env.events().publish((topic,), bounty_id.clone());
}

pub fn emit_reputation_changed(
    env: &Env,
    contributor: &Address,
    old_reputation: u32,
    new_reputation: u32,
    reason: Symbol,
) {
    let topic = Symbol::new(env, "reputation_changed");
    env.events().publish(
        (topic, contributor.clone()),
        (old_reputation, new_reputation, reason),
    );
}

/// Emitted when a stale claim is released back to `Open` via
/// `release_stale_claim`.
pub fn emit_claim_released(env: &Env, bounty_id: &BountyId, released_by: &Address) {
    let topic = Symbol::new(env, "claim_released");
    env.events()
        .publish((topic, released_by.clone()), bounty_id.clone());
}

/// Emitted when the admin adds a token to the reward-token allowlist.
pub fn emit_reward_token_added(env: &Env, admin: &Address, token: &Address) {
    let topic = Symbol::new(env, "reward_token_added");
    env.events().publish((topic, admin.clone()), token.clone());
}

/// Emitted when the admin removes a token from the reward-token allowlist.
pub fn emit_reward_token_removed(env: &Env, admin: &Address, token: &Address) {
    let topic = Symbol::new(env, "reward_token_removed");
    env.events().publish((topic, admin.clone()), token.clone());
}

/// Shared `(config_changed, key)` topic for every admin configuration change,
/// so an indexer can build an audit trail of admin actions from chain data.
fn config_changed_topic(env: &Env, key: &str) -> (Symbol, Symbol) {
    (Symbol::new(env, "config_changed"), Symbol::new(env, key))
}

pub fn emit_config_changed_u32(env: &Env, key: &str, old_value: u32, new_value: u32) {
    env.events()
        .publish(config_changed_topic(env, key), (old_value, new_value));
}

pub fn emit_config_changed_min_reward(
    env: &Env,
    token: &Address,
    old_value: i128,
    new_value: i128,
) {
    env.events().publish(
        config_changed_topic(env, "min_reward"),
        (token.clone(), old_value, new_value),
    );
}

pub fn emit_config_changed_address(
    env: &Env,
    key: &str,
    old_value: Option<Address>,
    new_value: &Address,
) {
    env.events().publish(
        config_changed_topic(env, key),
        (old_value, new_value.clone()),
    );
}

pub fn emit_ownership_transfer_started(
    env: &Env,
    bounty_id: &BountyId,
    current_owner: &Address,
    new_owner: &Address,
) {
    let topic = Symbol::new(env, "ownership_transfer_started");
    env.events().publish(
        (topic, current_owner.clone()),
        (bounty_id.clone(), new_owner.clone()),
    );
}

pub fn emit_ownership_transferred(
    env: &Env,
    bounty_id: &BountyId,
    previous_owner: &Address,
    new_owner: &Address,
) {
    let topic = Symbol::new(env, "ownership_transferred");
    env.events().publish(
        (topic, new_owner.clone()),
        (bounty_id.clone(), previous_owner.clone()),
    );
}

pub fn emit_admin_initialized(env: &Env, admin: &Address) {
    let topic = Symbol::new(env, "admin_initialized");
    env.events().publish((topic,), admin.clone());
}

pub fn emit_min_reward_amount_updated(env: &Env, token: &Address, amount: &i128) {
    let topic = Symbol::new(env, "min_reward_amount_updated");
    env.events()
        .publish((topic, token.clone()), *amount);
}

pub fn emit_bounty_ownership_transferred(
    env: &Env,
    bounty_id: &BytesN<32>,
    previous_owner: &Address,
    new_owner: &Address,
) {
    let topic = Symbol::new(env, "bounty_ownership_transferred");
    env.events().publish(
        (topic, bounty_id.clone()),
        (previous_owner.clone(), new_owner.clone()),
    );
}
