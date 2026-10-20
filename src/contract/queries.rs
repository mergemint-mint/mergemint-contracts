/// Maximum items returnable in a single paginated call.
///
/// Kept equal to `storage::PAGE_SIZE` so that a single "next page" call never
/// needs to read more than two storage pages.
const MAX_LIMIT: u32 = 50;

/// The deployed contract version, as a semver string.
///
/// This is the single source of truth for the contract version. Bump it here
/// (and keep `docs/versioning.md` in sync) whenever behavior changes between
/// releases so clients can detect mismatches via `version()`.
const CONTRACT_VERSION: &str = "1.0.0";

/// Resolve a `cursor` + `limit` window into a flat slice of IDs.
///
/// `all_ids` is the full ordered list (collected across pages when needed).
/// Returns `(items, next_cursor)` where `next_cursor` is `Some(offset)` of
/// the next item after the returned window, or `None` when the list is
/// exhausted.
fn paginate(
    env: &Env,
    all_ids: Vec<BountyId>,
    cursor: Option<u32>,
    limit: u32,
) -> (Vec<BountyId>, Option<u32>) {
    let effective_limit = if limit == 0 || limit > MAX_LIMIT {
        MAX_LIMIT
    } else {
        limit
    };
    let start = cursor.unwrap_or(0);
    let total = all_ids.len();
    let mut result = Vec::new(env);
    if start >= total {
        return (result, None);
    }
    let end = {
        let e = start + effective_limit;
        if e > total {
            total
        } else {
            e
        }
    };
    let mut i = start;
    while i < end {
        result.push_back(all_ids.get(i).unwrap());
        i += 1;
    }
    let next = if end < total { Some(end) } else { None };
    (result, next)
}

#[contractimpl]
impl MergeMintContract {
    /// Return the deployed contract version as a semver string.
    ///
    /// Sourced from the single `CONTRACT_VERSION` constant so the value stays
    /// in sync with `docs/versioning.md`. Cheap to call; lets the frontend,
    /// SDK and indexer warn when pointed at an unexpected contract version.
    pub fn version(env: Env) -> String {
        // `String`, not `Symbol`: Soroban symbols reject the `.` in semver.
        String::from_str(&env, CONTRACT_VERSION)
    }

    /// Return a single bounty by its ID, or `None` if it does not exist.
    ///
    /// If `bounty_id` has been archived (see `archive_bounty`), the full
    /// record no longer exists; this falls back to the compacted
    /// `BountySummary` and reconstructs a degenerate `Bounty` (empty tags,
    /// milestones, verifiers; a single assignee slot if the summary has
    /// one) so existing callers keep getting a `Bounty` rather than `None`.
    pub fn get_bounty(env: Env, bounty_id: BountyId) -> Option<Bounty> {
        // Never-allocated IDs (sequence >= count) and pruned entries (sequence
        // < count but missing from storage) both return None without panicking.
        if !storage::bounty_id_was_allocated(&env, &bounty_id) {
            return None;
        }
        if let Some(bounty) = storage::get_bounty(&env, &bounty_id) {
            return Some(bounty);
        }
        storage::get_bounty_archive(&env, &bounty_id).map(|summary| {
            let assignees = match summary.assignee {
                Some(address) => {
                    let mut v = Vec::new(&env);
                    v.push_back((address, 10_000u32));
                    v
                }
                None => Vec::new(&env),
            };
            Bounty {
                creator: summary.creator,
                reward_amount: summary.reward,
                // The original reward_token is not retained in the summary;
                // this placeholder (the contract's own address) is never a
                // real payout destination and must not be used for
                // transfers. Callers that need the real token should use
                // off-chain indexer history instead.
                reward_token: env.current_contract_address(),
                assignees,
                max_assignees: 1,
                status: summary.status,
                min_reputation: 0,
                deadline: None,
                required_verifiers: None,
                approval_threshold: 0,
                tags: Vec::new(&env),
                milestones: Vec::new(&env),
            }
        })
    }

    /// Return the archived summary for `bounty_id`, or `None` if it has not
    /// been archived. Lighter weight than `get_bounty` for history/listing
    /// UIs that only need id/creator/assignee/status/reward.
    pub fn get_bounty_summary(env: Env, bounty_id: BountyId) -> Option<BountySummary> {
        storage::get_bounty_archive(&env, &bounty_id)
    }

    /// Return the off-chain-facing metadata (title, description) for each of
    /// `ids`, in the same order. Entries are `None` for IDs with no stored meta.
    pub fn get_bounty_metas(env: Env, ids: Vec<BountyId>) -> Vec<Option<BountyMeta>> {
        let mut result: Vec<Option<BountyMeta>> = Vec::new(&env);
        for id in ids.iter() {
            result.push_back(storage::get_bounty_meta(&env, &id));
        }
        result
    }

    /// Return `BountyMeta` for each of `ids`, in the same order, for use by
    /// list pages that would otherwise call `get_bounty` once per card.
    ///
    /// Unknown IDs are skipped (not included in the result, not an error).
    /// The number of IDs processed per call is capped at `MAX_LIMIT` (50);
    /// any IDs beyond the cap are ignored.
    pub fn get_bounties_by_ids(env: Env, ids: Vec<BountyId>) -> Vec<BountyMeta> {
        let mut result: Vec<BountyMeta> = Vec::new(&env);
        let mut i = 0u32;
        for id in ids.iter() {
            if i >= MAX_LIMIT {
                break;
            }
            if let Some(meta) = storage::get_bounty_meta(&env, &id) {
                result.push_back(meta);
            }
            i += 1;
        }
        result
    }

    /// Return a contributor's profile (reputation, earnings, active claims),
    /// or `None` if `address` has never interacted with the contract.
    pub fn get_contributor(env: Env, address: Address) -> Option<Contributor> {
        storage::get_contributor(&env, &address)
    }

    /// Return the total number of bounties ever created (monotonic counter).
    ///
    /// This is the primary metric exposed by `GET /api/bounties/count` in the
    /// REST layer. It reads a single persistent u64 counter and is cheap to
    /// call even when thousands of bounties exist.
    pub fn get_bounty_count(env: Env) -> u64 {
        storage::get_bounty_count(&env)
    }

    /// Return each bounty in `ids`, in the same order. Entries are `None`
    /// for IDs that do not exist.
    pub fn get_bounties(env: Env, ids: Vec<BountyId>) -> Vec<Option<Bounty>> {
        let mut result = Vec::new(&env);
        for id in ids.iter() {
            result.push_back(storage::get_bounty(&env, &id));
        }
        result
    }

    /// Return a bounded page of bounty IDs for a given status.
    ///
    /// `cursor` is the zero-based offset of the first item to return (use
    /// the `next_cursor` from the previous response to advance pages).
    /// `limit` is capped at `MAX_LIMIT` (50). Returns `(items, next_cursor)`
    /// where `next_cursor` is `None` when the list is exhausted.
    ///
    /// Example: `get_bounties_by_status(env, sym, None, 20)` → first 20.
    pub fn get_bounties_by_status(
        env: Env,
        status: Symbol,
        cursor: Option<u32>,
        limit: u32,
    ) -> (Vec<BountyId>, Option<u32>) {
        crate::symbols::validate_symbol_or_fail(&env, crate::symbols::SymbolKind::Status, &status);
        let all = storage::get_bounties_by_status(&env, &status);
        paginate(&env, all, cursor, limit)
    }

    /// Return the total number of bounties currently in `status`.
    pub fn get_status_count(env: Env, status: Symbol) -> u32 {
        crate::symbols::validate_symbol_or_fail(&env, crate::symbols::SymbolKind::Status, &status);
        storage::get_status_count(&env, &status)
    }

    /// Return a bounded page of open bounty IDs.
    ///
    /// `cursor` is the zero-based offset of the first item; `limit` capped at 50.
    /// Returns `(items, next_cursor)` — pass `next_cursor` as `cursor` in the
    /// next call to advance pages. `next_cursor` is `None` when exhausted.
    pub fn get_open_bounties(
        env: Env,
        cursor: Option<u32>,
        limit: u32,
    ) -> (Vec<BountyId>, Option<u32>) {
        let all = storage::get_open_bounties(&env);
        paginate(&env, all, cursor, limit)
    }

    /// Return the total number of currently-open bounties.
    pub fn get_open_bounties_count(env: Env) -> u32 {
        storage::get_open_bounties_count(&env)
    }

    /// Return a page of open bounty IDs — legacy offset/limit variant.
    ///
    /// Kept for backward compatibility. Prefer `get_open_bounties` (cursor-based).
    /// `offset` is zero-based; `limit` is capped at 50 to bound ledger CPU cost.
    /// Returns an empty vec when `offset` is beyond the end of the list.
    pub fn get_open_bounties_paged(env: Env, offset: u32, limit: u32) -> Vec<BountyId> {
        let (items, _) = Self::get_open_bounties(env, Some(offset), limit);
        items
    }

    /// Return all open bounty IDs that carry the requested tag.
    ///
    /// Supports `GET /api/bounties?tag=<tag>`. Iterates the open-bounties index
    /// and looks up each bounty to check `bounty.tags`; callers can page the
    /// result with `get_open_bounties` first and apply filtering client-side
    /// for large lists.
    pub fn get_bounties_by_tag(env: Env, tag: Symbol) -> Vec<BountyId> {
        crate::symbols::validate_symbol_or_fail(&env, crate::symbols::SymbolKind::Tag, &tag);
        let open_ids = storage::get_open_bounties(&env);
        let mut result = Vec::new(&env);
        for id in open_ids.iter() {
            if let Some(bounty) = storage::get_bounty(&env, &id) {
                for t in bounty.tags.iter() {
                    if t == tag {
                        result.push_back(id.clone());
                        break;
                    }
                }
            }
        }
        result
    }

    /// Return the bounty ID of the in-progress bounty assigned to `address`, if any.
    ///
    /// Supports `GET /api/contributors/{address}/active-bounty`. Scans open and
    /// in-progress bounties for an assignee slot matching `address`. Returns `None`
    /// when the contributor has no active claim.
    pub fn get_contributor_active_bounty(env: Env, address: Address) -> Option<BountyId> {
        let in_progress_sym = Symbol::new(&env, "in_progress");
        let in_progress_ids = storage::get_bounties_by_status(&env, &in_progress_sym);
        for id in in_progress_ids.iter() {
            if let Some(bounty) = storage::get_bounty(&env, &id) {
                for (assignee, _weight) in bounty.assignees.iter() {
                    if assignee == address {
                        return Some(id);
                    }
                }
            }
        }
        None
    }

    /// Return a bounded page of bounty IDs `address` was an assignee on that
    /// have reached a terminal status (`"completed"` or `"cancelled"`).
    ///
    /// Unlike `get_contributor_active_bounty` (which only surfaces the
    /// current in-progress claim), this surfaces the contributor's bounty
    /// history. The index is maintained incrementally in
    /// `storage::move_bounty_status` as bounties transition status.
    ///
    /// Results are returned newest first (most recently completed/cancelled
    /// bounty at index 0). `offset` skips that many entries from the newest
    /// end; `limit` is capped at 50 (`MAX_LIMIT`), and `0` means the cap.
    /// Returns an empty `Vec` if the contributor has no completed or
    /// cancelled bounties, or if `offset` is past the end of the history.
    pub fn get_contributor_bounty_history(
        env: Env,
        address: Address,
        offset: u32,
        limit: u32,
    ) -> Vec<BountyId> {
        let all = storage::get_contributor_history(&env, &address);
        let mut newest_first = Vec::new(&env);
        let mut i = all.len();
        while i > 0 {
            i -= 1;
            newest_first.push_back(all.get(i).unwrap());
        }
        paginate(&env, newest_first, Some(offset), limit).0
    }

    /// Return the submission hash recorded by `submit_work` for `bounty_id`,
    /// or `None` if no work has been submitted yet.
    pub fn get_submission(env: Env, bounty_id: BountyId) -> Option<BytesN<32>> {
        storage::get_submission(&env, &bounty_id)
    }

    /// Return the list of `(sponsor_address, contributed_amount)` pairs for a
    /// bounty, as tracked by `sponsor_bounty`. Empty if nobody has sponsored it.
    pub fn get_bounty_sponsors(env: Env, bounty_id: BountyId) -> Vec<(Address, i128)> {
        storage::get_sponsors(&env, &bounty_id)
    }

    /// Return a bounded page of bounty IDs created by a specific creator address.
    ///
    /// `cursor` is the zero-based offset; `limit` capped at 50.
    /// Returns `(items, next_cursor)`. Pass `next_cursor` as `cursor` on the
    /// next call to advance pages. `next_cursor` is `None` when exhausted.
    ///
    /// The list is maintained in `DataKey::ContributorBounties(creator)` and
    /// appended to on each `create_bounty` call. Returns an empty `Vec` if the
    /// address has never created a bounty.
    /// Return a lightweight view of each milestone on `bounty_id`, in order.
    ///
    /// Avoids decoding the full `Bounty` (and its `Vec<(Address, u32)>`
    /// assignee list) just to render milestone progress. Each view carries
    /// the milestone's index, reward amount, completion flag and optional
    /// per-milestone deadline. Returns an empty `Vec` for a bounty with no
    /// milestones configured (i.e. an all-or-nothing bounty).
    ///
    /// # Panics
    /// * If `bounty_id` does not exist (`ContractError::BountyNotFound`).
    pub fn get_milestones(env: Env, bounty_id: BountyId) -> Vec<crate::types::MilestoneView> {
        let bounty = match storage::get_bounty(&env, &bounty_id) {
            Some(b) => b,
            None => crate::errors::fail(crate::errors::ContractError::BountyNotFound),
        };

        let mut result = Vec::new(&env);
        for (idx, milestone) in bounty.milestones.iter().enumerate() {
            result.push_back(crate::types::MilestoneView {
                index: idx as u32,
                reward: milestone.reward,
                completed: milestone.completed,
                deadline: milestone.deadline,
            });
        }
        result
    }

    /// Whether the admin has paused the contract (see `pause`).
    pub fn is_paused(env: Env) -> bool {
        storage::is_paused(&env)
    }

    /// Return the address currently configured as contract admin, if any.
    pub fn get_admin(env: Env) -> Option<Address> {
        storage::get_admin(&env)
    }

    /// Return `true` if `token` is on the admin-managed reward-token allowlist.
    pub fn is_reward_token_allowed(env: Env, token: Address) -> bool {
        storage::is_reward_token_allowed(&env, &token)
    }

    /// Return the current claim window (in ledgers) used by `release_stale_claim`.
    pub fn get_claim_window(env: Env) -> u32 {
        storage::get_claim_window(&env)
    }

    pub fn get_bounties_by_creator(
        env: Env,
        creator: Address,
        cursor: Option<u32>,
        limit: u32,
    ) -> (Vec<BountyId>, Option<u32>) {
        let all = storage::get_creator_bounties(&env, &creator);
        paginate(&env, all, cursor, limit)
    }

    /// Return a bounded page of bounty IDs paid in a specific reward token.
    ///
    /// `cursor` is the zero-based offset; `limit` capped at `MAX_LIMIT` (50),
    /// same cursor/limit pattern as `get_bounties_by_creator` and
    /// `get_open_bounties`. Returns `(items, next_cursor)`; `next_cursor` is
    /// `None` when exhausted.
    ///
    /// The index is maintained in `DataKey::TokenBounties(token)`: appended
    /// to on `create_bounty`, and pruned on `archive_bounty` once a bounty's
    /// full record is removed. Returns an empty `Vec` for a token that has
    /// never been used as a `reward_token` (an "unknown" token is simply one
    /// with no entries — there is nothing to validate against, since any
    /// Soroban address is a syntactically valid token argument).
    pub fn get_bounties_by_token(
        env: Env,
        token: Address,
        cursor: Option<u32>,
        limit: u32,
    ) -> (Vec<BountyId>, Option<u32>) {
        let all = storage::get_token_bounties(&env, &token);
        paginate(&env, all, cursor, limit)
    }

    /// Return the admin-configured cap on concurrent open+claimed bounties
    /// per creator. `0` means unlimited (the default).
    pub fn get_creator_open_limit(env: Env) -> u32 {
        storage::get_creator_open_limit(&env)
    }

    /// Return the current count of open+claimed bounties for `creator`.
    pub fn get_creator_open_count(env: Env, creator: Address) -> u32 {
        storage::get_creator_open_count(&env, &creator)
    }

    /// Return the admin-configured claim cooldown, in ledgers. `0` means the
    /// cooldown is disabled (the default).
    pub fn get_claim_cooldown(env: Env) -> u32 {
        storage::get_claim_cooldown(&env)
    }

    /// Return a snapshot of every contract-wide admin setting in one call.
    pub fn get_config(env: Env) -> ContractConfig {
        ContractConfig {
            admin: storage::get_admin(&env),
            pending_admin: storage::get_pending_admin(&env),
            creator_open_limit: storage::get_creator_open_limit(&env),
            claim_cooldown: storage::get_claim_cooldown(&env),
            claim_window: storage::get_claim_window(&env),
            reputation_penalty: storage::get_reputation_penalty(&env),
            appeal_window: storage::get_appeal_window(&env),
        }
    }

    /// Return the admin-configured minimum reward for `token` (`0` if unset;
    /// the global `MIN_REWARD_AMOUNT` always applies as well).
    pub fn get_min_reward(env: Env, token: Address) -> i128 {
        storage::get_min_reward(&env, &token)
    }

    /// Return the address proposed to take over `bounty_id`, if a
    /// `transfer_bounty_ownership` is pending.
    pub fn get_pending_owner(env: Env, bounty_id: BountyId) -> Option<Address> {
        storage::get_pending_owner(&env, &bounty_id)
    }

    /// Return the evidence hashes attached to `bounty_id`'s dispute, if any.
    pub fn get_dispute_evidence(env: Env, bounty_id: BountyId) -> Vec<soroban_sdk::BytesN<32>> {
        storage::get_dispute_evidence(&env, &bounty_id)
    }

    /// Return the admin-configured dispute appeal window, in ledgers. `0`
    /// means decisions are paid out immediately (the default).
    pub fn get_appeal_window(env: Env) -> u32 {
        storage::get_appeal_window(&env)
    }

    /// Return the pending (undecided-until-finalized) dispute resolution for
    /// `bounty_id`, if one has been recorded.
    pub fn get_pending_resolution(env: Env, bounty_id: BountyId) -> Option<PendingResolution> {
        storage::get_pending_resolution(&env, &bounty_id)
    }

    /// Return whether `bounty_id`'s dispute has already used its one
    /// allowed appeal.
    pub fn get_dispute_appealed(env: Env, bounty_id: BountyId) -> bool {
        storage::get_dispute_appealed(&env, &bounty_id)
    }
}
