#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::queries::{
        get_bounty, get_bounties, get_bounty_metas, get_claimed_at_ledger, get_pending_resolution,
        is_dispute_appealed,
    };
    use crate::state::{
        Bounty, BountyId, PendingResolution, BOUNTIES, BOUNTY_ARCHIVE, BOUNTY_METAS,
        CLAIMED_AT_LEDGER, DISPUTE_APPEALED, PENDING_RESOLUTION,
    };
    use soroban_sdk::{testutils::Address as _, Address, Env, Vec};

    #[test]
    fn test_get_bounties_mixed_ids() {
        let env = Env::default();
        
        let creator = Address::generate(&env);
        let bounty1 = Bounty {
            id: 1,
            creator: creator.clone(),
            amount: 1000,
            description: soroban_sdk::String::from_str(&env, "Test bounty 1"),
        };
        let bounty2 = Bounty {
            id: 2,
            creator: creator.clone(),
            amount: 2000,
            description: soroban_sdk::String::from_str(&env, "Test bounty 2"),
        };
        
        BOUNTIES.set(&env, &1, &bounty1);
        BOUNTIES.set(&env, &2, &bounty2);
        
        let mut ids = Vec::new(&env);
        ids.push_back(1);
        ids.push_back(999);
        ids.push_back(2);
        ids.push_back(1000);
        
        let results = get_bounties(&env, ids);
        
        assert_eq!(results.len(), 4);
        assert!(results.get(0).unwrap().is_some());
        assert_eq!(results.get(0).unwrap().unwrap().id, 1);
        assert!(results.get(1).unwrap().is_none());
        assert!(results.get(2).unwrap().is_some());
        assert_eq!(results.get(2).unwrap().unwrap().id, 2);
        assert!(results.get(3).unwrap().is_none());
    }

    #[test]
    fn test_get_bounties_empty_vec() {
        let env = Env::default();
        let ids = Vec::new(&env);
        let results = get_bounties(&env, ids);
        assert_eq!(results.len(), 0);
    }

    #[test]
    fn test_get_bounties_all_existing() {
        let env = Env::default();
        
        let creator = Address::generate(&env);
        let bounty1 = Bounty {
            id: 10,
            creator: creator.clone(),
            amount: 500,
            description: soroban_sdk::String::from_str(&env, "Bounty 10"),
        };
        let bounty2 = Bounty {
            id: 20,
            creator: creator.clone(),
            amount: 1500,
            description: soroban_sdk::String::from_str(&env, "Bounty 20"),
        };
        
        BOUNTIES.set(&env, &10, &bounty1);
        BOUNTIES.set(&env, &20, &bounty2);
        
        let mut ids = Vec::new(&env);
        ids.push_back(10);
        ids.push_back(20);
        
        let results = get_bounties(&env, ids);
        
        assert_eq!(results.len(), 2);
        assert!(results.get(0).unwrap().is_some());
        assert!(results.get(1).unwrap().is_some());
    }

    #[test]
    fn test_get_bounties_all_missing() {
        let env = Env::default();
        
        let mut ids = Vec::new(&env);
        ids.push_back(100);
        ids.push_back(200);
        ids.push_back(300);
        
        let results = get_bounties(&env, ids);
        
        assert_eq!(results.len(), 3);
        assert!(results.get(0).unwrap().is_none());
        assert!(results.get(1).unwrap().is_none());
        assert!(results.get(2).unwrap().is_none());
    }

    #[test]
    fn test_get_pending_resolution_default_none() {
        let env = Env::default();
        assert!(get_pending_resolution(&env, 1).is_none());
    }

    #[test]
    fn test_get_pending_resolution_populated() {
        let env = Env::default();
        let resolution = PendingResolution {
            winner: Address::generate(&env),
            resolved_at_ledger: 42,
        };
        PENDING_RESOLUTION.set(&env, &7, &resolution);

        let result = get_pending_resolution(&env, 7);
        assert!(result.is_some());
        let result = result.unwrap();
        assert_eq!(result.winner, resolution.winner);
        assert_eq!(result.resolved_at_ledger, 42);
        assert!(get_pending_resolution(&env, 8).is_none());
    }

    #[test]
    fn test_is_dispute_appealed_default_false() {
        let env = Env::default();
        assert!(!is_dispute_appealed(&env, 1));
    }

    #[test]
    fn test_is_dispute_appealed_populated() {
        let env = Env::default();
        DISPUTE_APPEALED.set(&env, &3, &true);
        assert!(is_dispute_appealed(&env, 3));
        assert!(!is_dispute_appealed(&env, 4));
    }

    #[test]
    fn test_get_claimed_at_ledger_default_none() {
        let env = Env::default();
        assert!(get_claimed_at_ledger(&env, 1).is_none());
    }

    #[test]
    fn test_get_claimed_at_ledger_populated() {
        let env = Env::default();
        CLAIMED_AT_LEDGER.set(&env, &5, &1234u32);
        assert_eq!(get_claimed_at_ledger(&env, 5), Some(1234u32));
        assert!(get_claimed_at_ledger(&env, 6).is_none());
    }

    #[test]
    fn test_get_bounty_metas_never_allocated() {
        let env = Env::default();
        let mut ids = Vec::new(&env);
        ids.push_back(500);
        ids.push_back(501);

        let results = get_bounty_metas(&env, ids);
        assert_eq!(results.len(), 2);
        assert!(results.get(0).unwrap().is_none());
        assert!(results.get(1).unwrap().is_none());
    }

    #[test]
    fn test_get_bounties_archive_fallback() {
        let env = Env::default();
        let creator = Address::generate(&env);
        let archived = Bounty {
            id: 30,
            creator: creator.clone(),
            amount: 300,
            description: soroban_sdk::String::from_str(&env, "Archived bounty"),
        };
        BOUNTY_ARCHIVE.set(&env, &30, &archived);

        let mut ids = Vec::new(&env);
        ids.push_back(30);
        let results = get_bounties(&env, ids);
        assert_eq!(results.len(), 1);
        assert!(results.get(0).unwrap().is_some());
        assert_eq!(results.get(0).unwrap().unwrap().id, 30);
    }

    #[test]
    fn test_get_bounties_mixed_active_archived_unallocated() {
        let env = Env::default();
        let creator = Address::generate(&env);

        let active = Bounty {
            id: 40,
            creator: creator.clone(),
            amount: 400,
            description: soroban_sdk::String::from_str(&env, "Active bounty"),
        };
        BOUNTIES.set(&env, &40, &active);

        let archived = Bounty {
            id: 41,
            creator: creator.clone(),
            amount: 410,
            description: soroban_sdk::String::from_str(&env, "Archived bounty"),
        };
        BOUNTY_ARCHIVE.set(&env, &41, &archived);

        let mut ids = Vec::new(&env);
        ids.push_back(40);
        ids.push_back(41);
        ids.push_back(9999);

        let results = get_bounties(&env, ids);
        assert_eq!(results.len(), 3);
        assert_eq!(results.get(0).unwrap().unwrap().id, 40);
        assert_eq!(results.get(1).unwrap().unwrap().id, 41);
        assert!(results.get(2).unwrap().is_none());
    }
}
