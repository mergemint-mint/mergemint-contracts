// SPDX-License-Identifier: MIT
#![cfg(all(test, feature = "fuzz"))]

extern crate std;

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::string::{String as StdString, ToString};

use proptest::prelude::*;
use soroban_sdk::{
    testutils::{Address as _, EnvTestConfig, Ledger as _},
    Address, Env, String as SorobanString, Symbol, Vec as SorobanVec,
};

use crate::contract::MergeMintContract;
use crate::types::Milestone;
use crate::MergeMintContractClient;

pub const ALLOWED_TAGS: &[&str] = &[
    "bug", "docs", "feature", "security", "test", "refactor", "design", "chore", "perf", "other",
];

const INVALID_TAG_CANDIDATES: &[&str] = &[
    "invalid",
    "fake",
    "bad_tag",
    "hacker",
    "unknown",
    "arbitrary",
    "random_tag",
    "tag123",
];

/// Initialises an isolated Soroban testing environment with an unregistered snapshot policy to prevent disk overhead.
fn create_test_env() -> (Env, MergeMintContractClient<'static>, Address, Address) {
    let env = Env::new_with_config(EnvTestConfig {
        capture_snapshot_at_drop: false,
    });
    env.mock_all_auths();
    let contract_id = env.register(MergeMintContract, ());
    let client = MergeMintContractClient::new(&env, &contract_id);
    let creator = Address::generate(&env);
    let sac = env.register_stellar_asset_contract_v2(creator.clone());
    let token_addr = sac.address();
    (env, client, creator, token_addr)
}

/// Executes a closure that is expected to trigger a contract failure and asserts the panic diagnostic contains the expected substring.
fn assert_contract_panic<F>(f: F, expected_substring: &str)
where
    F: FnOnce(),
{
    let result = catch_unwind(AssertUnwindSafe(f));
    assert!(
        result.is_err(),
        "Expected panic containing '{}', but call succeeded",
        expected_substring
    );
    let err = result.unwrap_err();
    let msg = if let Some(s) = err.downcast_ref::<&str>() {
        s.to_string()
    } else if let Some(s) = err.downcast_ref::<StdString>() {
        s.clone()
    } else {
        StdString::new()
    };
    assert!(
        msg.contains(expected_substring),
        "Expected panic message to contain '{}', but got: '{}'",
        expected_substring,
        msg
    );
}

/// Returns a proptest strategy generating diverse valid reward amounts including boundary values.
fn valid_reward_strategy() -> impl Strategy<Value = i128> {
    prop_oneof![
        10 => 100i128..=10_000i128,
        10 => 10_001i128..=1_000_000_000i128,
        5 => 1_000_000_001i128..=(i128::MAX - 1000),
        2 => Just(100i128),
        2 => Just(i128::MAX),
    ]
}

/// Returns an invalid tag string by index.
fn invalid_tag_candidate_str(idx: usize) -> &'static str {
    INVALID_TAG_CANDIDATES[idx % INVALID_TAG_CANDIDATES.len()]
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 256, ..ProptestConfig::default() })]

    #[test]
    fn prop_valid_bounties_invariants(
        reward in valid_reward_strategy(),
        tag_indices in proptest::collection::vec(0usize..10, 0..=5),
        max_assignees in 1u32..=500,
        deadline_offset in proptest::option::of(0u32..=1_000_000),
        num_verifiers in 0usize..=4,
        has_milestones in any::<bool>(),
        num_milestones in 1usize..=4,
        min_reputation in 0u32..=10_000,
    ) {
        let (env, client, creator, token_addr) = create_test_env();

        let mut tags = SorobanVec::new(&env);
        for &idx in &tag_indices {
            tags.push_back(Symbol::new(&env, ALLOWED_TAGS[idx]));
        }

        let required_verifiers = if num_verifiers > 0 {
            let mut v = SorobanVec::new(&env);
            for _ in 0..num_verifiers {
                v.push_back(Address::generate(&env));
            }
            Some(v)
        } else {
            None
        };

        let approval_threshold = if let Some(ref v) = required_verifiers {
            v.len() / 2 + 1
        } else {
            0
        };

        let current_seq = env.ledger().sequence();
        let deadline = deadline_offset.map(|offset| current_seq + offset);

        let mut milestones = SorobanVec::new(&env);
        if has_milestones && reward >= num_milestones as i128 {
            let base = reward / (num_milestones as i128);
            let remainder = reward % (num_milestones as i128);
            for i in 0..num_milestones {
                let r = if i == 0 { base + remainder } else { base };
                milestones.push_back(Milestone {
                    description: Symbol::new(&env, "m"),
                    reward: r,
                    completed: false,
                });
            }
        }

        let title = Symbol::new(&env, "bounty");
        let description = SorobanString::from_str(&env, "fuzz test bounty");
        let initial_count = client.get_bounty_count();

        let id = client.create_bounty(
            &creator,
            &title,
            &description,
            &reward,
            &token_addr,
            &min_reputation,
            &deadline,
            &tags,
            &max_assignees,
            &required_verifiers,
            &approval_threshold,
            &milestones,
        );

        prop_assert_eq!(client.get_bounty_count(), initial_count + 1);

        let bounty = client.get_bounty(&id).expect("bounty must exist");

        prop_assert_eq!(bounty.creator, creator);
        prop_assert_eq!(bounty.reward_amount, reward);
        prop_assert_eq!(bounty.reward_token, token_addr);
        prop_assert_eq!(bounty.status, Symbol::new(&env, "open"));
        prop_assert!(bounty.assignees.is_empty());
        prop_assert_eq!(bounty.max_assignees, max_assignees);
        prop_assert_eq!(bounty.min_reputation, min_reputation);
        prop_assert_eq!(bounty.deadline, deadline);
        prop_assert!(bounty.tags.len() <= 5);
        prop_assert_eq!(bounty.tags.len(), tags.len());
        for i in 0..tags.len() {
            prop_assert_eq!(bounty.tags.get(i).unwrap(), tags.get(i).unwrap());
        }
        prop_assert_eq!(bounty.approval_threshold, approval_threshold);
        if let Some(ref v) = required_verifiers {
            let stored_v = bounty.required_verifiers.as_ref().expect("verifiers must be stored");
            prop_assert_eq!(stored_v.len(), v.len());
            prop_assert!(bounty.approval_threshold <= stored_v.len());
        }
        prop_assert_eq!(bounty.milestones.len(), milestones.len());
        if !bounty.milestones.is_empty() {
            let mut sum: i128 = 0;
            for m in bounty.milestones.iter() {
                sum = sum.checked_add(m.reward).expect("milestone sum overflow");
            }
            prop_assert_eq!(sum, bounty.reward_amount);
        }

        let mut ids = SorobanVec::new(&env);
        ids.push_back(id.clone());
        let metas = client.get_bounty_metas(&ids);
        let meta = metas.get(0).unwrap().expect("meta must exist");
        prop_assert_eq!(meta.title, title);
        prop_assert_eq!(meta.description, description);
    }

    #[test]
    fn prop_negative_or_zero_reward_rejected(
        reward in prop_oneof![
            Just(0i128),
            -1_000_000_000i128..=-1i128,
            Just(i128::MIN),
        ]
    ) {
        let (env, client, creator, token_addr) = create_test_env();
        let res = client.try_create_bounty(
            &creator,
            &Symbol::new(&env, "bug"),
            &SorobanString::from_str(&env, "desc"),
            &reward,
            &token_addr,
            &0,
            &None,
            &SorobanVec::new(&env),
            &1,
            &None,
            &1,
            &SorobanVec::new(&env),
        );
        prop_assert!(res.is_err(), "Expected error for negative or zero reward: {}", reward);
    }

    #[test]
    fn prop_below_minimum_reward_rejected(
        reward in 1i128..100i128
    ) {
        let (env, client, creator, token_addr) = create_test_env();
        let res = client.try_create_bounty(
            &creator,
            &Symbol::new(&env, "bug"),
            &SorobanString::from_str(&env, "desc"),
            &reward,
            &token_addr,
            &0,
            &None,
            &SorobanVec::new(&env),
            &1,
            &None,
            &1,
            &SorobanVec::new(&env),
        );
        prop_assert!(res.is_err(), "Expected error for reward below minimum: {}", reward);
    }

    #[test]
    fn prop_milestone_sum_overflow_rejected(
        offset in 1i128..=10_000i128,
        extra in 1i128..=10_000i128,
    ) {
        let (env, client, creator, token_addr) = create_test_env();

        let mut milestones = SorobanVec::new(&env);
        milestones.push_back(Milestone {
            description: Symbol::new(&env, "m1"),
            reward: i128::MAX - offset,
            completed: false,
        });
        milestones.push_back(Milestone {
            description: Symbol::new(&env, "m2"),
            reward: offset + extra,
            completed: false,
        });

        let res = client.try_create_bounty(
            &creator,
            &Symbol::new(&env, "bug"),
            &SorobanString::from_str(&env, "desc"),
            &1000,
            &token_addr,
            &0,
            &None,
            &SorobanVec::new(&env),
            &1,
            &None,
            &1,
            &milestones,
        );
        prop_assert!(res.is_err(), "Expected error for milestone sum overflow");
    }

    #[test]
    fn prop_milestone_sum_mismatch_rejected(
        m1 in 50i128..=10_000i128,
        m2 in 50i128..=10_000i128,
        delta in prop_oneof![1i128..=500i128, -500i128..=-1i128],
    ) {
        let (env, client, creator, token_addr) = create_test_env();

        let mut milestones = SorobanVec::new(&env);
        milestones.push_back(Milestone {
            description: Symbol::new(&env, "m1"),
            reward: m1,
            completed: false,
        });
        milestones.push_back(Milestone {
            description: Symbol::new(&env, "m2"),
            reward: m2,
            completed: false,
        });

        let sum = m1 + m2;
        let mismatched_reward = if sum + delta >= 100 { sum + delta } else { sum + 100 };

        let res = client.try_create_bounty(
            &creator,
            &Symbol::new(&env, "bug"),
            &SorobanString::from_str(&env, "desc"),
            &mismatched_reward,
            &token_addr,
            &0,
            &None,
            &SorobanVec::new(&env),
            &1,
            &None,
            &1,
            &milestones,
        );
        prop_assert!(res.is_err(), "Expected error for milestone sum mismatch");
    }

    #[test]
    fn prop_too_many_tags_rejected(
        tag_count in 6usize..=15usize,
    ) {
        let (env, client, creator, token_addr) = create_test_env();

        let mut tags = SorobanVec::new(&env);
        for i in 0..tag_count {
            tags.push_back(Symbol::new(&env, ALLOWED_TAGS[i % ALLOWED_TAGS.len()]));
        }

        let res = client.try_create_bounty(
            &creator,
            &Symbol::new(&env, "bug"),
            &SorobanString::from_str(&env, "desc"),
            &1000,
            &token_addr,
            &0,
            &None,
            &tags,
            &1,
            &None,
            &1,
            &SorobanVec::new(&env),
        );
        prop_assert!(res.is_err(), "Expected error for tag count > 5: {}", tag_count);
    }

    #[test]
    fn prop_invalid_tag_rejected(
        invalid_idx in 0usize..8usize,
    ) {
        let (env, client, creator, token_addr) = create_test_env();

        let mut tags = SorobanVec::new(&env);
        tags.push_back(Symbol::new(&env, invalid_tag_candidate_str(invalid_idx)));

        let res = client.try_create_bounty(
            &creator,
            &Symbol::new(&env, "bug"),
            &SorobanString::from_str(&env, "desc"),
            &1000,
            &token_addr,
            &0,
            &None,
            &tags,
            &1,
            &None,
            &1,
            &SorobanVec::new(&env),
        );
        prop_assert!(res.is_err(), "Expected error for invalid tag");
    }

    #[test]
    fn prop_max_assignees_zero_rejected(
        reward in 100i128..=10_000i128,
    ) {
        let (env, client, creator, token_addr) = create_test_env();
        let res = client.try_create_bounty(
            &creator,
            &Symbol::new(&env, "bug"),
            &SorobanString::from_str(&env, "desc"),
            &reward,
            &token_addr,
            &0,
            &None,
            &SorobanVec::new(&env),
            &0,
            &None,
            &1,
            &SorobanVec::new(&env),
        );
        prop_assert!(res.is_err(), "Expected error for max_assignees = 0");
    }

    #[test]
    fn prop_past_deadline_rejected(
        seq in 10u32..=10_000u32,
        offset in 1u32..=10u32,
    ) {
        let (env, client, creator, token_addr) = create_test_env();
        env.ledger().set_sequence_number(seq);

        let past_deadline = seq.saturating_sub(offset);

        let res = client.try_create_bounty(
            &creator,
            &Symbol::new(&env, "bug"),
            &SorobanString::from_str(&env, "desc"),
            &1000,
            &token_addr,
            &0,
            &Some(past_deadline),
            &SorobanVec::new(&env),
            &1,
            &None,
            &1,
            &SorobanVec::new(&env),
        );
        prop_assert!(res.is_err(), "Expected error for past deadline: {}", past_deadline);
    }

    #[test]
    fn prop_approval_threshold_exceeds_verifiers_rejected(
        num_verifiers in 0usize..=4usize,
        excess in 1u32..=50u32,
    ) {
        let (env, client, creator, token_addr) = create_test_env();

        let mut verifiers = SorobanVec::new(&env);
        for _ in 0..num_verifiers {
            verifiers.push_back(Address::generate(&env));
        }

        let threshold = (num_verifiers as u32) + excess;

        let res = client.try_create_bounty(
            &creator,
            &Symbol::new(&env, "bug"),
            &SorobanString::from_str(&env, "desc"),
            &1000,
            &token_addr,
            &0,
            &None,
            &SorobanVec::new(&env),
            &1,
            &Some(verifiers),
            &threshold,
            &SorobanVec::new(&env),
        );
        prop_assert!(res.is_err(), "Expected error for approval threshold > verifiers len");
    }
}

#[test]
fn test_exact_error_messages() {
    let (env, client, creator, token_addr) = create_test_env();

    assert_contract_panic(
        || {
            client.create_bounty(
                &creator,
                &Symbol::new(&env, "bug"),
                &SorobanString::from_str(&env, "desc"),
                &0,
                &token_addr,
                &0,
                &None,
                &SorobanVec::new(&env),
                &1,
                &None,
                &1,
                &SorobanVec::new(&env),
            );
        },
        "reward_amount must be positive",
    );

    assert_contract_panic(
        || {
            client.create_bounty(
                &creator,
                &Symbol::new(&env, "bug"),
                &SorobanString::from_str(&env, "desc"),
                &99,
                &token_addr,
                &0,
                &None,
                &SorobanVec::new(&env),
                &1,
                &None,
                &1,
                &SorobanVec::new(&env),
            );
        },
        "reward_amount is below the minimum allowed",
    );

    let mut too_many = SorobanVec::new(&env);
    for _ in 0..6 {
        too_many.push_back(Symbol::new(&env, "bug"));
    }
    assert_contract_panic(
        || {
            client.create_bounty(
                &creator,
                &Symbol::new(&env, "bug"),
                &SorobanString::from_str(&env, "desc"),
                &100,
                &token_addr,
                &0,
                &None,
                &too_many,
                &1,
                &None,
                &1,
                &SorobanVec::new(&env),
            );
        },
        "too many tags",
    );

    let mut bad_tag = SorobanVec::new(&env);
    bad_tag.push_back(Symbol::new(&env, "invalid"));
    assert_contract_panic(
        || {
            client.create_bounty(
                &creator,
                &Symbol::new(&env, "bug"),
                &SorobanString::from_str(&env, "desc"),
                &100,
                &token_addr,
                &0,
                &None,
                &bad_tag,
                &1,
                &None,
                &1,
                &SorobanVec::new(&env),
            );
        },
        "invalid bounty tag",
    );

    assert_contract_panic(
        || {
            client.create_bounty(
                &creator,
                &Symbol::new(&env, "bug"),
                &SorobanString::from_str(&env, "desc"),
                &100,
                &token_addr,
                &0,
                &None,
                &SorobanVec::new(&env),
                &0,
                &None,
                &1,
                &SorobanVec::new(&env),
            );
        },
        "max_assignees must be at least 1",
    );

    env.ledger().set_sequence_number(100);
    assert_contract_panic(
        || {
            client.create_bounty(
                &creator,
                &Symbol::new(&env, "bug"),
                &SorobanString::from_str(&env, "desc"),
                &100,
                &token_addr,
                &0,
                &Some(50),
                &SorobanVec::new(&env),
                &1,
                &None,
                &1,
                &SorobanVec::new(&env),
            );
        },
        "bounty deadline passed",
    );

    let verifiers = SorobanVec::new(&env);
    assert_contract_panic(
        || {
            client.create_bounty(
                &creator,
                &Symbol::new(&env, "bug"),
                &SorobanString::from_str(&env, "desc"),
                &100,
                &token_addr,
                &0,
                &None,
                &SorobanVec::new(&env),
                &1,
                &Some(verifiers),
                &1,
                &SorobanVec::new(&env),
            );
        },
        "approval_threshold cannot exceed the number of required_verifiers",
    );

    let mut ms = SorobanVec::new(&env);
    ms.push_back(Milestone {
        description: Symbol::new(&env, "m"),
        reward: 50,
        completed: false,
    });
    assert_contract_panic(
        || {
            client.create_bounty(
                &creator,
                &Symbol::new(&env, "bug"),
                &SorobanString::from_str(&env, "desc"),
                &100,
                &token_addr,
                &0,
                &None,
                &SorobanVec::new(&env),
                &1,
                &None,
                &1,
                &ms,
            );
        },
        "milestone rewards do not sum to reward_amount",
    );

    let mut ms_overflow = SorobanVec::new(&env);
    ms_overflow.push_back(Milestone {
        description: Symbol::new(&env, "m1"),
        reward: i128::MAX,
        completed: false,
    });
    ms_overflow.push_back(Milestone {
        description: Symbol::new(&env, "m2"),
        reward: 1,
        completed: false,
    });
    assert_contract_panic(
        || {
            client.create_bounty(
                &creator,
                &Symbol::new(&env, "bug"),
                &SorobanString::from_str(&env, "desc"),
                &100,
                &token_addr,
                &0,
                &None,
                &SorobanVec::new(&env),
                &1,
                &None,
                &1,
                &ms_overflow,
            );
        },
        "reward amount arithmetic overflow",
    );

    let fake_token = Address::generate(&env);
    assert_contract_panic(
        || {
            client.create_bounty(
                &creator,
                &Symbol::new(&env, "bug"),
                &SorobanString::from_str(&env, "desc"),
                &100,
                &fake_token,
                &0,
                &None,
                &SorobanVec::new(&env),
                &1,
                &None,
                &1,
                &SorobanVec::new(&env),
            );
        },
        "invalid reward_token address",
    );
}

/// Executes 10,000 randomized fuzz test cases across batched environments validating invariant assertions.
#[test]
fn test_fuzz_create_bounty_10k_cases() {
    let mut rng_state: u64 = 0xdeadbeefcafe1337;
    let mut next_rand = || -> u64 {
        rng_state = rng_state.wrapping_mul(6364136223846793005).wrapping_add(1);
        rng_state
    };

    let total_cases: usize = 10_000;
    let batch_size: usize = 100;
    let batches = total_cases / batch_size;

    for _batch in 0..batches {
        let (env, client, _creator, token_addr) = create_test_env();

        for _ in 0..batch_size {
            let r = next_rand();
            let case_type = r % 10;
            let creator = Address::generate(&env);

            match case_type {
                0..=4 => {
                    let reward: i128 = match (r >> 4) % 5 {
                        0 => 100,
                        1 => i128::MAX,
                        2 => 100 + ((r >> 8) % 10_000) as i128,
                        3 => 1_000_000 + ((r >> 8) % 1_000_000_000) as i128,
                        _ => (i128::MAX / 2) + ((r >> 8) % 1_000_000) as i128,
                    };

                    let tag_count = ((r >> 12) % 6) as usize;
                    let mut tags = SorobanVec::new(&env);
                    for t in 0..tag_count {
                        tags.push_back(Symbol::new(
                            &env,
                            ALLOWED_TAGS[(t + (r as usize >> 16)) % ALLOWED_TAGS.len()],
                        ));
                    }

                    let max_assignees = 1 + ((r >> 20) % 100) as u32;
                    let has_deadline = ((r >> 24) % 2) == 1;
                    let current_seq = env.ledger().sequence();
                    let deadline = if has_deadline {
                        Some(current_seq + ((r >> 25) % 10_000) as u32)
                    } else {
                        None
                    };

                    let num_verifiers = ((r >> 28) % 4) as usize;
                    let required_verifiers = if num_verifiers > 0 {
                        let mut v = SorobanVec::new(&env);
                        for _ in 0..num_verifiers {
                            v.push_back(Address::generate(&env));
                        }
                        Some(v)
                    } else {
                        None
                    };

                    let approval_threshold = if let Some(ref v) = required_verifiers {
                        ((r >> 32) as u32 % v.len()) + 1
                    } else {
                        0
                    };

                    let mut milestones = SorobanVec::new(&env);
                    let has_milestones = ((r >> 36) % 2) == 1;
                    let num_milestones = 1 + ((r >> 38) % 4) as usize;
                    if has_milestones && reward >= num_milestones as i128 {
                        let base = reward / (num_milestones as i128);
                        let rem = reward % (num_milestones as i128);
                        for m_idx in 0..num_milestones {
                            let amt = if m_idx == 0 { base + rem } else { base };
                            milestones.push_back(Milestone {
                                description: Symbol::new(&env, "m"),
                                reward: amt,
                                completed: false,
                            });
                        }
                    }

                    let res = client.try_create_bounty(
                        &creator,
                        &Symbol::new(&env, "bounty"),
                        &SorobanString::from_str(&env, "desc"),
                        &reward,
                        &token_addr,
                        &0,
                        &deadline,
                        &tags,
                        &max_assignees,
                        &required_verifiers,
                        &approval_threshold,
                        &milestones,
                    );
                    assert!(res.is_ok(), "Valid bounty creation failed");
                    let id = res.unwrap().unwrap();
                    let stored = client.get_bounty(&id).expect("stored bounty");
                    assert_eq!(stored.reward_amount, reward);
                    assert_eq!(stored.creator, creator);
                    assert_eq!(stored.tags.len(), tags.len());
                    assert_eq!(stored.max_assignees, max_assignees);
                    assert_eq!(stored.deadline, deadline);
                    assert_eq!(stored.status, Symbol::new(&env, "open"));
                    if !milestones.is_empty() {
                        let sum: i128 = stored.milestones.iter().map(|m| m.reward).sum();
                        assert_eq!(sum, stored.reward_amount);
                    }
                }
                5 => {
                    let reward = ((r >> 8) % 100) as i128;
                    let res = client.try_create_bounty(
                        &creator,
                        &Symbol::new(&env, "bug"),
                        &SorobanString::from_str(&env, "desc"),
                        &reward,
                        &token_addr,
                        &0,
                        &None,
                        &SorobanVec::new(&env),
                        &1,
                        &None,
                        &1,
                        &SorobanVec::new(&env),
                    );
                    assert!(res.is_err());
                }
                6 => {
                    let count = 6 + ((r >> 8) % 5) as usize;
                    let mut tags = SorobanVec::new(&env);
                    for i in 0..count {
                        tags.push_back(Symbol::new(&env, ALLOWED_TAGS[i % ALLOWED_TAGS.len()]));
                    }
                    let res = client.try_create_bounty(
                        &creator,
                        &Symbol::new(&env, "bug"),
                        &SorobanString::from_str(&env, "desc"),
                        &1000,
                        &token_addr,
                        &0,
                        &None,
                        &tags,
                        &1,
                        &None,
                        &1,
                        &SorobanVec::new(&env),
                    );
                    assert!(res.is_err());
                }
                7 => {
                    let res = client.try_create_bounty(
                        &creator,
                        &Symbol::new(&env, "bug"),
                        &SorobanString::from_str(&env, "desc"),
                        &1000,
                        &token_addr,
                        &0,
                        &None,
                        &SorobanVec::new(&env),
                        &0,
                        &None,
                        &1,
                        &SorobanVec::new(&env),
                    );
                    assert!(res.is_err());
                }
                8 => {
                    env.ledger().set_sequence_number(1000);
                    let past_deadline = ((r >> 8) % 1000) as u32;
                    let res = client.try_create_bounty(
                        &creator,
                        &Symbol::new(&env, "bug"),
                        &SorobanString::from_str(&env, "desc"),
                        &1000,
                        &token_addr,
                        &0,
                        &Some(past_deadline),
                        &SorobanVec::new(&env),
                        &1,
                        &None,
                        &1,
                        &SorobanVec::new(&env),
                    );
                    assert!(res.is_err());
                }
                _ => {
                    let mut verifiers = SorobanVec::new(&env);
                    verifiers.push_back(Address::generate(&env));
                    let res = client.try_create_bounty(
                        &creator,
                        &Symbol::new(&env, "bug"),
                        &SorobanString::from_str(&env, "desc"),
                        &1000,
                        &token_addr,
                        &0,
                        &None,
                        &SorobanVec::new(&env),
                        &1,
                        &Some(verifiers),
                        &5,
                        &SorobanVec::new(&env),
                    );
                    assert!(res.is_err());
                }
            }
        }
    }
}
