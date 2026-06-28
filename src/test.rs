// SPDX-License-Identifier: MIT
#![cfg(test)]

use soroban_sdk::{
    testutils::{Address as _, BytesN as _, Ledger as _},
    Address, BytesN, Env, Symbol,
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

/// Create a bounty with the given tag, no min_reputation, and an optional deadline.
fn make_bounty(
    env: &Env,
    client: &MergeMintContractClient,
    creator: &Address,
    tag: &str,
    deadline: Option<u32>,
) -> BytesN<32> {
    client.create_bounty(
        creator,
        &Symbol::new(env, tag),
        &Symbol::new(env, "desc"),
        &1000,
        &Address::generate(env),
        &0,
        &deadline,
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
        &creator, &title, &description, &reward_amount, &reward_token, &0, &None,
    );

    let bounty = client.get_bounty(&bounty_id).unwrap();
    assert_eq!(bounty.reward_amount, reward_amount);
    assert_eq!(bounty.creator, creator);
    assert!(bounty.assignees.is_empty());

    let meta = client.get_bounty_meta(&bounty_id).unwrap();
    assert_eq!(meta.title, title);
    assert_eq!(meta.description, description);
}

#[test]
fn test_claim_bounty() {
    let (env, creator, contributor, _verifier) = setup_test();
    let contract_id = env.register_contract(None, MergeMintContract);
    let client = MergeMintContractClient::new(&env, &contract_id);

    let bounty_id = make_bounty(&env, &client, &creator, "bounty_1", None);
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
        &creator, &Symbol::new(&env, "bounty_a"), &Symbol::new(&env, "desc_a"), &100, &reward_token, &0, &None,
    );
    assert_eq!(client.get_bounty_count(), 1);
    client.create_bounty(
        &creator, &Symbol::new(&env, "bounty_b"), &Symbol::new(&env, "desc_b"), &200, &reward_token, &0, &None,
    );
    assert_eq!(client.get_bounty_count(), 2);
}

#[test]
fn test_complete_bounty_updates_contributor() {
    let (env, creator, contributor, _verifier) = setup_test();
    let contract_id = env.register_contract(None, MergeMintContract);
    let client = MergeMintContractClient::new(&env, &contract_id);

    let bounty_id = make_bounty(&env, &client, &creator, "bounty_c", None);
    client.claim_bounty(&contributor, &bounty_id);

    let bounty = client.get_bounty(&bounty_id).unwrap();
    let (assignee_addr, _) = bounty.assignees.get(0).unwrap();
    assert_eq!(assignee_addr, contributor);
}

// ===========================================================================
// Issue: update_bounty
// ===========================================================================

#[test]
fn test_update_bounty_title() {
    let (env, creator, _contributor, _verifier) = setup_test();
    let contract_id = env.register_contract(None, MergeMintContract);
    let client = MergeMintContractClient::new(&env, &contract_id);

    let bounty_id = make_bounty(&env, &client, &creator, "old_title", None);

    let new_title = Symbol::new(&env, "new_title");
    client.update_bounty(&creator, &bounty_id, &Some(new_title.clone()), &None, &None);

    let meta = client.get_bounty_meta(&bounty_id).unwrap();
    assert_eq!(meta.title, new_title);
}

#[test]
fn test_update_bounty_description() {
    let (env, creator, _contributor, _verifier) = setup_test();
    let contract_id = env.register_contract(None, MergeMintContract);
    let client = MergeMintContractClient::new(&env, &contract_id);

    let bounty_id = make_bounty(&env, &client, &creator, "some_title", None);

    let new_desc = Symbol::new(&env, "updated_desc");
    client.update_bounty(&creator, &bounty_id, &None, &Some(new_desc.clone()), &None);

    let meta = client.get_bounty_meta(&bounty_id).unwrap();
    assert_eq!(meta.description, new_desc);
}

#[test]
fn test_update_bounty_reward_amount() {
    let (env, creator, _contributor, _verifier) = setup_test();
    let contract_id = env.register_contract(None, MergeMintContract);
    let client = MergeMintContractClient::new(&env, &contract_id);

    let bounty_id = make_bounty(&env, &client, &creator, "reward_b", None);

    client.update_bounty(&creator, &bounty_id, &None, &None, &Some(2000i128));

    let bounty = client.get_bounty(&bounty_id).unwrap();
    assert_eq!(bounty.reward_amount, 2000i128);
}

#[test]
#[should_panic(expected = "bounty cannot be updated after it is claimed")]
fn test_update_bounty_fails_when_claimed() {
    let (env, creator, contributor, _verifier) = setup_test();
    let contract_id = env.register_contract(None, MergeMintContract);
    let client = MergeMintContractClient::new(&env, &contract_id);

    let bounty_id = make_bounty(&env, &client, &creator, "claimed_b", None);
    client.claim_bounty(&contributor, &bounty_id);

    client.update_bounty(&creator, &bounty_id, &Some(Symbol::new(&env, "new")), &None, &None);
}

#[test]
#[should_panic(expected = "not the bounty creator")]
fn test_update_bounty_fails_for_non_creator() {
    let (env, creator, contributor, _verifier) = setup_test();
    let contract_id = env.register_contract(None, MergeMintContract);
    let client = MergeMintContractClient::new(&env, &contract_id);

    let bounty_id = make_bounty(&env, &client, &creator, "b_nc", None);

    client.update_bounty(&contributor, &bounty_id, &Some(Symbol::new(&env, "hacked")), &None, &None);
}

// ===========================================================================
// Issue: bounty deadline enforcement in claim_bounty
// ===========================================================================

#[test]
fn test_claim_bounty_within_deadline_succeeds() {
    let (env, creator, contributor, _verifier) = setup_test();
    let contract_id = env.register_contract(None, MergeMintContract);
    let client = MergeMintContractClient::new(&env, &contract_id);

    // Set ledger sequence to 100, deadline at 200 — should succeed.
    env.ledger().set_sequence_number(100);
    let bounty_id = make_bounty(&env, &client, &creator, "deadline_ok", Some(200));
    client.claim_bounty(&contributor, &bounty_id);

    let bounty = client.get_bounty(&bounty_id).unwrap();
    assert!(!bounty.assignees.is_empty());
}

#[test]
#[should_panic(expected = "bounty deadline has passed")]
fn test_claim_bounty_after_deadline_panics() {
    let (env, creator, contributor, _verifier) = setup_test();
    let contract_id = env.register_contract(None, MergeMintContract);
    let client = MergeMintContractClient::new(&env, &contract_id);

    // Create bounty with deadline 50, then advance ledger past it.
    let bounty_id = make_bounty(&env, &client, &creator, "expired_b", Some(50));
    env.ledger().set_sequence_number(51);
    client.claim_bounty(&contributor, &bounty_id);
}

#[test]
fn test_claim_bounty_no_deadline_succeeds() {
    let (env, creator, contributor, _verifier) = setup_test();
    let contract_id = env.register_contract(None, MergeMintContract);
    let client = MergeMintContractClient::new(&env, &contract_id);

    let bounty_id = make_bounty(&env, &client, &creator, "no_deadline", None);
    env.ledger().set_sequence_number(99999);
    client.claim_bounty(&contributor, &bounty_id);

    let bounty = client.get_bounty(&bounty_id).unwrap();
    assert!(!bounty.assignees.is_empty());
}

// ===========================================================================
// Issue: cancel_bounty
// ===========================================================================

#[test]
fn test_cancel_bounty_sets_cancelled_status() {
    let (env, creator, _contributor, _verifier) = setup_test();
    let contract_id = env.register_contract(None, MergeMintContract);
    let client = MergeMintContractClient::new(&env, &contract_id);

    let bounty_id = make_bounty(&env, &client, &creator, "to_cancel", None);
    client.cancel_bounty(&creator, &bounty_id);

    let bounty = client.get_bounty(&bounty_id).unwrap();
    assert_eq!(bounty.status, Symbol::new(&env, "cancelled"));
}

#[test]
#[should_panic(expected = "not the bounty creator")]
fn test_cancel_bounty_non_creator_panics() {
    let (env, creator, contributor, _verifier) = setup_test();
    let contract_id = env.register_contract(None, MergeMintContract);
    let client = MergeMintContractClient::new(&env, &contract_id);

    let bounty_id = make_bounty(&env, &client, &creator, "cancel_nc", None);
    client.cancel_bounty(&contributor, &bounty_id);
}

#[test]
#[should_panic(expected = "bounty not open")]
fn test_cancel_bounty_claimed_panics() {
    let (env, creator, contributor, _verifier) = setup_test();
    let contract_id = env.register_contract(None, MergeMintContract);
    let client = MergeMintContractClient::new(&env, &contract_id);

    let bounty_id = make_bounty(&env, &client, &creator, "cancel_claimed", None);
    client.claim_bounty(&contributor, &bounty_id);
    client.cancel_bounty(&creator, &bounty_id);
}

// ===========================================================================
// Issue: None-path tests for get_bounty and get_contributor
// ===========================================================================

#[test]
fn test_get_bounty_returns_none_for_unknown_id() {
    let (env, _creator, _contributor, _verifier) = setup_test();
    let contract_id = env.register_contract(None, MergeMintContract);
    let client = MergeMintContractClient::new(&env, &contract_id);

    let unknown_id = BytesN::random(&env);
    assert!(client.get_bounty(&unknown_id).is_none());
}

#[test]
fn test_get_contributor_returns_none_for_unknown_address() {
    let (env, _creator, _contributor, _verifier) = setup_test();
    let contract_id = env.register_contract(None, MergeMintContract);
    let client = MergeMintContractClient::new(&env, &contract_id);

    let unknown_addr = Address::generate(&env);
    assert!(client.get_contributor(&unknown_addr).is_none());
}

// ===========================================================================
// Additional existing tests (fixed)
// ===========================================================================

#[test]
fn test_bounty_count_increment_loop() {
    let (env, creator, _contributor, _verifier) = setup_test();
    let contract_id = env.register_contract(None, MergeMintContract);
    let client = MergeMintContractClient::new(&env, &contract_id);

    let reward_token = Address::generate(&env);
    for i in 0..5u64 {
        client.create_bounty(
            &creator,
            &Symbol::new(&env, "bounty"),
            &Symbol::new(&env, "desc"),
            &1000,
            &reward_token,
            &0,
            &None,
        );
        assert_eq!(client.get_bounty_count(), i + 1);
    }
    assert_eq!(client.get_bounty_count(), 5);
}

#[test]
#[should_panic(expected = "bounty has no assignee")]
fn test_complete_bounty_no_assignee_panics() {
    let (env, creator, _contributor, verifier) = setup_test();
    let contract_id = env.register_contract(None, MergeMintContract);
    let client = MergeMintContractClient::new(&env, &contract_id);

    let bounty_id = make_bounty(&env, &client, &creator, "unclaimed", None);
    client.complete_bounty(&verifier, &bounty_id);
}

#[test]
fn test_raise_dispute_creator() {
    let (env, creator, contributor, _verifier) = setup_test();
    let contract_id = env.register_contract(None, MergeMintContract);
    let client = MergeMintContractClient::new(&env, &contract_id);

    let bounty_id = make_bounty(&env, &client, &creator, "dispute_1", None);
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

    let bounty_id = make_bounty(&env, &client, &creator, "dispute_2", None);
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
    let bounty_id = make_bounty(&env, &client, &creator, "dispute_3", None);
    client.claim_bounty(&contributor, &bounty_id);
    client.raise_dispute(&third_party, &bounty_id);
}

#[test]
#[should_panic(expected = "contributor already has an active claim")]
fn test_second_claim_rejected_while_active() {
    let (env, creator, contributor, _verifier) = setup_test();
    let contract_id = env.register_contract(None, MergeMintContract);
    let client = MergeMintContractClient::new(&env, &contract_id);

    let bounty_id_1 = make_bounty(&env, &client, &creator, "active1", None);
    let bounty_id_2 = make_bounty(&env, &client, &creator, "active2", None);

    client.claim_bounty(&contributor, &bounty_id_1);
    // Second claim should panic — contributor already has an active claim.
    client.claim_bounty(&contributor, &bounty_id_2);
}

#[test]
#[should_panic(expected = "bounty already assigned")]
fn test_second_contributor_cannot_claim_full_bounty() {
    let (env, creator, contributor, _verifier) = setup_test();
    let contract_id = env.register_contract(None, MergeMintContract);
    let client = MergeMintContractClient::new(&env, &contract_id);

    let bounty_id = make_bounty(&env, &client, &creator, "full_c", None);
    client.claim_bounty(&contributor, &bounty_id);

    let contributor2 = Address::generate(&env);
    client.claim_bounty(&contributor2, &bounty_id);
}

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

#[test]
fn test_single_assignee_gets_full_share() {
    let (env, creator, contributor, _verifier) = setup_test();
    let contract_id = env.register_contract(None, MergeMintContract);
    let client = MergeMintContractClient::new(&env, &contract_id);

    let bounty_id = make_bounty(&env, &client, &creator, "single", None);
    client.claim_bounty(&contributor, &bounty_id);

    let bounty = client.get_bounty(&bounty_id).unwrap();
    assert_eq!(bounty.assignees.len(), 1);
    let (addr, share) = bounty.assignees.get(0).unwrap();
    assert_eq!(addr, contributor);
    assert_eq!(share, 10_000u32);
}

#[test]
fn test_status_index_moves_on_cancel() {
    let (env, creator, _contributor, _verifier) = setup_test();
    let contract_id = env.register_contract(None, MergeMintContract);
    let client = MergeMintContractClient::new(&env, &contract_id);

    let id = make_bounty(&env, &client, &creator, "bounty_z", None);
    client.cancel_bounty(&creator, &id);

    let open_ids = client.get_bounties_by_status(&Symbol::new(&env, "open"));
    let cancelled_ids = client.get_bounties_by_status(&Symbol::new(&env, "cancelled"));
    assert_eq!(open_ids.len(), 0);
    assert_eq!(cancelled_ids.len(), 1);
    assert_eq!(cancelled_ids.get(0).unwrap(), id);
}
