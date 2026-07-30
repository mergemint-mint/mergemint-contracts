#![cfg(test)]

use soroban_sdk::{testutils::Address as _, Address, Env};
use crate::{BountyContract, BountyContractClient};
use crate::storage::{Bounty, BountyStatus};
use soroban_sdk::token::{StellarAssetClient, TokenClient};

#[test]
fn test_cancel_bounty_refunds_escrow() {
    let env = Env::default();
    env.mock_all_auths();
    
    let contract_id = env.register_contract(None, BountyContract);
    let client = BountyContractClient::new(&env, &contract_id);
    
    let creator = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let token_id = env.register_stellar_asset_contract(token_admin.clone());
    let token_client = TokenClient::new(&env, &token_id);
    let token_admin_client = StellarAssetClient::new(&env, &token_id);
    
    let reward_amount: i128 = 1000;
    token_admin_client.mint(&creator, &reward_amount);
    
    let creator_initial_balance = token_client.balance(&creator);
    assert_eq!(creator_initial_balance, reward_amount);
    
    token_client.transfer(&creator, &contract_id, &reward_amount);
    let creator_balance_after_deposit = token_client.balance(&creator);
    assert_eq!(creator_balance_after_deposit, 0);
    
    let bounty_id: u64 = 1;
    let bounty = Bounty {
        id: bounty_id,
        creator: creator.clone(),
        token_address: token_id.clone(),
        reward_amount,
        status: BountyStatus::Open,
        expiration_time: env.ledger().timestamp() + 86400,
    };
    crate::storage::set_bounty(&env, bounty_id, &bounty);
    
    client.cancel_bounty(&bounty_id, &creator);
    
    let creator_final_balance = token_client.balance(&creator);
    assert_eq!(creator_final_balance, reward_amount);
    
    let contract_balance = token_client.balance(&contract_id);
    assert_eq!(contract_balance, 0);
    
    let updated_bounty = crate::storage::get_bounty(&env, bounty_id).unwrap();
    assert_eq!(updated_bounty.status, BountyStatus::Cancelled);
}

#[test]
fn test_expire_bounty_refunds_escrow() {
    let env = Env::default();
    env.mock_all_auths();
    
    let contract_id = env.register_contract(None, BountyContract);
    let client = BountyContractClient::new(&env, &contract_id);
    
    let creator = Address::generate(&env);
    let token_admin = Address::generate(&env);
    let token_id = env.register_stellar_asset_contract(token_admin.clone());
    let token_client = TokenClient::new(&env, &token_id);
    let token_admin_client = StellarAssetClient::new(&env, &token_id);
    
    let reward_amount: i128 = 2000;
    token_admin_client.mint(&creator, &reward_amount);
    token_client.transfer(&creator, &contract_id, &reward_amount);
    
    let bounty_id: u64 = 2;
    let expiration_time = env.ledger().timestamp() + 100;
    let bounty = Bounty {
        id: bounty_id,
        creator: creator.clone(),
        token_address: token_id.clone(),
        reward_amount,
        status: BountyStatus::Open,
        expiration_time,
    };
    crate::storage::set_bounty(&env, bounty_id, &bounty);
    
    env.ledger().with_mut(|li| {
        li.timestamp = expiration_time + 1;
    });
    
    client.expire_bounty(&bounty_id);
    
    let creator_final_balance = token_client.balance(&creator);
    assert_eq!(creator_final_balance, reward_amount);
    
    let contract_balance = token_client.balance(&contract_id);
    assert_eq!(contract_balance, 0);
    
    let updated_bounty = crate::storage::get_bounty(&env, bounty_id).unwrap();
    assert_eq!(updated_bounty.status, BountyStatus::Expired);
}

#[test]
#[should_panic(expected = "Only creator can cancel bounty")]
fn test_cancel_bounty_non_creator_fails() {
    let env = Env::default();
    env.mock_all_auths();
    
    let contract_id = env.register_contract(None, BountyContract);
    let client = BountyContractClient::new(&env, &contract_id);
    
    let creator = Address::generate(&env);
    let non_creator = Address::generate(&env);
    let token_id = env.register_stellar_asset_contract(Address::generate(&env));
    
    let bounty_id: u64 = 3;
    let bounty = Bounty {
        id: bounty_id,
        creator: creator.clone(),
        token_address: token_id,
        reward_amount: 500,
        status: BountyStatus::Open,
        expiration_time: env.ledger().timestamp() + 86400,
    };
    crate::storage::set_bounty(&env, bounty_id, &bounty);
    
    client.cancel_bounty(&bounty_id, &non_creator);
}

#[test]
#[should_panic(expected = "Bounty has not expired yet")]
fn test_expire_bounty_before_expiration_fails() {
    let env = Env::default();
    env.mock_all_auths();
    
    let contract_id = env.register_contract(None, BountyContract);
    let client = BountyContractClient::new(&env, &contract_id);
    
    let creator = Address::generate(&env);
    let token_id = env.register_stellar_asset_contract(Address::generate(&env));
    
    let bounty_id: u64 = 4;
    let bounty = Bounty {
        id: bounty_id,
        creator: creator.clone(),
        token_address: token_id,
        reward_amount: 750,
        status: BountyStatus::Open,
        expiration_time: env.ledger().timestamp() + 86400,
    };
    crate::storage::set_bounty(&env, bounty_id, &bounty);
    
    client.expire_bounty(&bounty_id);
}
