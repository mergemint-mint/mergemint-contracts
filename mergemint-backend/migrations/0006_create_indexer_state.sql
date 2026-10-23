CREATE TABLE IF NOT EXISTS indexer_checkpoints (
    name TEXT PRIMARY KEY,
    ledger BIGINT NOT NULL CHECK (ledger >= 0),
    cursor TEXT,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS indexed_contract_events (
    event_id TEXT PRIMARY KEY,
    ledger BIGINT NOT NULL CHECK (ledger >= 0),
    event_type TEXT NOT NULL,
    contract_id TEXT NOT NULL,
    transaction_hash TEXT NOT NULL,
    ledger_closed_at TIMESTAMPTZ NOT NULL,
    topics JSONB NOT NULL,
    value JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_indexed_contract_events_ledger
    ON indexed_contract_events (ledger, event_id);

CREATE TABLE IF NOT EXISTS indexer_gaps (
    checkpoint_name TEXT NOT NULL,
    missing_from BIGINT NOT NULL CHECK (missing_from >= 0),
    missing_to BIGINT NOT NULL CHECK (missing_to >= missing_from),
    detected_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (checkpoint_name, missing_from, missing_to)
);