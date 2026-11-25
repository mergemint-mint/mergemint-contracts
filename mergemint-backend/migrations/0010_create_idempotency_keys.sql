-- Shared idempotency ledger for transaction-submitting endpoints (#1263).
--
-- The primary key is the claim primitive: INSERT ... ON CONFLICT (key) DO
-- NOTHING (issued by the application) lets exactly one replica win a key.
-- Every later transition is a conditional UPDATE keyed on both `status` and
-- `request_hash`, so a stale request can never complete or release another
-- request's reservation.
CREATE TABLE IF NOT EXISTS idempotency_keys (
    key TEXT PRIMARY KEY,
    request_hash TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('in_flight', 'completed')),
    response_body TEXT,
    expires_at TIMESTAMPTZ NOT NULL
);

-- Supports the periodic TTL sweep (DELETE WHERE expires_at <= NOW()).
CREATE INDEX IF NOT EXISTS idempotency_keys_expires_at_idx
    ON idempotency_keys (expires_at);
