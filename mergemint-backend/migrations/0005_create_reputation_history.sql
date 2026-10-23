CREATE TABLE IF NOT EXISTS reputation_history (
    event_id TEXT PRIMARY KEY,
    contributor_address TEXT NOT NULL,
    ledger BIGINT NOT NULL CHECK (ledger >= 0),
    timestamp TIMESTAMPTZ NOT NULL,
    old_value BIGINT NOT NULL CHECK (old_value >= 0),
    new_value BIGINT NOT NULL CHECK (new_value >= 0),
    reason TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_reputation_history_contributor_ledger
    ON reputation_history (contributor_address, ledger DESC, event_id DESC);