// SPDX-License-Identifier: MIT
use soroban_sdk::{contracttype, Address, BytesN, Symbol, Vec};

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub enum DataKey {
    BountyCount,
    Bounty(BytesN<32>),
    BountyMeta(BytesN<32>),
    Contributor(Address),
    StatusIndex(Symbol),
    OpenBounties,
    /// Stores the list of bounty IDs created by a specific address.
    CreatorBounties(Address),
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct Bounty {
    pub creator: Address,
    pub reward_amount: i128,
    pub reward_token: Address,
    pub assignees: Vec<(Address, u32)>,
    pub max_assignees: u32,
    #[allow(dead_code)]
    pub status: Symbol,
    pub min_reputation: u32,
    pub deadline: Option<u32>,
    /// Categorisation tags (max 5). Stored on-chain and readable via get_bounty.
    pub tags: Vec<Symbol>,
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
    #[allow(dead_code)]
    pub address: Address,
    pub reputation: u32,
    pub total_earned: i128,
    pub contribution_count: u32,
    pub active_claims: u32,
    pub metadata: Option<Symbol>,
}
