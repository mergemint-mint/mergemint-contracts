# Proposal: Shared Type Generation Across Contract/Backend/Frontend/SDK

## Problem

`Bounty` and `Contributor` are independently hand-written in at least five places:

- `mergemint-contracts/src/types.rs` (source of truth: the Soroban contract)
- `mergemint-backend/src/scval.rs`
- `mergemint-backend/src/dto.rs`
- `mergemint-contracts/sdk/src/index.ts`
- `mergemint-frontend/src/types.ts`

A contract field rename currently requires manually touching all five, with
nothing in CI enforcing that they stay consistent. This is a latent source of
silent runtime bugs (e.g. a field renamed in the contract but missed in the
frontend type would fail at the JSON/XDR boundary, not at compile time).

## Proposed direction

Generate the backend DTOs and the frontend/SDK TypeScript interfaces from the
contract's Rust types, rather than hand-writing all five independently.

Two viable approaches:

1. **Direct codegen from Rust types.** Use a build-time tool (e.g. `ts-rs` or
   a custom `soroban-sdk` contractspec reader) to emit TypeScript interfaces
   for the SDK and frontend directly from `mergemint-contracts/src/types.rs`.
   The backend's `dto.rs`/`scval.rs` could either consume the same derive
   macro or be generated from the same source.

2. **Shared schema as the source of truth.** Define a JSON-Schema or OpenAPI
   spec for the backend-facing shapes, generate the backend DTOs and the
   frontend/SDK TypeScript types from that spec, and keep the contract's Rust
   types as the authoritative shape that the schema is checked against (e.g.
   via a CI round-trip test).

Option 1 keeps a single source of truth (the contract) and removes an extra
artifact to maintain. Option 2 is more flexible if the backend ever needs
fields that don't map 1:1 to the contract, at the cost of introducing a
fourth artifact (the schema) that itself needs to stay in sync.

## Recommendation

Start with option 1 given the current close 1:1 mapping between contract
types and DTOs. Land the codegen tool + generated SDK/frontend types first
(smallest blast radius), and revisit the backend DTO layer separately since
it has more business-logic-specific fields.

## Scope

This is a design note only. Given the cross-repo scope (contracts, backend,
frontend, and SDK all need coordinated changes), implementation should be
tracked as a separate follow-up ticket with its own plan and tests
(round-trip type checks wired into CI).

## Decision record

**What actually landed:** [#523](https://github.com/mergemint-mint/mergemint-contracts/issues/523)
(PR [#544](https://github.com/mergemint-mint/mergemint-contracts/pull/544)) took
a much narrower approach than either option above: it consolidated the `Bounty`/
`Contributor` interfaces that were previously duplicated *within the SDK package*
into a single `sdk/src/types.ts`. It did not implement contract-to-TypeScript
codegen (Option 1) or a shared schema (Option 2), and it did not touch the
backend or frontend.

[#111](https://github.com/mergemint-mint/mergemint-contracts/issues/111) (a
related but distinct ask — a storage-migration strategy for `Bounty` struct
changes, not type-duplication itself) was closed without an associated
implementation; that topic remains undocumented.

**Remaining follow-up (not yet done):**
- `mergemint-backend/src/scval.rs` and `mergemint-backend/src/dto.rs` still
  hand-write their own shapes, independently of both the contract's
  `src/types.rs` and `sdk/src/types.ts`.
- `frontend/src/types.ts` and `frontend/src/lib/types.ts` still hand-write two
  *different* shapes of `Bounty`/`Contributor` (e.g. `reward: string` vs.
  `rewardAmount: bigint`), neither of which imports from `sdk/src/types.ts`.
- The original problem this doc set out to solve — a contract field rename
  silently going out of sync with the mergemint-backend/frontend/SDK — is therefore
  still open outside the SDK package itself. The codegen-from-Rust approach
  recommended above has not been attempted.
