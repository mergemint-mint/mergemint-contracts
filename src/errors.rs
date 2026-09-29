// SPDX-License-Identifier: MIT

use soroban_sdk::contracterror;

/// Stable numeric error codes surfaced in the contract spec.
///
/// Clients should match on these codes instead of panic strings.
/// The original human-readable messages are preserved as doc comments
/// on each variant for reference.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum ContractError {
    /// "unauthorized"
    Unauthorized = 1,
    /// "not found"
    NotFound = 2,
    /// "invalid input"
    InvalidInput = 3,
    /// "already exists"
    AlreadyExists = 4,
    /// "insufficient balance"
    InsufficientBalance = 5,
    /// "invalid state"
    InvalidState = 6,
    /// "overflow"
    Overflow = 7,
}

/// Raise a contract error with the given code.
///
/// Replaces the previous string-based `fail` helper so that errors are
/// reported through `panic_with_error!` and appear in the contract spec.
pub fn fail(err: ContractError) -> ! {
    soroban_sdk::panic_with_error!(&soroban_sdk::Env::default(), err)
}
