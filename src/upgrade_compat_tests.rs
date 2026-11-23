// SPDX-License-Identifier: MIT
//! Upgrade / persistent-storage compatibility harness (#1241).
//!
//! The contract has no `upgrade` entrypoint and no WASM fixture is checked in,
//! so `update_current_contract_wasm` cannot be driven here. An upgrade replaces
//! code but keeps storage under the same contract id; this harness models that
//! by re-binding a fresh client to the same contract id after the ledger
//! advances and asserting that data written before the "upgrade" still
//! deserializes and that the bounty lifecycle keeps working.

use crate::contract::MergeMintContract;
use crate::types::BountyId;
use crate::MergeMintContractClient;
use soroban_sdk::{
    testutils::{Address as _, Ledger as _},
    token::{StellarAssetClient, TokenClient},
    Address, Env, String, Symbol, Vec,
};

struct Ctx {
    env: Env,
    contract_id: Address,
    token: Address,
    creator: Address,
    contributor: Address,
    verifier: Address,
}

fn setup() -> Ctx {
    let env = Env::default();
    env.mock_all_auths();
    let creator = Address::generate(&env);
    let contributor = Address::generate(&env);
    let verifier = Address::generate(&env);
    let contract_id = env.register(MergeMintContract, ());
    let token = env.register_stellar_asset_contract_v2(creator.clone()).address();
    StellarAssetClient::new(&env, &token).mint(&contract_id, &2000);
    Ctx { env, contract_id, token, creator, contributor, verifier }
}

fn bounty(c: &Ctx, tag: &str) -> BountyId {
    MergeMintContractClient::new(&c.env, &c.contract_id).create_bounty(
        &c.creator,
        &Symbol::new(&c.env, tag),
        &String::from_str(&c.env, "desc"),
        &1000,
        &c.token,
        &0,
        &None,
        &Vec::new(&c.env),
        &1,
        &None,
        &1,
        &Vec::new(&c.env),
    )
}

/// Simulated upgrade: new client over the same contract id, later ledger.
fn upgrade(c: &Ctx) -> MergeMintContractClient<'static> {
    let seq = c.env.ledger().sequence();
    c.env.ledger().set_sequence_number(seq + 1_000);
    MergeMintContractClient::new(&c.env, &c.contract_id)
}

#[test]
fn version_is_stable_across_upgrade() {
    let c = setup();
    let before = MergeMintContractClient::new(&c.env, &c.contract_id).version();
    assert_eq!(upgrade(&c).version(), before);
}

#[test]
fn stored_bounty_and_contributor_deserialize_after_upgrade() {
    let c = setup();
    let old = MergeMintContractClient::new(&c.env, &c.contract_id);
    let id = bounty(&c, "snap");
    old.claim_bounty(&c.contributor, &id);
    let snapshot = old.get_bounty(&id).unwrap();

    let new = upgrade(&c);
    let after = new.get_bounty(&id).unwrap();
    assert_eq!(after.status, snapshot.status);
    assert_eq!(after.creator, snapshot.creator);
    assert_eq!(after.assignees.len(), snapshot.assignees.len());
    assert!(new.get_contributor(&c.contributor).is_some());
}

#[test]
fn pre_upgrade_bounty_remains_completable() {
    let c = setup();
    let id = bounty(&c, "comp");
    MergeMintContractClient::new(&c.env, &c.contract_id).claim_bounty(&c.contributor, &id);

    let new = upgrade(&c);
    new.complete_bounty(&c.verifier, &id);
    assert_eq!(
        new.get_bounty(&id).unwrap().status,
        Symbol::new(&c.env, "completed")
    );
}

#[test]
fn pre_upgrade_bounty_remains_refundable() {
    let c = setup();
    let id = bounty(&c, "refund");
    let tok = TokenClient::new(&c.env, &c.token);
    let escrow_before = tok.balance(&c.contract_id);

    let new = upgrade(&c);
    new.cancel_bounty(&c.creator, &id);
    assert_eq!(tok.balance(&c.contract_id), escrow_before - 1000);
    assert_eq!(
        new.get_bounty(&id).unwrap().status,
        Symbol::new(&c.env, "cancelled")
    );
}

/// Admin configuration and pause state live in instance storage and must
/// survive the upgrade; reads keep working while paused.
#[test]
fn admin_and_pause_state_survive_upgrade() {
    let c = setup();
    let old = MergeMintContractClient::new(&c.env, &c.contract_id);
    let admin = Address::generate(&c.env);
    old.init_admin(&admin);
    let id = bounty(&c, "paused");
    old.pause(&admin);

    let new = upgrade(&c);
    assert!(new.try_init_admin(&admin).is_err(), "admin must persist");
    assert!(new.try_claim_bounty(&c.contributor, &id).is_err(), "still paused");
    assert!(new.get_bounty(&id).is_some(), "queries work while paused");
    new.unpause(&admin);
    new.claim_bounty(&c.contributor, &id);
}
