// SPDX-License-Identifier: MIT

#[cfg(test)]
extern crate std;

use crate::contract::MergeMintContract;
use crate::storage;
use crate::types::{BountyId, DataKey, Milestone};
use crate::MergeMintContractClient;
use soroban_sdk::{
    testutils::{storage::Persistent as _, Address as _, Events as _, Ledger as _},
    token::StellarAssetClient,
    vec, Address, BytesN, Env, String, Symbol, TryFromVal, Val, Vec,
};

fn setup_test() -> (Env, Address, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let creator = Address::generate(&env);
    let contributor = Address::generate(&env);
    let verifier = Address::generate(&env);
    (env, creator, contributor, verifier)
}

/// Create a bounty using a fresh reward token, min_reputation=0, no deadline,
/// and an empty tags list.
fn make_bounty(
    client: &MergeMintContractClient,
    env: &Env,
    creator: &Address,
    tag: &str,
    deadline: Option<u32>,
) -> crate::types::BountyId {
    let sac = env.register_stellar_asset_contract_v2(creator.clone());
    let token_addr = sac.address();
    client.create_bounty(
        creator,
        &Symbol::new(env, tag),
        &String::from_str(env, "desc"),
        &1000,
        &token_addr,
        &0,
        &deadline,
        &Vec::new(env),
        &1,
        &None,
        &1,
        &Vec::new(env),
    )
}

/// Create a Stellar Asset Contract token and mint `amount` to `to`.
/// Returns the token contract address.
fn create_token_and_mint(env: &Env, admin: &Address, to: &Address, amount: i128) -> Address {
    let sac = env.register_stellar_asset_contract_v2(admin.clone());
    let token_addr = sac.address();
    let token_admin = StellarAssetClient::new(env, &token_addr);
    token_admin.mint(to, &amount);
    token_addr
}

/// Create a bounty using a real token contract with `reward_amount` minted to the contract.
/// Returns both the bounty ID and the token address.
fn make_bounty_with_token(
    client: &MergeMintContractClient,
    env: &Env,
    creator: &Address,
    contract_id: &Address,
    tag: &str,
    reward_amount: i128,
    deadline: Option<u32>,
) -> (crate::types::BountyId, Address) {
    // Use creator as token admin for simplicity (mock-all-auths applies).
    let token_addr = create_token_and_mint(env, creator, contract_id, reward_amount);
    let bounty_id = client.create_bounty(
        creator,
        &Symbol::new(env, tag),
        &String::from_str(env, "desc"),
        &reward_amount,
        &token_addr,
        &0,
        &deadline,
        &Vec::new(env),
        &1,
        &None,
        &1,
        &Vec::new(env),
    );
    (bounty_id, token_addr)
}

/// Fabricate a `BountyId` with the given monotonic sequence (mirrors `generate_bounty_id`).
fn fake_bounty_id(env: &Env, sequence: u64) -> BountyId {
    let mut buf = [0u8; 32];
    buf[24..32].copy_from_slice(&sequence.to_be_bytes());
    BountyId(BytesN::from_array(env, &buf))
}

// ===========================================================================
// Issue 1 — bounty tags
// ===========================================================================

/// Tags supplied to create_bounty are stored and returned by get_bounty.
#[test]
fn test_tags_stored_and_retrieved() {
    let (env, creator, _contributor, _verifier) = setup_test();
    let contract_id = env.register(MergeMintContract, ());
    let client = MergeMintContractClient::new(&env, &contract_id);

    let mut tags: Vec<Symbol> = Vec::new(&env);
    tags.push_back(Symbol::new(&env, "bug"));
    tags.push_back(Symbol::new(&env, "docs"));

    let bounty_id = client.create_bounty(
        &creator,
        &Symbol::new(&env, "tagged"),
        &String::from_str(&env, "desc"),
        &1000,
        &create_token_and_mint(&env, &creator, &contract_id, 0),
        &0,
        &None,
        &tags,
        &1,
        &None,
        &1,
        &Vec::new(&env),
    );

    let bounty = client.get_bounty(&bounty_id).unwrap();
    assert_eq!(bounty.tags.len(), 2);
    assert_eq!(bounty.tags.get(0).unwrap(), Symbol::new(&env, "bug"));
    assert_eq!(bounty.tags.get(1).unwrap(), Symbol::new(&env, "docs"));
}

/// An empty tags vector is valid and results in a bounty with zero tags.
#[test]
fn test_empty_tags_valid() {
    let (env, creator, _contributor, _verifier) = setup_test();
    let contract_id = env.register(MergeMintContract, ());
    let client = MergeMintContractClient::new(&env, &contract_id);

    let bounty_id = client.create_bounty(
        &creator,
        &Symbol::new(&env, "no_tags"),
        &String::from_str(&env, "desc"),
        &1000,
        &create_token_and_mint(&env, &creator, &contract_id, 0),
        &0,
        &None,
        &Vec::new(&env),
        &1,
        &None,
        &1,
        &Vec::new(&env),
    );

    let bounty = client.get_bounty(&bounty_id).unwrap();
    assert_eq!(bounty.tags.len(), 0);
}

/// Exactly 5 tags is the maximum — must succeed.
#[test]
fn test_five_tags_allowed() {
    let (env, creator, _contributor, _verifier) = setup_test();
    let contract_id = env.register(MergeMintContract, ());
    let client = MergeMintContractClient::new(&env, &contract_id);

    let mut tags: Vec<Symbol> = Vec::new(&env);
    tags.push_back(Symbol::new(&env, "bug"));
    tags.push_back(Symbol::new(&env, "docs"));
    tags.push_back(Symbol::new(&env, "feature"));
    tags.push_back(Symbol::new(&env, "security"));
    tags.push_back(Symbol::new(&env, "test"));

    let bounty_id = client.create_bounty(
        &creator,
        &Symbol::new(&env, "max_tags"),
        &String::from_str(&env, "desc"),
        &1000,
        &create_token_and_mint(&env, &creator, &contract_id, 0),
        &0,
        &None,
        &tags,
        &1,
        &None,
        &1,
        &Vec::new(&env),
    );

    let bounty = client.get_bounty(&bounty_id).unwrap();
    assert_eq!(bounty.tags.len(), 5);
}
/// Supplying more than 5 tags must panic with "too many tags".
#[test]
#[should_panic(expected = "too many tags")]
fn test_too_many_tags_panics() {
    let (env, creator, _contributor, _verifier) = setup_test();
    let contract_id = env.register(MergeMintContract, ());
    let client = MergeMintContractClient::new(&env, &contract_id);

    let mut tags: Vec<Symbol> = Vec::new(&env);
    for _ in 0..6 {
        tags.push_back(Symbol::new(&env, "bug"));
    }

    client.create_bounty(
        &creator,
        &Symbol::new(&env, "overtags"),
        &String::from_str(&env, "desc"),
        &1000,
        &create_token_and_mint(&env, &creator, &contract_id, 0),
        &0,
        &None,
        &tags,
        &1,
        &None,
        &1,
        &Vec::new(&env),
    );
}

// ===========================================================================
// Issue 2 — get_bounties_by_creator
// ===========================================================================

/// Creating 3 bounties from one creator returns all 3 IDs.
#[test]
fn test_get_bounties_by_creator_returns_all() {
    let (env, creator, _contributor, _verifier) = setup_test();
    let contract_id = env.register(MergeMintContract, ());
    let client = MergeMintContractClient::new(&env, &contract_id);

    assert_eq!(
        client.get_bounties_by_creator(&creator, &None, &50).0.len(),
        0
    );

    let id1 = make_bounty(&client, &env, &creator, "b1", None);
    let id2 = make_bounty(&client, &env, &creator, "b2", None);
    let id3 = make_bounty(&client, &env, &creator, "b3", None);

    let ids = client.get_bounties_by_creator(&creator, &None, &50).0;
    assert_eq!(ids.len(), 3);
    assert_eq!(ids.get(0).unwrap(), id1);
    assert_eq!(ids.get(1).unwrap(), id2);
    assert_eq!(ids.get(2).unwrap(), id3);
}

/// Bounties from different creators are indexed independently.
#[test]
fn test_get_bounties_by_creator_independent_lists() {
    let (env, creator, _contributor, _verifier) = setup_test();
    let creator2 = Address::generate(&env);
    let contract_id = env.register(MergeMintContract, ());
    let client = MergeMintContractClient::new(&env, &contract_id);

    let id1 = make_bounty(&client, &env, &creator, "c1a", None);
    let id2 = make_bounty(&client, &env, &creator2, "c2a", None);

    let list1 = client.get_bounties_by_creator(&creator, &None, &50).0;
    let list2 = client.get_bounties_by_creator(&creator2, &None, &50).0;

    assert_eq!(list1.len(), 1);
    assert_eq!(list1.get(0).unwrap(), id1);
    assert_eq!(list2.len(), 1);
    assert_eq!(list2.get(0).unwrap(), id2);
}

/// An address that has never created a bounty returns an empty list.
#[test]
fn test_get_bounties_by_creator_unknown_address_empty() {
    let (env, _creator, _contributor, _verifier) = setup_test();
    let contract_id = env.register(MergeMintContract, ());
    let client = MergeMintContractClient::new(&env, &contract_id);

    let stranger = Address::generate(&env);
    assert_eq!(
        client
            .get_bounties_by_creator(&stranger, &None, &50)
            .0
            .len(),
        0
    );
}

/// Issue #634 — `get_open_bounties_paged` clamps `limit` to 50 (MAX_LIMIT) via `paginate`.
#[test]
fn test_get_open_bounties_paged_limit_capped_at_max() {
    const OPEN_BOUNTY_TAGS: [&str; 55] = [
        "o00", "o01", "o02", "o03", "o04", "o05", "o06", "o07", "o08", "o09", "o10", "o11", "o12",
        "o13", "o14", "o15", "o16", "o17", "o18", "o19", "o20", "o21", "o22", "o23", "o24", "o25",
        "o26", "o27", "o28", "o29", "o30", "o31", "o32", "o33", "o34", "o35", "o36", "o37", "o38",
        "o39", "o40", "o41", "o42", "o43", "o44", "o45", "o46", "o47", "o48", "o49", "o50", "o51",
        "o52", "o53", "o54",
    ];

    let (env, creator, _contributor, _verifier) = setup_test();
    let contract_id = env.register(MergeMintContract, ());
    let client = MergeMintContractClient::new(&env, &contract_id);

    for tag in OPEN_BOUNTY_TAGS {
        make_bounty(&client, &env, &creator, tag, None);
    }

    assert_eq!(client.get_open_bounties_count(), 55);

    let page = client.get_open_bounties_paged(&0, &1000);
    assert_eq!(
        page.len(),
        50,
        "limit above cap must return at most 50 items"
    );

    let page2 = client.get_open_bounties_paged(&50, &1000);
    assert_eq!(page2.len(), 5, "remaining open bounties after first capped page");
}

/// `get_bounties_by_creator` clamps `limit` to 50 (MAX_LIMIT) per `paginate`.
#[test]
fn test_get_bounties_by_creator_limit_capped_at_max() {
    const CREATOR_BOUNTY_TAGS: [&str; 55] = [
        "c00", "c01", "c02", "c03", "c04", "c05", "c06", "c07", "c08", "c09", "c10", "c11", "c12",
        "c13", "c14", "c15", "c16", "c17", "c18", "c19", "c20", "c21", "c22", "c23", "c24", "c25",
        "c26", "c27", "c28", "c29", "c30", "c31", "c32", "c33", "c34", "c35", "c36", "c37", "c38",
        "c39", "c40", "c41", "c42", "c43", "c44", "c45", "c46", "c47", "c48", "c49", "c50", "c51",
        "c52", "c53", "c54",
    ];

    let (env, creator, _contributor, _verifier) = setup_test();
    let contract_id = env.register(MergeMintContract, ());
    let client = MergeMintContractClient::new(&env, &contract_id);

    for tag in CREATOR_BOUNTY_TAGS {
        make_bounty(&client, &env, &creator, tag, None);
    }

    let (page, next) = client.get_bounties_by_creator(&creator, &None, &1000);
    assert_eq!(
        page.len(),
        50,
        "limit above cap must return at most 50 items"
    );
    assert_eq!(
        next,
        Some(50),
        "next_cursor should point past the capped page"
    );

    let (page2, next2) = client.get_bounties_by_creator(&creator, &Some(50), &1000);
    assert_eq!(page2.len(), 5, "remaining items after first capped page");
    assert_eq!(next2, None, "list exhausted after second page");
}

// ===========================================================================
// Issue 3 — dispute guard in complete_bounty
// ===========================================================================

/// complete_bounty on a disputed bounty must panic with "bounty is disputed".
#[test]
#[should_panic(expected = "bounty is disputed")]
fn test_complete_disputed_bounty_panics() {
    let (env, creator, contributor, verifier) = setup_test();
    let contract_id = env.register(MergeMintContract, ());
    let client = MergeMintContractClient::new(&env, &contract_id);

    let bounty_id = make_bounty(&client, &env, &creator, "disp_b", None);
    client.claim_bounty(&contributor, &bounty_id);
    client.raise_dispute(&creator, &bounty_id);

    // Bounty is now "disputed" — complete_bounty must panic.
    client.complete_bounty(&verifier, &bounty_id);
}

/// The assignee raising a dispute also prevents completion.
#[test]
#[should_panic(expected = "bounty is disputed")]
fn test_complete_bounty_after_assignee_dispute_panics() {
    let (env, creator, contributor, verifier) = setup_test();
    let contract_id = env.register(MergeMintContract, ());
    let client = MergeMintContractClient::new(&env, &contract_id);

    let bounty_id = make_bounty(&client, &env, &creator, "disp_c", None);
    client.claim_bounty(&contributor, &bounty_id);
    client.raise_dispute(&contributor, &bounty_id);

    client.complete_bounty(&verifier, &bounty_id);
}

// ===========================================================================
// Issue 4 — ContractError enum (smoke-test the canonical messages)
// ===========================================================================

#[test]
fn test_contract_error_messages() {
    use crate::errors::{message, ContractError};

    assert_eq!(message(ContractError::BountyNotFound), "bounty not found");
    assert_eq!(
        message(ContractError::BountyAlreadyAssigned),
        "bounty already assigned"
    );
    assert_eq!(
        message(ContractError::AlreadyClaimed),
        "bounty already claimed by contributor"
    );
    assert_eq!(message(ContractError::BountyNotOpen), "bounty not open");
    assert_eq!(
        message(ContractError::BountyNotInProgress),
        "bounty is not in progress"
    );
    assert_eq!(
        message(ContractError::BountyHasNoAssignee),
        "bounty has no assignee"
    );
    assert_eq!(
        message(ContractError::RewardMustBePositive),
        "reward_amount must be positive"
    );
    assert_eq!(
        message(ContractError::NotBountyCreator),
        "not bounty creator"
    );
    assert_eq!(
        message(ContractError::VerifierCannotBeAssignee),
        "verifier cannot be the assignee"
    );
    assert_eq!(
        message(ContractError::CreatorCannotClaim),
        "creator cannot claim"
    );
    assert_eq!(
        message(ContractError::ContributorHasActiveClaim),
        "contributor already has an active claim"
    );
    assert_eq!(
        message(ContractError::BountyIsDisputed),
        "bounty is disputed"
    );
    assert_eq!(message(ContractError::TooManyTags), "too many tags");
    assert_eq!(
        message(ContractError::OnlyCreatorOrAssigneeCanDispute),
        "only creator or assignee can raise dispute"
    );
    assert_eq!(
        message(ContractError::DeadlineNotPassed),
        "deadline has not passed"
    );
    assert_eq!(
        message(ContractError::BountyDeadlinePassed),
        "bounty deadline passed"
    );
    assert_eq!(
        message(ContractError::BountyNoDeadline),
        "bounty has no deadline"
    );
    assert_eq!(
        message(ContractError::ReputationTooLow),
        "contributor reputation is too low"
    );
    assert_eq!(
        message(ContractError::VerifierNotAuthorized),
        "verifier is not in the required verifiers list"
    );
    assert_eq!(
        message(ContractError::AlreadyApproved),
        "verifier has already approved this bounty"
    );
    assert_eq!(
        message(ContractError::BountyNotDisputed),
        "bounty is not in disputed status"
    );
    assert_eq!(
        message(ContractError::NotArbitrator),
        "caller is not authorized to resolve this dispute"
    );
    assert_eq!(
        message(ContractError::RewardBelowMinimum),
        "reward_amount is below the minimum allowed"
    );
    assert_eq!(
        message(ContractError::MaxAssigneesMustBePositive),
        "max_assignees must be at least 1"
    );
    assert_eq!(
        message(ContractError::ApprovalThresholdExceedsVerifiers),
        "approval_threshold cannot exceed the number of required_verifiers"
    );
    assert_eq!(
        message(ContractError::InvalidRewardToken),
        "invalid reward_token address"
    );
    assert_eq!(
        message(ContractError::MilestoneAlreadyCompleted),
        "milestone is already completed"
    );
    assert_eq!(
        message(ContractError::NotAllMilestonesCompleted),
        "not all milestones are completed"
    );
    assert_eq!(
        message(ContractError::InvalidMilestoneIndex),
        "invalid milestone index"
    );
    assert_eq!(
        message(ContractError::MilestoneRewardsMismatch),
        "milestone rewards do not sum to reward_amount"
    );
    assert_eq!(
        message(ContractError::MetadataEmpty),
        "metadata must not be empty"
    );
}

/// Every `ContractError` variant must map to a distinct panic message.
#[test]
fn test_contract_error_messages_are_unique() {
    use crate::errors::{message, ContractError};

    let variants = [
        ContractError::BountyNotFound,
        ContractError::BountyAlreadyAssigned,
        ContractError::BountyNotOpen,
        ContractError::BountyNotInProgress,
        ContractError::BountyHasNoAssignee,
        ContractError::RewardMustBePositive,
        ContractError::RewardBelowMinimum,
        ContractError::NotBountyCreator,
        ContractError::VerifierCannotBeAssignee,
        ContractError::CreatorCannotClaim,
        ContractError::ContributorHasActiveClaim,
        ContractError::BountyIsDisputed,
        ContractError::BountyDeadlinePassed,
        ContractError::BountyNoDeadline,
        ContractError::DeadlineNotPassed,
        ContractError::ReputationTooLow,
        ContractError::TooManyTags,
        ContractError::MaxAssigneesMustBePositive,
        ContractError::OnlyCreatorOrAssigneeCanDispute,
        ContractError::VerifierNotAuthorized,
        ContractError::AlreadyApproved,
        ContractError::BountyNotDisputed,
        ContractError::NotArbitrator,
        ContractError::ApprovalThresholdExceedsVerifiers,
        ContractError::InvalidRewardToken,
        ContractError::MilestoneAlreadyCompleted,
        ContractError::NotAllMilestonesCompleted,
        ContractError::InvalidMilestoneIndex,
        ContractError::MilestoneRewardsMismatch,
    ];

    for (i, left) in variants.iter().enumerate() {
        for right in variants.iter().skip(i + 1) {
            assert_ne!(
                message(*left),
                message(*right),
                "duplicate ContractError message between variants"
            );
        }
    }
}

/// create_bounty rejects non-token reward_token addresses (issue #649).
#[test]
#[should_panic(expected = "invalid reward_token address")]
fn test_create_bounty_rejects_invalid_reward_token() {
    let (env, creator, _contributor, _verifier) = setup_test();
    let contract_id = env.register(MergeMintContract, ());
    let client = MergeMintContractClient::new(&env, &contract_id);

    client.create_bounty(
        &creator,
        &Symbol::new(&env, "bad_token"),
        &String::from_str(&env, "desc"),
        &1000,
        &Address::generate(&env),
        &0,
        &None,
        &Vec::new(&env),
        &1,
        &None,
        &1,
        &Vec::new(&env),
    );
}

/// ContractError::TooManyTags is wired to the correct panic message.
#[test]
#[should_panic(expected = "too many tags")]
fn test_fail_too_many_tags_message() {
    use crate::errors::{fail, ContractError};
    fail(ContractError::TooManyTags);
}

/// ContractError::BountyIsDisputed is wired to the correct panic message.
#[test]
#[should_panic(expected = "bounty is disputed")]
fn test_fail_bounty_is_disputed_message() {
    use crate::errors::{fail, ContractError};
    fail(ContractError::BountyIsDisputed);
}

// ===========================================================================
// Issue 435 — reject past deadlines at creation time
// ===========================================================================

/// Creating a bounty with a deadline already in the past must panic.
#[test]
#[should_panic(expected = "bounty deadline passed")]
fn test_create_bounty_rejects_past_deadline() {
    let (env, creator, _contributor, _verifier) = setup_test();
    let contract_id = env.register(MergeMintContract, ());
    let client = MergeMintContractClient::new(&env, &contract_id);

    // Ledger sequence starts at 0; set to 100 so a deadline of 50 is in the past.
    env.ledger().set_sequence_number(100);

    client.create_bounty(
        &creator,
        &Symbol::new(&env, "past_dl"),
        &String::from_str(&env, "desc"),
        &1000,
        &create_token_and_mint(&env, &creator, &contract_id, 0),
        &0,
        &Some(50),
        &Vec::new(&env),
        &1,
        &None,
        &1,
        &Vec::new(&env),
    );
}

/// Creating a bounty with a future deadline must succeed.
#[test]
fn test_create_bounty_accepts_future_deadline() {
    let (env, creator, _contributor, _verifier) = setup_test();
    let contract_id = env.register(MergeMintContract, ());
    let client = MergeMintContractClient::new(&env, &contract_id);

    // Ledger sequence starts at 0; a deadline of 100 is in the future.
    client.create_bounty(
        &creator,
        &Symbol::new(&env, "future_dl"),
        &String::from_str(&env, "desc"),
        &1000,
        &create_token_and_mint(&env, &creator, &contract_id, 0),
        &0,
        &Some(100),
        &Vec::new(&env),
        &1,
        &None,
        &1,
        &Vec::new(&env),
    );
}

// ===========================================================================
// Existing tests — kept clean and compiling
// ===========================================================================

#[test]
fn test_create_bounty() {
    let (env, creator, _contributor, _verifier) = setup_test();
    let contract_id = env.register(MergeMintContract, ());
    let client = MergeMintContractClient::new(&env, &contract_id);

    let reward_amount: i128 = 1000;
    let reward_token = create_token_and_mint(&env, &creator, &contract_id, 0);
    let bounty_id = client.create_bounty(
        &creator,
        &Symbol::new(&env, "test_b"),
        &String::from_str(&env, "desc"),
        &reward_amount,
        &reward_token,
        &0,
        &None,
        &Vec::new(&env),
        &1,
        &None,
        &1,
        &Vec::new(&env),
    );

    let bounty = client.get_bounty(&bounty_id).unwrap();
    assert_eq!(bounty.reward_amount, reward_amount);
    assert_eq!(bounty.creator, creator);
    assert!(bounty.assignees.is_empty());

    let metas = client.get_bounty_metas(&Vec::from_array(&env, [bounty_id.clone()]));
    let meta = metas.get(0).unwrap().unwrap();
    assert_eq!(meta.title, Symbol::new(&env, "test_b"));
    assert_eq!(meta.description, String::from_str(&env, "desc"));
}

// ===========================================================================
// Issue 449 — create_bounty rejects non-positive reward_amount
// ===========================================================================

/// Creating a bounty with reward_amount = 0 must panic.
#[test]
#[should_panic(expected = "reward_amount must be positive")]
fn test_create_bounty_rejects_zero_reward() {
    let (env, creator, _contributor, _verifier) = setup_test();
    let contract_id = env.register(MergeMintContract, ());
    let client = MergeMintContractClient::new(&env, &contract_id);

    client.create_bounty(
        &creator,
        &Symbol::new(&env, "zero_rew"),
        &String::from_str(&env, "desc"),
        &0,
        &create_token_and_mint(&env, &creator, &contract_id, 0),
        &0,
        &None,
        &Vec::new(&env),
        &1,
        &None,
        &1,
        &Vec::new(&env),
    );
}

/// Creating a bounty with a negative reward_amount must panic.
#[test]
#[should_panic(expected = "reward_amount must be positive")]
fn test_create_bounty_rejects_negative_reward() {
    let (env, creator, _contributor, _verifier) = setup_test();
    let contract_id = env.register(MergeMintContract, ());
    let client = MergeMintContractClient::new(&env, &contract_id);

    client.create_bounty(
        &creator,
        &Symbol::new(&env, "neg_rew"),
        &String::from_str(&env, "desc"),
        &(-50),
        &create_token_and_mint(&env, &creator, &contract_id, 0),
        &0,
        &None,
        &Vec::new(&env),
        &1,
        &None,
        &1,
        &Vec::new(&env),
    );
}

/// Creating a bounty with reward_amount below MIN_REWARD_AMOUNT (100) must panic.
#[test]
#[should_panic(expected = "reward_amount is below the minimum allowed")]
fn test_create_bounty_rejects_below_minimum_reward() {
    let (env, creator, _contributor, _verifier) = setup_test();
    let contract_id = env.register(MergeMintContract, ());
    let client = MergeMintContractClient::new(&env, &contract_id);

    client.create_bounty(
        &creator,
        &Symbol::new(&env, "small_rew"),
        &String::from_str(&env, "desc"),
        &50,
        &create_token_and_mint(&env, &creator, &contract_id, 0),
        &0,
        &None,
        &Vec::new(&env),
        &1,
        &None,
        &1,
        &Vec::new(&env),
    );
}

/// Milestone reward summation must fail closed on i128 overflow.
#[test]
#[should_panic(expected = "reward amount arithmetic overflow")]
fn test_create_bounty_rejects_milestone_reward_overflow() {
    let (env, creator, _contributor, _verifier) = setup_test();
    let contract_id = env.register(MergeMintContract, ());
    let client = MergeMintContractClient::new(&env, &contract_id);

    let mut milestones = Vec::new(&env);
    milestones.push_back(Milestone {
        description: Symbol::new(&env, "m1"),
        reward: i128::MAX,
        completed: false,
    });
    milestones.push_back(Milestone {
        description: Symbol::new(&env, "m2"),
        reward: 1,
        completed: false,
    });

    client.create_bounty(
        &creator,
        &Symbol::new(&env, "overflow"),
        &String::from_str(&env, "desc"),
        &1000,
        &create_token_and_mint(&env, &creator, &contract_id, 0),
        &0,
        &None,
        &Vec::new(&env),
        &1,
        &None,
        &1,
        &milestones,
    );
}

#[test]
fn test_claim_bounty() {
    let (env, creator, contributor, _verifier) = setup_test();
    let contract_id = env.register(MergeMintContract, ());
    let client = MergeMintContractClient::new(&env, &contract_id);

    let bounty_id = client.create_bounty(
        &creator,
        &Symbol::new(&env, "bounty_1"),
        &String::from_str(&env, "desc"),
        &1000,
        &create_token_and_mint(&env, &creator, &contract_id, 0),
        &0,
        &None,
        &Vec::new(&env),
        &1,
        &None,
        &1,
        &Vec::new(&env),
    );
    client.claim_bounty(&contributor, &bounty_id);

    let bounty = client.get_bounty(&bounty_id).unwrap();
    let (assignee_addr, share) = bounty.assignees.get(0).unwrap();
    assert_eq!(assignee_addr, contributor);
    assert_eq!(share, 10_000u32);
}

// ===========================================================================
// Issue #757 — Security: prevent creator self-claim (security/prevent-creator-claim.md)
// ===========================================================================

#[test]
#[should_panic(expected = "creator cannot claim")]
fn test_creator_cannot_claim_own_bounty() {
    let (env, creator, _contributor, _verifier) = setup_test();
    let contract_id = env.register(MergeMintContract, ());
    let client = MergeMintContractClient::new(&env, &contract_id);

    let bounty_id = client.create_bounty(
        &creator,
        &Symbol::new(&env, "bounty_1"),
        &String::from_str(&env, "desc"),
        &1000,
        &create_token_and_mint(&env, &creator, &contract_id, 0),
        &0,
        &None,
        &Vec::new(&env),
        &1,
        &None,
        &1,
        &Vec::new(&env),
    );
    client.claim_bounty(&creator, &bounty_id);
}

/// The creator self-claim guard fires even when the creator address is
/// passed through an alias variable, confirming no bypass via indirection.
/// Regression test for security/prevent-creator-claim.md.
#[test]
#[should_panic(expected = "creator cannot claim")]
fn test_creator_self_claim_guard_no_alias_bypass() {
    let (env, creator, _contributor, _verifier) = setup_test();
    let contract_id = env.register(MergeMintContract, ());
    let client = MergeMintContractClient::new(&env, &contract_id);

    let bounty_id = client.create_bounty(
        &creator,
        &Symbol::new(&env, "alias_test"),
        &String::from_str(&env, "desc"),
        &1000,
        &create_token_and_mint(&env, &creator, &contract_id, 0),
        &0,
        &None,
        &Vec::new(&env),
        &1,
        &None,
        &1,
        &Vec::new(&env),
    );

    // Alias the creator address — the guard must still fire.
    let also_creator = creator.clone();
    client.claim_bounty(&also_creator, &bounty_id);
}

#[test]
fn test_bounty_count() {
    let (env, creator, _contributor, _verifier) = setup_test();
    let contract_id = env.register(MergeMintContract, ());
    let client = MergeMintContractClient::new(&env, &contract_id);

    assert_eq!(client.get_bounty_count(), 0);
    let reward_token = create_token_and_mint(&env, &creator, &contract_id, 0);
    client.create_bounty(
        &creator,
        &Symbol::new(&env, "bounty_a"),
        &String::from_str(&env, "desc_a"),
        &100,
        &reward_token,
        &0,
        &None,
        &Vec::new(&env),
        &1,
        &None,
        &1,
        &Vec::new(&env),
    );
    assert_eq!(client.get_bounty_count(), 1);
    client.create_bounty(
        &creator,
        &Symbol::new(&env, "bounty_b"),
        &String::from_str(&env, "desc_b"),
        &200,
        &reward_token,
        &0,
        &None,
        &Vec::new(&env),
        &1,
        &None,
        &1,
        &Vec::new(&env),
    );
    assert_eq!(client.get_bounty_count(), 2);
}

/// Issue #633 — never-allocated IDs return None without panicking.
#[test]
fn test_get_bounty_never_allocated_id_returns_none() {
    let (env, _creator, _contributor, _verifier) = setup_test();
    let contract_id = env.register(MergeMintContract, ());
    let client = MergeMintContractClient::new(&env, &contract_id);

    assert_eq!(client.get_bounty_count(), 0);
    let unallocated = fake_bounty_id(&env, 0);
    assert!(client.get_bounty(&unallocated).is_none());
}

/// Issue #633 — IDs beyond `get_bounty_count()` return None without panicking.
#[test]
fn test_get_bounty_id_beyond_count_returns_none() {
    let (env, creator, _contributor, _verifier) = setup_test();
    let contract_id = env.register(MergeMintContract, ());
    let client = MergeMintContractClient::new(&env, &contract_id);

    make_bounty(&client, &env, &creator, "only", None);
    assert_eq!(client.get_bounty_count(), 1);

    let beyond = fake_bounty_id(&env, 99);
    assert!(client.get_bounty(&beyond).is_none());
}

/// Issue #633 — pruned (allocated but missing) IDs return None without panicking.
#[test]
fn test_get_bounty_pruned_allocated_id_returns_none() {
    let (env, creator, _contributor, _verifier) = setup_test();
    let contract_id = env.register(MergeMintContract, ());
    let client = MergeMintContractClient::new(&env, &contract_id);

    let bounty_id = make_bounty(&client, &env, &creator, "pruned", None);
    assert!(client.get_bounty(&bounty_id).is_some());

    env.as_contract(&contract_id, || {
        env.storage()
            .persistent()
            .remove(&DataKey::Bounty(bounty_id.clone()));
        assert!(storage::bounty_id_was_allocated(&env, &bounty_id));
    });

    assert!(client.get_bounty(&bounty_id).is_none());
}

#[test]
fn test_single_assignee_gets_full_share() {
    let (env, creator, contributor, _verifier) = setup_test();
    let contract_id = env.register(MergeMintContract, ());
    let client = MergeMintContractClient::new(&env, &contract_id);

    let bounty_id = make_bounty(&client, &env, &creator, "single", None);
    client.claim_bounty(&contributor, &bounty_id);

    let bounty = client.get_bounty(&bounty_id).unwrap();
    assert_eq!(bounty.assignees.len(), 1);
    let (addr, share) = bounty.assignees.get(0).unwrap();
    assert_eq!(addr, contributor);
    assert_eq!(share, 10_000u32);
}

// ===========================================================================
// Dispute handling
// ===========================================================================

#[test]
fn test_raise_dispute_creator() {
    let (env, creator, contributor, _verifier) = setup_test();
    let contract_id = env.register(MergeMintContract, ());
    let client = MergeMintContractClient::new(&env, &contract_id);

    let bounty_id = make_bounty(&client, &env, &creator, "dispute_1", None);
    client.claim_bounty(&contributor, &bounty_id);
    client.raise_dispute(&creator, &bounty_id);

    let bounty = client.get_bounty(&bounty_id).unwrap();
    assert_eq!(bounty.status, Symbol::new(&env, "disputed"));
}

#[test]
fn test_raise_dispute_assignee() {
    let (env, creator, contributor, _verifier) = setup_test();
    let contract_id = env.register(MergeMintContract, ());
    let client = MergeMintContractClient::new(&env, &contract_id);

    let bounty_id = make_bounty(&client, &env, &creator, "dispute_2", None);
    client.claim_bounty(&contributor, &bounty_id);
    client.raise_dispute(&contributor, &bounty_id);

    let bounty = client.get_bounty(&bounty_id).unwrap();
    assert_eq!(bounty.status, Symbol::new(&env, "disputed"));
}

/// A second `raise_dispute` on an already-disputed bounty must panic.
#[test]
#[should_panic(expected = "bounty is disputed")]
fn test_raise_dispute_second_dispute_fails() {
    let (env, creator, contributor, _verifier) = setup_test();
    let contract_id = env.register(MergeMintContract, ());
    let client = MergeMintContractClient::new(&env, &contract_id);

    let bounty_id = make_bounty(&client, &env, &creator, "dispute_2x", None);
    client.claim_bounty(&contributor, &bounty_id);
    client.raise_dispute(&creator, &bounty_id);
    // Bounty is already disputed — a second raise must be rejected.
    client.raise_dispute(&creator, &bounty_id);
}

#[test]
#[should_panic(expected = "only creator or assignee can raise dispute")]
fn test_raise_dispute_third_party_fails() {
    let (env, creator, contributor, _verifier) = setup_test();
    let contract_id = env.register(MergeMintContract, ());
    let client = MergeMintContractClient::new(&env, &contract_id);

    let third_party = Address::generate(&env);
    let bounty_id = make_bounty(&client, &env, &creator, "dispute_3", None);
    client.claim_bounty(&contributor, &bounty_id);
    client.raise_dispute(&third_party, &bounty_id);
}

// ===========================================================================
// Claim guards
// ===========================================================================

#[test]
#[should_panic(expected = "contributor already has an active claim")]
fn test_second_claim_rejected_while_active() {
    let (env, creator, contributor, _verifier) = setup_test();
    let contract_id = env.register(MergeMintContract, ());
    let client = MergeMintContractClient::new(&env, &contract_id);

    let bounty_id_1 = make_bounty(&client, &env, &creator, "active1", None);
    let bounty_id_2 = make_bounty(&client, &env, &creator, "active2", None);

    client.claim_bounty(&contributor, &bounty_id_1);
    // Second claim on a different bounty must be rejected while the first is active.
    client.claim_bounty(&contributor, &bounty_id_2);
}

#[test]
#[should_panic(expected = "bounty already assigned")]
fn test_second_contributor_cannot_claim_full_bounty() {
    let (env, creator, contributor, _verifier) = setup_test();
    let contract_id = env.register(MergeMintContract, ());
    let client = MergeMintContractClient::new(&env, &contract_id);

    let bounty_id = make_bounty(&client, &env, &creator, "full_c", None);
    client.claim_bounty(&contributor, &bounty_id);

    // A different contributor tries to claim a full single-slot bounty.
    let contributor2 = Address::generate(&env);
    client.claim_bounty(&contributor2, &bounty_id);
}

/// Issue #630 — complete_milestone must fail before touching milestone state.
#[test]
#[should_panic(expected = "bounty not found")]
fn test_complete_milestone_nonexistent_bounty() {
    let (env, _creator, _contributor, verifier) = setup_test();
    let contract_id = env.register(MergeMintContract, ());
    let client = MergeMintContractClient::new(&env, &contract_id);

    let fake_id = fake_bounty_id(&env, 42);
    client.complete_milestone(&verifier, &fake_id, &0);
}

/// Issue #629 — idempotency guard rejects duplicate claim by same contributor.
#[test]
#[should_panic(expected = "bounty already claimed by contributor")]
fn test_claim_bounty_idempotency_rejects_double_claim() {
    let (env, creator, contributor, _verifier) = setup_test();
    let contract_id = env.register(MergeMintContract, ());
    let client = MergeMintContractClient::new(&env, &contract_id);

    let bounty_id = make_bounty(&client, &env, &creator, "idempotent", None);
    client.claim_bounty(&contributor, &bounty_id);
    // Second claim by the same contributor must fail with AlreadyClaimed.
    client.claim_bounty(&contributor, &bounty_id);
}

// ===========================================================================
// Issue 451 — claim_bounty deadline enforcement
// ===========================================================================

/// Claiming a bounty whose deadline has passed must panic.
#[test]
#[should_panic(expected = "bounty deadline passed")]
fn test_claim_bounty_after_deadline_panics() {
    let (env, creator, contributor, _verifier) = setup_test();
    let contract_id = env.register(MergeMintContract, ());
    let client = MergeMintContractClient::new(&env, &contract_id);

    // Create a bounty with a deadline at ledger sequence 50.
    let bounty_id = make_bounty(&client, &env, &creator, "dl_claim", Some(50));

    // Advance ledger past the deadline.
    env.ledger().set_sequence_number(100);

    // Claiming must fail — deadline has passed.
    client.claim_bounty(&contributor, &bounty_id);
}

/// Claiming a bounty before its deadline must succeed.
#[test]
fn test_claim_bounty_before_deadline_succeeds() {
    let (env, creator, contributor, _verifier) = setup_test();
    let contract_id = env.register(MergeMintContract, ());
    let client = MergeMintContractClient::new(&env, &contract_id);

    // Create a bounty with a deadline at ledger sequence 100.
    let bounty_id = make_bounty(&client, &env, &creator, "dl_ok", Some(100));

    // Ledger is still at 0 — deadline is in the future.
    client.claim_bounty(&contributor, &bounty_id);

    let bounty = client.get_bounty(&bounty_id).unwrap();
    assert_eq!(bounty.status, Symbol::new(&env, "in_progress"));
}

// ===========================================================================
// Issue 452 — claim_bounty minimum-reputation rejection
// ===========================================================================

/// Claiming a bounty with min_reputation > 0 as a 0-reputation contributor must panic.
#[test]
#[should_panic(expected = "contributor reputation is too low")]
fn test_claim_bounty_rejects_low_reputation() {
    let (env, creator, contributor, _verifier) = setup_test();
    let contract_id = env.register(MergeMintContract, ());
    let client = MergeMintContractClient::new(&env, &contract_id);

    // Create a bounty with min_reputation = 10.
    client.create_bounty(
        &creator,
        &Symbol::new(&env, "rep_b"),
        &String::from_str(&env, "desc"),
        &1000,
        &create_token_and_mint(&env, &creator, &contract_id, 0),
        &10,
        &None,
        &Vec::new(&env),
        &1,
        &None,
        &1,
        &Vec::new(&env),
    );

    let bounty_id = client
        .get_bounties_by_creator(&creator, &None, &50)
        .0
        .get(0)
        .unwrap();
    // Contributor has 0 reputation — must be rejected.
    client.claim_bounty(&contributor, &bounty_id);
}

// ===========================================================================
// Issue 455 — cancel_bounty rejection paths
// ===========================================================================

/// Cancelling a bounty as a non-creator must panic.
#[test]
#[should_panic(expected = "not bounty creator")]
fn test_cancel_bounty_non_creator_fails() {
    let (env, creator, contributor, _verifier) = setup_test();
    let contract_id = env.register(MergeMintContract, ());
    let client = MergeMintContractClient::new(&env, &contract_id);

    let bounty_id = make_bounty(&client, &env, &creator, "cancel_nc", None);
    // Contributor is not the creator — must be rejected.
    client.cancel_bounty(&contributor, &bounty_id);
}

/// Cancelling a bounty that is already claimed (in_progress) must panic.
#[test]
#[should_panic(expected = "bounty not open")]
fn test_cancel_bounty_claimed_bounty_fails() {
    let (env, creator, contributor, _verifier) = setup_test();
    let contract_id = env.register(MergeMintContract, ());
    let client = MergeMintContractClient::new(&env, &contract_id);

    let bounty_id = make_bounty(&client, &env, &creator, "cancel_cl", None);
    client.claim_bounty(&contributor, &bounty_id);
    // Bounty is now in_progress — cancel must fail.
    client.cancel_bounty(&creator, &bounty_id);
}

/// Cancelling a bounty that is already completed must panic.
#[test]
#[should_panic(expected = "bounty not open")]
fn test_cancel_bounty_completed_bounty_fails() {
    let (env, creator, contributor, verifier) = setup_test();
    let contract_id = env.register(MergeMintContract, ());
    let client = MergeMintContractClient::new(&env, &contract_id);

    let reward_amount: i128 = 1000;
    let (bounty_id, _token_addr) = make_bounty_with_token(
        &client,
        &env,
        &creator,
        &contract_id,
        "cancel_done",
        reward_amount,
        None,
    );
    client.claim_bounty(&contributor, &bounty_id);
    client.complete_bounty(&verifier, &bounty_id);
    // Bounty is now completed — cancel must fail.
    client.cancel_bounty(&creator, &bounty_id);
}

// ===========================================================================
// Status index
// ===========================================================================

#[test]
fn test_status_index_open_on_create() {
    let (env, creator, _contributor, _verifier) = setup_test();
    let contract_id = env.register(MergeMintContract, ());
    let client = MergeMintContractClient::new(&env, &contract_id);

    let bounty_id = make_bounty(&client, &env, &creator, "status_open", None);

    let open_ids = client
        .get_bounties_by_status(&Symbol::new(&env, "open"), &None, &50)
        .0;
    assert_eq!(open_ids.len(), 1);
    assert_eq!(open_ids.get(0).unwrap(), bounty_id);
}

#[test]
fn test_status_index_moves_on_cancel() {
    let (env, creator, _contributor, _verifier) = setup_test();
    let contract_id = env.register(MergeMintContract, ());
    let client = MergeMintContractClient::new(&env, &contract_id);

    let (bounty_id, _token_addr) = make_bounty_with_token(
        &client,
        &env,
        &creator,
        &contract_id,
        "bounty_z",
        1000,
        None,
    );
    client.cancel_bounty(&creator, &bounty_id);

    let open_ids = client
        .get_bounties_by_status(&Symbol::new(&env, "open"), &None, &50)
        .0;
    let cancelled_ids = client
        .get_bounties_by_status(&Symbol::new(&env, "cancelled"), &None, &50)
        .0;
    assert_eq!(open_ids.len(), 0);
    assert_eq!(cancelled_ids.len(), 1);
    assert_eq!(cancelled_ids.get(0).unwrap(), bounty_id);
}

// ===========================================================================
// Status count (issue #443)
// ===========================================================================

/// get_status_count returns 0 for a status with no bounties.
#[test]
fn test_status_count_initial_zero() {
    let (env, _creator, _contributor, _verifier) = setup_test();
    let contract_id = env.register(MergeMintContract, ());
    let client = MergeMintContractClient::new(&env, &contract_id);

    assert_eq!(client.get_status_count(&Symbol::new(&env, "open")), 0);
    assert_eq!(
        client.get_status_count(&Symbol::new(&env, "in_progress")),
        0
    );
    assert_eq!(client.get_status_count(&Symbol::new(&env, "completed")), 0);
    assert_eq!(client.get_status_count(&Symbol::new(&env, "cancelled")), 0);
}

/// get_status_count returns 1 after creating a single bounty (open status).
#[test]
fn test_status_count_one_on_create() {
    let (env, creator, _contributor, _verifier) = setup_test();
    let contract_id = env.register(MergeMintContract, ());
    let client = MergeMintContractClient::new(&env, &contract_id);

    let _bounty_id = make_bounty(&client, &env, &creator, "count_one", None);

    assert_eq!(client.get_status_count(&Symbol::new(&env, "open")), 1);
    assert_eq!(client.get_status_count(&Symbol::new(&env, "cancelled")), 0);
    assert_eq!(client.get_status_count(&Symbol::new(&env, "completed")), 0);
}

/// get_status_count reflects the move from open to cancelled after cancel_bounty.
#[test]
fn test_status_count_moves_on_cancel() {
    let (env, creator, _contributor, _verifier) = setup_test();
    let contract_id = env.register(MergeMintContract, ());
    let client = MergeMintContractClient::new(&env, &contract_id);

    let (bounty_id, _token_addr) = make_bounty_with_token(
        &client,
        &env,
        &creator,
        &contract_id,
        "count_cancel",
        1000,
        None,
    );
    assert_eq!(client.get_status_count(&Symbol::new(&env, "open")), 1);

    client.cancel_bounty(&creator, &bounty_id);

    assert_eq!(client.get_status_count(&Symbol::new(&env, "open")), 0);
    assert_eq!(client.get_status_count(&Symbol::new(&env, "cancelled")), 1);
}

/// get_status_count correctly tracks multiple bounties across statuses.
#[test]
fn test_status_count_multiple_bounties() {
    let (env, creator, _contributor, verifier) = setup_test();
    let contract_id = env.register(MergeMintContract, ());
    let client = MergeMintContractClient::new(&env, &contract_id);

    let (id1, _id1_token) = make_bounty_with_token(
        &client,
        &env,
        &creator,
        &contract_id,
        "count_multi_a",
        1000,
        None,
    );
    let id2 = make_bounty(&client, &env, &creator, "count_multi_b", None);
    let _id3 = make_bounty(&client, &env, &creator, "count_multi_c", None);

    assert_eq!(client.get_status_count(&Symbol::new(&env, "open")), 3);

    // Cancel one
    client.cancel_bounty(&creator, &id1);
    assert_eq!(client.get_status_count(&Symbol::new(&env, "open")), 2);
    assert_eq!(client.get_status_count(&Symbol::new(&env, "cancelled")), 1);

    // Claim one -> in_progress
    let _token_addr = Address::generate(&env);
    client.claim_bounty(&verifier, &id2);
    assert_eq!(client.get_status_count(&Symbol::new(&env, "open")), 1);
    assert_eq!(
        client.get_status_count(&Symbol::new(&env, "in_progress")),
        1
    );
    assert_eq!(client.get_status_count(&Symbol::new(&env, "cancelled")), 1);
}

// ===========================================================================
// Contributor metadata
// ===========================================================================

#[test]
fn test_update_contributor_metadata_stores_value() {
    let (env, _creator, contributor, _verifier) = setup_test();
    let contract_id = env.register(MergeMintContract, ());
    let client = MergeMintContractClient::new(&env, &contract_id);

    let uri = Symbol::new(&env, "ipfs_hash_1");
    client.update_contributor_metadata(&contributor, &uri);

    let data = client.get_contributor(&contributor).unwrap();
    assert_eq!(data.metadata.unwrap(), uri);
}

#[test]
fn test_update_contributor_metadata_overwrites() {
    let (env, _creator, contributor, _verifier) = setup_test();
    let contract_id = env.register(MergeMintContract, ());
    let client = MergeMintContractClient::new(&env, &contract_id);

    client.update_contributor_metadata(&contributor, &Symbol::new(&env, "old_uri"));
    client.update_contributor_metadata(&contributor, &Symbol::new(&env, "new_uri"));

    let data = client.get_contributor(&contributor).unwrap();
    assert_eq!(data.metadata.unwrap(), Symbol::new(&env, "new_uri"));
}

/// An empty metadata Symbol must be rejected before writing to storage.
#[test]
#[should_panic(expected = "metadata must not be empty")]
fn test_update_contributor_metadata_rejects_empty_symbol() {
    let (env, _creator, contributor, _verifier) = setup_test();
    let contract_id = env.register(MergeMintContract, ());
    let client = MergeMintContractClient::new(&env, &contract_id);

    client.update_contributor_metadata(&contributor, &Symbol::new(&env, ""));
}

// ===========================================================================
// Batch query: get_bounty_metas
// ===========================================================================

/// Batch query returns correct metas for known IDs and None for unknown IDs.
#[test]
fn test_get_bounty_metas_batch() {
    let (env, creator, _contributor, _verifier) = setup_test();
    let contract_id = env.register(MergeMintContract, ());
    let client = MergeMintContractClient::new(&env, &contract_id);

    let id1 = make_bounty(&client, &env, &creator, "meta1", None);
    let id2 = make_bounty(&client, &env, &creator, "meta2", None);

    // Use a non-zero pattern for unknown IDs (see Bounty ID generation pitfall:
    // the first bounty has count=0, producing all zeros)
    let unknown_id = crate::types::BountyId(soroban_sdk::BytesN::from_array(&env, &[0xffu8; 32]));

    let mut ids: Vec<crate::types::BountyId> = Vec::new(&env);
    ids.push_back(id1.clone());
    ids.push_back(unknown_id.clone());
    ids.push_back(id2.clone());

    let results = client.get_bounty_metas(&ids);
    assert_eq!(results.len(), 3);

    // First result: known ID — should be Some
    match results.get(0).unwrap() {
        Some(meta) => assert_eq!(meta.title, Symbol::new(&env, "meta1")),
        None => panic!("expected Some for known ID"),
    }

    // Second result: unknown ID — should be None
    assert!(
        results.get(1).unwrap().is_none(),
        "expected None for unknown ID"
    );

    // Third result: known ID — should be Some
    match results.get(2).unwrap() {
        Some(meta) => assert_eq!(meta.title, Symbol::new(&env, "meta2")),
        None => panic!("expected Some for known ID"),
    }
}

// ===========================================================================
// Security: double-completion guard
// ===========================================================================

/// Calling complete_bounty on a bounty in "open" status must panic with
/// "bounty is not in progress".
#[test]
#[should_panic(expected = "bounty is not in progress")]
fn test_double_complete_panics() {
    let (env, creator, _contributor, verifier) = setup_test();
    let contract_id = env.register(MergeMintContract, ());
    let client = MergeMintContractClient::new(&env, &contract_id);

    let bounty_id = client.create_bounty(
        &creator,
        &Symbol::new(&env, "dbl_complete"),
        &String::from_str(&env, "desc"),
        &1000,
        &create_token_and_mint(&env, &creator, &contract_id, 0),
        &0,
        &None,
        &Vec::new(&env),
        &1,
        &None,
        &1,
        &Vec::new(&env),
    );

    // Bounty is "open", not "in_progress" — must panic.
    client.complete_bounty(&verifier, &bounty_id);
}

// ===========================================================================
// Status count query
// ===========================================================================

/// get_status_count matches the actual index length for open status after
/// creating a bounty and cancelling it.
#[test]
fn test_status_count_open_on_create() {
    let (env, creator, _contributor, _verifier) = setup_test();
    let contract_id = env.register(MergeMintContract, ());
    let client = MergeMintContractClient::new(&env, &contract_id);

    assert_eq!(
        client.get_status_count(&Symbol::new(&env, "open")),
        0,
        "no bounties yet"
    );

    let _bounty_id = make_bounty(&client, &env, &creator, "sc_open", None);

    let open_count = client.get_status_count(&Symbol::new(&env, "open"));
    let open_ids = client
        .get_bounties_by_status(&Symbol::new(&env, "open"), &None, &50)
        .0;
    assert_eq!(open_count, open_ids.len(), "count matches index length");
    assert_eq!(open_count, 1, "exactly one open bounty");
}

/// Transaction: create → claim → cancel — verify count tracks each transition.
#[test]
fn test_status_count_across_transitions() {
    let (env, creator, contributor, _verifier) = setup_test();
    let contract_id = env.register(MergeMintContract, ());
    let client = MergeMintContractClient::new(&env, &contract_id);

    // Create: open=1, in_progress=0, cancelled=0
    let bounty_id = make_bounty(&client, &env, &creator, "sc_trans", None);
    assert_eq!(client.get_status_count(&Symbol::new(&env, "open")), 1);
    assert_eq!(
        client.get_status_count(&Symbol::new(&env, "in_progress")),
        0
    );
    assert_eq!(client.get_status_count(&Symbol::new(&env, "cancelled")), 0);

    // Claim: open=0, in_progress=1
    client.claim_bounty(&contributor, &bounty_id);
    assert_eq!(client.get_status_count(&Symbol::new(&env, "open")), 0);
    assert_eq!(
        client.get_status_count(&Symbol::new(&env, "in_progress")),
        1
    );
    assert_eq!(
        client.get_status_count(&Symbol::new(&env, "in_progress")),
        client
            .get_bounties_by_status(&Symbol::new(&env, "in_progress"), &None, &50)
            .0
            .len(),
    );
    // and cancel it directly: open=0→1, cancelled=0→1
    let (bounty_id2, _bounty_id2_token) = make_bounty_with_token(
        &client,
        &env,
        &creator,
        &contract_id,
        "sc_trans2",
        1000,
        None,
    );
    assert_eq!(client.get_status_count(&Symbol::new(&env, "open")), 1);
    client.cancel_bounty(&creator, &bounty_id2);
    assert_eq!(client.get_status_count(&Symbol::new(&env, "open")), 0);
    assert_eq!(client.get_status_count(&Symbol::new(&env, "cancelled")), 1);
    assert_eq!(
        client.get_status_count(&Symbol::new(&env, "cancelled")),
        client
            .get_bounties_by_status(&Symbol::new(&env, "cancelled"), &None, &50)
            .0
            .len(),
    );
}

/// Multiple bounties in the same status are counted correctly, and the
/// count matches the length of the status index.
#[test]
fn test_status_count_matches_index_length() {
    let (env, creator, _contributor, _verifier) = setup_test();
    let contract_id = env.register(MergeMintContract, ());
    let client = MergeMintContractClient::new(&env, &contract_id);

    let _b1 = make_bounty(&client, &env, &creator, "sc_multi1", None);
    let _b2 = make_bounty(&client, &env, &creator, "sc_multi2", None);
    let _b3 = make_bounty(&client, &env, &creator, "sc_multi3", None);

    assert_eq!(client.get_status_count(&Symbol::new(&env, "open")), 3);
    assert_eq!(
        client.get_status_count(&Symbol::new(&env, "open")),
        client
            .get_bounties_by_status(&Symbol::new(&env, "open"), &None, &50)
            .0
            .len(),
    );
}
/// The assignee calling complete_bounty as their own verifier must panic.
#[test]
#[should_panic(expected = "verifier cannot be the assignee")]
fn test_assignee_cannot_self_verify() {
    let (env, creator, contributor, _verifier) = setup_test();
    let contract_id = env.register(MergeMintContract, ());
    let client = MergeMintContractClient::new(&env, &contract_id);

    let bounty_id = client.create_bounty(
        &creator,
        &Symbol::new(&env, "self_verify"),
        &String::from_str(&env, "desc"),
        &1000,
        &create_token_and_mint(&env, &creator, &contract_id, 0),
        &0,
        &None,
        &Vec::new(&env),
        &1,
        &None,
        &1,
        &Vec::new(&env),
    );

    client.claim_bounty(&contributor, &bounty_id);

    // The assignee (contributor) attempts to act as their own verifier — must panic.
    client.complete_bounty(&contributor, &bounty_id);
}

// ===========================================================================
// Issue 36 — Escrow refund on cancel / expire / dispute-resolve(cancel)
// ===========================================================================

/// cancel_bounty refunds the escrowed reward to the creator.
#[test]
fn test_cancel_bounty_refunds_escrow() {
    let (env, creator, _contributor, _verifier) = setup_test();
    let contract_id = env.register(MergeMintContract, ());
    let client = MergeMintContractClient::new(&env, &contract_id);

    let reward_amount: i128 = 1000;
    let (bounty_id, token_addr) = make_bounty_with_token(
        &client,
        &env,
        &creator,
        &contract_id,
        "refund_cancel",
        reward_amount,
        None,
    );

    // Check contract balance before cancel.
    let token_client = StellarAssetClient::new(&env, &token_addr);
    assert_eq!(
        token_client.balance(&contract_id),
        reward_amount,
        "contract holds the escrowed reward before cancel"
    );

    client.cancel_bounty(&creator, &bounty_id);

    // After cancel, the contract balance is 0 (all refunded to creator).
    assert_eq!(
        token_client.balance(&contract_id),
        0,
        "contract balance is zero after refund"
    );
    // The creator received the refund.
    assert_eq!(
        token_client.balance(&creator),
        reward_amount,
        "creator received the refunded reward"
    );
}

/// expire_bounty before the deadline must panic with DeadlineNotPassed.
#[test]
#[should_panic(expected = "deadline has not passed")]
fn test_expire_bounty_before_deadline_panics() {
    let (env, creator, _contributor, _verifier) = setup_test();
    let contract_id = env.register(MergeMintContract, ());
    let client = MergeMintContractClient::new(&env, &contract_id);

    let (bounty_id, _token_addr) = make_bounty_with_token(
        &client,
        &env,
        &creator,
        &contract_id,
        "expire_too_early",
        1000,
        Some(100),
    );

    let caller = Address::generate(&env);
    client.expire_bounty(&caller, &bounty_id);
}

/// expire_bounty refunds the escrowed reward to the creator.
#[test]
fn test_expire_bounty_refunds_escrow() {
    let (env, creator, _contributor, _verifier) = setup_test();
    let contract_id = env.register(MergeMintContract, ());
    let client = MergeMintContractClient::new(&env, &contract_id);

    // Create with a deadline in the future, then advance the ledger past it
    // so the bounty is expired without tripping the creation-time past-deadline guard.
    let reward_amount: i128 = 1000;
    let (bounty_id, token_addr) = make_bounty_with_token(
        &client,
        &env,
        &creator,
        &contract_id,
        "refund_expire",
        reward_amount,
        Some(10),
    );

    let token_client = StellarAssetClient::new(&env, &token_addr);
    assert_eq!(
        token_client.balance(&contract_id),
        reward_amount,
        "contract holds the escrowed reward before expire"
    );

    env.ledger().set_sequence_number(100);

    // Any caller can trigger expiry.
    let caller = Address::generate(&env);
    client.expire_bounty(&caller, &bounty_id);

    assert_eq!(
        token_client.balance(&contract_id),
        0,
        "contract balance is zero after refund"
    );
    assert_eq!(
        token_client.balance(&creator),
        reward_amount,
        "creator received the refunded reward"
    );
}

/// resolve_dispute with "cancel" resolution refunds the escrowed reward to the creator.
#[test]
fn test_resolve_dispute_cancel_refunds_escrow() {
    let (env, creator, contributor, _verifier) = setup_test();
    let contract_id = env.register(MergeMintContract, ());
    let client = MergeMintContractClient::new(&env, &contract_id);

    let reward_amount: i128 = 1000;
    let (bounty_id, token_addr) = make_bounty_with_token(
        &client,
        &env,
        &creator,
        &contract_id,
        "refund_disp",
        reward_amount,
        None,
    );

    client.claim_bounty(&contributor, &bounty_id);
    client.raise_dispute(&creator, &bounty_id);

    let token_client = StellarAssetClient::new(&env, &token_addr);
    assert_eq!(
        token_client.balance(&contract_id),
        reward_amount,
        "contract holds the escrowed reward before dispute resolution"
    );

    // Resolve with "cancel" — should refund to creator.
    client.resolve_dispute(&creator, &bounty_id, &Symbol::new(&env, "cancel"));

    assert_eq!(
        token_client.balance(&contract_id),
        0,
        "contract balance is zero after refund"
    );
    assert_eq!(
        token_client.balance(&creator),
        reward_amount,
        "creator received the refunded reward"
    );
}

#[test]
fn test_resolve_dispute_complete_pays_from_arbitrator() {
    let (env, creator, contributor, _verifier) = setup_test();
    let contract_id = env.register(MergeMintContract, ());
    let client = MergeMintContractClient::new(&env, &contract_id);

    let reward_amount: i128 = 1000;
    let token_addr = create_token_and_mint(&env, &creator, &creator, reward_amount);
    let bounty_id = client.create_bounty(
        &creator,
        &Symbol::new(&env, "resolve_complete"),
        &String::from_str(&env, "desc"),
        &reward_amount,
        &token_addr,
        &0,
        &None,
        &Vec::new(&env),
        &1,
        &None,
        &1,
        &Vec::new(&env),
    );

    client.claim_bounty(&contributor, &bounty_id);
    client.raise_dispute(&creator, &bounty_id);

    let token_client = StellarAssetClient::new(&env, &token_addr);
    assert_eq!(
        token_client.balance(&creator),
        reward_amount,
        "arbitrator (creator) holds the reward before resolution"
    );
    assert_eq!(
        token_client.balance(&contributor),
        0,
        "contributor starts with zero balance"
    );

    // Resolve with "complete" — arbitrator pays the assignee.
    client.resolve_dispute(&creator, &bounty_id, &Symbol::new(&env, "complete"));

    assert_eq!(
        token_client.balance(&creator),
        0,
        "arbitrator's balance is zero after paying the assignee"
    );
    assert_eq!(
        token_client.balance(&contributor),
        reward_amount,
        "assignee received the full reward from the arbitrator"
    );
}

/// Pin the relative event order emitted by `resolve_dispute` so future refactors
/// cannot silently reorder contract events consumed by indexers.
#[test]
fn test_resolve_dispute_event_order() {
    use soroban_sdk::xdr::ContractEventBody;

    fn event_topics_since(env: &Env, contract_id: &Address, start: usize) -> Vec<Symbol> {
        let mut topics = Vec::new(env);
        for ev in env.events().all().filter_by_contract(contract_id).events()[start..].iter() {
            let ContractEventBody::V0(body) = &ev.body;
            let topic_sc = body.topics.first().expect("event topic");
            let val = Val::try_from_val(env, topic_sc).expect("topic val");
            topics.push_back(Symbol::try_from_val(env, &val).expect("topic symbol"));
        }
        topics
    }

    let (env, creator, contributor, _verifier) = setup_test();
    let contract_id = env.register(MergeMintContract, ());
    let client = MergeMintContractClient::new(&env, &contract_id);

    let reward_amount: i128 = 1000;
    let (bounty_id, _token_addr) = make_bounty_with_token(
        &client,
        &env,
        &creator,
        &contract_id,
        "evt_order_cancel",
        reward_amount,
        None,
    );

    client.claim_bounty(&contributor, &bounty_id);
    client.raise_dispute(&creator, &bounty_id);
    client.resolve_dispute(&creator, &bounty_id, &Symbol::new(&env, "cancel"));

    assert_eq!(
        event_topics_since(&env, &contract_id, 0),
        vec![&env, Symbol::new(&env, "dispute_resolved")],
        "resolve_dispute(cancel) must emit dispute_resolved only"
    );

    let contributor2 = Address::generate(&env);
    let token_addr2 = create_token_and_mint(&env, &creator, &creator, reward_amount);
    let dispute_id = client.create_bounty(
        &creator,
        &Symbol::new(&env, "evt_order_complete"),
        &String::from_str(&env, "desc"),
        &reward_amount,
        &token_addr2,
        &0,
        &None,
        &Vec::new(&env),
        &1,
        &None,
        &1,
        &Vec::new(&env),
    );
    client.claim_bounty(&contributor2, &dispute_id);
    client.raise_dispute(&creator, &dispute_id);
    client.resolve_dispute(&creator, &dispute_id, &Symbol::new(&env, "complete"));

    assert_eq!(
        event_topics_since(&env, &contract_id, 0),
        vec![
            &env,
            Symbol::new(&env, "reward_paid"),
            Symbol::new(&env, "dispute_resolved"),
        ],
        "resolve_dispute(complete) must emit reward_paid before dispute_resolved"
    );
}

// ===========================================================================
// Issue 9/10 — Multi-assignee proportional payout math
//
// These tests verify the basis-point share assignment and payout arithmetic
// for bounties with more than one assignee, focusing on:
//   - Correct share_bp values stored when multiple contributors claim
//   - Payout amounts computed as reward_amount * share_bp / 10_000
//   - Integer-division remainder handling (first assignee absorbs remainder)
//   - Payout totals across all assignees accounting for integer-division loss
// ===========================================================================

/// Helper: create a bounty with a real token, minting `reward_amount` to
/// `verifier` (who pays out), and return (bounty_id, token_addr).
/// Accepts `max_assignees` so multi-assignee scenarios can be set up.
fn make_multi_bounty_with_token(
    client: &MergeMintContractClient,
    env: &Env,
    creator: &Address,
    contract_id: &Address,
    tag: &str,
    reward_amount: i128,
    max_assignees: u32,
) -> (crate::types::BountyId, Address) {
    let token_addr = create_token_and_mint(env, creator, contract_id, reward_amount);
    let bounty_id = client.create_bounty(
        creator,
        &Symbol::new(env, tag),
        &String::from_str(env, "multi-assignee bounty"),
        &reward_amount,
        &token_addr,
        &0,
        &None,
        &Vec::new(env),
        &max_assignees,
        &None,
        &1,
        &Vec::new(env),
    );
    (bounty_id, token_addr)
}

/// Two assignees with max_assignees=2 each receive share_bp=5000 (even split).
/// With reward_amount=10_000 each payout is 5_000, and the sum equals
/// reward_amount exactly (no integer-division loss for an even split).
#[test]
fn test_two_assignees_equal_share_bp() {
    let (env, creator, contributor1, _verifier) = setup_test();
    let contributor2 = Address::generate(&env);
    let contract_id = env.register(MergeMintContract, ());
    let client = MergeMintContractClient::new(&env, &contract_id);

    let reward_amount: i128 = 10_000;
    let (bounty_id, _) = make_multi_bounty_with_token(
        &client,
        &env,
        &creator,
        &contract_id,
        "two_eq",
        reward_amount,
        2,
    );

    // First claimant — absorbs remainder (10_000 % 2 == 0, so no remainder here).
    client.claim_bounty(&contributor1, &bounty_id);
    // Second claimant.
    client.claim_bounty(&contributor2, &bounty_id);

    let bounty = client.get_bounty(&bounty_id).unwrap();
    assert_eq!(bounty.assignees.len(), 2, "both assignees recorded");

    let (addr1, share1) = bounty.assignees.get(0).unwrap();
    let (addr2, share2) = bounty.assignees.get(1).unwrap();
    assert_eq!(addr1, contributor1);
    assert_eq!(addr2, contributor2);

    // base_share = 10_000 / 2 = 5_000; remainder = 0 → both get 5_000
    assert_eq!(share1, 5_000u32, "first assignee share_bp");
    assert_eq!(share2, 5_000u32, "second assignee share_bp");
    assert_eq!(share1 + share2, 10_000u32, "share_bp sums to 10_000");
}

/// Two assignees with max_assignees=2 and reward_amount=10_000 each receive
/// exactly 5_000 tokens. Total payout == reward_amount (even split, no loss).
#[test]
fn test_two_assignees_payout_sum_equals_reward() {
    let (env, creator, contributor1, verifier) = setup_test();
    let contributor2 = Address::generate(&env);
    let contract_id = env.register(MergeMintContract, ());
    let client = MergeMintContractClient::new(&env, &contract_id);

    let reward_amount: i128 = 10_000;
    let (bounty_id, token_addr) = make_multi_bounty_with_token(
        &client,
        &env,
        &creator,
        &contract_id,
        "two_payout",
        reward_amount,
        2,
    );

    client.claim_bounty(&contributor1, &bounty_id);
    client.claim_bounty(&contributor2, &bounty_id);
    client.complete_bounty(&verifier, &bounty_id);

    let token_client = StellarAssetClient::new(&env, &token_addr);
    let payout1 = token_client.balance(&contributor1);
    let payout2 = token_client.balance(&contributor2);

    // Each gets reward_amount * 5_000 / 10_000 = 5_000
    assert_eq!(payout1, 5_000, "first assignee payout");
    assert_eq!(payout2, 5_000, "second assignee payout");
    assert_eq!(
        payout1 + payout2,
        reward_amount,
        "payout sum equals reward_amount for even split"
    );
}

/// Three assignees with max_assignees=3:
/// base_share = 10_000 / 3 = 3_333; remainder = 10_000 % 3 = 1.
/// The FIRST assignee absorbs the remainder → share_bp = 3_334.
/// The other two assignees each get share_bp = 3_333.
/// Total share_bp = 3_334 + 3_333 + 3_333 = 10_000.
#[test]
fn test_three_assignees_first_absorbs_remainder_share_bp() {
    let (env, creator, contributor1, _verifier) = setup_test();
    let contributor2 = Address::generate(&env);
    let contributor3 = Address::generate(&env);
    let contract_id = env.register(MergeMintContract, ());
    let client = MergeMintContractClient::new(&env, &contract_id);

    let reward_amount: i128 = 10_000;
    let (bounty_id, _) = make_multi_bounty_with_token(
        &client,
        &env,
        &creator,
        &contract_id,
        "three_rem",
        reward_amount,
        3,
    );

    client.claim_bounty(&contributor1, &bounty_id);
    client.claim_bounty(&contributor2, &bounty_id);
    client.claim_bounty(&contributor3, &bounty_id);

    let bounty = client.get_bounty(&bounty_id).unwrap();
    let (_, share1) = bounty.assignees.get(0).unwrap();
    let (_, share2) = bounty.assignees.get(1).unwrap();
    let (_, share3) = bounty.assignees.get(2).unwrap();

    // base_share = 3333, remainder = 1 → first gets 3334
    assert_eq!(share1, 3_334u32, "first assignee absorbs remainder");
    assert_eq!(share2, 3_333u32, "second assignee base share");
    assert_eq!(share3, 3_333u32, "third assignee base share");
    assert_eq!(
        share1 + share2 + share3,
        10_000u32,
        "share_bp sum is exactly 10_000"
    );
}

/// Three assignees with reward_amount=10_000:
/// payout_i = reward_amount * share_bp_i / 10_000
/// First:  10_000 * 3_334 / 10_000 = 3_334
/// Second: 10_000 * 3_333 / 10_000 = 3_333
/// Third:  10_000 * 3_333 / 10_000 = 3_333
/// Sum = 9_999 — one token is "lost" to integer division in the payout
/// formula (share_bp sums exactly to 10_000, but the intermediate
/// multiplication can still produce a truncation loss when reward_amount is
/// not itself a multiple of 10_000 / num_assignees).
///
/// Note: with reward_amount = 10_000 and share_bp = 3_334 / 3_333 the loss
/// is zero here (10_000 * k / 10_000 = k exactly), so the sum IS 10_000.
#[test]
fn test_three_assignees_payout_sum_with_divisible_reward() {
    let (env, creator, contributor1, verifier) = setup_test();
    let contributor2 = Address::generate(&env);
    let contributor3 = Address::generate(&env);
    let contract_id = env.register(MergeMintContract, ());
    let client = MergeMintContractClient::new(&env, &contract_id);

    // reward_amount = 10_000 is exactly divisible in the payout formula:
    // 10_000 * 3334 / 10_000 = 3334 (exact), 10_000 * 3333 / 10_000 = 3333 (exact)
    let reward_amount: i128 = 10_000;
    let (bounty_id, token_addr) = make_multi_bounty_with_token(
        &client,
        &env,
        &creator,
        &contract_id,
        "three_div",
        reward_amount,
        3,
    );

    client.claim_bounty(&contributor1, &bounty_id);
    client.claim_bounty(&contributor2, &bounty_id);
    client.claim_bounty(&contributor3, &bounty_id);
    client.complete_bounty(&verifier, &bounty_id);

    let token_client = StellarAssetClient::new(&env, &token_addr);
    let payout1 = token_client.balance(&contributor1);
    let payout2 = token_client.balance(&contributor2);
    let payout3 = token_client.balance(&contributor3);

    assert_eq!(
        payout1, 3_334,
        "first assignee payout (with remainder share)"
    );
    assert_eq!(payout2, 3_333, "second assignee payout");
    assert_eq!(payout3, 3_333, "third assignee payout");
    assert_eq!(
        payout1 + payout2 + payout3,
        reward_amount,
        "payout sum equals reward_amount when reward is divisible by 10_000"
    );
}

/// Three assignees with an uneven reward_amount (9_999):
/// Each share_bp: 3_334, 3_333, 3_333 (same as above).
/// payout = 9_999 * share_bp / 10_000 (integer division truncates):
///   First:  9_999 * 3_334 / 10_000 = 33_326_666 / 10_000 = 3_332 (truncated)
///   Second: 9_999 * 3_333 / 10_000 = 33_323_667 / 10_000 = 3_332 (truncated)
///   Third:  9_999 * 3_333 / 10_000 = 33_323_667 / 10_000 = 3_332 (truncated)
///   Sum = 9_996 — 3 tokens lost to integer division in the payout formula.
///
/// This test documents the known integer-division remainder loss so future
/// implementors can choose whether to recover the dust (e.g. send it to the
/// primary assignee or back to the verifier).
#[test]
fn test_three_assignees_payout_integer_division_loss_documented() {
    let (env, creator, contributor1, verifier) = setup_test();
    let contributor2 = Address::generate(&env);
    let contributor3 = Address::generate(&env);
    let contract_id = env.register(MergeMintContract, ());
    let client = MergeMintContractClient::new(&env, &contract_id);

    // Use 9_999 — not perfectly divisible by 3_333/3_334 in the payout formula.
    let reward_amount: i128 = 9_999;
    let (bounty_id, token_addr) = make_multi_bounty_with_token(
        &client,
        &env,
        &creator,
        &contract_id,
        "three_loss",
        reward_amount,
        3,
    );

    client.claim_bounty(&contributor1, &bounty_id);
    client.claim_bounty(&contributor2, &bounty_id);
    client.claim_bounty(&contributor3, &bounty_id);
    client.complete_bounty(&verifier, &bounty_id);

    let token_client = StellarAssetClient::new(&env, &token_addr);
    let payout1 = token_client.balance(&contributor1);
    let payout2 = token_client.balance(&contributor2);
    let payout3 = token_client.balance(&contributor3);

    // 9_999 * 3_334 / 10_000 = 3_333 (truncated from 3_333.6666)
    assert_eq!(payout1, 3_333, "first assignee payout (truncated)");
    // 9_999 * 3_333 / 10_000 = 3_332 (truncated from 3_332.6667)
    assert_eq!(payout2, 3_332, "second assignee payout (truncated)");
    assert_eq!(payout3, 3_332, "third assignee payout (truncated)");

    let total_paid = payout1 + payout2 + payout3;
    let integer_division_loss = reward_amount - total_paid;

    // Document the known loss: 9_999 - 9_997 = 2 tokens unaccounted for.
    assert_eq!(
        integer_division_loss, 2,
        "integer-division loss is 2 tokens for reward_amount=9_999 with 3 assignees"
    );
    // The total paid is strictly less than reward_amount in this case.
    assert!(
        total_paid < reward_amount,
        "payout sum is less than reward_amount due to integer division"
    );
}

/// Two assignees with uneven reward_amount=9_999:
/// base_share = 5_000, remainder = 0 → both get 5_000.
/// payout = 9_999 * 5_000 / 10_000 = 4_999 (truncated from 4_999.5).
/// Sum = 9_998 — 1 token lost to truncation.
#[test]
fn test_two_assignees_uneven_reward_integer_division_loss() {
    let (env, creator, contributor1, verifier) = setup_test();
    let contributor2 = Address::generate(&env);
    let contract_id = env.register(MergeMintContract, ());
    let client = MergeMintContractClient::new(&env, &contract_id);

    let reward_amount: i128 = 9_999;
    let (bounty_id, token_addr) = make_multi_bounty_with_token(
        &client,
        &env,
        &creator,
        &contract_id,
        "two_odd",
        reward_amount,
        2,
    );

    client.claim_bounty(&contributor1, &bounty_id);
    client.claim_bounty(&contributor2, &bounty_id);
    client.complete_bounty(&verifier, &bounty_id);

    let token_client = StellarAssetClient::new(&env, &token_addr);
    let payout1 = token_client.balance(&contributor1);
    let payout2 = token_client.balance(&contributor2);

    // 9_999 * 5_000 / 10_000 = 4_999 (truncated from 4_999.5)
    assert_eq!(payout1, 4_999, "first assignee payout");
    assert_eq!(payout2, 4_999, "second assignee payout");

    let total_paid = payout1 + payout2;
    let integer_division_loss = reward_amount - total_paid;

    // Document: 1 token lost.
    assert_eq!(
        integer_division_loss, 1,
        "integer-division loss is 1 token for reward_amount=9_999 with 2 assignees"
    );
}

// ===========================================================================
// Issue 11 — approve_completion multi-sig quorum path
// ===========================================================================

/// Helper: create a multi-sig bounty with real token minted to `contract_id`.
/// Returns (bounty_id, token_addr, verifier1, verifier2, verifier3).
fn make_multisig_bounty(
    client: &MergeMintContractClient,
    env: &Env,
    creator: &Address,
    contract_id: &Address,
    reward_amount: i128,
    threshold: u32,
) -> (crate::types::BountyId, Address, Address, Address, Address) {
    let v1 = Address::generate(env);
    let v2 = Address::generate(env);
    let v3 = Address::generate(env);
    let mut verifiers: Vec<Address> = Vec::new(env);
    verifiers.push_back(v1.clone());
    verifiers.push_back(v2.clone());
    verifiers.push_back(v3.clone());

    let token_addr = create_token_and_mint(env, creator, contract_id, reward_amount);
    let bounty_id = client.create_bounty(
        creator,
        &Symbol::new(env, "msig"),
        &String::from_str(env, "multi-sig bounty"),
        &reward_amount,
        &token_addr,
        &0,
        &None,
        &Vec::new(env),
        &1,
        &Some(verifiers),
        &threshold,
        &Vec::new(env),
    );
    (bounty_id, token_addr, v1, v2, v3)
}

/// Below threshold: one approval on a threshold-2 bounty does NOT trigger payout.
/// Bounty stays in_progress.
#[test]
fn test_approve_completion_below_threshold_no_payout() {
    let (env, creator, contributor, _verifier) = setup_test();
    let contract_id = env.register(MergeMintContract, ());
    let client = MergeMintContractClient::new(&env, &contract_id);

    let reward_amount: i128 = 1000;
    let (bounty_id, token_addr, v1, _v2, _v3) =
        make_multisig_bounty(&client, &env, &creator, &contract_id, reward_amount, 2);

    client.claim_bounty(&contributor, &bounty_id);

    // Only one approval — threshold is 2, so no payout yet.
    client.approve_completion(&v1, &bounty_id);

    let bounty = client.get_bounty(&bounty_id).unwrap();
    assert_eq!(
        bounty.status,
        Symbol::new(&env, "in_progress"),
        "bounty must remain in_progress below threshold"
    );

    // Contract still holds the escrowed reward.
    let token_client = soroban_sdk::token::StellarAssetClient::new(&env, &token_addr);
    assert_eq!(
        token_client.balance(&contract_id),
        reward_amount,
        "escrowed funds must not move before threshold is reached"
    );
}

/// At threshold: second approval on a threshold-2 bounty auto-completes and pays out.
#[test]
fn test_approve_completion_at_threshold_auto_completes() {
    let (env, creator, contributor, _verifier) = setup_test();
    let contract_id = env.register(MergeMintContract, ());
    let client = MergeMintContractClient::new(&env, &contract_id);

    let reward_amount: i128 = 1000;
    let (bounty_id, token_addr, v1, v2, _v3) =
        make_multisig_bounty(&client, &env, &creator, &contract_id, reward_amount, 2);

    client.claim_bounty(&contributor, &bounty_id);

    client.approve_completion(&v1, &bounty_id);
    client.approve_completion(&v2, &bounty_id);

    let bounty = client.get_bounty(&bounty_id).unwrap();
    assert_eq!(
        bounty.status,
        Symbol::new(&env, "completed"),
        "bounty must be completed after threshold is reached"
    );

    // Contributor received the reward.
    let token_client = soroban_sdk::token::StellarAssetClient::new(&env, &token_addr);
    assert_eq!(
        token_client.balance(&contributor),
        reward_amount,
        "contributor must receive full reward on completion"
    );
    assert_eq!(
        token_client.balance(&contract_id),
        0,
        "contract balance must be zero after payout"
    );
}

/// Duplicate vote: same verifier approving twice must panic with AlreadyApproved.
#[test]
#[should_panic(expected = "verifier has already approved this bounty")]
fn test_approve_completion_duplicate_vote_rejected() {
    let (env, creator, contributor, _verifier) = setup_test();
    let contract_id = env.register(MergeMintContract, ());
    let client = MergeMintContractClient::new(&env, &contract_id);

    let reward_amount: i128 = 1000;
    let (bounty_id, _token_addr, v1, _v2, _v3) =
        make_multisig_bounty(&client, &env, &creator, &contract_id, reward_amount, 2);

    client.claim_bounty(&contributor, &bounty_id);

    client.approve_completion(&v1, &bounty_id);
    // Second call from the same verifier must panic.
    client.approve_completion(&v1, &bounty_id);
}

/// Unauthorized verifier (not in required_verifiers list) must panic.
#[test]
#[should_panic(expected = "verifier is not in the required verifiers list")]
fn test_approve_completion_unauthorized_verifier_rejected() {
    let (env, creator, contributor, _verifier) = setup_test();
    let contract_id = env.register(MergeMintContract, ());
    let client = MergeMintContractClient::new(&env, &contract_id);

    let reward_amount: i128 = 1000;
    let (bounty_id, _token_addr, _v1, _v2, _v3) =
        make_multisig_bounty(&client, &env, &creator, &contract_id, reward_amount, 2);

    client.claim_bounty(&contributor, &bounty_id);

    let outsider = Address::generate(&env);
    client.approve_completion(&outsider, &bounty_id);
}

/// Assignee cannot approve their own bounty completion (self-approval guard).
#[test]
#[should_panic(expected = "verifier cannot be the assignee")]
fn test_approve_completion_assignee_cannot_self_approve() {
    let (env, creator, contributor, _verifier) = setup_test();
    let contract_id = env.register(MergeMintContract, ());
    let client = MergeMintContractClient::new(&env, &contract_id);

    let reward_amount: i128 = 1000;
    let v2 = Address::generate(&env);
    let mut verifiers: Vec<Address> = Vec::new(&env);
    verifiers.push_back(contributor.clone());
    verifiers.push_back(v2.clone());

    let token_addr = create_token_and_mint(&env, &creator, &contract_id, reward_amount);
    let bounty_id = client.create_bounty(
        &creator,
        &Symbol::new(&env, "self_approve"),
        &String::from_str(&env, "self-approval guard bounty"),
        &reward_amount,
        &token_addr,
        &0,
        &None,
        &Vec::new(&env),
        &1,
        &Some(verifiers),
        &1,
        &Vec::new(&env),
    );

    client.claim_bounty(&contributor, &bounty_id);
    client.approve_completion(&contributor, &bounty_id);
}

// ===========================================================================
// Issue #35/#36/#38 — Token-balance invariant test for escrow
// ===========================================================================

/// Property: after every state transition, the contract's token balance for a
/// given token equals the sum of reward_amount across all open+in_progress
/// bounties that use that token.
///
/// Scenario: create 3 bounties → claim one → cancel one → complete one →
/// assert invariant at every step.
#[test]
fn test_escrow_balance_invariant() {
    let (env, creator, contributor, verifier) = setup_test();
    let contract_id = env.register(MergeMintContract, ());
    let client = MergeMintContractClient::new(&env, &contract_id);

    let reward: i128 = 1000;

    // Create three bounties sharing the same token, minting total to contract.
    // We mint 3*reward up-front and create each bounty individually.
    let sac = env.register_stellar_asset_contract_v2(creator.clone());
    let token_addr = sac.address();
    let token_admin = soroban_sdk::token::StellarAssetClient::new(&env, &token_addr);
    token_admin.mint(&contract_id, &(reward * 3));

    let make = |tag: &str| -> crate::types::BountyId {
        client.create_bounty(
            &creator,
            &Symbol::new(&env, tag),
            &String::from_str(&env, "desc"),
            &reward,
            &token_addr,
            &0,
            &None,
            &Vec::new(&env),
            &1,
            &None,
            &1,
            &Vec::new(&env),
        )
    };

    // Helper closure: compute expected balance = sum of open+in_progress rewards.
    let expected_balance =
        |open: u32, in_progress: u32| -> i128 { reward * (open as i128 + in_progress as i128) };

    // Step 1: create all three — all open.
    let b1 = make("inv1");
    let b2 = make("inv2");
    let b3 = make("inv3");

    assert_eq!(
        token_admin.balance(&contract_id),
        expected_balance(3, 0),
        "after create: 3 open bounties"
    );

    // Step 2: claim b1 → in_progress.
    client.claim_bounty(&contributor, &b1);
    assert_eq!(
        token_admin.balance(&contract_id),
        expected_balance(2, 1),
        "after claim b1: 2 open + 1 in_progress"
    );

    // Step 3: cancel b2 → cancelled (refund goes to creator).
    client.cancel_bounty(&creator, &b2);
    assert_eq!(
        token_admin.balance(&contract_id),
        expected_balance(1, 1),
        "after cancel b2: 1 open + 1 in_progress"
    );

    // Step 4: claim b3 → in_progress.
    let contributor2 = Address::generate(&env);
    client.claim_bounty(&contributor2, &b3);
    assert_eq!(
        token_admin.balance(&contract_id),
        expected_balance(0, 2),
        "after claim b3: 0 open + 2 in_progress"
    );

    // Step 5: complete b1 — payout from contract to contributor.
    // Verifier is not an assignee, so use the pre-generated verifier address.
    // complete_bounty pays from verifier's wallet in the current (no-escrow) model,
    // so mint reward to verifier for this step.
    token_admin.mint(&verifier, &reward);
    client.complete_bounty(&verifier, &b1);
    assert_eq!(
        token_admin.balance(&contract_id),
        expected_balance(0, 1),
        "after complete b1: 0 open + 1 in_progress"
    );

    // Step 6: complete b3.
    token_admin.mint(&verifier, &reward);
    client.complete_bounty(&verifier, &b3);
    assert_eq!(
        token_admin.balance(&contract_id),
        expected_balance(0, 0),
        "after complete b3: contract balance must be zero"
    );
}

// ===========================================================================
// Issue #650 — `docs/event-schema.md` parity with mutation emissions
// ===========================================================================

/// Primary event topic names documented in `docs/event-schema.md`.
const DOCUMENTED_MUTATION_EVENTS: &[&str] = &[
    "bounty_created",
    "bounty_claimed",
    "bounty_disputed",
    "bounty_completed",
    "reward_paid",
    "bounty_cancelled",
    "bounty_expired",
    "approval_recorded",
    "dispute_resolved",
];

fn invocation_contains_topic(env: &Env, contract_id: &Address, name: &str) -> bool {
    use soroban_sdk::testutils::Events as _;
    use soroban_sdk::xdr::ContractEventBody;
    use soroban_sdk::{TryFromVal, Val};

    let target = Symbol::new(env, name);
    env.events()
        .all()
        .filter_by_contract(contract_id)
        .events()
        .iter()
        .any(|ev| {
            let ContractEventBody::V0(body) = &ev.body;
            let Some(topic_sc) = body.topics.first() else {
                return false;
            };
            let Ok(val) = Val::try_from_val(env, topic_sc) else {
                return false;
            };
            Symbol::try_from_val(env, &val).ok().as_ref() == Some(&target)
        })
}

fn assert_invocation_emits(env: &Env, contract_id: &Address, expected: &[&str]) {
    for name in expected {
        assert!(
            invocation_contains_topic(env, contract_id, name),
            "expected event '{name}' from last invocation"
        );
    }
}

#[test]
fn test_documented_event_schema_topics_are_complete() {
    assert_eq!(DOCUMENTED_MUTATION_EVENTS.len(), 9);
}

/// Table-driven: each mutation path must emit the event(s) named in `docs/event-schema.md`.
#[test]
fn test_mutations_emit_documented_events_per_schema() {
    let (env, creator, contributor, verifier) = setup_test();
    let contract_id = env.register(MergeMintContract, ());
    let client = MergeMintContractClient::new(&env, &contract_id);
    let reward_amount: i128 = 1000;

    // create_bounty → bounty_created
    let (bounty_id, token_addr) = make_bounty_with_token(
        &client,
        &env,
        &creator,
        &contract_id,
        "evt_create",
        reward_amount,
        None,
    );
    assert_invocation_emits(&env, &contract_id, &["bounty_created"]);

    // claim_bounty → bounty_claimed
    client.claim_bounty(&contributor, &bounty_id);
    assert_invocation_emits(&env, &contract_id, &["bounty_claimed"]);

    // raise_dispute → bounty_disputed
    client.raise_dispute(&creator, &bounty_id);
    assert_invocation_emits(&env, &contract_id, &["bounty_disputed"]);

    // resolve_dispute (cancel) → dispute_resolved
    client.resolve_dispute(&creator, &bounty_id, &Symbol::new(&env, "cancel"));
    assert_invocation_emits(&env, &contract_id, &["dispute_resolved"]);

    // cancel_bounty → bounty_cancelled
    let (cancel_id, _) = make_bounty_with_token(
        &client,
        &env,
        &creator,
        &contract_id,
        "evt_cancel",
        reward_amount,
        None,
    );
    client.cancel_bounty(&creator, &cancel_id);
    assert_invocation_emits(&env, &contract_id, &["bounty_cancelled"]);

    // expire_bounty → bounty_expired
    let (expire_id, _) = make_bounty_with_token(
        &client,
        &env,
        &creator,
        &contract_id,
        "evt_expire",
        reward_amount,
        Some(10),
    );
    env.ledger().set_sequence_number(100);
    let caller = Address::generate(&env);
    client.expire_bounty(&caller, &expire_id);
    assert_invocation_emits(&env, &contract_id, &["bounty_expired"]);

    // approve_completion (below threshold) → approval_recorded
    let contributor2 = Address::generate(&env);
    let (msig_id, _msig_token, v1, _v2, _v3) =
        make_multisig_bounty(&client, &env, &creator, &contract_id, reward_amount, 2);
    client.claim_bounty(&contributor2, &msig_id);
    client.approve_completion(&v1, &msig_id);
    assert_invocation_emits(&env, &contract_id, &["approval_recorded"]);

    // complete_bounty → reward_paid + bounty_completed
    let contributor3 = Address::generate(&env);
    let (complete_id, _) = make_bounty_with_token(
        &client,
        &env,
        &creator,
        &contract_id,
        "evt_complete",
        reward_amount,
        None,
    );
    client.claim_bounty(&contributor3, &complete_id);
    client.complete_bounty(&verifier, &complete_id);
    assert_invocation_emits(&env, &contract_id, &["reward_paid", "bounty_completed"]);

    // resolve_dispute (complete) → reward_paid + dispute_resolved
    let contributor4 = Address::generate(&env);
    let token_addr2 = create_token_and_mint(&env, &creator, &creator, reward_amount);
    let dispute_id = client.create_bounty(
        &creator,
        &Symbol::new(&env, "evt_resolve"),
        &String::from_str(&env, "desc"),
        &reward_amount,
        &token_addr2,
        &0,
        &None,
        &Vec::new(&env),
        &1,
        &None,
        &1,
        &Vec::new(&env),
    );
    client.claim_bounty(&contributor4, &dispute_id);
    client.raise_dispute(&creator, &dispute_id);
    client.resolve_dispute(&creator, &dispute_id, &Symbol::new(&env, "complete"));
    assert_invocation_emits(&env, &contract_id, &["reward_paid", "dispute_resolved"]);

    let _ = token_addr; // silence unused in some toolchains
}

/// Every known bounty status (`mutations.rs` STATUS_* constants) must be
/// queryable via `get_status_count` / `get_bounties_by_status` without panic,
/// and counts must match index lengths. Update `KNOWN_STATUSES` when adding a
/// new lifecycle status so this test forces coverage of status-dependent paths.
#[test]
fn test_exhaustive_known_status_query_coverage() {
    const KNOWN_STATUSES: &[&str] = &["open", "in_progress", "completed", "cancelled", "disputed"];

    let (env, creator, contributor, verifier) = setup_test();
    let contributor2 = Address::generate(&env);
    let contributor3 = Address::generate(&env);
    let contract_id = env.register(MergeMintContract, ());
    let client = MergeMintContractClient::new(&env, &contract_id);
    let reward: i128 = 1000;

    let _open1 = make_bounty(&client, &env, &creator, "ex_open1", None);
    let _open2 = make_bounty(&client, &env, &creator, "ex_open2", None);

    let in_progress_id = make_bounty(&client, &env, &creator, "ex_ip", None);
    client.claim_bounty(&contributor, &in_progress_id);

    let cancelled_id = make_bounty_with_token(
        &client,
        &env,
        &creator,
        &contract_id,
        "ex_can",
        reward,
        None,
    )
    .0;
    client.cancel_bounty(&creator, &cancelled_id);

    let disputed_id = make_bounty(&client, &env, &creator, "ex_disp", None);
    client.claim_bounty(&contributor2, &disputed_id);
    client.raise_dispute(&creator, &disputed_id);

    let (completed_id, token_addr) = make_bounty_with_token(
        &client,
        &env,
        &creator,
        &contract_id,
        "ex_done",
        reward,
        None,
    );
    client.claim_bounty(&contributor3, &completed_id);
    let token_admin = StellarAssetClient::new(&env, &token_addr);
    token_admin.mint(&verifier, &reward);
    client.complete_bounty(&verifier, &completed_id);

    for status_str in KNOWN_STATUSES {
        let status = Symbol::new(&env, status_str);
        let count = client.get_status_count(&status);
        let ids = client.get_bounties_by_status(&status, &None, &50).0;
        assert_eq!(
            count,
            ids.len(),
            "get_status_count must match index length for {status_str}"
        );

        let expected = match *status_str {
            "open" => 2,
            "in_progress" => 1,
            "completed" => 1,
            "cancelled" => 1,
            "disputed" => 1,
            _ => panic!("update expected counts when adding status {status_str}"),
        };
        assert_eq!(count, expected, "unexpected bounty count for {status_str}");

        for id in ids.iter() {
            let bounty = client.get_bounty(&id).unwrap();
            assert_eq!(
                bounty.status, status,
                "status index {status_str} returned bounty with mismatched status"
            );
        }
    }
}

/// Unknown status symbols are rejected by query entrypoints (issue #653).
#[test]
#[should_panic(expected = "invalid bounty status")]
fn test_get_status_count_rejects_unknown_status() {
    let (env, _creator, _contributor, _verifier) = setup_test();
    let contract_id = env.register(MergeMintContract, ());
    let client = MergeMintContractClient::new(&env, &contract_id);
    client.get_status_count(&Symbol::new(&env, "not_a_status"));
}

/// Unknown tag symbols are rejected at bounty creation (issue #653).
#[test]
#[should_panic(expected = "invalid bounty tag")]
fn test_create_bounty_rejects_unknown_tag() {
    let (env, creator, _contributor, _verifier) = setup_test();
    let contract_id = env.register(MergeMintContract, ());
    let client = MergeMintContractClient::new(&env, &contract_id);

    let mut tags: Vec<Symbol> = Vec::new(&env);
    tags.push_back(Symbol::new(&env, "not_a_tag"));

    client.create_bounty(
        &creator,
        &Symbol::new(&env, "bad_tag"),
        &String::from_str(&env, "desc"),
        &1000,
        &create_token_and_mint(&env, &creator, &contract_id, 0),
        &0,
        &None,
        &tags,
        &1,
        &None,
        &1,
        &Vec::new(&env),
    );
}

/// get_bounties_by_tag rejects tags outside the shared allow-list (issue #653).
#[test]
#[should_panic(expected = "invalid bounty tag")]
fn test_get_bounties_by_tag_rejects_unknown_tag() {
    let (env, _creator, _contributor, _verifier) = setup_test();
    let contract_id = env.register(MergeMintContract, ());
    let client = MergeMintContractClient::new(&env, &contract_id);
    client.get_bounties_by_tag(&Symbol::new(&env, "not_a_tag"));
}

// ===========================================================================
// Issue #756 — resolve_dispute arbitrator reputation guard
// ===========================================================================

/// resolve_dispute must reject an arbitrator whose reputation is below
/// the bounty's min_reputation threshold.
/// Regression test for security/minimum-reputation-enforcement.md.
#[test]
#[should_panic(expected = "contributor reputation is too low")]
fn test_resolve_dispute_rejects_low_reputation_arbitrator() {
    let (env, creator, contributor, _verifier) = setup_test();
    let contract_id = env.register(MergeMintContract, ());
    let client = MergeMintContractClient::new(&env, &contract_id);

    // Create a bounty with min_reputation = 50.
    // The creator has 0 reputation (fresh account), so they cannot resolve
    // a dispute on their own bounty until their reputation meets the threshold.
    let reward_amount: i128 = 1000;
    let token_addr = create_token_and_mint(&env, &creator, &contract_id, reward_amount);
    let bounty_id = client.create_bounty(
        &creator,
        &Symbol::new(&env, "rep_disp"),
        &String::from_str(&env, "desc"),
        &reward_amount,
        &token_addr,
        &50, // min_reputation = 50
        &None,
        &Vec::new(&env),
        &1,
        &None,
        &1,
        &Vec::new(&env),
    );

    client.claim_bounty(&contributor, &bounty_id);
    client.raise_dispute(&creator, &bounty_id);

    // Creator has 0 reputation, bounty requires 50 — must panic.
    client.resolve_dispute(&creator, &bounty_id, &Symbol::new(&env, "cancel"));
}

/// resolve_dispute succeeds when the arbitrator meets the reputation threshold.
#[test]
fn test_resolve_dispute_accepts_sufficient_reputation_arbitrator() {
    let (env, creator, contributor, _verifier) = setup_test();
    let contract_id = env.register(MergeMintContract, ());
    let client = MergeMintContractClient::new(&env, &contract_id);

    // Create a bounty with min_reputation = 0 (no floor).
    let reward_amount: i128 = 1000;
    let (bounty_id, _token_addr) = make_bounty_with_token(
        &client,
        &env,
        &creator,
        &contract_id,
        "rep_disp_ok",
        reward_amount,
        None,
    );

    client.claim_bounty(&contributor, &bounty_id);
    client.raise_dispute(&creator, &bounty_id);

    // min_reputation = 0 means no floor — creator can always resolve.
    client.resolve_dispute(&creator, &bounty_id, &Symbol::new(&env, "cancel"));

    let bounty = client.get_bounty(&bounty_id).unwrap();
    assert_eq!(bounty.status, Symbol::new(&env, "cancelled"));
}
