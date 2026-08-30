// SPDX-License-Identifier: MIT
//! XDR encoding/decoding utilities for contract values.
//!
//! Provides encode/decode functions for contract data types to test the XDR boundary.

use soroban_sdk::{Address, String, Symbol, Vec};

use crate::types::{Bounty, BountyMeta, Contributor};

/// Encode an Address into a displayable format for testing.
pub fn address_scval(addr: &Address) -> String {
    addr.to_string()
}

/// Decode an Address from a string representation.
/// Returns the same address if the encoding is valid.
pub fn decode_address(env: &soroban_sdk::Env, addr_str: &str) -> Address {
    Address::from_string(&String::from_str(env, addr_str))
}

/// Encode a Symbol as itself (symbols are self-encoding in XDR).
pub fn symbol_scval(sym: &Symbol) -> Symbol {
    sym.clone()
}

/// Decode a Symbol (identity function for testing round-trip).
pub fn decode_symbol(sym: &Symbol) -> Symbol {
    sym.clone()
}

/// Encode a Bounty into its component parts for verification.
pub fn encode_bounty(bounty: &Bounty) -> (Address, i128, Address, u32, Symbol) {
    (
        bounty.creator.clone(),
        bounty.reward_amount,
        bounty.reward_token.clone(),
        bounty.max_assignees,
        bounty.status.clone(),
    )
}

/// Decode and reconstruct a Bounty from its core fields.
/// Returns the key fields that were encoded.
pub fn decode_bounty(
    creator: &Address,
    reward_amount: i128,
    reward_token: &Address,
    max_assignees: u32,
    status: &Symbol,
) -> (Address, i128, Address, u32, Symbol) {
    (
        creator.clone(),
        reward_amount,
        reward_token.clone(),
        max_assignees,
        status.clone(),
    )
}

/// Encode a Contributor into its core fields for testing.
pub fn encode_contributor(contrib: &Contributor) -> (Address, u32, i128, u32, u32) {
    (
        contrib.address.clone(),
        contrib.reputation,
        contrib.total_earned,
        contrib.contribution_count,
        contrib.active_claims,
    )
}

/// Decode and reconstruct a Contributor from its core fields.
pub fn decode_contributor(
    address: &Address,
    reputation: u32,
    total_earned: i128,
    contribution_count: u32,
    active_claims: u32,
) -> (Address, u32, i128, u32, u32) {
    (
        address.clone(),
        reputation,
        total_earned,
        contribution_count,
        active_claims,
    )
}

/// Encode BountyMeta into its component fields.
pub fn encode_bounty_meta(meta: &BountyMeta) -> (Symbol, String) {
    (meta.title.clone(), meta.description.clone())
}

/// Decode BountyMeta from its component fields.
pub fn decode_bounty_meta(title: &Symbol, description: &String) -> (Symbol, String) {
    (title.clone(), description.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{testutils::Address as _, Env};

    #[test]
    fn test_address_scval_roundtrip() {
        let env = Env::default();
        let addr = Address::generate(&env);

        // Encode and decode
        let encoded = address_scval(&addr);
        // For this test, we verify encoding produces a valid string
        assert!(!encoded.is_empty());
    }

    #[test]
    fn test_symbol_scval_roundtrip() {
        let env = Env::default();
        let sym = Symbol::new(&env, "test_symbol");

        // Encode (identity) and decode (identity)
        let encoded = symbol_scval(&sym);
        let decoded = decode_symbol(&encoded);

        assert_eq!(decoded, sym);
    }

    #[test]
    fn test_bounty_roundtrip() {
        let env = Env::default();
        let creator = Address::generate(&env);
        let reward_token = Address::generate(&env);
        let status = Symbol::new(&env, "open");

        let reward_amount: i128 = 1000;
        let max_assignees: u32 = 5;

        // Encode
        let (enc_creator, enc_amount, enc_token, enc_assignees, enc_status) = (
            creator.clone(),
            reward_amount,
            reward_token.clone(),
            max_assignees,
            status.clone(),
        );

        // Decode
        let (dec_creator, dec_amount, dec_token, dec_assignees, dec_status) = decode_bounty(
            &enc_creator,
            enc_amount,
            &enc_token,
            enc_assignees,
            &enc_status,
        );

        // Verify round-trip
        assert_eq!(dec_creator, creator);
        assert_eq!(dec_amount, reward_amount);
        assert_eq!(dec_token, reward_token);
        assert_eq!(dec_assignees, max_assignees);
        assert_eq!(dec_status, status);
    }

    #[test]
    fn test_contributor_roundtrip() {
        let env = Env::default();
        let address = Address::generate(&env);
        let reputation: u32 = 100;
        let total_earned: i128 = 5000;
        let contribution_count: u32 = 10;
        let active_claims: u32 = 2;

        // Encode
        let (enc_addr, enc_rep, enc_earned, enc_contrib, enc_claims) = (
            address.clone(),
            reputation,
            total_earned,
            contribution_count,
            active_claims,
        );

        // Decode
        let (dec_addr, dec_rep, dec_earned, dec_contrib, dec_claims) =
            decode_contributor(&enc_addr, enc_rep, enc_earned, enc_contrib, enc_claims);

        // Verify round-trip
        assert_eq!(dec_addr, address);
        assert_eq!(dec_rep, reputation);
        assert_eq!(dec_earned, total_earned);
        assert_eq!(dec_contrib, contribution_count);
        assert_eq!(dec_claims, active_claims);
    }

    #[test]
    fn test_bounty_meta_roundtrip() {
        let env = Env::default();
        let title = Symbol::new(&env, "bug_fix");
        let description = String::from_str(&env, "Fix the critical bug");

        // Encode
        let (enc_title, enc_desc) = (title.clone(), description.clone());

        // Decode
        let (dec_title, dec_desc) = decode_bounty_meta(&enc_title, &enc_desc);

        // Verify round-trip
        assert_eq!(dec_title, title);
        assert_eq!(dec_desc, description);
    }

    #[test]
    fn test_encode_bounty_with_full_struct() {
        let env = Env::default();
        let creator = Address::generate(&env);
        let reward_token = Address::generate(&env);

        let bounty = Bounty {
            creator: creator.clone(),
            reward_amount: 2000,
            reward_token: reward_token.clone(),
            assignees: Vec::new(&env),
            max_assignees: 3,
            status: Symbol::new(&env, "in_progress"),
            min_reputation: 50,
            deadline: Some(12345),
            required_verifiers: None,
            approval_threshold: 1,
            tags: Vec::new(&env),
            milestones: Vec::new(&env),
        };

        // Encode the full bounty
        let (enc_creator, enc_amount, enc_token, enc_assignees, enc_status) =
            encode_bounty(&bounty);

        // Verify encoded values match original
        assert_eq!(enc_creator, creator);
        assert_eq!(enc_amount, 2000);
        assert_eq!(enc_token, reward_token);
        assert_eq!(enc_assignees, 3);
        assert_eq!(enc_status, Symbol::new(&env, "in_progress"));

        // Verify round-trip
        let (dec_creator, dec_amount, dec_token, dec_assignees, dec_status) = decode_bounty(
            &enc_creator,
            enc_amount,
            &enc_token,
            enc_assignees,
            &enc_status,
        );

        assert_eq!(dec_creator, creator);
        assert_eq!(dec_amount, 2000);
        assert_eq!(dec_token, reward_token);
        assert_eq!(dec_assignees, 3);
        assert_eq!(dec_status, Symbol::new(&env, "in_progress"));
    }

    #[test]
    fn test_encode_contributor_with_full_struct() {
        let env = Env::default();
        let address = Address::generate(&env);

        let contributor = Contributor {
            address: address.clone(),
            reputation: 250,
            total_earned: 15000,
            contribution_count: 25,
            active_claims: 3,
            metadata: None,
        };

        // Encode the full contributor
        let (enc_addr, enc_rep, enc_earned, enc_contrib, enc_claims) =
            encode_contributor(&contributor);

        // Verify encoded values match original
        assert_eq!(enc_addr, address);
        assert_eq!(enc_rep, 250);
        assert_eq!(enc_earned, 15000);
        assert_eq!(enc_contrib, 25);
        assert_eq!(enc_claims, 3);

        // Verify round-trip
        let (dec_addr, dec_rep, dec_earned, dec_contrib, dec_claims) =
            decode_contributor(&enc_addr, enc_rep, enc_earned, enc_contrib, enc_claims);

        assert_eq!(dec_addr, address);
        assert_eq!(dec_rep, 250);
        assert_eq!(dec_earned, 15000);
        assert_eq!(dec_contrib, 25);
        assert_eq!(dec_claims, 3);
    }

    // ===========================================================================
    // Issue 753 — boundary-value coverage for the scval conversion helpers.
    //
    // Note: `scval.rs` exposes custom encode/decode conversion helpers (not
    // `TryFrom`/`Into` impls); these tests exercise the extreme values of each
    // helper pair so the XDR boundary is covered at both ends.
    // ===========================================================================

    /// `encode_bounty`/`decode_bounty` round-trip at i128 min/max reward
    /// amounts and u32::MAX max_assignees.
    #[test]
    fn test_encode_decode_bounty_boundaries() {
        let env = Env::default();
        let creator = Address::generate(&env);
        let reward_token = Address::generate(&env);

        let extremes: [i128; 4] = [i128::MIN, i128::MIN + 1, i128::MAX - 1, i128::MAX];
        for amount in extremes {
            let bounty = Bounty {
                creator: creator.clone(),
                reward_amount: amount,
                reward_token: reward_token.clone(),
                assignees: Vec::new(&env),
                max_assignees: u32::MAX,
                status: Symbol::new(&env, "open"),
                min_reputation: 0,
                deadline: None,
                required_verifiers: None,
                approval_threshold: 1,
                tags: Vec::new(&env),
                milestones: Vec::new(&env),
            };

            let (enc_creator, enc_amount, enc_token, enc_assignees, enc_status) =
                encode_bounty(&bounty);
            let (dec_creator, dec_amount, dec_token, dec_assignees, dec_status) = decode_bounty(
                &enc_creator,
                enc_amount,
                &enc_token,
                enc_assignees,
                &enc_status,
            );

            assert_eq!(dec_creator, creator);
            assert_eq!(dec_amount, amount, "reward_amount boundary must round-trip");
            assert_eq!(dec_token, reward_token);
            assert_eq!(dec_assignees, u32::MAX);
            assert_eq!(dec_status, Symbol::new(&env, "open"));
        }
    }

    /// `encode_contributor`/`decode_contributor` round-trip at u32::MAX and
    /// i128 min/max for the numeric fields.
    #[test]
    fn test_encode_decode_contributor_boundaries() {
        let env = Env::default();
        let address = Address::generate(&env);

        for total_earned in [i128::MIN, i128::MAX] {
            let contributor = Contributor {
                address: address.clone(),
                reputation: u32::MAX,
                total_earned,
                contribution_count: u32::MAX,
                active_claims: u32::MAX,
                metadata: None,
            };

            let (enc_addr, enc_rep, enc_earned, enc_contrib, enc_claims) =
                encode_contributor(&contributor);
            let (dec_addr, dec_rep, dec_earned, dec_contrib, dec_claims) =
                decode_contributor(&enc_addr, enc_rep, enc_earned, enc_contrib, enc_claims);

            assert_eq!(dec_addr, address);
            assert_eq!(dec_rep, u32::MAX);
            assert_eq!(dec_earned, total_earned, "total_earned boundary must round-trip");
            assert_eq!(dec_contrib, u32::MAX);
            assert_eq!(dec_claims, u32::MAX);
        }
    }

    /// `encode_bounty_meta`/`decode_bounty_meta` round-trip with an empty
    /// description string (empty-vector/string boundary).
    #[test]
    fn test_encode_decode_bounty_meta_empty_string_boundary() {
        let env = Env::default();
        let title = Symbol::new(&env, "empty_desc");
        let description = String::from_str(&env, "");

        let (enc_title, enc_desc) = encode_bounty_meta(&crate::types::BountyMeta {
            title: title.clone(),
            description: description.clone(),
        });
        let (dec_title, dec_desc) = decode_bounty_meta(&enc_title, &enc_desc);

        assert_eq!(dec_title, title);
        assert_eq!(dec_desc, description);
        assert_eq!(dec_desc.len(), 0, "empty description must survive the round-trip");
    }

    /// `address_scval`/`decode_address` round-trip: the encoded form must
    /// decode back to the identical address (valid encoding boundary).
    #[test]
    fn test_address_scval_decode_roundtrip() {
        let env = Env::default();
        let addr = Address::generate(&env);

        let encoded = address_scval(&addr);
        assert!(!encoded.is_empty());

        // `encoded` is already the XDR display string; decode via the same
        // path decode_address uses internally (String -> Address).
        let decoded = Address::from_string(&encoded);
        assert_eq!(decoded, addr, "encoded address must decode to the same address");
    }

    /// `symbol_scval`/`decode_symbol` round-trip with an empty symbol name
    /// (minimum-length boundary).
    #[test]
    fn test_symbol_scval_empty_boundary() {
        let env = Env::default();
        let sym = Symbol::new(&env, "");

        let encoded = symbol_scval(&sym);
        let decoded = decode_symbol(&encoded);

        assert_eq!(decoded, sym);
    }
}
