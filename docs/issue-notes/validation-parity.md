# Validation parity: `app/src/lib/validation.ts` vs. mergemint-backend

## What was implemented

`app/src/lib/validation.ts` already enforced `isValidRewardAmount` and
`isValidContractAddress`, but the reward-amount rule had no backend
counterpart being tested anywhere, and there was no `isValidDescriptionLength`
rule at all even though the UI enforces a 32-character on-chain `Symbol`
limit (`SYMBOL_MAX_LENGTH`) via the `maxLength` attribute in
`app/src/components/CreateBounty.tsx`.

1. **`fixtures/validation-parity.json`** (repo root) — a shared fixture of
   valid/invalid cases for both rules, with a `reason` on every case so a
   future failure is self-explanatory.
2. **`app/src/lib/validation.ts`** — added `isValidDescriptionLength`,
   matching the existing module's naming and style (a small exported
   predicate, same as `isValidRewardAmount`/`isValidContractAddress`).
3. **`mergemint-backend/src/validation.rs`** (new) — Rust ports of
   `is_valid_reward_amount` and `is_valid_description_length` that encode the
   exact same rules (positive decimal, ≤7 fractional digits; non-empty,
   ≤32 chars after trimming). Registered via `pub mod validation;` in
   `mergemint-backend/src/lib.rs`.
4. **Tests on both sides load the same fixture**:
   - `mergemint-backend/src/validation.rs` — `#[cfg(test)] mod tests` reads
     `fixtures/validation-parity.json` via `include_str!` and asserts every
     case against the Rust validators. Run with `cargo test` inside
     `mergemint-backend/`.
   - `app/src/lib/validation.test.ts` — reads the same JSON file and asserts
     every case against the TypeScript validators. Run with `npm test`
     inside `app/` (wired up as the `test` script in `app/package.json`).

## Why this approach

`app/` had no test runner configured at all (only `tsc --noEmit`). Rather
than pulling in a new test framework dependency — out of scope per this
issue, and the kind of "dependency bump" the issue explicitly asks PRs to
avoid — `validation.test.ts` is a small, dependency-free script using
`node:assert` and Node's built-in TypeScript type-stripping
(`node --experimental-strip-types`, no transpile step required). If `app/`
later adopts a real test framework, this file can be dropped in as-is.

## Scope

`isValidContractAddress` was left untouched; only the reward-amount and
description-length rules named in the issue were given a backend
counterpart and a shared fixture.
