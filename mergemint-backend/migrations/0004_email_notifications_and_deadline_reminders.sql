CREATE TABLE IF NOT EXISTS email_notification_subscriptions (
    id UUID PRIMARY KEY,
    contributor_address TEXT NOT NULL,
    email TEXT NOT NULL,
    verification_token TEXT UNIQUE,
    unsubscribe_token TEXT NOT NULL UNIQUE,
    verified BOOLEAN NOT NULL DEFAULT FALSE,
    active BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_email_notification_subscriptions_active_address
    ON email_notification_subscriptions (contributor_address, email)
    WHERE active;

CREATE INDEX IF NOT EXISTS idx_email_notification_subscriptions_recipient
    ON email_notification_subscriptions (contributor_address, verified, active);

CREATE TABLE IF NOT EXISTS deadline_reminders (
    bounty_id TEXT PRIMARY KEY,
    sent_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

ALTER TABLE bounties
    ADD COLUMN IF NOT EXISTS submitted_at TIMESTAMPTZ;

CREATE INDEX IF NOT EXISTS idx_bounties_deadline_reminder
    ON bounties (deadline)
    WHERE status = 'claimed' AND submitted_at IS NULL;
