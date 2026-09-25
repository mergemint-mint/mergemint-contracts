# Incident Runbook

What to do when a critical bug or key compromise is found in a deployed MergeMint contract: who does what, in what order, and which commands to run.

> **Implementation status.** The circuit-breaker entrypoints this runbook relies on are **not yet in `src/`**. `init`, `pause` and `unpause` are specified in [pause-upgrade-strategy.md](pause-upgrade-strategy.md) (follow-up ticket #1). `is_paused` and the two-step admin transfer (`transfer_admin`, `accept_admin`, `get_admin`) are the companions this runbook proposes to meet that doc's requirement that *changing the admin must require the current admin's signature*. Until they ship:
>
> - Follow [§ 3b — Containment without a pause flag](#3b-containment-without-a-pause-flag-current-contract) instead of § 3a.
> - Steps 1, 2, 4, 5, 6 and 7 apply unchanged.
>
> When the entrypoints land, check the signatures below against `src/contract/mutations.rs` and update this doc in the same PR.

---

## Severity levels

| Level | Definition | Example | Pause? | Target time to contain |
| ----- | ---------- | ------- | ------ | ---------------------- |
| **SEV-1** | Funds at risk or actively being drained; admin key compromised | Reentrancy in `complete_bounty` paying twice; leaked admin secret | **Yes, immediately** | 15 minutes |
| **SEV-2** | Incorrect state that can't move funds by itself, but could with other bugs | Status index out of sync; reputation miscounted | Usually | 2 hours |
| **SEV-3** | Degraded UX, no fund or state risk | SDK decode error; frontend shows wrong status | No | Next release |

When unsure, treat it as the higher severity. Unpausing is cheap; lost escrow is not.

---

## Roles

Assign these as soon as the incident is declared. One person may hold more than one role in a small team, but **Incident Lead** and **Key Holder** should be different people whenever possible.

| Role | Responsibility |
| ---- | -------------- |
| **Incident Lead** | Owns the timeline, makes the pause/unpause call, keeps the incident log |
| **Key Holder(s)** | Holds signing authority for the admin account; signs `pause`, `transfer_admin`, `upgrade` |
| **Investigator** | Reproduces the bug, scopes affected bounties, prepares the fix |
| **Comms** | Writes and posts all user-facing updates using the templates in § 5 |

---

## 0. Before an incident (preparation checklist)

Make sure these are true **now**, not during an incident:

- [ ] The admin address is a multi-sig Stellar account (quorum ≥ 2), as required by [pause-upgrade-strategy.md → Admin key governance](pause-upgrade-strategy.md#admin-key-governance-applies-to-both-options).
- [ ] At least two Key Holders can sign within 15 minutes, 24/7. Contact list kept in the private maintainers channel, **not** in this repo.
- [ ] Each Key Holder has `stellar-cli` 23.0.1 installed and their key imported (`stellar keys ls` shows it).
- [ ] The production `CONTRACT_ID`, network and admin address are recorded in the private ops notes.
- [ ] A pause/unpause dry run has been done on testnet within the last quarter (see § 8).
- [ ] Status page / announcement channels (GitHub Discussions, Discord, X) have credentials available to Comms.

---

## 1. Detect and declare

1. Whoever finds the issue opens a **private** channel with the maintainers. Do **not** open a public GitHub issue for a SEV-1/SEV-2. Use a [GitHub private security advisory](https://docs.github.com/en/code-security/security-advisories/working-with-repository-security-advisories/creating-a-repository-security-advisory) on the repository instead.
2. The Incident Lead declares the severity and starts an incident log (UTC timestamps, every action, every tx hash).
3. Set shell variables used in every command below:

   ```bash
   export NETWORK=mainnet            # or testnet
   export CONTRACT_ID=C...           # production contract
   export ADMIN=admin-multisig       # stellar-cli identity / account for the admin
   ```

---

## 2. Assess scope (≤ 10 minutes for SEV-1)

Get enough information to decide whether to pause. Don't wait for a full root cause.

```bash
# How many bounties exist and how many are open (escrow still held)
stellar contract invoke --id $CONTRACT_ID --network $NETWORK --source-account $ADMIN --send=no \
  -- get_bounty_count
stellar contract invoke --id $CONTRACT_ID --network $NETWORK --source-account $ADMIN --send=no \
  -- get_open_bounties_count

# Inspect a suspect bounty
stellar contract invoke --id $CONTRACT_ID --network $NETWORK --source-account $ADMIN --send=no \
  -- get_bounty --bounty_id <HEX_ID>
```

Also check recent contract events (see [event-schema.md](event-schema.md) and [horizon-polling.md](horizon-polling.md)) for unexpected `complete`/`cancel`/`expire` events or payouts.

Decision: **SEV-1 → pause now (§ 3).** SEV-2 → pause unless the Investigator can show the bug isn't exploitable.

---

## 3a. Contain — pause the contract

Pausing blocks every state-mutating entrypoint (`create_bounty`, `claim_bounty`, `complete_milestone`, `complete_bounty`, `approve_completion`, `raise_dispute`, `resolve_dispute`, `update_contributor_metadata`, `cancel_bounty`, `expire_bounty`). Each one panics with `"contract is paused"` via the `assert_not_paused` guard. Read-only queries keep working, so users and indexers can still see state.

**Entrypoint (planned, per pause-upgrade-strategy.md):**

```rust
pub fn pause(env: Env, admin: Address)    // admin.require_auth(); emits contract_paused
pub fn unpause(env: Env, admin: Address)  // admin.require_auth(); emits contract_unpaused
pub fn is_paused(env: Env) -> bool         // read-only
```

**Commands:**

```bash
# 1. Pause (requires the admin multi-sig to sign)
stellar contract invoke --id $CONTRACT_ID --network $NETWORK --source-account $ADMIN \
  -- pause --admin $ADMIN

# 2. Verify
stellar contract invoke --id $CONTRACT_ID --network $NETWORK --source-account $ADMIN --send=no \
  -- is_paused
# expected: true

# 3. Prove mutations are blocked: this must FAIL with "contract is paused"
stellar contract invoke --id $CONTRACT_ID --network $NETWORK --source-account $ADMIN --send=no \
  -- claim_bounty --contributor $ADMIN --bounty_id <ANY_OPEN_HEX_ID>
```

For a multi-sig admin, build the transaction with `--build-only`, collect signatures with `stellar tx sign`, then `stellar tx send`. Record the tx hash in the incident log.

## 3b. Containment without a pause flag (current contract)

Until `pause` ships, there's no on-chain circuit breaker. Do as much of the following as applies:

1. **Stop the off-chain entry points.** Put the frontend into maintenance mode and have the backend reject write routes (`create`, `claim`, `complete`, `resolve`). This stops most users, but **it doesn't stop direct contract calls**.
2. **Drain exposure where the contract allows it.** Bounty creators can call `cancel_bounty` on open bounties to recover escrow. Comms should ask creators to do this if the bug affects open escrow.
3. **Deploy a fixed contract** at a new address (§ 6), migrate integrations, and announce the new `CONTRACT_ID`.
4. Prioritize follow-up ticket #1 from [pause-upgrade-strategy.md](pause-upgrade-strategy.md#follow-up-tickets).

---

## 4. Rotate keys (if a key may be compromised)

Do this **after** pausing if the admin key is suspected compromised, since a compromised admin could unpause.

**Entrypoints (planned, two-step so a typo can't brick admin):**

```rust
pub fn transfer_admin(env: Env, admin: Address, new_admin: Address)  // current admin proposes
pub fn accept_admin(env: Env, new_admin: Address)                    // new admin confirms
pub fn get_admin(env: Env) -> Address                                // read-only
```

**Procedure:**

1. Create a fresh multi-sig account for the new admin with **new** signer keys that were never on the compromised machine(s).
2. Propose and accept:

   ```bash
   export NEW_ADMIN=G...   # new multi-sig account

   stellar contract invoke --id $CONTRACT_ID --network $NETWORK --source-account $ADMIN \
     -- transfer_admin --admin $ADMIN --new_admin $NEW_ADMIN

   stellar contract invoke --id $CONTRACT_ID --network $NETWORK --source-account $NEW_ADMIN \
     -- accept_admin --new_admin $NEW_ADMIN

   stellar contract invoke --id $CONTRACT_ID --network $NETWORK --source-account $NEW_ADMIN --send=no \
     -- get_admin
   # expected: $NEW_ADMIN
   ```

3. On the **old** multi-sig account, remove the compromised signers (`stellar tx new set-options --signer ... --signer-weight 0`) so it can't be used elsewhere.
4. Rotate every other credential the compromised person or machine could reach: backend deploy secrets, RPC provider API keys, GitHub Actions secrets, npm tokens for `@mergemint/sdk`.
5. Update the private ops notes with the new admin address.

If the compromised key is a **contributor or verifier** key rather than admin, there's no on-chain rotation. The owner must stop using that address. Maintainers should reassign any pending verifier role off-chain and, if necessary, resolve affected bounties through `resolve_dispute`.

---

## 5. Communicate

Comms posts on GitHub Discussions, Discord and X, and in-app via the frontend banner. Use UTC times. **Never** include exploit details until a fix is deployed.

**Initial notice (within 30 min of pausing):**

> We've paused the MergeMint contract (`<CONTRACT_ID>`) while we investigate an issue reported at `<HH:MM> UTC`. Your funds held in escrow are not affected by the pause itself, and you can still view all bounties. Creating, claiming and completing bounties is temporarily disabled. Next update by `<HH:MM> UTC`.

**Update (at least every 2 hours while paused):**

> Update on the MergeMint pause: `<what we know / what we're doing>`. The contract stays paused. Next update by `<HH:MM> UTC`.

**Resolution:**

> MergeMint is live again as of `<HH:MM> UTC`. `<One-sentence summary of the fix.>` `<Any action users need to take, e.g. re-submit a claim, or point integrations at new CONTRACT_ID>`. A full post-mortem will follow within 7 days.

If the fix requires integrators to change anything (new contract address, SDK version), notify known integrators directly as well and link [migration.md](migration.md).

---

## 6. Fix and redeploy

1. The Investigator writes a failing test that reproduces the bug in `src/test.rs`, then the fix. Develop in a private fork or a GitHub security advisory's temporary private fork.
2. At least one other maintainer reviews it. Run `cargo test`, `cargo clippy -- -D warnings`, and `scripts/integration_test.sh`.
3. Deploy the fix to **testnet** and run `scripts/smoke_test.sh` against it.
4. Deploy to mainnet:
   - **With `upgrade` available** (follow-up ticket #2): `stellar contract install` the new WASM, then `upgrade --admin $ADMIN --new_wasm_hash <HASH>`. The contract address stays the same.
   - **Without `upgrade`:** deploy a new contract (`scripts/deploy.sh`), then update `CONTRACT_ID` in backend config, frontend env, and SDK docs.

---

## 7. Resume and close

1. Unpause:

   ```bash
   stellar contract invoke --id $CONTRACT_ID --network $NETWORK --source-account $ADMIN \
     -- unpause --admin $ADMIN
   stellar contract invoke --id $CONTRACT_ID --network $NETWORK --source-account $ADMIN --send=no \
     -- is_paused
   # expected: false
   ```

2. Run a live smoke check (`scripts/smoke_test.sh` with a small reward) and watch events for 30 minutes.
3. Remove the frontend maintenance banner and re-enable backend write routes.
4. Post the resolution notice (§ 5).
5. Within 7 days, publish a blameless post-mortem covering timeline, root cause, impact, what went well and badly, and follow-up actions. Publish the GitHub security advisory.

---

## 8. Testnet dry run

Run this each quarter, and whenever the pause/admin code changes, so Key Holders have practiced the steps before a real incident.

```bash
export NETWORK=testnet
stellar keys generate dryrun-admin --network $NETWORK --fund
stellar keys generate dryrun-user  --network $NETWORK --fund
export ADMIN=dryrun-admin

# Deploy a fresh instance and initialise the admin
cargo build --release --target wasm32v1-none
export CONTRACT_ID=$(stellar contract deploy \
  --wasm target/wasm32v1-none/release/mergemint_contracts.wasm \
  --network $NETWORK --source-account $ADMIN)
stellar contract invoke --id $CONTRACT_ID --network $NETWORK --source-account $ADMIN \
  -- init --admin $ADMIN

# Walk through § 3a (pause → verify → prove a mutation fails),
# then § 4 (transfer_admin → accept_admin to dryrun-user → get_admin),
# then § 7 step 1 (unpause from the NEW admin, and confirm the OLD admin is rejected).
```

Record in the incident-drill log how long each step took and anything in this runbook that was wrong or unclear, then fix the doc.

---

## Related documents

- [pause-upgrade-strategy.md](pause-upgrade-strategy.md): design of the pause flag, upgrade path and admin governance
- [security.md](security.md): `require_auth` placement and escrow threat model
- [testing.md](testing.md): `integration_test.sh` and `smoke_test.sh`
- [migration.md](migration.md): what integrators must change after a redeploy
