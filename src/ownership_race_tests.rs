// SPDX-License-Identifier: MIT
//! Interleaving tests for the two-step bounty ownership transfer
//! (`transfer_bounty_ownership` / `accept_bounty_ownership`).

extern crate std;

use crate::contract::MergeMintContract;
use crate::types::BountyId;
use crate::MergeMintContractClient;
use soroban_sdk::{
    testutils::Address as _, token::StellarAssetClient, vec, Address, Env, String, Symbol, Vec,
};

struct Ctx {
    env: Env,
    contract_id: Address,
    creator: Address,
    new_owner: Address,
    other: Address,
    contributor: Address,
    verifier: Address,
}

fn ctx() -> Ctx {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(MergeMintContract, ());
    Ctx {
        creator: Address::generate(&env),
        new_owner: Address::generate(&env),
        other: Address::generate(&env),
        contributor: Address::generate(&env),
        verifier: Address::generate(&env),
        contract_id,
        env,
    }
}

fn make(c: &Ctx, owner: &Address, tag: &str) -> BountyId {
    let client = MergeMintContractClient::new(&c.env, &c.contract_id);
    let token = c
        .env
        .register_stellar_asset_contract_v2(owner.clone())
        .address();
    StellarAssetClient::new(&c.env, &token).mint(&c.contract_id, &1000);
    client.create_bounty(
        owner,
        &Symbol::new(&c.env, tag),
        &String::from_str(&c.env, "desc"),
        &1000,
        &token,
        &0,
        &None,
        &Vec::new(&c.env),
        &1,
        &None,
        &1,
        &Vec::new(&c.env),
    )
}

fn list(c: &Ctx, who: &Address) -> Vec<BountyId> {
    let client = MergeMintContractClient::new(&c.env, &c.contract_id);
    client.get_bounties_by_creator(who, &None, &10).0
}

#[test]
fn claim_by_pending_owner_blocks_accept_and_keeps_indexes() {
    let c = ctx();
    let client = MergeMintContractClient::new(&c.env, &c.contract_id);
    let id = make(&c, &c.creator, "claim");
    client.transfer_bounty_ownership(&c.creator, &id, &c.new_owner);

    // Pending owner claims the bounty before accepting.
    client.claim_bounty(&c.new_owner, &id);
    // Accepting now fails: the pending owner became an assignee.
    assert!(client
        .try_accept_bounty_ownership(&c.new_owner, &id)
        .is_err());

    assert_eq!(client.get_bounty(&id).unwrap().creator, c.creator);
    assert_eq!(list(&c, &c.creator), vec![&c.env, id.clone()]);
    assert_eq!(list(&c, &c.new_owner).len(), 0);
    assert_eq!(client.get_creator_open_count(&c.creator), 1);
    assert_eq!(client.get_creator_open_count(&c.new_owner), 0);
}

#[test]
fn accept_fails_when_new_owner_reaches_open_limit() {
    let c = ctx();
    let client = MergeMintContractClient::new(&c.env, &c.contract_id);
    client.init_admin(&c.other);
    client.set_creator_open_limit(&c.other, &1);

    let id = make(&c, &c.creator, "limit_a");
    make(&c, &c.new_owner, "limit_b");
    client.transfer_bounty_ownership(&c.creator, &id, &c.new_owner);

    assert!(client
        .try_accept_bounty_ownership(&c.new_owner, &id)
        .is_err());
    assert_eq!(client.get_creator_open_count(&c.creator), 1);
    assert_eq!(client.get_creator_open_count(&c.new_owner), 1);
    assert_eq!(list(&c, &c.creator), vec![&c.env, id.clone()]);
    assert_eq!(list(&c, &c.new_owner).len(), 1);
}

#[test]
fn cancel_while_pending_then_accept_keeps_counts_exact() {
    let c = ctx();
    let client = MergeMintContractClient::new(&c.env, &c.contract_id);
    let id = make(&c, &c.creator, "cancel");
    client.transfer_bounty_ownership(&c.creator, &id, &c.new_owner);
    client.cancel_bounty(&c.creator, &id);
    assert_eq!(client.get_creator_open_count(&c.creator), 0);

    // The cancelled bounty is not active, so accepting must not touch open counts.
    let _ = client.try_accept_bounty_ownership(&c.new_owner, &id);
    assert_eq!(client.get_creator_open_count(&c.creator), 0);
    assert_eq!(client.get_creator_open_count(&c.new_owner), 0);
}

#[test]
fn complete_while_pending_then_accept_keeps_counts_exact() {
    let c = ctx();
    let client = MergeMintContractClient::new(&c.env, &c.contract_id);
    let id = make(&c, &c.creator, "complete");
    client.transfer_bounty_ownership(&c.creator, &id, &c.new_owner);
    client.claim_bounty(&c.contributor, &id);
    client.complete_bounty(&c.verifier, &id);
    assert_eq!(client.get_creator_open_count(&c.creator), 0);

    let _ = client.try_accept_bounty_ownership(&c.new_owner, &id);
    assert_eq!(client.get_creator_open_count(&c.creator), 0);
    assert_eq!(client.get_creator_open_count(&c.new_owner), 0);
}

#[test]
fn second_proposal_overwrites_first_pending_owner() {
    let c = ctx();
    let client = MergeMintContractClient::new(&c.env, &c.contract_id);
    let id = make(&c, &c.creator, "overwrite");
    client.transfer_bounty_ownership(&c.creator, &id, &c.new_owner);
    client.transfer_bounty_ownership(&c.creator, &id, &c.other);
    assert_eq!(client.get_pending_owner(&id), Some(c.other.clone()));

    // The overwritten proposal can no longer be accepted.
    assert!(client
        .try_accept_bounty_ownership(&c.new_owner, &id)
        .is_err());
    client.accept_bounty_ownership(&c.other, &id);

    assert_eq!(client.get_bounty(&id).unwrap().creator, c.other);
    assert_eq!(list(&c, &c.creator).len(), 0);
    assert_eq!(list(&c, &c.new_owner).len(), 0);
    assert_eq!(list(&c, &c.other), vec![&c.env, id.clone()]);
    assert_eq!(client.get_creator_open_count(&c.creator), 0);
    assert_eq!(client.get_creator_open_count(&c.new_owner), 0);
    assert_eq!(client.get_creator_open_count(&c.other), 1);
}

#[test]
fn old_owner_loses_rights_after_accept() {
    let c = ctx();
    let client = MergeMintContractClient::new(&c.env, &c.contract_id);
    let id = make(&c, &c.creator, "rights");
    client.transfer_bounty_ownership(&c.creator, &id, &c.new_owner);
    client.accept_bounty_ownership(&c.new_owner, &id);
    assert!(client.try_cancel_bounty(&c.creator, &id).is_err());
    client.cancel_bounty(&c.new_owner, &id);
    assert_eq!(client.get_creator_open_count(&c.new_owner), 0);
}
