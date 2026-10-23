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
    /// Per-bounty list of `(sponsor_address, contributed_amount)` pairs for
    /// third-party co-funding. Maintained by `sponsor_bounty`; consumed by
    /// `cancel_bounty` / `expire_bounty` to refund each sponsor pro rata.
    Sponsors(BountyId),
    /// `BytesN<32>` work-submission hash recorded by `submit_work`, keyed by
    /// bounty id. Present once the assignee has submitted work for review.
    Submission(BountyId),
    /// Compacted summary record for a finished (completed/cancelled) bounty,
    /// written by `archive_bounty`. Once archived, the full `Bounty(id)` and
    /// `BountyMeta(id)` entries are removed to stop paying TTL-extension rent
    /// on data nobody reads anymore; `get_bounty` falls back to this record.
    BountyArchive(BountyId),
    /// Ledger sequence after which a finished bounty becomes eligible for
    /// `archive_bounty`. Written on the first (otherwise-failing) archive
    /// attempt against a finished bounty that has no marker yet, so the
    /// grace period is measured from "someone first tried to archive this"
    /// rather than from the exact completion/cancellation ledger (which the
    /// contract does not currently record).
    ArchiveEligibleAt(BountyId),
    /// Bounty IDs paid in a given reward token, keyed by the token contract
    /// address. Appended to on `create_bounty`; entries for archived
    /// bounties are pruned by `archive_bounty` so the index only lists
    /// bounties with a live full record.
    TokenBounties(Address),
    /// Contract admin address, set lazily by the first admin-only call.
    Admin,
    /// Admin-configured cap on concurrent open+claimed bounties per creator.
    /// `0` means unlimited (the default).
    CreatorOpenLimit,
    /// Current count of open+claimed bounties for a given creator.
    CreatorOpenCount(Address),
    /// Admin-configured cooldown (in ledgers) a contributor must wait after
    /// unclaiming a bounty before re-claiming it. `0` disables the cooldown.
    ClaimCooldown,
    /// Ledger sequence at which `contributor` last unclaimed `bounty_id`.
    /// Stored in temporary storage so it naturally expires.
    UnclaimedAt(BountyId, Address),
    /// Evidence hashes attached to a disputed bounty.
    DisputeEvidence(BountyId),
    /// Admin-configured appeal window, in ledgers, after a dispute decision
    /// and before it is finalized. `0` keeps the previous immediate-payout
    /// behaviour.
    AppealWindow,
    /// A dispute decision awaiting finalization (or appeal) for a bounty.
    PendingResolution(BountyId),
    /// Whether the single allowed appeal has already been used for a dispute.
    DisputeAppealed(BountyId),
    /// Ledger sequence at which the first assignee claimed a bounty.
    /// Used to detect stale claims eligible for `release_stale_claim`.
    ClaimedAtLedger(BountyId),
    /// Number of ledgers that may elapse after a claim before it becomes
    /// stale and eligible for `release_stale_claim`. Admin-configurable.
    ClaimWindow,
    /// Reputation penalty applied to assignees when a stale claim is released.
    ReputationPenalty,
    /// Set of reward token addresses the admin has allow-listed.
    AllowedRewardToken(Address),
    /// Set once the admin allow-lists the first reward token. Until then the
    /// allowlist is not enforced, so existing deployments keep accepting any
    /// `reward_token`.
    RewardTokenAllowlistEnabled,
    /// Admin-configured minimum `reward_amount` for bounties paid in a given
    /// token. Absent means only the global `MIN_REWARD_AMOUNT` applies.
    MinReward(Address),
    /// Admin-configured minimum `reward_amount` for bounties paid in a given
    /// token, keyed by the token address. Absent means only the global
    /// `MIN_REWARD_AMOUNT` applies.
    MinRewardAmount(Address),
    /// Address proposed by `transfer_admin`, pending `accept_admin`.
    PendingAdmin,
    /// Address proposed by `transfer_bounty_ownership` to take over a
    /// bounty, pending `accept_bounty_ownership`.
    PendingOwner(BountyId),
}

/// Snapshot of contract-wide admin settings, returned by `get_config`.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct ContractConfig {
    /// Current admin, if one has been established.
    pub admin: Option<Address>,
    /// Address proposed by `transfer_admin` that has not yet accepted.
    pub pending_admin: Option<Address>,
    /// Cap on concurrent open+claimed bounties per creator (`0` = unlimited).
    pub creator_open_limit: u32,
    /// Ledgers a contributor must wait before re-claiming an unclaimed bounty.
    pub claim_cooldown: u32,
    /// Ledgers after which an in-progress claim may be released as stale.
    pub claim_window: u32,
    /// Reputation penalty applied when a stale claim is released.
    pub reputation_penalty: u32,
    /// Ledgers a dispute decision waits for appeal before finalizing.
    pub appeal_window: u32,
}

/// An arbitrator's decision on a dispute, held in escrow until the appeal
/// window passes (`finalize_dispute`) or it is appealed (`appeal_dispute`).
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct PendingResolution {
    pub arbitrator: Address,
    pub resolution: Symbol,
    /// Ledger sequence at which the decision was recorded.
    pub decided_at: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct Milestone {
    pub description: Symbol,
    pub reward: i128,
    pub completed: bool,
    /// Optional ledger sequence deadline for this specific milestone.
    /// When set, milestones must be ordered non-decreasing by deadline and
    /// must not exceed the bounty's own deadline (validated in
    /// `create_bounty`). `None` preserves pre-existing behaviour for bounties
    /// created before this field existed.
    pub deadline: Option<u32>,
}

/// Lightweight read-only view of a single milestone, returned by
/// `get_milestones` so frontends can render milestone progress without
/// decoding the whole `Bounty` struct.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct MilestoneView {
    pub index: u32,
    pub reward: i128,
    pub completed: bool,
    pub deadline: Option<u32>,
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

/// Compacted record kept for a bounty after it has been archived.
///
/// Retains just enough data for history queries (id, creator, assignee,
/// status, reward) while the full `Bounty` and `BountyMeta` entries are
/// dropped from storage to stop accruing TTL-extension rent.
#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct BountySummary {
    pub id: BountyId,
    pub creator: Address,
    pub assignee: Option<Address>,
    pub status: Symbol,
    pub reward: i128,
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
