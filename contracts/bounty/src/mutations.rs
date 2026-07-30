use soroban_sdk::{contract, contractimpl, Address, Env, String, Vec};
use crate::storage::{self, Bounty, BountyStatus};
use crate::token;

#[contract]
pub struct BountyContract;

#[contractimpl]
impl BountyContract {
    pub fn cancel_bounty(env: Env, bounty_id: u64, caller: Address) {
        caller.require_auth();
        
        let mut bounty = storage::get_bounty(&env, bounty_id)
            .expect("Bounty not found");
        
        if bounty.creator != caller {
            panic!("Only creator can cancel bounty");
        }
        
        if bounty.status != BountyStatus::Open {
            panic!("Can only cancel open bounties");
        }
        
        let token_client = token::get_token_client(&env, &bounty.token_address);
        token_client.transfer(
            &env.current_contract_address(),
            &bounty.creator,
            &bounty.reward_amount
        );
        
        bounty.status = BountyStatus::Cancelled;
        storage::set_bounty(&env, bounty_id, &bounty);
    }
    
    pub fn expire_bounty(env: Env, bounty_id: u64) {
        let mut bounty = storage::get_bounty(&env, bounty_id)
            .expect("Bounty not found");
        
        if bounty.status != BountyStatus::Open {
            panic!("Can only expire open bounties");
        }
        
        let current_time = env.ledger().timestamp();
        if current_time < bounty.expiration_time {
            panic!("Bounty has not expired yet");
        }
        
        let token_client = token::get_token_client(&env, &bounty.token_address);
        token_client.transfer(
            &env.current_contract_address(),
            &bounty.creator,
            &bounty.reward_amount
        );
        
        bounty.status = BountyStatus::Expired;
        storage::set_bounty(&env, bounty_id, &bounty);
    }
}
