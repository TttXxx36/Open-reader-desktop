CREATE TABLE IF NOT EXISTS book_reading_state (
  book_id TEXT PRIMARY KEY NOT NULL REFERENCES books(id) ON DELETE CASCADE,
  position REAL NOT NULL DEFAULT 0 CHECK(position >= 0),
  read_state TEXT NOT NULL DEFAULT 'unread'
    CHECK(read_state IN ('unread', 'reading', 'finished')),
  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_book_reading_state_updated_at
  ON book_reading_state(updated_at);
