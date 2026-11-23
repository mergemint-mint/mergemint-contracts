// SPDX-License-Identifier: MIT
//! End-to-end `archive_bounty` lifecycle and post-archive query consistency.
extern crate std;

use crate::contract::MergeMintContract;
use crate::storage;
use crate::types::BountyId;
use crate::MergeMintContractClient;
use soroban_sdk::{
    testutils::{Address as _, Ledger as _},
    token::StellarAssetClient,
    vec, Address, Env, String, Symbol, Vec,
};

struct Ctx {
    env: Env,
    contract: Address,
    client: MergeMintContractClient<'static>,
    creator: Address,
    worker: Address,
    verifier: Address,
    token: Address,
}

fn ctx() -> Ctx {
    let env = Env::default();
    env.mock_all_auths();
    let contract = env.register(MergeMintContract, ());
    let client = MergeMintContractClient::new(&env, &contract);
    let creator = Address::generate(&env);
    let token = env.register_stellar_asset_contract_v2(creator.clone()).address();
    StellarAssetClient::new(&env, &token).mint(&contract, &1_000_000);
    Ctx {
        worker: Address::generate(&env),
        verifier: Address::generate(&env),
        env,
        contract,
        client,
        creator,
        token,
    }
}

fn new_bounty(c: &Ctx, title: &str) -> BountyId {
    c.client.create_bounty(
        &c.creator,
        &Symbol::new(&c.env, title),
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

fn completed(c: &Ctx, title: &str) -> BountyId {
    let id = new_bounty(c, title);
    c.client.claim_bounty(&c.worker, &id);
    c.client.complete_bounty(&c.verifier, &id);
    id
}

fn cancelled(c: &Ctx, title: &str) -> BountyId {
    let id = new_bounty(c, title);
    c.client.cancel_bounty(&c.creator, &id);
    id
}

/// Mark `id` archive-eligible `after` ledgers from now.
fn set_marker(c: &Ctx, id: &BountyId, after: u32) {
    let now = c.env.ledger().sequence();
    c.env.as_contract(&c.contract, || {
        storage::set_archive_eligible_at(&c.env, id, now + after)
    });
}

fn advance(c: &Ctx, n: u32) {
    c.env.ledger().with_mut(|l| l.sequence_number += n);
}

#[test]
fn archive_active_bounty_rejected() {
    let c = ctx();
    let id = new_bounty(&c, "active");
    assert!(c.client.try_archive_bounty(&id).is_err());
    c.client.claim_bounty(&c.worker, &id);
    assert!(c.client.try_archive_bounty(&id).is_err()); // in_progress
}

#[test]
fn archive_unknown_bounty_rejected() {
    let c = ctx();
    let id = new_bounty(&c, "known");
    c.client.cancel_bounty(&c.creator, &id);
    let other = completed(&c, "other");
    set_marker(&c, &other, 0);
    c.client.archive_bounty(&other);
    // Archived id is no longer archivable again.
    assert!(c.client.try_archive_bounty(&other).is_err());
}

#[test]
fn archive_grace_period_enforced_then_succeeds() {
    let c = ctx();
    let id = completed(&c, "grace");
    set_marker(&c, &id, 100);
    assert!(c.client.try_archive_bounty(&id).is_err());
    advance(&c, 99);
    assert!(c.client.try_archive_bounty(&id).is_err());
    advance(&c, 1);
    c.client.archive_bounty(&id);
    assert!(c.client.get_bounty_summary(&id).is_some());
}

#[test]
fn archive_completed_queries_consistent() {
    let c = ctx();
    let id = completed(&c, "done");
    let ids = vec![&c.env, id.clone()];
    assert_eq!(c.client.get_bounties_by_token(&c.token, &None, &10).0.len(), 1);
    set_marker(&c, &id, 0);
    c.client.archive_bounty(&id);

    let summary = c.client.get_bounty_summary(&id).expect("summary");
    assert_eq!(summary.creator, c.creator);
    assert_eq!(summary.assignee, Some(c.worker.clone()));
    assert_eq!(summary.reward, 1000);
    assert_eq!(summary.status, Symbol::new(&c.env, "completed"));

    let b = c.client.get_bounty(&id).expect("get_bounty falls back to summary");
    assert_eq!(b.creator, c.creator);
    assert_eq!(b.status, Symbol::new(&c.env, "completed"));

    // Token index pruned; meta removed.
    assert_eq!(c.client.get_bounties_by_token(&c.token, &None, &10).0.len(), 0);
    assert!(c.client.get_bounty_metas(&ids).get(0).unwrap().is_none());
    assert_eq!(c.client.get_bounties_by_ids(&ids).len(), 0);
}

#[test]
fn archive_cancelled_queries_consistent() {
    let c = ctx();
    let id = cancelled(&c, "cxl");
    set_marker(&c, &id, 0);
    c.client.archive_bounty(&id);
    let summary = c.client.get_bounty_summary(&id).expect("summary");
    assert_eq!(summary.assignee, None);
    assert_eq!(summary.status, Symbol::new(&c.env, "cancelled"));
    assert_eq!(c.client.get_bounties_by_token(&c.token, &None, &10).0.len(), 0);
}

#[test]
fn archive_only_prunes_the_archived_bounty_from_token_index() {
    let c = ctx();
    let done = completed(&c, "a");
    let keep = new_bounty(&c, "b");
    set_marker(&c, &done, 0);
    c.client.archive_bounty(&done);
    let (left, _) = c.client.get_bounties_by_token(&c.token, &None, &10);
    assert_eq!(left.len(), 1);
    assert_eq!(left.get(0).unwrap(), keep);
    assert!(c.client.get_bounty_summary(&keep).is_none());
}

#[test]
fn archived_bounty_rejects_mutating_calls() {
    let c = ctx();
    let id = cancelled(&c, "frozen");
    set_marker(&c, &id, 0);
    c.client.archive_bounty(&id);
    assert!(c.client.try_cancel_bounty(&c.creator, &id).is_err());
    assert!(c.client.try_claim_bounty(&c.worker, &id).is_err());
    assert!(c.client.try_complete_bounty(&c.verifier, &id).is_err());
    assert!(c.client.try_extend_deadline(&c.creator, &id, &99_999).is_err());
    assert!(c
        .client
        .try_update_tags(&c.creator, &id, &vec![&c.env, Symbol::new(&c.env, "bug")])
        .is_err());
    assert!(c.client.try_archive_bounty(&id).is_err());
}
