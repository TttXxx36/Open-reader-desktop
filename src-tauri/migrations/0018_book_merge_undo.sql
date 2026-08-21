ALTER TABLE book_merge_operations ADD COLUMN undone_at TEXT;

CREATE INDEX IF NOT EXISTS idx_book_merge_operations_status
  ON book_merge_operations(status, undo_until);
