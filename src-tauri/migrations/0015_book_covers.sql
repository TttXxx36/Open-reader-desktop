CREATE TABLE IF NOT EXISTS book_covers (
  book_id TEXT PRIMARY KEY NOT NULL,
  source_kind TEXT NOT NULL DEFAULT 'none'
    CHECK (source_kind IN ('none', 'local_path', 'remote_url')),
  source_value TEXT NOT NULL DEFAULT '',
  source_fingerprint TEXT NOT NULL DEFAULT '',
  cache_key TEXT NOT NULL,
  state TEXT NOT NULL DEFAULT 'missing'
    CHECK (state IN ('missing', 'stale', 'ready', 'blocked')),
  mime TEXT,
  width INTEGER,
  height INTEGER,
  byte_size INTEGER NOT NULL DEFAULT 0 CHECK (byte_size >= 0),
  fetched_at TEXT,
  last_error TEXT,
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  FOREIGN KEY (book_id) REFERENCES books(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_book_covers_state_updated
  ON book_covers(state, updated_at DESC);
