ALTER TABLE books ADD COLUMN content_kind TEXT NOT NULL DEFAULT 'text';

CREATE TABLE IF NOT EXISTS library_roots (
  id TEXT PRIMARY KEY NOT NULL,
  display_name TEXT NOT NULL,
  root_path TEXT NOT NULL UNIQUE,
  state TEXT NOT NULL DEFAULT 'unknown'
    CHECK (state IN ('unknown', 'available', 'missing', 'needs_relink')),
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  last_verified_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_library_roots_state
  ON library_roots(state, updated_at DESC);

CREATE TABLE IF NOT EXISTS image_sequences (
  book_id TEXT PRIMARY KEY NOT NULL,
  root_id TEXT NOT NULL,
  cache_key TEXT NOT NULL,
  direction TEXT NOT NULL DEFAULT 'ltr'
    CHECK (direction IN ('ltr', 'rtl', 'vertical')),
  spread TEXT NOT NULL DEFAULT 'single'
    CHECK (spread IN ('single', 'double', 'long_strip')),
  page_count INTEGER NOT NULL CHECK (page_count > 0),
  total_pixels INTEGER NOT NULL DEFAULT 0 CHECK (total_pixels >= 0),
  total_decoded_bytes INTEGER NOT NULL DEFAULT 0 CHECK (total_decoded_bytes >= 0),
  current_page INTEGER NOT NULL DEFAULT 0,
  zoom REAL NOT NULL DEFAULT 1.0 CHECK (zoom > 0.0 AND zoom <= 8.0),
  state TEXT NOT NULL DEFAULT 'ready'
    CHECK (state IN ('ready', 'missing', 'stale', 'needs_relink')),
  created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  FOREIGN KEY (book_id) REFERENCES books(id) ON DELETE CASCADE,
  FOREIGN KEY (root_id) REFERENCES library_roots(id) ON DELETE RESTRICT,
  CHECK (current_page >= 0 AND current_page < page_count),
  UNIQUE (root_id, cache_key)
);

CREATE INDEX IF NOT EXISTS idx_image_sequences_state_opened
  ON image_sequences(state, updated_at DESC);

CREATE TABLE IF NOT EXISTS image_sequence_pages (
  sequence_id TEXT NOT NULL,
  page_index INTEGER NOT NULL CHECK (page_index >= 0),
  relative_path TEXT NOT NULL CHECK (length(trim(relative_path)) > 0),
  file_size INTEGER NOT NULL CHECK (file_size >= 0),
  modified_at_ns INTEGER,
  content_digest TEXT,
  digest_version INTEGER NOT NULL DEFAULT 1 CHECK (digest_version > 0),
  mime TEXT NOT NULL
    CHECK (mime IN ('image/png', 'image/jpeg', 'image/gif', 'image/webp')),
  width INTEGER NOT NULL CHECK (width > 0),
  height INTEGER NOT NULL CHECK (height > 0),
  state TEXT NOT NULL DEFAULT 'ready'
    CHECK (state IN ('ready', 'missing', 'stale')),
  PRIMARY KEY (sequence_id, page_index),
  UNIQUE (sequence_id, relative_path),
  FOREIGN KEY (sequence_id) REFERENCES image_sequences(book_id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_image_sequence_pages_path
  ON image_sequence_pages(sequence_id, relative_path);
