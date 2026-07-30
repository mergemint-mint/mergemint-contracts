#[cfg(test)]
mod tests {
    use super::*;
    use ic_cdk::api::caller;
    use crate::types::{Bounty, BountyStatus};
    use crate::state::{with_state_mut, with_state};

    #[test]
    #[should_panic(expected = "NotBountyCreator")]
    fn test_cancel_bounty_non_creator_panics() {
        let bounty_id = 1u64;
        let creator = ic_cdk::export::Principal::from_text("aaaaa-aa").unwrap();
        let non_creator = ic_cdk::export::Principal::from_text("bbbbb-bb").unwrap();

        with_state_mut(|state| {
            let bounty = Bounty {
                id: bounty_id,
                creator,
                status: BountyStatus::Open,
                title: "Test Bounty".to_string(),
                description: "Test Description".to_string(),
                reward: 100,
                created_at: 0,
                updated_at: 0,
            };
            state.bounties.insert(bounty_id, bounty);
            state.status_index.entry(BountyStatus::Open).or_insert_with(Vec::new).push(bounty_id);
        });

        ic_cdk::api::set_caller(non_creator);
        cancel_bounty(bounty_id);
    }

    #[test]
    #[should_panic(expected = "BountyNotOpen")]
    fn test_cancel_bounty_non_open_status_panics() {
        let bounty_id = 2u64;
        let creator = ic_cdk::export::Principal::from_text("aaaaa-aa").unwrap();

        with_state_mut(|state| {
            let bounty = Bounty {
                id: bounty_id,
                creator,
                status: BountyStatus::Completed,
                title: "Completed Bounty".to_string(),
                description: "Already completed".to_string(),
                reward: 200,
                created_at: 0,
                updated_at: 0,
            };
            state.bounties.insert(bounty_id, bounty);
            state.status_index.entry(BountyStatus::Completed).or_insert_with(Vec::new).push(bounty_id);
        });

        ic_cdk::api::set_caller(creator);
        cancel_bounty(bounty_id);
    }

    #[test]
    fn test_cancel_bounty_happy_path() {
        let bounty_id = 3u64;
        let creator = ic_cdk::export::Principal::from_text("aaaaa-aa").unwrap();

        with_state_mut(|state| {
            let bounty = Bounty {
                id: bounty_id,
                creator,
                status: BountyStatus::Open,
                title: "Valid Bounty".to_string(),
                description: "Can be cancelled".to_string(),
                reward: 300,
                created_at: 0,
                updated_at: 0,
            };
            state.bounties.insert(bounty_id, bounty);
            state.status_index.entry(BountyStatus::Open).or_insert_with(Vec::new).push(bounty_id);
        });

        ic_cdk::api::set_caller(creator);
        cancel_bounty(bounty_id);

        with_state(|state| {
            let bounty = state.bounties.get(&bounty_id).unwrap();
            assert_eq!(bounty.status, BountyStatus::Cancelled);
            assert!(!state.status_index.get(&BountyStatus::Open).unwrap().contains(&bounty_id));
            assert!(state.status_index.get(&BountyStatus::Cancelled).unwrap().contains(&bounty_id));
        });
    }
}
