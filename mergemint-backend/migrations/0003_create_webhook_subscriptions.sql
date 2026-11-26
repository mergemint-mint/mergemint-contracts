-- Create webhook_subscriptions table for webhook notification subscriptions (#881).
--
-- Integrators subscribe to receive HMAC-signed webhook notifications
-- on bounty changes. Active subscriptions are queried by the dispatcher
-- upon receiving bounty events.

CREATE TABLE IF NOT EXISTS webhook_subscriptions (
    id TEXT PRIMARY KEY,
    url TEXT NOT NULL,
    secret TEXT NOT NULL,
    event_types TEXT[] NOT NULL DEFAULT '{}',
    active BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_webhook_subscriptions_active ON webhook_subscriptions (active);
