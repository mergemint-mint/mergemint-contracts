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

use crate::types::{Bounty, BountyId, BountyMeta, Contributor, DataKey};

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
    let last_item = last_page.get(last_page.len() - 1).unwrap();

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

        // Shrink the last page.
        let mut trimmed: Vec<BountyId> = Vec::new(env);
        let mut i = 0u32;
        while i < last_page.len() - 1 {
            trimmed.push_back(last_page.get(i).unwrap());
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
    let last_item = last_page.get(last_page.len() - 1).unwrap();

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

        let mut trimmed: Vec<BountyId> = Vec::new(env);
        let mut i = 0u32;
        while i < last_page.len() - 1 {
            trimmed.push_back(last_page.get(i).unwrap());
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
