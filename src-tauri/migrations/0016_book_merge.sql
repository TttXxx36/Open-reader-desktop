ALTER TABLE books ADD COLUMN lifecycle_state TEXT NOT NULL DEFAULT 'active'
  CHECK(lifecycle_state IN ('active', 'merged'));

CREATE INDEX IF NOT EXISTS idx_books_lifecycle_updated
  ON books(lifecycle_state, updated_at DESC);

CREATE TABLE IF NOT EXISTS book_merge_operations (
  id TEXT PRIMARY KEY NOT NULL,
  preview_id TEXT NOT NULL UNIQUE,
  canonical_book_id TEXT NOT NULL,
  status TEXT NOT NULL DEFAULT 'committed'
    CHECK(status IN ('committed', 'undone', 'expired')),
  plan_json TEXT NOT NULL CHECK(length(plan_json) <= 65536),
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  committed_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  undo_until TEXT NOT NULL,
  FOREIGN KEY (canonical_book_id) REFERENCES books(id) ON DELETE RESTRICT
);

CREATE INDEX IF NOT EXISTS idx_book_merge_operations_canonical
  ON book_merge_operations(canonical_book_id, committed_at DESC);

CREATE TABLE IF NOT EXISTS book_merge_items (
  operation_id TEXT NOT NULL,
  source_book_id TEXT NOT NULL,
  canonical_book_id TEXT NOT NULL,
  source_snapshot_json TEXT NOT NULL CHECK(length(source_snapshot_json) <= 262144),
  appended_chapter_ids_json TEXT NOT NULL DEFAULT '[]',
  PRIMARY KEY(operation_id, source_book_id),
  CHECK(source_book_id <> canonical_book_id),
  FOREIGN KEY(operation_id) REFERENCES book_merge_operations(id) ON DELETE RESTRICT,
  FOREIGN KEY(source_book_id) REFERENCES books(id) ON DELETE RESTRICT,
  FOREIGN KEY(canonical_book_id) REFERENCES books(id) ON DELETE RESTRICT
);

CREATE INDEX IF NOT EXISTS idx_book_merge_items_source
  ON book_merge_items(source_book_id);

CREATE TABLE IF NOT EXISTS book_aliases (
  alias_book_id TEXT PRIMARY KEY NOT NULL,
  canonical_book_id TEXT NOT NULL,
  operation_id TEXT NOT NULL,
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  CHECK(alias_book_id <> canonical_book_id),
  FOREIGN KEY(alias_book_id) REFERENCES books(id) ON DELETE RESTRICT,
  FOREIGN KEY(canonical_book_id) REFERENCES books(id) ON DELETE RESTRICT,
  FOREIGN KEY(operation_id) REFERENCES book_merge_operations(id) ON DELETE RESTRICT
);

CREATE INDEX IF NOT EXISTS idx_book_aliases_canonical
  ON book_aliases(canonical_book_id);

