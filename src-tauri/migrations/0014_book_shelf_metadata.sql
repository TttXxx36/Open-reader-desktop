ALTER TABLE books ADD COLUMN shelf_group TEXT NOT NULL DEFAULT '';
ALTER TABLE books ADD COLUMN tags_json TEXT NOT NULL DEFAULT '[]';
ALTER TABLE books ADD COLUMN custom_order INTEGER NOT NULL DEFAULT 0;

CREATE INDEX IF NOT EXISTS idx_books_shelf_group_order
  ON books(shelf_group, custom_order, updated_at DESC);
