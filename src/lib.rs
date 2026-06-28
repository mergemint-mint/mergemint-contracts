// SPDX-License-Identifier: MIT
#![no_std]

mod contract;
pub mod errors;
mod events;
mod storage;
mod types;

pub use contract::MergeMintContractClient;

#[cfg(test)]
mod test;
