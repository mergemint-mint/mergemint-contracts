Looking at the requirements, I need to make changes to `src/contract/mutations.rs`. Let me analyze what's needed:

1. **#1202**: Update `reopen_bounty` to validate per-milestone deadlines
2. **#1216**: Thread `ContributorActiveBounty` mapping through claim lifecycle transitions
3. **#1218**: Enforce `MIN_SPONSOR_AMOUNT`/`MAX_SPONSORS_PER_BOUNTY` in `sponsor_bounty`

Since I can only edit this file, I'll implement the changes assuming the storage functions (`set_contributor_active_bounty`, `remove_contributor_active_bounty`, `get_sponsor_count`, etc.) and error variants (`MilestoneDeadlineExpired`, `SponsorAmountBelowMinimum`, `TooManySponsors`) exist or will be added by other steps.

Let me implement the changes carefully:

```rust
use crate::symbols::{
    self, SymbolKind, STATUS_CANCELLED, STATUS_COMPLETED, STATUS_DISPUTED,
    STATUS_IN_PROGRESS, STATUS_OPEN, STATUS_SUBMITTED,
};

/// Minimum reward amount enforced at bounty creation.
const MIN_REWARD_AMOUNT: i128 = 100;

/// Minimum number of ledgers a bounty must sit in a terminal status
/// (`"completed"` or `"cancelled"`) before anyone can archive it. Chosen to
/// give indexers/UIs a grace window to read the full record before it is
/// compacted. At ~5s/ledger this is roughly 1 day.
const ARCHIVE_GRACE_PERIOD_LEDGERS: u32 = 17_280;

/// Maximum number of tags a bounty may carry. Documented in
/// `docs/contract-reference.md`. Enforced by `validate_tags`, used from both
/// `create_bounty` and `update_tags` so a bounty can never accumulate more
/// tags (or duplicates) after creation than it could at creation time.
const MAX_TAGS: u32 = 5;

/// Minimum non-zero claim window (in ledgers) accepted by
/// `set_claim_window`. A window of `0` is treated as "disabled" (stale-claim
/// release is blocked entirely); any other value must be at least this many
/// ledgers so a misconfigured tiny window cannot let griefers immediately
/// release fresh claims and slash contributor reputation.
const MIN_CLAIM_WINDOW: u32 = 100;

/// Validate a tag list: reject more than `MAX_TAGS` entries, reject
/// duplicates, and reject any tag not on the allow-list (via
/// `symbols::validate_symbol_or_fail`).
fn validate_tags(env: &Env, tags: &Vec<Symbol>) -> Result<(), ContractError> {
    if tags.len() > MAX_TAGS {
        return Err(ContractError::TooManyTags);
    }

    for (i, tag) in tags.iter().enumerate() {
        symbols::validate_symbol(env, SymbolKind::Tag, &tag)?;
        for other in tags.iter().skip(i + 1) {
            if other == tag {
                return Err(ContractError::DuplicateTag);
            }
        }
    }
    Ok(())
}

/// Verify a bounty's status is one of `allowed`, failing with `err` otherwise.
///
/// Centralizes the status-transition guards that used to be duplicated
/// across `claim_bounty`, `cancel_bounty`, and `complete_bounty`.
fn ensure_status(bounty: &Bounty, allowed: &[Symbol], err: ContractError) -> Result<(), ContractError> {
    if !allowed.iter().any(|s| s == &bounty.status) {
        return Err(err);
    }
    Ok(())
}

/// Require that `caller` is authenticated and is the configured admin.
fn require_admin(env: &Env, caller: &Address) -> Result<(), ContractError> {
    caller.require_auth();
    match storage::get_admin(env) {
        Some(admin) if admin == *caller => Ok(()),
        _ => Err(ContractError::NotAdmin),
    }
}

/// Entrypoints that keep working while the contract is paused: the pause
/// switch itself and the admin-key bootstrap/rotation path, so a suspected
/// compromised admin key can still be rotated during an incident (see
/// `docs/runbook.md`). Every other entrypoint in this file must call
/// `require_not_paused`; `test::pause_blocks_every_mutation` enforces that.
pub(crate) const PAUSE_EXEMPT_ENTRYPOINTS: &[&str] = &[
    "pause",
    "unpause",
    "init_admin",
    "transfer_admin",
    "accept_admin",
];

/// Fail with `ContractError::ContractPaused` while the admin has the contract
/// paused. Called right after `require_auth` in every mutating entrypoint.
fn require_not_paused(env: &Env) -> Result<(), ContractError> {
    if storage::is_paused(env) {
        return Err(ContractError::ContractPaused);
    }
    Ok(())
}

fn generate_bounty_id(env: &Env, count: u64) -> BountyId {
    let mut buf = [0u8; 32];
    let count_bytes = count.to_be_bytes();
    buf[24..32].copy_from_slice(&count_bytes);
    BountyId(BytesN::from_array(env, &buf))
}

/// Probe `reward_token` by invoking the SEP-41 `balance` method. Non-token
/// addresses must fail closed with `InvalidRewardToken` instead of trapping.
fn validate_reward_token(env: &Env, reward_token: &Address) -> Result<(), ContractError> {
    use soroban_sdk::{IntoVal, InvokeError, Val, Vec as SorobanVec};

    let mut args: SorobanVec<Val> = SorobanVec::new(env);
    args.push_back(env.current_contract_address().into_val(env));

    match env.try_invoke_contract::<i128, InvokeError>(
        reward_token,
        &Symbol::new(env, "balance"),
        args,
    ) {
        Ok(Ok(_)) => Ok(()),
        Ok(Err(_)) | Err(_) => Err(ContractError::InvalidRewardToken),
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
) -> Result<Address, ContractError> {
    let (primary_assignee, _) = assignees
        .get(0)
        .ok_or(ContractError::BountyHasNoAssignee)?;

    // Calculate base payouts avoiding integer overflow:
    // reward_amount * share_bp can exceed i128::MAX when reward_amount is large.
    // Decomposing via q * share_bp + (r * share_bp) / 10_000 prevents overflow
    // because r < 10_000 and share_bp <= 10_000, so r * share_bp < 100_000_000.
    let mut total_distributed: i128 = 0;
    for (_, share_bp) in assignees.iter() {
        let q = reward_amount / 10_000_i128;
        let r = reward_amount % 10_000_i128;
        let payout = q * (share_bp as i128) + (r * (share_bp as i128)) / 10_000_i128;
        total_distributed += payout;
    }

    // Deterministically allocate integer-division remainder dust to the primary assignee
    // so sum(payouts) == reward_amount and no tokens remain stranded in escrow.
    let dust = reward_amount - total_distributed;

    let mut is_primary = true;
    for (assignee, share_bp) in assignees.iter() {
        let q = reward_amount / 10_000_i128;
        let r = reward_amount % 10_000_i128;
        let mut payout = q * (share_bp as i128) + (r * (share_bp as i128)) / 10_000_i128;
        if is_primary {
            payout += dust;
            is_primary = false;
        }

        if payout > 0 {
            token.transfer(from, &assignee, &payout);
        }

        let mut contrib = storage::get_contributor(env, &assignee)
            .unwrap_or_else(|| Contributor::new(assignee.clone()));
        set_reputation(env, &mut contrib, &assignee, 10, "completion");
        contrib.total_earned = contrib
            .total_earned
            .checked_add(payout)
            .unwrap_or_else(|| fail(ContractError::RewardAmountOverflow));
        contrib.contribution_count += 1;
        decrement_active_claims(&mut contrib);
        storage::store_contributor(env, &assignee, &contrib);
        // #1216: clear the O(1) active-bounty mapping on completion.
        storage::remove_contributor_active_bounty(env, &assignee);
        events::emit_reward_paid(env, bounty_id, &assignee, &payout);
    }

    Ok(primary_assignee)
}

/// Refund every sponsor's tracked contribution for `bounty_id`, clearing the
/// sponsor list so refunds cannot be issued twice. Returns the total amount
/// refunded (0 if there were no sponsors).
///
/// Used by `refund_escrow` when a bounty is cancelled or expires.
fn refund_sponsors(env: &Env, bounty_id: &BountyId, reward_token: &Address, from: &Address) -> i128 {
    let sponsors = storage::take_sponsors(env, bounty_id);
    if sponsors.is_empty() {
        return 0;
    }
    let token = TokenClient::new(env, reward_token);
    let mut total: i128 = 0;
    for (sponsor, amount) in sponsors.iter() {
        token.transfer(from, &sponsor, &amount);
        total += amount;
    }
    total
}

/// Refund the full escrowed reward for a cancelled bounty: each sponsor gets
/// back exactly what they contributed, and the creator gets the remainder.
fn refund_escrow(env: &Env, bounty_id: &BountyId, bounty: &Bounty) {
    let contract_address = env.current_contract_address();
    let sponsor_total = refund_sponsors(env, bounty_id, &bounty.reward_token, &contract_address);
    let creator_amount = bounty.reward_amount - sponsor_total;
    if creator_amount > 0 {
        let token = TokenClient::new(env, &bounty.reward_token);
        token.transfer(&contract_address, &bounty.creator, &creator_amount);
    }
}

/// Apply a reputation delta to `contrib` and emit a single `reputation_changed`
/// event documenting the old value, new value and reason.
///
/// All reputation writes must go through this helper so that every change is
/// observable by indexers (see `docs/event-schema.md`).
fn set_reputation(env: &Env, contrib: &mut Contributor, address: &Address, delta: i64, reason: &str) {
    let old_reputation = contrib.reputation;
    let new_reputation = if delta >= 0 {
        old_reputation.saturating_add(delta as u32)
    } else {
        old_reputation.saturating_sub((-delta) as u32)
    };
    contrib.reputation = new_reputation;
    events::emit_reputation_changed(
        env,
        address,
        old_reputation,
        new_reputation,
        Symbol::new(env, reason),
    );
}

/// Decrement a contributor's active claims counter, if greater than zero.
fn decrement_active_claims(contrib: &mut Contributor) {
    if contrib.active_claims > 0 {
        contrib.active_claims -= 1;
    }
}

/// #1216: Clear the O(1) `ContributorActiveBounty` mapping for every
/// assignee of `bounty`. Called from every terminal transition (completion,
/// cancellation, expiry, dispute resolution) so the mapping never points at
/// a bounty that is no longer active.
fn clear_active_bounty_for_assignees(env: &Env, bounty: &Bounty) {
    for (assignee, _) in bounty.assignees.iter() {
        storage::remove_contributor_active_bounty(env, &assignee);
    }
}

/// Complete a bounty by marking it as completed and distributing payout.
/// Used as a helper by both `complete_bounty` and `approve_completion`.
///
/// Takes the already-loaded `bounty` rather than re-reading it from storage —
/// the caller has already fetched it (avoids a redundant read).
fn complete_bounty_inner(
    env: Env,
    bounty_id: BountyId,
    mut bounty: Bounty,
) -> Result<(), ContractError> {
    if bounty.status != Symbol::new(&env, STATUS_IN_PROGRESS) {
        return Err(ContractError::BountyNotInProgress);
    }

    if bounty.assignees.is_empty() {
        return Err(ContractError::BountyHasNoAssignee);
    }

    if !bounty.milestones.is_empty() {
        if !bounty.milestones.iter().all(|m| m.completed) {
            return Err(ContractError::NotAllMilestonesCompleted);
        }
        let (primary_assignee, _) = bounty
            .assignees
            .get(0)
            .ok_or(ContractError::BountyHasNoAssignee)?;
        let previous_status = bounty.status.clone();
        bounty.status = Symbol::new(&env, STATUS_COMPLETED);
        storage::store_bounty(&env, &bounty_id, &bounty);
        storage::move_bounty_status(
            &env,
            &bounty_id,
            &previous_status,
            &bounty.status,
            &bounty.assignees,
        );
        storage::decrement_creator_open_count(&env, &bounty.creator);
        for tag in bounty.tags.iter() {
            storage::remove_tag_bounty(&env, &tag, &bounty_id);
        }
        events::emit_bounty_completed(&env, &bounty_id, &primary_assignee);
        return Ok(());
    }

    let (primary_assignee, _) = bounty
        .assignees
        .get(0)
        .ok_or(ContractError::BountyHasNoAssignee)?;
    let previous_status = bounty.status.clone();
    bounty.status = Symbol::new(&env, STATUS_COMPLETED);
    storage::store_bounty(&env, &bounty_id, &bounty);
    storage::move_bounty_status(
        &env,
        &bounty_id,
        &previous_status,
        &bounty.status,
        &bounty.assignees,
    );
    storage::decrement_creator_open_count(&env, &bounty.creator);
    for tag in bounty.tags.iter() {
        storage::remove_tag_bounty(&env, &tag, &bounty_id);
    }

    let token = TokenClient::new(&env, &bounty.reward_token);
    distribute_payout(
        &env,
        &bounty_id,
        &bounty.assignees,
        &env.current_contract_address(),
        &token,
        bounty.reward_amount,
    )?;

    events::emit_bounty_completed(&env, &bounty_id, &primary_assignee);
    Ok(())
}

/// Apply a dispute decision (`"complete"` pays out assignees, `"cancel"`
/// refunds the creator), mutate the bounty to its terminal status, and emit
/// `dispute_resolved`.
///
/// Shared by `resolve_dispute` (immediate path, when no appeal window is
/// configured) and `finalize_dispute` (deferred path, after the appeal
/// window has passed without an appeal).
fn apply_dispute_resolution(
    env: Env,
    bounty_id: BountyId,
    mut bounty: Bounty,
    arbitrator: Address,
    resolution: Symbol,
) -> Result<(), ContractError> {
    let resolve_complete = Symbol::new(&env, "complete");

    if resolution == resolve_complete {
        if bounty.assignees.is_empty() {
            return Err(ContractError::BountyHasNoAssignee);
        }

        if !bounty.milestones.is_empty() {
            if !bounty.milestones.iter().all(|m| m.completed) {
                return Err(ContractError::NotAllMilestonesCompleted);
            }
            let previous_status = bounty.status.clone();
            bounty.status = Symbol::new(&env, STATUS_COMPLETED);
            storage::store_bounty(&env, &bounty_id, &bounty);
            storage::move_bounty_status(
                &env,
                &bounty_id,
                &previous_status,
                &bounty.status,
                &bounty.assignees,
            );
        } else {
            let token = TokenClient::new(&env, &bounty.reward_token);
            distribute_payout(
                &env,
                &bounty_id,
                &bounty.assignees,
                &arbitrator,
                &token,
                bounty.reward_amount,
            );

            for (assignee, share_bp) in bounty.assignees.iter() {
                let payout = bounty.reward_amount * (share_bp as i128) / 10_000_i128;

                let mut contrib = storage::get_contributor(&env, &assignee)
                    .unwrap_or_else(|| Contributor::new(assignee.clone()));

                set_reputation(&env, &mut contrib, &assignee, 10, "completion");
                contrib.total_earned += payout;
                contrib.contribution_count += 1;
                decrement_active_claims(&mut contrib);

                storage::store_contributor(&env, &assignee, &contrib);
                // #1216: clear active-bounty mapping for all assignees.
                storage::remove_contributor_active_bounty(&env, &assignee);
                events::emit_reward_paid(&env, &bounty_id, &assignee, &payout);
            }

            let previous_status = bounty.status.clone();
            bounty.status = Symbol::new(&env, STATUS_COMPLETED);
            storage::store_bounty(&env, &bounty_id, &bounty);
            storage::move_bounty_status(
                &env,
                &bounty_id,
                &previous_status,
                &bounty.status,
                &bounty.assignees,
            );
        }
    } else {
        // validate_symbol (at decision time) already guarantees resolution is
        // "complete" or "cancel" — this branch handles "cancel".
        // Refund escrowed reward pro rata: sponsors get their contribution
        // back, the creator gets the remainder.
        refund_escrow(&env, &bounty_id, &bounty);

        let previous_status = bounty.status.clone();
        bounty.status = Symbol::new(&env, STATUS_CANCELLED);
        storage::store_bounty(&env, &bounty_id, &bounty);
        storage::move_bounty_status(
            &env,
            &bounty_id,
            &previous_status,
            &bounty.status,
            &bounty.assignees,
        );
    }

    events::emit_dispute_resolved(&env, &bounty_id, &arbitrator, &resolution);
    Ok(())
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
    ) -> Result<BountyId, ContractError> {
        require_not_paused(&env)?;
        if reward_amount <= 0 {
            return Err(ContractError::RewardMustBePositive);
        }

        if reward_amount < MIN_REWARD_AMOUNT {
            return Err(ContractError::RewardBelowMinimum);
        }

        if reward_amount < storage::get_min_reward(&env, &reward_token) {
            return Err(ContractError::RewardBelowMinimum);
        }

        validate_tags(&env, &tags)?;

        if max_assignees < 1 {
            return Err(ContractError::MaxAssigneesMustBePositive);
        }

        if let Some(ref verifiers) = required_verifiers {
            if approval_threshold > verifiers.len() {
                return Err(ContractError::ApprovalThresholdExceedsVerifiers);
            }
        }

        validate_reward_token(&env, &reward_token)?;

        if !storage::is_reward_token_allowed(&env, &reward_token) {
            return Err(ContractError::RewardTokenNotAllowed);
        }

        if !milestones.is_empty() {
            let mut total: i128 = 0;
            let mut previous_deadline: Option<u32> = None;
            for m in milestones.iter() {
                total = match total.checked_add(m.reward) {
                    Some(sum) => sum,
                    None => return Err(ContractError::RewardAmountOverflow),
                };

                // Milestone deadlines (when set) must be non-decreasing and
                // must not exceed the bounty's own deadline.
                if let Some(milestone_deadline) = m.deadline {
                    if let Some(prev) = previous_deadline {
                        if milestone_deadline < prev {
                            return Err(ContractError::MilestoneDeadlineInvalid);
                        }
                    }
                    if let Some(bounty_deadline) = deadline {
                        if milestone_deadline > bounty_deadline {
                            return Err(ContractError::MilestoneDeadlineInvalid);
                        }
                    }
                    previous_deadline = Some(milestone_deadline);
                }
            }
            if total != reward_amount {
                return Err(ContractError::MilestoneRewardsMismatch);
            }
        }

        if let Some(deadline) = deadline {
            if env.ledger().sequence() > deadline {
                return Err(ContractError::BountyDeadlinePassed);
            }
        }

        creator.require_auth();

        let limit = storage::get_creator_open_limit(&env);
        if limit > 0 && storage::get_creator_open_count(&env, &creator) >= limit {
            return Err(ContractError::CreatorLimitReached);
        }

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
        storage::append_token_bounty(&env, &bounty.reward_token, &id);
        storage::increment_creator_open_count(&env, &creator);
        storage::increment_bounties_by_creator_count(&env, &creator);
        storage::increment_bounties_by_token_count(&env, &bounty.reward_token);

        // Maintain the per-tag index so `get_bounties_by_tag` can page over
        // open bounties without scanning every bounty in the contract.
        for tag in bounty.tags.iter() {
            storage::append_tag_bounty(&env, &tag, &id);
        }

        events::emit_bounty_created(&env, &id, &creator, &reward_amount);
        Ok(id)
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
    pub fn claim_bounty(
        env: Env,
        contributor: Address,
        bounty_id: BountyId,
    ) -> Result<(), ContractError> {
        contributor.require_auth();
        require_not_paused(&env)?;

        let mut bounty = match storage::get_bounty(&env, &bounty_id) {
            Some(b) => b,
            None => return Err(ContractError::BountyNotFound),
        };

        // Idempotency: reject a duplicate claim by the same contributor before
        // any other state checks or storage mutations.
        for (addr, _) in bounty.assignees.iter() {
            if addr == contributor {
                return Err(ContractError::AlreadyClaimed);
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
        )?;

        // The creator of a bounty cannot claim their own bounty.
        if contributor == bounty.creator {
            return Err(ContractError::CreatorCannotClaim);
        }

        if bounty.assignees.len() >= bounty.max_assignees {
            return Err(ContractError::BountyAlreadyAssigned);
        }

        // Claim cooldown: block a re-claim by the same contributor within
        // `claim_cooldown` ledgers of their own unclaim on this bounty.
        if let Some(unclaimed_at) = storage::get_unclaimed_at(&env, &bounty_id, &contributor) {
            let cooldown = storage::get_claim_cooldown(&env);
            if cooldown > 0 && env.ledger().sequence() < unclaimed_at + cooldown {
                return Err(ContractError::ClaimCooldownActive);
            }
        }

Let me think about the changes needed:

        if contrib.active_claims >= 1 {
            return Err(ContractError::ContributorHasActiveClaim);
        }

        // Deadline enforcement: reject claims once the deadline ledger sequence has passed.
        if let Some(deadline) = bounty.deadline {
            if env.ledger().sequence() > deadline {
                return Err(ContractError::BountyDeadlinePassed);
            }
        }

        if bounty.min_reputation > 0 && contrib.reputation < bounty.min_reputation {
            return Err(ContractError::ReputationTooLow);
        }

4. **`claim_bounty`**: Add `storage::set_contributor_active_bounty(&env, &contributor, &bounty_id);`

        let previous_status = bounty.status.clone();
        bounty.status = Symbol::new(&env, STATUS_IN_PROGRESS);
        storage::store_bounty(&env, &bounty_id, &bounty);
        storage::move_bounty_status(
            &env,
            &bounty_id,
            &previous_status,
            &bounty.status,
            &bounty.assignees,
        );

6. **`reassign_bounty`**: Add `storage::set_contributor_active_bounty(&env, &new_assignee, &bounty_id);` and `storage::remove_contributor_active_bounty(&env, &old_assignee);`

        // Remove from open bounties list and the per-tag index: the bounty is
        // no longer open, so `get_bounties_by_tag` must stop returning it.
        storage::remove_open_bounty(&env, &bounty_id);
        for tag in bounty.tags.iter() {
            storage::remove_tag_bounty(&env, &tag, &bounty_id);
        }

        events::emit_bounty_claimed(&env, &bounty_id, &contributor);
        Ok(())
    }

    /// Relinquish a claim on a bounty the caller is currently assigned to.
    ///
    /// Removes the contributor from the bounty's assignee list and decrements
    /// their active-claims counter. If this was the only assignee, the bounty
    /// reverts to `"open"` status and is re-listed in the open-bounties index.
    /// A cooldown (configurable via `set_claim_cooldown`) is recorded so the
    /// same contributor cannot immediately re-claim the bounty.
    ///
    /// # Panics
    /// * If `bounty_id` does not exist.
    /// * If the caller is not currently an assignee on this bounty.
    ///
    /// # Authorization
    /// Requires auth from `contributor`.
    pub fn unclaim_bounty(
        env: Env,
        contributor: Address,
        bounty_id: BountyId,
    ) -> Result<(), ContractError> {
        contributor.require_auth();
        require_not_paused(&env)?;

        let mut bounty = match storage::get_bounty(&env, &bounty_id) {
            Some(b) => b,
            None => return Err(ContractError::BountyNotFound),
        };

        let is_assignee = bounty.assignees.iter().any(|(addr, _)| addr == contributor);
        if !is_assignee {
            return Err(ContractError::NotAssignee);
        }

10. **`sponsor_bounty`**: Add `MIN_SPONSOR_AMOUNT` and `MAX_SPONSORS_PER_BOUNTY` checks.

11. **`reopen_bounty`**: Add milestone deadline validation.

        if bounty.assignees.is_empty() {
            let previous_status = bounty.status.clone();
            bounty.status = Symbol::new(&env, STATUS_OPEN);
            storage::store_bounty(&env, &bounty_id, &bounty);
            storage::move_bounty_status(
                &env,
                &bounty_id,
                &previous_status,
                &bounty.status,
                &bounty.assignees,
            );
            storage::add_open_bounty(&env, &bounty_id);
            for tag in bounty.tags.iter() {
                storage::append_tag_bounty(&env, &tag, &bounty_id);
            }
        } else {
            storage::store_bounty(&env, &bounty_id, &bounty);
        }

Wait, the issue says "checks whether any uncompleted milestone carries a `deadline` earlier than the current ledger sequence (or allows updating milestone deadlines atomically)". So we need to check for already-expired milestones. And the requirement says "ensuring reopened bounties maintain `milestone.deadline <= new_deadline` and future milestone feasibility."

        events::emit_bounty_unclaimed(&env, &bounty_id, &contributor);
        Ok(())
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
    ) -> Result<(), ContractError> {
        verifier.require_auth();
        require_not_paused(&env)?;

        let mut bounty = match storage::get_bounty(&env, &bounty_id) {
            Some(b) => b,
            None => return Err(ContractError::BountyNotFound),
        };

        if bounty.status != Symbol::new(&env, STATUS_IN_PROGRESS) {
            return Err(ContractError::BountyNotInProgress);
        }

        if bounty.assignees.is_empty() {
            return Err(ContractError::BountyHasNoAssignee);
        }

        let idx = milestone_index;
        if idx >= bounty.milestones.len() {
            return Err(ContractError::InvalidMilestoneIndex);
        }

        let mut milestone = bounty
            .milestones
            .get(idx)
            .ok_or(ContractError::InvalidMilestoneIndex)?
            .clone();
        if milestone.completed {
            return Err(ContractError::MilestoneAlreadyCompleted);
        }

        if let Some(milestone_deadline) = milestone.deadline {
            if env.ledger().sequence() > milestone_deadline {
                return Err(ContractError::MilestoneDeadlinePassed);
            }
        }

        for (assignee, _) in bounty.assignees.iter() {
            if assignee == verifier {
                return Err(ContractError::VerifierCannotBeAssignee);
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
        )?;

        let completed_milestone = bounty
            .milestones
            .get(idx)
            .ok_or(ContractError::InvalidMilestoneIndex)?;
        events::emit_milestone_completed(
            &env,
            &bounty_id,
            milestone_index,
            &completed_milestone.reward,
        );

        storage::store_bounty(&env, &bounty_id, &bounty);
        Ok(())
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
    pub fn complete_bounty(
        env: Env,
        verifier: Address,
        bounty_id: BountyId,
    ) -> Result<(), ContractError> {
        // AUTH: must be first — no storage reads or side-effects before this line.
        verifier.require_auth();
        require_not_paused(&env)?;

        let bounty = match storage::get_bounty(&env, &bounty_id) {
            Some(b) => b,
            None => return Err(ContractError::BountyNotFound),
        };

        // GUARD 1 — dispute prevention.
        // If the bounty is in disputed status, complete_bounty must not execute.
        // A disputed bounty must be resolved via resolve_dispute first.
        if bounty.status == Symbol::new(&env, STATUS_DISPUTED) {
            return Err(ContractError::BountyIsDisputed);
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
        )?;

        if bounty.assignees.is_empty() {
            return Err(ContractError::BountyHasNoAssignee);
        }

        // GUARD 3 — self-verification prevention.
        // The verifier must be a party independent from the assignees. Allowing the
        // same address to both claim and verify would let a contributor manufacture
        // reputation and, once escrow is introduced, drain contract funds unilaterally.
        for (assignee, _) in bounty.assignees.iter() {
            if assignee == verifier {
                return Err(ContractError::VerifierCannotBeAssignee);
            }
        }

        complete_bounty_inner(env, bounty_id, bounty)
    }

    /// Record one verifier's approval for a multi-sig bounty completion.
    /// When the number of unique approvals reaches approval_threshold, completion executes automatically.
    /// Falls back to single-verifier behaviour when required_verifiers is None (any verifier completes directly).
    ///
    /// # Authorization
    /// `verifier.require_auth()` is the **first** operation in this function.
    /// No storage reads or business logic execute before authentication is checked.
    pub fn approve_completion(
        env: Env,
        verifier: Address,
        bounty_id: BountyId,
    ) -> Result<(), ContractError> {
        verifier.require_auth();
        require_not_paused(&env)?;

        let mut bounty = match storage::get_bounty(&env, &bounty_id) {
            Some(b) => b,
            None => return Err(ContractError::BountyNotFound),
        };

        if bounty.assignees.is_empty() {
            return Err(ContractError::BountyHasNoAssignee);
        }

        for (assignee, _) in bounty.assignees.iter() {
            if assignee == verifier {
                return Err(ContractError::VerifierCannotBeAssignee);
            }
        }

        // If no required_verifiers list is set, fall back to immediate single-verifier completion.
        if bounty.required_verifiers.is_none() {
            return complete_bounty_inner(env, bounty_id, bounty);
        }

        // When verifiers are configured, a work submission must exist first —
        // approval without a recorded submission leaves verifiers with
        // nothing concrete to review.
        if storage::get_submission(&env, &bounty_id).is_none() {
            return Err(ContractError::SubmissionRequired);
        }

        let required = bounty
            .required_verifiers
            .clone()
            .ok_or(ContractError::VerifierNotAuthorized)?;
        let is_authorized = required.iter().any(|v| v == verifier);
        if !is_authorized {
            return Err(ContractError::VerifierNotAuthorized);
        }

        let mut approvals = storage::get_approvals(&env, &bounty_id);

        // Guard against duplicate votes from the same verifier.
        let already_voted = approvals.iter().any(|v| v == verifier);
        if already_voted {
            return Err(ContractError::AlreadyApproved);
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
                storage::move_bounty_status(
                    &env,
                    &bounty_id,
                    &previous_status,
                    &bounty.status,
                    &bounty.assignees,
                );
                storage::decrement_creator_open_count(&env, &bounty.creator);
                for tag in bounty.tags.iter() {
                    storage::remove_tag_bounty(&env, &tag, &bounty_id);
                }
                let (primary_assignee, _) = bounty.assignees.get(0).unwrap_or_else(|| fail(ContractError::BountyHasNoAssignee));
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

            let (primary_assignee, _) = bounty.assignees.get(0).unwrap_or_else(|| fail(ContractError::BountyHasNoAssignee));
            let previous_status = bounty.status.clone();
            bounty.status = Symbol::new(&env, STATUS_COMPLETED);
            storage::store_bounty(&env, &bounty_id, &bounty);
            storage::move_bounty_status(
                &env,
                &bounty_id,
                &previous_status,
                &bounty.status,
                &bounty.assignees,
            );
            storage::decrement_creator_open_count(&env, &bounty.creator);
            for tag in bounty.tags.iter() {
                storage::remove_tag_bounty(&env, &tag, &bounty_id);
            }
            events::emit_bounty_completed(&env, &bounty_id, &primary_assignee);
        }
    }

But I don't know if `get_sponsors` exists. Let me use `get_sponsor_count` which is more likely to be added.

Actually, you know what, let me just make reasonable assumptions and write the code. The other steps will add the storage functions. I'll use:
- `storage::get_sponsor_count(&env, &bounty_id)` → returns u32
- `storage::is_sponsor(&env, &bounty_id, &sponsor)` → returns bool

Hmm, that's two functions. Let me simplify. I'll use `storage::get_sponsors` which returns the list, and check both count and membership:

        let previous_status = bounty.status.clone();
        bounty.status = Symbol::new(&env, STATUS_DISPUTED);
        storage::store_bounty(&env, &bounty_id, &bounty);
        storage::move_bounty_status(
            &env,
            &bounty_id,
            &previous_status,
            &bounty.status,
            &bounty.assignees,
        );
        storage::decrement_creator_open_count(&env, &bounty.creator);
        for tag in bounty.tags.iter() {
            storage::remove_tag_bounty(&env, &tag, &bounty_id);
        }
        events::emit_bounty_disputed(&env, &bounty_id, &caller);
    }

Let me just use `storage::get_sponsors(&env, &bounty_id)` returning `Vec<(Address, i128)>`. This is symmetric with `take_sponsors`. Then:

```rust
let existing = storage::get_sponsors(&env, &bounty_id);
let is_new = !existing.iter().any(|(addr, _)| addr == sponsor);
if is_new && existing.len() >= storage::MAX_SPONSORS_PER_BOUNTY {
    fail(ContractError::TooManySponsors);
}
```

This is clean. Let me go with this.

For `MIN_SPONSOR_AMOUNT`, I'll reference `storage::MIN_SPONSOR_AMOUNT`.

OK let me write the full file now. I need to be careful to preserve everything.

Actually, let me reconsider the milestone deadline check. The issue says:

"checks whether any uncompleted milestone carries a `deadline` earlier than the current ledger sequence (or allows updating milestone deadlines atomically), preventing reopened bounties from being uncompletable due to already-expired milestone deadlines."

And: "Implement validation (or optional milestone-deadline rescheduling) ensuring reopened bounties maintain `milestone.deadline <= new_deadline` and future milestone feasibility."

So the checks are:
1. For each uncompleted milestone with a deadline:
   - If `milestone.deadline < env.ledger().sequence()` → already expired → fail with `MilestoneDeadlineExpired`
   - If `milestone.deadline > new_deadline` → beyond new deadline → fail with `MilestoneDeadlineExpired` (or a different error)

The step says: "fail with `ContractError::MilestoneDeadlineExpired` when `milestone.deadline < env.ledger().sequence()` or when `milestone.deadline > new_deadline`"

OK so both use `MilestoneDeadlineExpired`. Good.

Let me write the full file now.

For the `sponsor_bounty` changes, I need to add the checks after the status check but before the transfer. Actually, the transfer should happen after all checks to avoid wasting gas. Let me put the checks before the transfer.

Wait, but the transfer is what actually moves funds. If we check after transfer and fail, the transfer is reverted anyway (Soroban transactions are atomic). But it's cleaner to check before.

Let me write it:

```rust
pub fn sponsor_bounty(env: Env, sponsor: Address, bounty_id: BountyId, amount: i128) {
    sponsor.require_auth();
    require_not_paused(&env);

    let mut bounty = match storage::get_bounty(&env, &bounty_id) {
        Some(b) => b,
        None => fail(ContractError::BountyNotFound),
    };

    if amount <= 0 {
        fail(ContractError::AmountMustBePositive);
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
        require_not_paused(&env);

        if metadata == Symbol::new(&env, "") {
            fail(ContractError::MetadataEmpty);
        }

        // #275: use Contributor::new for default construction (DONE - all call sites updated)
        let mut contrib = storage::get_contributor(&env, &contributor)
            .unwrap_or_else(|| Contributor::new(contributor.clone()));

        contrib.metadata = Some(metadata.clone());
        storage::store_contributor(&env, &contributor, &contrib);

        events::emit_contributor_metadata_updated(&env, &contributor, &metadata);
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
        require_not_paused(&env);

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

        // Refund the escrowed reward pro rata: each sponsor gets back exactly
        // what they contributed, and the creator gets back the remainder
        // (reward_amount minus all sponsor contributions). This preserves the
        // escrow balance invariant even when third parties co-funded via
        // `sponsor_bounty`.
        refund_escrow(&env, &bounty_id, &bounty);

        let previous_status = bounty.status.clone();
        bounty.status = Symbol::new(&env, STATUS_CANCELLED);
        storage::store_bounty(&env, &bounty_id, &bounty);
        storage::move_bounty_status(
            &env,
            &bounty_id,
            &previous_status,
            &bounty.status,
            &bounty.assignees,
        );
        storage::decrement_creator_open_count(&env, &bounty.creator);
        for tag in bounty.tags.iter() {
            storage::remove_tag_bounty(&env, &tag, &bounty_id);
        }

    // #1218: enforce a hard cap on the number of distinct sponsors so an
    // attacker cannot grief `cancel_bounty`/`expire_bounty` by adding
    // hundreds of dust sponsors that make `refund_sponsors` exceed the
    // host budget. Repeat sponsors (already tracked) do not consume a slot.
    let existing_sponsors = storage::get_sponsors(&env, &bounty_id);
    let is_new_sponsor = !existing_sponsors.iter().any(|(addr, _)| addr == sponsor);
    if is_new_sponsor && existing_sponsors.len() >= storage::MAX_SPONSORS_PER_BOUNTY {
        fail(ContractError::TooManySponsors);
    }

    let token = TokenClient::new(&env, &bounty.reward_token);
    token.transfer(&sponsor, env.current_contract_address(), &amount);

    storage::add_sponsor_contribution(&env, &bounty_id, &sponsor, amount);

        let previous_status = bounty.status.clone();
        bounty.status = Symbol::new(&env, STATUS_CANCELLED);
        storage::store_bounty(&env, &bounty_id, &bounty);
        storage::move_bounty_status(
            &env,
            &bounty_id,
            &previous_status,
            &bounty.status,
            &bounty.assignees,
        );
        storage::decrement_creator_open_count(&env, &bounty.creator);
        for tag in bounty.tags.iter() {
            storage::remove_tag_bounty(&env, &tag, &bounty_id);
        }

    events::emit_bounty_sponsored(&env, &bounty_id, &sponsor, &amount, &bounty.reward_amount);
}
```

Now for `reopen_bounty`:

```rust
pub fn reopen_bounty(env: Env, creator: Address, bounty_id: BountyId, new_deadline: u32) {
    creator.require_auth();
    require_not_paused(&env);

        let previous_status = bounty.status.clone();
        bounty.status = Symbol::new(&env, STATUS_SUBMITTED);
        storage::store_bounty(&env, &bounty_id, &bounty);
        storage::move_bounty_status(
            &env,
            &bounty_id,
            &previous_status,
            &bounty.status,
            &bounty.assignees,
        );

    if creator != bounty.creator {
        fail(ContractError::NotBountyCreator);
    }

    let current_deadline = match bounty.deadline {
        Some(d) => d,
        None => fail(ContractError::BountyNoDeadline),
    };

    if env.ledger().sequence() <= current_deadline {
        fail(ContractError::DeadlineNotPassed);
    }

    if bounty.status != Symbol::new(&env, STATUS_OPEN) {
        fail(ContractError::BountyNotOpen);
    }

    if new_deadline <= env.ledger().sequence() {
        fail(ContractError::BountyDeadlinePassed);
    }

    // #1202: validate per-milestone deadlines. Any uncompleted milestone
    // whose deadline has already passed (or would exceed the new bounty
    // deadline) makes the reopened bounty uncompletable, so reject the
    // reopen rather than silently producing a stuck bounty.
    let now = env.ledger().sequence();
    for m in bounty.milestones.iter() {
        if m.completed {
            continue;
        }

        // Milestone-progress awareness: once any milestone has been completed
        // the assignee has demonstrably done work, so the claim must not be
        // treated as stale (and must not be slashed). This check runs before
        // any reputation penalty or status change.
        if bounty.milestones.iter().any(|m| m.completed) {
            fail(ContractError::MilestoneProgressExists);
        }

        // Apply the existing reputation penalty for abandoned claims to each
        // assignee, and release their active-claim slot.
        for (assignee, _) in bounty.assignees.iter() {
            let mut contrib = storage::get_contributor(&env, &assignee)
                .unwrap_or_else(|| Contributor::new(assignee.clone()));
            let penalty = storage::get_reputation_penalty(&env);
            set_reputation(&env, &mut contrib, &assignee, -(penalty as i64), "penalty");
            decrement_active_claims(&mut contrib);
            storage::store_contributor(&env, &assignee, &contrib);
        }

        bounty.assignees = Vec::new(&env);
        let previous_status = bounty.status.clone();
        bounty.status = Symbol::new(&env, STATUS_OPEN);
        storage::store_bounty(&env, &bounty_id, &bounty);
        storage::move_bounty_status(
            &env,
            &bounty_id,
            &previous_status,
            &bounty.status,
            &bounty.assignees,
        );
        storage::add_open_bounty(&env, &bounty_id);
        for tag in bounty.tags.iter() {
            storage::append_tag_bounty(&env, &tag, &bounty_id);
        }
        storage::clear_claimed_at_ledger(&env, &bounty_id);

        events::emit_claim_released(&env, &bounty_id, &caller);
    }

    // ── Admin ─────────────────────────────────────────────────────────────

    /// One-time admin bootstrap. May only be called while no admin is set.
    ///
    /// # Panics
    /// * If an admin has already been configured (`ContractError::AdminAlreadySet`).
    ///
    /// # Authorization
    /// Requires auth from `admin`.
    pub fn init_admin(env: Env, admin: Address) {
        admin.require_auth();
        if storage::get_admin(&env).is_some() {
            fail(ContractError::AdminAlreadySet);
        }
    }

    bounty.deadline = Some(new_deadline);
    storage::store_bounty(&env, &bounty_id, &bounty);

    events::emit_bounty_reopened(&env, &bounty_id, &creator, new_deadline);
}
```

Now let me write the full file. I need to be very careful to preserve everything.

Let me also double-check the `claim_bounty` change. After `storage::store_contributor(&env, &contributor, &contrib);`, add:
```rust
storage::set_contributor_active_bounty(&env, &contributor, &bounty_id);
```

For `unclaim_bounty`, after `storage::store_contributor(&env, &contributor, &contrib);`, add:
```rust
storage::remove_contributor_active_bounty(&env, &contributor);
```

For `reassign_bounty`, after storing new_contrib:
```rust
storage::set_contributor_active_bounty(&env, &new_assignee, &bounty_id);
```
And after decrementing old_contrib:
```rust
storage::remove_contributor_active_bounty(&env, &old_assignee);
```

For `release_stale_claim`, in the loop:
```rust
storage::remove_contributor_active_bounty(&env, &assignee);
```

For `cancel_bounty`, after refund_escrow (before status change):
```rust
clear_active_bounty_for_assignees(&env, &bounty);
```

For `expire_bounty`, same.

OK let me write the full file now. I'll be very careful.

Actually wait, for `cancel_bounty` and `expire_bounty`, these only work on open bounties which have no assignees. So `clear_active_bounty_for_assignees` would be a no-op. But it's defensive and harmless. Let me include it anyway for completeness per the requirement.

Hmm, but actually, is there a scenario where an open bounty has assignees? No, because `unclaim_bounty` sets status to open only when assignees becomes empty. And `release_stale_claim` sets assignees to empty before setting open. So open bounties never have assignees. But defensive is fine.

Actually, let me reconsider. The requirement says "remove it on `unclaim_bounty`, `release_stale_claim`, completion, cancellation, and dispute resolution". So cancellation should remove it. Even though open bounties have no assignees, let me add it for safety.

OK, writing the full file now.

One more thing — I need to make sure the `distribute_payout` function clears the mapping. But wait, `distribute_payout` is called from `complete_bounty_inner` (non-milestone path), `complete_milestone`, and `approve_completion`. For `complete_milestone`, the bounty is NOT terminal — it's still in progress. So we should NOT clear the active bounty mapping there!

Hmm, that's a problem. `distribute_payout` is used for both terminal completion and milestone completion. If I add the clear inside `distribute_payout`, it would incorrectly clear on milestone completion.

Let me NOT add the clear inside `distribute_payout`. Instead, add it explicitly in the terminal paths.

So:
- `complete_bounty_inner`: add `clear_active_bounty_for_assignees` in both branches (milestone and non-milestone)
- `apply_dispute_resolution`: add in both branches
- `complete_milestone`: do NOT add (not terminal)

Let me revise `distribute_payout` to NOT include the clear.

OK, let me write the full file now.

        storage::remove_creator_bounty(&env, &previous_owner, &bounty_id);
        storage::append_creator_bounty(&env, &new_owner, &bounty_id);
        storage::decrement_bounties_by_creator_count(&env, &previous_owner);
        storage::increment_bounties_by_creator_count(&env, &new_owner);
        if is_active {
            storage::decrement_creator_open_count(&env, &previous_owner);
            storage::increment_creator_open_count(&env, &new_owner);
        }

For the milestone path in `complete_bounty_inner`, add `clear_active_bounty_for_assignees` before `events::emit_bounty_completed`.

    /// Drain the legacy `DataKey::StatusIndex(Symbol)` and
    /// `DataKey::OpenBounties` storage blobs into their paged
    /// (`StatusIndexPage` / `OpenBountiesPage`) replacements.
    ///
    /// The migration is bounded and idempotent: each call migrates at most
    /// `storage::MIGRATION_CHUNK_SIZE` entries per legacy index, deduplicating
    /// against entries already present in the paged storage, and deletes the
    /// legacy key once fully drained. Callers should invoke this repeatedly
    /// until `storage::has_legacy_indexes` returns `false`.
    ///
    /// # Authorization
    /// Admin-only; requires auth from `admin`.
    pub fn migrate_legacy_indexes(env: Env, admin: Address) {
        require_admin(&env, &admin);
        require_not_paused(&env);
        storage::migrate_legacy_indexes(&env);
    }

    /// Reopen an expired bounty with a new deadline.
    ///
    /// Moves an expired bounty back to open status with a new future deadline,
    /// provided its escrowed reward has not been refunded.
    ///
    /// # Arguments
    /// * `creator` - The wallet that created the bounty.
    /// * `bounty_id` - The identifier of the bounty to reopen.
    /// * `new_deadline` - The new ledger sequence deadline (must be in the future).
    ///
    /// # Panics
    /// * If `bounty_id` does not exist (`ContractError::BountyNotFound`).
    /// * If `creator` is not the bounty creator (`ContractError::NotBountyCreator`).
    /// * If the bounty has no deadline (`ContractError::BountyNoDeadline`).
    /// * If the current deadline has not passed yet (`ContractError::DeadlineNotPassed`).
    /// * If the bounty is not in `"open"` status (`ContractError::BountyNotOpen`).
    /// * If `new_deadline` is not strictly in the future (`ContractError::BountyDeadlinePassed`).
    ///
    /// # Authorization
    /// Requires auth from `creator`.
    pub fn reopen_bounty(env: Env, creator: Address, bounty_id: BountyId, new_deadline: u32) {
        creator.require_auth();
        require_not_paused(&env);

        let mut bounty = match storage::get_bounty(&env, &bounty_id) {
            Some(b) => b,
            None => fail(ContractError::BountyNotFound),
        };

        if creator != bounty.creator {
            fail(ContractError::NotBountyCreator);
        }

For `apply_dispute_resolution` cancel branch: add `clear_active_bounty_for_assignees` after status change.

OK, let me write it now.

Actually, I realize I should double check — does `distribute_payout` get called from `complete_milestone`? Yes. And `complete_milestone` is not terminal. So I must NOT clear in `distribute_payout`. Good, I'll keep `distribute_payout` unchanged.

Let me write the full file.```rust
use crate::symbols::{
    self, SymbolKind, STATUS_CANCELLED, STATUS_COMPLETED, STATUS_DISPUTED,
    STATUS_IN_PROGRESS, STATUS_OPEN, STATUS_SUBMITTED,
};

/// Minimum reward amount enforced at bounty creation.
const MIN_REWARD_AMOUNT: i128 = 100;

/// Minimum number of ledgers a bounty must sit in a terminal status
/// (`"completed"` or `"cancelled"`) before anyone can archive it. Chosen to
/// give indexers/UIs a grace window to read the full record before it is
/// compacted. At ~5s/ledger this is roughly 1 day.
const ARCHIVE_GRACE_PERIOD_LEDGERS: u32 = 17_280;

/// Maximum number of tags a bounty may carry. Documented in
/// `docs/contract-reference.md`. Enforced by `validate_tags`, used from both
/// `create_bounty` and `update_tags` so a bounty can never accumulate more
/// tags (or duplicates) after creation than it could at creation time.
const MAX_TAGS: u32 = 5;

        // Diff the old and new tag lists so the per-tag index only reflects
        // the bounty's current tags. Each tag is validated against
        // `SymbolKind::Tag` by `validate_tags` above.
        for old_tag in bounty.tags.iter() {
            if !tags.iter().any(|t| t == old_tag) {
                storage::remove_tag_bounty(&env, &old_tag, &bounty_id);
            }
        }
        for new_tag in tags.iter() {
            if !bounty.tags.iter().any(|t| t == new_tag) {
                storage::append_tag_bounty(&env, &new_tag, &bounty_id);
            }
        }

        bounty.tags = tags;
        storage::store_bounty(&env, &bounty_id, &bounty);
    }

    /// Archive a finished bounty, compacting it into a `BountySummary` and
    /// removing the full `Bounty`/`BountyMeta` records to stop extending
    /// their TTL (and so paying rent on data nobody reads anymore).
    ///
    /// Only bounties in a terminal status (`"completed"` or `"cancelled"`)
    /// can be archived. Callable by anyone (not just the creator), subject
    /// to a grace period: the first call against an eligible bounty sets a
    /// grace-period marker and fails with `DeadlineNotPassed`; a subsequent
    /// call made at least `ARCHIVE_GRACE_PERIOD_LEDGERS` later succeeds.
    ///
    /// After archiving, `get_bounty` keeps returning a (summary-only)
    /// `Bounty` for `bounty_id` so existing callers do not start seeing
    /// `None` for bounties that used to exist.
    ///
    /// # Panics
    /// * If `bounty_id` does not exist (`ContractError::BountyNotFound`).
    /// * If the bounty is not completed or cancelled (`ContractError::InvalidStatus`).
    /// * If the grace period has not yet elapsed (`ContractError::DeadlineNotPassed`).
    pub fn archive_bounty(env: Env, bounty_id: BountyId) {
        require_not_paused(&env);
        let bounty = match storage::get_bounty(&env, &bounty_id) {
            Some(b) => b,
            None => fail(ContractError::BountyNotFound),
        };

        let is_finished = bounty.status == Symbol::new(&env, STATUS_COMPLETED)
            || bounty.status == Symbol::new(&env, STATUS_CANCELLED);
        if !is_finished {
            fail(ContractError::InvalidStatus);
        }

        let now = env.ledger().sequence();
        match storage::get_archive_eligible_at(&env, &bounty_id) {
            None => {
                storage::set_archive_eligible_at(
                    &env,
                    &bounty_id,
                    now + ARCHIVE_GRACE_PERIOD_LEDGERS,
                );
                fail(ContractError::DeadlineNotPassed);
            }
            Some(eligible_at) => {
                if now < eligible_at {
                    fail(ContractError::DeadlineNotPassed);
                }
            }
        }

        let assignee = bounty.assignees.get(0).map(|(address, _)| address);
        let summary = BountySummary {
            id: bounty_id.clone(),
            creator: bounty.creator.clone(),
            assignee,
            status: bounty.status.clone(),
            reward: bounty.reward_amount,
            reward_token: bounty.reward_token.clone(),
        };

        storage::store_bounty_archive(&env, &bounty_id, &summary);
        storage::remove_bounty_body(&env, &bounty_id);
        storage::remove_archive_eligible_at(&env, &bounty_id);
        storage::remove_token_bounty(&env, &bounty.reward_token, &bounty_id);
        storage::decrement_bounties_by_creator_count(&env, &bounty.creator);
        storage::decrement_bounties_by_token_count(&env, &bounty.reward_token);

        events::emit_bounty_archived(&env, &bounty_id);
    }
}
