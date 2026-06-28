use soroban_sdk::{contractimpl, token::TokenClient, Address, BytesN, Env, Symbol, Vec};

use crate::errors;
use crate::events;
use crate::storage;
use crate::types::{Bounty, BountyId, BountyMeta, Contributor};

const STATUS_OPEN: &str = "open";
const STATUS_IN_PROGRESS: &str = "in_progress";
const STATUS_COMPLETED: &str = "completed";
const STATUS_CANCELLED: &str = "cancelled";
const STATUS_DISPUTED: &str = "disputed";

fn generate_bounty_id(env: &Env, count: u64) -> BountyId {
    let mut buf = [0u8; 32];
    let count_bytes = count.to_be_bytes();
    buf[24..32].copy_from_slice(&count_bytes);
    BountyId(BytesN::from_array(env, &buf))
}

#[contractimpl]
impl MergeMintContract {
    /// Create a new bounty.
    ///
    /// The caller must be the `creator` (enforced by `require_auth`).
    /// The bounty is initialised with `"open"` status and `max_assignees = 1`.
    ///
    /// # Arguments
    /// * `creator` - Wallet that will own and manage this bounty.
    /// * `title` - Short human-readable title (max 32 chars via `Symbol`).
    /// * `description` - Longer description of the work required.
    /// * `reward_amount` - Raw token units for the reward. Must be positive.
    /// * `reward_token` - Soroban token contract address used for payout.
    /// * `min_reputation` - Minimum reputation score required to claim (0 = no minimum).
    /// * `deadline` - Optional ledger sequence deadline after which the bounty cannot be claimed.
    ///
    /// # Returns
    /// The newly generated `BountyId` that uniquely identifies this bounty.
    ///
    /// # Authorization
    /// Requires auth from `creator`.
    pub fn create_bounty(
        env: Env,
        creator: Address,
        title: Symbol,
        description: Symbol,
        reward_amount: i128,
        reward_token: Address,
        min_reputation: u32,
        deadline: Option<u32>,
    ) -> BountyId {
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

    /// Claim an open bounty.
    ///
    /// A contributor receives 10 000 basis points (full reward) when claiming a
    /// single-assignee bounty (`max_assignees == 1`). The contributor is added to
    /// the bounty's `assignees` list and the status transitions to `"in_progress"`.
    ///
    /// # Arguments
    /// * `contributor` - Wallet claiming the bounty.
    /// * `bounty_id` - The bounty to claim.
    ///
    /// # Panics
    /// * If `bounty_id` does not exist.
    /// * If the bounty is already at `max_assignees` capacity.
    /// * If the contributor is already an assignee on this bounty.
    /// * If the contributor already has an active claim on another bounty.
    /// * If the bounty deadline has passed (`env.ledger().sequence() > deadline`).
    /// * If the contributor's reputation is below `min_reputation`.
    ///
    /// # Authorization
    /// Requires auth from `contributor`.
    pub fn claim_bounty(env: Env, contributor: Address, bounty_id: BountyId) {
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

        // Deadline enforcement: reject claims once the deadline ledger sequence has passed.
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

        // Remove from open bounties list.
        let open = storage::get_open_bounties(&env);
        let mut new_open = Vec::new(&env);
        for existing_id in open.iter() {
            if existing_id != bounty_id {
                new_open.push_back(existing_id);
            }
        }
        storage::set_open_bounties(&env, &new_open);

        events::emit_bounty_claimed(&env, &bounty_id, &contributor);
    }

    /// Complete a bounty and distribute the reward.
    ///
    /// Transfers `reward_amount` from `verifier` to each assignee proportionally
    /// according to their basis-point share. Each assignee's reputation increases
    /// by 10 and their `active_claims` is decremented. The bounty status transitions
    /// to `"completed"`.
    ///
    /// # Arguments
    /// * `verifier` - Wallet that holds the tokens and initiates the payout.
    /// * `bounty_id` - The bounty to complete.
    ///
    /// # Panics
    /// * If `bounty_id` does not exist.
    /// * If the bounty has no assignees.
    /// * If the token transfer fails (insufficient balance, no allowance, etc.).
    ///
    /// # Authorization
    /// Requires auth from `verifier`.
    pub fn complete_bounty(env: Env, verifier: Address, bounty_id: BountyId) {
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

    /// Raise a dispute on a bounty.
    ///
    /// Only the bounty creator or an existing assignee may call this.
    /// Transitions the bounty status to `"disputed"`.
    ///
    /// # Arguments
    /// * `caller` - Wallet raising the dispute.
    /// * `bounty_id` - The bounty to dispute.
    ///
    /// # Panics
    /// * If `bounty_id` does not exist.
    /// * If `caller` is neither the creator nor an assignee.
    ///
    /// # Authorization
    /// Requires auth from `caller`.
    pub fn raise_dispute(env: Env, caller: Address, bounty_id: BountyId) {
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

    /// Update the on-chain metadata URI for a contributor profile.
    ///
    /// The `metadata` value is typically an IPFS hash or URL pointing to a JSON
    /// document containing the contributor's name, avatar, GitHub username, etc.
    ///
    /// # Arguments
    /// * `contributor` - Wallet whose metadata is being updated.
    /// * `metadata` - New metadata URI (`Symbol`) to store.
    ///
    /// # Authorization
    /// Requires auth from `contributor`. No other address may modify this field.
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

    /// Cancel an open bounty.
    ///
    /// Only the creator may cancel. The bounty must be in `"open"` status.
    /// Transitions the bounty status to `"cancelled"`.
    ///
    /// # Arguments
    /// * `caller` - Wallet requesting cancellation (must be the creator).
    /// * `bounty_id` - The bounty to cancel.
    ///
    /// # Panics
    /// * If `bounty_id` does not exist.
    /// * If `caller` is not the bounty creator.
    /// * If the bounty is not in `"open"` status.
    ///
    /// # Authorization
    /// Requires auth from `caller`. Only the creator may cancel.
    pub fn cancel_bounty(env: Env, caller: Address, bounty_id: BountyId) {
        caller.require_auth();

        let mut bounty = storage::get_bounty(&env, &bounty_id).expect("bounty not found");

        if caller != bounty.creator {
            panic!("{}", errors::NOT_BOUNTY_CREATOR);
        }

        if bounty.status != Symbol::new(&env, STATUS_OPEN) {
            panic!("{}", errors::BOUNTY_NOT_OPEN);
        }

        bounty.status = Symbol::new(&env, STATUS_CANCELLED);
        storage::store_bounty(&env, &bounty_id, &bounty);

        // Note: Escrow refund will go here once escrow is implemented.
        events::emit_bounty_cancelled(&env, &bounty_id, &caller);
    }

    /// Expire an open bounty whose deadline has passed.
    ///
    /// Permissionless: any caller may trigger expiry to keep the open-bounty list
    /// clean. The bounty must have a deadline set and the current ledger sequence
    /// must exceed that deadline. Transitions to `"cancelled"`.
    ///
    /// # Arguments
    /// * `caller` - Wallet triggering the expiry (any authenticated address).
    /// * `bounty_id` - The bounty to expire.
    ///
    /// # Panics
    /// * If `bounty_id` does not exist.
    /// * If the bounty has no deadline set.
    /// * If the deadline has not yet passed.
    /// * If the bounty is not in `"open"` status.
    ///
    /// # Authorization
    /// Requires auth from `caller` (any authenticated wallet may trigger expiry).
    pub fn expire_bounty(env: Env, caller: Address, bounty_id: BountyId) {
        caller.require_auth();

        let mut bounty = storage::get_bounty(&env, &bounty_id).expect("bounty not found");

        let deadline = match bounty.deadline {
            Some(d) => d,
            None => panic!("{}", errors::BOUNTY_NO_DEADLINE),
        };

        if env.ledger().sequence() <= deadline {
            panic!("{}", errors::DEADLINE_NOT_PASSED);
        }

        if bounty.status != Symbol::new(&env, STATUS_OPEN) {
            panic!("{}", errors::BOUNTY_NOT_OPEN);
        }

        bounty.status = Symbol::new(&env, STATUS_CANCELLED);
        storage::store_bounty(&env, &bounty_id, &bounty);

        // Escrow refund goes here once escrow is implemented.
        events::emit_bounty_expired(&env, &bounty_id, &bounty.creator);
    }

    /// Retrieve a bounty by its unique identifier.
    pub fn get_bounty(env: Env, bounty_id: BountyId) -> Option<Bounty> {
        storage::get_bounty(&env, &bounty_id)
    }

    /// Retrieve the metadata (title, description) for a bounty.
    pub fn get_bounty_meta(env: Env, bounty_id: BountyId) -> Option<BountyMeta> {
        storage::get_bounty_meta(&env, &bounty_id)
    }

    /// Retrieve a contributor profile by wallet address.
    pub fn get_contributor(env: Env, address: Address) -> Option<Contributor> {
        storage::get_contributor(&env, &address)
    }

    /// Return the total number of bounties ever created.
    pub fn get_bounty_count(env: Env) -> u64 {
        storage::get_bounty_count(&env)
    }

    /// Retrieve all bounty IDs currently in a given status.
    pub fn get_bounties_by_status(env: Env, status: Symbol) -> Vec<BountyId> {
        storage::get_bounties_by_status(&env, &status)
    }

    /// Retrieve all currently open bounty IDs.
    pub fn get_open_bounties(env: Env) -> Vec<BountyId> {
        storage::get_open_bounties(&env)
    }
}
