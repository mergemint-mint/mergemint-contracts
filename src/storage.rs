// SPDX-License-Identifier: MIT
//!
//! Persistent storage accessors for the MergeMint contract.
//!
//! # Key namespacing
//!
//! Every ledger entry is addressed by a typed [`DataKey`] variant defined in
//! `crate::types`. Soroban serialises each enum variant to a distinct XDR
//! discriminant, so **new data must be added as a new `DataKey` variant** —
//! never by overloading an existing variant or inventing ad-hoc byte prefixes.
//!
//! All functions in this module use **persistent** storage and call [`extend`]
//! after reads/writes to keep TTL fresh (~1 year; see constants below).
//!
//! # Key families
//!
//! | Family | `DataKey` variant | Value type | Role |
//! |--------|-------------------|------------|------|
//! | Global counter | `BountyCount` | `u64` | Monotonic bounty ID sequence |
//! | Bounty body | `Bounty(BountyId)` | `Bounty` | Canonical bounty state |
//! | Bounty metadata | `BountyMeta(BountyId)` | `BountyMeta` | Title/description sidecar |
//! | Contributor profile | `Contributor(Address)` | `Contributor` | Reputation, earnings, claims |
//! | Creator index | `ContributorBounties(Address)` | `Vec<BountyId>` | Bounties created by address |
//! | Status totals | `StatusCount(Symbol)` | `u32` | Item count per status label |
//! | Status index page | `StatusIndexPage(Symbol, u32)` | `Vec<BountyId>` | Paged shard (`PAGE_SIZE` IDs) |
//! | Open-bounty totals | `OpenBountiesCount` | `u32` | Count of claimable bounties |
//! | Open-bounty page | `OpenBountiesPage(u32)` | `Vec<BountyId>` | Paged open-bounty shard |
//! | Contributor history totals | `ContributorHistoryCount(Address)` | `u32` | Terminal-bounty count per contributor |
//! | Contributor history page | `ContributorHistoryPage(Address, u32)` | `Vec<BountyId>` | Paged history shard |
//! | Multi-sig votes | `Approvals(BountyId)` | `Vec<Address>` | Per-bounty verifier approvals |
//!
//! ## Legacy variants (read-only)
//!
//! - `StatusIndex(Symbol)` — pre-pagination status blob; do not write.
//! - `OpenBounties` — pre-pagination open list; do not write.
//! - `ContributorHistory(Address)` — pre-pagination history blob; do not write.
//!
//! ## Paged indexes
//!
//! `StatusIndexPage`, `OpenBountiesPage` and `ContributorHistoryPage` shard
//! large lists into pages of at most [`PAGE_SIZE`] entries to stay within
//! Soroban's per-entry size limit. Each index also maintains a companion count
//! key (`StatusCount` / `OpenBountiesCount` / `ContributorHistoryCount`). See
//! the inline layout comments in this file for append and swap-remove
//! semantics.
use soroban_sdk::{Address, Env, Symbol, Vec};

use crate::errors::{fail, ContractError};
use crate::types::{
    Bounty, BountyId, BountyMeta, BountySummary, Contributor, DataKey, PendingResolution,
};

/// Terminal statuses after which a bounty is recorded in a contributor's
/// history index. Kept in sync with the status literals in `contract/mutations.rs`.
const STATUS_COMPLETED: &str = "completed";
const STATUS_CANCELLED: &str = "cancelled";

/// Approximate 1 year in ledger sequences at 5 seconds per ledger.
const STORAGE_TTL_LEDGERS: u32 = 6_307_200;
/// Extend TTL when remaining life falls below half a year.
const STORAGE_TTL_THRESHOLD: u32 = STORAGE_TTL_LEDGERS / 2;

/// Maximum number of `BountyId` entries stored in a single index page.
///
/// Soroban's per-entry size limit is ~64 KB of XDR. A `BountyId` serialises to
/// 32 bytes; 50 entries ≈ 1.6 KB — well within the limit and safely below the
/// per-invocation resource budget even when several pages are read in one call.
/// Changing this constant is a breaking storage migration; see docs/architecture.md.
pub const PAGE_SIZE: u32 = 50;

// ── TTL helper ────────────────────────────────────────────────────────────────

fn extend<K: soroban_sdk::IntoVal<Env, soroban_sdk::Val>>(env: &Env, key: &K) {
    env.storage()
        .persistent()
        .extend_ttl(key, STORAGE_TTL_THRESHOLD, STORAGE_TTL_LEDGERS);
}

// ── BountyCount ───────────────────────────────────────────────────────────────

/// Returns the total number of bounties ever created.
///
/// Storage key: `DataKey::BountyCount`
/// Stored type: `u64`
/// Default value: `0` (returns 0 if no bounties have been created)
///
/// This is a singleton counter that monotonically increases with each
/// `create_bounty` call. The value is used as part of the bounty ID
/// generation to produce deterministic, unique identifiers.
pub fn get_bounty_count(env: &Env) -> u64 {
    let key = DataKey::BountyCount;
    let count: Option<u64> = env.storage().persistent().get(&key);
    if count.is_some() {
        extend(env, &key);
    }
    count.unwrap_or(0)
}

pub fn set_bounty_count(env: &Env, count: &u64) {
    let key = DataKey::BountyCount;
    env.storage().persistent().set(&key, count);
    extend(env, &key);
}

/// Decode the monotonic creation sequence embedded in a `BountyId`.
///
/// IDs are minted by `generate_bounty_id`, which writes the current
/// `get_bounty_count()` into the last eight bytes (big-endian).
pub fn bounty_id_sequence(id: &BountyId) -> u64 {
    let mut seq_bytes = [0u8; 8];
    for i in 0u32..8 {
        seq_bytes[i as usize] = id.0.get(24 + i).unwrap_or(0);
    }
    u64::from_be_bytes(seq_bytes)
}

/// Returns `true` when `id`'s embedded sequence is strictly less than the
/// number of bounties ever created (i.e. the ID was allocated at some point).
pub fn bounty_id_was_allocated(env: &Env, id: &BountyId) -> bool {
    bounty_id_sequence(id) < get_bounty_count(env)
}

// ── Bounty ────────────────────────────────────────────────────────────────────

/// Persists a `Bounty` struct under its unique 32-byte identifier.
///
/// Storage key: `DataKey::Bounty(id)`
/// Stored type: `Bounty`
/// Side effects: Overwrites any existing entry for the same `id`.
///
/// This is called during `create_bounty` (initial store) and
/// `claim_bounty` (when the assignee and status are updated).
pub fn store_bounty(env: &Env, id: &BountyId, bounty: &Bounty) {
    let key = DataKey::Bounty(id.clone());
    env.storage().persistent().set(&key, bounty);
    extend(env, &key);
}

/// Retrieves a `Bounty` by its 32-byte identifier, if it exists.
///
/// Storage key: `DataKey::Bounty(id)`
/// Stored type: `Bounty`
/// Returns: `Some(Bounty)` if found, `None` if no bounty exists for `id`.
///
/// Callers should handle the `None` case (e.g., by panicking with a
/// descriptive message as done in `claim_bounty` and `complete_bounty`).
pub fn get_bounty(env: &Env, id: &BountyId) -> Option<Bounty> {
    let key = DataKey::Bounty(id.clone());
    let bounty: Option<Bounty> = env.storage().persistent().get(&key);
    if bounty.is_some() {
        extend(env, &key);
    }
    bounty
}

/// Remove a bounty's full body and metadata from storage.
///
/// Called by `archive_bounty` once a `BountySummary` has been written, so
/// the full `Bounty`/`BountyMeta` entries stop accruing TTL-extension rent.
pub fn remove_bounty_body(env: &Env, id: &BountyId) {
    env.storage()
        .persistent()
        .remove(&DataKey::Bounty(id.clone()));
    env.storage()
        .persistent()
        .remove(&DataKey::BountyMeta(id.clone()));
}

/// Persist a compacted `BountySummary` for a finished bounty.
///
/// Storage key: `DataKey::BountyArchive(id)`.
pub fn store_bounty_archive(env: &Env, id: &BountyId, summary: &BountySummary) {
    let key = DataKey::BountyArchive(id.clone());
    env.storage().persistent().set(&key, summary);
    extend(env, &key);
}

/// Retrieve a bounty's archived summary, if it has been archived.
pub fn get_bounty_archive(env: &Env, id: &BountyId) -> Option<BountySummary> {
    let key = DataKey::BountyArchive(id.clone());
    let summary: Option<BountySummary> = env.storage().persistent().get(&key);
    if summary.is_some() {
        extend(env, &key);
    }
    summary
}

/// Read the ledger sequence after which `id` becomes archivable, if a
/// grace-period marker has been set by a prior `archive_bounty` attempt.
pub fn get_archive_eligible_at(env: &Env, id: &BountyId) -> Option<u32> {
    let key = DataKey::ArchiveEligibleAt(id.clone());
    let value: Option<u32> = env.storage().persistent().get(&key);
    if value.is_some() {
        extend(env, &key);
    }
    value
}

/// Set the ledger sequence after which `id` becomes archivable.
pub fn set_archive_eligible_at(env: &Env, id: &BountyId, eligible_at: u32) {
    let key = DataKey::ArchiveEligibleAt(id.clone());
    env.storage().persistent().set(&key, &eligible_at);
    extend(env, &key);
}

/// Remove the grace-period marker once a bounty has been archived.
pub fn remove_archive_eligible_at(env: &Env, id: &BountyId) {
    env.storage()
        .persistent()
        .remove(&DataKey::ArchiveEligibleAt(id.clone()));
}

pub fn store_bounty_meta(env: &Env, id: &BountyId, meta: &BountyMeta) {
    let key = DataKey::BountyMeta(id.clone());
    env.storage().persistent().set(&key, meta);
    extend(env, &key);
}

pub fn get_bounty_meta(env: &Env, id: &BountyId) -> Option<BountyMeta> {
    let key = DataKey::BountyMeta(id.clone());
    let meta: Option<BountyMeta> = env.storage().persistent().get(&key);
    if meta.is_some() {
        extend(env, &key);
    }
    meta
}

// ── Contributor ───────────────────────────────────────────────────────────────

pub fn store_contributor(env: &Env, address: &Address, contributor: &Contributor) {
    let key = DataKey::Contributor(address.clone());
    env.storage().persistent().set(&key, contributor);
    extend(env, &key);
}

pub fn get_contributor(env: &Env, address: &Address) -> Option<Contributor> {
    let key = DataKey::Contributor(address.clone());
    let contributor: Option<Contributor> = env.storage().persistent().get(&key);
    if contributor.is_some() {
        extend(env, &key);
    }
    contributor
}

// ── StatusIndex — paged ───────────────────────────────────────────────────────
//
// The status index is sharded into pages of at most PAGE_SIZE entries.
// Layout:
//   DataKey::StatusCount(status)           → u32  (total items, not page count)
//   DataKey::StatusIndexPage(status, page) → Vec<BountyId>  (0-indexed page number)
//
// Adding an item appends to the last page, creating a new page when full.
// Removing an item uses a swap-remove from within the page to stay O(1) per
// page write; the last item of the last page replaces the removed slot, and
// the last page is deleted when it becomes empty.

fn status_page_count(env: &Env, status: &Symbol) -> u32 {
    let total = get_status_count(env, status);
    if total == 0 {
        0
    } else {
        total.div_ceil(PAGE_SIZE)
    }
}

fn get_status_page(env: &Env, status: &Symbol, page: u32) -> Vec<BountyId> {
    let key = DataKey::StatusIndexPage(status.clone(), page);
    let result: Option<Vec<BountyId>> = env.storage().persistent().get(&key);
    if result.is_some() {
        extend(env, &key);
    }
    result.unwrap_or_else(|| Vec::new(env))
}

fn set_status_page(env: &Env, status: &Symbol, page: u32, items: &Vec<BountyId>) {
    let key = DataKey::StatusIndexPage(status.clone(), page);
    if items.is_empty() {
        env.storage().persistent().remove(&key);
    } else {
        env.storage().persistent().set(&key, items);
        extend(env, &key);
    }
}

/// Returns the total item count for a status (not page count).
pub fn get_status_count(env: &Env, status: &Symbol) -> u32 {
    let key = DataKey::StatusCount(status.clone());
    let count: Option<u32> = env.storage().persistent().get(&key);
    if count.is_some() {
        extend(env, &key);
    }
    count.unwrap_or(0)
}

pub fn set_status_count(env: &Env, status: &Symbol, count: &u32) {
    let key = DataKey::StatusCount(status.clone());
    env.storage().persistent().set(&key, count);
    extend(env, &key);
}

fn increment_status_count(env: &Env, status: &Symbol) {
    let count = get_status_count(env, status);
    set_status_count(env, status, &(count + 1));
}

fn decrement_status_count(env: &Env, status: &Symbol) {
    let count = get_status_count(env, status);
    if count > 0 {
        set_status_count(env, status, &(count - 1));
    }
}

/// Returns a single page of bounty IDs for a status.
///
/// `page` is 0-indexed. Returns an empty `Vec` when `page >= page_count`.
pub fn get_bounties_by_status_page(env: &Env, status: &Symbol, page: u32) -> Vec<BountyId> {
    get_status_page(env, status, page)
}

/// Read only the page window `[start_page ..= end_page]` for a status index,
/// returning the concatenated IDs plus the exact `next_cursor`.
///
/// At most two contiguous pages are read regardless of the index size, so
/// paginated queries never hydrate the full status list.
pub fn get_bounties_by_status_window(
    env: &Env,
    status: &Symbol,
    cursor: u32,
    limit: u32,
) -> (Vec<BountyId>, Option<u32>) {
    let total = get_status_count(env, status);
    let mut items: Vec<BountyId> = Vec::new(env);
    if limit == 0 || cursor >= total {
        return (items, None);
    }
    let start_page = cursor / PAGE_SIZE;
    let end_offset = cursor.saturating_add(limit).min(total);
    let end_page = (end_offset - 1) / PAGE_SIZE;

    let mut p = start_page;
    while p <= end_page {
        let page = get_status_page(env, status, p);
        let page_start = p * PAGE_SIZE;
        let mut i = 0u32;
        while i < page.len() {
            let flat = page_start + i;
            if flat >= cursor && flat < end_offset {
                if let Some(id) = page.get(i) {
                    items.push_back(id);
                }
            }
            i += 1;
        }
        p += 1;
    }

    let next_cursor = if end_offset < total {
        Some(end_offset)
    } else {
        None
    };
    (items, next_cursor)
}

/// Compatibility helper: returns **all** IDs across all pages for a status.
///
/// For callers that still need the full list (e.g., small status sets in tests).
/// For large indexes, prefer `get_bounties_by_status_page`.
pub fn get_bounties_by_status(env: &Env, status: &Symbol) -> Vec<BountyId> {
    let total = get_status_count(env, status);
    let mut result = Vec::new(env);
    if total == 0 {
        return result;
    }
    let pages = total.div_ceil(PAGE_SIZE);
    let mut i = 0u32;
    while i < pages {
        let page = get_status_page(env, status, i);
        for id in page.iter() {
            result.push_back(id);
        }
        i += 1;
    }
    result
}

pub fn add_bounty_to_status(env: &Env, bounty_id: &BountyId, status: &Symbol) {
    // Deduplicate: scan last page only (appends always go to the last page).
    let total = get_status_count(env, status);
    if total > 0 {
        let last_page_idx = (total - 1) / PAGE_SIZE;
        let last_page = get_status_page(env, status, last_page_idx);
        for id in last_page.iter() {
            if id == *bounty_id {
                return; // already present, nothing to do
            }
        }
        // Also check all pages if total > PAGE_SIZE (full dedup guarantee).
        if total > PAGE_SIZE {
            let _page_count = total.div_ceil(PAGE_SIZE);
            let mut p = 0u32;
            while p < last_page_idx {
                let pg = get_status_page(env, status, p);
                for id in pg.iter() {
                    if id == *bounty_id {
                        return;
                    }
                }
                p += 1;
            }
        }
    }

    // Append to the last page (or create page 0).
    let last_page_idx = if total == 0 {
        0
    } else {
        (total - 1) / PAGE_SIZE
    };
    // If total is exactly divisible by PAGE_SIZE and > 0, we start a new page.
    let target_page_idx = if total > 0 && total.is_multiple_of(PAGE_SIZE) {
        total / PAGE_SIZE
    } else {
        last_page_idx
    };
    let mut page = get_status_page(env, status, target_page_idx);
    page.push_back(bounty_id.clone());
    set_status_page(env, status, target_page_idx, &page);
    increment_status_count(env, status);
}

pub fn remove_bounty_from_status(env: &Env, bounty_id: &BountyId, status: &Symbol) {
    let total = get_status_count(env, status);
    if total == 0 {
        return;
    }
    let page_count = total.div_ceil(PAGE_SIZE);

    // Find the target item across all pages.
    let mut found_page: Option<u32> = None;
    let mut found_pos: Option<u32> = None;
    let mut p = 0u32;
    'outer: while p < page_count {
        let page = get_status_page(env, status, p);
        for (pos, id) in page.iter().enumerate() {
            if id == *bounty_id {
                found_page = Some(p);
                found_pos = Some(pos as u32);
                break 'outer;
            }
        }
        p += 1;
    }

    let (fp, fpos) = match (found_page, found_pos) {
        (Some(a), Some(b)) => (a, b),
        _ => return, // not found
    };

    // Swap the target slot with the very last item across all pages, then
    // shrink the last page by one.  This is O(1) writes (at most 2 pages).
    let last_page_idx = page_count - 1;
    let last_page = get_status_page(env, status, last_page_idx);
    if last_page.is_empty() {
        fail(ContractError::CorruptedIndex);
    }
    let last_item = match last_page.get(last_page.len() - 1) {
        Some(item) => item,
        None => fail(ContractError::CorruptedIndex),
    };

    if fp == last_page_idx && fpos == last_page.len() - 1 {
        // Removing the very last item — just pop it.
        let new_last = last_page.clone();
        // Rebuild without the last element
        let mut trimmed: Vec<BountyId> = Vec::new(env);
        let mut i = 0u32;
        while i < new_last.len() - 1 {
            trimmed.push_back(
                new_last
                    .get(i)
                    .unwrap_or_else(|| fail(ContractError::CorruptedIndex)),
            );
            i += 1;
        }
        set_status_page(env, status, last_page_idx, &trimmed);
    } else {
        // Put last_item into the found slot, then shrink the last page.
        let target_page = get_status_page(env, status, fp);
        let mut new_target: Vec<BountyId> = Vec::new(env);
        let mut i = 0u32;
        while i < target_page.len() {
            if i == fpos {
                new_target.push_back(last_item.clone());
            } else {
                new_target.push_back(
                    target_page
                        .get(i)
                        .unwrap_or_else(|| fail(ContractError::CorruptedIndex)),
                );
            }
            i += 1;
        }
        set_status_page(env, status, fp, &new_target);

        // Shrink the last page. When the target sat on the last page itself,
        // shrink the freshly written copy, not the stale `last_page` read
        // from before the swap (which would resurrect the removed id).
        let source = if fp == last_page_idx {
            new_target
        } else {
            last_page
        };
        let mut trimmed: Vec<BountyId> = Vec::new(env);
        let mut i = 0u32;
        while i < source.len() - 1 {
            trimmed.push_back(
                source
                    .get(i)
                    .unwrap_or_else(|| fail(ContractError::CorruptedIndex)),
            );
            i += 1;
        }
        set_status_page(env, status, last_page_idx, &trimmed);
    }

    decrement_status_count(env, status);
}

/// Move `bounty_id` from `old_status` to `new_status` in the paged status
/// index, recording each assignee in their contributor history when the new
/// status is terminal.
///
/// `assignees` is passed in by the caller (which already holds the loaded
/// `Bounty`) so this function never re-reads `DataKey::Bounty(id)` — saving a
/// persistent deserialization on every completion and cancellation.
pub fn move_bounty_status(
    env: &Env,
    bounty_id: &BountyId,
    old_status: &Symbol,
    new_status: &Symbol,
    assignees: &Vec<(Address, u32)>,
) {
    if old_status != new_status {
        remove_bounty_from_status(env, bounty_id, old_status);
        add_bounty_to_status(env, bounty_id, new_status);

        if *new_status == Symbol::new(env, STATUS_COMPLETED)
            || *new_status == Symbol::new(env, STATUS_CANCELLED)
        {
            for (assignee, _) in assignees.iter() {
                append_contributor_history(env, &assignee, bounty_id);
            }
        }
    }
}

// ── OpenBounties — paged ──────────────────────────────────────────────────────
//
// Layout:
//   DataKey::OpenBountiesCount             → u32
//   DataKey::OpenBountiesPage(page)        → Vec<BountyId>
//
// Same swap-remove strategy as StatusIndex pages.

/// Returns the total number of open bounties.
pub fn get_open_bounties_count(env: &Env) -> u32 {
    let key = DataKey::OpenBountiesCount;
    let count: Option<u32> = env.storage().persistent().get(&key);
    if count.is_some() {
        extend(env, &key);
    }
    count.unwrap_or(0)
}

fn set_open_bounties_count(env: &Env, count: u32) {
    let key = DataKey::OpenBountiesCount;
    env.storage().persistent().set(&key, &count);
    extend(env, &key);
}

fn get_open_bounties_page(env: &Env, page: u32) -> Vec<BountyId> {
    let key = DataKey::OpenBountiesPage(page);
    let result: Option<Vec<BountyId>> = env.storage().persistent().get(&key);
    if result.is_some() {
        extend(env, &key);
    }
    result.unwrap_or_else(|| Vec::new(env))
}

fn set_open_bounties_page(env: &Env, page: u32, items: &Vec<BountyId>) {
    let key = DataKey::OpenBountiesPage(page);
    if items.is_empty() {
        env.storage().persistent().remove(&key);
    } else {
        env.storage().persistent().set(&key, items);
        extend(env, &key);
    }
}

/// Returns a single page of open bounty IDs. `page` is 0-indexed.
pub fn get_open_bounties_page_data(env: &Env, page: u32) -> Vec<BountyId> {
    get_open_bounties_page(env, page)
}

/// Read only the page window `[start_page ..= end_page]` for the open-bounty
/// index, returning the concatenated IDs plus the exact `next_cursor`.
///
/// `cursor` is a flat item offset; `limit` is the maximum number of IDs to
/// return. At most two contiguous pages are read regardless of how many
/// bounties are indexed, keeping ledger reads O(1) in the total index size.
/// Returns `(items, None)` when the window reaches the end of the index.
pub fn get_open_bounties_window(
    env: &Env,
    cursor: u32,
    limit: u32,
) -> (Vec<BountyId>, Option<u32>) {
    let total = get_open_bounties_count(env);
    let mut items: Vec<BountyId> = Vec::new(env);
    if limit == 0 || cursor >= total {
        return (items, None);
    }
    let start_page = cursor / PAGE_SIZE;
    let end_offset = cursor.saturating_add(limit).min(total);
    let end_page = (end_offset - 1) / PAGE_SIZE;

    let mut p = start_page;
    while p <= end_page {
        let page = get_open_bounties_page(env, p);
        let page_start = p * PAGE_SIZE;
        let mut i = 0u32;
        while i < page.len() {
            let flat = page_start + i;
            if flat >= cursor && flat < end_offset {
                if let Some(id) = page.get(i) {
                    items.push_back(id);
                }
            }
            i += 1;
        }
        p += 1;
    }

    let next_cursor = if end_offset < total {
        Some(end_offset)
    } else {
        None
    };
    (items, next_cursor)
}

/// Compatibility helper: returns **all** open bounty IDs across all pages.
pub fn get_open_bounties(env: &Env) -> Vec<BountyId> {
    let total = get_open_bounties_count(env);
    let mut result = Vec::new(env);
    if total == 0 {
        return result;
    }
    let page_count = total.div_ceil(PAGE_SIZE);
    let mut i = 0u32;
    while i < page_count {
        let page = get_open_bounties_page(env, i);
        for id in page.iter() {
            result.push_back(id);
        }
        i += 1;
    }
    result
}

pub fn add_open_bounty(env: &Env, bounty_id: &BountyId) {
    let total = get_open_bounties_count(env);

    // Deduplicate: a `BountyId` must never appear twice in the paged index,
    // otherwise `OpenBountiesCount` drifts above the sum of page lengths and
    // phantom entries survive removal. Mirror `add_bounty_to_status` and scan
    // every page before appending.
    if total > 0 {
        let page_count = total.div_ceil(PAGE_SIZE);
        let mut p = 0u32;
        while p < page_count {
            let page = get_open_bounties_page(env, p);
            for id in page.iter() {
                if id == *bounty_id {
                    return; // already present, nothing to do
                }
            }
            p += 1;
        }
    }

    let target_page_idx = if total == 0 {
        0
    } else if total.is_multiple_of(PAGE_SIZE) {
        total / PAGE_SIZE
    } else {
        total.div_ceil(PAGE_SIZE) - 1
    };
    let mut page = get_open_bounties_page(env, target_page_idx);
    page.push_back(bounty_id.clone());
    set_open_bounties_page(env, target_page_idx, &page);
    set_open_bounties_count(env, total + 1);
}

pub fn remove_open_bounty(env: &Env, bounty_id: &BountyId) {
    let total = get_open_bounties_count(env);
    if total == 0 {
        return;
    }
    let page_count = total.div_ceil(PAGE_SIZE);

    let mut found_page: Option<u32> = None;
    let mut found_pos: Option<u32> = None;
    let mut p = 0u32;
    'outer: while p < page_count {
        let page = get_open_bounties_page(env, p);
        for (pos, id) in page.iter().enumerate() {
            if id == *bounty_id {
                found_page = Some(p);
                found_pos = Some(pos as u32);
                break 'outer;
            }
        }
        p += 1;
    }

    let (fp, fpos) = match (found_page, found_pos) {
        (Some(a), Some(b)) => (a, b),
        _ => return,
    };

    let last_page_idx = page_count - 1;
    let last_page = get_open_bounties_page(env, last_page_idx);
    if last_page.is_empty() {
        fail(ContractError::CorruptedIndex);
    }
    let last_item = match last_page.get(last_page.len() - 1) {
        Some(item) => item,
        None => fail(ContractError::CorruptedIndex),
    };

    if fp == last_page_idx && fpos == last_page.len() - 1 {
        let mut trimmed: Vec<BountyId> = Vec::new(env);
        let mut i = 0u32;
        while i < last_page.len() - 1 {
            trimmed.push_back(
                last_page
                    .get(i)
                    .unwrap_or_else(|| fail(ContractError::CorruptedIndex)),
            );
            i += 1;
        }
        set_open_bounties_page(env, last_page_idx, &trimmed);
    } else {
        let target_page = get_open_bounties_page(env, fp);
        let mut new_target: Vec<BountyId> = Vec::new(env);
        let mut i = 0u32;
        while i < target_page.len() {
            if i == fpos {
                new_target.push_back(last_item.clone());
            } else {
                new_target.push_back(
                    target_page
                        .get(i)
                        .unwrap_or_else(|| fail(ContractError::CorruptedIndex)),
                );
            }
            i += 1;
        }
        set_open_bounties_page(env, fp, &new_target);

        // Shrink the freshly written copy when the target was on the last
        // page; the pre-swap `last_page` would resurrect the removed id.
        let source = if fp == last_page_idx {
            new_target
        } else {
            last_page
        };
        let mut trimmed: Vec<BountyId> = Vec::new(env);
        let mut i = 0u32;
        while i < source.len() - 1 {
            trimmed.push_back(
                source
                    .get(i)
                    .unwrap_or_else(|| fail(ContractError::CorruptedIndex)),
            );
            i += 1;
        }
        set_open_bounties_page(env, last_page_idx, &trimmed);
    }

    set_open_bounties_count(env, total - 1);
}

// ── Legacy index migration ────────────────────────────────────────────────────
//
// Pre-pagination deployments stored the full status index and open-bounty list
// in single blobs (`DataKey::StatusIndex(Symbol)` / `DataKey::OpenBounties`).
// `migrate_legacy_indexes` drains those blobs into the paged shards in bounded
// chunks so a single invocation stays within the transaction CPU/write budget.
//
// The migration is idempotent: IDs already present in the paged shards are
// skipped, and the legacy key is deleted only once every entry has been
// migrated. Re-running after completion is a no-op.

/// Maximum number of legacy IDs migrated per `migrate_legacy_indexes` call.
///
/// Each migrated ID costs at most one page read + one page write, so a chunk
/// of this size keeps the write footprint well below the per-transaction
/// budget even when both legacy indexes are drained in the same call.
pub const MIGRATION_CHUNK_SIZE: u32 = 25;

/// Returns the raw legacy `StatusIndex(status)` blob, if present.
pub fn get_legacy_status_index(env: &Env, status: &Symbol) -> Option<Vec<BountyId>> {
    env.storage()
        .persistent()
        .get(&DataKey::StatusIndex(status.clone()))
}

/// Returns the raw legacy `OpenBounties` blob, if present.
pub fn get_legacy_open_bounties(env: &Env) -> Option<Vec<BountyId>> {
    env.storage().persistent().get(&DataKey::OpenBounties)
}

/// Returns `true` when `bounty_id` already appears in any status page.
fn status_index_contains(env: &Env, status: &Symbol, bounty_id: &BountyId) -> bool {
    let total = get_status_count(env, status);
    if total == 0 {
        return false;
    }
    let pages = total.div_ceil(PAGE_SIZE);
    let mut p = 0u32;
    while p < pages {
        let page = get_status_page(env, status, p);
        for id in page.iter() {
            if id == *bounty_id {
                return true;
            }
        }
        p += 1;
    }
    false
}

/// Returns `true` when `bounty_id` already appears in any open-bounty page.
fn open_bounties_contains(env: &Env, bounty_id: &BountyId) -> bool {
    let total = get_open_bounties_count(env);
    if total == 0 {
        return false;
    }
    let pages = total.div_ceil(PAGE_SIZE);
    let mut p = 0u32;
    while p < pages {
        let page = get_open_bounties_page(env, p);
        for id in page.iter() {
            if id == *bounty_id {
                return true;
            }
        }
        p += 1;
    }
    false
}

/// Append `bounty_id` to the status shard, skipping duplicates.
fn append_status_index_unique(env: &Env, status: &Symbol, bounty_id: &BountyId) {
    if status_index_contains(env, status, bounty_id) {
        return;
    }
    let total = get_status_count(env, status);
    let target_page_idx = if total == 0 {
        0
    } else if total.is_multiple_of(PAGE_SIZE) {
        total / PAGE_SIZE
    } else {
        total.div_ceil(PAGE_SIZE) - 1
    };
    let mut page = get_status_page(env, status, target_page_idx);
    page.push_back(bounty_id.clone());
    set_status_page(env, status, target_page_idx, &page);
    increment_status_count(env, status);
}

/// Append `bounty_id` to the open-bounty shard, skipping duplicates.
fn append_open_bounty_unique(env: &Env, bounty_id: &BountyId) {
    if open_bounties_contains(env, bounty_id) {
        return;
    }
    let total = get_open_bounties_count(env);
    let target_page_idx = if total == 0 {
        0
    } else if total.is_multiple_of(PAGE_SIZE) {
        total / PAGE_SIZE
    } else {
        total.div_ceil(PAGE_SIZE) - 1
    };
    let mut page = get_open_bounties_page(env, target_page_idx);
    page.push_back(bounty_id.clone());
    set_open_bounties_page(env, target_page_idx, &page);
    set_open_bounties_count(env, total + 1);
}

/// Migrate up to `chunk_size` entries from the legacy `StatusIndex(status)`
/// blob into the paged shard. Returns the number of entries drained.
///
/// The legacy key is deleted once the blob is fully drained. Entries already
/// present in the paged shard are skipped (deduplication) but still count as
/// drained so the migration always makes forward progress.
pub fn migrate_legacy_status_index(env: &Env, status: &Symbol, chunk_size: u32) -> u32 {
    let legacy = match get_legacy_status_index(env, status) {
        Some(list) => list,
        None => return 0,
    };
    let len = legacy.len();
    if len == 0 {
        env.storage()
            .persistent()
            .remove(&DataKey::StatusIndex(status.clone()));
        return 0;
    }

    let limit = chunk_size.min(len);
    let mut i = 0u32;
    while i < limit {
        if let Some(id) = legacy.get(i) {
            append_status_index_unique(env, status, &id);
        }
        i += 1;
    }

    if limit >= len {
        env.storage()
            .persistent()
            .remove(&DataKey::StatusIndex(status.clone()));
    } else {
        let mut remaining: Vec<BountyId> = Vec::new(env);
        let mut j = limit;
        while j < len {
            if let Some(id) = legacy.get(j) {
                remaining.push_back(id);
            }
            j += 1;
        }
        let key = DataKey::StatusIndex(status.clone());
        env.storage().persistent().set(&key, &remaining);
        extend(env, &key);
    }
    limit
}

/// Migrate up to `chunk_size` entries from the legacy `OpenBounties` blob into
/// the paged shard. Returns the number of entries drained.
pub fn migrate_legacy_open_bounties(env: &Env, chunk_size: u32) -> u32 {
    let legacy = match get_legacy_open_bounties(env) {
        Some(list) => list,
        None => return 0,
    };
    let len = legacy.len();
    if len == 0 {
        env.storage().persistent().remove(&DataKey::OpenBounties);
        return 0;
    }

    let limit = chunk_size.min(len);
    let mut i = 0u32;
    while i < limit {
        if let Some(id) = legacy.get(i) {
            append_open_bounty_unique(env, &id);
        }
        i += 1;
    }

    if limit >= len {
        env.storage().persistent().remove(&DataKey::OpenBounties);
    } else {
        let mut remaining: Vec<BountyId> = Vec::new(env);
        let mut j = limit;
        while j < len {
            if let Some(id) = legacy.get(j) {
                remaining.push_back(id);
            }
            j += 1;
        }
        let key = DataKey::OpenBounties;
        env.storage().persistent().set(&key, &remaining);
        extend(env, &key);
    }
    limit
}

/// Drain the legacy `StatusIndex(status)` and `OpenBounties` blobs into the
/// paged shards, migrating at most `chunk_size` entries from each blob.
///
/// Returns the total number of entries drained across both indexes. Safe to
/// call repeatedly: once a legacy key has been fully drained it is deleted and
/// subsequent calls are no-ops for that index.
pub fn migrate_legacy_indexes(env: &Env, status: &Symbol, chunk_size: u32) -> u32 {
    let chunk = if chunk_size == 0 {
        MIGRATION_CHUNK_SIZE
    } else {
        chunk_size
    };
    let mut drained = migrate_legacy_status_index(env, status, chunk);
    drained += migrate_legacy_open_bounties(env, chunk);
    drained
}

// Legacy set_open_bounties kept for any callers that haven't been migrated yet.
// It rebuilds the paged structure from a flat Vec — use only in tests / migration.
pub fn set_open_bounties(env: &Env, bounties: &Vec<BountyId>) {
    // Clear existing pages.
    let old_total = get_open_bounties_count(env);
    if old_total > 0 {
        let old_pages = old_total.div_ceil(PAGE_SIZE);
        let mut p = 0u32;
        while p < old_pages {
            let key = DataKey::OpenBountiesPage(p);
            env.storage().persistent().remove(&key);
            p += 1;
        }
    }
    set_open_bounties_count(env, 0);

    // Re-insert all entries.
    for id in bounties.iter() {
        add_open_bounty(env, &id);
    }
}

// ── Approvals ─────────────────────────────────────────────────────────────────

pub fn get_approvals(env: &Env, bounty_id: &BountyId) -> Vec<Address> {
    let key = DataKey::Approvals(bounty_id.clone());
    let result: Option<Vec<Address>> = env.storage().persistent().get(&key);
    if result.is_some() {
        extend(env, &key);
    }
    result.unwrap_or_else(|| Vec::new(env))
}

pub fn set_approvals(env: &Env, bounty_id: &BountyId, approvals: &Vec<Address>) {
    let key = DataKey::Approvals(bounty_id.clone());
    env.storage().persistent().set(&key, approvals);
    extend(env, &key);
}

// ── Sponsors ──────────────────────────────────────────────────────────────────

/// Returns the list of `(sponsor, contributed_amount)` pairs for a bounty.
pub fn get_sponsors(env: &Env, bounty_id: &BountyId) -> Vec<(Address, i128)> {
    let key = DataKey::Sponsors(bounty_id.clone());
    let result: Option<Vec<(Address, i128)>> = env.storage().persistent().get(&key);
    if result.is_some() {
        extend(env, &key);
    }
    result.unwrap_or_else(|| Vec::new(env))
}

fn set_sponsors(env: &Env, bounty_id: &BountyId, sponsors: &Vec<(Address, i128)>) {
    let key = DataKey::Sponsors(bounty_id.clone());
    if sponsors.is_empty() {
        env.storage().persistent().remove(&key);
    } else {
        env.storage().persistent().set(&key, sponsors);
        extend(env, &key);
    }
}

/// Maximum number of distinct sponsors tracked per bounty.
///
/// `cancel_bounty` / `expire_bounty` refund every sponsor in a single
/// invocation; an unbounded list would let an attacker grief those paths by
/// adding hundreds of dust sponsors until `refund_sponsors` exceeds Soroban's
/// host budget. Keep this well below the per-invocation entry limit.
pub const MAX_SPONSORS_PER_BOUNTY: u32 = 20;

/// Minimum amount a single sponsorship call may contribute. Prevents dust
/// entries from consuming sponsor slots and bloating refund loops.
pub const MIN_SPONSOR_AMOUNT: i128 = 1;

/// Records `amount` as contributed by `sponsor`, adding to any prior
/// contribution from the same address.
///
/// Rejects amounts below [`MIN_SPONSOR_AMOUNT`] with
/// `ContractError::SponsorAmountTooLow`, new sponsors beyond
/// [`MAX_SPONSORS_PER_BOUNTY`] with `ContractError::TooManySponsors`, and
/// accumulation overflow with `ContractError::RewardAmountOverflow`.
pub fn add_sponsor_contribution(env: &Env, bounty_id: &BountyId, sponsor: &Address, amount: i128) {
    if amount < MIN_SPONSOR_AMOUNT {
        fail(ContractError::SponsorAmountTooLow);
    }

    let sponsors = get_sponsors(env, bounty_id);
    let mut found = false;
    let mut updated: Vec<(Address, i128)> = Vec::new(env);
    for (addr, existing) in sponsors.iter() {
        if addr == *sponsor {
            let total = match existing.checked_add(amount) {
                Some(total) => total,
                None => fail(ContractError::RewardAmountOverflow),
            };
            updated.push_back((addr, total));
            found = true;
        } else {
            updated.push_back((addr, existing));
        }
    }
    if !found {
        if sponsors.len() >= MAX_SPONSORS_PER_BOUNTY {
            fail(ContractError::TooManySponsors);
        }
        updated.push_back((sponsor.clone(), amount));
    }
    set_sponsors(env, bounty_id, &updated);
}

/// Removes and returns all sponsor contributions for a bounty (used once
/// sponsors have been refunded on cancel/expiry, to prevent double refunds).
pub fn take_sponsors(env: &Env, bounty_id: &BountyId) -> Vec<(Address, i128)> {
    let sponsors = get_sponsors(env, bounty_id);
    let key = DataKey::Sponsors(bounty_id.clone());
    env.storage().persistent().remove(&key);
    sponsors
}

// ── Work submissions ──────────────────────────────────────────────────────────

pub fn get_submission(env: &Env, bounty_id: &BountyId) -> Option<soroban_sdk::BytesN<32>> {
    let key = DataKey::Submission(bounty_id.clone());
    let result: Option<soroban_sdk::BytesN<32>> = env.storage().persistent().get(&key);
    if result.is_some() {
        extend(env, &key);
    }
    result
}

pub fn set_submission(env: &Env, bounty_id: &BountyId, hash: &soroban_sdk::BytesN<32>) {
    let key = DataKey::Submission(bounty_id.clone());
    env.storage().persistent().set(&key, hash);
    extend(env, &key);
}

// ── Creator bounties index ────────────────────────────────────────────────────

pub fn get_creator_bounties(env: &Env, creator: &Address) -> Vec<BountyId> {
    let key = DataKey::ContributorBounties(creator.clone());
    let list: Option<Vec<BountyId>> = env.storage().persistent().get(&key);
    if list.is_some() {
        extend(env, &key);
    }
    list.unwrap_or_else(|| Vec::new(env))
}

pub fn append_creator_bounty(env: &Env, creator: &Address, bounty_id: &BountyId) {
    let mut list = get_creator_bounties(env, creator);
    list.push_back(bounty_id.clone());
    let key = DataKey::ContributorBounties(creator.clone());
    env.storage().persistent().set(&key, &list);
    extend(env, &key);
}

/// Remove `bounty_id` from `creator`'s index (used when ownership moves).
///
/// The key is deleted entirely once the index becomes empty so no stale
/// empty vector keeps accruing TTL-extension rent.
pub fn remove_creator_bounty(env: &Env, creator: &Address, bounty_id: &BountyId) {
    let list = get_creator_bounties(env, creator);
    let mut kept: Vec<BountyId> = Vec::new(env);
    for id in list.iter() {
        if id != *bounty_id {
            kept.push_back(id);
        }
    }
    let key = DataKey::ContributorBounties(creator.clone());
    if kept.is_empty() {
        env.storage().persistent().remove(&key);
    } else {
        env.storage().persistent().set(&key, &kept);
        extend(env, &key);
    }
}

/// Returns the number of bounties currently indexed under `creator`.
///
/// Storage key: `DataKey::BountiesByCreatorCount(creator)`
/// Stored type: `u32`
/// Default value: `0`
///
/// O(1) read backed by a persistent counter, so callers can compute page
/// counts without scanning every page of the creator index.
pub fn get_bounties_by_creator_count(env: &Env, creator: &Address) -> u32 {
    let key = DataKey::BountiesByCreatorCount(creator.clone());
    let count: Option<u32> = env.storage().persistent().get(&key);
    if count.is_some() {
        extend(env, &key);
    }
    count.unwrap_or(0)
}

fn set_bounties_by_creator_count(env: &Env, creator: &Address, count: u32) {
    let key = DataKey::BountiesByCreatorCount(creator.clone());
    env.storage().persistent().set(&key, &count);
    extend(env, &key);
}

/// Increment `creator`'s bounty index counter by one.
pub fn increment_bounties_by_creator_count(env: &Env, creator: &Address) {
    let current = get_bounties_by_creator_count(env, creator);
    set_bounties_by_creator_count(env, creator, current + 1);
}

/// Decrement `creator`'s bounty index counter by one, saturating at zero.
pub fn decrement_bounties_by_creator_count(env: &Env, creator: &Address) {
    let current = get_bounties_by_creator_count(env, creator);
    if current > 0 {
        set_bounties_by_creator_count(env, creator, current - 1);
    }
}

// ── Pending admin / bounty owner (two-step transfers) ─────────────────────────

pub fn get_pending_admin(env: &Env) -> Option<Address> {
    env.storage().instance().get(&DataKey::PendingAdmin)
}

pub fn set_pending_admin(env: &Env, pending: &Address) {
    env.storage()
        .instance()
        .set(&DataKey::PendingAdmin, pending);
}

pub fn clear_pending_admin(env: &Env) {
    env.storage().instance().remove(&DataKey::PendingAdmin);
}

/// Whether the admin has paused the contract. Defaults to `false`.
pub fn is_paused(env: &Env) -> bool {
    env.storage()
        .instance()
        .get(&DataKey::Paused)
        .unwrap_or(false)
}

pub fn set_paused(env: &Env, paused: bool) {
    env.storage().instance().set(&DataKey::Paused, &paused);
}

pub fn get_pending_owner(env: &Env, bounty_id: &BountyId) -> Option<Address> {
    let key = DataKey::PendingOwner(bounty_id.clone());
    let result = env.storage().persistent().get(&key);
    if result.is_some() {
        extend(env, &key);
    }
    result
}

pub fn set_pending_owner(env: &Env, bounty_id: &BountyId, pending: &Address) {
    let key = DataKey::PendingOwner(bounty_id.clone());
    env.storage().persistent().set(&key, pending);
    extend(env, &key);
}

pub fn clear_pending_owner(env: &Env, bounty_id: &BountyId) {
    env.storage()
        .persistent()
        .remove(&DataKey::PendingOwner(bounty_id.clone()));
}

// ── Per-token minimum reward ──────────────────────────────────────────────────

/// Admin-configured minimum reward for `token`; `0` when never set.
pub fn get_min_reward(env: &Env, token: &Address) -> i128 {
    env.storage()
        .instance()
        .get(&DataKey::MinReward(token.clone()))
        .unwrap_or(0)
}

pub fn set_min_reward(env: &Env, token: &Address, amount: i128) {
    env.storage()
        .instance()
        .set(&DataKey::MinReward(token.clone()), &amount);
}

// ── Token bounties index — paged ─────────────────────────────────────────────
//
// Popular reward tokens (e.g. XLM/USDC) can accumulate thousands of bounties.
// A single `Vec<BountyId>` blob would eventually exceed Soroban's ~64 KB
// per-entry XDR cap, so the index is sharded into pages of at most PAGE_SIZE
// entries. Layout:
//   DataKey::TokenBountiesCount(token)        → u32  (total items, not pages)
//   DataKey::TokenBountiesPage(token, page)   → Vec<BountyId>  (0-indexed)
//
// Appends go to the last page, creating a new page when full. Removal uses a
// swap-remove: the very last item across all pages fills the hole and the last
// page is trimmed by one, deleting it when it becomes empty.

fn get_token_bounties_page(env: &Env, token: &Address, page: u32) -> Vec<BountyId> {
    let key = DataKey::TokenBountiesPage(token.clone(), page);
    let result: Option<Vec<BountyId>> = env.storage().persistent().get(&key);
    if result.is_some() {
        extend(env, &key);
    }
    result.unwrap_or_else(|| Vec::new(env))
}

fn set_token_bounties_page(env: &Env, token: &Address, page: u32, items: &Vec<BountyId>) {
    let key = DataKey::TokenBountiesPage(token.clone(), page);
    if items.is_empty() {
        env.storage().persistent().remove(&key);
    } else {
        env.storage().persistent().set(&key, items);
        extend(env, &key);
    }
}

/// Returns the total number of bounties indexed for `token` (not page count).
pub fn get_token_bounties_count(env: &Env, token: &Address) -> u32 {
    let key = DataKey::TokenBountiesCount(token.clone());
    let count: Option<u32> = env.storage().persistent().get(&key);
    if count.is_some() {
        extend(env, &key);
    }
    count.unwrap_or(0)
}

fn set_token_bounties_count(env: &Env, token: &Address, count: u32) {
    let key = DataKey::TokenBountiesCount(token.clone());
    if count == 0 {
        env.storage().persistent().remove(&key);
    } else {
        env.storage().persistent().set(&key, &count);
        extend(env, &key);
    }
}

/// Returns a single page of bounty IDs for `token`. `page` is 0-indexed.
/// Returns an empty `Vec` when `page >= page_count`.
pub fn get_token_bounties_page_data(env: &Env, token: &Address, page: u32) -> Vec<BountyId> {
    get_token_bounties_page(env, token, page)
}

/// Compatibility helper: returns **all** bounty IDs paid in `token`, in
/// creation order. Walks every page; prefer `get_token_bounties_page_data`
/// for bounded reads.
pub fn get_token_bounties(env: &Env, token: &Address) -> Vec<BountyId> {
    let total = get_token_bounties_count(env, token);
    let mut result = Vec::new(env);
    if total == 0 {
        return result;
    }
    let page_count = total.div_ceil(PAGE_SIZE);
    let mut i = 0u32;
    while i < page_count {
        let page = get_token_bounties_page(env, token, i);
        for id in page.iter() {
            result.push_back(id);
        }
        i += 1;
    }
    result
}

/// Append `bounty_id` to `token`'s index. Called once from `create_bounty`.
pub fn append_token_bounty(env: &Env, token: &Address, bounty_id: &BountyId) {
    let total = get_token_bounties_count(env, token);
    let target_page_idx = if total == 0 {
        0
    } else if total.is_multiple_of(PAGE_SIZE) {
        total / PAGE_SIZE
    } else {
        total.div_ceil(PAGE_SIZE) - 1
    };
    let mut page = get_token_bounties_page(env, token, target_page_idx);
    page.push_back(bounty_id.clone());
    set_token_bounties_page(env, token, target_page_idx, &page);
    set_token_bounties_count(env, token, total + 1);
}

/// Remove `bounty_id` from `token`'s index. Called from `archive_bounty` so
/// the index never points at a pruned full record.
pub fn remove_token_bounty(env: &Env, token: &Address, bounty_id: &BountyId) {
    let total = get_token_bounties_count(env, token);
    if total == 0 {
        return;
    }
    let page_count = total.div_ceil(PAGE_SIZE);

    // Locate the target across pages.
    let mut found_page: Option<u32> = None;
    let mut found_pos: Option<u32> = None;
    let mut p = 0u32;
    'outer: while p < page_count {
        let page = get_token_bounties_page(env, token, p);
        for (pos, id) in page.iter().enumerate() {
            if id == *bounty_id {
                found_page = Some(p);
                found_pos = Some(pos as u32);
                break 'outer;
            }
        }
        p += 1;
    }

    let (fp, fpos) = match (found_page, found_pos) {
        (Some(a), Some(b)) => (a, b),
        _ => return,
    };

    let last_page_idx = page_count - 1;
    let last_page = get_token_bounties_page(env, token, last_page_idx);
    if last_page.is_empty() {
        fail(ContractError::CorruptedIndex);
    }
    let last_item = match last_page.get(last_page.len() - 1) {
        Some(item) => item,
        None => fail(ContractError::CorruptedIndex),
    };

    if fp == last_page_idx && fpos == last_page.len() - 1 {
        // Removing the very last item — just trim the last page.
        let mut trimmed: Vec<BountyId> = Vec::new(env);
        let mut i = 0u32;
        while i < last_page.len() - 1 {
            trimmed.push_back(
                last_page
                    .get(i)
                    .unwrap_or_else(|| fail(ContractError::CorruptedIndex)),
            );
            i += 1;
        }
        set_token_bounties_page(env, token, last_page_idx, &trimmed);
    } else {
        // Move the last item into the hole, then trim the last page.
        let target_page = get_token_bounties_page(env, token, fp);
        let mut new_target: Vec<BountyId> = Vec::new(env);
        let mut i = 0u32;
        while i < target_page.len() {
            if i == fpos {
                new_target.push_back(last_item.clone());
            } else {
                new_target.push_back(
                    target_page
                        .get(i)
                        .unwrap_or_else(|| fail(ContractError::CorruptedIndex)),
                );
            }
            i += 1;
        }
        set_token_bounties_page(env, token, fp, &new_target);

        // Shrink the freshly written copy when the target was on the last
        // page; the pre-swap `last_page` would resurrect the removed id.
        let source = if fp == last_page_idx {
            new_target
        } else {
            last_page
        };
        let mut trimmed: Vec<BountyId> = Vec::new(env);
        let mut i = 0u32;
        while i < source.len() - 1 {
            trimmed.push_back(
                source
                    .get(i)
                    .unwrap_or_else(|| fail(ContractError::CorruptedIndex)),
            );
            i += 1;
        }
        set_token_bounties_page(env, token, last_page_idx, &trimmed);
    }

    set_token_bounties_count(env, token, total - 1);
}

// ── Tag index — paged ────────────────────────────────────────────────────────
//
// Bounties are tagged with `Symbol`s. A full scan over every open bounty to
// answer a tag query is O(N); instead each tag maintains its own paged index.
// Layout:
//   DataKey::TagIndexCount(tag)        → u32  (total items, not pages)
//   DataKey::TagIndexPage(tag, page)   → Vec<BountyId>  (0-indexed)
//
// Same append / swap-remove / page-slice semantics as the other paged indexes.

fn get_tag_index_page(env: &Env, tag: &Symbol, page: u32) -> Vec<BountyId> {
    let key = DataKey::TagIndexPage(tag.clone(), page);
    let result: Option<Vec<BountyId>> = env.storage().persistent().get(&key);
    if result.is_some() {
        extend(env, &key);
    }
    result.unwrap_or_else(|| Vec::new(env))
}

fn set_tag_index_page(env: &Env, tag: &Symbol, page: u32, items: &Vec<BountyId>) {
    let key = DataKey::TagIndexPage(tag.clone(), page);
    if items.is_empty() {
        env.storage().persistent().remove(&key);
    } else {
        env.storage().persistent().set(&key, items);
        extend(env, &key);
    }
}

/// Returns the total number of bounties indexed under `tag` (not page count).
pub fn get_tag_index_count(env: &Env, tag: &Symbol) -> u32 {
    let key = DataKey::TagIndexCount(tag.clone());
    let count: Option<u32> = env.storage().persistent().get(&key);
    if count.is_some() {
        extend(env, &key);
    }
    count.unwrap_or(0)
}

fn set_tag_index_count(env: &Env, tag: &Symbol, count: u32) {
    let key = DataKey::TagIndexCount(tag.clone());
    if count == 0 {
        env.storage().persistent().remove(&key);
    } else {
        env.storage().persistent().set(&key, &count);
        extend(env, &key);
    }
}

/// Returns a single page of bounty IDs for `tag`. `page` is 0-indexed.
pub fn get_tag_bounties_page_data(env: &Env, tag: &Symbol, page: u32) -> Vec<BountyId> {
    get_tag_index_page(env, tag, page)
}

/// Append `bounty_id` to `tag`'s index. Idempotent: a bounty already present
/// in the index is not added twice.
pub fn append_tag_bounty(env: &Env, tag: &Symbol, bounty_id: &BountyId) {
    let total = get_tag_index_count(env, tag);
    if total > 0 {
        let page_count = total.div_ceil(PAGE_SIZE);
        let mut p = 0u32;
        while p < page_count {
            let page = get_tag_index_page(env, tag, p);
            for id in page.iter() {
                if id == *bounty_id {
                    return;
                }
            }
            p += 1;
        }
    }
    let target_page_idx = if total == 0 {
        0
    } else if total.is_multiple_of(PAGE_SIZE) {
        total / PAGE_SIZE
    } else {
        total.div_ceil(PAGE_SIZE) - 1
    };
    let mut page = get_tag_index_page(env, tag, target_page_idx);
    page.push_back(bounty_id.clone());
    set_tag_index_page(env, tag, target_page_idx, &page);
    set_tag_index_count(env, tag, total + 1);
}

/// Remove `bounty_id` from `tag`'s index using a swap-remove across pages.
pub fn remove_tag_bounty(env: &Env, tag: &Symbol, bounty_id: &BountyId) {
    let total = get_tag_index_count(env, tag);
    if total == 0 {
        return;
    }
    let page_count = total.div_ceil(PAGE_SIZE);

    let mut found_page: Option<u32> = None;
    let mut found_pos: Option<u32> = None;
    let mut p = 0u32;
    'outer: while p < page_count {
        let page = get_tag_index_page(env, tag, p);
        for (pos, id) in page.iter().enumerate() {
            if id == *bounty_id {
                found_page = Some(p);
                found_pos = Some(pos as u32);
                break 'outer;
            }
        }
        p += 1;
    }

    let (fp, fpos) = match (found_page, found_pos) {
        (Some(a), Some(b)) => (a, b),
        _ => return,
    };

    let last_page_idx = page_count - 1;
    let last_page = get_tag_index_page(env, tag, last_page_idx);
    if last_page.is_empty() {
        fail(ContractError::CorruptedIndex);
    }
    let last_item = match last_page.get(last_page.len() - 1) {
        Some(item) => item,
        None => fail(ContractError::CorruptedIndex),
    };

    if fp == last_page_idx && fpos == last_page.len() - 1 {
        let mut trimmed: Vec<BountyId> = Vec::new(env);
        let mut i = 0u32;
        while i < last_page.len() - 1 {
            trimmed.push_back(
                last_page
                    .get(i)
                    .unwrap_or_else(|| fail(ContractError::CorruptedIndex)),
            );
            i += 1;
        }
        set_tag_index_page(env, tag, last_page_idx, &trimmed);
    } else {
        let target_page = get_tag_index_page(env, tag, fp);
        let mut new_target: Vec<BountyId> = Vec::new(env);
        let mut i = 0u32;
        while i < target_page.len() {
            if i == fpos {
                new_target.push_back(last_item.clone());
            } else {
                new_target.push_back(
                    target_page
                        .get(i)
                        .unwrap_or_else(|| fail(ContractError::CorruptedIndex)),
                );
            }
            i += 1;
        }
        set_tag_index_page(env, tag, fp, &new_target);

        let source = if fp == last_page_idx {
            new_target
        } else {
            last_page
        };
        let mut trimmed: Vec<BountyId> = Vec::new(env);
        let mut i = 0u32;
        while i < source.len() - 1 {
            trimmed.push_back(
                source
                    .get(i)
                    .unwrap_or_else(|| fail(ContractError::CorruptedIndex)),
            );
            i += 1;
        }
        set_tag_index_page(env, tag, last_page_idx, &trimmed);
    }

    set_tag_index_count(env, tag, total - 1);
}

/// Read only the page window `[start_page ..= end_page]` for a tag index,
/// returning the concatenated IDs plus the exact `next_cursor`.
pub fn get_tag_bounties_window(
    env: &Env,
    tag: &Symbol,
    cursor: u32,
    limit: u32,
) -> (Vec<BountyId>, Option<u32>) {
    let total = get_tag_index_count(env, tag);
    let mut items: Vec<BountyId> = Vec::new(env);
    if limit == 0 || cursor >= total {
        return (items, None);
    }
    let start_page = cursor / PAGE_SIZE;
    let end_offset = cursor.saturating_add(limit).min(total);
    let end_page = (end_offset - 1) / PAGE_SIZE;

    let mut p = start_page;
    while p <= end_page {
        let page = get_tag_index_page(env, tag, p);
        let page_start = p * PAGE_SIZE;
        let mut i = 0u32;
        while i < page.len() {
            let flat = page_start + i;
            if flat >= cursor && flat < end_offset {
                if let Some(id) = page.get(i) {
                    items.push_back(id);
                }
            }
            i += 1;
        }
        p += 1;
    }

    let next_cursor = if end_offset < total {
        Some(end_offset)
    } else {
        None
    };
    (items, next_cursor)
}

// ── Contributor bounty history — paged ───────────────────────────────────────
//
// Layout:
//   DataKey::ContributorHistoryCount(address)        → u32
//   DataKey::ContributorHistoryPage(address, page)   → Vec<BountyId>
//
// Entries are appended in chronological order (oldest first) to the last page,
// creating a new page when the current one is full. Reads that need the most
// recent entries walk pages from the tail backwards, so only the requested
// window is materialised — the full history is never copied into a temporary
// `Vec`. Inserts are deduplicated against the last page (the only page that can
// receive the new entry) to keep the index idempotent.

/// Returns the total number of terminal bounties recorded for `address`.
pub fn get_contributor_history_count(env: &Env, address: &Address) -> u32 {
    let key = DataKey::ContributorHistoryCount(address.clone());
    let count: Option<u32> = env.storage().persistent().get(&key);
    if count.is_some() {
        extend(env, &key);
    }
    count.unwrap_or(0)
}

fn set_contributor_history_count(env: &Env, address: &Address, count: u32) {
    let key = DataKey::ContributorHistoryCount(address.clone());
    env.storage().persistent().set(&key, &count);
    extend(env, &key);
}

fn get_contributor_history_page(env: &Env, address: &Address, page: u32) -> Vec<BountyId> {
    let key = DataKey::ContributorHistoryPage(address.clone(), page);
    let result: Option<Vec<BountyId>> = env.storage().persistent().get(&key);
    if result.is_some() {
        extend(env, &key);
    }
    result.unwrap_or_else(|| Vec::new(env))
}

fn set_contributor_history_page(env: &Env, address: &Address, page: u32, items: &Vec<BountyId>) {
    let key = DataKey::ContributorHistoryPage(address.clone(), page);
    if items.is_empty() {
        env.storage().persistent().remove(&key);
    } else {
        env.storage().persistent().set(&key, items);
        extend(env, &key);
    }
}

/// Returns every bounty ID `address` was an assignee on when it reached a
/// terminal status (`"completed"` or `"cancelled"`), oldest first. Empty if
/// none.
///
/// Compatibility helper: walks all pages. Prefer
/// [`get_contributor_history_page_data`] for large histories.
pub fn get_contributor_history(env: &Env, address: &Address) -> Vec<BountyId> {
    let total = get_contributor_history_count(env, address);
    let mut result = Vec::new(env);
    if total == 0 {
        return result;
    }
    let page_count = total.div_ceil(PAGE_SIZE);
    let mut p = 0u32;
    while p < page_count {
        let page = get_contributor_history_page(env, address, p);
        for id in page.iter() {
            result.push_back(id);
        }
        p += 1;
    }
    result
}

/// Returns a single page of `address`'s history, oldest first. `page` is
/// 0-indexed; returns an empty `Vec` when `page >= page_count`.
pub fn get_contributor_history_page_data(env: &Env, address: &Address, page: u32) -> Vec<BountyId> {
    get_contributor_history_page(env, address, page)
}

/// Returns up to `limit` of `address`'s most recent history entries, newest
/// first, starting `offset` entries back from the tail.
///
/// Only the pages overlapping the requested window are read; the full history
/// is never materialised.
pub fn get_contributor_history_recent(
    env: &Env,
    address: &Address,
    offset: u32,
    limit: u32,
) -> Vec<BountyId> {
    let mut result = Vec::new(env);
    let total = get_contributor_history_count(env, address);
    if total == 0 || limit == 0 || offset >= total {
        return result;
    }

    // Number of entries still available after skipping `offset` from the tail.
    let available = total - offset;
    let take = if limit < available { limit } else { available };

    // Index (exclusive) of the newest entry we want, in chronological order.
    let mut end = total - offset; // exclusive
    let mut remaining = take;
    while remaining > 0 && end > 0 {
        let page_idx = (end - 1) / PAGE_SIZE;
        let page = get_contributor_history_page(env, address, page_idx);
        let page_len = page.len();
        if page_len == 0 {
            fail(ContractError::CorruptedIndex);
        }
        // Position within the page of the newest wanted entry (exclusive end).
        let page_end = end - page_idx * PAGE_SIZE;
        let page_end = if page_end > page_len { page_len } else { page_end };
        let mut pos = page_end;
        while pos > 0 && remaining > 0 {
            pos -= 1;
            if let Some(id) = page.get(pos) {
                result.push_back(id);
                remaining -= 1;
            }
        }
        end = page_idx * PAGE_SIZE;
    }
    result
}

fn append_contributor_history(env: &Env, address: &Address, bounty_id: &BountyId) {
    let total = get_contributor_history_count(env, address);
    let last_page_idx = if total == 0 {
        0
    } else {
        (total - 1) / PAGE_SIZE
    };

    // Deduplicate against the last page: appends only ever land there, so a
    // duplicate can only exist on the tail page.
    let mut page = get_contributor_history_page(env, address, last_page_idx);
    for id in page.iter() {
        if id == *bounty_id {
            return; // already recorded, nothing to do
        }
    }

    // Start a fresh page when the tail page is full.
    let target_page_idx = if total > 0 && total.is_multiple_of(PAGE_SIZE) {
        total / PAGE_SIZE
    } else {
        last_page_idx
    };
    if target_page_idx != last_page_idx {
        page = Vec::new(env);
    }
    page.push_back(bounty_id.clone());
    set_contributor_history_page(env, address, target_page_idx, &page);
    set_contributor_history_count(env, address, total + 1);
}

// ── Admin ─────────────────────────────────────────────────────────────────────

/// Returns the contract admin, if one has been set.
pub fn get_admin(env: &Env) -> Option<Address> {
    env.storage().instance().get(&DataKey::Admin)
}

/// Persist the contract admin address.
pub fn set_admin(env: &Env, admin: &Address) {
    env.storage().instance().set(&DataKey::Admin, admin);
}

// ── Creator open-bounty cap ───────────────────────────────────────────────────

/// Returns the configured cap on concurrent open+claimed bounties per
/// creator. `0` means unlimited (the default when never configured).
pub fn get_creator_open_limit(env: &Env) -> u32 {
    env.storage()
        .instance()
        .get(&DataKey::CreatorOpenLimit)
        .unwrap_or(0)
}

/// Configure the per-creator open-bounty cap. `0` disables the cap.
pub fn set_creator_open_limit(env: &Env, limit: u32) {
    env.storage()
        .instance()
        .set(&DataKey::CreatorOpenLimit, &limit);
}

/// Returns the current count of open+claimed bounties for `creator`.
pub fn get_creator_open_count(env: &Env, creator: &Address) -> u32 {
    let key = DataKey::CreatorOpenCount(creator.clone());
    let count: Option<u32> = env.storage().persistent().get(&key);
    if count.is_some() {
        extend(env, &key);
    }
    count.unwrap_or(0)
}

fn set_creator_open_count(env: &Env, creator: &Address, count: u32) {
    let key = DataKey::CreatorOpenCount(creator.clone());
    env.storage().persistent().set(&key, &count);
    extend(env, &key);
}

/// Increment `creator`'s open+claimed bounty counter by one.
pub fn increment_creator_open_count(env: &Env, creator: &Address) {
    let current = get_creator_open_count(env, creator);
    set_creator_open_count(env, creator, current + 1);
}

/// Decrement `creator`'s open+claimed bounty counter by one, saturating at
/// zero so the counter can never go negative.
pub fn decrement_creator_open_count(env: &Env, creator: &Address) {
    let current = get_creator_open_count(env, creator);
    if current > 0 {
        set_creator_open_count(env, creator, current - 1);
    }
}

// ── Claim cooldown ────────────────────────────────────────────────────────────

/// Returns the configured claim cooldown, in ledgers. `0` disables it
/// (the default).
pub fn get_claim_cooldown(env: &Env) -> u32 {
    env.storage()
        .instance()
        .get(&DataKey::ClaimCooldown)
        .unwrap_or(0)
}

/// Configure the claim cooldown, in ledgers. `0` disables it.
pub fn set_claim_cooldown(env: &Env, ledgers: u32) {
    env.storage()
        .instance()
        .set(&DataKey::ClaimCooldown, &ledgers);
}

/// Record that `contributor` unclaimed `bounty_id` at the current ledger
/// sequence. Stored in **temporary** storage with a TTL equal to the
/// configured cooldown (or a minimum of one ledger) so the entry expires on
/// its own once the cooldown has elapsed.
pub fn record_unclaim(env: &Env, bounty_id: &BountyId, contributor: &Address) {
    let key = DataKey::UnclaimedAt(bounty_id.clone(), contributor.clone());
    let now = env.ledger().sequence();
    env.storage().temporary().set(&key, &now);
    let ttl = get_claim_cooldown(env).max(1);
    env.storage().temporary().extend_ttl(&key, ttl, ttl);
}

/// Returns the ledger sequence at which `contributor` last unclaimed
/// `bounty_id`, if that record hasn't expired yet.
pub fn get_unclaimed_at(env: &Env, bounty_id: &BountyId, contributor: &Address) -> Option<u32> {
    let key = DataKey::UnclaimedAt(bounty_id.clone(), contributor.clone());
    env.storage().temporary().get(&key)
}

// ── Dispute evidence ──────────────────────────────────────────────────────────

/// Maximum number of evidence hashes retained per dispute.
pub const MAX_DISPUTE_EVIDENCE: u32 = 10;

/// Returns the evidence hashes attached to `bounty_id`'s dispute, if any.
pub fn get_dispute_evidence(env: &Env, bounty_id: &BountyId) -> Vec<soroban_sdk::BytesN<32>> {
    let key = DataKey::DisputeEvidence(bounty_id.clone());
    let result: Option<Vec<soroban_sdk::BytesN<32>>> = env.storage().persistent().get(&key);
    if result.is_some() {
        extend(env, &key);
    }
    result.unwrap_or_else(|| Vec::new(env))
}

/// Append `hash` to `bounty_id`'s dispute evidence list.
pub fn append_dispute_evidence(env: &Env, bounty_id: &BountyId, hash: &soroban_sdk::BytesN<32>) {
    let mut evidence = get_dispute_evidence(env, bounty_id);
    evidence.push_back(hash.clone());
    let key = DataKey::DisputeEvidence(bounty_id.clone());
    env.storage().persistent().set(&key, &evidence);
    extend(env, &key);
}

// ── Dispute appeal window ─────────────────────────────────────────────────────

/// Returns the configured appeal window, in ledgers. `0` keeps the
/// immediate-payout behaviour (the default).
pub fn get_appeal_window(env: &Env) -> u32 {
    env.storage()
        .instance()
        .get(&DataKey::AppealWindow)
        .unwrap_or(0)
}

/// Configure the appeal window, in ledgers. `0` disables it.
pub fn set_appeal_window(env: &Env, ledgers: u32) {
    env.storage()
        .instance()
        .set(&DataKey::AppealWindow, &ledgers);
}

/// Returns the pending (undecided) dispute resolution for `bounty_id`, if any.
pub fn get_pending_resolution(env: &Env, bounty_id: &BountyId) -> Option<PendingResolution> {
    let key = DataKey::PendingResolution(bounty_id.clone());
    let result = env.storage().persistent().get(&key);
    if result.is_some() {
        extend(env, &key);
    }
    result
}

/// Store a dispute decision awaiting finalization or appeal.
pub fn set_pending_resolution(env: &Env, bounty_id: &BountyId, resolution: &PendingResolution) {
    let key = DataKey::PendingResolution(bounty_id.clone());
    env.storage().persistent().set(&key, resolution);
    extend(env, &key);
}

/// Remove a bounty's pending dispute resolution (after finalize or appeal).
pub fn remove_pending_resolution(env: &Env, bounty_id: &BountyId) {
    let key = DataKey::PendingResolution(bounty_id.clone());
    env.storage().persistent().remove(&key);
}

/// Returns whether `bounty_id`'s dispute has already used its one allowed appeal.
pub fn get_dispute_appealed(env: &Env, bounty_id: &BountyId) -> bool {
    let key = DataKey::DisputeAppealed(bounty_id.clone());
    let result: Option<bool> = env.storage().persistent().get(&key);
    if result.is_some() {
        extend(env, &key);
    }
    result.unwrap_or(false)
}

/// Mark `bounty_id`'s dispute as having used its one allowed appeal.
pub fn set_dispute_appealed(env: &Env, bounty_id: &BountyId) {
    let key = DataKey::DisputeAppealed(bounty_id.clone());
    env.storage().persistent().set(&key, &true);
    extend(env, &key);
}

// ── Claim window / stale claims ───────────────────────────────────────────────

/// Default number of ledgers a claim may remain unsubmitted before it can be
/// released via `release_stale_claim`. ~7 days at 5s/ledger.
pub const DEFAULT_CLAIM_WINDOW: u32 = 120_960;

pub fn get_claim_window(env: &Env) -> u32 {
    env.storage()
        .instance()
        .get(&DataKey::ClaimWindow)
        .unwrap_or(DEFAULT_CLAIM_WINDOW)
}

pub fn set_claim_window(env: &Env, window: u32) {
    env.storage().instance().set(&DataKey::ClaimWindow, &window);
}

pub fn get_claimed_at_ledger(env: &Env, bounty_id: &BountyId) -> Option<u32> {
    let key = DataKey::ClaimedAtLedger(bounty_id.clone());
    let result = env.storage().persistent().get(&key);
    if result.is_some() {
        extend(env, &key);
    }
    result
}

pub fn set_claimed_at_ledger(env: &Env, bounty_id: &BountyId, ledger: u32) {
    let key = DataKey::ClaimedAtLedger(bounty_id.clone());
    env.storage().persistent().set(&key, &ledger);
    extend(env, &key);
}

pub fn clear_claimed_at_ledger(env: &Env, bounty_id: &BountyId) {
    let key = DataKey::ClaimedAtLedger(bounty_id.clone());
    env.storage().persistent().remove(&key);
}

/// Default reputation penalty applied when a stale claim is released.
pub const DEFAULT_REPUTATION_PENALTY: u32 = 10;

pub fn get_reputation_penalty(env: &Env) -> u32 {
    env.storage()
        .instance()
        .get(&DataKey::ReputationPenalty)
        .unwrap_or(DEFAULT_REPUTATION_PENALTY)
}

pub fn set_reputation_penalty(env: &Env, penalty: u32) {
    env.storage()
        .instance()
        .set(&DataKey::ReputationPenalty, &penalty);
}

// ── Reward token allowlist ────────────────────────────────────────────────────
//
// Keyed per-token so membership checks and admin add/remove are O(1) without
// needing to read/rewrite a single growing list. `create_bounty` rejects any
// `reward_token` that is not on this allowlist, once the admin has added the
// first token (before that the allowlist is not enforced). Removing a token only clears
// its membership flag; it never touches existing `Bounty` records, so
// in-flight bounties denominated in a removed token keep paying out normally.

pub fn is_reward_token_allowed(env: &Env, token: &Address) -> bool {
    if !is_reward_token_allowlist_enabled(env) {
        return true;
    }
    env.storage()
        .instance()
        .get(&DataKey::AllowedRewardToken(token.clone()))
        .unwrap_or(false)
}

/// The allowlist is opt-in: it is enforced only after the admin has added at
/// least one token. Removing tokens later never disables it again.
pub fn is_reward_token_allowlist_enabled(env: &Env) -> bool {
    env.storage()
        .instance()
        .get(&DataKey::RewardTokenAllowlistEnabled)
        .unwrap_or(false)
}

pub fn add_allowed_reward_token(env: &Env, token: &Address) {
    env.storage()
        .instance()
        .set(&DataKey::AllowedRewardToken(token.clone()), &true);
    env.storage()
        .instance()
        .set(&DataKey::RewardTokenAllowlistEnabled, &true);
}

pub fn remove_allowed_reward_token(env: &Env, token: &Address) {
    env.storage()
        .instance()
        .remove(&DataKey::AllowedRewardToken(token.clone()));
}
