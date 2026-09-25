# Migration Guide

This document has two parts:

- **[Part 1 — v2 migration for SDK consumers](#part-1--v2-migration-for-sdk-consumers)**: what changes in your code when you upgrade `@mergemint/sdk` and the contract to v2, with before/after snippets for each breaking change.
- **[Part 2 — Storage migration strategy](#part-2--storage-migration-strategy)**: how contract maintainers evolve `#[contracttype]` storage layouts without breaking existing ledger entries.

---

## Part 1 — v2 migration for SDK consumers

v2 is a **MAJOR** release under [versioning.md](versioning.md). It contains the breaking changes listed below. Deal with each one in order; none of them need storage migration for existing bounties.

| # | Change | Who is affected | Status |
| - | ------ | --------------- | ------ |
| 1 | [Paginated list queries](#1-paginated-list-queries) | Anyone listing bounties | Contract: **shipped**. SDK: v2 |
| 2 | [Batched bounty metadata](#2-batched-bounty-metadata-get_bounty_meta--get_bounty_metas) | Anyone reading titles/descriptions | Contract: **shipped**. SDK: v2 |
| 3 | [Numeric contract error codes](#3-numeric-contract-error-codes) | Anyone matching on error messages | Contract + SDK: v2 |

> **Heads-up for v1 SDK users:** changes 1 and 2 are already live in the contract. v1's `getOpenBounties()` and `getBountyMeta()` call the old entrypoint shapes, so against a current deployment they fail simulation and quietly return `[]` / `null`. If your bounty list looks empty, this is why. Upgrade to v2.

### Quick checklist

- [ ] Bump `@mergemint/sdk` to `^2.0.0`.
- [ ] Replace every `getOpenBounties()` call with the paginated form and loop on `nextCursor`.
- [ ] Replace `getBountyMeta(id)` with `getBountyMetas([ids])`.
- [ ] Replace string matching on `"Simulation failed: …"` messages with `ContractErrorCode` checks.
- [ ] If you call the contract directly (Rust client, `stellar contract invoke`, custom XDR), apply the raw-contract changes shown under each section too.
- [ ] Run your test suite against testnet before switching mainnet traffic.

---

### 1. Paginated list queries

**Why:** returning every open bounty in one call grows without limit and eventually exceeds Soroban's per-invocation CPU/read budget. List queries now return at most 50 IDs per call plus a cursor for the next page.

**Contract entrypoints affected:**

| Entrypoint | v1 | v2 |
| ---------- | -- | -- |
| `get_open_bounties` | `() -> Vec<BountyId>` | `(cursor: Option<u32>, limit: u32) -> (Vec<BountyId>, Option<u32>)` |
| `get_bounties_by_status` | `(status) -> Vec<BountyId>` | `(status, cursor: Option<u32>, limit: u32) -> (Vec<BountyId>, Option<u32>)` |
| `get_bounties_by_creator` | `(creator) -> Vec<BountyId>` | `(creator, cursor: Option<u32>, limit: u32) -> (Vec<BountyId>, Option<u32>)` |
| `get_open_bounties_paged` | — | `(offset: u32, limit: u32) -> Vec<BountyId>` (legacy helper, prefer the cursor form) |

`limit` is capped at 50; `0` or anything above 50 is treated as 50. `cursor: None` starts at the beginning. The returned cursor is `None` once the list is exhausted. `get_open_bounties_count()` and `get_status_count(status)` give totals if you need to render page numbers.

**SDK: before (v1)**

```ts
import { MergeMintSDK, TESTNET, createNetworkConfig } from "@mergemint/sdk";

const sdk = new MergeMintSDK(createNetworkConfig(TESTNET, CONTRACT_ID));

// Returned every open bounty ID in one call.
const ids: string[] = await sdk.getOpenBounties();
render(ids);
```

**SDK: after (v2)**

```ts
import { MergeMintSDK, TESTNET, createNetworkConfig, type Page } from "@mergemint/sdk";

const sdk = new MergeMintSDK(createNetworkConfig(TESTNET, CONTRACT_ID));

// One page (e.g. for an infinite-scroll list):
const first: Page<string> = await sdk.getOpenBounties({ limit: 20 });
render(first.items);
if (first.nextCursor !== null) {
  const second = await sdk.getOpenBounties({ cursor: first.nextCursor, limit: 20 });
  render(second.items);
}

// Every page (e.g. for a background job). Prefer paging in UIs.
const all: string[] = [];
let cursor: number | null = null;
do {
  const page: Page<string> = await sdk.getOpenBounties({ cursor, limit: 50 });
  all.push(...page.items);
  cursor = page.nextCursor;
} while (cursor !== null);

// The same shape applies to the new status/creator queries:
const disputed = await sdk.getBountiesByStatus("disputed", { limit: 20 });
const mine = await sdk.getBountiesByCreator(MY_ADDRESS, { cursor: null, limit: 20 });
```

`Page<T>` is `{ items: T[]; nextCursor: number | null }`. Both option fields are optional: `cursor` defaults to `null` (first page) and `limit` to `50`.

**Raw contract: before / after (`stellar contract invoke`)**

```bash
# before
stellar contract invoke --id $CONTRACT_ID --network testnet --source-account me --send=no \
  -- get_open_bounties

# after: first page, then pass the returned cursor back in
stellar contract invoke --id $CONTRACT_ID --network testnet --source-account me --send=no \
  -- get_open_bounties --limit 20
stellar contract invoke --id $CONTRACT_ID --network testnet --source-account me --send=no \
  -- get_open_bounties --cursor 20 --limit 20
```

**Raw contract: before / after (Rust client)**

```rust
// before
let ids: Vec<BytesN<32>> = client.get_open_bounties();

// after
let mut cursor: Option<u32> = None;
loop {
    let (ids, next) = client.get_open_bounties(&cursor, &50);
    for id in ids.iter() { /* ... */ }
    match next {
        Some(c) => cursor = Some(c),
        None => break,
    }
}
```

> Cursors are offsets into a live index. If bounties open or close between calls, a page can skip or repeat an ID. De-duplicate by ID if exact results matter.

---

### 2. Batched bounty metadata (`get_bounty_meta` → `get_bounty_metas`)

**Why:** list views needed one RPC round-trip per bounty to show titles. The single-ID entrypoint has been replaced by a batched one that returns results in the same order as the input.

**SDK: before (v1)**

```ts
const meta = await sdk.getBountyMeta(bountyId);
if (meta) console.log(meta.title);

// N round-trips for a list
const metas = await Promise.all(ids.map((id) => sdk.getBountyMeta(id)));
```

**SDK: after (v2)**

```ts
const [meta] = await sdk.getBountyMetas([bountyId]);
if (meta) console.log(meta.title);

// One round-trip for a whole page; result[i] corresponds to ids[i]
const page = await sdk.getOpenBounties({ limit: 20 });
const metas: Array<BountyMeta | null> = await sdk.getBountyMetas(page.items);
page.items.forEach((id, i) => render(id, metas[i]?.title ?? "(untitled)"));
```

**Raw contract**

```rust
// before
let meta: Option<BountyMeta> = client.get_bounty_meta(&id);

// after
let metas: Vec<Option<BountyMeta>> = client.get_bounty_metas(&vec![&env, id]);
```

---

### 3. Numeric contract error codes

**Why:** v1 aborts with a `panic!("<message>")`. To the caller that surfaces as an unstructured host error, so integrators had to string-match on English messages that could change in any release. v2 declares `ContractError` with `#[contracterror]` and aborts with `panic_with_error!`. Each failure then carries a **stable numeric code** that you can match on.

**Contract: before / after**

```rust
// before (v1): src/errors.rs
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ContractError {
    BountyNotFound,
    BountyAlreadyAssigned,
    // ...
}

pub fn fail(e: ContractError) -> ! {
    panic!("{}", message(e))
}

// after (v2)
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum ContractError {
    BountyNotFound = 1,
    BountyAlreadyAssigned = 2,
    // ... see table below; codes are append-only and never reused
}

pub fn fail(env: &Env, e: ContractError) -> ! {
    panic_with_error!(env, e)
}
```

**SDK: before (v1)**

```ts
try {
  const xdr = await sdk.claimBounty(contributor, bountyId, contributor);
  await signAndSubmit(xdr);
} catch (err) {
  const msg = err instanceof Error ? err.message : String(err);
  if (msg.includes("bounty already assigned")) {
    showToast("Someone beat you to it.");
  } else if (msg.includes("contributor reputation is too low")) {
    showToast("You need more reputation to claim this bounty.");
  } else {
    throw err;
  }
}
```

**SDK: after (v2)**

```ts
import { ContractErrorCode, MergeMintContractError } from "@mergemint/sdk";

try {
  const xdr = await sdk.claimBounty(contributor, bountyId, contributor);
  await signAndSubmit(xdr);
} catch (err) {
  if (!(err instanceof MergeMintContractError)) throw err;

  switch (err.contractCode) {
    case ContractErrorCode.BountyAlreadyAssigned:
      showToast("Someone beat you to it.");
      break;
    case ContractErrorCode.ReputationTooLow:
      showToast("You need more reputation to claim this bounty.");
      break;
    default:
      throw err;
  }
}
```

`MergeMintContractError` extends `MergeMintSdkError` with `code === "CONTRACT_ERROR"` and a `contractCode: ContractErrorCode`. Existing `catch (e) { if (e instanceof MergeMintSdkError) … }` blocks keep working; only message matching needs updating. Errors that don't come from the contract (bad config, RPC transport failures, invalid arguments) keep their v1 `MergeMintSdkError` codes.

If you parse simulation errors yourself without the SDK, v2 contract failures appear in the diagnostic as `Error(Contract, #<code>)`:

```ts
// before
const assigned = simError.includes("bounty already assigned");

// after
const m = /Error\(Contract, #(\d+)\)/.exec(simError);
const assigned = m !== null && Number(m[1]) === ContractErrorCode.BountyAlreadyAssigned;
```

**Rust tests: before / after**

```rust
// before
#[test]
#[should_panic(expected = "bounty not found")]
fn claim_missing_bounty() {
    client.claim_bounty(&contributor, &missing_id);
}

// after
#[test]
fn claim_missing_bounty() {
    assert_eq!(
        client.try_claim_bounty(&contributor, &missing_id),
        Err(Ok(ContractError::BountyNotFound))
    );
}
```

**Error code reference**

Codes follow the declaration order of `ContractError` in `src/errors.rs`. They are **append-only**: new errors get the next free number, and removed errors keep their number reserved.

| Code | `ContractError` variant | v1 panic message |
| ---: | ----------------------- | ---------------- |
| 1 | `BountyNotFound` | `"bounty not found"` |
| 2 | `BountyAlreadyAssigned` | `"bounty already assigned"` |
| 3 | `AlreadyClaimed` | `"bounty already claimed by contributor"` |
| 4 | `BountyNotOpen` | `"bounty not open"` |
| 5 | `BountyNotInProgress` | `"bounty is not in progress"` |
| 6 | `BountyHasNoAssignee` | `"bounty has no assignee"` |
| 7 | `RewardMustBePositive` | `"reward_amount must be positive"` |
| 8 | `RewardBelowMinimum` | `"reward_amount is below the minimum allowed"` |
| 9 | `NotBountyCreator` | `"not bounty creator"` |
| 10 | `VerifierCannotBeAssignee` | `"verifier cannot be the assignee"` |
| 11 | `CreatorCannotClaim` | `"creator cannot claim"` |
| 12 | `ContributorHasActiveClaim` | `"contributor already has an active claim"` |
| 13 | `BountyIsDisputed` | `"bounty is disputed"` |
| 14 | `BountyDeadlinePassed` | `"bounty deadline passed"` |
| 15 | `BountyNoDeadline` | `"bounty has no deadline"` |
| 16 | `DeadlineNotPassed` | `"deadline has not passed"` |
| 17 | `ReputationTooLow` | `"contributor reputation is too low"` |
| 18 | `TooManyTags` | `"too many tags"` |
| 19 | `MaxAssigneesMustBePositive` | `"max_assignees must be at least 1"` |
| 20 | `OnlyCreatorOrAssigneeCanDispute` | `"only creator or assignee can raise dispute"` |
| 21 | `VerifierNotAuthorized` | `"verifier is not in the required verifiers list"` |
| 22 | `AlreadyApproved` | `"verifier has already approved this bounty"` |
| 23 | `BountyNotDisputed` | `"bounty is not in disputed status"` |
| 24 | `NotArbitrator` | `"caller is not authorized to resolve this dispute"` |
| 25 | `ApprovalThresholdExceedsVerifiers` | `"approval_threshold cannot exceed the number of required_verifiers"` |
| 26 | `InvalidRewardToken` | `"invalid reward_token address"` |
| 27 | `InvalidStatus` | `"invalid bounty status"` |
| 28 | `InvalidTag` | `"invalid bounty tag"` |
| 29 | `InvalidResolution` | `"resolution must be 'complete' or 'cancel'"` |
| 30 | `MilestoneAlreadyCompleted` | `"milestone is already completed"` |
| 31 | `NotAllMilestonesCompleted` | `"not all milestones are completed"` |
| 32 | `InvalidMilestoneIndex` | `"invalid milestone index"` |
| 33 | `MilestoneRewardsMismatch` | `"milestone rewards do not sum to reward_amount"` |
| 34 | `RewardAmountOverflow` | `"reward amount arithmetic overflow"` |
| 35 | `MetadataEmpty` | `"metadata must not be empty"` |

---

### Testing your migration

1. Point a branch of your integration at a testnet deployment of the v2 contract.
2. Run your happy paths (list → view → claim → complete) and at least one expected failure per error you handle (e.g. claim an already-claimed bounty) to confirm your `ContractErrorCode` branches fire.
3. For list views, create more than 50 bounties on testnet (or pass a small `limit`) to make sure your code follows `nextCursor` past the first page.

---

## Part 2 — Storage migration strategy

Soroban persistent storage serialises `#[contracttype]` structs to XDR at compile time. Any change to a struct's field layout — adding, removing, or reordering fields — will cause existing ledger entries to fail deserialisation. `get_bounty` and `get_contributor` will panic rather than return stale data.

This document describes the available migration strategies and the recommended approach for MergeMint.

---

### Why schema changes break storage

The `#[contracttype]` macro generates XDR encoding derived from the field declaration order. Existing ledger entries encoded against `BountyV1` cannot be decoded with `BountyV2` if their XDR layouts differ. There is no automatic schema evolution in Soroban.

---

### Option 1 — Versioned structs (recommended)

Introduce a new struct for each breaking schema version and store an enum that wraps all versions under a single `DataKey`.

```rust
#[contracttype]
pub struct BountyV1 {
    pub creator: Address,
    pub reward_amount: i128,
    // ... original fields
}

#[contracttype]
pub struct BountyV2 {
    pub creator: Address,
    pub reward_amount: i128,
    pub tags: Vec<Symbol>,   // new field
    // ... other fields
}

#[contracttype]
pub enum BountyVersioned {
    V1(BountyV1),
    V2(BountyV2),
}
```

**On read** — deserialise the enum, match on the variant, and upgrade in place before returning:

```rust
pub fn get_bounty(env: &Env, id: BytesN<32>) -> Bounty {
    let versioned: BountyVersioned = env.storage().persistent()
        .get(&DataKey::Bounty(id.clone()))
        .unwrap_or_else(|| panic!("bounty not found"));

    match versioned {
        BountyVersioned::V1(v1) => migrate_v1_to_v2(v1),
        BountyVersioned::V2(v2) => v2,
    }
}
```

**Trade-offs:**
- No downtime required; migration is lazy and happens on first access.
- Ledger entries are never invalidated — old entries coexist with new ones.
- Read functions grow more complex with each version added.
- Old variants must be retained in the enum forever (or until a full migration is complete).

---

### Option 2 — Lazy migration on read (write-back pattern)

Similar to versioned structs but the migration is persisted back to storage on every read of an old entry, so old variants are gradually eliminated without a dedicated migration transaction.

```rust
pub fn get_bounty(env: &Env, id: BytesN<32>) -> BountyV2 {
    let versioned: BountyVersioned = env.storage().persistent()
        .get(&DataKey::Bounty(id.clone()))
        .expect("bounty not found");

    let current = match versioned {
        BountyVersioned::V1(v1) => {
            let upgraded = migrate_v1_to_v2(v1);
            // Write the upgraded entry back so future reads are free.
            env.storage().persistent()
                .set(&DataKey::Bounty(id), &BountyVersioned::V2(upgraded.clone()));
            upgraded
        }
        BountyVersioned::V2(v2) => v2,
    };
    current
}
```

**Trade-offs:**
- Eliminates old variants over time without a one-shot migration transaction.
- Every read of a stale entry pays a write fee.
- The contract must remain authorised to write during normal user operations.

---

### Option 3 — One-time admin migration function

Add an admin-only function that iterates over all bounties and rewrites them under the new schema in a single transaction.

```rust
pub fn migrate_bounties(env: &Env, admin: Address, ids: Vec<BytesN<32>>) {
    admin.require_auth();
    for id in ids.iter() {
        let old: BountyV1 = env.storage().persistent()
            .get(&DataKey::Bounty(id.clone()))
            .expect("bounty not found");
        let new = migrate_v1_to_v2(old);
        env.storage().persistent()
            .set(&DataKey::Bounty(id), &BountyVersioned::V2(new));
    }
}
```

**Trade-offs:**
- Clean cut-over; no version branching in read paths after migration completes.
- Migration must be executed before the new contract version goes live.
- Soroban instruction limits constrain how many entries can be migrated per invocation; large data sets require batched calls.
- Risk of partial migration if the transaction fails mid-way.

---

### Option 4 — Contract replacement

Deploy a new contract instance, copy state via an export/import mechanism, and redirect integrators to the new address.

**Trade-offs:**
- Guarantees a clean schema with no legacy code paths.
- All integrators must update to the new contract address.
- Requires coordinated deployment across the MergeMint API, TypeScript SDK, and any third-party tools.
- Most disruptive option; suitable only for major breaking changes that cannot be handled incrementally.

---

### Recommended approach for MergeMint

**Use versioned structs (Option 1) with lazy write-back (Option 2) for incremental changes.**

- Wrap `Bounty` and `Contributor` in `BountyVersioned` / `ContributorVersioned` enums.
- Perform in-place migration on read; write the upgraded struct back to storage immediately so future reads are free.
- Reserve the admin migration function (Option 3) for cases where a large number of entries must be upgraded before a deadline (e.g., TTL pressure).
- Reserve contract replacement (Option 4) for major architectural changes only.

### Versioning policy

| Change type | Strategy |
|---|---|
| Add optional field (`Option<T>`) | Versioned struct + lazy write-back |
| Add required field | Versioned struct + lazy write-back |
| Remove field | Versioned struct (keep old field in `V_n`, drop in `V_n+1`) |
| Reorder fields | Versioned struct |
| Change field type | Versioned struct |
| Rename `DataKey` variant | Admin migration or contract replacement |

### When to use `Option<T>` instead of a new struct version

If a new field is genuinely optional and `None` is a valid sentinel for "not set on old entries," adding `Option<T>` to the current struct avoids a version bump. Soroban XDR encodes `Option<T>` as a union with a presence flag, so `None` at the end of a struct is backward-compatible **only if** existing entries are regenerated with the `None` variant before the new code is deployed. In practice this means running an admin migration first; otherwise old entries still fail to deserialise.

---

### TTL considerations

Soroban persistent storage entries have a TTL. Migrated entries written back during a lazy read inherit the new TTL from the write call. Ensure that `extend_ttl` calls in `get_bounty` / `get_contributor` are applied to the migrated entry, not the stale one.

---

### Related documents

- [Architecture overview](architecture.md)
- [Event schema](event-schema.md)
- [CHANGELOG](../CHANGELOG.md)
