// SPDX-License-Identifier: MIT
//! Appeal-window boundary and double-appeal tests for dispute resolution (#1240).
//!
//! Audit note: `appeal_dispute` is allowed while `sequence < decided_at + window`
//! and `finalize_dispute` while `sequence >= decided_at + window`, using the
//! window value current at call time (it is not snapshotted in
//! `PendingResolution`; see `admin_window_change_applies_retroactively`).

use crate::contract::MergeMintContract;
use crate::types::BountyId;
use crate::MergeMintContractClient;
use soroban_sdk::{
    testutils::{Address as _, Ledger as _},
    token::StellarAssetClient,
    Address, Env, String, Symbol, Vec,
};

const START: u32 = 1_000;

struct Ctx {
    env: Env,
    client: MergeMintContractClient<'static>,
    creator: Address,
    contributor: Address,
    id: BountyId,
    decided_at: u32,
}

/// Bounty in `disputed` state with a pending "cancel" resolution decided at
/// `START` under an appeal window of `window` ledgers.
fn setup(window: u32) -> Ctx {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_sequence_number(START);
    let creator = Address::generate(&env);
    let contributor = Address::generate(&env);
    let contract_id = env.register(MergeMintContract, ());
    let client = MergeMintContractClient::new(&env, &contract_id);
    let token = env.register_stellar_asset_contract_v2(creator.clone()).address();
    StellarAssetClient::new(&env, &token).mint(&contract_id, &1000);
    let id = client.create_bounty(
        &creator,
        &Symbol::new(&env, "appeal"),
        &String::from_str(&env, "desc"),
        &1000,
        &token,
        &0,
        &None,
        &Vec::new(&env),
        &1,
        &None,
        &1,
        &Vec::new(&env),
    );
    client.set_appeal_window(&creator, &window);
    client.claim_bounty(&contributor, &id);
    client.raise_dispute(&creator, &id);
    client.resolve_dispute(&creator, &id, &Symbol::new(&env, "cancel"));
    Ctx { env, client, creator, contributor, id, decided_at: START }
}

fn at(c: &Ctx, seq: u32) {
    c.env.ledger().set_sequence_number(seq);
}

#[test]
fn appeal_allowed_one_ledger_before_window_closes() {
    let c = setup(10);
    at(&c, c.decided_at + 9);
    c.client.appeal_dispute(&c.contributor, &c.id);
}

#[test]
fn appeal_rejected_at_and_after_window_boundary() {
    for offset in [10u32, 11] {
        let c = setup(10);
        at(&c, c.decided_at + offset);
        assert!(c.client.try_appeal_dispute(&c.contributor, &c.id).is_err());
    }
}

#[test]
fn finalize_rejected_before_and_allowed_at_boundary() {
    let c = setup(10);
    at(&c, c.decided_at + 9);
    assert!(c.client.try_finalize_dispute(&c.creator, &c.id).is_err());
    at(&c, c.decided_at + 10);
    c.client.finalize_dispute(&c.creator, &c.id);
    // Pending decision is consumed: a second finalize fails.
    assert!(c.client.try_finalize_dispute(&c.creator, &c.id).is_err());
}

#[test]
fn finalize_allowed_one_ledger_after_boundary() {
    let c = setup(10);
    at(&c, c.decided_at + 11);
    c.client.finalize_dispute(&c.creator, &c.id);
}

#[test]
fn only_one_appeal_per_dispute() {
    let c = setup(10);
    c.client.appeal_dispute(&c.contributor, &c.id);
    // Appeal clears the pending decision.
    assert!(c.client.try_appeal_dispute(&c.contributor, &c.id).is_err());
    // Arbitrator re-decides; a second appeal is still rejected.
    c.client
        .resolve_dispute(&c.creator, &c.id, &Symbol::new(&c.env, "cancel"));
    assert!(c.client.try_appeal_dispute(&c.contributor, &c.id).is_err());
    assert!(c.client.try_appeal_dispute(&c.creator, &c.id).is_err());
}

#[test]
fn appeal_requires_party_to_the_bounty() {
    let c = setup(10);
    let stranger = Address::generate(&c.env);
    assert!(c.client.try_appeal_dispute(&stranger, &c.id).is_err());
    // A party can still appeal afterwards: the failed call mutated nothing.
    c.client.appeal_dispute(&c.creator, &c.id);
}

#[test]
fn appeal_without_pending_resolution_fails() {
    let c = setup(10);
    c.client.appeal_dispute(&c.contributor, &c.id);
    assert!(c.client.try_finalize_dispute(&c.creator, &c.id).is_err());
}

/// Documents current behaviour: the window is read at call time, so an admin
/// change while a resolution is pending moves its deadline.
#[test]
fn admin_window_change_applies_retroactively() {
    let c = setup(10);
    c.client.set_appeal_window(&c.creator, &100);
    at(&c, c.decided_at + 10);
    assert!(c.client.try_finalize_dispute(&c.creator, &c.id).is_err());
    c.client.appeal_dispute(&c.contributor, &c.id);
}

/// Deterministic sweep: for several windows, appeal succeeds iff the offset is
/// strictly inside the window, and finalize succeeds iff it is at/after it.
#[test]
fn boundary_sweep_over_windows_and_offsets() {
    for window in [1u32, 2, 5, 17] {
        for offset in 0..=(window + 1) {
            let a = setup(window);
            at(&a, a.decided_at + offset);
            let appealed = a.client.try_appeal_dispute(&a.contributor, &a.id).is_ok();
            assert_eq!(appealed, offset < window, "appeal w={window} o={offset}");

            let f = setup(window);
            at(&f, f.decided_at + offset);
            let finalized = f.client.try_finalize_dispute(&f.creator, &f.id).is_ok();
            assert_eq!(finalized, offset >= window, "finalize w={window} o={offset}");
        }
    }
}
