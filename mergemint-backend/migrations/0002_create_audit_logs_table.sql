-- Create audit_logs table to track admin actions
--
-- Stores audit trail for administrative actions like resolve_dispute.
-- Fields:
--   id: auto-incrementing primary key
--   actor: the address/account performing the action (e.g., arbitrator)
--   action: the type of action (e.g., 'resolve_dispute')
--   target: the bounty_id or other resource being acted upon
--   timestamp: unix timestamp (seconds) when the action occurred

CREATE TABLE IF NOT EXISTS audit_logs (
  id SERIAL PRIMARY KEY,
  actor VARCHAR(256) NOT NULL,
  action VARCHAR(100) NOT NULL,
  target VARCHAR(256) NOT NULL,
  timestamp BIGINT NOT NULL
);

-- Index on timestamp for efficient range queries and sorting
CREATE INDEX IF NOT EXISTS idx_audit_logs_timestamp ON audit_logs (timestamp DESC);

-- Index on actor for filtering audit logs by who performed the action
CREATE INDEX IF NOT EXISTS idx_audit_logs_actor ON audit_logs (actor);

-- Index on action for filtering by action type
CREATE INDEX IF NOT EXISTS idx_audit_logs_action ON audit_logs (action);
