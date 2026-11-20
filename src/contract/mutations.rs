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

/// Validate a tag list: reject more than `MAX_TAGS` entries, reject
/// duplicates, and reject any tag not on the allow-list (via
/// `symbols::validate_symbol_or_fail`).
fn validate_tags(env: &Env, tags: &Vec<Symbol>) {
    if tags.len() > MAX_TAGS {
        fail(ContractError::TooManyTags);
    }

    for (i, tag) in tags.iter().enumerate() {
        symbols::validate_symbol_or_fail(env, SymbolKind::Tag, &tag);
        for other in tags.iter().skip(i + 1) {
            if other == tag {
                fail(ContractError::DuplicateTag);
            }
        }
    }
}

/// Verify a bounty's status is one of `allowed`, failing with `err` otherwise.
///
/// Centralizes the status-transition guards that used to be duplicated
/// across `claim_bounty`, `cancel_bounty`, and `complete_bounty`.
fn ensure_status(bounty: &Bounty, allowed: &[Symbol], err: ContractError) {
    if !allowed.iter().any(|s| s == &bounty.status) {
        fail(err);
    }
}

/// Require that `caller` is authenticated and is the configured admin.
fn require_admin(env: &Env, caller: &Address) {
    caller.require_auth();
    match storage::get_admin(env) {
        Some(admin) if admin == *caller => {}
        _ => fail(ContractError::NotAdmin),
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
fn require_not_paused(env: &Env) {
    if storage::is_paused(env) {
        fail(ContractError::ContractPaused);
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
    let (primary_assignee, _) = assignees.get(0).unwrap_or_else(|| fail(ContractError::BountyHasNoAssignee));

    for (assignee, share_bp) in assignees.iter() {
        let payout = reward_amount * (share_bp as i128) / 10_000_i128;
        token.transfer(from, &assignee, &payout);

        let mut contrib = storage::get_contributor(env, &assignee)
            .unwrap_or_else(|| Contributor::new(assignee.clone()));
        set_reputation(env, &mut contrib, &assignee, 10, "completion");
        contrib.total_earned += payout;
        contrib.contribution_count += 1;
        if contrib.active_claims > 0 {
            contrib.active_claims -= 1;
        }
        storage::store_contributor(env, &assignee, &contrib);
        // #1216: clear the O(1) active-bounty mapping on completion.
        storage::remove_contributor_active_bounty(env, &assignee);
        events::emit_reward_paid(env, bounty_id, &assignee, &payout);
    }

    primary_assignee
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
        let (primary_assignee, _) = bounty.assignees.get(0).unwrap_or_else(|| fail(ContractError::BountyHasNoAssignee));
        let previous_status = bounty.status.clone();
        bounty.status = Symbol::new(&env, STATUS_COMPLETED);
        storage::store_bounty(&env, &bounty_id, &bounty);
        storage::move_bounty_status(&env, &bounty_id, &previous_status, &bounty.status);
        storage::decrement_creator_open_count(&env, &bounty.creator);
        // #1216: clear active-bounty mapping for all assignees.
        clear_active_bounty_for_assignees(&env, &bounty);
        events::emit_bounty_completed(&env, &bounty_id, &primary_assignee);
        return;
    }

    let (primary_assignee, _) = bounty.assignees.get(0).unwrap_or_else(|| fail(ContractError::BountyHasNoAssignee));
    let previous_status = bounty.status.clone();
    bounty.status = Symbol::new(&env, STATUS_COMPLETED);
    storage::store_bounty(&env, &bounty_id, &bounty);
    storage::move_bounty_status(&env, &bounty_id, &previous_status, &bounty.status);
    storage::decrement_creator_open_count(&env, &bounty.creator);

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
) {
    let resolve_complete = Symbol::new(&env, "complete");

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
            // #1216: clear active-bounty mapping for all assignees.
            clear_active_bounty_for_assignees(&env, &bounty);
        } else {
            let token = TokenClient::new(&env, &bounty.reward_token);

            for (assignee, share_bp) in bounty.assignees.iter() {
                let payout = bounty.reward_amount * (share_bp as i128) / 10_000_i128;
                token.transfer(&arbitrator, &assignee, &payout);

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
            storage::move_bounty_status(&env, &bounty_id, &previous_status, &bounty.status);
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
        storage::move_bounty_status(&env, &bounty_id, &previous_status, &bounty.status);
        // #1216: clear active-bounty mapping for all assignees.
        clear_active_bounty_for_assignees(&env, &bounty);
    }

    events::emit_dispute_resolved(&env, &bounty_id, &arbitrator, &resolution);
}

#[contractimpl]
impl MergeMintContract {
    // ... (create_bounty unchanged)
```

Wait, I need to be careful. Let me not truncate the file. Let me write out the full file properly.

Actually, since the file is very long, let me focus on writing the complete file with all the changes. I'll be careful to preserve everything.

Let me think about the changes needed:

1. **`distribute_payout`**: Add `storage::remove_contributor_active_bounty(env, &assignee);` after storing contributor.

2. **`complete_bounty_inner`**: Add `clear_active_bounty_for_assignees` calls in both milestone and non-milestone paths. Actually, `distribute_payout` already handles the non-milestone path. For the milestone path, need to add it.

3. **`apply_dispute_resolution`**: Add clearing in both branches.

4. **`claim_bounty`**: Add `storage::set_contributor_active_bounty(&env, &contributor, &bounty_id);`

5. **`unclaim_bounty`**: Add `storage::remove_contributor_active_bounty(&env, &contributor);`

6. **`reassign_bounty`**: Add `storage::set_contributor_active_bounty(&env, &new_assignee, &bounty_id);` and `storage::remove_contributor_active_bounty(&env, &old_assignee);`

7. **`release_stale_claim`**: Add `storage::remove_contributor_active_bounty(&env, &assignee);` in the loop.

8. **`cancel_bounty`**: Add `clear_active_bounty_for_assignees(&env, &bounty);` — but wait, cancel only works on open bounties which have no assignees. Still, defensive.

9. **`expire_bounty`**: Same as cancel — open only, no assignees. But defensive.

10. **`sponsor_bounty`**: Add `MIN_SPONSOR_AMOUNT` and `MAX_SPONSORS_PER_BOUNTY` checks.

11. **`reopen_bounty`**: Add milestone deadline validation.

Now, for the milestone validation in `reopen_bounty`:
- Iterate uncompleted milestones
- Fail with `ContractError::MilestoneDeadlineExpired` if `milestone.deadline < env.ledger().sequence()` (already expired)
- Fail if `milestone.deadline > new_deadline` (would be beyond new bounty deadline)

Wait, the issue says "checks whether any uncompleted milestone carries a `deadline` earlier than the current ledger sequence (or allows updating milestone deadlines atomically)". So we need to check for already-expired milestones. And the requirement says "ensuring reopened bounties maintain `milestone.deadline <= new_deadline` and future milestone feasibility."

So both checks:
- `milestone.deadline < env.ledger().sequence()` → expired → fail
- `milestone.deadline > new_deadline` → beyond new deadline → fail

For sponsor checks in `sponsor_bounty`:
- `amount < MIN_SPONSOR_AMOUNT` → fail with `SponsorAmountBelowMinimum`
- Check if sponsor is new and count >= MAX_SPONSORS_PER_BOUNTY → fail with `TooManySponsors`

I need to assume `storage::get_sponsor_count` or similar exists. Actually, looking at `add_sponsor_contribution`, it likely returns whether the sponsor is new. Let me think — the cleanest way is to check before calling. But I don't know the exact storage API.

Let me use a reasonable approach: check if the sponsor already exists in the list, and if not, check the count. I'll assume `storage::get_sponsor_count(env, bounty_id)` exists, or I can use `storage::get_sponsors` if it exists.

Actually, looking at `take_sponsors`, there's likely a `get_sponsors` too. But to be safe, let me assume `storage::get_sponsor_count` exists. Hmm, but I don't know for sure.

Let me look at what's most likely. The issue says "Implement `checked_add`, a bounded `MAX_SPONSORS_PER_BOUNTY` cap, and a minimum sponsorship floor (`MIN_SPONSOR_AMOUNT`)." — this is in `storage.rs`. So the cap enforcement might be in `storage::add_sponsor_contribution` itself.

But the step says: "Enforce `MIN_SPONSOR_AMOUNT`/`MAX_SPONSORS_PER_BOUNTY` in `sponsor_bounty` before calling `storage::add_sponsor_contribution`."

So I need to enforce in `sponsor_bounty`. I'll assume there's a way to check the sponsor count. Let me use `storage::get_sponsor_count(env, bounty_id)` — this is the most natural API. If it doesn't exist, the other step will add it.

Actually, let me be more careful. The safest approach that doesn't require knowing the exact API: check if the sponsor is already in the list. If not, check the count. But I need a way to get the count.

Let me assume `storage::get_sponsor_count(env, bounty_id) -> u32` exists. This is a reasonable assumption given the issue.

Actually, let me reconsider. The issue #1218 says the cap is in `storage.rs`. The step says enforce in `sponsor_bounty`. So maybe `add_sponsor_contribution` returns something, or there's a count function.

I'll use `storage::get_sponsor_count(&env, &bounty_id)` and assume it returns `u32`. And I'll check if the sponsor is new by checking if they're in the existing list.

Hmm, but I don't have `get_sponsors`. Let me just check the count and if the sponsor is new. Actually, the cleanest way is:

```rust
let existing_sponsors = storage::get_sponsors(&env, &bounty_id);
let is_new_sponsor = !existing_sponsors.iter().any(|(addr, _)| addr == sponsor);
if is_new_sponsor && existing_sponsors.len() >= storage::MAX_SPONSORS_PER_BOUNTY {
    fail(ContractError::TooManySponsors);
}
```

But I don't know if `get_sponsors` exists. Let me use `get_sponsor_count` which is more likely to be added.

Actually, you know what, let me just make reasonable assumptions and write the code. The other steps will add the storage functions. I'll use:
- `storage::get_sponsor_count(&env, &bounty_id)` → returns u32
- `storage::is_sponsor(&env, &bounty_id, &sponsor)` → returns bool

Hmm, that's two functions. Let me simplify. I'll use `storage::get_sponsors` which returns the list, and check both count and membership:

Actually, the simplest and most likely to exist: `storage::get_sponsor_count`. And for membership, I can check if the sponsor is already there by... hmm.

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

    // #1218: enforce a minimum sponsorship floor to prevent dust griefing.
    if amount < storage::MIN_SPONSOR_AMOUNT {
        fail(ContractError::SponsorAmountBelowMinimum);
    }

    // Co-funding is only meaningful while the bounty can still pay out:
    // reject terminal statuses and disputes.
    ensure_status(
        &bounty,
        &[
            Symbol::new(&env, STATUS_OPEN),
            Symbol::new(&env, STATUS_IN_PROGRESS),
            Symbol::new(&env, STATUS_SUBMITTED),
        ],
        ContractError::BountyNotOpen,
    );

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

    bounty.reward_amount += amount;
    storage::store_bounty(&env, &bounty_id, &bounty);

    events::emit_bounty_sponsored(&env, &bounty_id, &sponsor, &amount, &bounty.reward_amount);
}
```

Now for `reopen_bounty`:

```rust
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
        if let Some(milestone_deadline) = m.deadline {
            if milestone_deadline < now || milestone_deadline > new_deadline {
                fail(ContractError::MilestoneDeadlineExpired);
            }
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

Actually, for `complete_bounty_inner` non-milestone path, `distribute_payout` is called which decrements active_claims. Then I need to add `clear_active_bounty_for_assignees` after. Let me add it right before `events::emit_bounty_completed`.

For the milestone path in `complete_bounty_inner`, add `clear_active_bounty_for_assignees` before `events::emit_bounty_completed`.

For `apply_dispute_resolution` complete branch:
- Milestone path: add `clear_active_bounty_for_assignees` after status change
- Non-milestone path: add `clear_active_bounty_for_assignees` after the loop (or inside the loop, but the loop already decrements active_claims, so let me add it after the loop)

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

/// Validate a tag list: reject more than `MAX_TAGS` entries, reject
/// duplicates, and reject any tag not on the allow-list (via
/// `symbols::validate_symbol_or_fail`).
fn validate_tags(env: &Env, tags: &Vec<Symbol>) {
    if tags.len() > MAX_TAGS {
        fail(ContractError::TooManyTags);
    }

    for (i, tag) in tags.iter().enumerate() {
        symbols::validate_symbol_or_fail