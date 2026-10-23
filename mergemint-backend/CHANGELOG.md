# MergeMint Backend API Changelog

All notable changes to the `/api/tx/*` routes and the broader backend API are
documented here. Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

---

## [0.2.0](https://github.com/mergemint-mint/mergemint-contracts/compare/backend-v0.1.0...backend-v0.2.0) (2026-10-23)


### Added

* add glossary, mdBook docs, CSP, and security headers ([4b11961](https://github.com/mergemint-mint/mergemint-contracts/commit/4b11961a971dac7dfbb403509f39feb169cbe2e7))
* **backend:** add indexer reliability features ([f3f158b](https://github.com/mergemint-mint/mergemint-contracts/commit/f3f158b23ba68690c93ad8852040ca4ce513bd12))
* **backend:** add security headers middleware ([698ce41](https://github.com/mergemint-mint/mergemint-contracts/commit/698ce41e146d20b6e6bf930f129880d37cd4889c)), closes [#1115](https://github.com/mergemint-mint/mergemint-contracts/issues/1115)
* **backend:** resolve transaction and webhook issues ([3f84bbc](https://github.com/mergemint-mint/mergemint-contracts/commit/3f84bbcdff5273f2a2aaf9b0daa3c1176774288c))


### Fixed

* **backend:** recover stale webhook delivery leases ([944b2d2](https://github.com/mergemint-mint/mergemint-contracts/commit/944b2d2aa50e2874721cb35fab227344f8f8b862))
* **backend:** restore truncated db.rs and missing sha2 import ([748680e](https://github.com/mergemint-mint/mergemint-contracts/commit/748680e7c42774264fe7dc9d721dc89c4fdd4014))

## [Unreleased]

### Added
- Security headers on every response: `X-Content-Type-Options: nosniff`,
  `Referrer-Policy: no-referrer` and `X-Frame-Options: DENY`.
  `Strict-Transport-Security` is sent only when `HSTS_ENABLED=true`
  (`HSTS_MAX_AGE_SECS`, `HSTS_INCLUDE_SUBDOMAINS`). (#1115)
- `GET /api/v1/bounties/assignee/{address}` — paginated list of bounties where
  `address` is recorded as an assignee in the `assignees` join table. Symmetric
  counterpart to the existing `bounties_by_creator` query. (#481)
- `GET /api/v1/bounties/stream` — Server-Sent Events channel that broadcasts a
  bounty ID whenever `refresh_bounty` completes in the indexer. Clients receive
  `bounty_updated` events without polling. (#482)

### Security
- Webhook target URLs that resolve to private, loopback, link-local or cloud
  metadata addresses are refused at registration (`400`) and again at every
  delivery, through a DNS resolver that checks each address before
  connecting. This blocks DNS rebinding. Webhook requests no longer follow
  redirects or use proxies. `WEBHOOK_ALLOW_PRIVATE_TARGETS=true` allows
  private targets for local development. (#1116)
- `GET /bounties/stream` caps concurrent connections per client IP
  (`SSE_MAX_CONNECTIONS_PER_IP`, default 32) and in total
  (`SSE_MAX_CONNECTIONS`, default 1024), and returns `429` with `Retry-After`
  over a cap. `SSE_TRUST_X_FORWARDED_FOR=true` keys the per-IP cap on
  `X-Forwarded-For` behind a trusted proxy. New metrics:
  `mergemint_sse_active_connections` (gauge) and
  `mergemint_sse_rejected_connections_total{reason}` (counter). (#1118)

### Changed
- All stable endpoints now live under the `/api/v1/` prefix. Unprefixed
  `/api/*` paths remain as deprecated aliases and will be removed in a future
  minor release. (#483)

---

## [0.1.0] — 2026-07-01 (initial backfill)

### `/api/tx/*` route inventory

The following nine transaction-building routes were present at the initial
commit of `mergemint-backend`. All routes accept `Content-Type: application/json`
and return `application/json`.

#### `POST /api/tx/create-bounty`

Builds and returns a signed XDR envelope for creating a new on-chain bounty.

**Request**
```json
{
  "creator":     "<Stellar address>",
  "title":       "string (max 64 chars)",
  "description": "string (max 512 chars)",
  "reward":      "string (stroops)",
  "token":       "<Stellar asset code or contract ID>"
}
```

**Response** `200 OK`
```json
{ "xdr": "<base64-encoded TransactionEnvelope>" }
```

**Errors**
- `400` — missing or invalid field
- `422` — reward below on-chain minimum

---

#### `POST /api/tx/claim-bounty`

Builds the claim transaction for an open bounty.

**Request**
```json
{
  "bountyId": "<uuid>",
  "claimer":  "<Stellar address>"
}
```

**Response** `200 OK`
```json
{ "xdr": "<base64-encoded TransactionEnvelope>" }
```

**Errors**
- `404` — bounty not found
- `409` — bounty is not in `open` state

---

#### `POST /api/tx/submit-work`

Marks work as submitted for a claimed bounty.

**Request**
```json
{
  "bountyId":    "<uuid>",
  "contributor": "<Stellar address>",
  "workUrl":     "string (HTTPS URL)"
}
```

**Response** `200 OK`
```json
{ "xdr": "<base64-encoded TransactionEnvelope>" }
```

---

#### `POST /api/tx/approve-work`

Creator approves submitted work, triggering reward release.

**Request**
```json
{
  "bountyId": "<uuid>",
  "creator":  "<Stellar address>"
}
```

**Response** `200 OK`
```json
{ "xdr": "<base64-encoded TransactionEnvelope>" }
```

---

#### `POST /api/tx/reject-work`

Creator rejects submitted work, returning the bounty to `open`.

**Request**
```json
{
  "bountyId": "<uuid>",
  "creator":  "<Stellar address>",
  "reason":   "string (optional)"
}
```

**Response** `200 OK`
```json
{ "xdr": "<base64-encoded TransactionEnvelope>" }
```

---

#### `POST /api/tx/raise-dispute`

Either party raises a dispute on a bounty in `claimed` or `work_submitted` state.

**Request**
```json
{
  "bountyId":  "<uuid>",
  "initiator": "<Stellar address>",
  "reason":    "string"
}
```

**Response** `200 OK`
```json
{ "xdr": "<base64-encoded TransactionEnvelope>" }
```

---

#### `POST /api/tx/resolve-dispute`

Platform arbitrator resolves a disputed bounty.

**Request**
```json
{
  "bountyId":   "<uuid>",
  "arbitrator": "<Stellar address>",
  "resolution": "creator | contributor | split",
  "splitBps":   "number (0–10000, only when resolution=split)"
}
```

**Response** `200 OK`
```json
{ "xdr": "<base64-encoded TransactionEnvelope>" }
```

---

#### `POST /api/tx/cancel-bounty`

Creator cancels an open bounty and recovers the escrowed reward.

**Request**
```json
{
  "bountyId": "<uuid>",
  "creator":  "<Stellar address>"
}
```

**Response** `200 OK`
```json
{ "xdr": "<base64-encoded TransactionEnvelope>" }
```

---

#### `POST /api/tx/refresh-bounty`

Syncs the on-chain bounty state into the database. Called by the indexer after
detecting a relevant Soroban event. Also fires the SSE broadcast (see
`/api/v1/bounties/stream`).

**Request**
```json
{
  "bountyId":       "<uuid>",
  "ledger":         "number",
  "contractEvents": ["<base64 XDR event>"]
}
```

**Response** `200 OK`
```json
{ "ok": true, "bountyId": "<uuid>" }
```

---

[Unreleased]: https://github.com/mergemint-mint/mergemint-contracts/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/mergemint-mint/mergemint-contracts/releases/tag/v0.1.0
