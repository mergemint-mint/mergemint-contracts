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
    "cancel_admin_transfer",
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

/// Returns `true` when every byte of `hash` is zero (the all-zero sentinel).
fn is_zero_hash(hash: &BytesN<32>) -> bool {
    hash.to_array().iter().all(|b| *b == 0)
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

/// Transfer `amount` of `token` from `from` into the contract's own escrow
/// account (`env.current_contract_address()`).
///
/// Used by `create_bounty` (creator funds the reward up front) and
/// `sponsor_bounty` (third parties co-fund). Any failure from the SEP-41
/// token contract (insufficient balance, missing allowance, non-token
/// address) is mapped to `ContractError::TransferFailed` so callers get a
/// deterministic error instead of a host trap.
fn escrow_transfer(env: &Env, token: &Address, from: &Address, amount: i128) {
    let token_client = TokenClient::new(env, token);
    token_client.transfer(from, &env.current_contract_address(), &amount);
}

/// Shared payout loop used by `complete_bounty`, `approve_completion`,
/// `complete_milestone`, and `resolve_dispute`'s "complete" branch.
///
/// For each assignee, computes the proportional payout from `reward_amount`,
/// transfers the token out of the contract's escrow account, and emits the
/// `reward_paid` event. Returns the primary (first) assignee address.
///
/// Contributor accounting is intentionally *not* performed here: milestone
/// payouts must not touch `active_claims`, `contribution_count`, or award
/// completion reputation (see `complete_milestone`), while terminal
/// completion must finalize all of them exactly once (see
/// `complete_bounty_inner`). Callers apply the appropriate stat updates.
fn distribute_payout(
    env: &Env,
    bounty_id: &BountyId,
    assignees: &Vec<(Address, u32)>,
    token: &TokenClient,
    reward_amount: i128,
) -> Result<Address, ContractError> {
    let (primary_assignee, _) = assignees
        .get(0)
        .ok_or(ContractError::BountyHasNoAssignee)?;
    let escrow = env.current_contract_address();

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

/// Finalize contributor accounting for a terminal completion: award the
/// `+10` completion reputation, bump `contribution_count`, and release the
/// assignee's `active_claims` slot. Called exactly once per assignee by
/// `complete_bounty_inner` (and the equivalent branches in
/// `approve_completion` / `apply_dispute_resolution`), never by
/// `complete_milestone`.
fn finalize_completion_stats(env: &Env, assignees: &Vec<(Address, u32)>) {
    for (assignee, _) in assignees.iter() {
        let mut contrib = storage::get_contributor(env, &assignee)
            .unwrap_or_else(|| Contributor::new(assignee.clone()));
        set_reputation(env, &mut contrib, &assignee, 10, "completion");
        contrib.contribution_count += 1;
        decrement_active_claims(&mut contrib);
        storage::store_contributor(env, &assignee, &contrib);
    }
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

/// Compute the total reward already paid out for completed milestones.
///
/// When a bounty has milestones, individual milestone rewards are transferred
/// by `complete_milestone` as work progresses. Only the *uncompleted*
/// milestone rewards remain in escrow; refunding the full `reward_amount`
/// would over-pay.
fn completed_milestone_total(bounty: &Bounty) -> i128 {
    if bounty.milestones.is_empty() {
        return 0;
    }
    bounty
        .milestones
        .iter()
        .filter(|m| m.completed)
        .fold(0i128, |acc, m| acc.saturating_add(m.reward))
}

/// Refund the escrowed reward for a cancelled or expired bounty.
///
/// For milestone bounties, only the uncompleted milestone rewards remain in
/// escrow — completed milestone funds were already paid out by
/// `complete_milestone`. This function therefore only refunds
/// `reward_amount - completed_milestone_total`, preserving the invariant
/// that no funds are double-counted.
///
/// Each sponsor gets back exactly what they contributed (up to the
/// refundable amount); the creator receives the remainder.
fn refund_escrow(env: &Env, bounty_id: &BountyId, bounty: &Bounty) {
    let contract_address = env.current_contract_address();
    // Deduct already-paid milestone rewards so we only refund what is still escrowed.
    let refundable = bounty
        .reward_amount
        .saturating_sub(completed_milestone_total(bounty));
    if refundable <= 0 {
        // All funds were already distributed; nothing left to refund.
        // Still consume the sponsors list so it cannot be replayed.
        storage::take_sponsors(env, bounty_id);
        return;
    }
    let sponsor_total = refund_sponsors(env, bounty_id, &bounty.reward_token, &contract_address);
    // Clamp: sponsor_total must never exceed refundable (data invariant).
    let creator_amount = refundable.saturating_sub(sponsor_total);
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
    // #1180: `submit_work` moves a bounty to `submitted`; it must still be
    // completable from there, or it is wedged forever.
    ensure_status(
        &bounty,
        &[
            Symbol::new(&env, STATUS_IN_PROGRESS),
            Symbol::new(&env, STATUS_SUBMITTED),
        ],
        ContractError::BountyNotInProgress,
    )?;

    if bounty.assignees.is_empty() {
        return Err(ContractError::BountyHasNoAssignee);
    }

    if !bounty.milestones.is_empty() {
        if !bounty.milestones.iter().all(|m| m.completed) {
            return Err(ContractError::NotAllMilestonesCompleted);
        }
        // Milestone rewards were already paid out by `complete_milestone`;
        // finalize the terminal accounting exactly once here.
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
        finalize_completion_stats(&env, &bounty.assignees);
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
    finalize_completion_stats(&env, &bounty.assignees);

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
            finalize_completion_stats(&env, &bounty.assignees);
        } else {
            // Pay from the contract's escrow account, not the arbitrator's
            // wallet: `create_bounty` (and `sponsor_bounty`) already moved the
            // reward into escrow, so the arbitrator never funds the payout.
            let token = TokenClient::new(&env, &bounty.reward_token);
            // An independent arbitrator (#1195) is not a party to the bounty
            // and must not fund it: pay from the contract's escrow. The
            // creator acting as its own arbitrator keeps paying from its wallet.
            let payer = if arbitrator == bounty.creator {
                arbitrator.clone()
            } else {
                env.current_contract_address()
            };

            distribute_payout(
                &env,
                &bounty_id,
                &bounty.assignees,
                &payer,
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
            finalize_completion_stats(&env, &bounty.assignees);

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

    // #1197: a disputed bounty stays counted against `creator_open_limit`
    // until it reaches a terminal status here.
    storage::decrement_creator_open_count(&env, &bounty.creator);

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
    /// * If `required_verifiers` is `Some` but empty (`EmptyVerifierList`), longer
    ///   than `MAX_VERIFIERS` (`TooManyVerifiers`), contains duplicates
    ///   (`DuplicateVerifier`) or the creator (`CreatorCannotBeVerifier`), or
    ///   `approval_threshold` is `0` (`InvalidApprovalThreshold`).
    /// * If `reward_token` is not a valid Soroban token contract.
    /// * If milestone reward summation overflows (`ContractError::RewardAmountOverflow`).
    /// * If `milestones` is non-empty and their rewards do not sum to `reward_amount`.
    ///
    /// # Authorization
    /// Requires auth from `creator`. `creator.require_auth()` is the **first**
    /// operation in this function: no pause check, storage read, TTL
    /// extension, or cross-contract `validate_reward_token` probe runs before
    /// authentication succeeds, so unauthenticated callers cannot trigger
    /// cross-contract invocations or persistent storage writes.
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
        // AUTH: must be first — no storage reads, pause checks, or
        // cross-contract invocations before this line.
        creator.require_auth();
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
            validate_verifiers(&creator, verifiers, approval_threshold)?;
            if approval_threshold > verifiers.len() {
                return Err(ContractError::ApprovalThresholdExceedsVerifiers);
            }
        }

        validate_reward_token(&env, &reward_token)?;

        if !storage::is_reward_token_allowed(&env, &reward_token) {
            return Err(ContractError::RewardTokenNotAllowed);
        }

        if milestones.len() > MAX_MILESTONES {
            fail(ContractError::TooManyMilestones);
        }

        if !milestones.is_empty() {
            let mut total: i128 = 0;
            let mut previous_deadline: Option<u32> = None;
            for m in milestones.iter() {
                if m.reward <= 0 {
                    fail(ContractError::MilestoneRewardMustBePositive);
                }
                if m.completed {
                    fail(ContractError::MilestoneAlreadyCompleted);
                }
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

        let limit = storage::get_creator_open_limit(&env);
        if limit > 0 && storage::get_creator_open_count(&env, &creator) >= limit {
            return Err(ContractError::CreatorLimitReached);
        }

        // Escrow the reward up front: move `reward_amount` from the creator
        // into the contract's own account so every payout path (completion,
        // milestone, dispute "complete") is backed by funds already held by
        // the contract. This is atomic with the rest of `create_bounty`: if
        // the transfer fails the whole invocation reverts and no bounty is
        // stored.
        escrow_transfer(&env, &reward_token, &creator, reward_amount);

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
            if cooldown > 0 && env.ledger().sequence() < unclaimed_at.saturating_add(cooldown) {
                fail(ContractError::ClaimCooldownActive);
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

        // GUARD: only an in-progress bounty can be unclaimed. Completed,
        // cancelled, disputed, and submitted bounties are terminal or under
        // review and must not be mutated by an assignee walking away.
        ensure_status(
            &bounty,
            &[Symbol::new(&env, STATUS_IN_PROGRESS)],
            ContractError::BountyNotInProgress,
        );

        let is_assignee = bounty.assignees.iter().any(|(addr, _)| addr == contributor);
        if !is_assignee {
            return Err(ContractError::NotAssignee);
        }

        let mut remaining: Vec<(Address, u32)> = Vec::new(&env);
        for (addr, share) in bounty.assignees.iter() {
            if addr != contributor {
                remaining.push_back((addr, share));
            }
        }

        // Rebalance basis-point shares across the remaining assignees so they
        // always sum to exactly 10_000. The first remaining assignee absorbs
        // any division remainder, mirroring `claim_bounty`.
        let remaining_count = remaining.len();
        if remaining_count > 0 {
            let base_share: u32 = 10_000 / remaining_count;
            let remainder: u32 = 10_000 % remaining_count;
            let mut rebalanced: Vec<(Address, u32)> = Vec::new(&env);
            for (i, (addr, _)) in remaining.iter().enumerate() {
                let share = if i == 0 {
                    base_share + remainder
                } else {
                    base_share
                };
                rebalanced.push_back((addr, share));
            }
            remaining = rebalanced;
        }
        bounty.assignees = remaining;

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
            // The last assignee left: clear the claim timestamp so a future
            // claim starts a fresh claim window.
            storage::clear_claimed_at_ledger(&env, &bounty_id);
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

        // #1180: milestones may still be signed off after work is submitted.
        ensure_status(
            &bounty,
            &[
                Symbol::new(&env, STATUS_IN_PROGRESS),
                Symbol::new(&env, STATUS_SUBMITTED),
            ],
            ContractError::BountyNotInProgress,
        );

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

        // Intermediate milestone payout: transfer the proportional reward and
        // credit `total_earned` only. `active_claims`, `contribution_count`,
        // and completion reputation are finalized once, by
        // `complete_bounty_inner`, when the last milestone completes.
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
        // Depends on claim_bounty having written STATUS_IN_PROGRESS (or
        // submit_work having written STATUS_SUBMITTED, #1180) and
        // complete_bounty writing STATUS_COMPLETED below (checks-effects-interactions).
        ensure_status(
            &bounty,
            &[
                Symbol::new(&env, STATUS_IN_PROGRESS),
                Symbol::new(&env, STATUS_SUBMITTED),
            ],
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

        // #1180: approvals are only meaningful on a claimed bounty, before or
        // after `submit_work`. Disputed bounties go through resolve_dispute.
        if bounty.status == Symbol::new(&env, STATUS_DISPUTED) {
            fail(ContractError::BountyIsDisputed);
        }
        ensure_status(
            &bounty,
            &[
                Symbol::new(&env, STATUS_IN_PROGRESS),
                Symbol::new(&env, STATUS_SUBMITTED),
            ],
            ContractError::BountyNotInProgress,
        );

        // If no required_verifiers list is set, fall back to immediate single-verifier completion.
        if bounty.required_verifiers.is_none() {
            return complete_bounty_inner(env, bounty_id, bounty);
        }

        // #1179: the multi-sig branch must apply the same status guards as
        // the single-verifier path. A disputed bounty must go through
        // resolve_dispute, and open, completed or cancelled bounties must
        // never accumulate approvals.
        if bounty.status == Symbol::new(&env, STATUS_DISPUTED) {
            fail(ContractError::BountyIsDisputed);
        }
        ensure_status(
            &bounty,
            &[
                Symbol::new(&env, STATUS_IN_PROGRESS),
                Symbol::new(&env, STATUS_SUBMITTED),
            ],
            ContractError::BountyNotInProgress,
        );

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
                finalize_completion_stats(&env, &bounty.assignees);
                for tag in bounty.tags.iter() {
                    storage::remove_tag_bounty(&env, &tag, &bounty_id);
                }
                let (primary_assignee, _) = bounty
                    .assignees
                    .get(0)
                    .unwrap_or_else(|| fail(ContractError::BountyHasNoAssignee));
                events::emit_bounty_completed(&env, &bounty_id, &primary_assignee);
                return;
            }

            // #1179: checks-effects-interactions. Persist the terminal status,
            // the status index move and the creator count *before* any
            // external SEP-41 transfer, so a re-entrant call observes a
            // completed bounty and cannot trigger a second payout.
            let (primary_assignee, _) = bounty.assignees.get(0).unwrap_or_else(|| fail(ContractError::BountyHasNoAssignee));
            let previous_status = bounty.status.clone();
            bounty.status = Symbol::new(&env, STATUS_COMPLETED);
            storage::store_bounty(&env, &bounty_id, &bounty);
            storage::move_bounty_status(&env, &bounty_id, &previous_status, &bounty.status);
            storage::decrement_creator_open_count(&env, &bounty.creator);
            storage::cleanup_terminal_bounty_keys(&env, &bounty_id);

            let token = TokenClient::new(&env, &bounty.reward_token);
            distribute_payout(&env, &bounty_id, &bounty.assignees, &token, bounty.reward_amount);
            finalize_completion_stats(&env, &bounty.assignees);

            let (primary_assignee, _) = bounty
                .assignees
                .get(0)
                .unwrap_or_else(|| fail(ContractError::BountyHasNoAssignee));
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

        if bounty.status == Symbol::new(&env, STATUS_DISPUTED) {
            fail(ContractError::BountyIsDisputed);
        }

        // #1180: a submitted bounty is still live and must remain disputable.
        ensure_status(
            &bounty,
            &[
                Symbol::new(&env, STATUS_OPEN),
                Symbol::new(&env, STATUS_IN_PROGRESS),
                Symbol::new(&env, STATUS_SUBMITTED),
            ],
            ContractError::BountyNotDisputed,
        );

        let is_assignee = bounty.assignees.iter().any(|(addr, _)| addr == caller);
        if caller != bounty.creator && !is_assignee {
            fail(ContractError::OnlyCreatorOrAssigneeCanDispute);
        }

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
        // #1197: a disputed bounty is still active, so it keeps its slot in
        // `creator_open_count` until `apply_dispute_resolution` makes it
        // terminal. Decrementing here let creators bypass the limit by
        // disputing their own bounties, then double-decrement on resolution.
        events::emit_bounty_disputed(&env, &bounty_id, &caller);
    }

    /// Resolve a disputed bounty. Only the admin-designated arbitrator may call this when one
    /// is set via `set_arbitrator` (#1195); otherwise the bounty creator acts as arbitrator.
    /// resolution must be the Symbol "complete" (pay assignees) or "cancel" (refund creator).
    ///
    /// When resolution is "complete" and the creator is the arbitrator, the arbitrator's wallet
    /// funds the payout to each assignee (mirroring complete_bounty's verifier-funds-the-payout
    /// model); a designated arbitrator is not a party to the bounty, so its decision pays from
    /// the contract's escrow.
    ///
    /// # Authorization
    /// `arbitrator.require_auth()` is the **first** operation in this function.
    /// No storage reads or business logic execute before authentication is checked.
    pub fn resolve_dispute(env: Env, arbitrator: Address, bounty_id: BountyId, resolution: Symbol) {
        arbitrator.require_auth();
        require_not_paused(&env);

```rust
let existing = storage::get_sponsors(&env, &bounty_id);
let is_new = !existing.iter().any(|(addr, _)| addr == sponsor);
if is_new && existing.len() >= storage::MAX_SPONSORS_PER_BOUNTY {
    fail(ContractError::TooManySponsors);
}
```

This is clean. Let me go with this.

        // #1195: an admin-designated independent arbitrator, when configured,
        // is the only party allowed to resolve, and must not have a stake in
        // the outcome. Without one, the bounty creator keeps the role.
        match storage::get_arbitrator(&env) {
            Some(designated) => {
                if arbitrator != designated {
                    fail(ContractError::NotArbitrator);
                }
                let is_assignee = bounty.assignees.iter().any(|(addr, _)| addr == arbitrator);
                if arbitrator == bounty.creator || is_assignee {
                    fail(ContractError::ArbitratorConflict);
                }
            }
            None => {
                if arbitrator != bounty.creator {
                    fail(ContractError::NotArbitrator);
                }
            }
        }
        // For `MIN_SPONSOR_AMOUNT`, reference `storage::MIN_SPONSOR_AMOUNT`.

OK let me write the full file now. I need to be careful to preserve everything.

        // GUARD (#1196): a decision already waiting out its appeal window must
        // be finalized or appealed first. Otherwise the arbitrator could keep
        // resetting `decided_at` to stall `finalize_dispute` indefinitely, or
        // swap the decision mid-window after the parties have relied on it.
        if storage::get_pending_resolution(&env, &bounty_id).is_some() {
            fail(ContractError::ResolutionAlreadyPending);
        }

        // GUARD: arbitrator (creator) must meet the bounty's own min_reputation threshold.
        // This reflects the security/minimum-reputation-enforcement.md recommendation
        // that dispute resolvers are subject to a reputation floor.
        let arbitrator_contrib = storage::get_contributor(&env, &arbitrator)
            .unwrap_or_else(|| Contributor::new(arbitrator.clone()));
        if bounty.min_reputation > 0 && arbitrator_contrib.reputation < bounty.min_reputation {
            fail(ContractError::ReputationTooLow);
        }

        // GUARD: reopened bounties must remain completable — no uncompleted
        // milestone may already be past its deadline or extend beyond the new
        // bounty deadline.
        for milestone in bounty.milestones.iter() {
            if milestone.completed {
                continue;
            }
            if milestone.deadline < env.ledger().sequence()
                || milestone.deadline > new_deadline
            {
                fail(ContractError::MilestoneDeadlineExpired);
            }
        }

"checks whether any uncompleted milestone carries a `deadline` earlier than the current ledger sequence (or allows updating milestone deadlines atomically), preventing reopened bounties from being uncompletable due to already-expired milestone deadlines."

And: "Implement validation (or optional milestone-deadline rescheduling) ensuring reopened bounties maintain `milestone.deadline <= new_deadline` and future milestone feasibility."

So the checks are:
1. For each uncompleted milestone with a deadline:
   - If `milestone.deadline < env.ledger().sequence()` → already expired → fail with `MilestoneDeadlineExpired`
   - If `milestone.deadline > new_deadline` → beyond new deadline → fail with `MilestoneDeadlineExpired` (or a different error)

The step says: "fail with `ContractError::MilestoneDeadlineExpired` when `milestone.deadline < env.ledger().sequence()` or when `milestone.deadline > new_deadline`"

        let window = storage::get_appeal_window(&env);
        if env.ledger().sequence() < pending.decided_at.saturating_add(window) {
            fail(ContractError::FinalizeTooEarly);
        }

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

        let window = storage::get_appeal_window(&env);
        if env.ledger().sequence() >= pending.decided_at + window {
            fail(ContractError::AppealWindowClosed);
        }

        storage::remove_pending_resolution(&env, &bounty_id);
        storage::set_dispute_appealed(&env, &bounty_id);

        events::emit_dispute_appealed(&env, &bounty_id, &caller);
    }

    /// Set the admin-configurable appeal window, in ledgers, that follows a
    /// dispute decision before it can be finalized. A value of `0` keeps the
    /// original immediate-payout behaviour (the default).
    ///
    /// Uses the same lazily-established admin as the other admin entrypoints.
    ///
    /// # Authorization
    /// Requires auth from `caller`. Restricted to the contract admin once one
    /// has been established.
    pub fn set_appeal_window(env: Env, caller: Address, ledgers: u32) {
        caller.require_auth();
        require_not_paused(&env);

        match storage::get_admin(&env) {
            Some(admin) => {
                if caller != admin {
                    fail(ContractError::NotAdmin);
                }
            }
            None => storage::set_admin(&env, &caller),
        }

        let old = storage::get_appeal_window(&env);
        storage::set_appeal_window(&env, ledgers);
        events::emit_config_changed_u32(&env, "appeal_window", old, ledgers);
    }

    /// Attach an evidence hash to an open dispute.
    ///
    /// Only the bounty creator or an assignee may call this. Evidence can no
    /// longer be added once the dispute has been resolved (status is no
    /// longer `"disputed"`), and at most `storage::MAX_DISPUTE_EVIDENCE`
    /// entries are retained per dispute.
    ///
    /// # Arguments
    /// * `caller` - The creator or an assignee of the disputed bounty.
    /// * `bounty_id` - The disputed bounty.
    /// * `hash` - A 32-byte hash referencing off-chain evidence.
    ///
    /// # Panics
    /// * If `bounty_id` does not exist.
    /// * If the bounty is not currently in `"disputed"` status.
    /// * If `caller` is neither the creator nor an assignee.
    /// * If the evidence cap has already been reached.
    ///
    /// # Authorization
    /// Requires auth from `caller`.
    pub fn add_dispute_evidence(env: Env, caller: Address, bounty_id: BountyId, hash: BytesN<32>) {
        caller.require_auth();
        require_not_paused(&env);

        let bounty = match storage::get_bounty(&env, &bounty_id) {
            Some(b) => b,
            None => fail(ContractError::BountyNotFound),
        };

        if bounty.status != Symbol::new(&env, STATUS_DISPUTED) {
            fail(ContractError::DisputeAlreadyResolved);
        }

        let is_assignee = bounty.assignees.iter().any(|(addr, _)| addr == caller);
        if caller != bounty.creator && !is_assignee {
            fail(ContractError::OnlyPartyCanAddEvidence);
        }

        if is_zero_hash(&hash) {
            fail(ContractError::ZeroHash);
        }

        let existing = storage::get_dispute_evidence(&env, &bounty_id);
        if existing.iter().any(|h| h == hash) {
            fail(ContractError::DuplicateEvidence);
        }
        if existing.len() >= storage::MAX_DISPUTE_EVIDENCE {
            fail(ContractError::EvidenceLimitReached);
        }

        storage::append_dispute_evidence(&env, &bounty_id, &hash);
        events::emit_dispute_evidence_added(&env, &bounty_id, &caller, &hash);
    }

    if amount <= 0 {
        fail(ContractError::AmountMustBePositive);
    }

        let window = storage::get_appeal_window(&env);
        if env.ledger().sequence() >= pending.decided_at.saturating_add(window) {
            fail(ContractError::AppealWindowClosed);
        }

        storage::remove_pending_resolution(&env, &bounty_id);
        storage::set_dispute_appealed(&env, &bounty_id);

        events::emit_dispute_appealed(&env, &bounty_id, &caller);
    }

    /// Set the admin-configurable appeal window, in ledgers, that follows a
    /// dispute decision before it can be finalized. A value of `0` keeps the
    /// original immediate-payout behaviour (the default).
    ///
    /// Uses the same lazily-established admin as the other admin entrypoints.
    ///
    /// # Authorization
    /// Requires auth from `caller`. Restricted to the contract admin once one
    /// has been established.
    pub fn set_appeal_window(env: Env, caller: Address, ledgers: u32) {
        caller.require_auth();
        require_not_paused(&env);

        match storage::get_admin(&env) {
            Some(admin) => {
                if caller != admin {
                    fail(ContractError::NotAdmin);
                }
            }
            None => storage::set_admin(&env, &caller),
        }

        validate_window(ledgers);
        let old = storage::get_appeal_window(&env);
        storage::set_appeal_window(&env, ledgers);
        events::emit_config_changed_u32(&env, "appeal_window", old, ledgers);
    }

    /// Attach an evidence hash to an open dispute.
    ///
    /// Only the bounty creator or an assignee may call this. Evidence can no
    /// longer be added once the dispute has been resolved (status is no
    /// longer `"disputed"`), and at most `storage::MAX_DISPUTE_EVIDENCE`
    /// entries are retained per dispute.
    ///
    /// # Arguments
    /// * `caller` - The creator or an assignee of the disputed bounty.
    /// * `bounty_id` - The disputed bounty.
    /// * `hash` - A 32-byte hash referencing off-chain evidence.
    ///
    /// # Panics
    /// * If `bounty_id` does not exist.
    /// * If the bounty is not currently in `"disputed"` status.
    /// * If `caller` is neither the creator nor an assignee.
    /// * If the evidence cap has already been reached.
    ///
    /// # Authorization
    /// Requires auth from `caller`.
    pub fn add_dispute_evidence(env: Env, caller: Address, bounty_id: BountyId, hash: BytesN<32>) {
        caller.require_auth();
        require_not_paused(&env);

        let bounty = match storage::get_bounty(&env, &bounty_id) {
            Some(b) => b,
            None => fail(ContractError::BountyNotFound),
        };

        if bounty.status != Symbol::new(&env, STATUS_DISPUTED) {
            fail(ContractError::DisputeAlreadyResolved);
        }

        let is_assignee = bounty.assignees.iter().any(|(addr, _)| addr == caller);
        if caller != bounty.creator && !is_assignee {
            fail(ContractError::OnlyPartyCanAddEvidence);
        }

        let existing = storage::get_dispute_evidence(&env, &bounty_id);
        if existing.len() >= storage::MAX_DISPUTE_EVIDENCE {
            fail(ContractError::EvidenceLimitReached);
        }

        storage::append_dispute_evidence(&env, &bounty_id, &hash);
        events::emit_dispute_evidence_added(&env, &bounty_id, &caller, &hash);
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

        if is_zero_hash(&submission_hash) {
            fail(ContractError::ZeroHash);
        }

        storage::set_submission(&env, &bounty_id, &submission_hash);

    events::emit_bounty_sponsored(&env, &bounty_id, &sponsor, &amount, &bounty.reward_amount);
}

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
    }

        // #1183: milestone bounties pay out per milestone, and
        // `sum(milestones.reward) == reward_amount` is an invariant set at
        // creation. There is no sound way to allocate a top-up across
        // milestones (some may already be paid), so sponsorship is rejected.
        if !bounty.milestones.is_empty() {
            fail(ContractError::MilestoneRewardsMismatch);
        }

        // #1183: checked addition before any funds move, so an overflow can
        // never leave tokens escrowed against an unchanged reward.
        let new_reward = bounty
            .reward_amount
            .checked_add(amount)
            .unwrap_or_else(|| fail(ContractError::RewardAmountOverflow));

        let token = TokenClient::new(&env, &bounty.reward_token);
        token.transfer(&sponsor, env.current_contract_address(), &amount);

        storage::add_sponsor_contribution(&env, &bounty_id, &sponsor, amount);

        bounty.reward_amount = new_reward;
        storage::store_bounty(&env, &bounty_id, &bounty);

        events::emit_bounty_sponsored(&env, &bounty_id, &sponsor, &amount, &bounty.reward_amount);
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
        // If the bounty's global deadline has also passed, cancel it and
        // refund escrow rather than re-opening it into an immediately-
        // unclaimable state.  This prevents funds from being locked
        // indefinitely in a bounty that can never be re-claimed.
        let deadline_passed = bounty
            .deadline
            .map(|d| env.ledger().sequence() > d)
            .unwrap_or(false);

        if deadline_passed {
            // Refund remaining escrowed funds (minus any already-paid
            // milestone rewards) and move to cancelled.
            refund_escrow(&env, &bounty_id, &bounty);
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
        } else {
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
        }
        storage::clear_claimed_at_ledger(&env, &bounty_id);

        events::emit_claim_released(&env, &bounty_id, &caller);
    }

    // ── Admin ─────────────────────────────────────────────────────────────

    /// One-time admin bootstrap. May only be called while no admin is set.
    ///
    /// # Deployment bootstrap sequence
    /// 1. Deploy the contract.
    /// 2. Call `init_admin(admin)` from the intended admin key. This is the
    ///    **only** way an admin is ever established — no config setter
    ///    auto-assigns the admin.
    /// 3. Call the config setters (`set_appeal_window`,
    ///    `set_creator_open_limit`, `set_claim_cooldown`, `set_claim_window`,
    ///    `set_reputation_penalty`, `add_reward_token`, `set_min_reward`, …)
    ///    from that admin key. Before step 2 every one of them fails closed
    ///    with `ContractError::NotAdmin`.
    /// 4. Optionally rotate the key via `transfer_admin` / `accept_admin`.
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

    /// Pause the contract. While paused every state-mutating entrypoint
    /// (except those in `PAUSE_EXEMPT_ENTRYPOINTS`) fails with
    /// `ContractError::ContractPaused`; queries keep working.
    ///
    /// # Authorization
    /// Admin-only; requires auth from `admin`.
    pub fn pause(env: Env, admin: Address) {
        require_admin(&env, &admin);
        if storage::is_paused(&env) {
            fail(ContractError::AlreadyPaused);
        }
        storage::set_paused(&env, true);
        events::emit_contract_paused(&env, &admin);
    }

    /// Lift a pause set by `pause`.
    ///
    /// # Authorization
    /// Admin-only; requires auth from `admin`.
    pub fn unpause(env: Env, admin: Address) {
        require_admin(&env, &admin);
        if !storage::is_paused(&env) {
            fail(ContractError::NotPaused);
        }
        storage::set_paused(&env, false);
        events::emit_contract_unpaused(&env, &admin);
    }

    bounty.deadline = Some(new_deadline);
    storage::store_bounty(&env, &bounty_id, &bounty);

    events::emit_bounty_reopened(&env, &bounty_id, &creator, new_deadline);
}

    /// Configure the claim window (in ledgers) used by `release_stale_claim`.
    ///
    /// # Authorization
    /// Admin-only; requires auth from `admin`.
    pub fn set_claim_window(env: Env, admin: Address, window: u32) {
        require_admin(&env, &admin);
        require_not_paused(&env);
        validate_window(window);
        let old = storage::get_claim_window(&env);
        storage::set_claim_window(&env, window);
        events::emit_config_changed_u32(&env, "claim_window", old, window);
    }

Let me also double-check the `claim_bounty` change. After `storage::store_contributor(&env, &contributor, &contrib);`, add:
```rust
storage::set_contributor_active_bounty(&env, &contributor, &bounty_id);
```

    /// Add a token to the on-chain reward-token allowlist. Only tokens on
    /// this list may be used as `reward_token` in `create_bounty`.
    ///
    /// # Authorization
    /// Admin-only; requires auth from `admin`.
    pub fn add_reward_token(env: Env, admin: Address, token: Address) {
        require_admin(&env, &admin);
        require_not_paused(&env);
        validate_reward_token(&env, &token);
        storage::add_allowed_reward_token(&env, &token);
        events::emit_reward_token_added(&env, &admin, &token);
    }

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

    /// Set the admin-configurable cap on concurrent open+claimed bounties
    /// per creator. A `limit` of `0` disables the cap (unlimited, the default).
    ///
    /// # Authorization
    /// Admin-only; requires auth from `caller`. The admin must have been
    /// bootstrapped via `init_admin` first — there is no lazy takeover.
    pub fn set_creator_open_limit(env: Env, caller: Address, limit: u32) {
        require_admin(&env, &caller);
        require_not_paused(&env);

        let old = storage::get_creator_open_limit(&env);
        storage::set_creator_open_limit(&env, limit);
        events::emit_config_changed_u32(&env, "creator_open_limit", old, limit);
    }

    /// Set the admin-configurable claim cooldown (in ledgers) that a
    /// contributor must wait after unclaiming a bounty before re-claiming it.
    /// A value of `0` disables the cooldown (the default).
    ///
    /// # Authorization
    /// Admin-only; requires auth from `caller`. The admin must have been
    /// bootstrapped via `init_admin` first — there is no lazy takeover.
    pub fn set_claim_cooldown(env: Env, caller: Address, ledgers: u32) {
        require_admin(&env, &caller);
        require_not_paused(&env);

        validate_window(ledgers);
        let old = storage::get_claim_cooldown(&env);
        storage::set_claim_cooldown(&env, ledgers);
        events::emit_config_changed_u32(&env, "claim_cooldown", old, ledgers);
    }

    /// Set the minimum `reward_amount` accepted by `create_bounty` for
    /// bounties paid in `token`. Applies on top of the global
    /// `MIN_REWARD_AMOUNT`; `0` removes the per-token minimum.
    ///
    /// # Authorization
    /// Admin-only; requires auth from `admin`.
    pub fn set_min_reward(env: Env, admin: Address, token: Address, amount: i128) {
        require_admin(&env, &admin);
        require_not_paused(&env);
        if amount < 0 {
            fail(ContractError::AmountMustBePositive);
        }
        validate_reward_token(&env, &token);
        let old = storage::get_min_reward(&env, &token);
        storage::set_min_reward(&env, &token, amount);
        events::emit_config_changed_min_reward(&env, &token, old, amount);
    }

    /// Designate (`Some`) or clear (`None`) an independent dispute arbitrator
    /// (#1195). While set, only this address may call `resolve_dispute`, and
    /// it is rejected on any bounty where it is the creator or an assignee;
    /// a "complete" decision then pays assignees from the contract's escrow.
    /// While unset, the bounty creator acts as arbitrator, as before.
    /// Emits `config_changed` / `arbitrator`.
    ///
    /// # Authorization
    /// Admin-only; requires auth from `admin`.
    pub fn set_arbitrator(env: Env, admin: Address, arbitrator: Option<Address>) {
        require_admin(&env, &admin);
        require_not_paused(&env);
        let old = storage::get_arbitrator(&env);
        storage::set_arbitrator(&env, &arbitrator);
        events::emit_config_changed_arbitrator(&env, old, arbitrator);
    }

    /// Begin a two-step admin transfer by proposing `new_admin`. The
    /// transfer completes only once `new_admin` calls `accept_admin`.
    ///
    /// # Authorization
    /// Admin-only; requires auth from `admin`.
    pub fn transfer_admin(env: Env, admin: Address, new_admin: Address) {
        require_admin(&env, &admin);
        storage::set_pending_admin(&env, &new_admin);
    }

    /// Cancel a pending admin transfer, e.g. after nominating a wrong
    /// address. Clears `PendingAdmin` so the nominee can no longer
    /// `accept_admin`, and emits `config_changed` for `pending_admin`.
    /// Pause-exempt, so an erroneous nomination can be revoked mid-incident.
    ///
    /// # Panics
    /// * If no transfer is pending (`ContractError::NoPendingAdmin`).
    ///
    /// # Authorization
    /// Admin-only; requires auth from `admin`.
    pub fn cancel_admin_transfer(env: Env, admin: Address) {
        require_admin(&env, &admin);
        let pending = match storage::get_pending_admin(&env) {
            Some(p) => p,
            None => fail(ContractError::NoPendingAdmin),
        };
        storage::clear_pending_admin(&env);
        events::emit_config_changed_pending_admin_cleared(&env, &pending);
    }

    /// Complete a pending admin transfer. Emits `config_changed` for `admin`.
    ///
    /// # Panics
    /// * If `new_admin` is not the pending admin (`ContractError::NotPendingAdmin`).
    ///
    /// # Authorization
    /// Requires auth from `new_admin`.
    pub fn accept_admin(env: Env, new_admin: Address) {
        new_admin.require_auth();
        if storage::get_pending_admin(&env) != Some(new_admin.clone()) {
            fail(ContractError::NotPendingAdmin);
        }
        let old = storage::get_admin(&env);
        storage::set_admin(&env, &new_admin);
        storage::clear_pending_admin(&env);
        events::emit_config_changed_address(&env, "admin", old, &new_admin);
    }

Hmm, that's a problem. `distribute_payout` is used for both terminal completion and milestone completion. If I add the clear inside `distribute_payout`, it would incorrectly clear on milestone completion.

Let me NOT add the clear inside `distribute_payout`. Instead, add it explicitly in the terminal paths.

So:
- `complete_bounty_inner`: add `clear_active_bounty_for_assignees` in both branches (milestone and non-milestone)
- `apply_dispute_resolution`: add in both branches
- `complete_milestone`: do NOT add (not terminal)

        // #1193: transferring to yourself is a no-op that would only leave a
        // pending entry behind.
        if new_owner == bounty.creator {
            fail(ContractError::OwnerCannotBeCreator);
        }

        // #1193: ownership of a finished bounty is meaningless; only active
        // bounties may change hands.
        ensure_status(
            &bounty,
            &[
                Symbol::new(&env, STATUS_OPEN),
                Symbol::new(&env, STATUS_IN_PROGRESS),
                Symbol::new(&env, STATUS_SUBMITTED),
            ],
            ContractError::InvalidStatus,
        );

        if bounty.assignees.iter().any(|(addr, _)| addr == new_owner) {
            fail(ContractError::OwnerCannotBeAssignee);
        }

        storage::set_pending_owner(&env, &bounty_id, &new_owner);
        events::emit_ownership_transfer_started(&env, &bounty_id, &creator, &new_owner);
    }

    /// Complete a pending bounty ownership transfer. Moves the bounty between
    /// the creator indexes backing `get_bounties_by_creator`, and its slot in
    /// the per-creator open-bounty count while the bounty is still active.
    ///
    /// # Panics
    /// * If `bounty_id` does not exist (`ContractError::BountyNotFound`).
    /// * If `new_owner` is not the pending owner (`ContractError::NotPendingOwner`).
    /// * If `new_owner` became an assignee since the transfer started
    ///   (`ContractError::OwnerCannotBeAssignee`).
    /// * If the bounty is active and `new_owner` is at the creator open-bounty
    ///   cap (`ContractError::CreatorLimitReached`).
    ///
    /// # Authorization
    /// Requires auth from `new_owner`.
    pub fn accept_bounty_ownership(env: Env, new_owner: Address, bounty_id: BountyId) {
        new_owner.require_auth();
        require_not_paused(&env);

        let mut bounty = match storage::get_bounty(&env, &bounty_id) {
            Some(b) => b,
            None => fail(ContractError::BountyNotFound),
        };

        if storage::get_pending_owner(&env, &bounty_id) != Some(new_owner.clone()) {
            fail(ContractError::NotPendingOwner);
        }

        // #1193: a transfer started on an active bounty must not complete
        // once the bounty has reached a terminal status.
        if bounty.status == Symbol::new(&env, STATUS_COMPLETED)
            || bounty.status == Symbol::new(&env, STATUS_CANCELLED)
        {
            fail(ContractError::InvalidStatus);
        }

        if bounty.assignees.iter().any(|(addr, _)| addr == new_owner) {
            fail(ContractError::OwnerCannotBeAssignee);
        }

        // Statuses counted by the per-creator open-bounty cap.
        // `disputed` stays active until `apply_dispute_resolution` (#1197).
        let is_active = bounty.status == Symbol::new(&env, STATUS_OPEN)
            || bounty.status == Symbol::new(&env, STATUS_IN_PROGRESS)
            || bounty.status == Symbol::new(&env, STATUS_SUBMITTED)
            || bounty.status == Symbol::new(&env, STATUS_DISPUTED);
        if is_active {
            let limit = storage::get_creator_open_limit(&env);
            if limit > 0 && storage::get_creator_open_count(&env, &new_owner) >= limit {
                fail(ContractError::CreatorLimitReached);
            }
        }

        let previous_owner = bounty.creator.clone();
        bounty.creator = new_owner.clone();
        storage::store_bounty(&env, &bounty_id, &bounty);
        storage::clear_pending_owner(&env, &bounty_id);

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

    /// Revoke a pending, not-yet-accepted ownership transfer (#1193).
    ///
    /// Lets the creator recover from a transfer to a wrong or unresponsive
    /// address. After cancellation, `accept_bounty_ownership` by the former
    /// pending owner fails with `NotPendingOwner`.
    ///
    /// # Panics
    /// * If `bounty_id` does not exist (`ContractError::BountyNotFound`).
    /// * If `creator` is not the bounty creator (`ContractError::NotBountyCreator`).
    /// * If no transfer is pending (`ContractError::NoPendingOwner`).
    ///
    /// # Events
    /// Emits `ownership_transfer_cancelled` with `(bounty_id, pending_owner)`.
    ///
    /// # Authorization
    /// Requires auth from `creator`.
    pub fn cancel_bounty_ownership_transfer(env: Env, creator: Address, bounty_id: BountyId) {
        creator.require_auth();
        require_not_paused(&env);

        let bounty = match storage::get_bounty(&env, &bounty_id) {
            Some(b) => b,
            None => fail(ContractError::BountyNotFound),
        };

        if creator != bounty.creator {
            fail(ContractError::NotBountyCreator);
        }

        let pending = match storage::get_pending_owner(&env, &bounty_id) {
            Some(p) => p,
            None => fail(ContractError::NoPendingOwner),
        };

        storage::clear_pending_owner(&env, &bounty_id);
        events::emit_ownership_transfer_cancelled(&env, &bounty_id, &creator, &pending);
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

    /// Start the archive grace period for a finished bounty that has no
    /// grace-period marker yet (#1181), without panicking.
    ///
    /// Bounties that reach `completed`/`cancelled` get their marker
    /// automatically; this exists for bounties that finished before markers
    /// were recorded at the terminal transition. Idempotent: an existing
    /// marker is left unchanged. Returns the ledger sequence from which
    /// `archive_bounty` will succeed.
    ///
    /// # Panics
    /// * If `bounty_id` does not exist (`ContractError::BountyNotFound`).
    /// * If the bounty is not completed or cancelled (`ContractError::InvalidStatus`).
    pub fn schedule_archive(env: Env, bounty_id: BountyId) -> u32 {
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

        storage::ensure_archive_eligible_at(&env, &bounty_id)
    }

    /// Archive a finished bounty, compacting it into a `BountySummary` and
    /// removing the full `Bounty`/`BountyMeta` records to stop extending
    /// their TTL (and so paying rent on data nobody reads anymore).
    ///
    /// Only bounties in a terminal status (`"completed"` or `"cancelled"`)
    /// can be archived. Callable by anyone (not just the creator), subject
    /// to a grace period: the bounty becomes archivable
    /// `storage::ARCHIVE_GRACE_PERIOD_LEDGERS` after it reached its terminal
    /// status (or after `schedule_archive`, for bounties finished before the
    /// marker was recorded at the transition). Earlier calls fail with
    /// `DeadlineNotPassed` and leave the marker unchanged (#1181).
    ///
    /// After archiving, `get_bounty` keeps returning a (summary-only)
    /// `Bounty` for `bounty_id` so existing callers do not start seeing
    /// `None` for bounties that used to exist.
    ///
    /// # Panics
    /// * If `bounty_id` does not exist (`ContractError::BountyNotFound`).
    /// * If the bounty is not completed or cancelled (`ContractError::InvalidStatus`).
    /// * If the grace period has not yet elapsed (`ContractError::DeadlineNotPassed`).
    ///
    /// # Authorization
    /// No caller auth is required (anyone may archive), but the pause check
    /// runs before any storage read.
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

        // #1181: the grace-period marker is written durably when the bounty
        // reaches a terminal status (`storage::move_bounty_status`), or by
        // `schedule_archive` for bounties finished before that existed. It is
        // never written here: a write followed by `fail` is rolled back with
        // the rest of the invocation, so the marker would never persist.
        let now = env.ledger().sequence();
        match storage::get_archive_eligible_at(&env, &bounty_id) {
            None => fail(ContractError::DeadlineNotPassed),
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
