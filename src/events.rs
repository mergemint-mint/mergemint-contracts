// SPDX-License-Identifier: MIT
use soroban_sdk::{Address, BytesN, Env, Symbol};

pub fn emit_bounty_created(env: &Env, bounty_id: &BytesN<32>, creator: &Address, reward: &i128) {
    let topic = Symbol::new(env, "bounty_created");
    env.events()
        .publish((topic, creator.clone()), (bounty_id.clone(), *reward));
}

pub fn emit_bounty_claimed(env: &Env, bounty_id: &BytesN<32>, contributor: &Address) {
    let topic = Symbol::new(env, "bounty_claimed");
    env.events()
        .publish((topic, contributor.clone()), bounty_id.clone());
}

pub fn emit_bounty_completed(env: &Env, bounty_id: &BytesN<32>, contributor: &Address) {
    let topic = Symbol::new(env, "bounty_completed");
    env.events()
        .publish((topic, contributor.clone()), bounty_id.clone());
}

pub fn emit_reward_paid(env: &Env, bounty_id: &BytesN<32>, contributor: &Address, amount: &i128) {
    let topic = Symbol::new(env, "reward_paid");
    env.events()
        .publish((topic, contributor.clone()), (bounty_id.clone(), *amount));
}

pub fn emit_bounty_disputed(env: &Env, bounty_id: &BytesN<32>, caller: &Address) {
    let topic = Symbol::new(env, "bounty_disputed");
    env.events().publish(
        (topic, caller.clone()),
        bounty_id.clone(),
    );
}

pub fn emit_bounty_updated(env: &Env, bounty_id: &BytesN<32>, creator: &Address) {
    let topic = Symbol::new(env, "bounty_updated");
    env.events().publish(
        (topic, creator.clone()),
        bounty_id.clone(),
    );
}

pub fn emit_bounty_cancelled(env: &Env, bounty_id: &BytesN<32>, creator: &Address) {
    let topic = Symbol::new(env, "bounty_cancelled");
    env.events().publish(
        (topic, creator.clone()),
        bounty_id.clone(),
    );
}

pub fn emit_bounty_expired(env: &Env, bounty_id: &BytesN<32>, creator: &Address) {
    let topic = Symbol::new(env, "bounty_expired");
    env.events().publish(
        (topic, creator.clone()),
        bounty_id.clone(),
    );
}

pub fn emit_approval_recorded(
    env: &Env,
    bounty_id: &BytesN<32>,
    verifier: &Address,
    approval_count: u32,
) {
    let topic = Symbol::new(env, "approval_recorded");
    env.events()
        .publish((topic, verifier.clone()), (bounty_id.clone(), approval_count));
}

pub fn emit_dispute_resolved(
    env: &Env,
    bounty_id: &BytesN<32>,
    arbitrator: &Address,
    resolution: &Symbol,
) {
    let topic = Symbol::new(env, "dispute_resolved");
    env.events()
        .publish((topic, arbitrator.clone()), (bounty_id.clone(), resolution.clone()));
}
