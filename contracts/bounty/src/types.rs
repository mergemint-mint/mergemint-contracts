use soroban_sdk::{contracttype, Symbol, String};

/// Maximum length (in bytes) of a `BountyMeta` description.
///
/// `description` is stored as a UTF-8 `String` (not a Soroban `Symbol`),
/// so it is bounded by a byte length rather than the 32-char symbol cap.
pub const DESCRIPTION_MAX_LENGTH: u32 = 280;

/// Maximum length (in characters) of a `BountyMeta` title `Symbol`.
pub const TITLE_MAX_LENGTH: u32 = 32;

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BountyMeta {
    /// Short identifier stored as a Soroban `Symbol` (<= 32 chars).
    pub title: Symbol,
    /// Longer free-form text stored as a UTF-8 `String`.
    pub description: String,
}

impl BountyMeta {
    /// Returns `true` when `description` is within the allowed UTF-8 byte length.
    pub fn is_valid_description(description: &String) -> bool {
        description.len() <= DESCRIPTION_MAX_LENGTH
    }
}
