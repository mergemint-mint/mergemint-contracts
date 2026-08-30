/// Maximum items returnable in a single paginated call.
///
/// Kept equal to `storage::PAGE_SIZE` so that a single "next page" call never
/// needs to read more than two storage pages.
const MAX_LIMIT: u32 = 50;

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
    /// Return a single bounty by its ID, or `None` if it does not exist.
    pub fn get_bounty(env: Env, bounty_id: BountyId) -> Option<Bounty> {
        // Never-allocated IDs (sequence >= count) and pruned entries (sequence
        // < count but missing from storage) both return None without panicking.
        if !storage::bounty_id_was_allocated(&env, &bounty_id) {
            return None;
        }
        storage::get_bounty(&env, &bounty_id)
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

    /// Return every bounty ID `address` was an assignee on that has reached a
    /// terminal status (`"completed"` or `"cancelled"`).
    ///
    /// Unlike `get_contributor_active_bounty` (which only surfaces the
    /// current in-progress claim), this surfaces the contributor's full
    /// bounty history. The index is maintained incrementally in
    /// `storage::move_bounty_status` as bounties transition status, so this
    /// call is O(1) rather than a scan. Returns an empty `Vec` if the
    /// contributor has no completed or cancelled bounties.
    pub fn get_contributor_bounty_history(env: Env, address: Address) -> Vec<BountyId> {
        storage::get_contributor_history(&env, &address)
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
    pub fn get_bounties_by_creator(
        env: Env,
        creator: Address,
        cursor: Option<u32>,
        limit: u32,
    ) -> (Vec<BountyId>, Option<u32>) {
        let all = storage::get_creator_bounties(&env, &creator);
        paginate(&env, all, cursor, limit)
    }
}
