// SPDX-License-Identifier: MIT
use soroban_sdk::{Address, Env, Symbol};

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
    env.events().publish(
        (topic, caller.clone()),
        bounty_id.clone(),
    );
}

pub fn emit_bounty_updated(env: &Env, bounty_id: &BountyId, creator: &Address) {
    let topic = Symbol::new(env, "bounty_updated");
    env.events().publish(
        (topic, creator.clone()),
        bounty_id.clone(),
    );
}

pub fn emit_bounty_cancelled(env: &Env, bounty_id: &BountyId, creator: &Address) {
    let topic = Symbol::new(env, "bounty_cancelled");
    env.events().publish(
        (topic, creator.clone()),
        bounty_id.clone(),
    );
}

pub fn emit_bounty_expired(env: &Env, bounty_id: &BountyId, creator: &Address) {
    let topic = Symbol::new(env, "bounty_expired");
    env.events().publish(
        (topic, creator.clone()),
        bounty_id.clone(),
    );
}
