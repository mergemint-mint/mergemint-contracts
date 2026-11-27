use soroban_sdk::{contracttype, Address, Env};

/// Event emitted when a new bounty is created.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BountyCreated {
    pub bounty_id: u64,
    pub creator: Address,
    pub reward: i128,
    pub deadline: Option<u64>,
}

/// Event emitted when a bounty is claimed by a contributor.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BountyClaimed {
    pub bounty_id: u64,
    pub contributor: Address,
}

/// Event emitted when a bounty is completed and paid out.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BountyCompleted {
    pub bounty_id: u64,
    pub contributor: Address,
    pub reward: i128,
}

/// Event emitted when a bounty is cancelled by its creator.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BountyCancelled {
    pub bounty_id: u64,
    pub creator: Address,
}

/// Event emitted when a bounty's deadline is extended by its creator.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeadlineExtended {
    pub bounty_id: u64,
    pub creator: Address,
    pub old_deadline: u64,
    pub new_deadline: u64,
}

/// Emit a `DeadlineExtended` event so indexers and the UI can refresh
/// countdowns without polling.
pub fn emit_deadline_extended(
    env: &Env,
    bounty_id: u64,
    creator: Address,
    old_deadline: u64,
    new_deadline: u64,
) {
    let event = DeadlineExtended {
        bounty_id,
        creator,
        old_deadline,
        new_deadline,
    };
    env.events().publish(("deadline_extended", bounty_id), event);
}
