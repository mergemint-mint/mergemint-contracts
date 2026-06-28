use soroban_sdk::{contract, contractimpl, token::TokenClient, Address, BytesN, Env, Symbol, Vec};

use crate::errors;
use crate::events;
use crate::storage;
use crate::types::{Bounty, BountyMeta, Contributor};

const STATUS_OPEN: &str = "open";
const STATUS_IN_PROGRESS: &str = "in_progress";
const STATUS_COMPLETED: &str = "completed";
const STATUS_CANCELLED: &str = "cancelled";
const STATUS_DISPUTED: &str = "disputed";

fn generate_bounty_id(env: &Env, count: u64) -> BytesN<32> {
    let mut buf = [0u8; 32];
    let count_bytes = count.to_be_bytes();
    buf[24..32].copy_from_slice(&count_bytes);
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
    ) -> BytesN<32> {
        creator.require_auth();

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
            required_verifiers: None,
            approval_threshold: 1,
        };

        let meta = BountyMeta { title, description };

        storage::store_bounty(&env, &id, &bounty);
        storage::store_bounty_meta(&env, &id, &meta);
        storage::set_bounty_count(&env, &(count + 1));
        storage::add_bounty_to_status(&env, &id, &bounty.status);

        let mut open = storage::get_open_bounties(&env);
        open.push_back(id.clone());
        storage::set_open_bounties(&env, &open);

        events::emit_bounty_created(&env, &id, &creator, &reward_amount);
        id
    }

    /// Claim an open bounty. A contributor receives 10 000 basis points (full reward)
    /// when claiming a single-assignee bounty (`max_assignees == 1`).
    /// For multi-assignee bounties the caller must supply an explicit `share` in basis
    /// points; the sum of all shares must not exceed 10 000.
    pub fn claim_bounty(env: Env, contributor: Address, bounty_id: BytesN<32>) {
        contributor.require_auth();

        let mut bounty = match storage::get_bounty(&env, &bounty_id) {
            Some(b) => b,
            None => panic!("{}", errors::BOUNTY_NOT_FOUND),
        };

        // Reject if already at capacity.
        if bounty.assignees.len() >= bounty.max_assignees {
            panic!("{}", errors::BOUNTY_ALREADY_ASSIGNED);
        }

        // Reject if the contributor is already listed.
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

        // Deadline enforcement: if a deadline is set and has passed, reject the claim
        if let Some(deadline) = bounty.deadline {
            if env.ledger().sequence() > deadline {
                panic!("{}", errors::BOUNTY_DEADLINE_PASSED);
            }
        }

        if bounty.min_reputation > 0 {
            let contributor_profile = storage::get_contributor(&env, &contributor).unwrap_or(Contributor {
                address: contributor.clone(),
                reputation: 0,
                total_earned: 0,
                contribution_count: 0,
                active_claims: 0,
                metadata: None,
            });
            if contributor_profile.reputation < bounty.min_reputation {
                panic!("contributor reputation is too low");
            }
        }

        let share_bp: u32 = 10_000u32 / bounty.max_assignees;
        bounty.assignees.push_back((contributor.clone(), share_bp));
        let previous_status = Symbol::new(&env, STATUS_OPEN);
        bounty.status = Symbol::new(&env, STATUS_IN_PROGRESS);
        storage::store_bounty(&env, &bounty_id, &bounty);
        storage::move_bounty_status(&env, &bounty_id, &previous_status, &bounty.status);

        contrib.active_claims += 1;
        storage::store_contributor(&env, &contributor, &contrib);

        events::emit_bounty_claimed(&env, &bounty_id, &contributor);

        let mut open = storage::get_open_bounties(&env);
        let mut new_open = Vec::new(&env);
        for existing_id in open.iter() {
            if existing_id != bounty_id {
                new_open.push_back(existing_id);
            }
        }
        storage::set_open_bounties(&env, &new_open);
    }

    /// Complete a bounty: distribute `reward_amount` proportionally across all assignees
    /// according to their basis-point shares (shares sum to 10 000).
    pub fn complete_bounty(env: Env, verifier: Address, bounty_id: BytesN<32>) {
        verifier.require_auth();

        let mut bounty = match storage::get_bounty(&env, &bounty_id) {
            Some(b) => b,
            None => panic!("{}", errors::BOUNTY_NOT_FOUND),
        };

        if bounty.assignees.is_empty() {
            panic!("{}", errors::BOUNTY_HAS_NO_ASSIGNEE);
        }

        let token = TokenClient::new(&env, &bounty.reward_token);

        for (assignee, share_bp) in bounty.assignees.iter() {
            let payout = (bounty.reward_amount as i128) * (share_bp as i128) / 10_000_i128;
            token.transfer(&verifier, &assignee, &payout);

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
            events::emit_reward_paid(&env, &bounty_id, &assignee, &payout);
        }

        // Use the first assignee as the primary for the completion event (backward compat).
        let (primary_assignee, _) = bounty.assignees.get(0).unwrap();

        let previous_status = bounty.status.clone();
        bounty.status = Symbol::new(&env, STATUS_COMPLETED);
        storage::store_bounty(&env, &bounty_id, &bounty);
        storage::move_bounty_status(&env, &bounty_id, &previous_status, &bounty.status);

        events::emit_bounty_completed(&env, &bounty_id, &primary_assignee);
    }

    /// Record one verifier's approval for a multi-sig bounty completion.
    /// When the number of unique approvals reaches approval_threshold, completion executes automatically.
    /// Falls back to single-verifier behaviour when required_verifiers is None (any verifier completes directly).
    pub fn approve_completion(env: Env, verifier: Address, bounty_id: BytesN<32>) {
        verifier.require_auth();

        let mut bounty = match storage::get_bounty(&env, &bounty_id) {
            Some(b) => b,
            None => panic!("{}", errors::BOUNTY_NOT_FOUND),
        };

        if bounty.assignees.is_empty() {
            panic!("{}", errors::BOUNTY_HAS_NO_ASSIGNEE);
        }

        // If no required_verifiers list is set, fall back to immediate single-verifier completion.
        if bounty.required_verifiers.is_none() {
            MergeMintContract::complete_bounty(env, verifier, bounty_id);
            return;
        }

        let required = bounty.required_verifiers.as_ref().unwrap();
        let is_authorized = required.iter().any(|v| v == verifier);
        if !is_authorized {
            panic!("{}", errors::VERIFIER_NOT_AUTHORIZED);
        }

        let mut approvals = storage::get_approvals(&env, &bounty_id);

        // Guard against duplicate votes from the same verifier.
        let already_voted = approvals.iter().any(|v| v == verifier);
        if already_voted {
            panic!("{}", errors::ALREADY_APPROVED);
        }

        approvals.push_back(verifier.clone());
        storage::set_approvals(&env, &bounty_id, &approvals);

        let approval_count = approvals.len();
        events::emit_approval_recorded(&env, &bounty_id, &verifier, approval_count);

        let threshold = if bounty.approval_threshold == 0 {
            1
        } else {
            bounty.approval_threshold
        };

        if approval_count >= threshold {
            let token = TokenClient::new(&env, &bounty.reward_token);

            for (assignee, share_bp) in bounty.assignees.iter() {
                let payout =
                    (bounty.reward_amount as i128) * (share_bp as i128) / 10_000_i128;
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
                events::emit_reward_paid(&env, &bounty_id, &assignee, &payout);
            }

            let (primary_assignee, _) = bounty.assignees.get(0).unwrap();
            let previous_status = bounty.status.clone();
            bounty.status = Symbol::new(&env, STATUS_COMPLETED);
            storage::store_bounty(&env, &bounty_id, &bounty);
            storage::move_bounty_status(&env, &bounty_id, &previous_status, &bounty.status);
            events::emit_bounty_completed(&env, &bounty_id, &primary_assignee);
        }
    }

    pub fn raise_dispute(env: Env, caller: Address, bounty_id: BytesN<32>) {
        caller.require_auth();

        let mut bounty = storage::get_bounty(&env, &bounty_id).expect("bounty not found");

        // Allow creator or any assignee to raise a dispute.
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

    /// Resolve a disputed bounty. Only the bounty creator (acting as arbitrator) may call this.
    /// resolution must be the Symbol "complete" (pay assignees) or "cancel" (refund creator).
    pub fn resolve_dispute(
        env: Env,
        arbitrator: Address,
        bounty_id: BytesN<32>,
        resolution: Symbol,
    ) {
        arbitrator.require_auth();

        let mut bounty = match storage::get_bounty(&env, &bounty_id) {
            Some(b) => b,
            None => panic!("{}", errors::BOUNTY_NOT_FOUND),
        };

        if bounty.status != Symbol::new(&env, STATUS_DISPUTED) {
            panic!("{}", errors::BOUNTY_NOT_DISPUTED);
        }

        // The arbitrator must be the bounty creator; there is no separate admin address.
        if arbitrator != bounty.creator {
            panic!("{}", errors::NOT_ARBITRATOR);
        }

        let resolve_complete = Symbol::new(&env, "complete");
        let resolve_cancel = Symbol::new(&env, "cancel");

        if resolution == resolve_complete {
            let token = TokenClient::new(&env, &bounty.reward_token);

            for (assignee, share_bp) in bounty.assignees.iter() {
                let payout =
                    (bounty.reward_amount as i128) * (share_bp as i128) / 10_000_i128;
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
                events::emit_reward_paid(&env, &bounty_id, &assignee, &payout);
            }

            let previous_status = bounty.status.clone();
            bounty.status = Symbol::new(&env, STATUS_COMPLETED);
            storage::store_bounty(&env, &bounty_id, &bounty);
            storage::move_bounty_status(&env, &bounty_id, &previous_status, &bounty.status);
        } else if resolution == resolve_cancel {
            // Escrow refund to creator goes here once escrow is implemented.
            let previous_status = bounty.status.clone();
            bounty.status = Symbol::new(&env, STATUS_CANCELLED);
            storage::store_bounty(&env, &bounty_id, &bounty);
            storage::move_bounty_status(&env, &bounty_id, &previous_status, &bounty.status);
        } else {
            panic!("resolution must be 'complete' or 'cancel'");
        }

        events::emit_dispute_resolved(&env, &bounty_id, &arbitrator, &resolution);
    }

    /// Update the on-chain metadata URI for a contributor profile.
    /// Only the contributor themselves may call this (enforced by `require_auth`).
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

    /// Cancel a bounty. Only the creator can cancel.
    /// Security-critical: prevents non-creators from cancelling and potentially
    /// triggering escrow refunds they shouldn't receive.
    pub fn cancel_bounty(env: Env, caller: Address, bounty_id: BytesN<32>) {
        caller.require_auth();

        let mut bounty = storage::get_bounty(&env, &bounty_id).expect("bounty not found");

        // Security check: only creator can cancel
        if caller != bounty.creator {
            panic!("{}", errors::NOT_BOUNTY_CREATOR);
        }

        // Guard: bounty must be open to be cancelled
        if bounty.status != Symbol::new(&env, STATUS_OPEN) {
            panic!("{}", errors::BOUNTY_NOT_OPEN);
        }

        bounty.status = Symbol::new(&env, STATUS_CANCELLED);
        storage::store_bounty(&env, &bounty_id, &bounty);

        // Note: Escrow refund will go here once escrow is implemented.
        events::emit_bounty_cancelled(&env, &bounty_id, &caller);
    }

    /// Expire an open bounty whose deadline has passed.
    /// Design choice: permissionless — any caller can trigger expiry to keep the
    /// open list clean without requiring the creator to be online. The caller
    /// still needs to authenticate (require_auth) so the transaction is signed.
    /// Once escrow is implemented this will trigger a refund to the creator.
    pub fn expire_bounty(env: Env, caller: Address, bounty_id: BytesN<32>) {
        caller.require_auth();

        let mut bounty = storage::get_bounty(&env, &bounty_id).expect("bounty not found");

        // Guard: must have a deadline set.
        let deadline = match bounty.deadline {
            Some(d) => d,
            None => panic!("{}", errors::BOUNTY_NO_DEADLINE),
        };

        // Guard: deadline must have passed.
        if env.ledger().sequence() <= deadline {
            panic!("{}", errors::DEADLINE_NOT_PASSED);
        }

        // Guard: only open bounties can be expired.
        if bounty.status != Symbol::new(&env, STATUS_OPEN) {
            panic!("{}", errors::BOUNTY_NOT_OPEN);
        }

        bounty.status = Symbol::new(&env, STATUS_CANCELLED);
        storage::store_bounty(&env, &bounty_id, &bounty);

        // Escrow refund goes here once escrow is implemented.

        events::emit_bounty_expired(&env, &bounty_id, &bounty.creator);
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
