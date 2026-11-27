#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::queries::{get_bounty, get_bounties};
    use crate::state::{Bounty, BountyId, BOUNTIES};
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
}
