-- Durable webhook delivery queue and dead-letter storage (#1033).
CREATE TABLE IF NOT EXISTS webhook_deliveries (
    id BIGSERIAL PRIMARY KEY,
    subscription_id TEXT NOT NULL,
    subscription_url TEXT NOT NULL,
    subscription_secret TEXT NOT NULL,
    payload JSONB NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending', 'delivering', 'delivered', 'dead_letter')),
    attempts INTEGER NOT NULL DEFAULT 0 CHECK (attempts >= 0),
    last_status INTEGER,
    last_error TEXT,
    next_attempt_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    delivered_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_webhook_deliveries_pending
    ON webhook_deliveries (next_attempt_at, id)
    WHERE status = 'pending';

CREATE INDEX IF NOT EXISTS idx_webhook_deliveries_failed
    ON webhook_deliveries (created_at DESC, id DESC)
    WHERE status = 'dead_letter';

CREATE TABLE IF NOT EXISTS webhook_delivery_attempts (
    id BIGSERIAL PRIMARY KEY,
    delivery_id BIGINT NOT NULL REFERENCES webhook_deliveries(id) ON DELETE CASCADE,
    attempt_number INTEGER NOT NULL CHECK (attempt_number > 0),
    status_code INTEGER,
    error TEXT,
    attempted_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_webhook_delivery_attempts_delivery
    ON webhook_delivery_attempts (delivery_id, attempt_number DESC);
