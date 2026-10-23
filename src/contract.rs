use soroban_sdk::{
    contract, contractimpl, token::TokenClient, Address, BytesN, Env, Symbol,
};

use crate::events;
use crate::storage;
use crate::types::{Bounty, ContractConfig, Contributor};

const STATUS_OPEN: &str = "open";
const STATUS_IN_PROGRESS: &str = "in_progress";

fn generate_bounty_id(env: &Env) -> BytesN<32> {
    let count = storage::get_bounty_count(env);
    let mut buf = [0u8; 32];
    let count_bytes = count.to_be_bytes();
    buf[24..32].copy_from_slice(&count_bytes);
    BytesN::from_array(env, &buf)
}

fn require_admin(env: &Env) -> Address {
    let admin = storage::get_admin(env).expect("admin not initialized");
    admin.require_auth();
    admin
}

#[contract]
pub struct MergeMintContract;

#[contractimpl]
impl MergeMintContract {
    pub fn init_admin(env: Env, admin: Address) {
        if storage::get_admin(&env).is_some() {
            panic!("admin already initialized");
        }
        storage::set_admin(&env, &admin);
        events::emit_admin_initialized(&env, &admin);
    }

    pub fn set_min_reward_amount(env: Env, token: Address, amount: i128) {
        require_admin(&env);
        if amount < 0 {
            panic!("min reward amount cannot be negative");
        }
        storage::set_min_reward_amount(&env, &token, &amount);
        events::emit_min_reward_amount_updated(&env, &token, &amount);
    }

    pub fn get_min_reward_amount(env: Env, token: Address) -> i128 {
        storage::get_min_reward_amount(&env, &token)
    }

    pub fn create_bounty(
        env: Env,
        creator: Address,
        title: Symbol,
        description: Symbol,
        reward_amount: i128,
        reward_token: Address,
    ) -> BytesN<32> {
        creator.require_auth();

        let min_reward = storage::get_min_reward_amount(&env, &reward_token);
        if reward_amount < min_reward {
            panic!("reward amount below configured minimum for token");
        }

        let count = storage::get_bounty_count(&env);
        let id = generate_bounty_id(&env);

        let bounty = Bounty {
            creator,
            title,
            description,
            reward_amount,
            reward_token,
            assignee: None,
            status: Symbol::new(&env, STATUS_OPEN),
        };

        storage::store_bounty(&env, &id, &bounty);
        storage::set_bounty_count(&env, &(count + 1));

        events::emit_bounty_created(&env, &id, &bounty.creator, &reward_amount);
        id
    }

    pub fn claim_bounty(env: Env, contributor: Address, bounty_id: BytesN<32>) {
        contributor.require_auth();

        let mut bounty = storage::get_bounty(&env, &bounty_id).expect("bounty not found");

        if bounty.assignee.is_some() {
            panic!("bounty already assigned");
        }

        bounty.assignee = Some(contributor.clone());
        bounty.status = Symbol::new(&env, STATUS_IN_PROGRESS);

        storage::store_bounty(&env, &bounty_id, &bounty);
        events::emit_bounty_claimed(&env, &bounty_id, &contributor);
    }

    pub fn complete_bounty(env: Env, verifier: Address, bounty_id: BytesN<32>) {
        verifier.require_auth();

        let bounty = storage::get_bounty(&env, &bounty_id).expect("bounty not found");
        let assignee = bounty.assignee.clone().expect("bounty has no assignee");

        let token = TokenClient::new(&env, &bounty.reward_token);
        token.transfer(&verifier, &assignee, &bounty.reward_amount);

        let mut contributor = storage::get_contributor(&env, &assignee).unwrap_or(Contributor {
            address: assignee.clone(),
            reputation: 0,
            total_earned: 0,
            contribution_count: 0,
        });

        contributor.reputation += 10;
        contributor.total_earned += bounty.reward_amount;
        contributor.contribution_count += 1;

        storage::store_contributor(&env, &assignee, &contributor);

        events::emit_bounty_completed(&env, &bounty_id, &assignee);
        events::emit_reward_paid(&env, &bounty_id, &assignee, &bounty.reward_amount);
    }

    pub fn get_bounty(env: Env, bounty_id: BytesN<32>) -> Option<Bounty> {
        storage::get_bounty(&env, &bounty_id)
    }

    pub fn get_contributor(env: Env, address: Address) -> Option<Contributor> {
        storage::get_contributor(&env, &address)
    }

    pub fn get_bounty_count(env: Env) -> u64 {
        storage::get_bounty_count(&env)
    }

    pub fn transfer_bounty_ownership(
        env: Env,
        current_owner: Address,
        bounty_id: BytesN<32>,
        new_owner: Address,
    ) {
        current_owner.require_auth();

        let mut bounty = storage::get_bounty(&env, &bounty_id).expect("bounty not found");

        if bounty.creator != current_owner {
            panic!("only the current owner can transfer bounty ownership");
        }

        bounty.creator = new_owner.clone();
        storage::store_bounty(&env, &bounty_id, &bounty);

        events::emit_bounty_ownership_transferred(&env, &bounty_id, &current_owner, &new_owner);
    }

    pub fn get_config(env: Env) -> ContractConfig {
        ContractConfig {
            admin: storage::get_admin(&env),
            bounty_count: storage::get_bounty_count(&env),
        }
    }
}
