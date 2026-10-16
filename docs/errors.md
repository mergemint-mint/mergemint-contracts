# MergeMint Unified API & Error Code Reference

This document provides a single, authoritative reference for errors across all three layers of the MergeMint platform:
1. **On-chain Contract Errors** (Soroban smart contract numeric error codes 1–7)
2. **Backend API Errors** (`mergemint-backend` REST & SSE `AppError` responses)
3. **TypeScript SDK Errors** (`@mergemint/sdk` typed error classes and `MergeMintErrorCode` variants)

---

## 1. Cross-Layer Unified Mapping Table

| Contract Code | Contract Name | Backend Status | Backend Error Message | SDK Error Class | SDK Error Code | Suggested User-Facing UI Message |
|:---:|---|:---:|---|---|---|---|
| **1** | `Unauthorized` | `401 / 403` | `unauthorized` | `UnauthorizedError` | `UNAUTHORIZED` | You are not authorized to perform this action. Please check your connected wallet. |
| **2** | `NotFound` | `404` | `bounty not found` | `NotFoundError` | `NOT_FOUND` | The requested bounty or contributor record was not found. |
| **3** | `InvalidInput` | `400` | `invalid input parameters` | `InvalidArgumentError` | `INVALID_ARGUMENT` | Invalid input parameters. Please check all form fields and values. |
| **4** | `AlreadyExists` | `400` | `resource already exists` | `SimulationFailedError` | `SIMULATION_FAILED` | This claim or bounty already exists. Duplicate operations are rejected. |
| **5** | `InsufficientBalance` | `400` | `insufficient token balance` | `SimulationFailedError` | `SIMULATION_FAILED` | Insufficient token balance in your wallet to cover escrow funding. |
| **6** | `InvalidState` | `400` | `invalid bounty state transition` | `SimulationFailedError` | `SIMULATION_FAILED` | The bounty is not in the required state for this action (e.g. not in progress). |
| **7** | `Overflow` | `400` | `arithmetic overflow` | `SimulationFailedError` | `SIMULATION_FAILED` | Calculation overflow detected. Please verify numerical inputs. |
| — | *N/A (Idempotency)* | `409` | `request already in flight` | `MergeMintSdkError` | `SIMULATION_FAILED` | A conflicting operation is currently in flight. Please wait a moment. |
| — | *N/A (Rate Limit)* | `429` | `rate limit exceeded` | `MergeMintSdkError` | `SIMULATION_FAILED` | Too many requests. Please wait a moment before trying again. |
| — | *N/A (Config)* | — | *Client-only* | `InvalidConfigError` | `INVALID_CONFIG` | Invalid network configuration or contract parameters. |
| — | *N/A (Contract ID)* | — | *Client-only* | `InvalidContractIdError` | `INVALID_CONTRACT_ID` | The configured contract ID is invalid or a placeholder. |
| — | *N/A (RPC URL)* | — | *Client-only* | `InvalidRpcUrlError` | `INVALID_RPC_URL` | The configured Soroban RPC URL is invalid or unreachable. |
| — | *N/A (Fee Bump)* | — | *Client-only* | `FeeBumpFailedError` | `FEE_BUMP_FAILED` | Network congestion exceeded retry threshold. Please retry with a higher base fee. |
| — | *N/A (Server Error)* | `500` | `internal server error` | `TransactionFailedError` | `TRANSACTION_FAILED` | Service temporarily unavailable. Please retry shortly. |

---

## 2. On-Chain Contract Errors (`src/errors.rs`)

Soroban contracts use numeric error codes mapped via the `#[contracterror]` macro. When a precondition fails, the contract invokes `soroban_sdk::panic_with_error!(&env, err)`.

### Code 1: `Unauthorized`
- **Definition:** Caller is not permitted to perform this action.
- **Trigger Points:**
  - `complete_bounty`: Caller is not an assigned contributor.
  - `approve_completion`: Caller is neither the creator nor an authorized verifier.
  - `extend_deadline`: Caller is not the bounty creator.
  - `cancel_bounty`: Caller is not the bounty creator or contract admin.
- **Resolution:** Verify that the signing wallet matches the expected role (`require_auth()`).

### Code 2: `NotFound`
- **Definition:** The requested resource does not exist in instance or persistent storage.
- **Trigger Points:**
  - Passing an unregistered `bounty_id` to any mutation or query.
  - Querying contributor profile for an address that has made zero interactions.
- **Resolution:** Verify the `bounty_id` hex string against on-chain events or indexer listings.

### Code 3: `InvalidInput`
- **Definition:** One or more arguments fail validation bounds.
- **Trigger Points:**
  - `reward_amount == 0` or below minimum threshold.
  - Empty or invalid symbol strings (exceeding 32 characters).
  - Empty tags or assignees vectors when non-empty vectors are strictly required.
- **Resolution:** Check parameters before assembling the invocation.

### Code 4: `AlreadyExists`
- **Definition:** Attempted duplicate registration of an existing entity.
- **Trigger Points:**
  - `claim_bounty`: Contributor has already claimed the specified bounty.
  - Initializing an already initialized contract instance.
- **Resolution:** Ensure the contributor has not already submitted an active claim.

### Code 5: `InsufficientBalance`
- **Definition:** The funding account lacks sufficient tokens for the required transfer.
- **Trigger Points:**
  - `create_bounty`: Creator lacks enough SEP-41 reward tokens to fund escrow.
  - `top_up`: Funder does not hold the specified additional amount.
- **Resolution:** Prompt the user to deposit additional tokens into their Stellar account.

### Code 6: `InvalidState`
- **Definition:** The bounty state machine rejected an invalid lifecycle transition.
- **Trigger Points:**
  - Claiming a bounty whose status is not `Open`.
  - Completing a bounty whose status is not `InProgress`.
  - Approving a bounty that has already been approved, cancelled, or is under active dispute.
  - Calling `top_up` after a bounty has already expired or completed.
- **Resolution:** Check `get_bounty(id).status` prior to presenting write actions in the UI.

### Code 7: `Overflow`
- **Definition:** Integer calculation exceeded limits (checked arithmetic guard).
- **Trigger Points:**
  - Sum of milestone reward allocations exceeds total `reward_amount`.
  - Total assignees share basis points (`share_bp`) sum exceeds 10,000 (100%).
- **Resolution:** Verify milestone sum and basis points calculations client-side.

---

## 3. Backend REST & SSE Errors (`mergemint-backend`)

The backend REST API exposes errors through standard JSON payloads conforming to `AppError`:

```json
{
  "code": 400,
  "message": "bounty not found"
}
```

### HTTP Status Codes
- **400 Bad Request:** Malformed payload, failed validation bounds, or contract domain precondition failure.
- **401 / 403 Unauthorized:** Missing or invalid authentication token / signature.
- **404 Not Found:** Requested bounty or entity is missing from the database.
- **409 Conflict:** Idempotency key conflict (transaction already in flight or committed).
- **429 Too Many Requests:** Rate limit exceeded on sliding window limit.
- **500 Internal Server Error:** Unexpected backend exception. Details are strictly redacted in production.

### Redaction & Security Semantics
- **500 Internal Server Error:** Detail messages are strictly redacted in production. Raw database query strings, stack traces, and internal URLs are never sent over the wire.
- **Validation Errors (400):** Sanitized messages inform the client of the specific invalid property without reflecting unescaped input (XSS prevention).
- **Rate Limiting (429):** Returned when IP or principal exceeds sliding window limits (configured per endpoint).

---

## 4. TypeScript SDK Errors (`@mergemint/sdk`)

All SDK errors inherit from `MergeMintSdkError`:

```typescript
try {
  await sdk.claimBounty(contributor, bountyId, contributor);
} catch (err) {
  if (err instanceof UnauthorizedError) {
    console.error("Wallet signature mismatch");
  } else if (err instanceof NotFoundError) {
    console.error("Bounty does not exist");
  } else if (err instanceof SimulationFailedError) {
    console.error("Simulation failed:", err.rawMessage);
  }
}
```

### SDK Error Classes & Codes

| Class | `err.code` (`MergeMintErrorCode`) | Description |
|---|---|---|
| `UnauthorizedError` | `UNAUTHORIZED` | Caller lacked authorization for on-chain invocation |
| `NotFoundError` | `NOT_FOUND` | Resource not found on-chain or in RPC state |
| `InvalidArgumentError` | `INVALID_ARGUMENT` | Client-side argument validation failed |
| `SimulationFailedError` | `SIMULATION_FAILED` | Soroban RPC simulation returned an error result |
| `TransactionFailedError` | `TRANSACTION_FAILED` | Transaction submission failed or was dropped |
| `InvalidConfigError` | `INVALID_CONFIG` | Provided network configuration is invalid |
| `InvalidContractIdError` | `INVALID_CONTRACT_ID` | Provided contract ID is invalid or placeholder |
| `InvalidRpcUrlError` | `INVALID_RPC_URL` | Provided Soroban RPC URL is invalid or malformed |
| `FeeBumpFailedError` | `FEE_BUMP_FAILED` | All fee-bump retry attempts exhausted under congestion |

---

## 5. Drift Prevention & Automated Testing

To ensure documentation never drifts from code:
- **Contract Codes:** All numeric codes 1–7 in `src/errors.rs` are verified against the documented codes in unit test `test_contract_errors_match_documented_reference`.
- **SDK Error Codes:** All 9 variants of `MergeMintErrorCode` in `sdk/src/types.ts` are verified against `docs/errors.md` in `sdk/src/sdk-issues.test.ts`.
- **Backend Error Mapping:** Redaction and error response formats in `mergemint-backend/src/routes/tx.rs` are tested against mock error fixtures.
