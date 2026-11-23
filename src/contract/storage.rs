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
    /// Ledger sequence at which the current claim(s) were made.
    ClaimedAtLedger,
    /// Total amount of tokens held in contract escrow.
    EscrowBalance,
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

/// Read the ledger sequence at which the current claim(s) were made.
///
/// Returns `None` when no claim timestamp is recorded.
pub fn get_claimed_at_ledger(env: &Env) -> Option<u32> {
    env.storage().instance().get(&DataKey::ClaimedAtLedger)
}

/// Record the ledger sequence at which the current claim(s) were made.
pub fn set_claimed_at_ledger(env: &Env, ledger: u32) {
    env.storage()
        .instance()
        .set(&DataKey::ClaimedAtLedger, &ledger);
}

/// Clear any recorded claim timestamp.
pub fn clear_claimed_at_ledger(env: &Env) {
    env.storage().instance().remove(&DataKey::ClaimedAtLedger);
}

/// Read the total amount of tokens held in contract escrow, defaulting to zero.
pub fn get_escrow_balance(env: &Env) -> i128 {
    env.storage()
        .instance()
        .get(&DataKey::EscrowBalance)
        .unwrap_or(0)
}

/// Persist the total amount of tokens held in contract escrow.
pub fn set_escrow_balance(env: &Env, balance: i128) {
    env.storage()
        .instance()
        .set(&DataKey::EscrowBalance, &balance);
}

/// Add `amount` to the tracked escrow balance, saturating at the type bounds.
///
/// Returns the updated escrow balance.
pub fn add_escrow_balance(env: &Env, amount: i128) -> i128 {
    let updated = get_escrow_balance(env).saturating_add(amount);
    set_escrow_balance(env, updated);
    updated
}

/// Subtract `amount` from the tracked escrow balance, clamping at zero.
///
/// Returns the updated escrow balance.
pub fn sub_escrow_balance(env: &Env, amount: i128) -> i128 {
    let updated = get_escrow_balance(env).saturating_sub(amount);
    set_escrow_balance(env, updated);
    updated
}
