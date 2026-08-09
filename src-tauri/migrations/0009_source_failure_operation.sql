ALTER TABLE source_failure_history ADD COLUMN operation_id TEXT;

CREATE INDEX IF NOT EXISTS idx_source_failure_history_operation
    ON source_failure_history(operation_id, created_at DESC, id DESC);
