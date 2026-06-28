use soroban_sdk::{Address, Env, Symbol, Vec};

use crate::types::{Bounty, BountyId, BountyMeta, Contributor, DataKey};

/// Approximate 1 year in ledger sequences at 5 seconds per ledger.
const STORAGE_TTL_LEDGERS: u32 = 6_307_200;
/// Extend TTL when remaining life falls below half a year.
const STORAGE_TTL_THRESHOLD: u32 = STORAGE_TTL_LEDGERS / 2;

/// Returns the total number of bounties ever created.
///
/// Storage key: `DataKey::BountyCount`
/// Stored type: `u64`
/// Default value: `0` (returns 0 if no bounties have been created)
///
/// This is a singleton counter that monotonically increases with each
/// `create_bounty` call. The value is used as part of the bounty ID
/// generation to produce deterministic, unique identifiers.
pub fn get_bounty_count(env: &Env) -> u64 {
    let key = DataKey::BountyCount;
    let count: Option<u64> = env.storage().persistent().get(&key);
    if count.is_some() {
        env.storage()
            .persistent()
            .extend_ttl(&key, STORAGE_TTL_THRESHOLD, STORAGE_TTL_LEDGERS);
    }
    count.unwrap_or(0)
}

/// Sets the global bounty counter to a new value.
///
/// Storage key: `DataKey::BountyCount`
/// Stored type: `u64`
///
/// This should only be called from `create_bounty` when incrementing the
/// counter after a new bounty has been stored. Setting arbitrary values
/// could break the monotonicity of bounty IDs.
pub fn set_bounty_count(env: &Env, count: &u64) {
    let key = DataKey::BountyCount;
    env.storage().persistent().set(&key, count);
    env.storage()
        .persistent()
        .extend_ttl(&key, STORAGE_TTL_THRESHOLD, STORAGE_TTL_LEDGERS);
}

/// Persists a `Bounty` struct under its unique 32-byte identifier.
///
/// Storage key: `DataKey::Bounty(id)`
/// Stored type: `Bounty`
/// Side effects: Overwrites any existing entry for the same `id`.
///
/// This is called during `create_bounty` (initial store) and
/// `claim_bounty` (when the assignee and status are updated).
pub fn store_bounty(env: &Env, id: &BountyId, bounty: &Bounty) {
    let key = DataKey::Bounty(id.clone());
    env.storage().persistent().set(&key, bounty);
    env.storage()
        .persistent()
        .extend_ttl(&key, STORAGE_TTL_THRESHOLD, STORAGE_TTL_LEDGERS);
}

/// Retrieves a `Bounty` by its 32-byte identifier, if it exists.
///
/// Storage key: `DataKey::Bounty(id)`
/// Stored type: `Bounty`
/// Returns: `Some(Bounty)` if found, `None` if no bounty exists for `id`.
///
/// Callers should handle the `None` case (e.g., by panicking with a
/// descriptive message as done in `claim_bounty` and `complete_bounty`).
pub fn get_bounty(env: &Env, id: &BountyId) -> Option<Bounty> {
    let key = DataKey::Bounty(id.clone());
    let bounty: Option<Bounty> = env.storage().persistent().get(&key);
    if bounty.is_some() {
        env.storage()
            .persistent()
            .extend_ttl(&key, STORAGE_TTL_THRESHOLD, STORAGE_TTL_LEDGERS);
    }
    bounty
}

pub fn store_bounty_meta(env: &Env, id: &BountyId, meta: &BountyMeta) {
    env.storage()
        .temporary()
        .set(&DataKey::BountyMeta(id.clone()), meta);
}

pub fn get_bounty_meta(env: &Env, id: &BountyId) -> Option<BountyMeta> {
    env.storage()
        .temporary()
        .get(&DataKey::BountyMeta(id.clone()))
}

/// Persists a `Contributor` profile under the contributor's wallet address.
///
/// Storage key: `DataKey::Contributor(address)`
/// Stored type: `Contributor`
/// Side effects: Overwrites the previous entry for the same `address`.
///
/// Called during `complete_bounty` to update reputation, total earned,
/// and contribution count. Creates a new entry (via `unwrap_or`) if the
/// contributor hasn't been recorded yet.
pub fn store_contributor(env: &Env, address: &Address, contributor: &Contributor) {
    let key = DataKey::Contributor(address.clone());
    env.storage().persistent().set(&key, contributor);
    env.storage()
        .persistent()
        .extend_ttl(&key, STORAGE_TTL_THRESHOLD, STORAGE_TTL_LEDGERS);
}

/// Retrieves a `Contributor` profile by wallet address, if it exists.
///
/// Storage key: `DataKey::Contributor(address)`
/// Stored type: `Contributor`
/// Returns: `Some(Contributor)` if found, `None` if the address has never
/// completed a bounty.
///
/// Callers should use `unwrap_or` with a default `Contributor` when a
/// fresh profile is needed (see `complete_bounty` for an example).
pub fn get_contributor(env: &Env, address: &Address) -> Option<Contributor> {
    let key = DataKey::Contributor(address.clone());
    let contributor: Option<Contributor> = env.storage().persistent().get(&key);
    if contributor.is_some() {
        env.storage()
            .persistent()
            .extend_ttl(&key, STORAGE_TTL_THRESHOLD, STORAGE_TTL_LEDGERS);
    }
    contributor
}

pub fn get_bounties_by_status(env: &Env, status: &Symbol) -> Vec<BountyId> {
    let key = DataKey::StatusIndex(status.clone());
    let result: Option<Vec<BountyId>> = env.storage().persistent().get(&key);
    if result.is_some() {
        env.storage()
            .persistent()
            .extend_ttl(&key, STORAGE_TTL_THRESHOLD, STORAGE_TTL_LEDGERS);
    }
    result.unwrap_or_else(|| Vec::new(env))
}

pub fn set_bounties_by_status(env: &Env, status: &Symbol, bounties: &Vec<BountyId>) {
    let key = DataKey::StatusIndex(status.clone());
    env.storage().persistent().set(&key, bounties);
    env.storage()
        .persistent()
        .extend_ttl(&key, STORAGE_TTL_THRESHOLD, STORAGE_TTL_LEDGERS);
}

pub fn add_bounty_to_status(env: &Env, bounty_id: &BountyId, status: &Symbol) {
    let mut current = get_bounties_by_status(env, status);

    if current.iter().all(|existing_id| existing_id != *bounty_id) {
        current.push_back(bounty_id.clone());
    }

    set_bounties_by_status(env, status, &current);
}

pub fn remove_bounty_from_status(env: &Env, bounty_id: &BountyId, status: &Symbol) {
    let current = get_bounties_by_status(env, status);
    let mut updated = Vec::new(env);

    for existing_id in current.iter() {
        if existing_id != *bounty_id {
            updated.push_back(existing_id);
        }
    }

    set_bounties_by_status(env, status, &updated);
}

pub fn move_bounty_status(
    env: &Env,
    bounty_id: &BountyId,
    old_status: &Symbol,
    new_status: &Symbol,
) {
    if old_status != new_status {
        remove_bounty_from_status(env, bounty_id, old_status);
        add_bounty_to_status(env, bounty_id, new_status);
    }
}

pub fn get_open_bounties(env: &Env) -> Vec<BountyId> {
    let key = DataKey::OpenBounties;
    let result: Option<Vec<BountyId>> = env.storage().persistent().get(&key);
    if result.is_some() {
        env.storage()
            .persistent()
            .extend_ttl(&key, STORAGE_TTL_THRESHOLD, STORAGE_TTL_LEDGERS);
    }
    result.unwrap_or_else(|| Vec::new(env))
}

pub fn set_open_bounties(env: &Env, bounties: &Vec<BountyId>) {
    let key = DataKey::OpenBounties;
    env.storage().persistent().set(&key, bounties);
    env.storage()
        .persistent()
        .extend_ttl(&key, STORAGE_TTL_THRESHOLD, STORAGE_TTL_LEDGERS);
}
