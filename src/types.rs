// SPDX-License-Identifier: MIT
use soroban_sdk::{contracttype, Address, BytesN, Symbol, Vec};

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
    StatusIndex(Symbol),
    OpenBounties,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct Bounty {
    /// The wallet address of the maintainer who created this bounty.
    /// Only this address may update or cancel the bounty.
    pub creator: Address,
    /// The total reward in raw token units (not human-readable decimals).
    /// For a token with 7 decimal places, a value of `1_000_000_0` equals 1.0 token.
    /// Must be positive (> 0); enforced by `create_bounty`.
    pub reward_amount: i128,
    /// The contract address of the Soroban token used to pay the reward
    /// (e.g. USDC, XLM native asset contract). Must implement the Soroban
    /// token interface (`transfer`, `balance`).
    pub reward_token: Address,
    /// The list of contributors who have claimed this bounty, each paired with
    /// their basis-point share of the reward (shares must sum to 10 000).
    /// Empty while the bounty is `"open"`; populated by `claim_bounty` calls.
    pub assignees: Vec<(Address, u32)>,
    /// Maximum number of contributors allowed to claim this bounty.
    /// Once `assignees.len() == max_assignees` the bounty is no longer claimable.
    pub max_assignees: u32,
    /// Lifecycle state of the bounty. Valid values and transitions:
    /// - `"open"` — initial state after `create_bounty`
    /// - `"in_progress"` — at least one contributor has claimed via `claim_bounty`
    /// - `"completed"` — reward distributed via `complete_bounty`
    /// - `"disputed"` — raised by creator or assignee via `raise_dispute`
    /// - `"cancelled"` — cancelled by creator or expired past deadline
    ///
    /// Exposed in the contract type for off-chain indexers; internal contract
    /// logic derives state from `assignees` presence and explicit status writes.
    #[allow(dead_code)]
    pub status: Symbol,
    /// Minimum reputation score a contributor must hold to claim this bounty.
    /// A value of `0` means any contributor can claim regardless of reputation.
    /// Reputation is a monotonically increasing `u32` (+10 per completed bounty).
    pub min_reputation: u32,
    /// Optional ledger sequence number after which the bounty cannot be claimed.
    /// If set, claim_bounty will reject claims from contributors once the ledger
    /// sequence number exceeds this value.
    pub deadline: Option<u32>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct BountyMeta {
    /// Short human-readable title for the bounty (max 32 chars, Soroban `Symbol` limit).
    pub title: Symbol,
    /// Longer description of the work required to complete the bounty.
    pub description: Symbol,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct Contributor {
    /// The wallet address that identifies this contributor on-chain.
    /// Stored so off-chain consumers can identify the contributor from the
    /// struct alone without keying back through storage.
    #[allow(dead_code)]
    pub address: Address,
    /// Reputation score accumulated through completed bounties.
    /// Incremented by +10 for every successfully completed bounty.
    /// Monotonically increasing — never decreases.
    pub reputation: u32,
    /// Cumulative raw token units earned across all completed bounties.
    /// Uses the same unit as `Bounty::reward_amount` (raw, not human-readable).
    pub total_earned: i128,
    /// Total number of bounties this contributor has successfully completed.
    pub contribution_count: u32,
    /// Number of bounties the contributor has claimed but not yet completed.
    /// Limited to 1 concurrent active claim; `claim_bounty` will panic if > 0.
    pub active_claims: u32,
    /// Optional URI pointing to an off-chain profile document (e.g. an IPFS
    /// link to a JSON file with name, avatar, and GitHub username).
    /// Controlled and updatable by the contributor via `update_contributor_metadata`.
    pub metadata: Option<Symbol>,
}
