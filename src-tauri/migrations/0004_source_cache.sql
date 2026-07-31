CREATE TABLE IF NOT EXISTS source_cache (
  cache_key TEXT PRIMARY KEY NOT NULL,
  source_id TEXT NOT NULL,
  kind TEXT NOT NULL,
  payload TEXT NOT NULL,
  fetched_at INTEGER NOT NULL,
  expires_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_source_cache_expiry
  ON source_cache(expires_at);

CREATE INDEX IF NOT EXISTS idx_source_cache_source
  ON source_cache(source_id, kind);
