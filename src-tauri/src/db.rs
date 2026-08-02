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
                metadata.group_name,
                metadata.source_type,
                metadata.weight,
                metadata.enabled_explore,
                metadata.custom_order,
                metadata.comment,
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
        (6_i64, include_str!("../migrations/0006_source_metadata.sql")),
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
        statement
            .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))?
            .collect::<Result<Vec<_>, _>>()?
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
             WHERE id = ?10",
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
