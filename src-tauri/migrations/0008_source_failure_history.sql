CREATE TABLE IF NOT EXISTS source_failure_history (
    id TEXT PRIMARY KEY NOT NULL,
    source_id TEXT NOT NULL,
    source_name TEXT NOT NULL,
    stage TEXT NOT NULL,
    reason_code TEXT NOT NULL,
    message TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_source_failure_history_source_created
    ON source_failure_history(source_id, created_at DESC, id DESC);

CREATE INDEX IF NOT EXISTS idx_source_failure_history_created
    ON source_failure_history(created_at DESC, id DESC);
