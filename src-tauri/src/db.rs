use crate::{
    image_relink::{
        preview_relink, sha256_file, ImageRelinkAssignment, ImageRelinkPreview, RelinkPage,
        MAX_DIGEST_FILE_BYTES, MAX_DIGEST_TOTAL_BYTES,
    },
    image_sequence::{
        modified_at_ns, normalize_relative_image_path, resolve_image_page_path,
        validate_image_root_path,
    },
    library::ParsedBook,
    source::BookSource,
};
use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
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
    #[error("invalid image sequence: {0}")]
    InvalidImageSequence(String),
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
    pub content_kind: String,
    pub chapter_count: i64,
    pub current_chapter: i64,
    pub progress: f64,
    pub updated_at: String,
    pub image_sequence_state: Option<String>,
    pub image_sequence_missing_pages: i64,
    pub image_sequence_stale_pages: i64,
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
pub struct ImageSequencePageSummary {
    pub sequence_id: String,
    pub page_index: i64,
    pub relative_path: String,
    pub file_size: i64,
    pub modified_at_ns: Option<i64>,
    pub content_digest: Option<String>,
    pub digest_version: i64,
    pub mime: String,
    pub width: i64,
    pub height: i64,
    pub state: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImageSequenceSummary {
    pub book_id: String,
    pub title: String,
    pub author: Option<String>,
    pub root_id: String,
    pub root_path: String,
    pub cache_key: String,
    pub direction: String,
    pub spread: String,
    pub page_count: i64,
    pub total_pixels: i64,
    pub total_decoded_bytes: i64,
    pub current_page: i64,
    pub zoom: f64,
    pub state: String,
    pub progress: f64,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImageSequenceDetail {
    pub sequence: ImageSequenceSummary,
    pub pages: Vec<ImageSequencePageSummary>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ImageSequencePageWrite {
    pub page_index: i64,
    pub relative_path: String,
    pub file_size: i64,
    pub modified_at_ns: Option<i64>,
    pub content_digest: Option<String>,
    #[serde(default = "default_digest_version")]
    pub digest_version: i64,
    pub mime: String,
    pub width: i64,
    pub height: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ImageSequenceWrite {
    pub book_id: Option<String>,
    pub title: String,
    pub author: Option<String>,
    pub root_path: String,
    pub cache_key: String,
    pub direction: String,
    pub spread: String,
    pub page_count: i64,
    pub total_pixels: i64,
    pub total_decoded_bytes: i64,
    pub current_page: i64,
    pub zoom: f64,
    pub pages: Vec<ImageSequencePageWrite>,
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
    pub operation_id: Option<String>,
    pub message: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SourceFailureCount {
    pub code: String,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct SourceFailureStats {
    pub total: usize,
    pub by_reason: Vec<SourceFailureCount>,
    pub by_stage: Vec<SourceFailureCount>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SourceRequestMetric {
    pub stage: String,
    pub attempts: usize,
    pub successes: usize,
    pub failures: usize,
    pub cache_hits: usize,
    pub failure_rate: f64,
    pub cache_hit_rate: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct SourceRequestMetrics {
    pub total_attempts: usize,
    pub total_successes: usize,
    pub total_failures: usize,
    pub total_cache_hits: usize,
    pub failure_rate: f64,
    pub cache_hit_rate: f64,
    pub by_stage: Vec<SourceRequestMetric>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceRuleOutcome {
    Success,
    NoMatch,
    Failure,
    Skipped,
}

#[derive(Debug, Clone, Serialize)]
pub struct SourceRuleMetric {
    pub stage: String,
    pub rule_key: String,
    pub attempts: usize,
    pub successes: usize,
    pub no_matches: usize,
    pub failures: usize,
    pub skipped: usize,
    pub success_rate: f64,
    pub failure_rate: f64,
    pub observed: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct SourceRuleMetrics {
    pub total_attempts: usize,
    pub total_successes: usize,
    pub total_no_matches: usize,
    pub total_failures: usize,
    pub total_skipped: usize,
    pub success_rate: f64,
    pub failure_rate: f64,
    pub observed: bool,
    pub by_rule: Vec<SourceRuleMetric>,
}

impl Database {
    pub fn open(app_data_dir: &Path) -> Result<Self, DbError> {
        fs::create_dir_all(app_data_dir)?;
        let database_path: PathBuf = app_data_dir.join("open-reader.db");
        let mut connection = Connection::open(database_path)?;
        connection.execute_batch("PRAGMA foreign_keys = ON;")?;
        apply_migrations(&mut connection)?;
        backfill_source_metadata(&connection)?;

        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    pub fn list_books(&self) -> Result<Vec<BookSummary>, DbError> {
        let connection = self.connection.lock().map_err(|_| DbError::Lock)?;
        let mut statement = connection.prepare(
            "SELECT b.id, b.title, b.author, b.format, b.content_kind, COUNT(c.id),                     b.current_chapter, b.progress, b.updated_at,
                    (SELECT s.state FROM image_sequences s WHERE s.book_id = b.id),
                    COALESCE((SELECT COUNT(*) FROM image_sequence_pages p
                              WHERE p.sequence_id = b.id AND p.state = 'missing'), 0),
                    COALESCE((SELECT COUNT(*) FROM image_sequence_pages p
                              WHERE p.sequence_id = b.id AND p.state = 'stale'), 0)
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
            "INSERT INTO books (id, title, author, path, format, content_kind, current_chapter, progress)
             VALUES (?1, ?2, ?3, ?4, ?5, 'text', 0, 0)",
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

    pub fn save_image_sequence(
        &self,
        write: ImageSequenceWrite,
    ) -> Result<ImageSequenceSummary, DbError> {
        let pages = validate_image_sequence_write(&write)?;
        let root_path = validate_image_root_path(&write.root_path)
            .map_err(|error| DbError::InvalidImageSequence(error.to_string()))?;
        let book_id = write
            .book_id
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| generated_id("image-book"));
        let title = write.title.trim();
        let cache_key = write.cache_key.trim();
        let root_display_name = root_path
            .rsplit(['/', '\\'])
            .find(|value| !value.is_empty())
            .unwrap_or(root_path.as_str());
        let progress = if write.page_count <= 1 {
            0.0
        } else {
            (write.current_page as f64 / (write.page_count - 1) as f64).clamp(0.0, 1.0)
        };

        let mut connection = self.connection.lock().map_err(|_| DbError::Lock)?;
        let transaction = connection.transaction()?;

        let existing_kind: Option<String> = transaction
            .query_row(
                "SELECT content_kind FROM books WHERE id = ?1",
                params![book_id.as_str()],
                |row| row.get(0),
            )
            .optional()?;
        if let Some(kind) = existing_kind {
            if kind != "image_sequence" {
                return Err(DbError::InvalidImageSequence(
                    "不能把文本书籍改写为图片序列".to_string(),
                ));
            }
        }

        let root_id: Option<String> = transaction
            .query_row(
                "SELECT id FROM library_roots WHERE root_path = ?1",
                params![root_path.as_str()],
                |row| row.get(0),
            )
            .optional()?;
        let root_id = root_id.unwrap_or_else(|| generated_id("library-root"));
        transaction.execute(
            "INSERT INTO library_roots
                (id, display_name, root_path, state, last_verified_at)
             VALUES (?1, ?2, ?3, 'available', CURRENT_TIMESTAMP)
             ON CONFLICT(root_path) DO UPDATE SET
                display_name = excluded.display_name,
                state = 'available',
                updated_at = CURRENT_TIMESTAMP,
                last_verified_at = CURRENT_TIMESTAMP",
            params![root_id.as_str(), root_display_name, root_path.as_str()],
        )?;
        let root_id: String = transaction.query_row(
            "SELECT id FROM library_roots WHERE root_path = ?1",
            params![root_path.as_str()],
            |row| row.get(0),
        )?;

        transaction.execute(
            "INSERT INTO books
                (id, title, author, path, format, content_kind, current_chapter, progress)
             VALUES (?1, ?2, ?3, ?4, 'image-sequence', 'image_sequence', ?5, ?6)
             ON CONFLICT(id) DO UPDATE SET
                title = excluded.title,
                author = excluded.author,
                path = excluded.path,
                format = excluded.format,
                content_kind = excluded.content_kind,
                current_chapter = excluded.current_chapter,
                progress = excluded.progress,
                updated_at = CURRENT_TIMESTAMP",
            params![
                book_id.as_str(),
                title,
                write.author.as_deref(),
                root_path.as_str(),
                write.current_page,
                progress,
            ],
        )?;

        transaction.execute(
            "INSERT INTO image_sequences
                (book_id, root_id, cache_key, direction, spread, page_count,
                 total_pixels, total_decoded_bytes, current_page, zoom, state)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 'ready')
             ON CONFLICT(book_id) DO UPDATE SET
                root_id = excluded.root_id,
                cache_key = excluded.cache_key,
                direction = excluded.direction,
                spread = excluded.spread,
                page_count = excluded.page_count,
                total_pixels = excluded.total_pixels,
                total_decoded_bytes = excluded.total_decoded_bytes,
                current_page = excluded.current_page,
                zoom = excluded.zoom,
                state = 'ready',
                updated_at = CURRENT_TIMESTAMP",
            params![
                book_id.as_str(),
                root_id.as_str(),
                cache_key,
                write.direction.as_str(),
                write.spread.as_str(),
                write.page_count,
                write.total_pixels,
                write.total_decoded_bytes,
                write.current_page,
                write.zoom,
            ],
        )?;

        transaction.execute(
            "DELETE FROM image_sequence_pages WHERE sequence_id = ?1",
            params![book_id.as_str()],
        )?;
        for page in pages {
            transaction.execute(
                "INSERT INTO image_sequence_pages
                    (sequence_id, page_index, relative_path, file_size, modified_at_ns,
                     content_digest, digest_version, mime, width, height, state)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 'ready')",
                params![
                    book_id.as_str(),
                    page.page_index,
                    page.relative_path,
                    page.file_size,
                    page.modified_at_ns,
                    page.content_digest,
                    page.digest_version,
                    page.mime,
                    page.width,
                    page.height,
                ],
            )?;
        }

        transaction.commit()?;
        drop(connection);
        self.get_image_sequence(&book_id)
            .map(|detail| detail.sequence)
    }

    pub fn list_image_sequences(&self) -> Result<Vec<ImageSequenceSummary>, DbError> {
        let connection = self.connection.lock().map_err(|_| DbError::Lock)?;
        let mut statement = connection.prepare(
            "SELECT s.book_id, b.title, b.author, s.root_id, r.root_path,
                    s.cache_key, s.direction, s.spread, s.page_count,
                    s.total_pixels, s.total_decoded_bytes, s.current_page,
                    s.zoom, s.state, b.progress, s.updated_at
             FROM image_sequences s
             JOIN books b ON b.id = s.book_id
             JOIN library_roots r ON r.id = s.root_id
             ORDER BY s.updated_at DESC, s.book_id DESC",
        )?;
        let rows = statement.query_map([], image_sequence_from_row)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
    }

    pub fn get_image_sequence(&self, book_id: &str) -> Result<ImageSequenceDetail, DbError> {
        let connection = self.connection.lock().map_err(|_| DbError::Lock)?;
        let sequence = connection
            .query_row(
                "SELECT s.book_id, b.title, b.author, s.root_id, r.root_path,
                        s.cache_key, s.direction, s.spread, s.page_count,
                        s.total_pixels, s.total_decoded_bytes, s.current_page,
                        s.zoom, s.state, b.progress, s.updated_at
                 FROM image_sequences s
                 JOIN books b ON b.id = s.book_id
                 JOIN library_roots r ON r.id = s.root_id
                 WHERE s.book_id = ?1",
                params![book_id],
                image_sequence_from_row,
            )
            .optional()?
            .ok_or(DbError::NotFound)?;

        let mut statement = connection.prepare(
            "SELECT sequence_id, page_index, relative_path, file_size, modified_at_ns,
                    content_digest, digest_version, mime, width, height, state
             FROM image_sequence_pages
             WHERE sequence_id = ?1
             ORDER BY page_index",
        )?;
        let pages = statement
            .query_map(params![book_id], image_sequence_page_from_row)?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(ImageSequenceDetail { sequence, pages })
    }

    pub fn preview_image_sequence_relink(
        &self,
        book_id: &str,
        new_root_path: &str,
    ) -> Result<ImageRelinkPreview, DbError> {
        let detail = self.get_image_sequence(book_id)?;
        let pages = detail
            .pages
            .iter()
            .map(|page| RelinkPage {
                page_index: page.page_index,
                relative_path: page.relative_path.clone(),
                file_size: page.file_size,
            })
            .collect::<Vec<_>>();
        preview_relink(book_id, &detail.sequence.root_path, new_root_path, &pages)
            .map_err(DbError::InvalidImageSequence)
    }

    pub fn apply_image_sequence_relink(
        &self,
        book_id: &str,
        new_root_path: &str,
        assignments: Vec<ImageRelinkAssignment>,
    ) -> Result<ImageSequenceDetail, DbError> {
        let detail = self.get_image_sequence(book_id)?;
        let new_root_path = validate_image_root_path(new_root_path)
            .map_err(|error| DbError::InvalidImageSequence(error.to_string()))?;
        if !fs::metadata(&new_root_path)
            .map(|metadata| metadata.is_dir())
            .unwrap_or(false)
        {
            return Err(DbError::InvalidImageSequence(
                "新图片根目录不存在或不是目录".to_string(),
            ));
        }

        let pages_by_index = detail
            .pages
            .iter()
            .map(|page| (page.page_index, page))
            .collect::<HashMap<_, _>>();
        let mut updates = HashMap::new();
        for assignment in assignments {
            let page = pages_by_index
                .get(&assignment.page_index)
                .ok_or_else(|| DbError::InvalidImageSequence("重新关联包含未知页码".to_string()))?;
            if page.relative_path != assignment.old_relative_path {
                return Err(DbError::InvalidImageSequence(
                    "重新关联预览已过期，请重新扫描".to_string(),
                ));
            }
            let Some(new_relative_path) = assignment.new_relative_path.clone() else {
                continue;
            };
            let new_relative_path = normalize_relative_image_path(&new_relative_path)
                .map_err(|error| DbError::InvalidImageSequence(error.to_string()))?;
            let new_path = resolve_image_page_path(&new_root_path, &new_relative_path)
                .map_err(|error| DbError::InvalidImageSequence(error.to_string()))?;
            let metadata = fs::metadata(new_path)
                .ok()
                .filter(|metadata| metadata.is_file())
                .ok_or_else(|| {
                    DbError::InvalidImageSequence(
                        "重新关联期间有图片已消失，请重新扫描".to_string(),
                    )
                })?;
            let observed_size = i64::try_from(metadata.len()).unwrap_or(i64::MAX);
            if observed_size != assignment.file_size {
                return Err(DbError::InvalidImageSequence(
                    "重新关联期间图片大小发生变化，请重新扫描".to_string(),
                ));
            }
            let observed_modified_at_ns = modified_at_ns(&metadata);
            let state = if assignment.match_kind == "relative"
                && observed_size == page.file_size
                && page.modified_at_ns == observed_modified_at_ns
            {
                "ready"
            } else {
                "stale"
            };
            if updates
                .insert(
                    assignment.page_index,
                    (
                        new_relative_path,
                        observed_size,
                        observed_modified_at_ns,
                        state,
                    ),
                )
                .is_some()
            {
                return Err(DbError::InvalidImageSequence(
                    "重新关联包含重复页码".to_string(),
                ));
            }
        }
        if updates.is_empty() {
            return Err(DbError::InvalidImageSequence(
                "没有可重新关联的图片".to_string(),
            ));
        }

        let root_display_name = new_root_path
            .rsplit(['/', '\\'])
            .find(|value| !value.is_empty())
            .unwrap_or(new_root_path.as_str());
        let mut connection = self.connection.lock().map_err(|_| DbError::Lock)?;
        let transaction = connection.transaction()?;
        let root_id: Option<String> = transaction
            .query_row(
                "SELECT id FROM library_roots WHERE root_path = ?1",
                params![new_root_path.as_str()],
                |row| row.get(0),
            )
            .optional()?;
        let root_id = root_id.unwrap_or_else(|| generated_id("library-root"));
        transaction.execute(
            "INSERT INTO library_roots
                (id, display_name, root_path, state, last_verified_at)
             VALUES (?1, ?2, ?3, 'available', CURRENT_TIMESTAMP)
             ON CONFLICT(root_path) DO UPDATE SET
                display_name = excluded.display_name,
                state = 'available',
                updated_at = CURRENT_TIMESTAMP,
                last_verified_at = CURRENT_TIMESTAMP",
            params![root_id.as_str(), root_display_name, new_root_path.as_str()],
        )?;
        let root_id: String = transaction.query_row(
            "SELECT id FROM library_roots WHERE root_path = ?1",
            params![new_root_path.as_str()],
            |row| row.get(0),
        )?;

        let mut missing_pages = 0_i64;
        let mut stale_pages = 0_i64;
        for page in &detail.pages {
            if let Some((relative_path, file_size, modified_at_ns, state)) =
                updates.get(&page.page_index)
            {
                if *state == "stale" {
                    stale_pages += 1;
                }
                transaction.execute(
                    "UPDATE image_sequence_pages
                     SET relative_path = ?1, file_size = ?2, modified_at_ns = ?3, state = ?4
                     WHERE sequence_id = ?5 AND page_index = ?6",
                    params![
                        relative_path,
                        file_size,
                        modified_at_ns,
                        state,
                        book_id,
                        page.page_index,
                    ],
                )?;
            } else {
                missing_pages += 1;
                transaction.execute(
                    "UPDATE image_sequence_pages
                     SET state = 'missing'
                     WHERE sequence_id = ?1 AND page_index = ?2",
                    params![book_id, page.page_index],
                )?;
            }
        }
        let sequence_state = if missing_pages > 0 {
            "missing"
        } else if stale_pages > 0 {
            "stale"
        } else {
            "ready"
        };
        transaction.execute(
            "UPDATE library_roots
             SET state = 'available', updated_at = CURRENT_TIMESTAMP,
                 last_verified_at = CURRENT_TIMESTAMP
             WHERE id = ?1",
            params![root_id.as_str()],
        )?;
        transaction.execute(
            "UPDATE books
             SET path = ?1, updated_at = CURRENT_TIMESTAMP
             WHERE id = ?2",
            params![new_root_path.as_str(), book_id],
        )?;
        transaction.execute(
            "UPDATE image_sequences
             SET root_id = ?1, state = ?2, updated_at = CURRENT_TIMESTAMP
             WHERE book_id = ?3",
            params![root_id.as_str(), sequence_state, book_id],
        )?;
        transaction.commit()?;
        drop(connection);
        self.get_image_sequence(book_id)
    }

    pub fn refresh_image_sequence_state(
        &self,
        book_id: &str,
    ) -> Result<ImageSequenceDetail, DbError> {
        let detail = self.get_image_sequence(book_id)?;
        let root_available = fs::metadata(&detail.sequence.root_path)
            .map(|metadata| metadata.is_dir())
            .unwrap_or(false);
        let mut page_updates = Vec::with_capacity(detail.pages.len());
        let mut missing_pages = 0usize;
        let mut stale_pages = 0usize;

        if root_available {
            for page in &detail.pages {
                let path = resolve_image_page_path(&detail.sequence.root_path, &page.relative_path)
                    .map_err(|error| DbError::InvalidImageSequence(error.to_string()))?;
                let metadata = fs::metadata(path)
                    .ok()
                    .filter(|metadata| metadata.is_file());
                let (state, observed_modified_at_ns) = match metadata {
                    None => {
                        missing_pages += 1;
                        ("missing".to_string(), None)
                    }
                    Some(metadata) => {
                        let observed_size = i64::try_from(metadata.len()).unwrap_or(i64::MAX);
                        let observed_modified_at_ns = modified_at_ns(&metadata);
                        let bootstrap = page.modified_at_ns.is_none()
                            && page.file_size == observed_size
                            && page.state != "stale";
                        let matches = page.file_size == observed_size
                            && page.modified_at_ns == observed_modified_at_ns
                            && page.state != "stale";
                        if bootstrap || matches {
                            ("ready".to_string(), observed_modified_at_ns)
                        } else {
                            stale_pages += 1;
                            ("stale".to_string(), None)
                        }
                    }
                };
                page_updates.push((page.page_index, state, observed_modified_at_ns));
            }
        }

        let root_state = if root_available {
            "available"
        } else {
            "needs_relink"
        };
        let sequence_state = if !root_available {
            "needs_relink"
        } else if missing_pages > 0 {
            "missing"
        } else if stale_pages > 0 {
            "stale"
        } else {
            "ready"
        };

        let mut connection = self.connection.lock().map_err(|_| DbError::Lock)?;
        let transaction = connection.transaction()?;
        transaction.execute(
            "UPDATE library_roots
             SET state = ?1, updated_at = CURRENT_TIMESTAMP,
                 last_verified_at = CURRENT_TIMESTAMP
             WHERE id = ?2",
            params![root_state, detail.sequence.root_id.as_str()],
        )?;
        transaction.execute(
            "UPDATE image_sequences
             SET state = ?1, updated_at = CURRENT_TIMESTAMP
             WHERE book_id = ?2",
            params![sequence_state, book_id],
        )?;
        for (page_index, state, observed_modified_at_ns) in page_updates {
            transaction.execute(
                "UPDATE image_sequence_pages
                 SET state = ?1,
                     modified_at_ns = COALESCE(?2, modified_at_ns)
                 WHERE sequence_id = ?3 AND page_index = ?4",
                params![state, observed_modified_at_ns, book_id, page_index],
            )?;
        }
        transaction.commit()?;
        drop(connection);
        self.get_image_sequence(book_id)
    }

    pub fn verify_image_sequence_digests(
        &self,
        book_id: &str,
    ) -> Result<ImageSequenceDetail, DbError> {
        let detail = self.refresh_image_sequence_state(book_id)?;
        if detail.sequence.state == "needs_relink" {
            return Ok(detail);
        }

        let stale_pages = detail
            .pages
            .iter()
            .filter(|page| page.state == "stale")
            .collect::<Vec<_>>();
        if stale_pages.is_empty() {
            return Ok(detail);
        }

        let mut total_digest_bytes = 0_u64;
        let mut page_updates = Vec::with_capacity(stale_pages.len());
        for page in stale_pages {
            let path = resolve_image_page_path(&detail.sequence.root_path, &page.relative_path)
                .map_err(|error| DbError::InvalidImageSequence(error.to_string()))?;
            let Some(metadata) = fs::metadata(&path)
                .ok()
                .filter(|metadata| metadata.is_file())
            else {
                page_updates.push((page.page_index, "missing".to_string(), None));
                continue;
            };

            if page.content_digest.is_none() {
                page_updates.push((page.page_index, "stale".to_string(), None));
                continue;
            }

            if metadata.len() > MAX_DIGEST_FILE_BYTES {
                return Err(DbError::InvalidImageSequence(format!(
                    "单页图片超过 {} MB SHA-256 复核上限",
                    MAX_DIGEST_FILE_BYTES / (1024 * 1024)
                )));
            }
            if total_digest_bytes.saturating_add(metadata.len()) > MAX_DIGEST_TOTAL_BYTES {
                return Err(DbError::InvalidImageSequence(format!(
                    "本次 SHA-256 复核总量超过 {} MB 上限",
                    MAX_DIGEST_TOTAL_BYTES / (1024 * 1024)
                )));
            }

            let (digest, hashed_bytes) =
                sha256_file(&path, MAX_DIGEST_FILE_BYTES).map_err(DbError::InvalidImageSequence)?;
            total_digest_bytes = total_digest_bytes
                .checked_add(hashed_bytes)
                .ok_or_else(|| {
                    DbError::InvalidImageSequence("图片复核总大小超出安全范围".to_string())
                })?;
            if total_digest_bytes > MAX_DIGEST_TOTAL_BYTES {
                return Err(DbError::InvalidImageSequence(format!(
                    "本次 SHA-256 复核总量超过 {} MB 上限",
                    MAX_DIGEST_TOTAL_BYTES / (1024 * 1024)
                )));
            }

            let expected_digest = page
                .content_digest
                .as_deref()
                .unwrap_or_default()
                .strip_prefix("sha256:")
                .unwrap_or_else(|| page.content_digest.as_deref().unwrap_or_default());
            let state = if expected_digest.eq_ignore_ascii_case(&digest) {
                "ready"
            } else {
                "stale"
            };
            let observed_modified_at_ns = if state == "ready" {
                modified_at_ns(&metadata)
            } else {
                None
            };
            page_updates.push((page.page_index, state.to_string(), observed_modified_at_ns));
        }

        let mut states = detail
            .pages
            .iter()
            .map(|page| (page.page_index, page.state.as_str()))
            .collect::<HashMap<_, _>>();
        for (page_index, state, _) in &page_updates {
            states.insert(*page_index, state.as_str());
        }
        let sequence_state = if states.values().any(|state| *state == "missing") {
            "missing"
        } else if states.values().any(|state| *state == "stale") {
            "stale"
        } else {
            "ready"
        };

        let mut connection = self.connection.lock().map_err(|_| DbError::Lock)?;
        let transaction = connection.transaction()?;
        transaction.execute(
            "UPDATE library_roots
             SET state = 'available', updated_at = CURRENT_TIMESTAMP,
                 last_verified_at = CURRENT_TIMESTAMP
             WHERE id = ?1",
            params![detail.sequence.root_id.as_str()],
        )?;
        transaction.execute(
            "UPDATE image_sequences
             SET state = ?1, updated_at = CURRENT_TIMESTAMP
             WHERE book_id = ?2",
            params![sequence_state, book_id],
        )?;
        for (page_index, state, observed_modified_at_ns) in page_updates {
            transaction.execute(
                "UPDATE image_sequence_pages
                 SET state = ?1,
                     modified_at_ns = COALESCE(?2, modified_at_ns)
                 WHERE sequence_id = ?3 AND page_index = ?4",
                params![state, observed_modified_at_ns, book_id, page_index],
            )?;
        }
        transaction.commit()?;
        drop(connection);
        self.get_image_sequence(book_id)
    }

    pub fn save_image_sequence_progress(
        &self,
        book_id: &str,
        current_page: i64,
        zoom: f64,
        direction: &str,
        spread: &str,
    ) -> Result<ImageSequenceSummary, DbError> {
        if !matches!(direction, "ltr" | "rtl" | "vertical") {
            return Err(DbError::InvalidImageSequence("阅读方向无效".to_string()));
        }
        if !matches!(spread, "single" | "double" | "long_strip") {
            return Err(DbError::InvalidImageSequence("排版模式无效".to_string()));
        }
        if !zoom.is_finite() || !(0.0 < zoom && zoom <= 8.0) {
            return Err(DbError::InvalidImageSequence(
                "缩放比例必须在 0 到 8 之间".to_string(),
            ));
        }

        let connection = self.connection.lock().map_err(|_| DbError::Lock)?;
        let page_count: i64 = connection
            .query_row(
                "SELECT page_count FROM image_sequences WHERE book_id = ?1",
                params![book_id],
                |row| row.get(0),
            )
            .optional()?
            .ok_or(DbError::NotFound)?;
        if current_page < 0 || current_page >= page_count {
            return Err(DbError::InvalidImageSequence(
                "当前页码超出图片序列范围".to_string(),
            ));
        }
        let progress = if page_count <= 1 {
            0.0
        } else {
            (current_page as f64 / (page_count - 1) as f64).clamp(0.0, 1.0)
        };
        let mut transaction = connection.unchecked_transaction()?;
        transaction.execute(
            "UPDATE image_sequences
             SET current_page = ?1, zoom = ?2, direction = ?3, spread = ?4,
                 state = CASE WHEN state IN ('missing', 'stale') THEN state ELSE 'ready' END,
                 updated_at = CURRENT_TIMESTAMP
             WHERE book_id = ?5",
            params![current_page, zoom, direction, spread, book_id],
        )?;
        transaction.execute(
            "UPDATE books
             SET current_chapter = ?1, progress = ?2, updated_at = CURRENT_TIMESTAMP
             WHERE id = ?3",
            params![current_page, progress, book_id],
        )?;
        transaction.commit()?;
        drop(connection);
        self.get_image_sequence(book_id)
            .map(|detail| detail.sequence)
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
            transaction.execute_batch(
                "DELETE FROM source_cache;
                 DELETE FROM source_request_metrics;
                 DELETE FROM source_rule_metrics;
                 DELETE FROM book_sources;",
            )?;
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
        connection.execute(
            "DELETE FROM source_request_metrics WHERE source_id = ?1",
            params![source_id],
        )?;
        connection.execute(
            "DELETE FROM source_rule_metrics WHERE source_id = ?1",
            params![source_id],
        )?;
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
                "SELECT payload, source_id, kind
                 FROM source_cache
                 WHERE cache_key = ?1 AND expires_at > ?2",
                params![cache_key, now],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                },
            )
            .optional()?;

        if let Some((_, source_id, kind)) = payload.as_ref() {
            record_source_cache_hit_locked(&connection, source_id, kind)?;
        }

        if payload.is_none() {
            connection.execute(
                "DELETE FROM source_cache
                 WHERE cache_key = ?1 AND expires_at <= ?2",
                params![cache_key, now],
            )?;
        }

        Ok(payload.map(|(payload, _, _)| payload))
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

    pub fn record_source_request_outcome(
        &self,
        source_id: &str,
        stage: &str,
        success: bool,
    ) -> Result<(), DbError> {
        let connection = self.connection.lock().map_err(|_| DbError::Lock)?;
        let successes = i64::from(success);
        let failures = i64::from(!success);
        connection.execute(
            "INSERT INTO source_request_metrics
                (source_id, stage, attempts, successes, failures, cache_hits)
             VALUES (?1, ?2, 1, ?3, ?4, 0)
             ON CONFLICT(source_id, stage) DO UPDATE SET
                attempts = attempts + excluded.attempts,
                successes = successes + excluded.successes,
                failures = failures + excluded.failures,
                updated_at = CURRENT_TIMESTAMP",
            params![
                bounded_history_text(source_id, 256),
                bounded_history_text(stage, 128),
                successes,
                failures,
            ],
        )?;
        Ok(())
    }

    pub fn record_source_cache_hit(&self, source_id: &str, stage: &str) -> Result<(), DbError> {
        let connection = self.connection.lock().map_err(|_| DbError::Lock)?;
        record_source_cache_hit_locked(
            &connection,
            &bounded_history_text(source_id, 256),
            &bounded_history_text(stage, 128),
        )?;
        Ok(())
    }

    pub fn source_request_metrics(&self) -> Result<SourceRequestMetrics, DbError> {
        let connection = self.connection.lock().map_err(|_| DbError::Lock)?;
        let (total_attempts, total_successes, total_failures, total_cache_hits): (
            i64,
            i64,
            i64,
            i64,
        ) = connection.query_row(
            "SELECT
                COALESCE(SUM(attempts), 0),
                COALESCE(SUM(successes), 0),
                COALESCE(SUM(failures), 0),
                COALESCE(SUM(cache_hits), 0)
             FROM source_request_metrics",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )?;
        let mut statement = connection.prepare(
            "SELECT stage,
                    COALESCE(SUM(attempts), 0),
                    COALESCE(SUM(successes), 0),
                    COALESCE(SUM(failures), 0),
                    COALESCE(SUM(cache_hits), 0)
             FROM source_request_metrics
             GROUP BY stage
             ORDER BY stage",
        )?;
        let by_stage = statement
            .query_map([], |row| {
                let attempts: i64 = row.get(1)?;
                let successes: i64 = row.get(2)?;
                let failures: i64 = row.get(3)?;
                let cache_hits: i64 = row.get(4)?;
                Ok(SourceRequestMetric {
                    stage: row.get(0)?,
                    attempts: non_negative_usize(attempts),
                    successes: non_negative_usize(successes),
                    failures: non_negative_usize(failures),
                    cache_hits: non_negative_usize(cache_hits),
                    failure_rate: ratio(failures, attempts),
                    cache_hit_rate: ratio(cache_hits, attempts.saturating_add(cache_hits)),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(SourceRequestMetrics {
            total_attempts: non_negative_usize(total_attempts),
            total_successes: non_negative_usize(total_successes),
            total_failures: non_negative_usize(total_failures),
            total_cache_hits: non_negative_usize(total_cache_hits),
            failure_rate: ratio(total_failures, total_attempts),
            cache_hit_rate: ratio(
                total_cache_hits,
                total_attempts.saturating_add(total_cache_hits),
            ),
            by_stage,
        })
    }

    pub fn record_source_rule_outcome(
        &self,
        source_id: &str,
        stage: &str,
        rule_key: &str,
        outcome: SourceRuleOutcome,
    ) -> Result<(), DbError> {
        let connection = self.connection.lock().map_err(|_| DbError::Lock)?;
        let (successes, no_matches, failures, skipped) = match outcome {
            SourceRuleOutcome::Success => (1_i64, 0_i64, 0_i64, 0_i64),
            SourceRuleOutcome::NoMatch => (0_i64, 1_i64, 0_i64, 0_i64),
            SourceRuleOutcome::Failure => (0_i64, 0_i64, 1_i64, 0_i64),
            SourceRuleOutcome::Skipped => (0_i64, 0_i64, 0_i64, 1_i64),
        };
        let attempts = successes + no_matches + failures;
        connection.execute(
            "INSERT INTO source_rule_metrics
                (source_id, stage, rule_key, attempts, successes, no_matches, failures, skipped)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(source_id, stage, rule_key) DO UPDATE SET
                attempts = attempts + excluded.attempts,
                successes = successes + excluded.successes,
                no_matches = no_matches + excluded.no_matches,
                failures = failures + excluded.failures,
                skipped = skipped + excluded.skipped,
                updated_at = CURRENT_TIMESTAMP",
            params![
                bounded_history_text(source_id, 256),
                bounded_history_text(stage, 128),
                bounded_history_text(rule_key, 128),
                attempts,
                successes,
                no_matches,
                failures,
                skipped,
            ],
        )?;
        Ok(())
    }

    pub fn source_rule_metrics(&self) -> Result<SourceRuleMetrics, DbError> {
        let connection = self.connection.lock().map_err(|_| DbError::Lock)?;
        let (total_attempts, total_successes, total_no_matches, total_failures, total_skipped): (
            i64,
            i64,
            i64,
            i64,
            i64,
        ) = connection.query_row(
            "SELECT
                COALESCE(SUM(attempts), 0),
                COALESCE(SUM(successes), 0),
                COALESCE(SUM(no_matches), 0),
                COALESCE(SUM(failures), 0),
                COALESCE(SUM(skipped), 0)
             FROM source_rule_metrics",
            [],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            },
        )?;

        let mut statement = connection.prepare(
            "SELECT stage, rule_key,
                    COALESCE(SUM(attempts), 0),
                    COALESCE(SUM(successes), 0),
                    COALESCE(SUM(no_matches), 0),
                    COALESCE(SUM(failures), 0),
                    COALESCE(SUM(skipped), 0)
             FROM source_rule_metrics
             GROUP BY stage, rule_key
             ORDER BY stage, rule_key",
        )?;
        let by_rule = statement
            .query_map([], |row| {
                let attempts: i64 = row.get(2)?;
                let successes: i64 = row.get(3)?;
                let no_matches: i64 = row.get(4)?;
                let failures: i64 = row.get(5)?;
                let skipped: i64 = row.get(6)?;
                Ok(SourceRuleMetric {
                    stage: row.get(0)?,
                    rule_key: row.get(1)?,
                    attempts: non_negative_usize(attempts),
                    successes: non_negative_usize(successes),
                    no_matches: non_negative_usize(no_matches),
                    failures: non_negative_usize(failures),
                    skipped: non_negative_usize(skipped),
                    success_rate: ratio(successes, attempts),
                    failure_rate: ratio(failures, attempts),
                    observed: attempts > 0,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(SourceRuleMetrics {
            total_attempts: non_negative_usize(total_attempts),
            total_successes: non_negative_usize(total_successes),
            total_no_matches: non_negative_usize(total_no_matches),
            total_failures: non_negative_usize(total_failures),
            total_skipped: non_negative_usize(total_skipped),
            success_rate: ratio(total_successes, total_attempts),
            failure_rate: ratio(total_failures, total_attempts),
            observed: total_attempts > 0,
            by_rule,
        })
    }

    pub fn record_source_failure_history(
        &self,
        source_id: &str,
        source_name: &str,
        stage: &str,
        reason_code: &str,
        operation_id: Option<&str>,
        message: &str,
    ) -> Result<(), DbError> {
        let connection = self.connection.lock().map_err(|_| DbError::Lock)?;
        connection.execute(
            "INSERT INTO source_failure_history
                (id, source_id, source_name, stage, reason_code, operation_id, message)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                generated_id("source-failure"),
                bounded_history_text(source_id, 256),
                bounded_history_text(source_name, 256),
                bounded_history_text(stage, 128),
                bounded_history_text(reason_code, 64),
                operation_id.map(|value| bounded_history_text(value, 128)),
                bounded_history_text(message, 512),
            ],
        )?;
        connection.execute(
            "DELETE FROM source_failure_history
             WHERE id IN (
                 SELECT id
                 FROM source_failure_history
                 ORDER BY created_at DESC, id DESC
                 LIMIT -1 OFFSET 512
             )",
            [],
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
            "SELECT id, source_id, source_name, stage, reason_code, operation_id, message, created_at
             FROM source_failure_history
             WHERE (?1 IS NULL OR source_id = ?1)
             ORDER BY created_at DESC, id DESC
             LIMIT ?2",
        )?;
        let rows =
            statement.query_map(params![source_id, limit], source_failure_history_from_row)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
    }

    pub fn clear_source_failure_history(&self, source_id: Option<&str>) -> Result<usize, DbError> {
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

    pub fn source_failure_stats(&self) -> Result<SourceFailureStats, DbError> {
        let connection = self.connection.lock().map_err(|_| DbError::Lock)?;
        let total: i64 =
            connection.query_row("SELECT COUNT(*) FROM source_failure_history", [], |row| {
                row.get(0)
            })?;

        let mut reason_statement = connection.prepare(
            "SELECT reason_code, COUNT(*)
             FROM source_failure_history
             GROUP BY reason_code
             ORDER BY reason_code",
        )?;
        let by_reason = reason_statement
            .query_map([], |row| {
                Ok(SourceFailureCount {
                    code: row.get(0)?,
                    count: usize::try_from(row.get::<_, i64>(1)?.max(0)).unwrap_or(usize::MAX),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        let mut stage_statement = connection.prepare(
            "SELECT stage, COUNT(*)
             FROM source_failure_history
             GROUP BY stage
             ORDER BY stage",
        )?;
        let by_stage = stage_statement
            .query_map([], |row| {
                Ok(SourceFailureCount {
                    code: row.get(0)?,
                    count: usize::try_from(row.get::<_, i64>(1)?.max(0)).unwrap_or(usize::MAX),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(SourceFailureStats {
            total: usize::try_from(total.max(0)).unwrap_or(usize::MAX),
            by_reason,
            by_stage,
        })
    }

    pub fn get_book_detail(&self, book_id: &str) -> Result<BookDetail, DbError> {
        let connection = self.connection.lock().map_err(|_| DbError::Lock)?;
        let book = connection
            .query_row(
                "SELECT b.id, b.title, b.author, b.format, b.content_kind, COUNT(c.id),                         b.current_chapter, b.progress, b.updated_at,
                    (SELECT s.state FROM image_sequences s WHERE s.book_id = b.id),
                    COALESCE((SELECT COUNT(*) FROM image_sequence_pages p
                              WHERE p.sequence_id = b.id AND p.state = 'missing'), 0),
                    COALESCE((SELECT COUNT(*) FROM image_sequence_pages p
                              WHERE p.sequence_id = b.id AND p.state = 'stale'), 0)
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
                "SELECT b.id, b.title, b.author, b.format, b.content_kind, COUNT(c.id),                         b.current_chapter, b.progress, b.updated_at,
                    (SELECT s.state FROM image_sequences s WHERE s.book_id = b.id),
                    COALESCE((SELECT COUNT(*) FROM image_sequence_pages p
                              WHERE p.sequence_id = b.id AND p.state = 'missing'), 0),
                    COALESCE((SELECT COUNT(*) FROM image_sequence_pages p
                              WHERE p.sequence_id = b.id AND p.state = 'stale'), 0)
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
        (
            9_i64,
            include_str!("../migrations/0009_source_failure_operation.sql"),
        ),
        (
            10_i64,
            include_str!("../migrations/0010_source_request_metrics.sql"),
        ),
        (
            11_i64,
            include_str!("../migrations/0011_source_rule_metrics.sql"),
        ),
        (
            12_i64,
            include_str!("../migrations/0012_source_rule_skipped.sql"),
        ),
        (
            13_i64,
            include_str!("../migrations/0013_image_sequences.sql"),
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
        content_kind: row.get(4)?,
        chapter_count: row.get(5)?,
        current_chapter: row.get(6)?,
        progress: row.get(7)?,
        updated_at: row.get(8)?,
        image_sequence_state: row.get(9)?,
        image_sequence_missing_pages: row.get(10)?,
        image_sequence_stale_pages: row.get(11)?,
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

fn default_digest_version() -> i64 {
    1
}

fn validate_image_sequence_write(
    write: &ImageSequenceWrite,
) -> Result<Vec<ImageSequencePageWrite>, DbError> {
    if write.title.trim().is_empty() {
        return Err(DbError::InvalidImageSequence(
            "图片序列标题不能为空".to_string(),
        ));
    }
    if write.cache_key.trim().is_empty() || write.cache_key.len() > 256 {
        return Err(DbError::InvalidImageSequence(
            "图片序列缓存键无效".to_string(),
        ));
    }
    if !matches!(write.direction.as_str(), "ltr" | "rtl" | "vertical") {
        return Err(DbError::InvalidImageSequence("阅读方向无效".to_string()));
    }
    if !matches!(write.spread.as_str(), "single" | "double" | "long_strip") {
        return Err(DbError::InvalidImageSequence("排版模式无效".to_string()));
    }
    if write.page_count <= 0
        || write.page_count > 2_048
        || write.pages.len() as i64 != write.page_count
    {
        return Err(DbError::InvalidImageSequence(
            "图片页数与页清单不一致或超出上限".to_string(),
        ));
    }
    if write.total_pixels < 0 || write.total_decoded_bytes < 0 {
        return Err(DbError::InvalidImageSequence(
            "图片序列资源统计不能为负数".to_string(),
        ));
    }
    if write.current_page < 0 || write.current_page >= write.page_count {
        return Err(DbError::InvalidImageSequence(
            "当前页码超出图片序列范围".to_string(),
        ));
    }
    if !write.zoom.is_finite() || !(0.0 < write.zoom && write.zoom <= 8.0) {
        return Err(DbError::InvalidImageSequence(
            "缩放比例必须在 0 到 8 之间".to_string(),
        ));
    }

    let mut pages = write.pages.clone();
    for (expected_index, page) in pages.iter_mut().enumerate() {
        if page.page_index != expected_index as i64 {
            return Err(DbError::InvalidImageSequence(
                "图片页码必须从 0 连续递增".to_string(),
            ));
        }
        page.relative_path = normalize_relative_image_path(&page.relative_path)
            .map_err(|error| DbError::InvalidImageSequence(error.to_string()))?;
        if page.file_size < 0 || page.width <= 0 || page.height <= 0 || page.digest_version <= 0 {
            return Err(DbError::InvalidImageSequence(
                "图片页元数据无效".to_string(),
            ));
        }
        if !matches!(
            page.mime.as_str(),
            "image/png" | "image/jpeg" | "image/gif" | "image/webp"
        ) {
            return Err(DbError::InvalidImageSequence(
                "图片 MIME 类型不受支持".to_string(),
            ));
        }
        if page
            .content_digest
            .as_ref()
            .is_some_and(|value| value.len() > 128)
        {
            return Err(DbError::InvalidImageSequence(
                "图片摘要不能超过 128 字节".to_string(),
            ));
        }
    }

    Ok(pages)
}

fn image_sequence_from_row(row: &Row<'_>) -> rusqlite::Result<ImageSequenceSummary> {
    Ok(ImageSequenceSummary {
        book_id: row.get(0)?,
        title: row.get(1)?,
        author: row.get(2)?,
        root_id: row.get(3)?,
        root_path: row.get(4)?,
        cache_key: row.get(5)?,
        direction: row.get(6)?,
        spread: row.get(7)?,
        page_count: row.get(8)?,
        total_pixels: row.get(9)?,
        total_decoded_bytes: row.get(10)?,
        current_page: row.get(11)?,
        zoom: row.get(12)?,
        state: row.get(13)?,
        progress: row.get(14)?,
        updated_at: row.get(15)?,
    })
}

fn image_sequence_page_from_row(row: &Row<'_>) -> rusqlite::Result<ImageSequencePageSummary> {
    Ok(ImageSequencePageSummary {
        sequence_id: row.get(0)?,
        page_index: row.get(1)?,
        relative_path: row.get(2)?,
        file_size: row.get(3)?,
        modified_at_ns: row.get(4)?,
        content_digest: row.get(5)?,
        digest_version: row.get(6)?,
        mime: row.get(7)?,
        width: row.get(8)?,
        height: row.get(9)?,
        state: row.get(10)?,
    })
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

fn record_source_cache_hit_locked(
    connection: &Connection,
    source_id: &str,
    stage: &str,
) -> rusqlite::Result<usize> {
    connection.execute(
        "INSERT INTO source_request_metrics
            (source_id, stage, attempts, successes, failures, cache_hits)
         VALUES (?1, ?2, 0, 0, 0, 1)
         ON CONFLICT(source_id, stage) DO UPDATE SET
            cache_hits = cache_hits + 1,
            updated_at = CURRENT_TIMESTAMP",
        params![source_id, stage],
    )
}

fn bounded_history_text(value: &str, max_chars: usize) -> String {
    value.chars().take(max_chars).collect()
}

fn non_negative_usize(value: i64) -> usize {
    usize::try_from(value.max(0)).unwrap_or(usize::MAX)
}

fn ratio(numerator: i64, denominator: i64) -> f64 {
    if denominator <= 0 {
        0.0
    } else {
        (numerator.max(0) as f64 / denominator as f64).clamp(0.0, 1.0)
    }
}

fn source_failure_history_from_row(row: &Row<'_>) -> rusqlite::Result<SourceFailureHistory> {
    Ok(SourceFailureHistory {
        id: row.get(0)?,
        source_id: row.get(1)?,
        source_name: row.get(2)?,
        stage: row.get(3)?,
        reason_code: row.get(4)?,
        operation_id: row.get(5)?,
        message: row.get(6)?,
        created_at: row.get(7)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn persists_image_sequence_schema_and_enforces_recovery_contract() {
        let directory = std::env::temp_dir().join(format!(
            "open-reader-image-schema-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        let database = Database::open(&directory).expect("database should open");
        let connection = database.connection.lock().expect("database lock");
        let root_path = directory.join("images").to_string_lossy().to_string();

        connection
            .execute(
                "INSERT INTO books (id, title, format) VALUES (?1, ?2, ?3)",
                params!["legacy-book", "Legacy", "txt"],
            )
            .expect("legacy book should insert");
        let legacy_kind: String = connection
            .query_row(
                "SELECT content_kind FROM books WHERE id = ?1",
                params!["legacy-book"],
                |row| row.get(0),
            )
            .expect("legacy content kind should read");
        assert_eq!(legacy_kind, "text");

        connection
            .execute(
                "INSERT INTO library_roots (id, display_name, root_path, state)
                 VALUES (?1, ?2, ?3, ?4)",
                params!["root-1", "Fixture images", root_path, "available"],
            )
            .expect("library root should insert");
        connection
            .execute(
                "INSERT INTO books (id, title, format, content_kind)
                 VALUES (?1, ?2, ?3, ?4)",
                params![
                    "image-book",
                    "Image fixture",
                    "image-sequence",
                    "image_sequence"
                ],
            )
            .expect("image book should insert");
        connection
            .execute(
                "INSERT INTO image_sequences
                   (book_id, root_id, cache_key, page_count, total_pixels, total_decoded_bytes)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    "image-book",
                    "root-1",
                    "imgseq-v1-fixture",
                    1_i64,
                    100_i64,
                    400_i64
                ],
            )
            .expect("image sequence should insert");
        connection
            .execute(
                "INSERT INTO image_sequence_pages
                   (sequence_id, page_index, relative_path, file_size, modified_at_ns,
                    content_digest, mime, width, height)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    "image-book",
                    0_i64,
                    "chapter/001.png",
                    123_i64,
                    456_i64,
                    "sha256:fixture",
                    "image/png",
                    10_i64,
                    10_i64
                ],
            )
            .expect("image page should insert");

        let stored: (String, String, String, i64) = connection
            .query_row(
                "SELECT b.content_kind, s.state, p.relative_path, s.current_page
                 FROM books b
                 JOIN image_sequences s ON s.book_id = b.id
                 JOIN image_sequence_pages p ON p.sequence_id = s.book_id
                 WHERE b.id = ?1",
                params!["image-book"],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .expect("image sequence should read");
        assert_eq!(
            stored,
            (
                "image_sequence".to_string(),
                "ready".to_string(),
                "chapter/001.png".to_string(),
                0,
            )
        );

        let invalid_direction = connection.execute(
            "UPDATE image_sequences SET direction = 'diagonal' WHERE book_id = ?1",
            params!["image-book"],
        );
        assert!(invalid_direction.is_err());

        connection
            .execute("DELETE FROM books WHERE id = ?1", params!["image-book"])
            .expect("book deletion should cascade");
        let remaining_sequences: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM image_sequences WHERE book_id = ?1",
                params!["image-book"],
                |row| row.get(0),
            )
            .expect("sequence count should read");
        let remaining_pages: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM image_sequence_pages WHERE sequence_id = ?1",
                params!["image-book"],
                |row| row.get(0),
            )
            .expect("page count should read");
        assert_eq!(remaining_sequences, 0);
        assert_eq!(remaining_pages, 0);

        drop(connection);
        drop(database);
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn saves_and_restores_image_sequence_records() {
        let directory = std::env::temp_dir().join(format!(
            "open-reader-image-record-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        let root_path = directory.join("images").to_string_lossy().to_string();
        let database = Database::open(&directory).expect("database should open");
        let saved = database
            .save_image_sequence(ImageSequenceWrite {
                book_id: None,
                title: "Image fixture".to_string(),
                author: Some("Fixture author".to_string()),
                root_path: root_path.clone(),
                cache_key: "imgseq-v1-record".to_string(),
                direction: "ltr".to_string(),
                spread: "single".to_string(),
                page_count: 2,
                total_pixels: 20_000,
                total_decoded_bytes: 80_000,
                current_page: 0,
                zoom: 1.0,
                pages: vec![
                    ImageSequencePageWrite {
                        page_index: 0,
                        relative_path: "chapter/001.png".to_string(),
                        file_size: 100,
                        modified_at_ns: Some(1),
                        content_digest: Some("sha256:first".to_string()),
                        digest_version: 1,
                        mime: "image/png".to_string(),
                        width: 100,
                        height: 100,
                    },
                    ImageSequencePageWrite {
                        page_index: 1,
                        relative_path: r"chapter\002.png".to_string(),
                        file_size: 200,
                        modified_at_ns: Some(2),
                        content_digest: None,
                        digest_version: 1,
                        mime: "image/png".to_string(),
                        width: 100,
                        height: 100,
                    },
                ],
            })
            .expect("image sequence should save");
        assert_eq!(saved.title, "Image fixture");
        assert_eq!(saved.page_count, 2);
        assert_eq!(saved.current_page, 0);
        assert_eq!(saved.progress, 0.0);

        let detail = database
            .get_image_sequence(&saved.book_id)
            .expect("image sequence should read");
        assert_eq!(detail.pages.len(), 2);
        assert_eq!(detail.pages[1].relative_path, "chapter/002.png");

        let listed = database
            .list_image_sequences()
            .expect("image sequences should list");
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].book_id, saved.book_id);

        let progressed = database
            .save_image_sequence_progress(&saved.book_id, 1, 1.25, "rtl", "double")
            .expect("image progress should save");
        assert_eq!(progressed.current_page, 1);
        assert_eq!(progressed.progress, 1.0);
        assert_eq!(progressed.direction, "rtl");
        assert_eq!(progressed.spread, "double");

        drop(database);
        let reopened = Database::open(&directory).expect("database should reopen");
        let restored = reopened
            .get_image_sequence(&saved.book_id)
            .expect("image sequence should restore");
        assert_eq!(restored.sequence.current_page, 1);
        assert_eq!(restored.sequence.zoom, 1.25);
        assert_eq!(restored.sequence.state, "ready");
        assert_eq!(restored.pages[1].relative_path, "chapter/002.png");

        let mut invalid = ImageSequenceWrite {
            book_id: None,
            title: "Invalid".to_string(),
            author: None,
            root_path,
            cache_key: "imgseq-v1-invalid".to_string(),
            direction: "ltr".to_string(),
            spread: "single".to_string(),
            page_count: 1,
            total_pixels: 1,
            total_decoded_bytes: 4,
            current_page: 0,
            zoom: 1.0,
            pages: vec![ImageSequencePageWrite {
                page_index: 0,
                relative_path: "../escape.png".to_string(),
                file_size: 1,
                modified_at_ns: None,
                content_digest: None,
                digest_version: 1,
                mime: "image/png".to_string(),
                width: 1,
                height: 1,
            }],
        };
        assert!(matches!(
            reopened.save_image_sequence(invalid.clone()),
            Err(DbError::InvalidImageSequence(_))
        ));
        invalid.root_path = "relative-root".to_string();
        assert!(matches!(
            reopened.save_image_sequence(invalid),
            Err(DbError::InvalidImageSequence(_))
        ));

        drop(reopened);
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn refreshes_image_sequence_states_and_bootstraps_file_metadata() {
        let directory = std::env::temp_dir().join(format!(
            "open-reader-image-state-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        let root = directory.join("images");
        fs::create_dir_all(&root).expect("image root should exist");
        fs::write(root.join("001.png"), b"one").expect("first page should write");
        fs::write(root.join("002.png"), b"two-two").expect("second page should write");

        let root_path = root.to_string_lossy().to_string();
        let database = Database::open(&directory).expect("database should open");
        let saved = database
            .save_image_sequence(ImageSequenceWrite {
                book_id: None,
                title: "State fixture".to_string(),
                author: None,
                root_path: root_path.clone(),
                cache_key: "imgseq-v1-state".to_string(),
                direction: "ltr".to_string(),
                spread: "single".to_string(),
                page_count: 2,
                total_pixels: 20,
                total_decoded_bytes: 40,
                current_page: 0,
                zoom: 1.0,
                pages: vec![
                    ImageSequencePageWrite {
                        page_index: 0,
                        relative_path: "001.png".to_string(),
                        file_size: 3,
                        modified_at_ns: None,
                        content_digest: None,
                        digest_version: 1,
                        mime: "image/png".to_string(),
                        width: 1,
                        height: 1,
                    },
                    ImageSequencePageWrite {
                        page_index: 1,
                        relative_path: "002.png".to_string(),
                        file_size: 7,
                        modified_at_ns: None,
                        content_digest: None,
                        digest_version: 1,
                        mime: "image/png".to_string(),
                        width: 1,
                        height: 1,
                    },
                ],
            })
            .expect("image sequence should save");

        let ready = database
            .refresh_image_sequence_state(&saved.book_id)
            .expect("existing files should be ready");
        assert_eq!(ready.sequence.state, "ready");
        assert!(ready
            .pages
            .iter()
            .all(|page| page.state == "ready" && page.modified_at_ns.is_some()));

        fs::write(root.join("001.png"), b"changed").expect("first page should change");
        let stale = database
            .refresh_image_sequence_state(&saved.book_id)
            .expect("changed file should be inspected");
        assert_eq!(stale.sequence.state, "stale");
        assert_eq!(stale.pages[0].state, "stale");
        assert_eq!(stale.pages[1].state, "ready");

        fs::remove_file(root.join("002.png")).expect("second page should delete");
        let missing = database
            .refresh_image_sequence_state(&saved.book_id)
            .expect("missing file should be inspected");
        assert_eq!(missing.sequence.state, "missing");
        assert_eq!(missing.pages[0].state, "stale");
        assert_eq!(missing.pages[1].state, "missing");

        fs::remove_dir_all(&root).expect("image root should remove");
        let relink = database
            .refresh_image_sequence_state(&saved.book_id)
            .expect("missing root should be recoverable");
        assert_eq!(relink.sequence.state, "needs_relink");

        let connection = database.connection.lock().expect("database lock");
        let root_state: String = connection
            .query_row(
                "SELECT state FROM library_roots WHERE id = ?1",
                params![relink.sequence.root_id.as_str()],
                |row| row.get(0),
            )
            .expect("root state should read");
        assert_eq!(root_state, "needs_relink");

        drop(connection);
        drop(database);
        let _ = fs::remove_dir_all(directory);
    }

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
    fn upgrades_legacy_failure_history_with_operation_ids() {
        let directory = std::env::temp_dir().join(format!(
            "open-reader-source-failure-migration-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        fs::create_dir_all(&directory).expect("directory should create");
        let database_path = directory.join("open-reader.db");
        let connection = Connection::open(&database_path).expect("legacy database should open");
        connection
            .execute_batch(
                "CREATE TABLE schema_migrations (
                   version INTEGER PRIMARY KEY NOT NULL,
                   applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
                 );",
            )
            .expect("migration table should create");
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
            connection
                .execute_batch(sql)
                .expect("legacy migration should apply");
            connection
                .execute(
                    "INSERT INTO schema_migrations (version) VALUES (?1)",
                    params![version],
                )
                .expect("legacy migration should be recorded");
        }
        drop(connection);

        let database = Database::open(&directory).expect("legacy database should upgrade");
        database
            .record_source_failure_history(
                "source-legacy",
                "Legacy",
                "search",
                "request",
                Some("operation-upgraded"),
                "legacy failure",
            )
            .expect("upgraded history should accept operation IDs");
        let entries = database
            .list_source_failure_history(None, 10)
            .expect("upgraded history should list");
        assert_eq!(entries.len(), 1);
        assert_eq!(
            entries[0].operation_id.as_deref(),
            Some("operation-upgraded")
        );
        drop(database);
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn aggregates_source_request_metrics() {
        let directory = std::env::temp_dir().join(format!(
            "open-reader-source-request-metrics-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        let database = Database::open(&directory).expect("database should open");
        database
            .record_source_request_outcome("source-a", "search", true)
            .expect("success should record");
        database
            .record_source_request_outcome("source-a", "search", false)
            .expect("failure should record");
        database
            .record_source_cache_hit("source-a", "book")
            .expect("cache hit should record");

        let metrics = database
            .source_request_metrics()
            .expect("request metrics should read");
        assert_eq!(metrics.total_attempts, 2);
        assert_eq!(metrics.total_successes, 1);
        assert_eq!(metrics.total_failures, 1);
        assert_eq!(metrics.total_cache_hits, 1);
        assert!((metrics.failure_rate - 0.5).abs() < 0.001);
        assert!((metrics.cache_hit_rate - (1.0 / 3.0)).abs() < 0.001);
        assert_eq!(metrics.by_stage.len(), 2);
        assert_eq!(metrics.by_stage[0].stage, "book");
        assert_eq!(metrics.by_stage[0].cache_hits, 1);
        assert_eq!(metrics.by_stage[1].stage, "search");
        assert_eq!(metrics.by_stage[1].attempts, 2);

        drop(database);
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn aggregates_source_rule_metrics() {
        let directory = std::env::temp_dir().join(format!(
            "open-reader-source-rule-metrics-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        let database = Database::open(&directory).expect("database should open");
        database
            .record_source_rule_outcome("source-a", "search", "item", SourceRuleOutcome::Success)
            .expect("success should record");
        database
            .record_source_rule_outcome("source-a", "search", "item", SourceRuleOutcome::NoMatch)
            .expect("no-match should record");
        database
            .record_source_rule_outcome(
                "source-a",
                "content",
                "content",
                SourceRuleOutcome::Failure,
            )
            .expect("failure should record");
        database
            .record_source_rule_outcome("source-a", "search", "item", SourceRuleOutcome::Skipped)
            .expect("skipped should record");

        let metrics = database
            .source_rule_metrics()
            .expect("rule metrics should read");
        assert_eq!(metrics.total_attempts, 3);
        assert_eq!(metrics.total_successes, 1);
        assert_eq!(metrics.total_no_matches, 1);
        assert_eq!(metrics.total_failures, 1);
        assert_eq!(metrics.total_skipped, 1);
        assert!(metrics.observed);
        assert!((metrics.success_rate - (1.0 / 3.0)).abs() < 0.001);
        assert!((metrics.failure_rate - (1.0 / 3.0)).abs() < 0.001);
        assert_eq!(metrics.by_rule.len(), 2);
        assert_eq!(metrics.by_rule[0].stage, "content");
        assert_eq!(metrics.by_rule[0].rule_key, "content");
        assert_eq!(metrics.by_rule[0].failures, 1);
        assert_eq!(metrics.by_rule[1].stage, "search");
        assert_eq!(metrics.by_rule[1].no_matches, 1);
        assert_eq!(metrics.by_rule[1].successes, 1);
        assert_eq!(metrics.by_rule[1].skipped, 1);

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
                None,
                "request failed",
            )
            .expect("failure should persist");
        database
            .record_source_failure_history(
                "source-b",
                "Beta",
                "search",
                "timeout",
                Some("operation-b"),
                &"x".repeat(600),
            )
            .expect("second failure should persist");

        let all = database
            .list_source_failure_history(None, 10)
            .expect("history should list");
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].source_id, "source-b");
        assert_eq!(all[0].operation_id.as_deref(), Some("operation-b"));
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
        let stats = database
            .source_failure_stats()
            .expect("failure stats should read");
        assert_eq!(stats.total, 0);
        assert!(stats.by_reason.is_empty());
        assert!(stats.by_stage.is_empty());
        drop(database);
        let _ = fs::remove_dir_all(directory);
    }
}
