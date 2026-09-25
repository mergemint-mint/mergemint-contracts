# MergeMint Backend API Guide

This guide is for integrators calling `mergemint-backend` over HTTP. It covers
every route mounted in `src/main.rs`, the global limits applied to all of
them, rate limits, and how to use `Idempotency-Key`.

Every curl example on this page was run against a local build (see
[Running locally](#running-locally)). The responses shown were captured from
that run. The in-memory store starts empty, so a few success responses are
taken from the handler code and unit tests instead. Those are marked
*(from code)*.

- [Running locally](#running-locally)
- [Conventions](#conventions)
- [Global limits and headers](#global-limits-and-headers)
- [Rate limits](#rate-limits)
- [Idempotency keys](#idempotency-keys)
- Routes:
  [`GET /bounties`](#get-bounties) ·
  [`GET /bounties/assignee/:address`](#get-bountiesassigneeaddress) ·
  [`POST /bounties/:id/claim`](#post-bountiesidclaim) ·
  [`GET /bounties/stream`](#get-bountiesstream) ·
  [`POST /tx/self-claim`](#post-txself-claim) ·
  [`POST /tx/resolve-dispute`](#post-txresolve-dispute)
- [Error reference](#error-reference)

---

## Running locally

```bash
cd mergemint-backend
CORS_ALLOWED_ORIGINS=http://localhost:5173 cargo run
# mergemint-backend listening address=0.0.0.0:8080
```

| Env var | Purpose |
|---|---|
| `CORS_ALLOWED_ORIGINS` | Comma-separated list of browser origins allowed to call the API. When empty or unset, no cross-origin browser requests are allowed. |
| `ALLOWLISTED_REWARD_TOKENS` | Comma-separated reward-token allow-list for create-bounty flows. A warning is logged at startup when it is empty. |
| `RUST_LOG` | Log filter. Default: `mergemint_backend=info,tower_http=debug,axum::rejection=trace`. |

The server always binds `0.0.0.0:8080`. The examples below use:

```bash
export BASE=http://localhost:8080
export ADDR=GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA   # 56-char StrKey
```

> **Path prefix:** the backend mounts its routes at the root (`/bounties`,
> `/tx/...`). The canonical frontend (`frontend/src/lib/api.ts`) calls
> `${VITE_API_BASE_URL ?? '/api/v1'}/bounties`, which assumes a reverse proxy
> that strips `/api/v1`. When you call the backend directly, leave the
> prefix off.

---

## Conventions

- Request bodies are JSON and **must** be sent with
  `Content-Type: application/json`. Otherwise the request is rejected with
  `415`.
- Handler errors use the `AppError` shape `{"code": <u16>, "message": "<string>"}`.
  The one exception is `GET /bounties/assignee/:address`, which returns
  `{"error": "<string>"}` for a bad address.
- Extractor errors (a malformed query string, missing JSON fields, a wrong
  content type) come from Axum. They return a **`text/plain`** body, not JSON.
- 5xx responses always carry `"message": "internal server error"`. Details are
  logged on the server and never returned to the client.
- Addresses are Stellar StrKeys: 56 chars, starting with `G` (account) or `C`
  (contract), using the base32 alphabet `A–Z2–7`.

---

## Global limits and headers

These are applied to **every** route by the middleware stack in `src/main.rs`.

| Concern | Value | Behaviour |
|---|---|---|
| Max request body | 1 MiB (`MAX_BODY_BYTES`) | `413 Payload Too Large` |
| Request timeout | 30 s (`REQUEST_TIMEOUT`) | `408 Request Timeout` (the request is cancelled) |
| Correlation ID | `x-request-id` | Echoed back when you send one, otherwise a UUID v4 is generated. It is included in every server log line for the request. |
| CORS | `CORS_ALLOWED_ORIGINS` | Methods `GET, POST`. Allowed request headers: **`content-type` only**. |

```bash
# Propagate your own trace ID
curl -i -H 'x-request-id: my-trace-123' "$BASE/bounties"
```

```http
HTTP/1.1 200 OK
content-type: application/json
x-request-id: my-trace-123

{"bounties":[],"next_cursor":null}
```

```bash
# Oversized body (1.1 MB) is rejected before reaching the handler
head -c 1100000 /dev/zero | tr '\0' a > big.json
curl -i -X POST "$BASE/tx/self-claim" -H 'Content-Type: application/json' --data-binary @big.json
# HTTP/1.1 413 Payload Too Large
```

```bash
# CORS preflight from an allowed origin
curl -i -X OPTIONS "$BASE/bounties" \
  -H 'Origin: http://localhost:5173' \
  -H 'Access-Control-Request-Method: POST' \
  -H 'Access-Control-Request-Headers: content-type'
```

```http
HTTP/1.1 200 OK
access-control-allow-origin: http://localhost:5173
access-control-allow-methods: GET,POST
access-control-allow-headers: content-type
```

A request from an origin that is not on the list gets no
`access-control-allow-origin` header, so the browser blocks the response.

> ⚠️ **Browser integrators:** `idempotency-key` and `x-request-id` are **not**
> in the CORS allowed-headers list, so a cross-origin browser `fetch` that sets
> them fails preflight. Server-to-server callers and same-origin deployments
> (behind the `/api/v1` proxy) are unaffected.

---

## Rate limits

| Route | Limit | Key | Window |
|---|---|---|---|
| `POST /tx/self-claim` | 10 requests (`SELF_CLAIM_RATE_LIMIT`) | `claimant` field of the body | Fixed 60 s window, starting at the key's first request |
| all other routes | none | — | — |

How it works:

- The counter is incremented **before** the handler logic runs, so failed
  attempts (404, 400) count toward the limit too.
- An idempotent **replay**, or a `409` for a key that is already in flight,
  returns before the limiter runs, so it does **not** use up a token.
- The limiter is kept in process memory. With several replicas, each one
  enforces its own limit, and the counts reset when the process restarts.
- No `Retry-After` header is sent. Back off until the 60 s window has elapsed.

```bash
C=GBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB
for i in $(seq 1 11); do
  curl -s -o /dev/null -w '%{http_code} ' -X POST "$BASE/tx/self-claim" \
    -H 'Content-Type: application/json' \
    -d "{\"bounty_id\":\"42\",\"claimant\":\"$C\"}"
done; echo
# 404 404 404 404 404 404 404 404 404 404 429
```

```http
HTTP/1.1 429 Too Many Requests
content-type: application/json

{"code":429,"message":"rate limit exceeded for self_claim; retry after the window resets"}
```

---

## Idempotency keys

`POST /tx/self-claim` builds a chain transaction, so retrying it naively after
a timeout could submit twice. Send an `Idempotency-Key` header to get
exactly-once behaviour:

| State of the key | Response |
|---|---|
| Not sent / empty | Processed normally, with no deduplication. |
| New | Reserved as *in flight*, then processed. |
| In flight (a concurrent request is still running) | `409 {"code":409,"message":"a request with this Idempotency-Key is already in progress"}` |
| Completed successfully | `200` replaying the **cached body** of the first success. The handler does not run again. |
| Previous attempt failed (4xx) | The reservation is released, so the same key can be reused after you fix the request. |

Guidelines:

- Generate a fresh random key (UUID v4) for each *logical* operation, and reuse
  it only when retrying that same operation.
- Keys are global, not per-route or per-user. Do not reuse a key for a
  different request body: you would get the first request's cached response.
- Keys are held in process memory with no expiry. They are lost on restart and
  are not shared across replicas.
- `POST /tx/resolve-dispute` does **not** read `Idempotency-Key` today. The
  header is ignored there.

```bash
KEY=$(uuidgen)   # or: python3 -c 'import uuid; print(uuid.uuid4())'
curl -i -X POST "$BASE/tx/self-claim" \
  -H 'Content-Type: application/json' \
  -H "Idempotency-Key: $KEY" \
  -d "{\"bounty_id\":\"42\",\"claimant\":\"$ADDR\"}"
```

Against the empty local store, this returns `404` and releases the key:

```http
HTTP/1.1 404 Not Found

{"code":404,"message":"bounty not found"}
```

---

## Routes

### `GET /bounties`

Lists bounties newest-first with cursor pagination.

| Query param | Type | Default | Notes |
|---|---|---|---|
| `limit` | integer | `20` | Clamped to `100` (`MAX_LIST_LIMIT`). |
| `cursor` | RFC 3339 timestamp | none | Returns rows with `created_at` strictly older than the cursor. Pass the previous page's `next_cursor`. URL-encode `+` offsets as `%2B`. |

```bash
curl -i "$BASE/bounties?limit=5"
```

```http
HTTP/1.1 200 OK
content-type: application/json

{"bounties":[],"next_cursor":null}
```

A populated page looks like this *(from code)*:

```json
{
  "bounties": [
    {"id": "1", "creator": "G...", "assignee": null, "created_at": "2026-09-01T12:00:00Z"}
  ],
  "next_cursor": "2026-09-01T12:00:00+00:00"
}
```

```bash
# Next page
curl -i "$BASE/bounties?limit=10&cursor=2026-09-01T00:00:00Z"
# 200 {"bounties":[],"next_cursor":null}

# Malformed cursor → 400, text/plain
curl -i "$BASE/bounties?cursor=notadate"
# HTTP/1.1 400 Bad Request
# Failed to deserialize query string: input contains invalid characters
```

---

### `GET /bounties/assignee/:address`

Lists bounties whose recorded assignee is `address`. Takes the same `limit` /
`cursor` params and returns the same page shape as `GET /bounties`.

```bash
curl -i "$BASE/bounties/assignee/$ADDR"
```

```http
HTTP/1.1 200 OK
content-type: application/json

{"bounties":[],"next_cursor":null}
```

A syntactically invalid address is rejected before the store is queried:

```bash
curl -i "$BASE/bounties/assignee/not-an-address"
```

```http
HTTP/1.1 400 Bad Request
content-type: application/json

{"error":"assignee address is not a syntactically valid Stellar address"}
```

---

### `POST /bounties/:id/claim`

Broadcasts `id` on the [bounty stream](#get-bountiesstream) and acknowledges
it. There is no request body.

> **Stub:** this route does not authenticate the caller, check that the
> bounty exists, or touch the chain. The real claim happens on-chain through
> the contract's `claim_bounty`, which the user's wallet signs. Treat this
> route as a notification hook only.

```bash
curl -i -X POST "$BASE/bounties/42/claim"
```

```http
HTTP/1.1 200 OK
content-type: application/json

{"id":"42","status":"claimed"}
```

---

### `GET /bounties/stream`

A Server-Sent Events stream of bounty state changes. It emits one event per
`POST /bounties/:id/claim`:

```
event: bounty_updated
data: {"bountyId":"<id>"}
```

Keep-alive comments are sent periodically. The broadcast buffer holds 100
messages, so a client that falls further behind silently skips the missed
events. After reconnecting, refetch `GET /bounties`.

```bash
# Terminal 1
curl -N "$BASE/bounties/stream"

# Terminal 2
curl -s -X POST "$BASE/bounties/7/claim"
```

Terminal 1 output:

```
event: bounty_updated
data: {"bountyId":"7"}
```

---

### `POST /tx/self-claim`

Builds a payout transaction that lets a claimant collect a bounty after its
staleness window (`expires_at`) has passed without the creator resolving it.
The contract enforces the same rule on-chain. This check only avoids building
transactions that are bound to fail.

**Rate limited** (10 / 60 s per `claimant`) and **idempotent** with
`Idempotency-Key`. See above.

| Body field | Type | Notes |
|---|---|---|
| `bounty_id` | string | Required. |
| `claimant` | string | Required. It is also the rate-limit key. |

```bash
curl -i -X POST "$BASE/tx/self-claim" \
  -H 'Content-Type: application/json' \
  -H "Idempotency-Key: $(uuidgen)" \
  -d "{\"bounty_id\":\"42\",\"claimant\":\"$ADDR\"}"
```

Responses:

| Status | Body | When |
|---|---|---|
| `200` | `{"ok":true,"xdr":"<unsigned tx>"}` *(from code)* | The staleness window has elapsed. Sign the XDR with the wallet and submit it. |
| `400` | `{"code":400,"message":"self-claim not yet available: staleness window has not elapsed"}` | `now <= expires_at` |
| `404` | `{"code":404,"message":"bounty not found"}` | Unknown `bounty_id` (this is what the empty local store returns). |
| `409` | `{"code":409,"message":"a request with this Idempotency-Key is already in progress"}` | A concurrent request is using the same key. |
| `415` / `422` | text/plain | Missing `Content-Type`, or missing fields. |
| `429` | `{"code":429,"message":"rate limit exceeded for self_claim; retry after the window resets"}` | Rate limit hit. |

> The returned `xdr` is currently a placeholder string
> (`XDR:bounty=…,winner=…,amount=…`) from the stub `build_payout_xdr`. It is
> not a real Stellar envelope yet.

---

### `POST /tx/resolve-dispute`

Builds the payout transaction that resolves a dispute in favour of `winner`.
Only the bounty creator may act as arbitrator. This is checked before the XDR
is built. Every call is audit-logged with `verifier`, `bounty_id`, `outcome`
(`success`/`failure`) and `timestamp`.

Not rate limited, and it does not read `Idempotency-Key`.

| Body field | Type | Notes |
|---|---|---|
| `bounty_id` | string | Required. |
| `arbitrator` | string | Required. Must equal the bounty's `creator`. |
| `winner` | string | Required. Address to pay. |

```bash
curl -i -X POST "$BASE/tx/resolve-dispute" \
  -H 'Content-Type: application/json' \
  -d "{\"bounty_id\":\"42\",\"arbitrator\":\"$ADDR\",\"winner\":\"$ADDR\"}"
```

```http
HTTP/1.1 404 Not Found
content-type: application/json

{"code":404,"message":"bounty not found"}
```

Responses:

| Status | Body | When |
|---|---|---|
| `200` | `{"ok":true,"xdr":"<unsigned tx>"}` *(from code)* | `arbitrator` is the creator. |
| `400` | `{"code":400,"message":"only the bounty creator may act as arbitrator"}` | Arbitrator mismatch. |
| `404` | `{"code":404,"message":"bounty not found"}` | Unknown `bounty_id`. |
| `415` | `Expected request with \`Content-Type: application/json\`` | Header missing. |
| `422` | `Failed to deserialize the JSON body into the target type: missing field \`arbitrator\` …` | A required field is missing. |

```bash
# Missing field → 422 (text/plain)
curl -i -X POST "$BASE/tx/resolve-dispute" -H 'Content-Type: application/json' -d '{"bounty_id":"42"}'

# Missing Content-Type → 415 (text/plain)
curl -i -X POST "$BASE/tx/resolve-dispute" -d '{"bounty_id":"42"}'
```

> The backend only *builds* the transaction. The arbitrator's wallet must sign
> it, and the contract's `resolve_dispute` re-checks every guard on-chain (see
> [docs/contract-reference.md](../../docs/contract-reference.md#resolve_dispute)).

---

## Error reference

| Status | Source | Body type | Typical cause |
|---|---|---|---|
| 400 | handler | JSON `AppError` (or `{"error":…}` on the assignee route) | Domain precondition failed, or a bad address. |
| 400 | Axum `Query` | text/plain | Malformed query string, such as a non-RFC 3339 `cursor`. |
| 404 | handler | JSON `AppError` | Unknown bounty. |
| 404 | router | empty | Unknown path. |
| 405 | router | empty | Wrong method, e.g. `GET /tx/self-claim`. |
| 408 | `TimeoutLayer` | empty | The request took longer than 30 s. |
| 409 | idempotency | JSON `AppError` | The same `Idempotency-Key` is still in flight. |
| 413 | `RequestBodyLimitLayer` | text/plain | The body is larger than 1 MiB. |
| 415 | Axum `Json` | text/plain | `Content-Type` is not `application/json`. |
| 422 | Axum `Json` | text/plain | Missing or mistyped JSON fields. |
| 429 | rate limiter | JSON `AppError` | `self_claim` limit exceeded. |
| 5xx | handler | JSON `AppError`, message always `"internal server error"` | Server-side failure. Look up the details in the logs by `x-request-id`. |
