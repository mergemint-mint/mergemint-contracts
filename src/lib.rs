// SPDX-License-Identifier: MIT
#![no_std]
#![allow(dead_code, deprecated, unused_imports, clippy::too_many_arguments)]

mod contract;
mod errors;
mod events;
mod scval;
mod storage;
mod symbols;
mod types;

pub use crate::errors::*;

pub use contract::MergeMintContractClient;

#[cfg(test)]
mod test;
