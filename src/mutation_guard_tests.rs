// SPDX-License-Identifier: MIT
//! Boundary tests that kill `cargo-mutants` operator mutants (`>` vs `>=`,
//! `<` vs `<=`) on the guards in `contract/mutations.rs` and `symbols.rs`.
extern crate std;

use crate::contract::MergeMintContract;
use crate::symbols::{validate_symbol, SymbolKind};
use crate::types::BountyId;
use crate::MergeMintContractClient;
use soroban_sdk::{testutils::Address as _, vec, Address, Env, String, Symbol, Vec};

fn env_client() -> (Env, MergeMintContractClient<'static>, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let id = env.register(MergeMintContract, ());
    let client = MergeMintContractClient::new(&env, &id);
    let creator = Address::generate(&env);
    let token = env.register_stellar_asset_contract_v2(creator.clone()).address();
    (env, client, creator, token)
}

fn create(
    env: &Env,
    c: &MergeMintContractClient,
    creator: &Address,
    token: &Address,
    reward: i128,
    tags: Vec<Symbol>,
    verifiers: Option<Vec<Address>>,
    threshold: u32,
) -> Result<BountyId, ()> {
    match c.try_create_bounty(
        creator,
        &Symbol::new(env, "guard"),
        &String::from_str(env, "d"),
        &reward,
        token,
        &0,
        &None,
        &tags,
        &1,
        &verifiers,
        &threshold,
        &Vec::new(env),
    ) {
        Ok(Ok(id)) => Ok(id),
        _ => Err(()),
    }
}

#[test]
fn min_reward_boundary_99_rejected_100_accepted() {
    let (env, c, creator, token) = env_client();
    assert!(create(&env, &c, &creator, &token, 99, Vec::new(&env), None, 1).is_err());
    assert!(create(&env, &c, &creator, &token, 100, Vec::new(&env), None, 1).is_ok());
    assert!(create(&env, &c, &creator, &token, 0, Vec::new(&env), None, 1).is_err());
}

#[test]
fn max_tags_boundary_five_ok_six_rejected() {
    let (env, c, creator, token) = env_client();
    let names = ["bug", "docs", "feature", "security", "test", "refactor"];
    let mut five = Vec::new(&env);
    for n in &names[..5] {
        five.push_back(Symbol::new(&env, n));
    }
    let mut six = five.clone();
    six.push_back(Symbol::new(&env, names[5]));
    assert!(create(&env, &c, &creator, &token, 1000, five, None, 1).is_ok());
    assert!(create(&env, &c, &creator, &token, 1000, six, None, 1).is_err());
}

#[test]
fn approval_threshold_boundary_equal_ok_above_rejected() {
    let (env, c, creator, token) = env_client();
    let v = vec![&env, Address::generate(&env), Address::generate(&env)];
    assert!(create(&env, &c, &creator, &token, 1000, Vec::new(&env), Some(v.clone()), 2).is_ok());
    assert!(create(&env, &c, &creator, &token, 1000, Vec::new(&env), Some(v), 3).is_err());
}

#[test]
fn symbol_allow_lists_accept_each_member_and_reject_neighbours() {
    let env = Env::default();
    for s in ["open", "in_progress", "completed", "cancelled", "disputed", "expired", "submitted"] {
        assert!(validate_symbol(&env, SymbolKind::Status, &Symbol::new(&env, s)).is_ok());
    }
    for s in ["bug", "docs", "feature", "security", "test", "refactor", "design", "chore", "perf", "other"] {
        assert!(validate_symbol(&env, SymbolKind::Tag, &Symbol::new(&env, s)).is_ok());
    }
    for s in ["complete", "cancel"] {
        assert!(validate_symbol(&env, SymbolKind::Resolution, &Symbol::new(&env, s)).is_ok());
    }
    for s in ["opened", "complet", "bugs", "Open", "canceled"] {
        let sym = Symbol::new(&env, s);
        assert!(validate_symbol(&env, SymbolKind::Status, &sym).is_err());
        assert!(validate_symbol(&env, SymbolKind::Tag, &sym).is_err());
        assert!(validate_symbol(&env, SymbolKind::Resolution, &sym).is_err());
    }
    // Kinds must not leak into each other.
    assert!(validate_symbol(&env, SymbolKind::Tag, &Symbol::new(&env, "open")).is_err());
    assert!(validate_symbol(&env, SymbolKind::Status, &Symbol::new(&env, "bug")).is_err());
    assert!(validate_symbol(&env, SymbolKind::Resolution, &Symbol::new(&env, "open")).is_err());
}

#[test]
fn min_reputation_boundary_zero_reputation_blocked_when_floor_is_one() {
    let (env, c, creator, token) = env_client();
    let id = c.create_bounty(
        &creator,
        &Symbol::new(&env, "rep"),
        &String::from_str(&env, "d"),
        &1000,
        &token,
        &1,
        &None,
        &Vec::new(&env),
        &1,
        &None,
        &1,
        &Vec::new(&env),
    );
    let worker = Address::generate(&env);
    assert!(c.try_claim_bounty(&worker, &id).is_err());
}

#[test]
fn deadline_boundary_claim_at_deadline_vs_after() {
    use soroban_sdk::testutils::Ledger as _;
    let (env, c, creator, token) = env_client();
    let seq = env.ledger().sequence();
    let id = c.create_bounty(
        &creator,
        &Symbol::new(&env, "dl"),
        &String::from_str(&env, "d"),
        &1000,
        &token,
        &0,
        &Some(seq + 10),
        &Vec::new(&env),
        &1,
        &None,
        &1,
        &Vec::new(&env),
    );
    env.ledger().with_mut(|l| l.sequence_number = seq + 11);
    let worker = Address::generate(&env);
    assert!(c.try_claim_bounty(&worker, &id).is_err());
}
