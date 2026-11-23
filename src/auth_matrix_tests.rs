// SPDX-License-Identifier: MIT
//! Strict `env.mock_auths` authorization matrix tests (#1239).
//!
//! Setup uses `mock_all_auths`; every assertion then clears the mock with
//! `mock_auths(&[])` or replaces it with an exact `MockAuth` tree, so only the
//! declared signer / function / arguments / sub-invocations are authorized.

use crate::contract::MergeMintContract;
use crate::types::BountyId;
use crate::MergeMintContractClient;
use soroban_sdk::{
    testutils::{Address as _, MockAuth, MockAuthInvoke},
    token::StellarAssetClient,
    Address, Env, IntoVal, String, Symbol, Vec,
};

struct Ctx {
    env: Env,
    contract_id: Address,
    token: Address,
    creator: Address,
    contributor: Address,
    admin: Address,
    id: BountyId,
}

fn setup() -> Ctx {
    let env = Env::default();
    env.mock_all_auths();
    let creator = Address::generate(&env);
    let contributor = Address::generate(&env);
    let admin = Address::generate(&env);
    let contract_id = env.register(MergeMintContract, ());
    let client = MergeMintContractClient::new(&env, &contract_id);
    let token = env.register_stellar_asset_contract_v2(creator.clone()).address();
    StellarAssetClient::new(&env, &token).mint(&contract_id, &1000);
    let id = client.create_bounty(
        &creator,
        &Symbol::new(&env, "auth"),
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
    client.init_admin(&admin);
    Ctx { env, contract_id, token, creator, contributor, admin, id }
}

/// With no authorization at all, every mutating entrypoint must be rejected.
#[test]
fn every_mutating_entrypoint_rejects_missing_auth() {
    let c = setup();
    let client = MergeMintContractClient::new(&c.env, &c.contract_id);
    let other = Address::generate(&c.env);
    c.env.mock_auths(&[]);

    assert!(client.try_claim_bounty(&c.contributor, &c.id).is_err());
    assert!(client.try_unclaim_bounty(&c.contributor, &c.id).is_err());
    assert!(client.try_raise_dispute(&c.creator, &c.id).is_err());
    assert!(client.try_resolve_dispute(&c.creator, &c.id, &Symbol::new(&c.env, "cancel")).is_err());
    assert!(client.try_finalize_dispute(&other, &c.id).is_err());
    assert!(client.try_appeal_dispute(&c.creator, &c.id).is_err());
    assert!(client.try_set_appeal_window(&c.admin, &5).is_err());
    assert!(client.try_cancel_bounty(&c.creator, &c.id).is_err());
    assert!(client.try_complete_bounty(&other, &c.id).is_err());
    assert!(client.try_sponsor_bounty(&other, &c.id, &10).is_err());
    assert!(client.try_extend_deadline(&c.creator, &c.id, &100).is_err());
    assert!(client.try_update_tags(&c.creator, &c.id, &Vec::new(&c.env)).is_err());
    assert!(client.try_reassign_bounty(&c.creator, &c.id, &other).is_err());
    assert!(client.try_update_contributor_metadata(&c.contributor, &Symbol::new(&c.env, "m")).is_err());
    assert!(client.try_pause(&c.admin).is_err());
    assert!(client.try_unpause(&c.admin).is_err());
    assert!(client.try_set_claim_window(&c.admin, &5).is_err());
    assert!(client.try_set_claim_cooldown(&c.admin, &5).is_err());
    assert!(client.try_set_creator_open_limit(&c.admin, &5).is_err());
    assert!(client.try_set_min_reward(&c.admin, &c.token, &1).is_err());
    assert!(client.try_add_reward_token(&c.admin, &c.token).is_err());
    assert!(client.try_remove_reward_token(&c.admin, &c.token).is_err());
    assert!(client.try_transfer_admin(&c.admin, &other).is_err());

    // No state was mutated by the rejected calls.
    assert_eq!(client.get_bounty(&c.id).unwrap().assignees.len(), 0);
}

/// Authorizing a different signer than the one the entrypoint requires fails.
#[test]
fn wrong_signer_is_rejected_for_admin_entrypoint() {
    let c = setup();
    let client = MergeMintContractClient::new(&c.env, &c.contract_id);
    let attacker = Address::generate(&c.env);
    c.env.mock_auths(&[MockAuth {
        address: &attacker,
        invoke: &MockAuthInvoke {
            contract: &c.contract_id,
            fn_name: "pause",
            args: (c.admin.clone(),).into_val(&c.env),
            sub_invokes: &[],
        },
    }]);
    assert!(client.try_pause(&c.admin).is_err());
}

/// An exact mock for the right signer, function and arguments succeeds.
#[test]
fn exact_mock_auth_allows_claim_bounty() {
    let c = setup();
    let client = MergeMintContractClient::new(&c.env, &c.contract_id);
    c.env.mock_auths(&[MockAuth {
        address: &c.contributor,
        invoke: &MockAuthInvoke {
            contract: &c.contract_id,
            fn_name: "claim_bounty",
            args: (c.contributor.clone(), c.id.clone()).into_val(&c.env),
            sub_invokes: &[],
        },
    }]);
    client.claim_bounty(&c.contributor, &c.id);
    assert_eq!(client.get_bounty(&c.id).unwrap().assignees.len(), 1);
}

/// A mock authorizing the same signer for a different function does not leak
/// authority to other entrypoints.
#[test]
fn mock_auth_for_other_function_does_not_authorize_call() {
    let c = setup();
    let client = MergeMintContractClient::new(&c.env, &c.contract_id);
    c.env.mock_auths(&[MockAuth {
        address: &c.creator,
        invoke: &MockAuthInvoke {
            contract: &c.contract_id,
            fn_name: "extend_deadline",
            args: (c.creator.clone(), c.id.clone(), 100u32).into_val(&c.env),
            sub_invokes: &[],
        },
    }]);
    assert!(client.try_cancel_bounty(&c.creator, &c.id).is_err());
}

/// Sub-invocation tree for `sponsor_bounty`: the sponsor must authorize both
/// the contract call and the nested token transfer into escrow.
#[test]
fn sponsor_bounty_requires_token_transfer_sub_invocation() {
    let c = setup();
    let client = MergeMintContractClient::new(&c.env, &c.contract_id);
    let sponsor = Address::generate(&c.env);
    StellarAssetClient::new(&c.env, &c.token).mint(&sponsor, &50);
    let amount: i128 = 50;

    // Parent call authorized without the nested transfer: must fail.
    c.env.mock_auths(&[MockAuth {
        address: &sponsor,
        invoke: &MockAuthInvoke {
            contract: &c.contract_id,
            fn_name: "sponsor_bounty",
            args: (sponsor.clone(), c.id.clone(), amount).into_val(&c.env),
            sub_invokes: &[],
        },
    }]);
    assert!(client.try_sponsor_bounty(&sponsor, &c.id, &amount).is_err());

    // Full tree: succeeds.
    c.env.mock_auths(&[MockAuth {
        address: &sponsor,
        invoke: &MockAuthInvoke {
            contract: &c.contract_id,
            fn_name: "sponsor_bounty",
            args: (sponsor.clone(), c.id.clone(), amount).into_val(&c.env),
            sub_invokes: &[MockAuthInvoke {
                contract: &c.token,
                fn_name: "transfer",
                args: (sponsor.clone(), c.contract_id.clone(), amount).into_val(&c.env),
                sub_invokes: &[],
            }],
        },
    }]);
    client.sponsor_bounty(&sponsor, &c.id, &amount);
}

/// Wrong-role callers are rejected even when properly authenticated.
#[test]
fn wrong_role_callers_are_rejected() {
    let c = setup();
    let client = MergeMintContractClient::new(&c.env, &c.contract_id);
    let stranger = Address::generate(&c.env);
    assert!(client.try_cancel_bounty(&stranger, &c.id).is_err());
    assert!(client.try_extend_deadline(&stranger, &c.id, &100).is_err());
    assert!(client.try_pause(&stranger).is_err());
    assert!(client.try_set_claim_window(&stranger, &5).is_err());
    assert!(client.try_raise_dispute(&stranger, &c.id).is_err());
}
