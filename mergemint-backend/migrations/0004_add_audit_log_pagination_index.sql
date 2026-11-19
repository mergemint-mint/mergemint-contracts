CREATE INDEX IF NOT EXISTS idx_audit_logs_action_timestamp_id
  ON audit_logs (action, timestamp DESC, id DESC);