ALTER TABLE book_sources ADD COLUMN source_url TEXT;
ALTER TABLE book_sources ADD COLUMN group_name TEXT NOT NULL DEFAULT '';
ALTER TABLE book_sources ADD COLUMN source_type INTEGER NOT NULL DEFAULT 0;
ALTER TABLE book_sources ADD COLUMN weight INTEGER NOT NULL DEFAULT 0;
ALTER TABLE book_sources ADD COLUMN enabled_explore INTEGER NOT NULL DEFAULT 0 CHECK (enabled_explore IN (0, 1));
ALTER TABLE book_sources ADD COLUMN custom_order INTEGER NOT NULL DEFAULT 0;
ALTER TABLE book_sources ADD COLUMN comment TEXT NOT NULL DEFAULT '';
ALTER TABLE book_sources ADD COLUMN book_url_pattern TEXT;
ALTER TABLE book_sources ADD COLUMN explore_url TEXT;

CREATE INDEX IF NOT EXISTS idx_book_sources_group_order
  ON book_sources(group_name COLLATE NOCASE, custom_order, weight DESC, name COLLATE NOCASE);
