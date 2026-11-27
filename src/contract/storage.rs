use soroban_sdk::{contracttype, Address, Env};

/// Storage keys used by the bounty contract.
#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    /// Address of the contract admin.
    Admin,
    /// Reputation penalty applied when a claimed bounty expires.
    ReputationPenalty,
    /// Reputation score for a given contributor.
    Reputation(Address),
}

/// Default reputation penalty applied to abandoned (expired) claims.
const DEFAULT_REPUTATION_PENALTY: u32 = 10;

/// Read the configured reputation penalty, falling back to the default.
pub fn get_reputation_penalty(env: &Env) -> u32 {
    env.storage()
        .instance()
        .get(&DataKey::ReputationPenalty)
        .unwrap_or(DEFAULT_REPUTATION_PENALTY)
}

/// Configure the reputation penalty applied when a claimed bounty expires.
pub fn set_reputation_penalty(env: &Env, penalty: u32) {
    env.storage()
        .instance()
        .set(&DataKey::ReputationPenalty, &penalty);
}

/// Read a contributor's reputation score, defaulting to zero.
pub fn get_reputation(env: &Env, contributor: &Address) -> u32 {
    env.storage()
        .persistent()
        .get(&DataKey::Reputation(contributor.clone()))
        .unwrap_or(0)
}

/// Persist a contributor's reputation score.
pub fn set_reputation(env: &Env, contributor: &Address, reputation: u32) {
    env.storage()
        .persistent()
        .set(&DataKey::Reputation(contributor.clone()), &reputation);
}

/// Apply the configured penalty to a contributor, clamping at zero.
///
/// Returns the new reputation score after the penalty is applied.
pub fn apply_reputation_penalty(env: &Env, contributor: &Address) -> u32 {
    let current = get_reputation(env, contributor);
    let penalty = get_reputation_penalty(env);
    let updated = current.saturating_sub(penalty);
    set_reputation(env, contributor, updated);
    updated
}
