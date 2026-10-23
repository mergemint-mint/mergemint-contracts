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
//! | Multi-sig votes | `Approvals(BountyId)` | `Vec<Address>` | Per-bounty verifier approvals |
//!
//! ## Legacy variants (read-only)
//!
//! - `StatusIndex(Symbol)` — pre-pagination status blob; do not write.
//! - `OpenBounties` — pre-pagination open list; do not write.
//!
//! ## Paged indexes
//!
//! `StatusIndexPage` and `OpenBountiesPage` shard large lists into pages of at
//! most [`PAGE_SIZE`] entries to stay within Soroban's per-entry size limit.
//! Each index also maintains a companion count key (`StatusCount` /
//! `OpenBountiesCount`). See the inline layout comments in this file for
//! append and swap-remove semantics.
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
            trimmed.push_back(new_last.get(i).unwrap());
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
                new_target.push_back(target_page.get(i).unwrap());
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
            trimmed.push_back(source.get(i).unwrap());
            i += 1;
        }
        set_status_page(env, status, last_page_idx, &trimmed);
    }

    decrement_status_count(env, status);
}

pub fn move_bounty_status(
    env: &Env,
    bounty_id: &BountyId,
    old_status: &Symbol,
    new_status: &Symbol,
) {
    if old_status != new_status {
        remove_bounty_from_status(env, bounty_id, old_status);
        add_bounty_to_status(env, bounty_id, new_status);

        if *new_status == Symbol::new(env, STATUS_COMPLETED)
            || *new_status == Symbol::new(env, STATUS_CANCELLED)
        {
            if let Some(bounty) = get_bounty(env, bounty_id) {
                for (assignee, _) in bounty.assignees.iter() {
                    append_contributor_history(env, &assignee, bounty_id);
                }
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
            trimmed.push_back(last_page.get(i).unwrap());
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
                new_target.push_back(target_page.get(i).unwrap());
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
            trimmed.push_back(source.get(i).unwrap());
            i += 1;
        }
        set_open_bounties_page(env, last_page_idx, &trimmed);
    }

    set_open_bounties_count(env, total - 1);
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

/// Records `amount` as contributed by `sponsor`, adding to any prior
/// contribution from the same address.
pub fn add_sponsor_contribution(env: &Env, bounty_id: &BountyId, sponsor: &Address, amount: i128) {
    let mut sponsors = get_sponsors(env, bounty_id);
    let mut found = false;
    let mut updated: Vec<(Address, i128)> = Vec::new(env);
    for (addr, existing) in sponsors.iter() {
        if addr == *sponsor {
            updated.push_back((addr, existing + amount));
            found = true;
        } else {
            updated.push_back((addr, existing));
        }
    }
    if !found {
        updated.push_back((sponsor.clone(), amount));
    }
    sponsors = updated;
    set_sponsors(env, bounty_id, &sponsors);
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
    env.storage()
        .persistent()
        .get(&DataKey::ContributorBounties(creator.clone()))
        .unwrap_or_else(|| Vec::new(env))
}

pub fn append_creator_bounty(env: &Env, creator: &Address, bounty_id: &BountyId) {
    let mut list = get_creator_bounties(env, creator);
    list.push_back(bounty_id.clone());
    env.storage()
        .persistent()
        .set(&DataKey::ContributorBounties(creator.clone()), &list);
}

/// Remove `bounty_id` from `creator`'s index (used when ownership moves).
pub fn remove_creator_bounty(env: &Env, creator: &Address, bounty_id: &BountyId) {
    let list = get_creator_bounties(env, creator);
    let mut kept: Vec<BountyId> = Vec::new(env);
    for id in list.iter() {
        if id != *bounty_id {
            kept.push_back(id);
        }
    }
    env.storage()
        .persistent()
        .set(&DataKey::ContributorBounties(creator.clone()), &kept);
}

// ── Pending admin / bounty owner (two-step transfers) ─────────────────────────

pub fn get_pending_admin(env: &Env) -> Option<Address> {
    env.storage().instance().get(&DataKey::PendingAdmin)
}

pub fn set_pending_admin(env: &Env, pending: &Address) {
    env.storage().instance().set(&DataKey::PendingAdmin, pending);
}

pub fn clear_pending_admin(env: &Env) {
    env.storage().instance().remove(&DataKey::PendingAdmin);
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

// ── Token bounties index ─────────────────────────────────────────────────────

/// Returns every bounty ID paid in `token`, in creation order. Empty if
/// `token` has never been used as a `reward_token`.
pub fn get_token_bounties(env: &Env, token: &Address) -> Vec<BountyId> {
    env.storage()
        .persistent()
        .get(&DataKey::TokenBounties(token.clone()))
        .unwrap_or_else(|| Vec::new(env))
}

/// Append `bounty_id` to `token`'s index. Called once from `create_bounty`.
pub fn append_token_bounty(env: &Env, token: &Address, bounty_id: &BountyId) {
    let mut list = get_token_bounties(env, token);
    list.push_back(bounty_id.clone());
    env.storage()
        .persistent()
        .set(&DataKey::TokenBounties(token.clone()), &list);
}

/// Remove `bounty_id` from `token`'s index. Called from `archive_bounty` so
/// the index never points at a pruned full record.
pub fn remove_token_bounty(env: &Env, token: &Address, bounty_id: &BountyId) {
    let list = get_token_bounties(env, token);
    let mut trimmed: Vec<BountyId> = Vec::new(env);
    for id in list.iter() {
        if id != *bounty_id {
            trimmed.push_back(id);
        }
    }
    env.storage()
        .persistent()
        .set(&DataKey::TokenBounties(token.clone()), &trimmed);
}

// ── Contributor bounty history ───────────────────────────────────────────────

/// Returns every bounty ID `address` was an assignee on when it reached a
/// terminal status (`"completed"` or `"cancelled"`). Empty if none.
pub fn get_contributor_history(env: &Env, address: &Address) -> Vec<BountyId> {
    let key = DataKey::ContributorHistory(address.clone());
    let list: Option<Vec<BountyId>> = env.storage().persistent().get(&key);
    if list.is_some() {
        extend(env, &key);
    }
    list.unwrap_or_else(|| Vec::new(env))
}

fn append_contributor_history(env: &Env, address: &Address, bounty_id: &BountyId) {
    let mut list = get_contributor_history(env, address);
    list.push_back(bounty_id.clone());
    let key = DataKey::ContributorHistory(address.clone());
    env.storage().persistent().set(&key, &list);
    extend(env, &key);
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

pub fn get_admin(env: &Env) -> Option<Address> {
    env.storage().instance().get(&DataKey::Admin)
}

pub fn set_admin(env: &Env, admin: &Address) {
    env.storage().instance().set(&DataKey::Admin, admin);
}

pub fn get_min_reward_amount(env: &Env, token: &Address) -> i128 {
    env.storage()
        .persistent()
        .get(&DataKey::MinRewardAmount(token.clone()))
        .unwrap_or(0)
}

pub fn set_min_reward_amount(env: &Env, token: &Address, amount: &i128) {
    env.storage()
        .persistent()
        .set(&DataKey::MinRewardAmount(token.clone()), amount);
}
