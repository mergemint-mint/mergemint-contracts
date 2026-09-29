use soroban_sdk::{
    contract, contractimpl, token::TokenClient, Address, BytesN, Env, String, Symbol, Vec,
};

use crate::errors;
use crate::errors::{fail, ContractError};
use crate::events;
use crate::storage;
use crate::types::{Bounty, BountyId, BountyMeta, Contributor, Dispute, Milestone};

/// Maximum protocol fee, expressed in basis points (10 percent).
pub const MAX_FEE_BPS: u32 = 1000;

#[contract]
pub struct MergeMintContract;

include!("lifecycle.rs");
include!("disputes.rs");
include!("milestones.rs");
include!("queries.rs");

#[contractimpl]
impl MergeMintContract {
    /// Returns a page of bounties associated with `tag`.
    ///
    /// Invalid tags fail with `InvalidTag`, matching `create_bounty` validation.
    pub fn get_bounties_by_tag(
        env: Env,
        tag: Symbol,
        offset: u32,
        limit: u32,
    ) -> Result<Vec<Bounty>, ContractError> {
        if !storage::is_valid_tag(&env, &tag) {
            fail(&env, ContractError::InvalidTag);
        }
        Ok(storage::get_bounties_by_tag(&env, &tag, offset, limit))
    }

    /// Returns the total number of bounties associated with `tag`.
    ///
    /// Invalid tags fail with `InvalidTag`, matching `create_bounty` validation.
    pub fn get_tag_count(env: Env, tag: Symbol) -> Result<u32, ContractError> {
        if !storage::is_valid_tag(&env, &tag) {
            fail(&env, ContractError::InvalidTag);
        }
        Ok(storage::get_tag_count(&env, &tag))
    }

    /// Replaces the tags on an Open bounty. Callable only by the bounty creator.
    ///
    /// Stale tag index entries are removed and new ones added so that
    /// `get_bounties_by_tag` stays consistent. The `TooManyTags` limit still
    /// applies, and a `BountyTagsUpdated` event is emitted for the indexer.
    pub fn update_tags(
        env: Env,
        creator: Address,
        bounty_id: BountyId,
        tags: Vec<Symbol>,
    ) -> Result<(), ContractError> {
        mutations::update_tags(env, creator, bounty_id, tags)
    }
}
