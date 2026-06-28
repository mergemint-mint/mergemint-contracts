// SPDX-License-Identifier: MIT
use soroban_sdk::{Address, BytesN, Env, Symbol, Vec};

use crate::types::{Bounty, BountyMeta, Contributor, DataKey};

pub fn get_bounty_count(env: &Env) -> u64 {
    env.storage()
        .persistent()
        .get(&DataKey::BountyCount)
        .unwrap_or(0)
}

pub fn set_bounty_count(env: &Env, count: &u64) {
    env.storage().persistent().set(&DataKey::BountyCount, count);
}

pub fn store_bounty(env: &Env, id: &BytesN<32>, bounty: &Bounty) {
    env.storage()
        .persistent()
        .set(&DataKey::Bounty(id.clone()), bounty);
}

pub fn get_bounty(env: &Env, id: &BytesN<32>) -> Option<Bounty> {
    env.storage().persistent().get(&DataKey::Bounty(id.clone()))
}

pub fn store_bounty_meta(env: &Env, id: &BytesN<32>, meta: &BountyMeta) {
    env.storage()
        .temporary()
        .set(&DataKey::BountyMeta(id.clone()), meta);
}

pub fn get_bounty_meta(env: &Env, id: &BytesN<32>) -> Option<BountyMeta> {
    env.storage()
        .temporary()
        .get(&DataKey::BountyMeta(id.clone()))
}

pub fn store_contributor(env: &Env, address: &Address, contributor: &Contributor) {
    env.storage()
        .persistent()
        .set(&DataKey::Contributor(address.clone()), contributor);
}

pub fn get_contributor(env: &Env, address: &Address) -> Option<Contributor> {
    env.storage()
        .persistent()
        .get(&DataKey::Contributor(address.clone()))
}

pub fn get_bounties_by_status(env: &Env, status: &Symbol) -> Vec<BytesN<32>> {
    env.storage()
        .persistent()
        .get(&DataKey::StatusIndex(status.clone()))
        .unwrap_or_else(|| Vec::new(env))
}

pub fn set_bounties_by_status(env: &Env, status: &Symbol, bounties: &Vec<BytesN<32>>) {
    env.storage()
        .persistent()
        .set(&DataKey::StatusIndex(status.clone()), bounties);
}

pub fn add_bounty_to_status(env: &Env, bounty_id: &BytesN<32>, status: &Symbol) {
    let mut current = get_bounties_by_status(env, status);
    if current.iter().all(|id| id != *bounty_id) {
        current.push_back(bounty_id.clone());
    }
    set_bounties_by_status(env, status, &current);
}

pub fn remove_bounty_from_status(env: &Env, bounty_id: &BytesN<32>, status: &Symbol) {
    let current = get_bounties_by_status(env, status);
    let mut updated = Vec::new(env);
    for id in current.iter() {
        if id != *bounty_id {
            updated.push_back(id);
        }
    }
    set_bounties_by_status(env, status, &updated);
}

pub fn move_bounty_status(
    env: &Env,
    bounty_id: &BytesN<32>,
    old_status: &Symbol,
    new_status: &Symbol,
) {
    if old_status != new_status {
        remove_bounty_from_status(env, bounty_id, old_status);
        add_bounty_to_status(env, bounty_id, new_status);
    }
}

pub fn get_open_bounties(env: &Env) -> Vec<BytesN<32>> {
    env.storage()
        .persistent()
        .get(&DataKey::OpenBounties)
        .unwrap_or_else(|| Vec::new(env))
}

pub fn set_open_bounties(env: &Env, bounties: &Vec<BytesN<32>>) {
    env.storage()
        .persistent()
        .set(&DataKey::OpenBounties, bounties);
}

// ── Issue #3: Creator bounties index ─────────────────────────────────────────

pub fn get_creator_bounties(env: &Env, creator: &Address) -> Vec<BytesN<32>> {
    env.storage()
        .persistent()
        .get(&DataKey::CreatorBounties(creator.clone()))
        .unwrap_or_else(|| Vec::new(env))
}

pub fn append_creator_bounty(env: &Env, creator: &Address, bounty_id: &BytesN<32>) {
    let mut list = get_creator_bounties(env, creator);
    list.push_back(bounty_id.clone());
    env.storage()
        .persistent()
        .set(&DataKey::CreatorBounties(creator.clone()), &list);
}
