use soroban_sdk::{contracttype, Address, Vec};

/// Status of a bounty in its lifecycle.
#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BountyStatus {
    Open,
    InProgress,
    Completed,
    Cancelled,
}

/// A bounty that can be claimed by up to `max_assignees` contributors.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Bounty {
    pub id: u64,
    pub creator: Address,
    pub reward: i128,
    pub status: BountyStatus,
    /// Maximum number of contributors that may claim this bounty.
    pub max_assignees: u32,
    /// Contributors that have claimed this bounty, in claim order.
    pub assignees: Vec<Address>,
}

impl Bounty {
    /// Returns true when the bounty has reached its assignee capacity.
    pub fn is_full(&self) -> bool {
        self.assignees.len() >= self.max_assignees
    }

    /// Returns true when `address` has already claimed this bounty.
    pub fn has_assignee(&self, address: &Address) -> bool {
        self.assignees.iter().any(|a| a == *address)
    }

    /// Splits `reward` evenly across all assignees.
    ///
    /// Returns the per-assignee share and the rounding dust (remainder).
    /// The dust is returned explicitly so callers can account for it and
    /// keep escrow balanced instead of silently losing it to integer division.
    pub fn split_reward(&self) -> (i128, i128) {
        let count = self.assignees.len() as i128;
        if count == 0 {
            return (0, self.reward);
        }
        let share = self.reward / count;
        let dust = self.reward - share * count;
        (share, dust)
    }
}
