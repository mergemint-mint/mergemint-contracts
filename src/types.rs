// SPDX-License-Identifier: MIT
use soroban_sdk::{contracttype, Address, BytesN, String, Symbol, Vec};

/// A type-safe identifier for a bounty.
///
/// Wraps a raw `BytesN<32>` to prevent accidental substitution with other
/// 32-byte values (hashes, keys, nonces). Serialises identically to the
/// inner `BytesN<32>` when used with `#[contracttype]`.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct BountyId(pub BytesN<32>);

impl From<BytesN<32>> for BountyId {
    fn from(value: BytesN<32>) -> Self {
        BountyId(value)
    }
}

impl From<BountyId> for BytesN<32> {
    fn from(value: BountyId) -> Self {
        value.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub enum DataKey {
    BountyCount,
    Bounty(BountyId),
    BountyMeta(BountyId),
    Contributor(Address),
    ContributorBounties(Address),
    /// Bounty IDs a contributor was an assignee on when the bounty reached a
    /// terminal status (`"completed"` or `"cancelled"`). Maintained
    /// incrementally by `storage::move_bounty_status`.
    ContributorHistory(Address),
    /// Legacy single-blob status index — replaced by StatusIndexPage.
    /// Kept in the enum so existing serialised keys can still be read during a
    /// migration pass. New code must not write this variant.
    StatusIndex(Symbol),
    StatusCount(Symbol),
    /// Paged status index shard. `page` is 0-indexed; each shard holds at most
    /// `storage::PAGE_SIZE` entries. See storage.rs for the layout contract.
    StatusIndexPage(Symbol, u32),
    /// Legacy single-blob open-bounties list — replaced by OpenBountiesPage.
    OpenBounties,
    /// Total number of open bounties (sum across all OpenBountiesPage shards).
    OpenBountiesCount,
    /// Paged open-bounties shard. `page` is 0-indexed.
    OpenBountiesPage(u32),
    Approvals(BountyId),
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct Milestone {
    pub description: Symbol,
    pub reward: i128,
    pub completed: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct Bounty {
    pub creator: Address,
    pub reward_amount: i128,
    pub reward_token: Address,
    pub assignees: Vec<(Address, u32)>,
    pub max_assignees: u32,
    pub status: Symbol,
    pub min_reputation: u32,
    pub deadline: Option<u32>,
    /// Optional list of addresses permitted to approve completion.
    /// When set, approve_completion enforces that only listed addresses may vote.
    /// When None, the single-verifier complete_bounty flow applies unchanged.
    pub required_verifiers: Option<Vec<Address>>,
    /// Number of approvals required before completion executes automatically.
    /// Only meaningful when required_verifiers is Some. A value of 0 is treated as 1.
    pub approval_threshold: u32,
    /// Categorisation tags for the bounty (e.g. "bug", "docs", "feature").
    /// At most 5 tags are allowed; `create_bounty` panics with `TooManyTags`
    /// if the caller supplies more than 5.
    pub tags: Vec<Symbol>,
    /// Optional staged payouts. When empty, the bounty is all-or-nothing.
    pub milestones: Vec<Milestone>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct BountyMeta {
    pub title: Symbol,
    pub description: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct Contributor {
    #[allow(dead_code)]
    pub address: Address,
    pub reputation: u32,
    pub total_earned: i128,
    pub contribution_count: u32,
    pub active_claims: u32,
    pub metadata: Option<Symbol>,
}

/// Pagination metadata returned alongside a page of results.
///
/// Used by `get_open_bounties_paged` to let callers know the total number of
/// open bounties so they can compute the number of pages without a separate
/// `get_bounty_count` call.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct PageInfo {
    /// Zero-based offset of the first item in this page.
    pub offset: u32,
    /// Number of items requested (may be fewer if near the end of the list).
    pub limit: u32,
    /// Total number of open bounties at query time.
    pub total: u32,
}

impl Contributor {
    pub fn new(address: Address) -> Self {
        Self {
            address,
            reputation: 0,
            total_earned: 0,
            contribution_count: 0,
            active_claims: 0,
            metadata: None,
        }
    }
}
