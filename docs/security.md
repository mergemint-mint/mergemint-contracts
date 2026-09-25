# Security Threat Model

> For a structured STRIDE analysis across the contract, backend, frontend and wallet, see [threat-model.md](threat-model.md).

This document analyses known attack vectors against the MergeMint contract, rates their severity, describes current mitigations, and identifies residual risk. It covers the **current no-escrow design** (the contract never holds a token balance; the verifier pushes tokens directly from their own wallet) as well as the **planned escrow model** where the contract will custody funds.

---

## Severity scale

| Rating | Meaning |
|--------|---------|
| **Critical** | Direct, unconditional fund loss or unauthorised privilege escalation |
| **High** | Likely fund loss or state corruption under realistic conditions |
| **Medium** | Requires unusual preconditions or yields only partial impact |
| **Low** | Negligible financial impact; primarily affects data integrity |

---

## Current model (no-escrow) threat vectors

### 1. Verifier collusion

**Severity:** High

**Description:** The verifier role is unconstrained by the contract — any address that calls `complete_bounty` and has a sufficient token balance can pay a reward. A verifier and a bounty creator (or a verifier acting alone) can therefore collude: they designate a fake contributor as assignee via `claim_bounty`, then the verifier calls `complete_bounty` to transfer tokens to that address and award +10 reputation — all without any genuine work having been done. Because the contract has no on-chain way to verify off-chain contribution quality, this is a social-trust attack rather than a code flaw.

**Affected functions:** `claim_bounty`, `complete_bounty`

**Current mitigations:**
- All parties must individually authenticate with `require_auth()`, so no single actor can execute the attack alone — at least two colluding parties are required (verifier + contributor, or creator + contributor with creator also acting as verifier).
- Events (`bounty_claimed`, `bounty_completed`, `reward_paid`) are emitted on-chain and consumed by the MergeMint API indexer, giving off-chain observers an audit trail.

**Residual risk:** High. Collusion cannot be detected or prevented purely on-chain. Off-chain reputation systems, dispute resolution, and community governance are the necessary backstop. See also threat vector #3 (self-verify) for the degenerate single-actor case.

---

### 2. Front-running of `claim_bounty`

**Severity:** Medium

**Description:** On Stellar, multiple transactions can land in the same ledger. If a legitimate contributor broadcasts `claim_bounty` and a malicious observer monitors the network and submits a competing `claim_bounty` for the same bounty with a higher fee, the malicious transaction may be ordered first within that ledger. The attacker claims the bounty slot before the legitimate contributor.

**Affected functions:** `claim_bounty`

**Current mitigations:**
- The double-assignment guard (`assignees.len() >= max_assignees`) ensures only one claim succeeds for a single-assignee bounty — the second transaction will panic cleanly with `BOUNTY_ALREADY_ASSIGNED`, preventing silent data corruption.
- Stellar's ledger ordering is not purely fee-based and is harder to predict than Ethereum's mempool, reducing reliable front-running opportunity.

**Residual risk:** Medium. A well-resourced attacker with low-latency network access can still outrun a legitimate claim. The protocol does not whitelist specific contributors per bounty. Integrators who need claim exclusivity should add an off-chain reservation layer or implement allowlist logic before calling `claim_bounty`.

---

### 3. Self-claim (creator claiming their own bounty)

**Severity:** Medium

**Description:** There is no on-chain check preventing a bounty creator from also being the contributor who calls `claim_bounty`. A creator could therefore post a bounty, claim it themselves, then coordinate with a verifier to `complete_bounty` — earning the reward back plus +10 reputation for no external work. The error constant `CREATOR_CANNOT_CLAIM` exists in `src/errors.rs` but is **not enforced** anywhere in `contract.rs`.

**Affected functions:** `claim_bounty`

**Current mitigations:** None enforced on-chain.

**Residual risk:** High. This is an unmitigated on-chain vulnerability. The fix is a single guard at the top of `claim_bounty`:

```rust
if contributor == bounty.creator {
    panic!("{}", errors::CREATOR_CANNOT_CLAIM);
}
```

Until this guard is added, off-chain tooling (MergeMint API, front-end) must reject self-claims as a compensating control.

---

### 4. Self-verify (verifier is also an assignee)

**Severity:** High — **FIXED** in this PR

**Description:** There was no on-chain check preventing the verifier who calls `complete_bounty` from also being one of the bounty's assignees. In this scenario the verifier calls `token.transfer(&verifier, &assignee, &payout)` where both sides resolve to their own address — a net-zero token movement — but they still receive +10 reputation and an increment to `contribution_count` and `total_earned`. The error constant `VERIFIER_CANNOT_BE_ASSIGNEE` existed in `src/errors.rs` but was not enforced in `contract.rs`.

**Affected functions:** `complete_bounty`

**Fix applied:** A guard iterates the `assignees` list at the start of `complete_bounty` (before any token transfer) and panics with `"verifier cannot be the assignee"` if the verifier address matches any assignee.

```rust
for (assignee, _) in bounty.assignees.iter() {
    if assignee == verifier {
        panic!("verifier cannot be the assignee");
    }
}
```

**Test added:** `test_assignee_cannot_self_verify` in `src/test.rs` — asserts that `complete_bounty` panics with `"verifier cannot be the assignee"` when the contributor calls the function using their own address as verifier.

**Residual risk:** None in the current no-escrow model. Once escrow is introduced this guard prevents a full contract drain — see escrow checklist.

---

### 5. Double-completion (reward drain via repeated `complete_bounty`)

**Severity:** High — **FIXED** in this PR

**Description:** `complete_bounty` did not validate that `bounty.status == STATUS_IN_PROGRESS` before executing. After the first successful call the status was written as `STATUS_COMPLETED` and stored, but the `assignees` list remained populated. A second call by the same verifier therefore passed the `assignees.is_empty()` guard, executed another `token.transfer` for each assignee, and incremented every assignee's `reputation`, `total_earned`, and `contribution_count` again. This could be repeated as many times as the verifier held tokens.

**Affected functions:** `complete_bounty`

**Fix applied:** A status guard is now the first business-logic check in `complete_bounty` (immediately after `require_auth` and the bounty-not-found check). If the bounty is not in `STATUS_IN_PROGRESS`, the function panics before any state mutation or token transfer.

```rust
if bounty.status != Symbol::new(&env, STATUS_IN_PROGRESS) {
    panic!("{}", errors::BOUNTY_NOT_IN_PROGRESS);
}
```

**Test added:** `test_double_complete_panics` in `src/test.rs` — creates a bounty, claims it, completes it once (succeeds), then calls `complete_bounty` again and asserts a panic with `"bounty is not in progress"`.

**Residual risk:** None. The guard blocks all repeat calls regardless of caller. Under escrow this guard also prevents draining the contract's entire token balance.

---

### 6. Reentrancy via token transfer

**Severity:** Low (current model), High (escrow model)

**Description:** `complete_bounty` calls `token.transfer` before updating `bounty.status` to `STATUS_COMPLETED` and persisting the change. In the EVM this ordering would be a critical reentrancy bug. On Soroban, the runtime enforces single-contract-at-a-time execution — a token contract cannot call back into `MergeMintContract` during the same invocation — eliminating classic reentrancy in the current design.

However, when the escrow model is introduced (the contract holds token balances), this ordering will become critical. Any escape to an external contract before the status is written creates a reentrancy window on future Soroban versions or cross-contract paths that may be introduced.

**Affected functions:** `complete_bounty`

**Current mitigations:**
- Soroban's single-contract execution model prevents reentrancy in the current runtime.

**Residual risk:** Low now, Critical under escrow. Enforce checks-effects-interactions proactively: write `bounty.status = STATUS_COMPLETED` and call `storage::store_bounty` *before* any `token.transfer` call. This also eliminates the double-completion vulnerability (#5) as a side effect.

---

### 7. Reputation inflation via multi-claim griefing

**Severity:** Low

**Description:** An actor with control over both a verifier key and multiple contributor keys can create many low-value bounties, claim each with a different contributor key, then call `complete_bounty` for each. Each completion awards +10 reputation and +1 `contribution_count`. Because `reward_amount` can be arbitrarily small (no minimum is enforced), the economic cost per reputation point approaches zero.

**Affected functions:** `create_bounty`, `complete_bounty`

**Current mitigations:**
- Each `claim_bounty` call requires a unique contributor address (duplicate-address guard is enforced).
- The `active_claims` limit (currently 1 per contributor) prevents a single address from gaming the count without completing prior bounties.
- Transaction fees on Stellar impose a small but non-zero cost per operation.

**Residual risk:** Medium. An attacker with many keys can still inflate reputation cheaply. Mitigations include enforcing a minimum `reward_amount` in `create_bounty` and adding a `CONTRIBUTOR_HAS_ACTIVE_CLAIM` style cap on total lifetime completions per verifier.

---

## Escrow threat model

When escrow is introduced the contract will hold tokens on behalf of bounty creators (`create_bounty` transfers reward tokens *into* the contract; `complete_bounty` and `cancel_bounty` transfer them *out*). This changes the threat surface significantly.

### Token balance invariant

> **The contract's token balance for any given token must always equal the sum of `reward_amount` across all bounties in `open` or `in_progress` status that use that token.**

Maintaining this invariant is the primary correctness goal for all escrow-related code paths. Any deviation — even transient — represents a fund safety bug.

### Attack vectors

#### Stuck funds (locked tokens)

**Description:** A bug prevents a bounty from ever reaching `completed` or `cancelled`, locking the escrowed tokens permanently.

**Example scenarios:**
- `cancel_bounty` panics unconditionally due to a logic error.
- Status index corruption leaves a bounty in an unresolvable state.
- A missing code path for a status transition leaves a bounty stuck.

**Mitigations:**
- Ensure `cancel_bounty` is callable by the creator for any bounty in `open` or `in_progress` status — it must always provide an exit.
- Consider a verifier-only emergency cancel path as a backstop.
- Write invariant-checking tests that verify the contract balance equals the sum of open bounty rewards after every state transition.

#### Fund drain (double-completion or reentrancy)

**Description:** An attacker triggers multiple payouts for a single bounty, draining more tokens than the bounty's `reward_amount`.

**Mitigations:**
- Enforce checks-effects-interactions strictly: update the bounty status to `completed` and persist it *before* calling `token.transfer`.
- Validate that `bounty.status == STATUS_IN_PROGRESS` at the top of `complete_bounty` — this prevents double-completion even without reentrancy guards. This is also the fix for threat #5 in the current model.

#### Griefing (ledger pollution)

**Description:** A malicious actor creates many bounties (locking the minimum viable reward in each) and immediately cancels them, burning transaction fees and polluting the status index.

**Mitigations:**
- Enforce a minimum `reward_amount` in `create_bounty` to raise the economic cost of griefing.
- Consider a creation fee (paid to the contract or burned) that is separate from the bounty reward.

### High-risk code paths (escrow)

| Function          | Risk                                      | Key invariant to enforce                              |
|-------------------|-------------------------------------------|-------------------------------------------------------|
| `create_bounty`   | Token transfer in; under-transfer         | `contract_balance += reward_amount` after the call    |
| `complete_bounty` | Token transfer out; double-payment        | Status set to `completed` before `token.transfer`     |
| `cancel_bounty`   | Token transfer out; stuck-fund if blocked | Always reachable by creator; status set before transfer |

### Pre-merge checklist for escrow

- [x] Add status guard to `complete_bounty`: panic if `status != in_progress`. *(done — see threat #5)*
- [ ] Enforce `reward_amount > 0` in `create_bounty`.
- [x] Add creator-cannot-claim guard in `claim_bounty` (fixes threat #3). *(done — see threat #3)*
- [x] Add verifier-cannot-be-assignee guard in `complete_bounty` (fixes threat #4). *(done — see threat #4)*
- [ ] Fuzz `reward_amount` edge cases (0, `i128::MAX`, negative).
- [ ] Add integration test: `contract_balance == sum(open + in_progress rewards)` after every state transition.
- [x] Confirm `complete_bounty` panics when called on an already-completed bounty. *(done — `test_double_complete_panics`)*
- [ ] Review token contract for any re-entrant callbacks into this contract.
- [ ] Have at least one contributor who was not the author review the token transfer ordering.

---

## `require_auth` placement audit

**Date:** 2026-06-29  
**Scope:** All state-mutating functions in `src/contract/mutations.rs`

### Rule

`require_auth()` **must be the first executable line** in every state-mutating contract function. No storage reads, computations, or cross-contract calls may execute before it. Reasons:

1. **Fail fast.** Unauthenticated calls are rejected before any CPU or storage is consumed.
2. **Auditability.** Reviewers can confirm auth is always present by inspecting the first line alone.
3. **Side-effect hygiene.** On Soroban, failed transactions do not persist state changes, but placing auth after storage reads could expose information about contract state to callers who will never be authorised. Future protocol changes could also make pre-auth side effects observable.

### Audit findings

| Function | Auth call | First executable line? | Verdict |
|---|---|---|---|
| `create_bounty` | `creator.require_auth()` | Yes | ✅ Pass |
| `claim_bounty` | `contributor.require_auth()` | Yes | ✅ Pass |
| `complete_bounty` | `verifier.require_auth()` | Yes | ✅ Pass |
| `approve_completion` | `verifier.require_auth()` | Yes | ✅ Pass |
| `raise_dispute` | `caller.require_auth()` | Yes | ✅ Pass |
| `resolve_dispute` | `arbitrator.require_auth()` | Yes | ✅ Pass |
| `update_contributor_metadata` | `contributor.require_auth()` | Yes | ✅ Pass |
| `cancel_bounty` | `caller.require_auth()` | Yes | ✅ Pass |
| `expire_bounty` | `caller.require_auth()` | Yes | ✅ Pass |

**Outcome:** All 9 mutating functions pass. No reordering was required. `# Authorization` doc sections were added to `approve_completion`, `raise_dispute`, and `resolve_dispute`, which previously lacked them.

### Enforcement going forward

Every new state-mutating function added to `MergeMintContract` must:
1. Accept the authenticated principal as the first argument.
2. Call `principal.require_auth()` as the very first statement in the function body.
3. Include a `# Authorization` section in its doc comment explaining who must authenticate.

## Slither triage (`contracts/bounty`)

The `Solidity Static Analysis` workflow runs Slither with `--fail-medium`, so
any new Medium or High finding fails CI. The JSON report is uploaded as the
`slither-report` workflow artifact. Each Medium/High finding raised against
`BountyRefresh.sol` has been resolved as follows:

| Detector | Impact | Resolution |
| --- | --- | --- |
| `uninitialized-local` (`reason` in `_processRefreshTask`) | Medium | **Fixed** — initialised to `""`. |
| `reentrancy-no-eth` (`_processRefreshTask`) | Medium | **False positive** — suppressed inline. |
| `reentrancy-benign` (`_processRefreshTask`) | Low | **False positive** — suppressed inline, same reason. |

**Why the reentrancy findings are false positives.** The only external call in
`_processRefreshTask` is `this._executeRefresh(...)`, a self-call made so the
refresh can be wrapped in `try/catch` and retried. `_executeRefresh` is guarded
by `onlySelf`, so no other account can reach it, and the only entry point,
`processBatchParallel`, is `nonReentrant`. No third-party code runs between
the call and the storage writes that follow it, so there is nothing that could
re-enter and observe inconsistent task or batch state.

Remaining Low/Informational findings (`calls-loop`, `timestamp`,
`costly-loop`, `naming-convention`, `solc-version`) are below the CI gate and
are accepted: the loop is bounded by `MAX_BATCH_SIZE`, the `timestamp` hits
are boolean flag checks on a struct that happens to contain timestamps, and
`_executeRefresh` keeps its name for ABI compatibility.

To reproduce locally:

```sh
pip install slither-analyzer
npm install --no-save @openzeppelin/contracts@^4.9.0
slither contracts/bounty --filter-paths node_modules \
  --solc-remaps "@openzeppelin=node_modules/@openzeppelin" --fail-medium
```
