// SPDX-License-Identifier: MIT
//! Ledger-sequence boundary and TTL expiration tests (#1238).
//!
//! Audit note: `env.ledger().sequence()` is read in `contract/mutations.rs`
//! (deadlines, claim cooldown, stale claims, appeal windows) and in
//! `storage.rs` (`record_unclaim`). These tests exercise the boundary ledger
//! numbers through `testutils::Ledger`.

use crate::contract::MergeMintContract;
use crate::types::{BountyId, DataKey};
use crate::MergeMintContractClient;
use soroban_sdk::{
    testutils::{storage::Persistent as _, Address as _, Ledger as _},
    token::StellarAssetClient,
    Address, Env, String, Symbol, Vec,
};

const STORAGE_TTL_LEDGERS: u32 = 6_307_200;
const STORAGE_TTL_THRESHOLD: u32 = STORAGE_TTL_LEDGERS / 2;

fn setup() -> (Env, Address, MergeMintContractClient<'static>, Address, Address, BountyId) {
    let env = Env::default();
    env.mock_all_auths();
    let creator = Address::generate(&env);
    let contributor = Address::generate(&env);
    let contract_id = env.register(MergeMintContract, ());
    let client = MergeMintContractClient::new(&env, &contract_id);
    let token = env.register_stellar_asset_contract_v2(creator.clone()).address();
    StellarAssetClient::new(&env, &token).mint(&contract_id, &1000);
    let id = client.create_bounty(
        &creator,
        &Symbol::new(&env, "ttl"),
        &String::from_str(&env, "desc"),
        &1000,
        &token,
        &0,
        &None,
        &Vec::new(&env),
        &1,
        &None,
        &1,
        &Vec::new(&env),
    );
    (env, contract_id, client, creator, contributor, id)
}

/// `UnclaimedAt` lives in temporary storage and must expire on its own once
/// the configured cooldown has elapsed.
#[test]
fn unclaimed_at_temporary_entry_expires_after_cooldown() {
    let (env, contract_id, client, creator, contributor, id) = setup();
    client.set_claim_cooldown(&creator, &10);
    client.claim_bounty(&contributor, &id);
    client.unclaim_bounty(&contributor, &id);

    let key = DataKey::UnclaimedAt(id.clone(), contributor.clone());
    let present = |env: &Env| {
        env.as_contract(&contract_id, || env.storage().temporary().has(&key))
    };
    assert!(present(&env), "entry must exist right after unclaim");

    let now = env.ledger().sequence();
    env.ledger().set_sequence_number(now + 5);
    assert!(present(&env), "entry must survive inside the cooldown");

    env.ledger().set_sequence_number(now + 11);
    assert!(!present(&env), "entry must auto-expire after the cooldown");
}

/// Persistent entries are re-armed to a full TTL once their remaining life
/// drops below `STORAGE_TTL_THRESHOLD`.
#[test]
fn persistent_ttl_is_refreshed_past_threshold() {
    let (env, contract_id, client, _creator, contributor, id) = setup();
    client.claim_bounty(&contributor, &id);
    let key = DataKey::Contributor(contributor.clone());

    env.ledger().set_sequence_number(STORAGE_TTL_THRESHOLD + 1);
    assert!(client.get_contributor(&contributor).is_some());
    let ttl = env.as_contract(&contract_id, || env.storage().persistent().get_ttl(&key));
    assert!(
        ttl > STORAGE_TTL_THRESHOLD,
        "touching the entry past the threshold must refresh its TTL"
    );
}

/// Long-lived bounties stay readable across repeated advance-and-touch cycles.
#[test]
fn long_lived_bounty_survives_repeated_refresh() {
    let (env, _contract_id, client, _creator, _contributor, id) = setup();
    let mut seq = 0u32;
    for _ in 0..4 {
        seq += STORAGE_TTL_THRESHOLD + 1;
        env.ledger().set_sequence_number(seq);
        assert!(client.get_bounty(&id).is_some());
    }
}

/// Read-only entrypoints never trap at the extreme sequence numbers.
#[test]
fn boundary_sequence_numbers_do_not_trap_on_reads() {
    let (env, _contract_id, client, _creator, _contributor, _id) = setup();
    for seq in [0u32, 1, STORAGE_TTL_THRESHOLD, STORAGE_TTL_LEDGERS, u32::MAX - 1, u32::MAX] {
        env.ledger().set_sequence_number(seq);
        assert_eq!(client.version(), String::from_str(&env, "1.0.0"));
    }
}
