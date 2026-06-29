use soroban_sdk::{contract, contractimpl, token::TokenClient, Address, BytesN, Env, Symbol, Vec};

use crate::errors;
use crate::events;
use crate::storage;
use crate::types::{
    Bounty, BountyMeta, Contributor,
    STATUS_OPEN, STATUS_IN_PROGRESS, STATUS_COMPLETED, STATUS_CANCELLED, STATUS_DISPUTED,
};

fn generate_bounty_id(env: &Env, count: u64) -> BytesN<32> {
    let mut buf = [0u8; 32];
    buf[24..32].copy_from_slice(&count.to_be_bytes());
    BytesN::from_array(env, &buf)
}

#[contract]
pub struct MergeMintContract;

#[contractimpl]
impl MergeMintContract {
    pub fn create_bounty(
        env: Env,
        creator: Address,
        title: Symbol,
        description: Symbol,
        reward_amount: i128,
        reward_token: Address,
        min_reputation: u32,
        deadline: Option<u32>,
        verifier: Option<Address>, // #264
    ) -> BytesN<32> {
        if reward_amount <= 0 {
            panic!("{}", errors::REWARD_MUST_BE_POSITIVE);
        }

        creator.require_auth();

        // #263: lock reward tokens from creator into the contract
        let token = TokenClient::new(&env, &reward_token);
        token.transfer(&creator, &env.current_contract_address(), &reward_amount);

        let count = storage::get_bounty_count(&env);
        let id = generate_bounty_id(&env, count);

        let bounty = Bounty {
            creator: creator.clone(),
            reward_amount,
            reward_token,
            assignees: Vec::new(&env),
            max_assignees: 1,
            status: Symbol::new(&env, STATUS_OPEN),
            min_reputation,
            deadline,
            verifier, // #264
        };

        storage::store_bounty(&env, &id, &bounty);
        storage::store_bounty_meta(&env, &id, &BountyMeta { title, description });
        storage::set_bounty_count(&env, &(count + 1));
        storage::add_bounty_to_status(&env, &id, &bounty.status);

        let mut open = storage::get_open_bounties(&env);
        open.push_back(id.clone());
        storage::set_open_bounties(&env, &open);

        events::emit_bounty_created(&env, &id, &creator, &reward_amount);
        id
    }

    pub fn claim_bounty(env: Env, contributor: Address, bounty_id: BytesN<32>) {
        contributor.require_auth();

        let mut bounty = storage::get_bounty(&env, &bounty_id)
            .unwrap_or_else(|| panic!("{}", errors::BOUNTY_NOT_FOUND));

        if bounty.status != Symbol::new(&env, STATUS_OPEN) {
            panic!("{}", errors::BOUNTY_NOT_OPEN);
        }

        if bounty.assignees.len() >= bounty.max_assignees {
            panic!("{}", errors::BOUNTY_ALREADY_ASSIGNED);
        }

        for (addr, _) in bounty.assignees.iter() {
            if addr == contributor {
                panic!("{}", errors::BOUNTY_ALREADY_ASSIGNED);
            }
        }

        let mut contrib = storage::get_contributor(&env, &contributor)
            .unwrap_or(Contributor {
                address: contributor.clone(),
                reputation: 0,
                total_earned: 0,
                contribution_count: 0,
                active_claims: 0,
                metadata: None,
            });

        if contrib.active_claims >= 1 {
            panic!("{}", errors::CONTRIBUTOR_HAS_ACTIVE_CLAIM);
        }

        if let Some(deadline) = bounty.deadline {
            if env.ledger().sequence() > deadline {
                panic!("{}", errors::BOUNTY_DEADLINE_PASSED);
            }
        }

        if bounty.min_reputation > 0 && contrib.reputation < bounty.min_reputation {
            panic!("contributor reputation is too low");
        }

        let previous_status = bounty.status.clone();
        bounty.assignees.push_back((contributor.clone(), 10_000u32));
        bounty.status = Symbol::new(&env, STATUS_IN_PROGRESS);

        storage::store_bounty(&env, &bounty_id, &bounty);
        storage::move_bounty_status(&env, &bounty_id, &previous_status, &bounty.status);

        contrib.active_claims += 1;
        storage::store_contributor(&env, &contributor, &contrib);

        events::emit_bounty_claimed(&env, &bounty_id, &contributor);

        let open = storage::get_open_bounties(&env);
        let mut new_open = Vec::new(&env);
        for existing_id in open.iter() {
            if existing_id != bounty_id {
                new_open.push_back(existing_id);
            }
        }
        storage::set_open_bounties(&env, &new_open);
    }

    pub fn complete_bounty(env: Env, verifier: Address, bounty_id: BytesN<32>) {
        verifier.require_auth();

        let mut bounty = storage::get_bounty(&env, &bounty_id)
            .unwrap_or_else(|| panic!("{}", errors::BOUNTY_NOT_FOUND));

        if bounty.status != Symbol::new(&env, STATUS_IN_PROGRESS) {
            panic!("{}", errors::BOUNTY_NOT_IN_PROGRESS);
        }

        if bounty.assignees.is_empty() {
            panic!("{}", errors::BOUNTY_HAS_NO_ASSIGNEE);
        }

        // #264: if a designated verifier was set, enforce it
        if let Some(ref designated) = bounty.verifier {
            if verifier != *designated {
                panic!("caller is not the designated verifier");
            }
        }

        let token = TokenClient::new(&env, &bounty.reward_token);

        for (assignee, share_bp) in bounty.assignees.iter() {
            let payout = bounty.reward_amount * (share_bp as i128) / 10_000_i128;
            // #263: pay out from the contract's own escrowed balance
            token.transfer(&env.current_contract_address(), &assignee, &payout);

            let mut contrib = storage::get_contributor(&env, &assignee)
                .unwrap_or(Contributor {
                    address: assignee.clone(),
                    reputation: 0,
                    total_earned: 0,
                    contribution_count: 0,
                    active_claims: 0,
                    metadata: None,
                });

            contrib.reputation += 10;
            contrib.total_earned += payout;
            contrib.contribution_count += 1;
            if contrib.active_claims > 0 {
                contrib.active_claims -= 1;
            }

            storage::store_contributor(&env, &assignee, &contrib);
            // #271: register contributor in the index on first completion
            storage::add_to_contributor_index(&env, &assignee);
            events::emit_reward_paid(&env, &bounty_id, &assignee, &payout);
        }

        let (primary_assignee, _) = bounty.assignees.get(0).unwrap();

        let previous_status = bounty.status.clone();
        bounty.status = Symbol::new(&env, STATUS_COMPLETED);
        storage::store_bounty(&env, &bounty_id, &bounty);
        storage::move_bounty_status(&env, &bounty_id, &previous_status, &bounty.status);

        events::emit_bounty_completed(&env, &bounty_id, &primary_assignee);
    }

    pub fn cancel_bounty(env: Env, caller: Address, bounty_id: BytesN<32>) {
        caller.require_auth();

        let mut bounty = storage::get_bounty(&env, &bounty_id)
            .expect(errors::BOUNTY_NOT_FOUND);

        if caller != bounty.creator {
            panic!("{}", errors::NOT_BOUNTY_CREATOR);
        }

        if bounty.status != Symbol::new(&env, STATUS_OPEN) {
            panic!("{}", errors::BOUNTY_NOT_OPEN);
        }

        // #263: refund escrowed tokens to the creator
        let token = TokenClient::new(&env, &bounty.reward_token);
        token.transfer(&env.current_contract_address(), &bounty.creator, &bounty.reward_amount);

        let previous_status = bounty.status.clone();
        bounty.status = Symbol::new(&env, STATUS_CANCELLED);
        storage::store_bounty(&env, &bounty_id, &bounty);
        storage::move_bounty_status(&env, &bounty_id, &previous_status, &bounty.status);

        events::emit_bounty_cancelled(&env, &bounty_id, &caller);
    }

    pub fn raise_dispute(env: Env, caller: Address, bounty_id: BytesN<32>) {
        caller.require_auth();

        let mut bounty = storage::get_bounty(&env, &bounty_id)
            .expect(errors::BOUNTY_NOT_FOUND);

        let is_assignee = bounty.assignees.iter().any(|(addr, _)| addr == caller);
        if caller != bounty.creator && !is_assignee {
            panic!("only creator or assignee can raise dispute");
        }

        let previous_status = bounty.status.clone();
        bounty.status = Symbol::new(&env, STATUS_DISPUTED);
        storage::store_bounty(&env, &bounty_id, &bounty);
        storage::move_bounty_status(&env, &bounty_id, &previous_status, &bounty.status);
        events::emit_bounty_disputed(&env, &bounty_id, &caller);
    }

    pub fn expire_bounty(env: Env, caller: Address, bounty_id: BytesN<32>) {
        caller.require_auth();

        let mut bounty = storage::get_bounty(&env, &bounty_id)
            .expect(errors::BOUNTY_NOT_FOUND);

        let deadline = bounty.deadline
            .unwrap_or_else(|| panic!("{}", errors::BOUNTY_NO_DEADLINE));

        if env.ledger().sequence() <= deadline {
            panic!("{}", errors::DEADLINE_NOT_PASSED);
        }

        if bounty.status != Symbol::new(&env, STATUS_OPEN) {
            panic!("{}", errors::BOUNTY_NOT_OPEN);
        }

        // #263: refund escrowed tokens to the creator on expiry
        let token = TokenClient::new(&env, &bounty.reward_token);
        token.transfer(&env.current_contract_address(), &bounty.creator, &bounty.reward_amount);

        let previous_status = bounty.status.clone();
        bounty.status = Symbol::new(&env, STATUS_CANCELLED);
        storage::store_bounty(&env, &bounty_id, &bounty);
        storage::move_bounty_status(&env, &bounty_id, &previous_status, &bounty.status);

        events::emit_bounty_expired(&env, &bounty_id, &bounty.creator);
    }

    pub fn update_contributor_metadata(env: Env, contributor: Address, metadata: Symbol) {
        contributor.require_auth();

        let mut contrib = storage::get_contributor(&env, &contributor)
            .unwrap_or(Contributor {
                address: contributor.clone(),
                reputation: 0,
                total_earned: 0,
                contribution_count: 0,
                active_claims: 0,
                metadata: None,
            });

        contrib.metadata = Some(metadata);
        storage::store_contributor(&env, &contributor, &contrib);
    }

    // #271: return top `limit` contributors sorted by reputation descending
    pub fn get_top_contributors(env: Env, limit: u32) -> Vec<Contributor> {
        let index = storage::get_contributor_index(&env);
        let mut contributors: Vec<Contributor> = Vec::new(&env);

        for address in index.iter() {
            if let Some(c) = storage::get_contributor(&env, &address) {
                contributors.push_back(c);
            }
        }

        // Simple insertion sort (on-chain, small N expected)
        let len = contributors.len();
        for i in 1..len {
            let mut j = i;
            while j > 0 {
                let a = contributors.get(j - 1).unwrap();
                let b = contributors.get(j).unwrap();
                if a.reputation < b.reputation {
                    contributors.set(j - 1, b);
                    contributors.set(j, a);
                    j -= 1;
                } else {
                    break;
                }
            }
        }

        let take = if limit as u32 <= len { limit as u32 } else { len };
        let mut result: Vec<Contributor> = Vec::new(&env);
        for i in 0..take {
            result.push_back(contributors.get(i).unwrap());
        }
        result
    }

    pub fn get_bounty(env: Env, bounty_id: BytesN<32>) -> Option<Bounty> {
        storage::get_bounty(&env, &bounty_id)
    }

    pub fn get_bounty_meta(env: Env, bounty_id: BytesN<32>) -> Option<BountyMeta> {
        storage::get_bounty_meta(&env, &bounty_id)
    }

    pub fn get_contributor(env: Env, address: Address) -> Option<Contributor> {
        storage::get_contributor(&env, &address)
    }

    pub fn get_bounty_count(env: Env) -> u64 {
        storage::get_bounty_count(&env)
    }

    pub fn get_bounties_by_status(env: Env, status: Symbol) -> Vec<BytesN<32>> {
        storage::get_bounties_by_status(&env, &status)
    }

    pub fn get_open_bounties(env: Env) -> Vec<BytesN<32>> {
        storage::get_open_bounties(&env)
    }
}
