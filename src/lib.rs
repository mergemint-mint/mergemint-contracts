// SPDX-License-Identifier: MIT
#![no_std]

mod contract;
mod errors;
mod events;
mod storage;
mod types;

pub use crate::errors::*;

pub use contract::MergeMintContractClient;

#[cfg(test)]
mod test;
