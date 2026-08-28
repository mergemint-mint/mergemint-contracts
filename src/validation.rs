// SPDX-License-Identifier: MIT
use soroban_sdk::{Env, Symbol};

use crate::errors::{fail, ContractError};

/// Identifies which allow-list a raw `Symbol` value must be checked against.
///
/// Shared by `queries.rs` and `mutations.rs` so both modules validate
/// caller-supplied `Symbol` values (status, dispute resolution) the same way
/// instead of each maintaining its own ad-hoc comparison list.
pub enum SymbolKind {
    /// A `Bounty.status` value: `"open"`, `"in_progress"`, `"completed"`,
    /// `"cancelled"`, or `"disputed"`.
    Status,
    /// A `resolve_dispute` resolution value: `"complete"` or `"cancel"`.
    Resolution,
}

impl SymbolKind {
    fn allowed_values(&self) -> &'static [&'static str] {
        match self {
            SymbolKind::Status => &["open", "in_progress", "completed", "cancelled", "disputed"],
            SymbolKind::Resolution => &["complete", "cancel"],
        }
    }
}

/// Panics with `ContractError::InvalidSymbolValue` unless `value` matches one
/// of the allowed values for `kind`.
pub fn validate_symbol(env: &Env, kind: SymbolKind, value: &Symbol) {
    let is_allowed = kind
        .allowed_values()
        .iter()
        .any(|allowed| Symbol::new(env, allowed) == *value);

    if !is_allowed {
        fail(ContractError::InvalidSymbolValue);
    }
}
