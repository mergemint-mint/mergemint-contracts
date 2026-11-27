use soroban_sdk::{contracttype, Address, String, Vec};

/// Status of a bounty.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BountyStatus {
    Open,
    Claimed,
    Submitted,
    Completed,
    Cancelled,
}

/// A bounty created by a creator.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Bounty {
    pub id: u64,
    pub creator: Address,
    pub title: String,
    pub description: String,
    pub reward: i128,
    pub status: BountyStatus,
    pub deadline: u64,
    pub claimant: Option<Address>,
    /// Minimum reputation a contributor must have to claim this bounty.
    /// Defaults to zero for backward compatibility.
    pub min_reputation: u32,
}

/// A claim on a bounty by a contributor.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Claim {
    pub bounty_id: u64,
    pub contributor: Address,
    pub submitted_at: u64,
}

/// Persistent contract state.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContractState {
    pub bounties: Vec<Bounty>,
    pub claims: Vec<Claim>,
    pub next_bounty_id: u64,
}

impl ContractState {
    /// Create a new, empty contract state.
    pub fn new() -> Self {
        Self {
            bounties: Vec::new(),
            claims: Vec::new(),
            next_bounty_id: 1,
        }
    }
}
