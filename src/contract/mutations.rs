use crate::symbols::{
    self, SymbolKind, STATUS_CANCELLED, STATUS_COMPLETED, STATUS_DISPUTED, STATUS_IN_PROGRESS,
    STATUS_OPEN,
};

/// Minimum reward amount enforced at bounty creation.
const MIN_REWARD_AMOUNT: i128 = 100;

/// Verify a bounty's status is one of `allowed`, failing with `err` otherwise.
///
/// Centralizes the status-transition guards that used to be duplicated
/// across `claim_bounty`, `cancel_bounty`, and `complete_bounty`.
fn ensure_status(bounty: &Bounty, allowed: &[Symbol], err: ContractError) {
    if !allowed.iter().any(|s| s == &bounty.status) {
        fail(err);
    }
}

fn generate_bounty_id(env: &Env, count: u64) -> BountyId {
    let mut buf = [0u8; 32];
    let count_bytes = count.to_be_bytes();
    buf[24..32].copy_from_slice(&count_bytes);
    BountyId(BytesN::from_array(env, &buf))
}

/// Probe `reward_token` by invoking the SEP-41 `balance` method. Non-token
/// addresses must fail closed with `InvalidRewardToken` instead of trapping.
fn validate_reward_token(env: &Env, reward_token: &Address) {
    use soroban_sdk::{IntoVal, InvokeError, Val, Vec as SorobanVec};

    let mut args: SorobanVec<Val> = SorobanVec::new(env);
    args.push_back(env.current_contract_address().into_val(env));

    match env.try_invoke_contract::<i128, InvokeError>(
        reward_token,
        &Symbol::new(env, "balance"),
        args,
    ) {
        Ok(Ok(_)) => {}
        Ok(Err(_)) | Err(_) => fail(ContractError::InvalidRewardToken),
    }
}

/// Shared payout loop used by `complete_bounty`, `approve_completion`,
/// and `resolve_dispute`'s "complete" branch.
///
/// For each assignee, computes the proportional payout from `reward_amount`,
/// transfers the token, updates the contributor profile, and emits the
/// `reward_paid` event. Returns the primary (first) assignee address.
fn distribute_payout(
    env: &Env,
    bounty_id: &BountyId,
    assignees: &Vec<(Address, u32)>,
    from: &Address,
    token: &TokenClient,
    reward_amount: i128,
) -> Address {
    let (primary_assignee, _) = assignees.get(0).unwrap();

    for (assignee, share_bp) in assignees.iter() {
        let payout = reward_amount * (share_bp as i128) / 10_000_i128;
        token.transfer(from, &assignee, &payout);

        let mut contrib = storage::get_contributor(env, &assignee)
            .unwrap_or_else(|| Contributor::new(assignee.clone()));
        contrib.reputation += 10;
        contrib.total_earned += payout;
        contrib.contribution_count += 1;
        if contrib.active_claims > 0 {
            contrib.active_claims -= 1;
        }
        storage::store_contributor(env, &assignee, &contrib);
        events::emit_reward_paid(env, bounty_id, &assignee, &payout);
    }

    primary_assignee
}

/// Decrement a contributor's active claims counter, if greater than zero.
fn decrement_active_claims(contrib: &mut Contributor) {
    if contrib.active_claims > 0 {
        contrib.active_claims -= 1;
    }
}

/// Complete a bounty by marking it as completed and distributing payout.
/// Used as a helper by both `complete_bounty` and `approve_completion`.
///
/// Takes the already-loaded `bounty` rather than re-reading it from storage —
/// the caller has already fetched it (avoids a redundant read).
fn complete_bounty_inner(env: Env, bounty_id: BountyId, mut bounty: Bounty) {
    if bounty.status != Symbol::new(&env, STATUS_IN_PROGRESS) {
        fail(ContractError::BountyNotInProgress);
    }

    if bounty.assignees.is_empty() {
        fail(ContractError::BountyHasNoAssignee);
    }

    if !bounty.milestones.is_empty() {
        if !bounty.milestones.iter().all(|m| m.completed) {
            fail(ContractError::NotAllMilestonesCompleted);
        }
        let (primary_assignee, _) = bounty.assignees.get(0).unwrap();
        let previous_status = bounty.status.clone();
        bounty.status = Symbol::new(&env, STATUS_COMPLETED);
        storage::store_bounty(&env, &bounty_id, &bounty);
        storage::move_bounty_status(&env, &bounty_id, &previous_status, &bounty.status);
        events::emit_bounty_completed(&env, &bounty_id, &primary_assignee);
        return;
    }

    let (primary_assignee, _) = bounty.assignees.get(0).unwrap();
    let previous_status = bounty.status.clone();
    bounty.status = Symbol::new(&env, STATUS_COMPLETED);
    storage::store_bounty(&env, &bounty_id, &bounty);
    storage::move_bounty_status(&env, &bounty_id, &previous_status, &bounty.status);

    let token = TokenClient::new(&env, &bounty.reward_token);
    distribute_payout(
        &env,
        &bounty_id,
        &bounty.assignees,
        &env.current_contract_address(),
        &token,
        bounty.reward_amount,
    );

    events::emit_bounty_completed(&env, &bounty_id, &primary_assignee);
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
    /// * `reward_amount` - Raw token units for the reward. Must be at least `MIN_REWARD_AMOUNT`.
    /// * `reward_token` - Soroban token contract address used for payout.
    /// * `min_reputation` - Minimum reputation score required to claim (0 = no minimum).
    /// * `deadline` - Optional ledger sequence deadline after which the bounty cannot be claimed.
    /// * `tags` - Categorisation tags (e.g. "bug", "docs"). At most 5 tags allowed.
    /// * `max_assignees` - Maximum number of contributors who can claim this bounty (must be >= 1).
    /// * `required_verifiers` - Optional list of addresses permitted to approve completion via
    ///   `approve_completion`. When `None`, the single-verifier `complete_bounty` flow applies.
    /// * `approval_threshold` - Number of unique approvals required before completion executes
    ///   automatically. Only meaningful when `required_verifiers` is `Some`; must not exceed the
    ///   number of verifiers.
    /// * `milestones` - Optional staged payouts. When empty, the bounty is all-or-nothing.
    ///   When provided, `reward_amount` must equal the sum of all `milestone.reward` values.
    ///
    /// # Returns
    /// The newly generated `BountyId` that uniquely identifies this bounty.
    ///
    /// # Panics
    /// * If `reward_amount` is not strictly positive.
    /// * If `reward_amount` is below `MIN_REWARD_AMOUNT` (`ContractError::RewardBelowMinimum`).
    /// * If `tags.len() > 5` (`ContractError::TooManyTags`).
    /// * If `max_assignees < 1` (`ContractError::MaxAssigneesMustBePositive`).
    /// * If `approval_threshold` exceeds `required_verifiers.len()` when set
    ///   (`ContractError::ApprovalThresholdExceedsVerifiers`).
    /// * If `reward_token` is not a valid Soroban token contract.
    /// * If milestone reward summation overflows (`ContractError::RewardAmountOverflow`).
    /// * If `milestones` is non-empty and their rewards do not sum to `reward_amount`.
    ///
    /// # Authorization
    /// Requires auth from `creator`.
    pub fn create_bounty(
        env: Env,
        creator: Address,
        title: Symbol,
        description: String,
        reward_amount: i128,
        reward_token: Address,
        min_reputation: u32,
        deadline: Option<u32>,
        tags: Vec<Symbol>,
        max_assignees: u32,
        required_verifiers: Option<Vec<Address>>,
        approval_threshold: u32,
        milestones: Vec<Milestone>,
    ) -> BountyId {
        if reward_amount <= 0 {
            fail(ContractError::RewardMustBePositive);
        }

        if reward_amount < MIN_REWARD_AMOUNT {
            fail(ContractError::RewardBelowMinimum);
        }

        if tags.len() > 5 {
            fail(ContractError::TooManyTags);
        }

        for tag in tags.iter() {
            symbols::validate_symbol_or_fail(&env, SymbolKind::Tag, &tag);
        }

        if max_assignees < 1 {
            fail(ContractError::MaxAssigneesMustBePositive);
        }

        if let Some(ref verifiers) = required_verifiers {
            if approval_threshold > verifiers.len() {
                fail(ContractError::ApprovalThresholdExceedsVerifiers);
            }
        }

        validate_reward_token(&env, &reward_token);

        if !milestones.is_empty() {
            let mut total: i128 = 0;
            for m in milestones.iter() {
                total = match total.checked_add(m.reward) {
                    Some(sum) => sum,
                    None => fail(ContractError::RewardAmountOverflow),
                };
            }
            if total != reward_amount {
                fail(ContractError::MilestoneRewardsMismatch);
            }
        }

        if let Some(deadline) = deadline {
            if env.ledger().sequence() > deadline {
                fail(ContractError::BountyDeadlinePassed);
            }
        }

        creator.require_auth();

        let count = storage::get_bounty_count(&env);
        let id = generate_bounty_id(&env, count);

        let bounty = Bounty {
            creator: creator.clone(),
            reward_amount,
            reward_token,
            assignees: Vec::new(&env),
            max_assignees,
            status: Symbol::new(&env, STATUS_OPEN),
            min_reputation,
            deadline,
            required_verifiers,
            approval_threshold,
            tags,
            milestones,
        };

        storage::store_bounty(&env, &id, &bounty);
        storage::store_bounty_meta(&env, &id, &BountyMeta { title, description });
        storage::set_bounty_count(&env, &(count + 1));
        storage::add_bounty_to_status(&env, &id, &bounty.status);
        storage::append_creator_bounty(&env, &creator, &id);
        storage::add_open_bounty(&env, &id);

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
            None => fail(ContractError::BountyNotFound),
        };

        // Idempotency: reject a duplicate claim by the same contributor before
        // any other state checks or storage mutations.
        for (addr, _) in bounty.assignees.iter() {
            if addr == contributor {
                fail(ContractError::AlreadyClaimed);
            }
        }

        // GUARD: a bounty in a terminal/blocked state can never be claimed.
        // Note this is intentionally broader than `status == STATUS_OPEN`: a
        // multi-assignee bounty moves to "in_progress" after its first claim
        // and must remain claimable by further contributors while capacity
        // remains (enforced below by the max_assignees check).
        ensure_status(
            &bounty,
            &[
                Symbol::new(&env, STATUS_OPEN),
                Symbol::new(&env, STATUS_IN_PROGRESS),
            ],
            ContractError::BountyNotOpen,
        );

        // The creator of a bounty cannot claim their own bounty.
        if contributor == bounty.creator {
            fail(ContractError::CreatorCannotClaim);
        }

        if bounty.assignees.len() >= bounty.max_assignees {
            fail(ContractError::BountyAlreadyAssigned);
        }

        // #275: use Contributor::new for default construction (DONE - all call sites updated)
        let mut contrib = storage::get_contributor(&env, &contributor)
            .unwrap_or_else(|| Contributor::new(contributor.clone()));

        if contrib.active_claims >= 1 {
            fail(ContractError::ContributorHasActiveClaim);
        }

        // Deadline enforcement: reject claims once the deadline ledger sequence has passed.
        if let Some(deadline) = bounty.deadline {
            if env.ledger().sequence() > deadline {
                fail(ContractError::BountyDeadlinePassed);
            }
        }

        if bounty.min_reputation > 0 && contrib.reputation < bounty.min_reputation {
            fail(ContractError::ReputationTooLow);
        }

        // Compute per-assignee share as an equal split of 10,000 basis points.
        // The first assignee receives any remainder from the division.
        let base_share: u32 = 10_000 / bounty.max_assignees;
        let remainder: u32 = 10_000 % bounty.max_assignees;
        let share_bp = if bounty.assignees.is_empty() {
            base_share + remainder
        } else {
            base_share
        };
        bounty.assignees.push_back((contributor.clone(), share_bp));

        let previous_status = bounty.status.clone();
        bounty.status = Symbol::new(&env, STATUS_IN_PROGRESS);
        storage::store_bounty(&env, &bounty_id, &bounty);
        storage::move_bounty_status(&env, &bounty_id, &previous_status, &bounty.status);

        contrib.active_claims += 1;
        storage::store_contributor(&env, &contributor, &contrib);

        // Remove from open bounties list.
        storage::remove_open_bounty(&env, &bounty_id);

        events::emit_bounty_claimed(&env, &bounty_id, &contributor);
    }

    /// Complete a single milestone and pay out its reward.
    ///
    /// Transfers `milestone.reward` from `verifier` to each assignee proportionally.
    /// The milestone is marked `completed` so it cannot be paid out twice.
    ///
    /// # Arguments
    /// * `verifier` - Wallet that holds the tokens and initiates the payout.
    /// * `bounty_id` - The bounty containing the milestone.
    /// * `milestone_index` - Zero-based index of the milestone to complete.
    ///
    /// # Panics
    /// * If `bounty_id` does not exist.
    /// * If the bounty status is not `"in_progress"`.
    /// * If the bounty has no assignees.
    /// * If `milestone_index` is out of bounds.
    /// * If the milestone is already completed.
    /// * If `verifier` is one of the bounty assignees.
    ///
    /// # Authorization
    /// `verifier.require_auth()` is the **first** operation.
    pub fn complete_milestone(
        env: Env,
        verifier: Address,
        bounty_id: BountyId,
        milestone_index: u32,
    ) {
        verifier.require_auth();

        let mut bounty = match storage::get_bounty(&env, &bounty_id) {
            Some(b) => b,
            None => fail(ContractError::BountyNotFound),
        };

        if bounty.status != Symbol::new(&env, STATUS_IN_PROGRESS) {
            fail(ContractError::BountyNotInProgress);
        }

        if bounty.assignees.is_empty() {
            fail(ContractError::BountyHasNoAssignee);
        }

        let idx = milestone_index;
        if idx >= bounty.milestones.len() {
            fail(ContractError::InvalidMilestoneIndex);
        }

        let mut milestone = bounty.milestones.get(idx).unwrap().clone();
        if milestone.completed {
            fail(ContractError::MilestoneAlreadyCompleted);
        }

        for (assignee, _) in bounty.assignees.iter() {
            if assignee == verifier {
                fail(ContractError::VerifierCannotBeAssignee);
            }
        }

        milestone.completed = true;
        let milestone_reward = milestone.reward;
        bounty.milestones.set(idx, milestone);

        let token = TokenClient::new(&env, &bounty.reward_token);
        distribute_payout(
            &env,
            &bounty_id,
            &bounty.assignees,
            &env.current_contract_address(),
            &token,
            milestone_reward,
        );

        let completed_milestone = bounty.milestones.get(idx).unwrap();
        events::emit_milestone_completed(
            &env,
            &bounty_id,
            milestone_index,
            &completed_milestone.reward,
        );

        storage::store_bounty(&env, &bounty_id, &bounty);
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
    /// * If the bounty status is not `"in_progress"` (prevents double-completion).
    /// * If the bounty has no assignees.
    /// * If `verifier` is one of the bounty assignees (prevents self-verification).
    /// * If the token transfer fails (insufficient balance, no allowance, etc.).
    ///
    /// # Authorization
    /// `verifier.require_auth()` is the **first** operation in this function.
    /// No storage reads or business logic execute before authentication is checked.
    pub fn complete_bounty(env: Env, verifier: Address, bounty_id: BountyId) {
        // AUTH: must be first — no storage reads or side-effects before this line.
        verifier.require_auth();

        let bounty = match storage::get_bounty(&env, &bounty_id) {
            Some(b) => b,
            None => fail(ContractError::BountyNotFound),
        };

        // GUARD 1 — dispute prevention.
        // If the bounty is in disputed status, complete_bounty must not execute.
        // A disputed bounty must be resolved via resolve_dispute first.
        if bounty.status == Symbol::new(&env, STATUS_DISPUTED) {
            fail(ContractError::BountyIsDisputed);
        }

        // GUARD 2 — double-completion prevention.
        // Reject the call if the bounty is not currently in progress. This blocks
        // repeat calls on already-completed bounties and any other terminal state.
        // Depends on claim_bounty having written STATUS_IN_PROGRESS and
        // complete_bounty writing STATUS_COMPLETED below (checks-effects-interactions).
        ensure_status(
            &bounty,
            &[Symbol::new(&env, STATUS_IN_PROGRESS)],
            ContractError::BountyNotInProgress,
        );

        if bounty.assignees.is_empty() {
            fail(ContractError::BountyHasNoAssignee);
        }

        // GUARD 3 — self-verification prevention.
        // The verifier must be a party independent from the assignees. Allowing the
        // same address to both claim and verify would let a contributor manufacture
        // reputation and, once escrow is introduced, drain contract funds unilaterally.
        for (assignee, _) in bounty.assignees.iter() {
            if assignee == verifier {
                fail(ContractError::VerifierCannotBeAssignee);
            }
        }

        complete_bounty_inner(env, bounty_id, bounty);
    }

    /// Record one verifier's approval for a multi-sig bounty completion.
    /// When the number of unique approvals reaches approval_threshold, completion executes automatically.
    /// Falls back to single-verifier behaviour when required_verifiers is None (any verifier completes directly).
    ///
    /// # Authorization
    /// `verifier.require_auth()` is the **first** operation in this function.
    /// No storage reads or business logic execute before authentication is checked.
    pub fn approve_completion(env: Env, verifier: Address, bounty_id: BountyId) {
        verifier.require_auth();

        let mut bounty = match storage::get_bounty(&env, &bounty_id) {
            Some(b) => b,
            None => fail(ContractError::BountyNotFound),
        };

        if bounty.assignees.is_empty() {
            fail(ContractError::BountyHasNoAssignee);
        }

        for (assignee, _) in bounty.assignees.iter() {
            if assignee == verifier {
                fail(ContractError::VerifierCannotBeAssignee);
            }
        }

        // If no required_verifiers list is set, fall back to immediate single-verifier completion.
        if bounty.required_verifiers.is_none() {
            complete_bounty_inner(env, bounty_id, bounty);
            return;
        }

        let required = bounty.required_verifiers.clone().unwrap();
        let is_authorized = required.iter().any(|v| v == verifier);
        if !is_authorized {
            fail(ContractError::VerifierNotAuthorized);
        }

        let mut approvals = storage::get_approvals(&env, &bounty_id);

        // Guard against duplicate votes from the same verifier.
        let already_voted = approvals.iter().any(|v| v == verifier);
        if already_voted {
            fail(ContractError::AlreadyApproved);
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
            if !bounty.milestones.is_empty() {
                if !bounty.milestones.iter().all(|m| m.completed) {
                    fail(ContractError::NotAllMilestonesCompleted);
                }
                let previous_status = bounty.status.clone();
                bounty.status = Symbol::new(&env, STATUS_COMPLETED);
                storage::store_bounty(&env, &bounty_id, &bounty);
                storage::move_bounty_status(&env, &bounty_id, &previous_status, &bounty.status);
                let (primary_assignee, _) = bounty.assignees.get(0).unwrap();
                events::emit_bounty_completed(&env, &bounty_id, &primary_assignee);
                return;
            }

            let token = TokenClient::new(&env, &bounty.reward_token);
            distribute_payout(
                &env,
                &bounty_id,
                &bounty.assignees,
                &env.current_contract_address(),
                &token,
                bounty.reward_amount,
            );

            let (primary_assignee, _) = bounty.assignees.get(0).unwrap();
            let previous_status = bounty.status.clone();
            bounty.status = Symbol::new(&env, STATUS_COMPLETED);
            storage::store_bounty(&env, &bounty_id, &bounty);
            storage::move_bounty_status(&env, &bounty_id, &previous_status, &bounty.status);
            events::emit_bounty_completed(&env, &bounty_id, &primary_assignee);
        }
    }

    /// Raise a dispute on a bounty.
    ///
    /// Only the bounty creator or an existing assignee may call this.
    /// Transitions the bounty status to `"disputed"`.
    ///
    /// # Authorization
    /// `caller.require_auth()` is the **first** operation in this function.
    /// No storage reads or business logic execute before authentication is checked.
    pub fn raise_dispute(env: Env, caller: Address, bounty_id: BountyId) {
        caller.require_auth();

        let mut bounty = match storage::get_bounty(&env, &bounty_id) {
            Some(b) => b,
            None => fail(ContractError::BountyNotFound),
        };

        if bounty.status == Symbol::new(&env, STATUS_DISPUTED) {
            fail(ContractError::BountyIsDisputed);
        }

        if bounty.status != Symbol::new(&env, STATUS_OPEN)
            && bounty.status != Symbol::new(&env, STATUS_IN_PROGRESS)
        {
            fail(ContractError::BountyNotDisputed);
        }

        let is_assignee = bounty.assignees.iter().any(|(addr, _)| addr == caller);
        if caller != bounty.creator && !is_assignee {
            fail(ContractError::OnlyCreatorOrAssigneeCanDispute);
        }

        let previous_status = bounty.status.clone();
        bounty.status = Symbol::new(&env, STATUS_DISPUTED);
        storage::store_bounty(&env, &bounty_id, &bounty);
        storage::move_bounty_status(&env, &bounty_id, &previous_status, &bounty.status);
        events::emit_bounty_disputed(&env, &bounty_id, &caller);
    }

    /// Resolve a disputed bounty. Only the bounty creator (acting as arbitrator) may call this.
    /// resolution must be the Symbol "complete" (pay assignees) or "cancel" (refund creator).
    ///
    /// When resolution is "complete", the arbitrator's wallet funds the payout to each assignee
    /// (mirroring complete_bounty's verifier-funds-the-payout model), since the contract itself
    /// holds no escrow.
    ///
    /// # Authorization
    /// `arbitrator.require_auth()` is the **first** operation in this function.
    /// No storage reads or business logic execute before authentication is checked.
    pub fn resolve_dispute(env: Env, arbitrator: Address, bounty_id: BountyId, resolution: Symbol) {
        arbitrator.require_auth();

        let mut bounty = match storage::get_bounty(&env, &bounty_id) {
            Some(b) => b,
            None => fail(ContractError::BountyNotFound),
        };

        if bounty.status != Symbol::new(&env, STATUS_DISPUTED) {
            fail(ContractError::BountyNotDisputed);
        }

        // The arbitrator must be the bounty creator; there is no separate admin address.
        if arbitrator != bounty.creator {
            fail(ContractError::NotArbitrator);
        }

        // GUARD: arbitrator (creator) must meet the bounty's own min_reputation threshold.
        // This reflects the security/minimum-reputation-enforcement.md recommendation
        // that dispute resolvers are subject to a reputation floor.
        let arbitrator_contrib = storage::get_contributor(&env, &arbitrator)
            .unwrap_or_else(|| Contributor::new(arbitrator.clone()));
        if bounty.min_reputation > 0 && arbitrator_contrib.reputation < bounty.min_reputation {
            fail(ContractError::ReputationTooLow);
        }

        let resolve_complete = Symbol::new(&env, "complete");
        let resolve_cancel = Symbol::new(&env, "cancel");

        if resolution == resolve_complete {
            if bounty.assignees.is_empty() {
                fail(ContractError::BountyHasNoAssignee);
            }

            if !bounty.milestones.is_empty() {
                if !bounty.milestones.iter().all(|m| m.completed) {
                    fail(ContractError::NotAllMilestonesCompleted);
                }
                let previous_status = bounty.status.clone();
                bounty.status = Symbol::new(&env, STATUS_COMPLETED);
                storage::store_bounty(&env, &bounty_id, &bounty);
                storage::move_bounty_status(&env, &bounty_id, &previous_status, &bounty.status);
            } else {
                let token = TokenClient::new(&env, &bounty.reward_token);

                for (assignee, share_bp) in bounty.assignees.iter() {
                    let payout = bounty.reward_amount * (share_bp as i128) / 10_000_i128;
                    token.transfer(&arbitrator, &assignee, &payout);

                    let mut contrib = storage::get_contributor(&env, &assignee)
                        .unwrap_or_else(|| Contributor::new(assignee.clone()));

                    contrib.reputation += 10;
                    contrib.total_earned += payout;
                    contrib.contribution_count += 1;
                    decrement_active_claims(&mut contrib);

                    storage::store_contributor(&env, &assignee, &contrib);
                    events::emit_reward_paid(&env, &bounty_id, &assignee, &payout);
                }

                let previous_status = bounty.status.clone();
                bounty.status = Symbol::new(&env, STATUS_COMPLETED);
                storage::store_bounty(&env, &bounty_id, &bounty);
                storage::move_bounty_status(&env, &bounty_id, &previous_status, &bounty.status);
            }
        } else if resolution == resolve_cancel {
            // Refund escrowed reward to creator before mutating status.
            let token = TokenClient::new(&env, &bounty.reward_token);
            token.transfer(
                &env.current_contract_address(),
                &bounty.creator,
                &bounty.reward_amount,
            );

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

        if metadata == Symbol::new(&env, "") {
            fail(ContractError::MetadataEmpty);
        }

        // #275: use Contributor::new for default construction (DONE - all call sites updated)
        let mut contrib = storage::get_contributor(&env, &contributor)
            .unwrap_or_else(|| Contributor::new(contributor.clone()));

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

        let mut bounty = match storage::get_bounty(&env, &bounty_id) {
            Some(b) => b,
            None => fail(ContractError::BountyNotFound),
        };

        if caller != bounty.creator {
            fail(ContractError::NotBountyCreator);
        }

        // Terminal statuses must never be cancelled (or re-cancelled).
        if bounty.status == Symbol::new(&env, STATUS_COMPLETED)
            || bounty.status == Symbol::new(&env, STATUS_CANCELLED)
        {
            fail(ContractError::BountyNotOpen);
        }

        ensure_status(
            &bounty,
            &[Symbol::new(&env, STATUS_OPEN)],
            ContractError::BountyNotOpen,
        );

        // Refund escrowed reward to creator before mutating status.
        let token = TokenClient::new(&env, &bounty.reward_token);
        token.transfer(
            &env.current_contract_address(),
            &bounty.creator,
            &bounty.reward_amount,
        );

        let previous_status = bounty.status.clone();
        bounty.status = Symbol::new(&env, STATUS_CANCELLED);
        storage::store_bounty(&env, &bounty_id, &bounty);
        storage::move_bounty_status(&env, &bounty_id, &previous_status, &bounty.status);

        events::emit_bounty_cancelled(&env, &bounty_id, &caller);
    }

    /// Expire an open bounty whose deadline has passed.
    ///
    /// Any authenticated address may trigger expiry — no privileged role is required,
    /// but the caller must still sign the transaction. The bounty must have a deadline
    /// set and the current ledger sequence must exceed that deadline.
    /// Transitions to `"cancelled"`.
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

        let mut bounty = match storage::get_bounty(&env, &bounty_id) {
            Some(b) => b,
            None => fail(ContractError::BountyNotFound),
        };

        let deadline = match bounty.deadline {
            Some(d) => d,
            None => fail(ContractError::BountyNoDeadline),
        };

        if env.ledger().sequence() <= deadline {
            fail(ContractError::DeadlineNotPassed);
        }

        if bounty.status != Symbol::new(&env, STATUS_OPEN) {
            fail(ContractError::BountyNotOpen);
        }

        // Refund escrowed reward to creator before mutating status.
        let token = TokenClient::new(&env, &bounty.reward_token);
        token.transfer(
            &env.current_contract_address(),
            &bounty.creator,
            &bounty.reward_amount,
        );

        let previous_status = bounty.status.clone();
        bounty.status = Symbol::new(&env, STATUS_CANCELLED);
        storage::store_bounty(&env, &bounty_id, &bounty);
        storage::move_bounty_status(&env, &bounty_id, &previous_status, &bounty.status);

        events::emit_bounty_expired(&env, &bounty_id, &bounty.creator);
    }
}
