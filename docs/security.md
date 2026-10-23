# Security Threat Model

> For a structured STRIDE analysis across the contract, backend, frontend and wallet, see [threat-model.md](threat-model.md).

This document analyses known attack vectors against the MergeMint contract, rates their severity, describes current mitigations, and identifies residual risk. It covers the **current no-escrow design** (the contract never holds a token balance; the [verifier](glossary.md#verifier) pushes tokens directly from their own wallet) as well as the **planned escrow model** where the contract will custody funds.

---

## Admin key rotation (two-step transfer)

Admin rights control pausing, upgrades and protocol configuration. A single-step transfer is risky: a typo in the new address permanently locks the contract's admin functions. The contract therefore uses a **propose and accept** flow.

### Flow

1. **Propose** — the current admin calls `propose_admin(new_admin)`. The address is stored as the *pending admin*; the current admin remains in control. Emits an `admin_proposed` event.
2. **Accept** — only the pending admin can call `accept_admin()`. This proves the pending address holds the key. On success the pending admin becomes the current admin and the pending slot is cleared. Emits an `admin_accepted` event.
3. **Cancel** — the current admin can call `cancel_admin_proposal()` at any time to discard a pending proposal. Emits an `admin_proposal_cancelled` event.

### Operator guidance

- Always verify the new admin address out-of-band (e.g. a signed message or a known-good key) **before** calling `propose_admin`.
- After proposing, confirm the pending admin can sign transactions and call `accept_admin` promptly. Until acceptance, the current admin retains full control.
- If the pending admin is unresponsive or the address was mistyped, call `cancel_admin_proposal` and start over — the current admin is never locked out by a bad proposal.
- Rotate keys during a low-activity window and monitor the emitted events via the MergeMint API indexer to confirm each step landed.

### Residual risk

Low. A typo in `propose_admin` cannot lock the contract because acceptance requires the pending address to authenticate; the current admin can always cancel. The only residual risk is the current admin losing their own key before a proposal is accepted, which is outside the contract's control.

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

**Description:** The verifier role is unconstrained by the contract — any address that calls `complete_bounty` and has a sufficient token balance can pay a reward. A verifier and a bounty creator (or a verifier acting alone) can therefore collude: they designate a fake contributor as [assignee](glossary.md#assignee) via `claim_bounty`, then the verifier calls `complete_bounty` to transfer tokens to that address and award +10 reputation — all without any genuine work having been done. Because the contract has no on-chain way to verify off-chain contribution quality, this is a social-trust attack rather than a code flaw.

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

---

## Backend security headers

The backend adds these headers to every response, including JSON routes, the
`/bounties/stream` SSE stream, CORS preflights and error responses (413, 504).
A value set by a handler is never overwritten. See
`mergemint-backend/src/security_headers.rs`.

| Header                      | Value                       | Why                                                        |
| --------------------------- | --------------------------- | ---------------------------------------------------------- |
| `X-Content-Type-Options`    | `nosniff`                   | Browsers never treat a JSON body as HTML or script.        |
| `Referrer-Policy`           | `no-referrer`               | API URLs (e.g. email verification links) are never leaked. |
| `X-Frame-Options`           | `DENY`                      | Pages such as `/swagger-ui` cannot be framed.              |
| `Strict-Transport-Security` | `max-age=<n>` (opt-in only) | Browsers keep using HTTPS for the API.                     |

HSTS must only be sent when clients reach the backend over HTTPS, which is
usually through a TLS-terminating proxy. It is off by default so local HTTP
development keeps working.

| Env var                   | Default    | Meaning                                       |
| ------------------------- | ---------- | --------------------------------------------- |
| `HSTS_ENABLED`            | `false`    | Send `Strict-Transport-Security` when `true`. |
| `HSTS_MAX_AGE_SECS`       | `31536000` | HSTS `max-age` in seconds.                    |
| `HSTS_INCLUDE_SUBDOMAINS` | `false`    | Add `includeSubDomains` when `true`.          |

These headers do not change CORS (`Access-Control-*` headers are still set by
the CORS layer) or SSE (`Content-Type`, caching and streaming are untouched).
To check a deployment, run a header scanner such as
`curl -sI https://<api-host>/health` or <https://securityheaders.com>.

## Frontend Content Security Policy

The frontend ships a strict Content Security Policy that limits what an XSS
bug, for example in a Markdown bounty description, could do. The policy is
built in `frontend/src/security/csp.ts` and:

- is sent by `vite preview`, which serves the production build;
- is written to `dist/_headers` on `vite build`, which Netlify and Cloudflare
  Pages read. Other hosts (nginx, a CDN) should copy the header from that file.

The Vite dev server does not send it, because Vite injects an inline
React-refresh script there. Test the policy with `vite build && vite preview`.

### Rollout: report-only first, then enforce

| `VITE_CSP_MODE`         | Header sent                           |
| ----------------------- | ------------------------------------- |
| `report-only` (default) | `Content-Security-Policy-Report-Only` |
| `enforce`               | `Content-Security-Policy`             |
| `off`                   | none                                  |

Set `VITE_CSP_REPORT_URI` to collect violation reports. Once a release runs in
report-only mode with no unexpected violations in the console or the reports
(check wallet connect, bounty pages, the live SSE feed and the stats page),
switch to `enforce`.

### Allowed origins

| Directive         | Allowed                                     | Why                                                                                                              |
| ----------------- | ------------------------------------------- | ---------------------------------------------------------------------------------------------------------------- |
| `script-src`      | `'self'`                                    | Only the app's own bundled scripts. No `unsafe-inline` or `unsafe-eval`, and `index.html` has no inline scripts. |
| `connect-src`     | `'self'`                                    | Backend API and SSE when served under the same origin (`/api/v1`).                                               |
|                   | Origin of `VITE_API_BASE_URL`               | Backend API and SSE when hosted on a separate origin.                                                            |
|                   | `https://soroban-testnet.stellar.org`, `https://horizon-testnet.stellar.org` | Default RPC and Horizon for `VITE_NETWORK=testnet`.                         |
|                   | `https://horizon.stellar.org`               | Default Horizon for `VITE_NETWORK=mainnet`.                                                                      |
|                   | Origins in `VITE_RPC_URLS`                  | Extra RPC/Horizon endpoints (comma-separated).                                                                   |
| `frame-src`       | `https://albedo.link`                       | The Albedo wallet's signing frame.                                                                               |
| `style-src`       | `'self' 'unsafe-inline'`                    | A few components render small `<style>` blocks. Styles cannot run code.                                          |
| `img-src`         | `'self' data:`                              | App images and inline icons.                                                                                     |
| `worker-src`      | `'self'`                                    | The offline service worker.                                                                                      |
| `object-src`      | `'none'`                                    | No plugins.                                                                                                      |
| `frame-ancestors` | `'none'`                                    | The app cannot be framed (clickjacking).                                                                         |

Freighter and xBull are browser extensions. Their content scripts talk to the
page with `window.postMessage`, which CSP does not restrict, so they need no
entry. If a new wallet or external service is added, add its origin in
`csp.ts` and in this table.
