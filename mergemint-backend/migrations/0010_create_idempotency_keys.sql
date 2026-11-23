-- Shared idempotency-key ledger for multi-replica transaction submission (#1263).
--
-- A client-supplied Idempotency-Key is unique across every backend instance,
-- so a retry routed to a different replica replays the original response
-- instead of submitting the chain transaction a second time.
--
-- request_hash is a SHA-256 digest of the canonical JSON request payload. A
-- key reused with a different payload is rejected rather than replaying an
-- unrelated response.
--
-- expires_at backs two TTLs: short-lived `in_flight` reservations (a crashed
-- worker cannot block a key forever) and longer-lived `completed` responses
-- (cached bodies do not accumulate without bound).

CREATE TABLE IF NOT EXISTS idempotency_keys (
    key TEXT PRIMARY KEY,
    request_hash TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('in_flight', 'completed')),
    -- Ownership proof for an in_flight reservation. A slow request whose
    -- reservation expired cannot complete or release a newer claim.
    reservation_token TEXT NOT NULL,
    response_body TEXT,
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CONSTRAINT idempotency_keys_status_body CHECK (
        (status = 'in_flight' AND response_body IS NULL)
        OR (status = 'completed' AND response_body IS NOT NULL)
    )
);

CREATE INDEX IF NOT EXISTS idx_idempotency_keys_expires_at
    ON idempotency_keys (expires_at);
