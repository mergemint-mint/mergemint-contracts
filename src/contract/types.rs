use soroban_sdk::{contracttype, Address, String};

/// Configuration for the reputation system.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReputationConfig {
    /// Reputation awarded when a bounty is completed successfully.
    pub reward: u32,
    /// Reputation penalty applied when a claimed bounty expires (abandoned claim).
    pub penalty: u32,
}

impl ReputationConfig {
    /// Default configuration used when none has been set in storage.
    pub const fn default_config() -> Self {
        Self {
            reward: 10,
            penalty: 5,
        }
    }
}

/// A contributor profile tracked by the contract.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Profile {
    pub address: Address,
    pub reputation: u32,
    pub name: String,
}

/// Event emitted whenever a profile's reputation changes.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReputationChanged {
    pub address: Address,
    pub old_reputation: u32,
    pub new_reputation: u32,
}

/// Storage keys used by the contract.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataKey {
    /// Global reputation configuration.
    ReputationConfig,
    /// A contributor profile keyed by address.
    Profile(Address),
}
