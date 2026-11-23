> **Status: Planned** — The MergeMint contract requires no changes to support passkey-based callers (see [What Changes Are Required](#what-changes-are-required-in-the-mergemint-contract) below). The fee-sponsorship relayer and smart-wallet factory described here are **not yet implemented** in this repo.

# Passkey Authentication for MergeMint Contributors

A guide for platform integrators on how Soroban smart wallets and WebAuthn passkeys work, how a `claim_bounty` invocation is signed in a passkey-based setup, and what this means for the MergeMint contract.

---

## Background: The Problem with Raw Key Pairs

The standard Stellar account model requires contributors to manage a raw Ed25519 key pair. Losing the secret key means permanent loss of access. This is a significant barrier for open-source contributors who are comfortable with GitHub OAuth or device-based authentication (Face ID, Touch ID, Windows Hello) but have never managed a blockchain private key.

Soroban's smart wallet model removes this barrier. Contributors get a contract account (C-account) whose authorization logic is defined in code — specifically, code that verifies WebAuthn (passkey) signatures instead of raw key signatures.

---

## How Passkey-Based Wallets Work on Soroban

### Two account types

Stellar has two kinds of accounts relevant here:

- **G-accounts** — classic Stellar accounts identified by a public key starting with `G`. They sign transaction envelopes with a raw Ed25519 keypair.
- **C-accounts** — contract accounts identified by an address starting with `C`. They have no keypair. Their authorization logic is implemented in a smart contract via a `__check_auth` function.

A smart wallet is a C-account whose `__check_auth` function verifies a WebAuthn/passkey signature using the ES256 (secp256r1) algorithm, which Soroban supports natively.

### Smart wallet contract structure

A minimal Soroban smart wallet consists of:

1. **A factory contract** — deploys individual wallet instances with deterministic addresses, registering the user's passkey public key as the initial signer.
2. **A wallet contract** — stores the passkey public key on-chain and implements `__check_auth` to verify WebAuthn signatures.

When a user registers, their device generates an ES256 keypair inside the secure enclave. The public key is extracted and stored in the wallet contract on-chain. The private key never leaves the device.

### The `__check_auth` function

Soroban's authorization framework calls `__check_auth` on a C-account when it needs to verify that account authorized a given invocation. For a passkey wallet:

```rust
fn __check_auth(
    env: Env,
    signature_payload: BytesN<32>,  // SHA-256 hash of the auth preimage
    signature: WebAuthnSignature,   // authenticatorData + clientDataJSON + compact sig
    auth_context: Vec<Context>,
) -> Result<(), Error> {
    let stored_public_key = env.storage().instance().get(&DataKey::Signer);
    verify_webauthn_signature(&env, &stored_public_key, &signature_payload, &signature)
}
```

The Soroban runtime handles replay prevention (nonces) and expiry — the wallet contract only needs to verify the cryptographic signature itself.

---

## Signing a `claim_bounty` Invocation with a Passkey

C-accounts cannot sign transaction envelopes. They authorize via **auth entries** — a detachable signature over a specific contract invocation, separate from the transaction that submits it. This enables **sponsored transactions**: a fee-paying G-account submits the transaction while the contributor's C-account authorizes the action.

### Step-by-step flow

```
Contributor (browser)          MergeMint backend / relayer        Stellar network
        │                               │                               │
        │  1. Request claim options     │                               │
        │──────────────────────────────▶│                               │
        │                               │ Simulate claim_bounty         │
        │                               │ (Recording Mode) to get       │
        │                               │ auth entry needing signature   │
        │                               │                               │
        │                               │ Derive WebAuthn challenge      │
        │                               │ from simulated TX              │
        │◀──────────────────────────────│                               │
        │  2. WebAuthn prompt           │                               │
        │  (Touch ID / Face ID)         │                               │
        │  Passkey signs challenge      │                               │
        │──────────────────────────────▶│                               │
        │  3. authenticatorData +       │                               │
        │     clientDataJSON + sig      │                               │
        │                               │ Attach signature as auth       │
        │                               │ entry on claim_bounty TX       │
        │                               │                               │
        │                               │ Re-simulate (Enforcing Mode)  │
        │                               │ Fee-payer G-account signs     │
        │                               │ and submits TX                │
        │──────────────────────────────▶│                               │
        │                               │ __check_auth runs             │
        │                               │ on wallet contract,           │
        │                               │ verifies passkey sig          │
        │◀──────────────────────────────│◀──────────────────────────────│
        │  4. Claim confirmed           │                               │
```

### Key points

**The WebAuthn challenge must not be random.** It must be derived from the simulated transaction. This binds the passkey signature to the exact `claim_bounty` invocation the contributor is authorizing — preventing any substitution attack.

**Two simulation passes are required:**
- *Recording Mode* (first pass): skips `require_auth` validation, returns which auth entries need signatures.
- *Enforcing Mode* (second pass, after signing): executes `__check_auth` on the wallet contract to validate signatures and produce accurate resource/fee estimates.

**The contributor never pays XLM fees.** A backend-controlled G-account (the fee-payer / relayer) acts as the transaction source. The contributor's C-account only signs the auth entry.

---

## What Changes Are Required in the MergeMint Contract

**Nothing.** The MergeMint contract requires no modification to support passkey-based callers.

The contract calls `contributor.require_auth()` in `claim_bounty`. Soroban's authorization framework resolves this for any address type — G-account or C-account — by invoking `__check_auth` on the C-account's wallet contract. The MergeMint contract never sees the passkey signature; the runtime handles it transparently.

The `Address` type in Soroban is polymorphic: it represents both G-accounts and C-accounts. `require_auth()` works the same way on both.

---

## Limitations and Current Constraints

**Fee sponsorship is required.** C-accounts cannot be the transaction source and cannot pay fees directly. A relayer or backend service must hold a funded G-account to submit transactions on contributors' behalf. MergeMint's platform layer needs to implement this.

**Recovery must be planned.** If a user loses access to their passkey device, recovery requires a pre-configured backup mechanism (e.g., a backend-controlled recovery signer that can call `rotate_signer` on the wallet contract). This is a backend / wallet contract concern, not a MergeMint contract concern.

**Algorithm support is ES256 / secp256r1.** Soroban supports secp256r1 natively for `__check_auth` signature verification. Passkey credentials must be generated

---

## Frontend Integration (`frontend/src/lib/passkey.ts`)

The frontend `PasskeyAuth` helper must never fabricate security-sensitive values. The following rules apply to any implementation and are enforced by the Vitest suite in `frontend/src/lib/passkey.test.ts`:

**Challenges are server-issued and single-use.** The client must fetch the WebAuthn challenge from the backend (`GET /api/passkey/challenge`) rather than generating it with `crypto.getRandomValues`. The backend stores the challenge, binds it to the session, and consumes it on verification. A locally generated challenge cannot be validated server-side and is trivially replayable.

**`rp.id` is derived from the deployment host.** The relying-party ID must be `window.location.hostname` (or a configured parent domain), never a hardcoded value such as `mergemint.app`. A hardcoded `rp.id` breaks on localhost, preview deployments, and self-hosted instances, and causes the browser to reject the credential.

**Assertions are verified server-side.** After `navigator.credentials.get()` resolves, the client POSTs the raw assertion (`authenticatorData`, `clientDataJSON`, `signature`, `credentialId`) to `POST /api/passkey/verify`. The backend validates the challenge, origin, `rpIdHash`, and signature. The client must not treat a resolved promise as proof of authentication.

**Wallet addresses come from the backend.** The Soroban smart-wallet address (a `C...` strkey) is returned by the backend after successful verification. The client must never synthesize an address (e.g. `0x${Math.random()...}`); a fabricated address is not a valid Stellar address and cannot authorize on-chain actions.

**Errors are typed.** Failures surface as a typed `PasskeyError` with a stable `code` (`unsupported`, `challenge_fetch_failed`, `challenge_mismatch`, `verification_failed`, `cancelled`). Callers must not rely on `as any` casts or string matching.

**Unsupported browsers degrade gracefully.** When `window.PublicKeyCredential` or `navigator.credentials` is unavailable, the helper throws `PasskeyError` with code `unsupported` so the UI can fall back to an alternative sign-in method.
