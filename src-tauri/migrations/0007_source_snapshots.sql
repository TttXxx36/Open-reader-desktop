CREATE TABLE IF NOT EXISTS source_snapshots (
    id TEXT PRIMARY KEY NOT NULL,
    label TEXT NOT NULL,
    payload_json TEXT NOT NULL,
    source_count INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_source_snapshots_created
    ON source_snapshots(created_at DESC);
