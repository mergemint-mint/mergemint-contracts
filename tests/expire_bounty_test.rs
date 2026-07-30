use crate::*;

#[test]
#[should_panic(expected = "BountyNoDeadline")]
fn test_expire_bounty_no_deadline() {
    let mut bounty = Bounty {
        id: 1,
        title: "Test Bounty".to_string(),
        description: "Test Description".to_string(),
        reward: 1000,
        status: BountyStatus::Open,
        creator: "alice".to_string(),
        deadline: None,
        created_at: 0,
    };

    expire_bounty(&mut bounty, 1000);
}

#[test]
#[should_panic(expected = "DeadlineNotPassed")]
fn test_expire_bounty_deadline_not_passed() {
    let mut bounty = Bounty {
        id: 2,
        title: "Test Bounty".to_string(),
        description: "Test Description".to_string(),
        reward: 1000,
        status: BountyStatus::Open,
        creator: "alice".to_string(),
        deadline: Some(2000),
        created_at: 0,
    };

    expire_bounty(&mut bounty, 1000);
}

#[test]
fn test_expire_bounty_success() {
    let mut bounty = Bounty {
        id: 3,
        title: "Test Bounty".to_string(),
        description: "Test Description".to_string(),
        reward: 1000,
        status: BountyStatus::Open,
        creator: "alice".to_string(),
        deadline: Some(1000),
        created_at: 0,
    };

    expire_bounty(&mut bounty, 2000);

    assert_eq!(bounty.status, BountyStatus::Cancelled);
}

#[test]
fn test_expire_bounty_exact_deadline() {
    let mut bounty = Bounty {
        id: 4,
        title: "Test Bounty".to_string(),
        description: "Test Description".to_string(),
        reward: 1000,
        status: BountyStatus::Open,
        creator: "alice".to_string(),
        deadline: Some(1000),
        created_at: 0,
    };

    expire_bounty(&mut bounty, 1000);

    assert_eq!(bounty.status, BountyStatus::Cancelled);
}
