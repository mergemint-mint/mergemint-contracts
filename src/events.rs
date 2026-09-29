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
