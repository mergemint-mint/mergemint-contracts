# Emergency-Pause and Upgrade Strategy

**Status:** Proposal — follow-up implementation ticket required before merging any code.
**Date:** 2026-07-29

**Implementation status (confirmed against current source):** Nothing in this
document has been implemented yet. There is no `DataKey::Admin`, `DataKey::Paused`,
`init`/`pause`/`unpause`/`upgrade` function, or `assert_not_paused` guard anywhere
in `src/`. Every mutating entrypoint in `src/contract/mutations.rs` runs
unconditionally once `require_auth` passes. This remains a design proposal only —
none of Phase 1 (Option A) or Phase 2 (Option B) below has landed, and none of the
three follow-up tickets listed at the bottom of this doc has been opened yet.

---

## Problem

The deployed MergeMint contract has no circuit-breaker. If a critical bug is found
post-deployment the only current remediation is deploying a new contract, migrating
state off-chain, and updating every integration to the new address. That is slow,
expensive, and disruptive.

This note evaluates two options and recommends an approach.

---

## Option A — Admin-gated pause flag

### Mechanism

1. Store a `paused: bool` flag under a new `DataKey::Paused` key.
2. Add a small helper `assert_not_paused(env: &Env)` that reads the flag and panics
   with `"contract is paused"` if set.
3. Call `assert_not_paused` at the top of every state-mutating function, immediately
   after `require_auth` (which must remain the very first statement per the auth-
   placement rule in `docs/security.md`).
4. Add two admin-only functions — `pause(admin)` and `unpause(admin)` — that gate
   writes to the flag.
5. Store an `admin: Address` at deploy time via a one-shot `init(admin)` function.

### Pros

- Simple to reason about: one boolean, one guard, two functions.
- Zero overhead for callers when the contract is not paused — single storage lookup.
- Predictable: any observer can query the flag to know whether the contract is live.
- No Soroban-specific complexity; works with the current SDK today.

### Cons

- The admin key is a single point of failure: if lost or compromised the contract can
  be permanently paused or never paused.
- Coarse-grained: pauses all mutating functions at once. A bitmask flag per function
  would allow partial pausing at the cost of more complexity.
- Escrowed funds are locked while paused. Mitigate by adding an
  `emergency_withdraw(admin, bounty_id)` path that operates even when paused.
- Does not fix the bug — only stops the bleeding. A redeploy is still needed to
  remediate the underlying issue.

### Implementation sketch

```rust
// In types.rs — new DataKey variants
DataKey::Admin,
DataKey::Paused,

// In mutations.rs
pub fn init(env: Env, admin: Address) {
    if storage::get_admin(&env).is_some() {
        panic!("already initialised");
    }
    admin.require_auth();
    storage::set_admin(&env, &admin);
    storage::set_paused(&env, false);
}

pub fn pause(env: Env, admin: Address) {
    admin.require_auth();
    assert_admin(&env, &admin);
    storage::set_paused(&env, true);
    events::emit_contract_paused(&env, &admin);
}

pub fn unpause(env: Env, admin: Address) {
    admin.require_auth();
    assert_admin(&env, &admin);
    storage::set_paused(&env, false);
    events::emit_contract_unpaused(&env, &admin);
}

fn assert_not_paused(env: &Env) {
    if storage::get_paused(env) {
        panic!("contract is paused");
    }
}

// In every mutating function, immediately after require_auth:
pub fn create_bounty(env: Env, creator: Address, ...) -> BountyId {
    creator.require_auth();
    assert_not_paused(&env);
    // ... existing logic unchanged
}
```

---

## Option B — Soroban contract upgrade (wasm swap)

### Mechanism

Soroban supports replacing a contract's Wasm blob in-place via
`env.deployer().update_current_contract_wasm(new_wasm_hash)`. The contract address
and all existing storage slots are preserved; only the executable code changes.

1. Add an `upgrade(admin: Address, new_wasm_hash: BytesN<32>)` function.
2. The operator uploads the patched Wasm (`stellar contract install`), obtaining its hash.
3. `upgrade` is called with that hash; subsequent invocations execute the new Wasm.

### Pros

- Allows complete bug remediation — the logic is actually fixed, not just halted.
- Storage is preserved; no state migration is needed for layout-compatible patches.
- Established pattern in the Soroban ecosystem (used by the reference token contract
  and multiple audited DeFi protocols).

### Cons

- **Breaks immutability**: an upgradeable contract is not trustless. Sophisticated
  integrators and auditors may reject it on this basis.
- The admin key has unlimited power — it can replace any logic. A compromise is
  catastrophic rather than merely disruptive.
- Requires strict storage-versioning discipline: the new Wasm must be compatible with
  every storage slot written by old Wasm already on-chain.
- Does not address the gap between a bug being found and the fix being deployed;
  the pause mechanism fills that gap.

### Implementation sketch

```rust
pub fn upgrade(env: Env, admin: Address, new_wasm_hash: BytesN<32>) {
    admin.require_auth();
    let stored = storage::get_admin(&env).expect("not initialised");
    if admin != stored {
        panic!("not admin");
    }
    env.deployer().update_current_contract_wasm(new_wasm_hash);
}
```

---

## Recommendation

**Implement both, in order.**

**Phase 1 (immediate — low risk):** Implement Option A (pause flag). Provides the
circuit breaker with minimal complexity and no change to the trust model beyond
adding an admin key.

**Phase 2 (next milestone — higher scrutiny):** Implement Option B (wasm upgrade)
behind a time-locked or multi-sig admin. Provides full remediation capability; its
elevated power demands a governance model for the admin key.

### Admin key governance (applies to both options)

The admin key must be:
- A multi-sig Stellar account (multiple signers, quorum threshold ≥ 2) or a
  dedicated governance contract, not a single hot wallet.
- Set immutably at deploy time; changing the admin must require the current admin's
  signature.
- Documented publicly so integrators know the trust assumptions they are accepting.

---

## Follow-up tickets

| # | Title | Scope |
|---|-------|-------|
| 1 | `feat: admin init and pause/unpause` | `DataKey::Admin`, `DataKey::Paused`, `init`, `pause`, `unpause`, `assert_not_paused`, guard in all 9 mutating functions, unit tests |
| 2 | `feat: contract wasm upgrade mechanism` | `upgrade` function, storage-version helper, deployment runbook in `docs/` |
| 3 | `sec: define admin key governance model` | Multi-sig vs governance contract decision, update `docs/security.md` |

---

## References

- [Soroban upgradeable contract example](https://developers.stellar.org/docs/smart-contracts/example-contracts/upgradeable-contract)
- `docs/security.md` — `require_auth` placement audit and escrow threat model
- Soroban SDK `Deployer::update_current_contract_wasm`
