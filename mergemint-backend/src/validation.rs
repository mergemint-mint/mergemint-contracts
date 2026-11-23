// mergemint-backend/src/validation.rs
//
// Input validation rules that mirror `app/src/lib/validation.ts` on the
// frontend. Both sides enforce the same reward-amount, title-symbol, and
// description-length rules independently (the frontend for fast UI feedback,
// the backend as the authoritative check before a bounty is persisted);
// `validation-parity.json` at the repo root is the shared fixture the
// `#[cfg(test)]` module below uses to assert the two never drift apart.

/// On-chain `BountyMeta.title` is stored as a Soroban `Symbol`, which caps out
/// at 32 characters. Mirrors `SYMBOL_MAX_LENGTH` in
/// `app/src/lib/validation.ts`.
pub const SYMBOL_MAX_LENGTH: usize = 32;

/// `BountyMeta.description` is a UTF-8 `String` (not a `Symbol`), so it is
/// bounded by a character count rather than the 32-char symbol cap. Mirrors
/// `DESCRIPTION_MAX_LENGTH` in `app/src/lib/validation.ts`.
pub const DESCRIPTION_MAX_LENGTH: usize = 280;

/// Mirrors the frontend's `REWARD_AMOUNT_REGEX` (`^\d+(\.\d{1,7})?$`) plus the
/// `parseFloat(value) > 0` strictly-positive check.
pub fn is_valid_reward_amount(value: &str) -> bool {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return false;
    }

    let (whole, fraction) = match trimmed.split_once('.') {
        Some((whole, fraction)) => (whole, Some(fraction)),
        None => (trimmed, None),
    };

    if whole.is_empty() || !whole.bytes().all(|b| b.is_ascii_digit()) {
        return false;
    }

    if let Some(fraction) = fraction {
        if fraction.is_empty()
            || fraction.len() > 7
            || !fraction.bytes().all(|b| b.is_ascii_digit())
        {
            return false;
        }
    }

    // Strictly positive — reject "0" and "0.0000000".
    let whole_is_zero = whole.bytes().all(|b| b == b'0');
    let fraction_is_zero = fraction.is_none_or(|f| f.bytes().all(|b| b == b'0'));
    !(whole_is_zero && fraction_is_zero)
}

/// Mirrors the frontend's `isValidTitle`: a Soroban `Symbol` must be 1–32
/// characters drawn from `[a-zA-Z0-9_]`.
pub fn is_valid_title(value: &str) -> bool {
    let trimmed = value.trim();
    !trimmed.is_empty()
        && trimmed.chars().count() <= SYMBOL_MAX_LENGTH
        && trimmed
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

/// Mirrors the frontend's `isValidDescription`: a UTF-8 `String` that is
/// non-empty once trimmed and no longer than [`DESCRIPTION_MAX_LENGTH`]
/// characters (counted as Unicode scalar values, not bytes).
pub fn is_valid_description(value: &str) -> bool {
    let trimmed = value.trim();
    !trimmed.is_empty() && trimmed.chars().count() <= DESCRIPTION_MAX_LENGTH
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Debug, Deserialize)]
    struct Case {
        value: String,
        valid: bool,
        #[allow(dead_code)]
        reason: String,
    }

    #[derive(Debug, Deserialize)]
    struct Fixture {
        #[serde(rename = "rewardAmount")]
        reward_amount: Vec<Case>,
        #[serde(rename = "title")]
        title: Vec<Case>,
        #[serde(rename = "description")]
        description: Vec<Case>,
    }

    fn load_fixture() -> Fixture {
        // Path is relative to this file, resolved at compile time, so the
        // fixture travels with the crate regardless of the current working
        // directory `cargo test` is invoked from.
        let raw = include_str!("../../fixtures/validation-parity.json");
        serde_json::from_str(raw).expect("fixtures/validation-parity.json must be valid JSON")
    }

    #[test]
    fn reward_amount_matches_fixture() {
        let fixture = load_fixture();
        for case in &fixture.reward_amount {
            assert_eq!(
                is_valid_reward_amount(&case.value),
                case.valid,
                "reward amount {:?} expected valid={} ({}), backend disagreed",
                case.value,
                case.valid,
                case.reason
            );
        }
    }

    #[test]
    fn title_matches_fixture() {
        let fixture = load_fixture();
        for case in &fixture.title {
            assert_eq!(
                is_valid_title(&case.value),
                case.valid,
                "title {:?} expected valid={} ({}), backend disagreed",
                case.value,
                case.valid,
                case.reason
            );
        }
    }

    #[test]
    fn description_matches_fixture() {
        let fixture = load_fixture();
        for case in &fixture.description {
            assert_eq!(
                is_valid_description(&case.value),
                case.valid,
                "description {:?} expected valid={} ({}), backend disagreed",
                case.value,
                case.valid,
                case.reason
            );
        }
    }
}
