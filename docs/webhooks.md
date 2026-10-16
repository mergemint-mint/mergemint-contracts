# Webhook Payload and Signature Reference

MergeMint dispatches real-time HTTP `POST` notifications to registered subscriber endpoints whenever bounty events occur on the contract and backend.

---

## 1. Delivery Headers

Every webhook request delivered by MergeMint includes standard headers to identify the event, unique delivery ID, and cryptographic signatures:

| Header | Description | Example |
|---|---|---|
| `Content-Type` | MIME payload type | `application/json` |
| `X-MergeMint-Signature` | HMAC-SHA256 signature prefix with `sha256=` | `sha256=a1b2c3d4e5f6...` |
| `X-Hub-Signature-256` | GitHub-compatible HMAC-SHA256 signature header | `sha256=a1b2c3d4e5f6...` |
| `X-MergeMint-Event` | Event identifier string | `bounty.created` |
| `X-MergeMint-Delivery` | Unique UUID string for this delivery attempt | `evt_8f1a2b3c` |

---

## 2. Managing Subscriptions

### Register a Webhook Endpoint

Send a `POST` request to `/api/webhooks/subscriptions`:

```bash
curl -X POST https://api.mergemint.io/api/webhooks/subscriptions \
  -H "Content-Type: application/json" \
  -d '{
    "url": "https://example.com/api/webhooks",
    "secret": "whsec_your_shared_signing_secret_here",
    "event_types": ["bounty.created", "bounty.completed", "reward.paid"]
  }'
```

* Leave `event_types` empty or use `["*"]` to subscribe to all events.

### List Subscriptions

```bash
curl https://api.mergemint.io/api/webhooks/subscriptions
```

### Delete a Subscription

```bash
curl -X DELETE https://api.mergemint.io/api/webhooks/subscriptions/{subscription_id}
```

---

## 3. Payload Envelope

All webhooks share a top-level JSON envelope containing metadata and a contextual `data` payload:

```json
{
  "id": "evt_b10a4e76d8b9432f",
  "event": "bounty.created",
  "timestamp": "2026-10-16T22:00:00Z",
  "bounty_id": "bounty_42",
  "data": {
    "title": "Add RFC 7807 error formatting",
    "creator": "GBR4...STELLAR",
    "reward_amount": "1000",
    "reward_token": "CDLDVFKHEZ2RVB3NG4UQA4VPD3TSHV6XMHXMHP2BSGCJ2IIWVTOHGDSG",
    "deadline": 100000,
    "status": "open"
  }
}
```

---

## 4. Supported Event Types

### `bounty.created`
Fired when a new bounty is registered and escrowed in the contract.
```json
{
  "id": "evt_001",
  "event": "bounty.created",
  "timestamp": "2026-10-16T22:01:00Z",
  "bounty_id": "bounty_101",
  "data": {
    "title": "Implement Webhook Verification",
    "creator": "GA7QYNF7SOWQ3GLR2BGMZEHXAVIRZA4KVWLTJJFC7MGXUA74P7UJVWAC",
    "reward_amount": "500",
    "reward_token": "CAS3J7GYLGXMF6TDJBBYYSE3VUMAS45GQQ7AOFQVU4AQ52DGXZAOVP7C",
    "tags": ["feature", "backend"],
    "deadline": 120500,
    "status": "open"
  }
}
```

### `bounty.claimed`
Fired when a contributor claims an open bounty.
```json
{
  "id": "evt_002",
  "event": "bounty.claimed",
  "timestamp": "2026-10-16T22:05:00Z",
  "bounty_id": "bounty_101",
  "data": {
    "bounty_id": "bounty_101",
    "contributor": "GCFXHS4GMXC3CF2NTUAW2HGNYFSTBXQUSP4YZHTHY3R5662K4O64AOWS",
    "status": "in_progress"
  }
}
```

### `bounty.completed`
Fired when verifiers approve the contributor's solution.
```json
{
  "id": "evt_003",
  "event": "bounty.completed",
  "timestamp": "2026-10-16T22:15:00Z",
  "bounty_id": "bounty_101",
  "data": {
    "bounty_id": "bounty_101",
    "contributor": "GCFXHS4GMXC3CF2NTUAW2HGNYFSTBXQUSP4YZHTHY3R5662K4O64AOWS",
    "status": "completed"
  }
}
```

### `reward.paid`
Fired after token escrow transfer confirms to the contributor's wallet.
```json
{
  "id": "evt_004",
  "event": "reward.paid",
  "timestamp": "2026-10-16T22:15:05Z",
  "bounty_id": "bounty_101",
  "data": {
    "bounty_id": "bounty_101",
    "recipient": "GCFXHS4GMXC3CF2NTUAW2HGNYFSTBXQUSP4YZHTHY3R5662K4O64AOWS",
    "amount": "500",
    "token": "CAS3J7GYLGXMF6TDJBBYYSE3VUMAS45GQQ7AOFQVU4AQ52DGXZAOVP7C"
  }
}
```

### `bounty.expired`
Fired when a bounty passes its deadline and escrow is refunded.
```json
{
  "id": "evt_005",
  "event": "bounty.expired",
  "timestamp": "2026-10-16T22:30:00Z",
  "bounty_id": "bounty_102",
  "data": {
    "bounty_id": "bounty_102",
    "creator": "GA7QYNF7SOWQ3GLR2BGMZEHXAVIRZA4KVWLTJJFC7MGXUA74P7UJVWAC",
    "status": "cancelled",
    "refunded": true
  }
}
```

### `bounty.disputed`
Fired when a dispute is opened by the creator or assignee.
```json
{
  "id": "evt_006",
  "event": "bounty.disputed",
  "timestamp": "2026-10-16T22:35:00Z",
  "bounty_id": "bounty_103",
  "data": {
    "bounty_id": "bounty_103",
    "initiator": "GA7QYNF7SOWQ3GLR2BGMZEHXAVIRZA4KVWLTJJFC7MGXUA74P7UJVWAC",
    "status": "disputed"
  }
}
```

---

## 5. Signature Verification

Always verify the HMAC-SHA256 signature using the **raw binary request body** before processing payloads.

### Node.js Example

```javascript
const crypto = require('crypto');

function verifyWebhook(secret, rawBody, signatureHeader) {
  if (!signatureHeader || !signatureHeader.startsWith('sha256=')) {
    return false;
  }
  const receivedHex = signatureHeader.slice('sha256='.length);
  const expectedHex = crypto
    .createHmac('sha256', secret)
    .update(rawBody)
    .digest('hex');

  const receivedBuf = Buffer.from(receivedHex, 'hex');
  const expectedBuf = Buffer.from(expectedHex, 'hex');

  if (receivedBuf.length !== expectedBuf.length) {
    return false;
  }
  return crypto.timingSafeEqual(receivedBuf, expectedBuf);
}

// Express route example:
// app.post('/webhook', express.raw({ type: 'application/json' }), (req, res) => {
//   const signature = req.headers['x-mergemint-signature'];
//   if (!verifyWebhook(process.env.WEBHOOK_SECRET, req.body, signature)) {
//     return res.status(401).send('Invalid signature');
//   }
//   const payload = JSON.parse(req.body.toString('utf8'));
//   console.log('Received valid event:', payload.event);
//   res.status(200).send('OK');
// });
```

### Python Example

```python
import hmac
import hashlib

def verify_webhook(secret: str, raw_body: bytes, signature_header: str) -> bool:
    if not signature_header or not signature_header.startswith("sha256="):
        return False
    received_hex = signature_header[len("sha256="):]
    expected_hex = hmac.new(
        key=secret.encode("utf-8"),
        msg=raw_body,
        digestmod=hashlib.sha256
    ).hexdigest()
    return hmac.compare_digest(received_hex, expected_hex)

# FastAPI route example:
# @app.post("/webhook")
# async def handle_webhook(request: Request):
#     raw_body = await request.body()
#     sig = request.headers.get("X-MergeMint-Signature", "")
#     if not verify_webhook(SECRET, raw_body, sig):
#         raise HTTPException(status_code=401, detail="Invalid signature")
#     data = await request.json()
#     return {"status": "success"}
```

---

## 6. Delivery Retries and Backoff Behavior

MergeMint guarantees at-least-once delivery for webhook events under the following retry policy:

* **Exponential Backoff:** If the receiver responds with a server error (`5xx`), network timeout, or rate limit (`429 Too Many Requests`), the dispatcher retries up to **3 attempts**:
  * **Attempt 1:** Immediate initial attempt.
  * **Attempt 2:** Delay of `200 ms`.
  * **Attempt 3:** Delay of `400 ms` (scaled by factor `2.0`, capped at `30 seconds`).
* **Non-Retryable Errors:** Client errors (`4xx` other than `429`, such as `400 Bad Request` or `404 Not Found`) indicate unrecoverable payload or path issues and are **not retried**.
* **Idempotency:** Because deliveries may be retried, consumer endpoints should track processed `X-MergeMint-Delivery` IDs or `payload.id` to guarantee idempotent processing.
