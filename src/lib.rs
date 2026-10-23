// SPDX-License-Identifier: MIT
#![no_std]
#![allow(dead_code, deprecated, unused_imports, clippy::too_many_arguments)]
// Non-test contract code must raise typed `ContractError`s (via `fail`)
// instead of panicking opaquely. `cfg_attr` keeps this from firing inside
// `#[cfg(test)]` modules, where panics/unwraps are the normal assertion
// idiom.
#![cfg_attr(
    not(test),
    warn(clippy::unwrap_used, clippy::expect_used, clippy::panic)
)]

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

#[cfg(all(test, feature = "fuzz"))]
mod fuzz_tests;
