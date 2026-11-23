// SPDX-License-Identifier: MIT
//! UTF-8 / symbol-length / special-character round-trip tests for
//! `BountyMeta` through `create_bounty`, `get_bounty_metas`, `get_bounties_by_ids`.
extern crate std;

use crate::contract::MergeMintContract;
use crate::types::BountyId;
use crate::MergeMintContractClient;
use proptest::prelude::*;
use soroban_sdk::{testutils::Address as _, vec, Address, Env, String, Symbol, Vec};
use std::string::String as StdString;
use std::vec::Vec as StdVec;

fn roundtrip(title: &str, desc: &str) {
    let env = Env::default();
    env.mock_all_auths();
    let id = env.register(MergeMintContract, ());
    let client = MergeMintContractClient::new(&env, &id);
    let creator = Address::generate(&env);
    let token = env.register_stellar_asset_contract_v2(creator.clone()).address();
    let bid: BountyId = client.create_bounty(
        &creator,
        &Symbol::new(&env, title),
        &String::from_str(&env, desc),
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
    let ids = vec![&env, bid];
    let metas = client.get_bounty_metas(&ids);
    let meta = metas.get(0).unwrap().expect("meta present");
    assert_eq!(meta.title, Symbol::new(&env, title));
    assert_eq!(meta.description.len() as usize, desc.len());
    let mut buf = std::vec![0u8; desc.len()];
    meta.description.copy_into_slice(&mut buf);
    assert_eq!(buf.as_slice(), desc.as_bytes(), "byte-for-byte description");
    let by_ids = client.get_bounties_by_ids(&ids);
    assert_eq!(by_ids.len(), 1);
    assert_eq!(by_ids.get(0).unwrap(), meta);
}

#[test]
fn description_edge_cases_roundtrip() {
    let long = "x".repeat(4096);
    let cases: StdVec<&str> = std::vec![
        "",
        " ",
        "plain ascii",
        "caf\u{e9} \u{4e2d}\u{6587} \u{3053}\u{3093}\u{306b}\u{3061}\u{306f}",
        "emoji \u{1F600}\u{1F680}\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}",
        "ctrl \u{0}\u{1}\u{7}\u{8}\t\r\n\u{1b}[0m \u{7f}",
        "rtl \u{202e}abc\u{202c} zwj\u{200b} bom\u{feff}",
        "quotes \"'`<>&;{}[]\\/ %s %d",
        &long,
    ];
    for d in cases {
        roundtrip("edge", d);
    }
}

#[test]
fn symbol_length_boundaries_roundtrip() {
    for len in [1usize, 2, 31, 32] {
        let t: StdString = "aZ_9".chars().cycle().take(len).collect();
        roundtrip(&t, "d");
    }
    roundtrip("_", "d");
    roundtrip("0", "d");
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(24))]
    #[test]
    fn arbitrary_utf8_description_and_valid_symbol_roundtrip(
        title in "[a-zA-Z0-9_]{1,32}",
        desc in any::<StdString>().prop_map(|s| s.chars().take(512).collect::<StdString>()),
    ) {
        roundtrip(&title, &desc);
    }
}
