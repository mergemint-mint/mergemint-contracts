// SPDX-License-Identifier: MIT
use soroban_sdk::{contracttype, Address, BytesN, Symbol, Vec};

// #274: status string constants co-located with the Bounty type
pub const STATUS_OPEN: &str = "open";
pub const STATUS_IN_PROGRESS: &str = "in_progress";
pub const STATUS_COMPLETED: &str = "completed";
pub const STATUS_CANCELLED: &str = "cancelled";
pub const STATUS_DISPUTED: &str = "disputed";

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub enum DataKey {
    BountyCount,
    Bounty(BytesN<32>),
    BountyMeta(BytesN<32>),
    Contributor(Address),
    StatusIndex(Symbol),
    OpenBounties,
    ContributorIndex, // #271: ordered list of all contributor addresses
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
    pub verifier: Option<Address>, // #264: optional designated verifier
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct BountyMeta {
    pub title: Symbol,
    pub description: Symbol,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct Contributor {
    pub address: Address,
    pub reputation: u32,
    pub total_earned: i128,
    pub contribution_count: u32,
    pub active_claims: u32,
    pub metadata: Option<Symbol>,
}
