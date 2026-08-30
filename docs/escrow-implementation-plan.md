# Escrow Implementation Plan

**Status:** Partially implemented — Phases 2, 3, and 4 are complete; Phase 1 (deposit on create) is not yet implemented.
**Date:** 2026-07-29

## Problem

`docs/security.md:154-214` describes a full escrow model but none of it exists in code.
`create_bounty` never transfers tokens in; `complete_bounty` pays from the verifier's
wallet. The contract never holds a token balance.

## Implementation Status

| Phase | Title | Status |
|-------|-------|--------|
| 1 | Deposit on create | ⬜ Not Started |
| 2 | Refund on cancel/expire/dispute-cancel | ✅ Done |
| 3 | Payout from contract on complete | ✅ Done |
| 4 | Balance invariant tests | ✅ Done |

## Phased Plan

### Phase 1 — Deposit on create (ticket #35) <!-- STATUS: NOT STARTED -->

- `create_bounty` calls `token.transfer(&creator, &env.current_contract_address(), &reward_amount)` after `creator.require_auth()` and all validation, before writing storage.
- Checks-effects-interactions: storage is written **before** the transfer.
- Testnet-only feature flag: gate behind `#[cfg(feature = "escrow")]` until audited.

### Phase 2 — Refund on cancel / expire / dispute-cancel (ticket #36) <!-- STATUS: DONE -->

- `cancel_bounty`, `expire_bounty`, and the `"cancel"` branch of `resolve_dispute` call `token.transfer(&env.current_contract_address(), &bounty.creator, &reward_amount)`.
- Status is written to `cancelled` **before** the transfer (checks-effects-interactions).

### Phase 3 — Payout from contract on complete (ticket #37) <!-- STATUS: DONE -->

- `complete_bounty` and `approve_completion` replace `token.transfer(&verifier, &assignee, &payout)` with `token.transfer(&env.current_contract_address(), &assignee, &payout)`.
- The `verifier` parameter no longer needs a token balance.

### Phase 4 — Balance invariant tests (ticket #38) <!-- STATUS: DONE -->

- Add property-style test: create/claim/cancel/complete a mix of bounties and after every transition assert `contract_balance == sum(reward_amount for bounties in open|in_progress)`.
- See `docs/security.md:211` checklist item.

## Token Balance Invariant

> For any reward token T, `T.balance(contract_address) == Σ reward_amount` over all bounties where `status ∈ {open, in_progress}` and `reward_token == T`.

All code paths must maintain this invariant. Any deviation is a fund-safety bug.

## Rollout

1. Merge Phase 1–3 behind `escrow` feature flag on testnet.
2. Run invariant tests and external audit.
3. Remove feature flag and ship to mainnet.

## Tickets

| # | Title |
|---|-------|
| 35 | `feat: deposit reward into contract on create_bounty` |
| 36 | `feat: refund escrow on cancel/expire/dispute-cancel` |
| 37 | `feat: pay assignees from contract balance on complete` |
| 38 | `test: token-balance invariant test harness` |
