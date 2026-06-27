use soroban_sdk::{
    contract, contractimpl, token::TokenClient, Address, BytesN, Env, Symbol,
};

use crate::events;
use crate::storage;
use crate::types::{Bounty, Contributor};

const STATUS_OPEN: &str = "open";
const STATUS_IN_PROGRESS: &str = "in_progress";

fn generate_bounty_id(env: &Env) -> BytesN<32> {
    let count = storage::get_bounty_count(env);
    let mut buf = [0u8; 32];
    let count_bytes = count.to_be_bytes();
    buf[24..32].copy_from_slice(&count_bytes);
    BytesN::from_array(env, &buf)
}

#[contract]
pub struct MergeMintContract;

include!("mutations.rs");
include!("queries.rs");
