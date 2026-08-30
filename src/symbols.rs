// SPDX-License-Identifier: MIT
use soroban_sdk::{Env, Symbol};

use crate::errors::{fail, ContractError};

pub const STATUS_OPEN: &str = "open";
pub const STATUS_IN_PROGRESS: &str = "in_progress";
pub const STATUS_COMPLETED: &str = "completed";
pub const STATUS_CANCELLED: &str = "cancelled";
pub const STATUS_DISPUTED: &str = "disputed";

const ALLOWED_STATUSES: &[&str] = &[
    STATUS_OPEN,
    STATUS_IN_PROGRESS,
    STATUS_COMPLETED,
    STATUS_CANCELLED,
    STATUS_DISPUTED,
];

/// Tags accepted by `create_bounty` and `get_bounties_by_tag`.
const ALLOWED_TAGS: &[&str] = &[
    "bug", "docs", "feature", "security", "test", "refactor", "design", "chore", "perf", "other",
];

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum SymbolKind {
    Status,
    Tag,
}

fn symbol_is(env: &Env, value: &Symbol, expected: &str) -> bool {
    *value == Symbol::new(env, expected)
}

/// Returns `Ok(())` when `value` is on the allow-list for `kind`.
pub fn validate_symbol(env: &Env, kind: SymbolKind, value: &Symbol) -> Result<(), ContractError> {
    let allowed = match kind {
        SymbolKind::Status => ALLOWED_STATUSES,
        SymbolKind::Tag => ALLOWED_TAGS,
    };
    if allowed
        .iter()
        .any(|candidate| symbol_is(env, value, candidate))
    {
        Ok(())
    } else {
        Err(match kind {
            SymbolKind::Status => ContractError::InvalidStatus,
            SymbolKind::Tag => ContractError::InvalidTag,
        })
    }
}

/// Panics with the canonical `ContractError` message when validation fails.
pub fn validate_symbol_or_fail(env: &Env, kind: SymbolKind, value: &Symbol) {
    if let Err(error) = validate_symbol(env, kind, value) {
        fail(error);
    }
}
