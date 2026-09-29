# Integrations

## Freighter Deep-Link for `claim_bounty`

### Overview

[Freighter](https://www.freighter.app/) is the most widely used Stellar browser
wallet. It supports a transaction deep-link scheme that lets a web page open
Freighter with a pre-built transaction ready for the user to review and sign —
no manual transaction construction required.

Using this scheme, the MergeMint frontend can render a single "Claim" button
that, when clicked, opens Freighter with the correct `claim_bounty` invocation
pre-filled.

---

### Prerequisites

| Requirement | Notes |
|---|---|
| Freighter ≥ 5.5.0 | Earlier versions do not support the `stellar:` URI scheme |
| Stellar SDK (JS) | `@stellar/stellar-sdk` ≥ 12.x for XDR helpers |
| Network | Testnet (`Test SDF Network ; September 2015`) or Mainnet (`Public Global Stellar Network ; September 2015`) |

---

### The `stellar:` URI scheme

Freighter recognises URIs of the form:

```
stellar:<base64url-encoded XDR transaction envelope>
```

The XDR envelope encodes a `TransactionEnvelope` containing a single
`InvokeHostFunction` operation that calls `claim_bounty` on the MergeMint
contract.

---

### XDR transaction structure

The `InvokeHostFunction` operation must supply:

| Field | Value |
|---|---|
| `contract_id` | The deployed MergeMint contract address |
| `function` | `claim_bounty` |
| `args[0]` | `ScVal::Address` — the contributor's Stellar account address |
| `args[1]` | `ScVal::Bytes(32)` — the `BytesN<32>` bounty ID |

No footprint override is needed; Soroban simulation resolves the read/write
set automatically when the link is opened by Freighter.

---

### Generating the deep-link (JavaScript)

```js
import {
  Contract,
  Networks,
  TransactionBuilder,
  BASE_FEE,
  nativeToScVal,
  xdr,
} from "@stellar/stellar-sdk";
import { SorobanRpc } from "@stellar/stellar-sdk";

const RPC_URL = "https://soroban-testnet.stellar.org"; // or mainnet RPC
const CONTRACT_ID = "C..."; // deployed MergeMint contract address
const NETWORK_PASSPHRASE = Networks.TESTNET; // or Networks.PUBLIC

/**
 * Build and return a `stellar:` deep-link that opens Freighter with a
 * pre-filled claim_bounty transaction.
 *
 * @param {string} contributorAddress  - G... Stellar account of the contributor
 * @param {Uint8Array} bountyId        - 32-byte bounty ID
 * @returns {Promise<string>}          - the full `stellar:` URI
 */
export async function buildClaimBountyLink(contributorAddress, bountyId) {
  const server = new SorobanRpc.Server(RPC_URL);
  const account = await server.getAccount(contributorAddress);

  const contract = new Contract(CONTRACT_ID);

  const contributorScVal = nativeToScVal(contributorAddress, { type: "address" });
  const bountyIdScVal = xdr.ScVal.scvBytes(Buffer.from(bountyId));

  const tx = new TransactionBuilder(account, {
    fee: BASE_FEE,
    networkPassphrase: NETWORK_PASSPHRASE,
  })
    .addOperation(
      contract.call("claim_bounty", contributorScVal, bountyIdScVal)
    )
    .setTimeout(300) // 5-minute signing window
    .build();

  // Simulate to populate the transaction footprint (Soroban requirement)
  const simResult = await server.simulateTransaction(tx);
  if (SorobanRpc.Api.isSimulationError(simResult)) {
    throw new Error(`Simulation failed: ${simResult.error}`);
  }
  const preparedTx = SorobanRpc.assembleTransaction(tx, simResult).build();

  const xdrEnvelope = preparedTx.toEnvelope().toXDR("base64");
  // base64url-encode (replace + → -, / → _, strip =)
  const base64url = xdrEnvelope
    .replace(/\+/g, "-")
    .replace(/\//g, "_")
    .replace(/=+$/, "");

  return `stellar:${base64url}`;
}
```

---

### Example button (React)

```jsx
import { buildClaimBountyLink } from "./stellarUtils";

function ClaimButton({ bountyId, contributorAddress }) {
  const handleClaim = async () => {
    const link = await buildClaimBountyLink(contributorAddress, bountyId);
    window.location.href = link; // Freighter intercepts the stellar: scheme
  };

  return <button onClick={handleClaim}>Claim Bounty</button>;
}
```

---

### Manual verification on testnet

1. Deploy the contract to Testnet and note the contract address.
2. Call `create_bounty` to create a test bounty and capture the returned
   32-byte ID.
3. Run `buildClaimBountyLink` with a funded Testnet contributor address and the
   bounty ID.
4. Open the resulting `stellar:` URI in a browser with Freighter installed.
5. Confirm that Freighter opens the "Review transaction" screen showing a call
   to `claim_bounty` with the correct contract, contributor, and bounty ID.
6. Approve the transaction and verify on
   [Stellar Expert (Testnet)](https://stellar.expert/explorer/testnet) that the
   bounty status changed to `in_progress`.

---

### Limitations

- **Freighter version**: The `stellar:` URI scheme requires Freighter ≥ 5.5.0.
  Users on older versions will see a blank page or no response.
- **Network selection**: Freighter determines the network from the transaction's
  `networkPassphrase`. If the user's Freighter is set to a different network the
  transaction will be rejected at signing time.
- **Account must exist on-chain**: `TransactionBuilder` requires a sequence
  number, so the contributor account must be funded before the link is generated.
  For Testnet, use the
  [Friendbot](https://friendbot.stellar.org/?addr=<contributor_address>) to fund
  new accounts.
- **Deep-link size**: Very large XDR envelopes (rare for single-operation
  invocations) may exceed browser URL length limits. The assembled Soroban
  transaction for `claim_bounty` is well within typical limits (~2–4 KB).

---

## Webhook Notifications

### Overview

MergeMint backend provides real-time HTTP POST webhook notifications for bounty state transitions. External consumers (such as Discord bots, analytics monitors, and notification services) subscribe to bounty events and receive signed payloads with exponential backoff delivery retries.

Supported events:
- `bounty_created`: Emitted when a new bounty is published.
- `bounty_claimed`: Emitted when a contributor claims an open bounty.
- `bounty_completed`: Emitted when milestone or final completion is verified.
- `bounty_cancelled`: Emitted when an open bounty is cancelled by its creator.
- `bounty_updated`: Emitted when bounty metadata or state transitions occur.
- `*`: Wildcard subscription to receive all bounty events.

---

### Subscription Endpoints

| Method | Endpoint | Description |
|---|---|---|
| `POST` | `/webhooks/subscriptions` | Register a new webhook endpoint URL and shared secret |
| `GET` | `/webhooks/subscriptions` | List registered webhook subscriptions |
| `GET` | `/webhooks/subscriptions/:id` | Retrieve subscription details by ID |
| `DELETE` | `/webhooks/subscriptions/:id` | Delete an existing webhook subscription |

#### Create Subscription Request

`POST /webhooks/subscriptions`

```json
{
  "url": "https://example.com/api/mergemint-webhook",
  "secret": "your-secure-webhook-secret-key",
  "event_types": ["bounty_claimed", "bounty_completed"]
}
```

Validation constraints:
- `url`: Must be a valid absolute HTTP or HTTPS URL.
- `secret`: Non-empty string used as the HMAC-SHA256 pre-shared key.
- `event_types`: Optional array of event names or `["*"]` for wildcard. Defaults to wildcard if omitted or empty.

---

### Delivery Headers & Payload

Each delivery POST request includes standard delivery and security headers:

| Header | Description |
|---|---|
| `X-MergeMint-Signature` | Hex-encoded HMAC-SHA256 digest formatted as `sha256=<hex>` |
| `X-Hub-Signature-256` | Standard GitHub-compatible signature formatted as `sha256=<hex>` |
| `X-MergeMint-Event` | The event name (e.g. `bounty_claimed`) |
| `X-MergeMint-Delivery` | Unique UUID v4 identifying the delivery attempt |
| `Content-Type` | `application/json` |

#### Payload Schema

```json
{
  "id": "e4f3a76e-57b1-4cfa-93d8-5f042db6cae2",
  "event": "bounty_claimed",
  "timestamp": 1740000000,
  "bounty_id": "0x4a2e8c9b...",
  "data": {
    "id": "0x4a2e8c9b...",
    "creator": "GB...",
    "title": "Build soroban integration",
    "description": "Implement SDK integration",
    "reward_amount": "100000000",
    "reward_token": "native",
    "status": "in_progress",
    "assignee": "GC...",
    "deadline": 1750000000,
    "tags": ["soroban", "rust"],
    "created_at": 1739000000
  }
}
```

---

### Signature Verification

Subscribers verify payload authenticity by recomputing the HMAC-SHA256 signature using the shared secret and comparing it against the header using constant-time comparison to prevent timing attacks.

#### Node.js / TypeScript Example

```ts
import * as crypto from "crypto";

export function verifyMergeMintWebhook(
  rawBody: Buffer | string,
  signatureHeader: string,
  secret: string
): boolean {
  if (!signatureHeader.startsWith("sha256=")) {
    return false;
  }
  const receivedHex = signatureHeader.slice(7);
  const expectedHex = crypto
    .createHmac("sha256", secret)
    .update(rawBody)
    .digest("hex");

  const receivedBuffer = Buffer.from(receivedHex, "hex");
  const expectedBuffer = Buffer.from(expectedHex, "hex");

  if (receivedBuffer.length !== expectedBuffer.length) {
    return false;
  }
  return crypto.timingSafeEqual(receivedBuffer, expectedBuffer);
}
```

#### Rust Example

```rust
use hmac::{Hmac, Mac};
use sha2::Sha256;

pub fn verify_signature(secret: &str, raw_body: &[u8], header_value: &str) -> bool {
    let Some(signature_hex) = header_value.strip_prefix("sha256=") else {
        return false;
    };
    let Ok(expected_bytes) = hex::decode(signature_hex) else {
        return false;
    };
    let Ok(mut mac) = Hmac::<Sha256>::new_from_slice(secret.as_bytes()) else {
        return false;
    };
    mac.update(raw_body);
    mac.verify_slice(&expected_bytes).is_ok()
}
```

---

### Retry Policy & Error Handling

Deliveries follow an exponential backoff retry strategy:
- **Success Criteria**: HTTP status codes in the 2xx range (`200 OK` - `299`).
- **Transient Failures (Retried)**: HTTP 5xx server errors, HTTP 429 rate limit responses, and network connection/timeout failures.
  - Initial delay: 200 ms
  - Backoff multiplier: 2.0x
  - Maximum attempts: 3
- **Permanent Failures (Not Retried)**: HTTP 4xx client errors (excluding 429) indicate an invalid endpoint or client rejection; delivery aborts immediately without consuming retry quotas.

