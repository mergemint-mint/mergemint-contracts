use bounty_program::*;

#[test]
#[should_panic(expected = "RewardMustBePositive")]
fn test_create_bounty_zero_reward_panics() {
    let mut bounties = vec![];
    create_bounty(
        &mut bounties,
        "Test Bounty".to_string(),
        "Description".to_string(),
        0,
    );
}

#[test]
#[should_panic(expected = "RewardMustBePositive")]
fn test_create_bounty_negative_reward_panics() {
    let mut bounties = vec![];
    create_bounty(
        &mut bounties,
        "Test Bounty".to_string(),
        "Description".to_string(),
        -100,
    );
}

#[test]
fn test_create_bounty_positive_reward_succeeds() {
    let mut bounties = vec![];
    create_bounty(
        &mut bounties,
        "Valid Bounty".to_string(),
        "Valid Description".to_string(),
        100,
    );
    assert_eq!(bounties.len(), 1);
    assert_eq!(bounties[0].reward_amount, 100);
}
