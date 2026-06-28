// SPDX-License-Identifier: MIT
#![cfg(test)]

use soroban_sdk::{
    testutils::Address as _,
    token::StellarAssetClient,
    Address, BytesN, Env, Symbol, Vec,
};

use crate::contract::MergeMintContract;
use crate::contract::MergeMintContractClient;

fn setup_test() -> (Env, Address, Address, Address) {
    let env = Env::default();
    let creator = Address::generate(&env);
    let contributor = Address::generate(&env);
    let verifier = Address::generate(&env);
    env.mock_all_auths();
    (env, creator, contributor, verifier)
}

/// Create a bounty with no tags and no deadline (convenience wrapper).
fn make_bounty(
    env: &Env,
    client: &MergeMintContractClient,
    creator: &Address,
    tag: &str,
) -> BytesN<32> {
    client.create_bounty(
        creator,
        &Symbol::new(env, tag),
        &Symbol::new(env, "desc"),
        &1000,
        &Address::generate(env),
        &0u32,
        &None,
        &Vec::new(env),
    )
}

// ===========================================================================
// Core lifecycle tests
// ===========================================================================

#[test]
fn test_create_bounty() {
    let (env, creator, _contributor, _verifier) = setup_test();
    let contract_id = env.register_contract(None, MergeMintContract);
    let client = MergeMintContractClient::new(&env, &contract_id);

    let title = Symbol::new(&env, "test_bounty");
    let description = Symbol::new(&env, "Test_bounty_desc");
    let reward_amount: i128 = 1000;
    let reward_token = Address::generate(&env);

    let bounty_id = client.create_bounty(
        &creator,
        &title,
        &description,
        &reward_amount,
        &reward_token,
        &0u32,
        &None,
        &Vec::new(&env),
    );

    let bounty = client.get_bounty(&bounty_id).unwrap();
    assert_eq!(bounty.reward_amount, reward_amount);
    assert_eq!(bounty.creator, creator);
    assert!(bounty.assignees.is_empty());
    assert!(bounty.tags.is_empty());

    let meta = client.get_bounty_meta(&bounty_id).unwrap();
    assert_eq!(meta.title, title);
    assert_eq!(meta.description, description);
}

#[test]
fn test_claim_bounty() {
    let (env, creator, contributor, _verifier) = setup_test();
    let contract_id = env.register_contract(None, MergeMintContract);
    let client = MergeMintContractClient::new(&env, &contract_id);

    let bounty_id = make_bounty(&env, &client, &creator, "bounty_1");
    client.claim_bounty(&contributor, &bounty_id);
    let bounty = client.get_bounty(&bounty_id).unwrap();
    let (assignee_addr, share) = bounty.assignees.get(0).unwrap();
    assert_eq!(assignee_addr, contributor);
    assert_eq!(share, 10_000u32);
}

#[test]
fn test_bounty_count() {
    let (env, creator, _contributor, _verifier) = setup_test();
    let contract_id = env.register_contract(None, MergeMintContract);
    let client = MergeMintContractClient::new(&env, &contract_id);

    assert_eq!(client.get_bounty_count(), 0);
    let reward_token = Address::generate(&env);
    client.create_bounty(
        &creator,
        &Symbol::new(&env, "bounty_a"),
        &Symbol::new(&env, "desc_a"),
        &100,
        &reward_token,
        &0u32,
        &None,
        &Vec::new(&env),
    );
    assert_eq!(client.get_bounty_count(), 1);
    client.create_bounty(
        &creator,
        &Symbol::new(&env, "bounty_b"),
        &Symbol::new(&env, "desc_b"),
        &200,
        &reward_token,
        &0u32,
        &None,
        &Vec::new(&env),
    );
    assert_eq!(client.get_bounty_count(), 2);
}

#[test]
fn test_complete_bounty_updates_contributor() {
    let (env, creator, contributor, _verifier) = setup_test();
    let contract_id = env.register_contract(None, MergeMintContract);
    let client = MergeMintContractClient::new(&env, &contract_id);

    let bounty_id = make_bounty(&env, &client, &creator, "bounty_c");
    client.claim_bounty(&contributor, &bounty_id);
    let bounty = client.get_bounty(&bounty_id).unwrap();
    let (assignee_addr, _) = bounty.assignees.get(0).unwrap();
    assert_eq!(assignee_addr, contributor);
}

// ===========================================================================
// Issue: bounty ID uniqueness across sequential creates
// ===========================================================================

#[test]
fn test_bounty_id_uniqueness_sequential() {
    let (env, creator, _contributor, _verifier) = setup_test();
    let contract_id = env.register_contract(None, MergeMintContract);
    let client = MergeMintContractClient::new(&env, &contract_id);

    let mut seen_ids = Vec::<BytesN<32>>::new(&env);
    for i in 0..20u64 {
        let bounty_id = make_bounty(&env, &client, &creator, "bounty");
        for j in 0..seen_ids.len() {
            assert_ne!(seen_ids.get(j).unwrap(), bounty_id, "Duplicate ID at {}", i);
        }
        seen_ids.push_back(bounty_id.clone());
        let id_bytes = bounty_id.to_array();
        let counter_bytes: [u8; 8] = id_bytes[24..32].try_into().unwrap();
        let encoded_count = u64::from_be_bytes(counter_bytes);
        assert_eq!(encoded_count, i);
    }
}

// ===========================================================================
// Issue: complete_bounty panics when no assignee
// ===========================================================================

#[test]
#[should_panic(expected = "bounty has no assignee")]
fn test_complete_bounty_no_assignee_panics() {
    let (env, creator, _contributor, verifier) = setup_test();
    let contract_id = env.register_contract(None, MergeMintContract);
    let client = MergeMintContractClient::new(&env, &contract_id);

    let bounty_id = make_bounty(&env, &client, &creator, "unclaimed");
    assert!(client.get_bounty(&bounty_id).unwrap().assignees.is_empty());
    client.complete_bounty(&verifier, &bounty_id);
}

// ===========================================================================
// Issue: contributor reputation accumulates correctly
// ===========================================================================

#[test]
fn test_contributor_reputation_accumulation() {
    let (env, creator, contributor, verifier) = setup_test();
    let contract_id = env.register_contract(None, MergeMintContract);
    let client = MergeMintContractClient::new(&env, &contract_id);

    // Set up a real token so complete_bounty can transfer.
    let token_admin = Address::generate(&env);
    let reward_token = env.register_stellar_asset_contract_v2(token_admin.clone()).address();
    let token_sac = StellarAssetClient::new(&env, &reward_token);
    // Mint enough for all 3 completions: 1000 + 1500 + 2000 = 4500.
    token_sac.mint(&verifier, &4500);

    assert!(client.get_contributor(&contributor).is_none());

    let mut expected_reputation: u32 = 0;
    let mut expected_earned: i128 = 0;
    let mut expected_count: u32 = 0;

    for i in 0..3u32 {
        let reward: i128 = 1000 + (i as i128) * 500;
        let bounty_id = client.create_bounty(
            &creator,
            &Symbol::new(&env, "rep_b"),
            &Symbol::new(&env, "rep_d"),
            &reward,
            &reward_token,
            &0u32,
            &None,
            &Vec::new(&env),
        );
        client.claim_bounty(&contributor, &bounty_id);
        client.complete_bounty(&verifier, &bounty_id);

        expected_reputation += 10;
        expected_earned += reward;
        expected_count += 1;

        let data = client.get_contributor(&contributor).unwrap();
        assert_eq!(data.reputation, expected_reputation);
        assert_eq!(data.total_earned, expected_earned);
        assert_eq!(data.contribution_count, expected_count);
        assert_eq!(data.address, contributor);
    }

    let final_data = client.get_contributor(&contributor).unwrap();
    assert_eq!(final_data.reputation, 30);
    assert_eq!(final_data.total_earned, 1000 + 1500 + 2000);
    assert_eq!(final_data.contribution_count, 3);
}

// ===========================================================================
// Issue: bounty tags — feat/bounty-tags
// ===========================================================================

#[test]
fn test_create_bounty_with_tags_stored_and_retrieved() {
    let (env, creator, _contributor, _verifier) = setup_test();
    let contract_id = env.register_contract(None, MergeMintContract);
    let client = MergeMintContractClient::new(&env, &contract_id);

    let mut tags = Vec::new(&env);
    tags.push_back(Symbol::new(&env, "bug"));
    tags.push_back(Symbol::new(&env, "rust"));

    let bounty_id = client.create_bounty(
        &creator,
        &Symbol::new(&env, "tagged"),
        &Symbol::new(&env, "desc"),
        &500,
        &Address::generate(&env),
        &0u32,
        &None,
        &tags,
    );

    let bounty = client.get_bounty(&bounty_id).unwrap();
    assert_eq!(bounty.tags.len(), 2);
    assert_eq!(bounty.tags.get(0).unwrap(), Symbol::new(&env, "bug"));
    assert_eq!(bounty.tags.get(1).unwrap(), Symbol::new(&env, "rust"));
}

#[test]
fn test_create_bounty_empty_tags_valid() {
    let (env, creator, _contributor, _verifier) = setup_test();
    let contract_id = env.register_contract(None, MergeMintContract);
    let client = MergeMintContractClient::new(&env, &contract_id);

    let bounty_id = make_bounty(&env, &client, &creator, "no_tags");
    let bounty = client.get_bounty(&bounty_id).unwrap();
    assert!(bounty.tags.is_empty());
}

#[test]
fn test_create_bounty_five_tags_valid() {
    let (env, creator, _contributor, _verifier) = setup_test();
    let contract_id = env.register_contract(None, MergeMintContract);
    let client = MergeMintContractClient::new(&env, &contract_id);

    let mut tags = Vec::new(&env);
    tags.push_back(Symbol::new(&env, "a"));
    tags.push_back(Symbol::new(&env, "b"));
    tags.push_back(Symbol::new(&env, "c"));
    tags.push_back(Symbol::new(&env, "d"));
    tags.push_back(Symbol::new(&env, "e"));

    let bounty_id = client.create_bounty(
        &creator,
        &Symbol::new(&env, "max_tags"),
        &Symbol::new(&env, "desc"),
        &500,
        &Address::generate(&env),
        &0u32,
        &None,
        &tags,
    );
    let bounty = client.get_bounty(&bounty_id).unwrap();
    assert_eq!(bounty.tags.len(), 5);
}

#[test]
#[should_panic(expected = "too many tags")]
fn test_create_bounty_too_many_tags_panics() {
    let (env, creator, _contributor, _verifier) = setup_test();
    let contract_id = env.register_contract(None, MergeMintContract);
    let client = MergeMintContractClient::new(&env, &contract_id);

    let mut tags = Vec::new(&env);
    for _ in 0..6 {
        tags.push_back(Symbol::new(&env, "tag"));
    }

    client.create_bounty(
        &creator,
        &Symbol::new(&env, "too_many"),
        &Symbol::new(&env, "desc"),
        &500,
        &Address::generate(&env),
        &0u32,
        &None,
        &tags,
    );
}

// ===========================================================================
// Issue: get_bounties_by_creator — feat/bounties-by-creator
// ===========================================================================

#[test]
fn test_get_bounties_by_creator_returns_all() {
    let (env, creator, _contributor, _verifier) = setup_test();
    let contract_id = env.register_contract(None, MergeMintContract);
    let client = MergeMintContractClient::new(&env, &contract_id);

    let id1 = make_bounty(&env, &client, &creator, "b1");
    let id2 = make_bounty(&env, &client, &creator, "b2");
    let id3 = make_bounty(&env, &client, &creator, "b3");

    let ids = client.get_bounties_by_creator(&creator);
    assert_eq!(ids.len(), 3);
    assert_eq!(ids.get(0).unwrap(), id1);
    assert_eq!(ids.get(1).unwrap(), id2);
    assert_eq!(ids.get(2).unwrap(), id3);
}

#[test]
fn test_get_bounties_by_creator_independent_lists() {
    let (env, creator, creator2, _verifier) = setup_test();
    let contract_id = env.register_contract(None, MergeMintContract);
    let client = MergeMintContractClient::new(&env, &contract_id);

    make_bounty(&env, &client, &creator, "c1_b1");
    make_bounty(&env, &client, &creator, "c1_b2");
    make_bounty(&env, &client, &creator2, "c2_b1");

    let ids1 = client.get_bounties_by_creator(&creator);
    let ids2 = client.get_bounties_by_creator(&creator2);

    assert_eq!(ids1.len(), 2);
    assert_eq!(ids2.len(), 1);
}

#[test]
fn test_get_bounties_by_creator_empty_for_new_address() {
    let (env, _creator, _contributor, _verifier) = setup_test();
    let contract_id = env.register_contract(None, MergeMintContract);
    let client = MergeMintContractClient::new(&env, &contract_id);

    let stranger = Address::generate(&env);
    let ids = client.get_bounties_by_creator(&stranger);
    assert_eq!(ids.len(), 0);
}

// ===========================================================================
// Issue: dispute mechanism — complete_bounty blocked when disputed
// ===========================================================================

#[test]
fn test_raise_dispute_creator() {
    let (env, creator, contributor, _verifier) = setup_test();
    let contract_id = env.register_contract(None, MergeMintContract);
    let client = MergeMintContractClient::new(&env, &contract_id);

    let bounty_id = make_bounty(&env, &client, &creator, "dispute_b1");
    client.claim_bounty(&contributor, &bounty_id);
    client.raise_dispute(&creator, &bounty_id);

    let bounty = client.get_bounty(&bounty_id).unwrap();
    assert_eq!(bounty.status, Symbol::new(&env, "disputed"));
}

#[test]
fn test_raise_dispute_assignee() {
    let (env, creator, contributor, _verifier) = setup_test();
    let contract_id = env.register_contract(None, MergeMintContract);
    let client = MergeMintContractClient::new(&env, &contract_id);

    let bounty_id = make_bounty(&env, &client, &creator, "dispute_b2");
    client.claim_bounty(&contributor, &bounty_id);
    client.raise_dispute(&contributor, &bounty_id);

    let bounty = client.get_bounty(&bounty_id).unwrap();
    assert_eq!(bounty.status, Symbol::new(&env, "disputed"));
}

#[test]
#[should_panic(expected = "only creator or assignee can raise dispute")]
fn test_raise_dispute_third_party_fails() {
    let (env, creator, contributor, _verifier) = setup_test();
    let contract_id = env.register_contract(None, MergeMintContract);
    let client = MergeMintContractClient::new(&env, &contract_id);

    let third_party = Address::generate(&env);
    let bounty_id = make_bounty(&env, &client, &creator, "dispute_b3");
    client.claim_bounty(&contributor, &bounty_id);
    client.raise_dispute(&third_party, &bounty_id);
}

#[test]
#[should_panic(expected = "bounty is disputed")]
fn test_complete_bounty_fails_when_disputed() {
    let (env, creator, contributor, verifier) = setup_test();
    let contract_id = env.register_contract(None, MergeMintContract);
    let client = MergeMintContractClient::new(&env, &contract_id);

    let bounty_id = make_bounty(&env, &client, &creator, "dispute_block");
    client.claim_bounty(&contributor, &bounty_id);
    client.raise_dispute(&creator, &bounty_id);
    client.complete_bounty(&verifier, &bounty_id);
}

// ===========================================================================
// Issue: ContractError enum — error strings are consistent
// ===========================================================================

#[test]
#[should_panic(expected = "bounty not found")]
fn test_error_bounty_not_found() {
    let (env, _creator, _contributor, verifier) = setup_test();
    let contract_id = env.register_contract(None, MergeMintContract);
    let client = MergeMintContractClient::new(&env, &contract_id);

    let fake_id = BytesN::from_array(&env, &[0u8; 32]);
    client.complete_bounty(&verifier, &fake_id);
}

#[test]
#[should_panic(expected = "bounty already assigned")]
fn test_error_bounty_already_assigned() {
    let (env, creator, contributor, _verifier) = setup_test();
    let contract_id = env.register_contract(None, MergeMintContract);
    let client = MergeMintContractClient::new(&env, &contract_id);

    let bounty_id = make_bounty(&env, &client, &creator, "assigned_b");
    client.claim_bounty(&contributor, &bounty_id);

    let contributor2 = Address::generate(&env);
    client.claim_bounty(&contributor2, &bounty_id);
}

#[test]
#[should_panic(expected = "contributor already has an active claim")]
fn test_error_contributor_has_active_claim() {
    let (env, creator, contributor, _verifier) = setup_test();
    let contract_id = env.register_contract(None, MergeMintContract);
    let client = MergeMintContractClient::new(&env, &contract_id);

    let id1 = make_bounty(&env, &client, &creator, "act_c1");
    let id2 = make_bounty(&env, &client, &creator, "act_c2");

    client.claim_bounty(&contributor, &id1);
    client.claim_bounty(&contributor, &id2);
}

// ===========================================================================
// Status index tests
// ===========================================================================

#[test]
fn test_status_index_open_on_create() {
    let (env, creator, _contributor, _verifier) = setup_test();
    let contract_id = env.register_contract(None, MergeMintContract);
    let client = MergeMintContractClient::new(&env, &contract_id);

    let id = make_bounty(&env, &client, &creator, "bounty_x");
    let open_ids = client.get_bounties_by_status(&Symbol::new(&env, "open"));
    assert_eq!(open_ids.len(), 1);
    assert_eq!(open_ids.get(0).unwrap(), id);
}

#[test]
fn test_status_index_moves_on_claim() {
    let (env, creator, contributor, _verifier) = setup_test();
    let contract_id = env.register_contract(None, MergeMintContract);
    let client = MergeMintContractClient::new(&env, &contract_id);

    let id = make_bounty(&env, &client, &creator, "bounty_y");
    client.claim_bounty(&contributor, &id);

    assert_eq!(client.get_bounties_by_status(&Symbol::new(&env, "open")).len(), 0);
    assert_eq!(client.get_bounties_by_status(&Symbol::new(&env, "in_progress")).len(), 1);
}

#[test]
fn test_status_index_moves_on_cancel() {
    let (env, creator, _contributor, _verifier) = setup_test();
    let contract_id = env.register_contract(None, MergeMintContract);
    let client = MergeMintContractClient::new(&env, &contract_id);

    let id = make_bounty(&env, &client, &creator, "bounty_z");
    client.cancel_bounty(&creator, &id);

    assert_eq!(client.get_bounties_by_status(&Symbol::new(&env, "open")).len(), 0);
    assert_eq!(client.get_bounties_by_status(&Symbol::new(&env, "cancelled")).len(), 1);
}

// ===========================================================================
// Contributor metadata tests
// ===========================================================================

#[test]
fn test_update_contributor_metadata_stores_value() {
    let (env, _creator, contributor, _verifier) = setup_test();
    let contract_id = env.register_contract(None, MergeMintContract);
    let client = MergeMintContractClient::new(&env, &contract_id);

    let uri = Symbol::new(&env, "ipfs_hash_123");
    client.update_contributor_metadata(&contributor, &uri);

    let data = client.get_contributor(&contributor).unwrap();
    assert_eq!(data.metadata.unwrap(), uri);
}

#[test]
fn test_update_contributor_metadata_overwrite() {
    let (env, _creator, contributor, _verifier) = setup_test();
    let contract_id = env.register_contract(None, MergeMintContract);
    let client = MergeMintContractClient::new(&env, &contract_id);

    client.update_contributor_metadata(&contributor, &Symbol::new(&env, "old_uri"));
    client.update_contributor_metadata(&contributor, &Symbol::new(&env, "new_uri"));

    let data = client.get_contributor(&contributor).unwrap();
    assert_eq!(data.metadata.unwrap(), Symbol::new(&env, "new_uri"));
}
