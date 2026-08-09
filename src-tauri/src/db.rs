use crate::{library::ParsedBook, source::BookSource};
use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::Serialize;
use std::{
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DbError {
    #[error("sqlite error: {0}")]
    Sql(#[from] rusqlite::Error),
    #[error("filesystem error: {0}")]
    Io(#[from] std::io::Error),
    #[error("database lock is unavailable")]
    Lock,
    #[error("book not found")]
    NotFound,
}

pub struct Database {
    connection: Mutex<Connection>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BookSummary {
    pub id: String,
    pub title: String,
    pub author: Option<String>,
    pub format: String,
    pub chapter_count: i64,
    pub current_chapter: i64,
    pub progress: f64,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChapterSummary {
    pub id: String,
    pub title: String,
    pub index: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct BookDetail {
    pub book: BookSummary,
    pub chapters: Vec<ChapterSummary>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SourceSummary {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub config_json: String,
    pub updated_at: String,
    pub source_url: Option<String>,
    pub group_name: String,
    pub source_type: i64,
    pub weight: i64,
    pub enabled_explore: bool,
    pub custom_order: i64,
    pub comment: String,
    pub book_url_pattern: Option<String>,
    pub explore_url: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SourceSnapshotSummary {
    pub id: String,
    pub label: String,
    pub source_count: i64,
    pub created_at: String,
}

#[derive(Debug, Clone)]
pub struct SourceWrite {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub config_json: String,
    pub source_url: Option<String>,
    pub group_name: String,
    pub source_type: i64,
    pub weight: i64,
    pub enabled_explore: bool,
    pub custom_order: i64,
    pub comment: String,
    pub book_url_pattern: Option<String>,
    pub explore_url: Option<String>,
}

impl SourceWrite {
    pub fn from_source(
        id: String,
        source: &BookSource,
        config_json: String,
        enabled: bool,
    ) -> Self {
        let metadata = SourceMetadata::from(source);
        Self {
            id,
            name: source.name.clone(),
            enabled,
            config_json,
            source_url: metadata.source_url,
            group_name: metadata.group_name,
            source_type: metadata.source_type,
            weight: metadata.weight,
            enabled_explore: metadata.enabled_explore,
            custom_order: metadata.custom_order,
            comment: metadata.comment,
            book_url_pattern: metadata.book_url_pattern,
            explore_url: metadata.explore_url,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct SourceMetadata {
    source_url: Option<String>,
    group_name: String,
    source_type: i64,
    weight: i64,
    enabled_explore: bool,
    custom_order: i64,
    comment: String,
    book_url_pattern: Option<String>,
    explore_url: Option<String>,
}

impl From<&BookSource> for SourceMetadata {
    fn from(source: &BookSource) -> Self {
        Self {
            source_url: source.source_url.clone(),
            group_name: source.group.clone().unwrap_or_default(),
            source_type: source.source_type,
            weight: source.weight,
            enabled_explore: source.enabled_explore,
            custom_order: source.custom_order,
            comment: source.comment.clone().unwrap_or_default(),
            book_url_pattern: source.book_url_pattern.clone(),
            explore_url: source.explore_url.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ChapterContent {
    pub id: String,
    pub title: String,
    pub content: String,
    pub content_format: String,
    pub index: i64,
    pub total: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct SourceCacheStats {
    pub entries: usize,
    pub bytes: usize,
    pub expired_entries: usize,
    pub oldest_fetched_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SourceFailureHistory {
    pub id: String,
    pub source_id: String,
    pub source_name: String,
    pub stage: String,
    pub reason_code: String,
    pub message: String,
    pub created_at: String,
}

impl Database {
    pub fn open(app_data_dir: &Path) -> Result<Self, DbError> {
        fs::create_dir_all(app_data_dir)?;
        let database_path: PathBuf = app_data_dir.join("open-reader.db");
        let mut connection = Connection::open(database_path)?;
        apply_migrations(&mut connection)?;
        backfill_source_metadata(&connection)?;

        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    pub fn list_books(&self) -> Result<Vec<BookSummary>, DbError> {
        let connection = self.connection.lock().map_err(|_| DbError::Lock)?;
        let mut statement = connection.prepare(
            "SELECT b.id, b.title, b.author, b.format, COUNT(c.id),                     b.current_chapter, b.progress, b.updated_at
             FROM books b
             LEFT JOIN chapters c ON c.book_id = b.id
             GROUP BY b.id
             ORDER BY b.updated_at DESC",
        )?;
        let rows = statement.query_map([], book_from_row)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
    }

    pub fn import_book(
        &self,
        source_name: &str,
        parsed: ParsedBook,
    ) -> Result<BookSummary, DbError> {
        let book_id = format!(
            "book-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        );
        let ParsedBook {
            title,
            author,
            format,
            chapters,
        } = parsed;

        let mut connection = self.connection.lock().map_err(|_| DbError::Lock)?;
        let transaction = connection.transaction()?;

        transaction.execute(
            "DELETE FROM chapters WHERE book_id IN (SELECT id FROM books WHERE path = ?1)",
            params![source_name],
        )?;
        transaction.execute("DELETE FROM books WHERE path = ?1", params![source_name])?;
        transaction.execute(
            "INSERT INTO books (id, title, author, path, format, current_chapter, progress)
             VALUES (?1, ?2, ?3, ?4, ?5, 0, 0)",
            params![book_id, title, author, source_name, format],
        )?;

        for (index, chapter) in chapters.into_iter().enumerate() {
            let chapter_id = format!("{book_id}-chapter-{index}");
            transaction.execute(
                "INSERT INTO chapters (id, book_id, chapter_index, title, content, content_format)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    chapter_id,
                    book_id,
                    index as i64,
                    chapter.title,
                    chapter.content,
                    chapter.content_format
                ],
            )?;
        }

        transaction.commit()?;
        drop(connection);
        self.get_book_summary(&book_id)
    }

    pub fn list_sources(&self) -> Result<Vec<SourceSummary>, DbError> {
        let connection = self.connection.lock().map_err(|_| DbError::Lock)?;
        let mut statement = connection.prepare(
            "SELECT id, name, enabled, config_json, updated_at,
                    source_url, group_name, source_type, weight,
                    enabled_explore, custom_order, comment, book_url_pattern, explore_url
             FROM book_sources
             ORDER BY group_name COLLATE NOCASE, custom_order, weight DESC, name COLLATE NOCASE",
        )?;
        let rows = statement.query_map([], source_from_row)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
    }

    pub fn save_source(
        &self,
        source_id: Option<&str>,
        name: &str,
        config_json: &str,
        metadata: &SourceMetadata,
    ) -> Result<SourceSummary, DbError> {
        let id = source_id
            .filter(|value| !value.trim().is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| generated_id("source"));
        let connection = self.connection.lock().map_err(|_| DbError::Lock)?;
        connection.execute(
            "INSERT INTO book_sources (
               id, name, config_json, source_url, group_name, source_type, weight,
               enabled_explore, custom_order, comment, book_url_pattern, explore_url
             )
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
             ON CONFLICT(id) DO UPDATE SET
               name = excluded.name,
               config_json = excluded.config_json,
               source_url = excluded.source_url,
               group_name = excluded.group_name,
               source_type = excluded.source_type,
               weight = excluded.weight,
               enabled_explore = excluded.enabled_explore,
               custom_order = excluded.custom_order,
               comment = excluded.comment,
               book_url_pattern = excluded.book_url_pattern,
               explore_url = excluded.explore_url,
               updated_at = CURRENT_TIMESTAMP",
            params![
                id,
                name,
                config_json,
                metadata.source_url.as_deref(),
                metadata.group_name.as_str(),
                metadata.source_type,
                metadata.weight,
                metadata.enabled_explore,
                metadata.custom_order,
                metadata.comment.as_str(),
                metadata.book_url_pattern.as_deref(),
                metadata.explore_url.as_deref()
            ],
        )?;
        connection
            .query_row(
                "SELECT id, name, enabled, config_json, updated_at,
                    source_url, group_name, source_type, weight,
                    enabled_explore, custom_order, comment, book_url_pattern, explore_url
                 FROM book_sources
                 WHERE id = ?1",
                params![id],
                source_from_row,
            )
            .map_err(DbError::from)
    }

    pub fn apply_sources_atomic(
        &self,
        writes: &[SourceWrite],
        replace_all: bool,
    ) -> Result<Vec<SourceSummary>, DbError> {
        let mut connection = self.connection.lock().map_err(|_| DbError::Lock)?;
        let transaction = connection.transaction()?;

        if replace_all {
            transaction.execute_batch("DELETE FROM source_cache; DELETE FROM book_sources;")?;
        }

        for write in writes {
            transaction.execute(
                "INSERT INTO book_sources (
                   id, name, enabled, config_json, source_url, group_name, source_type, weight,
                   enabled_explore, custom_order, comment, book_url_pattern, explore_url
                 )
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
                 ON CONFLICT(id) DO UPDATE SET
                   name = excluded.name,
                   enabled = excluded.enabled,
                   config_json = excluded.config_json,
                   source_url = excluded.source_url,
                   group_name = excluded.group_name,
                   source_type = excluded.source_type,
                   weight = excluded.weight,
                   enabled_explore = excluded.enabled_explore,
                   custom_order = excluded.custom_order,
                   comment = excluded.comment,
                   book_url_pattern = excluded.book_url_pattern,
                   explore_url = excluded.explore_url,
                   updated_at = CURRENT_TIMESTAMP",
                params![
                    write.id.as_str(),
                    write.name.as_str(),
                    write.enabled,
                    write.config_json.as_str(),
                    write.source_url.as_deref(),
                    write.group_name.as_str(),
                    write.source_type,
                    write.weight,
                    write.enabled_explore,
                    write.custom_order,
                    write.comment.as_str(),
                    write.book_url_pattern.as_deref(),
                    write.explore_url.as_deref(),
                ],
            )?;
        }

        transaction.commit()?;
        drop(connection);
        self.list_sources()
    }

    pub fn create_source_snapshot(
        &self,
        label: &str,
        payload_json: &str,
        source_count: i64,
    ) -> Result<SourceSnapshotSummary, DbError> {
        let id = generated_id("source-snapshot");
        let connection = self.connection.lock().map_err(|_| DbError::Lock)?;
        connection.execute(
            "INSERT INTO source_snapshots (id, label, payload_json, source_count)
             VALUES (?1, ?2, ?3, ?4)",
            params![id, label, payload_json, source_count],
        )?;
        connection
            .query_row(
                "SELECT id, label, source_count, created_at
                 FROM source_snapshots
                 WHERE id = ?1",
                params![id],
                |row| {
                    Ok(SourceSnapshotSummary {
                        id: row.get(0)?,
                        label: row.get(1)?,
                        source_count: row.get(2)?,
                        created_at: row.get(3)?,
                    })
                },
            )
            .map_err(DbError::from)
    }

    pub fn list_source_snapshots(&self) -> Result<Vec<SourceSnapshotSummary>, DbError> {
        let connection = self.connection.lock().map_err(|_| DbError::Lock)?;
        let mut statement = connection.prepare(
            "SELECT id, label, source_count, created_at
             FROM source_snapshots
             ORDER BY created_at DESC, id DESC
             LIMIT 20",
        )?;
        let rows = statement.query_map([], |row| {
            Ok(SourceSnapshotSummary {
                id: row.get(0)?,
                label: row.get(1)?,
                source_count: row.get(2)?,
                created_at: row.get(3)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
    }

    pub fn get_source_snapshot(&self, snapshot_id: &str) -> Result<String, DbError> {
        let connection = self.connection.lock().map_err(|_| DbError::Lock)?;
        connection
            .query_row(
                "SELECT payload_json FROM source_snapshots WHERE id = ?1",
                params![snapshot_id],
                |row| row.get(0),
            )
            .optional()?
            .ok_or(DbError::NotFound)
    }

    pub fn set_source_enabled(
        &self,
        source_id: &str,
        enabled: bool,
    ) -> Result<SourceSummary, DbError> {
        let connection = self.connection.lock().map_err(|_| DbError::Lock)?;
        let changed = connection.execute(
            "UPDATE book_sources
             SET enabled = ?1, updated_at = CURRENT_TIMESTAMP
             WHERE id = ?2",
            params![enabled, source_id],
        )?;
        if changed == 0 {
            return Err(DbError::NotFound);
        }
        connection
            .query_row(
                "SELECT id, name, enabled, config_json, updated_at,
                    source_url, group_name, source_type, weight,
                    enabled_explore, custom_order, comment, book_url_pattern, explore_url
                 FROM book_sources
                 WHERE id = ?1",
                params![source_id],
                source_from_row,
            )
            .map_err(DbError::from)
    }

    pub fn delete_source(&self, source_id: &str) -> Result<(), DbError> {
        let connection = self.connection.lock().map_err(|_| DbError::Lock)?;
        let changed =
            connection.execute("DELETE FROM book_sources WHERE id = ?1", params![source_id])?;
        if changed == 0 {
            return Err(DbError::NotFound);
        }
        Ok(())
    }

    pub fn get_source_cache(&self, cache_key: &str) -> Result<Option<String>, DbError> {
        let now = unix_timestamp();
        let connection = self.connection.lock().map_err(|_| DbError::Lock)?;
        let payload = connection
            .query_row(
                "SELECT payload
                 FROM source_cache
                 WHERE cache_key = ?1 AND expires_at > ?2",
                params![cache_key, now],
                |row| row.get(0),
            )
            .optional()?;

        if payload.is_none() {
            connection.execute(
                "DELETE FROM source_cache
                 WHERE cache_key = ?1 AND expires_at <= ?2",
                params![cache_key, now],
            )?;
        }

        Ok(payload)
    }

    pub fn get_source_cache_any(&self, cache_key: &str) -> Result<Option<String>, DbError> {
        let connection = self.connection.lock().map_err(|_| DbError::Lock)?;
        connection
            .query_row(
                "SELECT payload
                 FROM source_cache
                 WHERE cache_key = ?1",
                params![cache_key],
                |row| row.get(0),
            )
            .optional()
            .map_err(DbError::from)
    }

    pub fn save_source_cache(
        &self,
        cache_key: &str,
        source_id: &str,
        kind: &str,
        payload: &str,
        ttl_secs: u64,
    ) -> Result<(), DbError> {
        let fetched_at = unix_timestamp();
        let expires_at = fetched_at.saturating_add(ttl_secs.min(i64::MAX as u64) as i64);
        let connection = self.connection.lock().map_err(|_| DbError::Lock)?;
        connection.execute(
            "INSERT INTO source_cache (cache_key, source_id, kind, payload, fetched_at, expires_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(cache_key) DO UPDATE SET
               source_id = excluded.source_id,
               kind = excluded.kind,
               payload = excluded.payload,
               fetched_at = excluded.fetched_at,
               expires_at = excluded.expires_at",
            params![cache_key, source_id, kind, payload, fetched_at, expires_at],
        )?;
        Ok(())
    }

    pub fn clear_expired_source_cache(&self) -> Result<usize, DbError> {
        let connection = self.connection.lock().map_err(|_| DbError::Lock)?;
        let changed = connection.execute(
            "DELETE FROM source_cache WHERE expires_at <= ?1",
            params![unix_timestamp()],
        )?;
        Ok(changed)
    }

    pub fn prune_source_cache(
        &self,
        max_entries: usize,
        max_bytes: usize,
    ) -> Result<usize, DbError> {
        let max_entries = max_entries.max(1);
        let max_bytes = max_bytes.max(1);
        let connection = self.connection.lock().map_err(|_| DbError::Lock)?;
        let mut remove = Vec::new();
        let mut kept_entries = 0usize;
        let mut kept_bytes = 0usize;

        {
            let mut statement = connection.prepare(
                "SELECT cache_key, length(CAST(payload AS BLOB))
                 FROM source_cache
                 ORDER BY fetched_at DESC, cache_key DESC",
            )?;
            let rows = statement.query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
            })?;

            for row in rows {
                let (key, length) = row?;
                let bytes = usize::try_from(length.max(0)).unwrap_or(usize::MAX);
                if kept_entries < max_entries && kept_bytes.saturating_add(bytes) <= max_bytes {
                    kept_entries += 1;
                    kept_bytes = kept_bytes.saturating_add(bytes);
                } else {
                    remove.push(key);
                }
            }
        }

        for key in &remove {
            connection.execute(
                "DELETE FROM source_cache WHERE cache_key = ?1",
                params![key],
            )?;
        }

        Ok(remove.len())
    }

    pub fn source_cache_stats(&self) -> Result<SourceCacheStats, DbError> {
        let now = unix_timestamp();
        let connection = self.connection.lock().map_err(|_| DbError::Lock)?;
        let (entries, bytes, expired_entries, oldest_fetched_at): (i64, i64, i64, Option<i64>) =
            connection.query_row(
                "SELECT COUNT(*),
                        COALESCE(SUM(length(CAST(payload AS BLOB))), 0),
                        COALESCE(SUM(CASE WHEN expires_at <= ?1 THEN 1 ELSE 0 END), 0),
                        MIN(fetched_at)
                 FROM source_cache",
                params![now],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )?;

        Ok(SourceCacheStats {
            entries: usize::try_from(entries.max(0)).unwrap_or(usize::MAX),
            bytes: usize::try_from(bytes.max(0)).unwrap_or(usize::MAX),
            expired_entries: usize::try_from(expired_entries.max(0)).unwrap_or(usize::MAX),
            oldest_fetched_at,
        })
    }

    pub fn record_source_failure_history(
        &self,
        source_id: &str,
        source_name: &str,
        stage: &str,
        reason_code: &str,
        message: &str,
    ) -> Result<(), DbError> {
        let connection = self.connection.lock().map_err(|_| DbError::Lock)?;
        connection.execute(
            "INSERT INTO source_failure_history
                (id, source_id, source_name, stage, reason_code, message)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                generated_id("source-failure"),
                bounded_history_text(source_id, 256),
                bounded_history_text(source_name, 256),
                bounded_history_text(stage, 128),
                bounded_history_text(reason_code, 64),
                bounded_history_text(message, 512),
            ],
        )?;
        Ok(())
    }

    pub fn list_source_failure_history(
        &self,
        source_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<SourceFailureHistory>, DbError> {
        let limit = limit.clamp(1, 256) as i64;
        let source_id = source_id.filter(|value| !value.trim().is_empty());
        let connection = self.connection.lock().map_err(|_| DbError::Lock)?;
        let mut statement = connection.prepare(
            "SELECT id, source_id, source_name, stage, reason_code, message, created_at
             FROM source_failure_history
             WHERE (?1 IS NULL OR source_id = ?1)
             ORDER BY created_at DESC, id DESC
             LIMIT ?2",
        )?;
        let rows = statement.query_map(params![source_id, limit], source_failure_history_from_row)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
    }

    pub fn clear_source_failure_history(
        &self,
        source_id: Option<&str>,
    ) -> Result<usize, DbError> {
        let source_id = source_id.filter(|value| !value.trim().is_empty());
        let connection = self.connection.lock().map_err(|_| DbError::Lock)?;
        let changed = match source_id {
            Some(source_id) => connection.execute(
                "DELETE FROM source_failure_history WHERE source_id = ?1",
                params![source_id],
            )?,
            None => connection.execute("DELETE FROM source_failure_history", [])?,
        };
        Ok(changed)
    }

    pub fn get_book_detail(&self, book_id: &str) -> Result<BookDetail, DbError> {
        let connection = self.connection.lock().map_err(|_| DbError::Lock)?;
        let book = connection
            .query_row(
                "SELECT b.id, b.title, b.author, b.format, COUNT(c.id),                         b.current_chapter, b.progress, b.updated_at
                 FROM books b
                 LEFT JOIN chapters c ON c.book_id = b.id
                 WHERE b.id = ?1
                 GROUP BY b.id",
                params![book_id],
                book_from_row,
            )
            .optional()?
            .ok_or(DbError::NotFound)?;

        let mut statement = connection.prepare(
            "SELECT id, title, chapter_index
             FROM chapters
             WHERE book_id = ?1
             ORDER BY chapter_index",
        )?;
        let chapters = statement
            .query_map(params![book_id], |row| {
                Ok(ChapterSummary {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    index: row.get(2)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(BookDetail { book, chapters })
    }

    pub fn get_chapter_content(
        &self,
        book_id: &str,
        chapter_id: &str,
    ) -> Result<ChapterContent, DbError> {
        let connection = self.connection.lock().map_err(|_| DbError::Lock)?;
        let total: i64 = connection.query_row(
            "SELECT COUNT(*) FROM chapters WHERE book_id = ?1",
            params![book_id],
            |row| row.get(0),
        )?;
        connection
            .query_row(
                "SELECT id, title, content, content_format, chapter_index
                 FROM chapters
                 WHERE book_id = ?1 AND id = ?2",
                params![book_id, chapter_id],
                |row| {
                    Ok(ChapterContent {
                        id: row.get(0)?,
                        title: row.get(1)?,
                        content: row.get(2)?,
                        content_format: row.get(3)?,
                        index: row.get(4)?,
                        total,
                    })
                },
            )
            .optional()?
            .ok_or(DbError::NotFound)
    }

    pub fn save_progress(
        &self,
        book_id: &str,
        chapter_id: &str,
        current_chapter: i64,
        progress: f64,
    ) -> Result<(), DbError> {
        let connection = self.connection.lock().map_err(|_| DbError::Lock)?;
        let changed = connection.execute(
            "UPDATE books
             SET current_chapter = ?1, progress = ?2, updated_at = CURRENT_TIMESTAMP
             WHERE id = ?3
               AND EXISTS (SELECT 1 FROM chapters WHERE id = ?4 AND book_id = ?3)",
            params![
                current_chapter,
                progress.clamp(0.0, 1.0),
                book_id,
                chapter_id
            ],
        )?;
        if changed == 0 {
            return Err(DbError::NotFound);
        }
        Ok(())
    }

    fn get_book_summary(&self, book_id: &str) -> Result<BookSummary, DbError> {
        let connection = self.connection.lock().map_err(|_| DbError::Lock)?;
        connection
            .query_row(
                "SELECT b.id, b.title, b.author, b.format, COUNT(c.id),                         b.current_chapter, b.progress, b.updated_at
                 FROM books b
                 LEFT JOIN chapters c ON c.book_id = b.id
                 WHERE b.id = ?1
                 GROUP BY b.id",
                params![book_id],
                book_from_row,
            )
            .optional()?
            .ok_or(DbError::NotFound)
    }
}

fn apply_migrations(connection: &mut Connection) -> Result<(), DbError> {
    connection.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
           version INTEGER PRIMARY KEY NOT NULL,
           applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
         );",
    )?;

    for (version, sql) in [
        (1_i64, include_str!("../migrations/0001_init.sql")),
        (2_i64, include_str!("../migrations/0002_library.sql")),
        (3_i64, include_str!("../migrations/0003_sources.sql")),
        (4_i64, include_str!("../migrations/0004_source_cache.sql")),
        (5_i64, include_str!("../migrations/0005_content_format.sql")),
        (
            6_i64,
            include_str!("../migrations/0006_source_metadata.sql"),
        ),
        (
            7_i64,
            include_str!("../migrations/0007_source_snapshots.sql"),
        ),
        (
            8_i64,
            include_str!("../migrations/0008_source_failure_history.sql"),
        ),
    ] {
        let applied: Option<i64> = connection
            .query_row(
                "SELECT version FROM schema_migrations WHERE version = ?1",
                params![version],
                |row| row.get(0),
            )
            .optional()?;

        if applied.is_none() {
            let transaction = connection.transaction()?;
            transaction.execute_batch(sql)?;
            transaction.execute(
                "INSERT INTO schema_migrations (version) VALUES (?1)",
                params![version],
            )?;
            transaction.commit()?;
        }
    }

    Ok(())
}

fn book_from_row(row: &Row<'_>) -> rusqlite::Result<BookSummary> {
    Ok(BookSummary {
        id: row.get(0)?,
        title: row.get(1)?,
        author: row.get(2)?,
        format: row.get(3)?,
        chapter_count: row.get(4)?,
        current_chapter: row.get(5)?,
        progress: row.get(6)?,
        updated_at: row.get(7)?,
    })
}

fn source_from_row(row: &Row<'_>) -> rusqlite::Result<SourceSummary> {
    Ok(SourceSummary {
        id: row.get(0)?,
        name: row.get(1)?,
        enabled: row.get::<_, i64>(2)? != 0,
        config_json: row.get(3)?,
        updated_at: row.get(4)?,
        source_url: row.get(5)?,
        group_name: row.get(6)?,
        source_type: row.get(7)?,
        weight: row.get(8)?,
        enabled_explore: row.get::<_, i64>(9)? != 0,
        custom_order: row.get(10)?,
        comment: row.get(11)?,
        book_url_pattern: row.get(12)?,
        explore_url: row.get(13)?,
    })
}

fn backfill_source_metadata(connection: &Connection) -> Result<(), DbError> {
    let rows = {
        let mut statement = connection.prepare("SELECT id, config_json FROM book_sources")?;
        let rows = statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        rows
    };

    for (id, config_json) in rows {
        let Ok(source) = serde_json::from_str::<BookSource>(&config_json) else {
            continue;
        };
        let metadata = SourceMetadata::from(&source);
        connection.execute(
            "UPDATE book_sources
             SET source_url = ?1,
                 group_name = ?2,
                 source_type = ?3,
                 weight = ?4,
                 enabled_explore = ?5,
                 custom_order = ?6,
                 comment = ?7,
                 book_url_pattern = ?8,
                 explore_url = ?9
             WHERE id = ?10
               AND source_url IS NULL
               AND group_name = ''
               AND source_type = 0
               AND weight = 0
               AND enabled_explore = 0
               AND custom_order = 0
               AND comment = ''
               AND book_url_pattern IS NULL
               AND explore_url IS NULL",
            params![
                metadata.source_url.as_deref(),
                metadata.group_name,
                metadata.source_type,
                metadata.weight,
                metadata.enabled_explore,
                metadata.custom_order,
                metadata.comment,
                metadata.book_url_pattern.as_deref(),
                metadata.explore_url.as_deref(),
                id
            ],
        )?;
    }
    Ok(())
}

fn unix_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .min(i64::MAX as u64) as i64
}

fn generated_id(prefix: &str) -> String {
    format!(
        "{prefix}-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    )
}

fn bounded_history_text(value: &str, max_chars: usize) -> String {
    value.chars().take(max_chars).collect()
}

fn source_failure_history_from_row(row: &Row<'_>) -> rusqlite::Result<SourceFailureHistory> {
    Ok(SourceFailureHistory {
        id: row.get(0)?,
        source_id: row.get(1)?,
        source_name: row.get(2)?,
        stage: row.get(3)?,
        reason_code: row.get(4)?,
        message: row.get(5)?,
        created_at: row.get(6)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn persists_and_orders_source_metadata() {
        let directory = std::env::temp_dir().join(format!(
            "open-reader-source-metadata-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        let database = Database::open(&directory).expect("database should open");
        database
            .save_source(
                Some("source-beta"),
                "Beta",
                r#"{"name":"Beta"}"#,
                &SourceMetadata {
                    source_url: Some("https://beta.example.test/".to_string()),
                    group_name: "Beta".to_string(),
                    source_type: 0,
                    weight: 10,
                    enabled_explore: true,
                    custom_order: 2,
                    comment: "beta".to_string(),
                    book_url_pattern: None,
                    explore_url: Some("https://beta.example.test/explore".to_string()),
                },
            )
            .expect("beta source should save");
        database
            .save_source(
                Some("source-alpha"),
                "Alpha",
                r#"{"name":"Alpha"}"#,
                &SourceMetadata {
                    source_url: Some("https://alpha.example.test/".to_string()),
                    group_name: "Alpha".to_string(),
                    source_type: 0,
                    weight: 20,
                    enabled_explore: false,
                    custom_order: 1,
                    comment: "alpha".to_string(),
                    book_url_pattern: None,
                    explore_url: None,
                },
            )
            .expect("alpha source should save");

        let listed = database.list_sources().expect("sources should list");
        assert_eq!(
            listed
                .iter()
                .map(|source| source.id.as_str())
                .collect::<Vec<_>>(),
            vec!["source-alpha", "source-beta"]
        );
        assert_eq!(listed[0].group_name, "Alpha");
        assert_eq!(listed[0].weight, 20);
        assert!(listed[1].enabled_explore);
        assert_eq!(
            listed[1].explore_url.as_deref(),
            Some("https://beta.example.test/explore")
        );

        drop(database);
        let reopened = Database::open(&directory).expect("database should reopen");
        let persisted = reopened
            .list_sources()
            .expect("metadata should persist after reopen");
        assert_eq!(persisted[0].group_name, "Alpha");
        assert_eq!(persisted[1].comment, "beta");
        drop(reopened);
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn snapshots_and_replaces_sources_atomically() {
        let directory = std::env::temp_dir().join(format!(
            "open-reader-source-snapshot-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        let database = Database::open(&directory).expect("database should open");
        let first = SourceWrite {
            id: "source-first".to_string(),
            name: "First".to_string(),
            enabled: true,
            config_json: r#"{"name":"First"}"#.to_string(),
            source_url: Some("https://first.example.test/".to_string()),
            group_name: "Test".to_string(),
            source_type: 0,
            weight: 1,
            enabled_explore: false,
            custom_order: 0,
            comment: "before".to_string(),
            book_url_pattern: None,
            explore_url: None,
        };
        database
            .apply_sources_atomic(&[first], false)
            .expect("initial source should save");

        let snapshot = database
            .create_source_snapshot("test snapshot", r#"{"version":1,"sources":[]}"#, 1)
            .expect("snapshot should save");
        assert_eq!(
            database
                .list_source_snapshots()
                .expect("snapshots should list")
                .first()
                .map(|item| item.id.as_str()),
            Some(snapshot.id.as_str())
        );
        assert_eq!(
            database
                .get_source_snapshot(&snapshot.id)
                .expect("snapshot payload should read"),
            r#"{"version":1,"sources":[]}"#
        );

        let second = SourceWrite {
            id: "source-second".to_string(),
            name: "Second".to_string(),
            enabled: false,
            config_json: r#"{"name":"Second"}"#.to_string(),
            source_url: None,
            group_name: "Restored".to_string(),
            source_type: 0,
            weight: 0,
            enabled_explore: false,
            custom_order: 0,
            comment: String::new(),
            book_url_pattern: None,
            explore_url: None,
        };
        database
            .apply_sources_atomic(&[second], true)
            .expect("replacement should be atomic");
        let listed = database.list_sources().expect("sources should list");
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, "source-second");
        assert_eq!(listed[0].group_name, "Restored");

        drop(database);
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn persists_source_configuration_and_enabled_state() {
        let directory = std::env::temp_dir().join(format!(
            "open-reader-db-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        let database = Database::open(&directory).expect("database should open");
        let saved = database
            .save_source(
                None,
                "Fixture",
                r#"{"name":"Fixture"}"#,
                &SourceMetadata::default(),
            )
            .expect("source should save");
        assert!(saved.enabled);
        assert_eq!(
            database.list_sources().expect("sources should list").len(),
            1
        );
        database
            .save_source_cache("cache-key", &saved.id, "book", r#"{"title":"Fixture"}"#, 60)
            .expect("cache should save");
        let stats = database
            .source_cache_stats()
            .expect("cache stats should read");
        assert_eq!(stats.entries, 1);
        assert!(stats.bytes > 0);
        assert_eq!(stats.expired_entries, 0);
        assert_eq!(
            database
                .get_source_cache("cache-key")
                .expect("cache should read"),
            Some(r#"{"title":"Fixture"}"#.to_string())
        );
        {
            let connection = database.connection.lock().expect("database lock");
            connection
                .execute("UPDATE source_cache SET expires_at = 0", [])
                .expect("cache should expire");
        }
        assert_eq!(
            database
                .get_source_cache_any("cache-key")
                .expect("stale cache should read"),
            Some(r#"{"title":"Fixture"}"#.to_string())
        );
        assert_eq!(
            database
                .clear_expired_source_cache()
                .expect("cache cleanup should work"),
            1
        );
        assert_eq!(
            database
                .get_source_cache("cache-key")
                .expect("expired cache should miss"),
            None
        );

        for (key, payload) in [("a", "1111"), ("b", "2222"), ("c", "3333")] {
            database
                .save_source_cache(key, &saved.id, "book", payload, 60)
                .expect("cache should save");
        }
        assert_eq!(
            database
                .prune_source_cache(2, 1024)
                .expect("entry pruning should work"),
            1
        );
        {
            let connection = database.connection.lock().expect("database lock");
            let remaining: i64 = connection
                .query_row(
                    "SELECT COUNT(*) FROM source_cache WHERE cache_key != 'cache-key'",
                    [],
                    |row| row.get(0),
                )
                .expect("remaining cache count");
            assert_eq!(remaining, 2);
        }

        database
            .save_source_cache("bytes-a", &saved.id, "book", "1234", 60)
            .expect("cache should save");
        database
            .save_source_cache("bytes-b", &saved.id, "book", "章节", 60)
            .expect("cache should save");
        assert_eq!(
            database
                .prune_source_cache(100, 5)
                .expect("byte pruning should work"),
            3
        );
        {
            let connection = database.connection.lock().expect("database lock");
            let bytes: i64 = connection
                .query_row(
                    "SELECT COALESCE(SUM(length(CAST(payload AS BLOB))), 0) FROM source_cache",
                    [],
                    |row| row.get(0),
                )
                .expect("remaining cache bytes");
            assert!(bytes <= 5);
        }

        let disabled = database
            .set_source_enabled(&saved.id, false)
            .expect("source should update");
        assert!(!disabled.enabled);

        database
            .delete_source(&saved.id)
            .expect("source should delete");
        assert!(database
            .list_sources()
            .expect("sources should list")
            .is_empty());

        drop(database);
        let _ = fs::remove_dir_all(directory);
    }
    #[test]
    fn persists_and_clears_source_failure_history() {
        let directory = std::env::temp_dir().join(format!(
            "open-reader-source-failure-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        let database = Database::open(&directory).expect("database should open");
        database
            .record_source_failure_history(
                "source-a",
                "Alpha",
                "search",
                "request",
                "request failed",
            )
            .expect("failure should persist");
        database
            .record_source_failure_history(
                "source-b",
                "Beta",
                "search",
                "timeout",
                &"x".repeat(600),
            )
            .expect("second failure should persist");

        let all = database
            .list_source_failure_history(None, 10)
            .expect("history should list");
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].source_id, "source-b");
        assert_eq!(all[0].message.chars().count(), 512);
        assert_eq!(
            database
                .list_source_failure_history(Some("source-a"), 10)
                .expect("filtered history should list")
                .len(),
            1
        );
        assert_eq!(
            database
                .clear_source_failure_history(Some("source-a"))
                .expect("source history should clear"),
            1
        );
        assert_eq!(
            database
                .clear_source_failure_history(None)
                .expect("all history should clear"),
            1
        );
        assert!(database
            .list_source_failure_history(None, 10)
            .expect("history should be empty")
            .is_empty());

        drop(database);
        let _ = fs::remove_dir_all(directory);
    }

}
