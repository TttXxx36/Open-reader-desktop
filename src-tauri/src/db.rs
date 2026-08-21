use crate::{
    cover::{cover_cache_key, normalize_cover_source, CoverSourceKind},
    image_relink::{
        preview_relink_with_cancel, sha256_file, ImageRelinkAssignment, ImageRelinkPreview,
        RelinkPage, MAX_DIGEST_FILE_BYTES, MAX_DIGEST_TOTAL_BYTES,
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
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    sync::{atomic::AtomicBool, Mutex},
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
    #[error("invalid book metadata: {0}")]
    InvalidBookMetadata(String),
    #[error("invalid source snapshot: {0}")]
    InvalidSourceSnapshot(String),
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
    pub cover_path: Option<String>,
    pub cover_state: Option<String>,
    pub shelf_group: String,
    pub tags: Vec<String>,
    pub custom_order: i64,
    pub chapter_count: i64,
    pub current_chapter: i64,
    pub progress: f64,
    pub updated_at: String,
    pub image_sequence_state: Option<String>,
    pub image_sequence_missing_pages: i64,
    pub image_sequence_stale_pages: i64,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct BookListOptions {
    #[serde(default)]
    pub group: Option<String>,
    #[serde(default)]
    pub query: Option<String>,
    #[serde(default)]
    pub sort: Option<String>,
    #[serde(default)]
    pub descending: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BookMetadataWrite {
    pub book_id: String,
    pub shelf_group: String,
    pub tags: Vec<String>,
    pub cover_path: Option<String>,
    pub custom_order: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BookMetadataBatchWrite {
    pub book_ids: Vec<String>,
    #[serde(default)]
    pub shelf_group: Option<String>,
    #[serde(default)]
    pub tags: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BookCoverSummary {
    pub book_id: String,
    pub source_kind: String,
    pub source_value: String,
    pub source_fingerprint: String,
    pub cache_key: String,
    pub state: String,
    pub mime: Option<String>,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub byte_size: i64,
    pub fetched_at: Option<String>,
    pub last_error: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BookCoverWrite {
    pub book_id: String,
    pub source_kind: CoverSourceKind,
    pub source_value: String,
    #[serde(default)]
    pub source_fingerprint: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChapterSummary {
    pub id: String,
    pub title: String,
    pub index: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReadingState {
    pub position: f64,
    pub read_state: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct BookDetail {
    pub book: BookSummary,
    pub chapters: Vec<ChapterSummary>,
    pub reading_state: ReadingState,
}

#[derive(Debug, Clone, Serialize)]
pub struct DuplicateBookGroup {
    pub key: String,
    pub books: Vec<BookSummary>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BookMergePreviewRequest {
    pub book_ids: Vec<String>,
    pub canonical_book_id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BookMergePreviewRevalidateRequest {
    pub preview: BookMergePreviewRequest,
    pub preview_id: String,
    pub created_at: i64,
    pub expires_at: i64,
    pub input_fingerprint: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BookMergeCommitRequest {
    pub preview: BookMergePreviewRequest,
    pub preview_id: String,
    pub created_at: i64,
    pub expires_at: i64,
    pub input_fingerprint: String,
    pub progress_book_id: String,
    #[serde(default)]
    pub final_shelf_group: Option<String>,
    #[serde(default)]
    pub final_tags: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BookMergeCommitResult {
    pub operation_id: String,
    pub preview_id: String,
    pub canonical_book_id: String,
    pub archived_book_ids: Vec<String>,
    pub appended_chapter_ids: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BookMergeUndoRequest {
    pub operation_id: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct BookMergeUndoResult {
    pub operation_id: String,
    pub canonical_book_id: String,
    pub restored_book_ids: Vec<String>,
    pub removed_chapter_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BookAliasResolution {
    pub requested_book_id: String,
    pub canonical_book_id: String,
    pub redirected: bool,
    pub hops: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookMergeBookPreview {
    pub id: String,
    pub title: String,
    pub author: Option<String>,
    pub format: String,
    pub content_kind: String,
    pub chapter_count: i64,
    pub progress: f64,
    pub current_chapter: i64,
    pub shelf_group: String,
    pub tags: Vec<String>,
    pub cover_state: Option<String>,
    pub image_sequence_state: Option<String>,
    pub image_sequence_root_id: Option<String>,
    pub image_sequence_page_count: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BookMergeChapterCandidate {
    pub source_book_id: String,
    pub chapter_id: String,
    pub title: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct BookMergeProgressCandidate {
    pub book_id: String,
    pub progress: f64,
    pub current_chapter: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct BookMergeCoverCandidate {
    pub book_id: String,
    pub state: Option<String>,
    pub source_kind: Option<String>,
    pub cache_key: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BookMergePreview {
    pub preview_id: String,
    pub created_at: i64,
    pub expires_at: i64,
    pub input_fingerprint: String,
    pub canonical_book_id: String,
    pub archived_book_ids: Vec<String>,
    pub books: Vec<BookMergeBookPreview>,
    pub append_candidates: Vec<BookMergeChapterCandidate>,
    pub chapter_conflicts: Vec<BookMergeChapterCandidate>,
    pub identical_chapter_count: i64,
    pub progress_candidates: Vec<BookMergeProgressCandidate>,
    pub suggested_shelf_group: String,
    pub suggested_tags: Vec<String>,
    pub cover_candidates: Vec<BookMergeCoverCandidate>,
    pub image_sequence_blocked: bool,
    pub conflicts: Vec<String>,
    pub blocked_reasons: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MergeChapterSnapshot {
    id: String,
    title: String,
    digest: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MergeBookSnapshot {
    book: BookMergeBookPreview,
    updated_at: String,
    cover_source_kind: Option<String>,
    cover_cache_key: Option<String>,
    chapters: Vec<MergeChapterSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MergeReadingStateSnapshot {
    position: f64,
    read_state: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MergeAppendedChapterSnapshot {
    id: String,
    title: String,
    digest: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MergeUndoPlan {
    #[serde(default = "default_merge_undo_plan_version")]
    version: i64,
    canonical_book_id: String,
    archived_book_ids: Vec<String>,
    canonical_before: MergeBookSnapshot,
    canonical_after: MergeBookSnapshot,
    canonical_reading_before: Option<MergeReadingStateSnapshot>,
    canonical_reading_after: Option<MergeReadingStateSnapshot>,
    appended_chapters: Vec<MergeAppendedChapterSnapshot>,
}

fn default_merge_undo_plan_version() -> i64 {
    2
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

const SOURCE_SNAPSHOT_RETENTION_COUNT: i64 = 20;
const MAX_SOURCE_SNAPSHOT_BYTES: usize = 128 * 1024 * 1024;

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
        self.list_books_with_options(&BookListOptions::default())
    }

    pub fn list_books_with_options(
        &self,
        options: &BookListOptions,
    ) -> Result<Vec<BookSummary>, DbError> {
        let connection = self.connection.lock().map_err(|_| DbError::Lock)?;
        let group = options.group.as_deref().unwrap_or_default().trim();
        let query = options.query.as_deref().unwrap_or_default().trim();
        let order_by = match options.sort.as_deref().unwrap_or("updated_at") {
            "title" => "b.title COLLATE NOCASE",
            "author" => "COALESCE(b.author, '') COLLATE NOCASE",
            "progress" => "b.progress",
            "custom_order" => "b.custom_order",
            _ => "b.updated_at",
        };
        let direction = if options.descending.unwrap_or(true) {
            "DESC"
        } else {
            "ASC"
        };
        let sql = format!(
            "SELECT b.id, b.title, b.author, b.format, b.content_kind, b.cover_path,
                    (SELECT c.state FROM book_covers c WHERE c.book_id = b.id),
                    b.shelf_group, b.tags_json, b.custom_order, COUNT(c.id),
                    b.current_chapter, b.progress, b.updated_at,
                    (SELECT s.state FROM image_sequences s WHERE s.book_id = b.id),
                    COALESCE((SELECT COUNT(*) FROM image_sequence_pages p
                              WHERE p.sequence_id = b.id AND p.state = 'missing'), 0),
                    COALESCE((SELECT COUNT(*) FROM image_sequence_pages p
                              WHERE p.sequence_id = b.id AND p.state = 'stale'), 0)
             FROM books b
             LEFT JOIN chapters c ON c.book_id = b.id
             WHERE b.lifecycle_state = 'active'
               AND (?1 = '' OR b.shelf_group = ?1)
               AND (?2 = ''
                    OR LOWER(b.title) LIKE '%' || LOWER(?2) || '%'
                    OR LOWER(COALESCE(b.author, '')) LIKE '%' || LOWER(?2) || '%'
                    OR LOWER(b.tags_json) LIKE '%' || LOWER(?2) || '%')
             GROUP BY b.id
             ORDER BY {order_by} {direction}, b.id"
        );
        let mut statement = connection.prepare(&sql)?;
        let rows = statement.query_map(params![group, query], book_from_row)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(DbError::from)
    }

    pub fn find_duplicate_books(&self) -> Result<Vec<DuplicateBookGroup>, DbError> {
        let connection = self.connection.lock().map_err(|_| DbError::Lock)?;
        let group_keys = {
            let mut statement = connection.prepare(
                "SELECT LOWER(TRIM(b.title)), LOWER(TRIM(COALESCE(b.author, ''))), b.format
                 FROM books b
                 WHERE b.lifecycle_state = 'active'
                 GROUP BY LOWER(TRIM(b.title)), LOWER(TRIM(COALESCE(b.author, ''))), b.format
                 HAVING COUNT(*) > 1
                 ORDER BY COUNT(*) DESC, LOWER(TRIM(b.title)), b.format
                 LIMIT 128",
            )?;
            let rows = statement.query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })?;
            rows.collect::<Result<Vec<_>, _>>()?
        };

        let mut groups = Vec::new();
        for (normalized_title, normalized_author, book_format) in group_keys {
            let mut statement = connection.prepare(
                "SELECT b.id, b.title, b.author, b.format, b.content_kind, b.cover_path,
                    (SELECT c.state FROM book_covers c WHERE c.book_id = b.id),
                        b.shelf_group, b.tags_json, b.custom_order, COUNT(c.id),
                        b.current_chapter, b.progress, b.updated_at,
                        (SELECT s.state FROM image_sequences s WHERE s.book_id = b.id),
                        COALESCE((SELECT COUNT(*) FROM image_sequence_pages p
                                  WHERE p.sequence_id = b.id AND p.state = 'missing'), 0),
                        COALESCE((SELECT COUNT(*) FROM image_sequence_pages p
                                  WHERE p.sequence_id = b.id AND p.state = 'stale'), 0)
                 FROM books b
                 LEFT JOIN chapters c ON c.book_id = b.id
                 WHERE b.lifecycle_state = 'active'
                   AND LOWER(TRIM(b.title)) = ?1
                   AND LOWER(TRIM(COALESCE(b.author, ''))) = ?2
                   AND b.format = ?3
                 GROUP BY b.id
                 ORDER BY b.updated_at DESC, b.id
                 LIMIT 256",
            )?;
            let books = statement
                .query_map(
                    params![normalized_title, normalized_author, book_format],
                    book_from_row,
                )?
                .collect::<Result<Vec<_>, _>>()?;
            if books.len() < 2 {
                continue;
            }
            groups.push(DuplicateBookGroup {
                key: format!("{normalized_title}\u{1f}{normalized_author}\u{1f}{book_format}"),
                books,
            });
        }

        Ok(groups)
    }

    pub fn preview_book_merge(
        &self,
        request: BookMergePreviewRequest,
    ) -> Result<BookMergePreview, DbError> {
        let canonical_book_id = request.canonical_book_id.trim().to_string();
        if canonical_book_id.is_empty() {
            return Err(DbError::InvalidBookMetadata(
                "合并预览必须指定保留书籍".to_string(),
            ));
        }

        let mut book_ids = Vec::with_capacity(request.book_ids.len());
        let mut seen = HashSet::new();
        for raw_book_id in request.book_ids {
            let book_id = raw_book_id.trim().to_string();
            if book_id.is_empty() {
                return Err(DbError::InvalidBookMetadata(
                    "合并预览不能包含空书籍 ID".to_string(),
                ));
            }
            if !seen.insert(book_id.clone()) {
                return Err(DbError::InvalidBookMetadata(
                    "合并预览不能包含重复书籍 ID".to_string(),
                ));
            }
            book_ids.push(book_id);
        }
        if !(2..=8).contains(&book_ids.len()) {
            return Err(DbError::InvalidBookMetadata(
                "合并预览一次只接受 2 到 8 本书".to_string(),
            ));
        }
        if !seen.contains(&canonical_book_id) {
            return Err(DbError::InvalidBookMetadata(
                "保留书籍必须属于当前预览候选".to_string(),
            ));
        }

        book_ids.sort();
        let mut ordered_ids = vec![canonical_book_id.clone()];
        ordered_ids.extend(
            book_ids
                .into_iter()
                .filter(|book_id| book_id != &canonical_book_id),
        );

        let connection = self.connection.lock().map_err(|_| DbError::Lock)?;
        let snapshots = load_merge_snapshots(&connection, &ordered_ids)?;

        let canonical = snapshots
            .first()
            .expect("validated merge candidates should not be empty");
        let canonical_title = normalize_merge_text(&canonical.book.title);
        let canonical_author = normalize_merge_text(canonical.book.author.as_deref().unwrap_or(""));
        let canonical_format = normalize_merge_text(&canonical.book.format);
        if snapshots.iter().any(|snapshot| {
            normalize_merge_text(&snapshot.book.title) != canonical_title
                || normalize_merge_text(snapshot.book.author.as_deref().unwrap_or(""))
                    != canonical_author
                || normalize_merge_text(&snapshot.book.format) != canonical_format
        }) {
            return Err(DbError::InvalidBookMetadata(
                "合并预览要求标题、作者和格式一致".to_string(),
            ));
        }

        let mut canonical_chapters = HashMap::new();
        for chapter in &canonical.chapters {
            let key = normalize_merge_text(&chapter.title);
            if let Some(existing_digest) = canonical_chapters.get(&key) {
                if existing_digest != &chapter.digest {
                    // 保留冲突信息，避免把重复标题当作可安全合并。
                    canonical_chapters.insert(key, String::new());
                }
            } else {
                canonical_chapters.insert(key, chapter.digest.clone());
            }
        }

        let mut append_candidates = Vec::new();
        let mut chapter_conflicts = Vec::new();
        let mut identical_chapter_count = 0_i64;
        for snapshot in snapshots.iter().skip(1) {
            for chapter in &snapshot.chapters {
                let key = normalize_merge_text(&chapter.title);
                match canonical_chapters.get(&key) {
                    Some(digest) if !digest.is_empty() && digest == &chapter.digest => {
                        identical_chapter_count += 1;
                    }
                    Some(_) => chapter_conflicts.push(BookMergeChapterCandidate {
                        source_book_id: snapshot.book.id.clone(),
                        chapter_id: chapter.id.clone(),
                        title: chapter.title.clone(),
                        reason: "same_title_content_conflict".to_string(),
                    }),
                    None => append_candidates.push(BookMergeChapterCandidate {
                        source_book_id: snapshot.book.id.clone(),
                        chapter_id: chapter.id.clone(),
                        title: chapter.title.clone(),
                        reason: "new_title".to_string(),
                    }),
                }
            }
        }

        let mut suggested_shelf_group = canonical.book.shelf_group.clone();
        if suggested_shelf_group.trim().is_empty() {
            suggested_shelf_group = snapshots
                .iter()
                .find_map(|snapshot| {
                    let group = snapshot.book.shelf_group.trim();
                    (!group.is_empty()).then(|| group.to_string())
                })
                .unwrap_or_default();
        }
        let mut suggested_tags = Vec::new();
        for snapshot in &snapshots {
            for tag in &snapshot.book.tags {
                let tag = tag.trim();
                if !tag.is_empty() && !suggested_tags.iter().any(|item| item == tag) {
                    suggested_tags.push(tag.to_string());
                }
            }
        }
        suggested_tags.sort();

        let progress_candidates = snapshots
            .iter()
            .map(|snapshot| BookMergeProgressCandidate {
                book_id: snapshot.book.id.clone(),
                progress: snapshot.book.progress,
                current_chapter: snapshot.book.current_chapter,
            })
            .collect::<Vec<_>>();
        let cover_candidates = snapshots
            .iter()
            .map(|snapshot| BookMergeCoverCandidate {
                book_id: snapshot.book.id.clone(),
                state: snapshot.book.cover_state.clone(),
                source_kind: snapshot.cover_source_kind.clone(),
                cache_key: snapshot.cover_cache_key.clone(),
            })
            .collect::<Vec<_>>();

        let mut conflicts = Vec::new();
        let mut blocked_reasons = Vec::new();
        if !chapter_conflicts.is_empty() {
            conflicts.push(format!(
                "{} 个章节标题相同但正文不同",
                chapter_conflicts.len()
            ));
            blocked_reasons.push("存在章节正文冲突，默认不允许合并".to_string());
        }
        let image_snapshots = snapshots
            .iter()
            .filter(|snapshot| snapshot.book.content_kind == "image_sequence");
        let image_roots = image_snapshots
            .filter_map(|snapshot| snapshot.book.image_sequence_root_id.as_deref())
            .collect::<HashSet<_>>();
        let image_sequence_blocked = snapshots
            .iter()
            .any(|snapshot| snapshot.book.content_kind == "image_sequence");
        if image_sequence_blocked {
            conflicts.push("包含图片序列书籍".to_string());
            blocked_reasons.push("图片序列默认阻止合并，需明确只保留某一序列".to_string());
        }
        if image_roots.len() > 1 {
            conflicts.push("图片序列根目录不同".to_string());
        }
        if snapshots.iter().any(|snapshot| {
            snapshot
                .book
                .image_sequence_state
                .as_deref()
                .is_some_and(|state| state != "ready")
        }) {
            conflicts.push("存在图片序列缺页或待复核状态".to_string());
        }

        let created_at = unix_timestamp();
        let input_fingerprint = merge_preview_fingerprint(&snapshots);
        let archived_book_ids = snapshots
            .iter()
            .skip(1)
            .map(|snapshot| snapshot.book.id.clone())
            .collect::<Vec<_>>();

        Ok(BookMergePreview {
            preview_id: generated_id("merge-preview"),
            created_at,
            expires_at: created_at.saturating_add(5 * 60),
            input_fingerprint,
            canonical_book_id,
            archived_book_ids,
            books: snapshots
                .iter()
                .map(|snapshot| snapshot.book.clone())
                .collect(),
            append_candidates,
            chapter_conflicts,
            identical_chapter_count,
            progress_candidates,
            suggested_shelf_group,
            suggested_tags,
            cover_candidates,
            image_sequence_blocked,
            conflicts,
            blocked_reasons,
        })
    }

    pub fn revalidate_book_merge_preview(
        &self,
        request: BookMergePreviewRevalidateRequest,
    ) -> Result<BookMergePreview, DbError> {
        let preview_id = request.preview_id.trim();
        if preview_id.is_empty()
            || preview_id.len() > 128
            || !preview_id.starts_with("merge-preview-")
        {
            return Err(DbError::InvalidBookMetadata(
                "合并预览 ID 无效，请重新生成预览".to_string(),
            ));
        }

        let expected_fingerprint = request.input_fingerprint.trim();
        if expected_fingerprint.is_empty()
            || expected_fingerprint.len() > 128
            || !expected_fingerprint.starts_with("merge-v1-")
        {
            return Err(DbError::InvalidBookMetadata(
                "合并预览指纹无效，请重新生成预览".to_string(),
            ));
        }

        if request.created_at <= 0
            || request.expires_at <= request.created_at
            || request.expires_at - request.created_at > 5 * 60
        {
            return Err(DbError::InvalidBookMetadata(
                "合并预览有效期参数无效，请重新生成预览".to_string(),
            ));
        }

        let now = unix_timestamp();
        if request.created_at > now.saturating_add(60) {
            return Err(DbError::InvalidBookMetadata(
                "合并预览时间来自未来，请重新生成预览".to_string(),
            ));
        }
        if now > request.expires_at {
            return Err(DbError::InvalidBookMetadata(
                "合并预览已过期，请重新生成预览".to_string(),
            ));
        }

        let mut current = self.preview_book_merge(request.preview)?;
        if current.input_fingerprint != expected_fingerprint {
            return Err(DbError::InvalidBookMetadata(
                "书籍数据已变化，请重新生成预览".to_string(),
            ));
        }

        current.preview_id = preview_id.to_string();
        current.created_at = request.created_at;
        current.expires_at = request.expires_at;
        Ok(current)
    }

    pub fn commit_book_merge(
        &self,
        request: BookMergeCommitRequest,
    ) -> Result<BookMergeCommitResult, DbError> {
        let progress_book_id = request.progress_book_id.trim().to_string();
        if progress_book_id.is_empty() {
            return Err(DbError::InvalidBookMetadata(
                "合并提交必须指定阅读进度来源".to_string(),
            ));
        }

        let validated = self.revalidate_book_merge_preview(BookMergePreviewRevalidateRequest {
            preview: request.preview.clone(),
            preview_id: request.preview_id.clone(),
            created_at: request.created_at,
            expires_at: request.expires_at,
            input_fingerprint: request.input_fingerprint.clone(),
        })?;
        if !validated
            .books
            .iter()
            .any(|book| book.id == progress_book_id)
        {
            return Err(DbError::InvalidBookMetadata(
                "阅读进度来源必须属于当前合并预览".to_string(),
            ));
        }
        if !validated.conflicts.is_empty() || !validated.blocked_reasons.is_empty() {
            let reasons = validated
                .blocked_reasons
                .iter()
                .chain(validated.conflicts.iter())
                .cloned()
                .collect::<Vec<_>>()
                .join("；");
            return Err(DbError::InvalidBookMetadata(format!(
                "当前合并预览存在阻止项：{reasons}"
            )));
        }
        if validated
            .books
            .iter()
            .any(|book| book.content_kind != "text")
            || validated
                .cover_candidates
                .iter()
                .any(|cover| cover.source_kind.as_deref() == Some("remote_url"))
        {
            return Err(DbError::InvalidBookMetadata(
                "纯文本合并不支持图片序列或远程封面".to_string(),
            ));
        }
        if validated.append_candidates.len() > 512 {
            return Err(DbError::InvalidBookMetadata(
                "单次合并最多追加 512 个章节".to_string(),
            ));
        }

        let final_shelf_group = request
            .final_shelf_group
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or(validated.suggested_shelf_group.trim())
            .to_string();
        if final_shelf_group.len() > 128 {
            return Err(DbError::InvalidBookMetadata(
                "书架分组不能超过 128 字节".to_string(),
            ));
        }
        let final_tags = normalize_book_tags(
            request
                .final_tags
                .as_ref()
                .unwrap_or(&validated.suggested_tags),
        )?;
        let final_tags_json = serde_json::to_string(&final_tags)
            .map_err(|error| DbError::InvalidBookMetadata(format!("标签序列化失败：{error}")))?;

        let canonical_book_id = validated.canonical_book_id.clone();
        let archived_book_ids = validated.archived_book_ids.clone();
        let operation_id = generated_id("merge-operation");

        let mut ordered_ids = request
            .preview
            .book_ids
            .iter()
            .map(|book_id| book_id.trim().to_string())
            .collect::<Vec<_>>();
        ordered_ids.sort();
        ordered_ids.retain(|book_id| book_id != &canonical_book_id);
        ordered_ids.insert(0, canonical_book_id.clone());

        let mut connection = self.connection.lock().map_err(|_| DbError::Lock)?;
        let transaction = connection.transaction()?;
        let duplicate_preview: i64 = transaction.query_row(
            "SELECT COUNT(*) FROM book_merge_operations WHERE preview_id = ?1",
            params![request.preview_id.trim()],
            |row| row.get(0),
        )?;
        if duplicate_preview > 0 {
            return Err(DbError::InvalidBookMetadata(
                "该合并预览已经提交过，不能重复执行".to_string(),
            ));
        }

        let snapshots = load_merge_snapshots(&transaction, &ordered_ids)?;
        let current_fingerprint = merge_preview_fingerprint(&snapshots);
        if current_fingerprint != request.input_fingerprint.trim() {
            return Err(DbError::InvalidBookMetadata(
                "书籍数据已变化，请重新生成预览".to_string(),
            ));
        }
        if snapshots
            .iter()
            .any(|snapshot| snapshot.book.content_kind != "text")
        {
            return Err(DbError::InvalidBookMetadata(
                "纯文本合并不支持图片序列书籍".to_string(),
            ));
        }
        let canonical_before = snapshots
            .first()
            .cloned()
            .ok_or_else(|| DbError::InvalidBookMetadata("canonical 书籍快照不存在".to_string()))?;
        let canonical_reading_before =
            load_reading_state_snapshot(&transaction, &canonical_book_id)?;

        let mut next_chapter_index: i64 = transaction.query_row(
            "SELECT COALESCE(MAX(chapter_index), -1) + 1 FROM chapters WHERE book_id = ?1",
            params![canonical_book_id],
            |row| row.get(0),
        )?;
        let mut total_body_bytes = 0usize;
        let mut appended_by_source: HashMap<String, Vec<String>> = HashMap::new();
        let mut appended_chapter_ids = Vec::new();
        let mut appended_for_plan = Vec::new();
        for (append_index, candidate) in validated.append_candidates.iter().enumerate() {
            let chapter = transaction
                .query_row(
                    "SELECT title, content, content_format
                     FROM chapters
                     WHERE id = ?1 AND book_id = ?2",
                    params![candidate.chapter_id, candidate.source_book_id],
                    |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, String>(2)?,
                        ))
                    },
                )
                .optional()?
                .ok_or_else(|| {
                    DbError::InvalidBookMetadata("章节数据已变化，请重新生成预览".to_string())
                })?;
            let body_bytes = chapter.1.len();
            if body_bytes > 8 * 1024 * 1024 {
                return Err(DbError::InvalidBookMetadata(
                    "单个追加章节正文不能超过 8 MiB".to_string(),
                ));
            }
            total_body_bytes = total_body_bytes.saturating_add(body_bytes);
            if total_body_bytes > 64 * 1024 * 1024 {
                return Err(DbError::InvalidBookMetadata(
                    "本次追加章节正文总量不能超过 64 MiB".to_string(),
                ));
            }
            let new_chapter_id = format!("{operation_id}-chapter-{append_index}");
            transaction.execute(
                "INSERT INTO chapters (id, book_id, chapter_index, title, content, content_format)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    new_chapter_id,
                    canonical_book_id,
                    next_chapter_index,
                    chapter.0,
                    chapter.1,
                    chapter.2
                ],
            )?;
            next_chapter_index += 1;
            appended_by_source
                .entry(candidate.source_book_id.clone())
                .or_default()
                .push(new_chapter_id.clone());
            appended_for_plan.push(MergeAppendedChapterSnapshot {
                id: new_chapter_id.clone(),
                title: chapter.0,
                digest: merge_text_digest(&chapter.1),
            });
            appended_chapter_ids.push(new_chapter_id);
        }

        let progress = validated
            .progress_candidates
            .iter()
            .find(|candidate| candidate.book_id == progress_book_id)
            .ok_or_else(|| DbError::InvalidBookMetadata("阅读进度来源不存在".to_string()))?;
        let canonical_reading_after = if progress_book_id == canonical_book_id {
            canonical_reading_before.clone()
        } else {
            load_reading_state_snapshot(&transaction, &progress_book_id)?
                .or_else(|| canonical_reading_before.clone())
        };
        let mut canonical_after = canonical_before.clone();
        canonical_after.book.shelf_group = final_shelf_group.clone();
        canonical_after.book.tags = final_tags.clone();
        canonical_after.book.progress = progress.progress;
        canonical_after.book.current_chapter = progress.current_chapter;
        canonical_after.book.chapter_count +=
            i64::try_from(appended_for_plan.len()).unwrap_or(i64::MAX);
        canonical_after
            .chapters
            .extend(
                appended_for_plan
                    .iter()
                    .map(|chapter| MergeChapterSnapshot {
                        id: chapter.id.clone(),
                        title: chapter.title.clone(),
                        digest: chapter.digest.clone(),
                    }),
            );
        let plan = MergeUndoPlan {
            version: 2,
            canonical_book_id: canonical_book_id.clone(),
            archived_book_ids: archived_book_ids.clone(),
            canonical_before,
            canonical_after,
            canonical_reading_before,
            canonical_reading_after,
            appended_chapters: appended_for_plan,
        };
        let plan_json = serde_json::to_string(&plan).map_err(|error| {
            DbError::InvalidBookMetadata(format!("合并计划序列化失败：{error}"))
        })?;
        if plan_json.len() > 64 * 1024 {
            return Err(DbError::InvalidBookMetadata(
                "合并计划超过 64 KiB 限制".to_string(),
            ));
        }
        transaction.execute(
            "INSERT INTO book_merge_operations
                 (id, preview_id, canonical_book_id, status, plan_json, undo_until)
             VALUES (?1, ?2, ?3, 'committed', ?4, datetime('now', '+7 days'))",
            params![
                operation_id,
                request.preview_id.trim(),
                canonical_book_id,
                plan_json
            ],
        )?;

        for snapshot in snapshots.iter().skip(1) {
            let source_snapshot_json = serde_json::json!({
                "book": {
                    "id": &snapshot.book.id,
                    "title": &snapshot.book.title,
                    "author": &snapshot.book.author,
                    "format": &snapshot.book.format,
                    "content_kind": &snapshot.book.content_kind,
                    "progress": snapshot.book.progress,
                    "current_chapter": snapshot.book.current_chapter,
                    "shelf_group": &snapshot.book.shelf_group,
                    "tags": &snapshot.book.tags,
                },
                "merge_snapshot": snapshot,
                "updated_at": &snapshot.updated_at,
                "cover_source_kind": &snapshot.cover_source_kind,
                "cover_cache_key": &snapshot.cover_cache_key,
                "chapters": snapshot.chapters.iter().map(|chapter| serde_json::json!({
                    "id": chapter.id,
                    "title": chapter.title,
                    "digest": chapter.digest,
                })).collect::<Vec<_>>(),
            })
            .to_string();
            if source_snapshot_json.len() > 256 * 1024 {
                return Err(DbError::InvalidBookMetadata(
                    "书籍快照超过 256 KiB 限制".to_string(),
                ));
            }
            let appended_ids_json = serde_json::to_string(
                appended_by_source
                    .get(&snapshot.book.id)
                    .cloned()
                    .unwrap_or_default()
                    .as_slice(),
            )
            .map_err(|error| {
                DbError::InvalidBookMetadata(format!("追加章节序列化失败：{error}"))
            })?;
            transaction.execute(
                "INSERT INTO book_merge_items
                     (operation_id, source_book_id, canonical_book_id,
                      source_snapshot_json, appended_chapter_ids_json)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    operation_id,
                    snapshot.book.id,
                    canonical_book_id,
                    source_snapshot_json,
                    appended_ids_json
                ],
            )?;
        }

        transaction.execute(
            "UPDATE books
             SET shelf_group = ?1,
                 tags_json = ?2,
                 progress = ?3,
                 current_chapter = ?4,
                 updated_at = CURRENT_TIMESTAMP
             WHERE id = ?5 AND lifecycle_state = 'active'",
            params![
                final_shelf_group,
                final_tags_json,
                progress.progress,
                progress.current_chapter,
                canonical_book_id
            ],
        )?;
        if progress_book_id != canonical_book_id {
            if let Some((position, read_state)) = transaction
                .query_row(
                    "SELECT position, read_state FROM book_reading_state WHERE book_id = ?1",
                    params![progress_book_id],
                    |row| Ok((row.get::<_, f64>(0)?, row.get::<_, String>(1)?)),
                )
                .optional()?
            {
                transaction.execute(
                    "INSERT INTO book_reading_state (book_id, position, read_state, updated_at)
                     VALUES (?1, ?2, ?3, CURRENT_TIMESTAMP)
                     ON CONFLICT(book_id) DO UPDATE SET
                         position = excluded.position,
                         read_state = excluded.read_state,
                         updated_at = CURRENT_TIMESTAMP",
                    params![canonical_book_id, position, read_state],
                )?;
            }
        }

        for source_book_id in &archived_book_ids {
            let changed = transaction.execute(
                "UPDATE books
                 SET lifecycle_state = 'merged', updated_at = CURRENT_TIMESTAMP
                 WHERE id = ?1 AND lifecycle_state = 'active'",
                params![source_book_id],
            )?;
            if changed != 1 {
                return Err(DbError::InvalidBookMetadata(
                    "待归档书籍状态已变化，请重新生成预览".to_string(),
                ));
            }
            transaction.execute(
                "INSERT INTO book_aliases (alias_book_id, canonical_book_id, operation_id)
                 VALUES (?1, ?2, ?3)",
                params![source_book_id, canonical_book_id, operation_id],
            )?;
        }

        transaction.commit()?;
        Ok(BookMergeCommitResult {
            operation_id,
            preview_id: request.preview_id,
            canonical_book_id,
            archived_book_ids,
            appended_chapter_ids,
        })
    }

    pub fn undo_book_merge(
        &self,
        request: BookMergeUndoRequest,
    ) -> Result<BookMergeUndoResult, DbError> {
        let operation_id = request.operation_id.trim();
        if operation_id.is_empty()
            || operation_id.len() > 128
            || !operation_id.starts_with("merge-operation-")
        {
            return Err(DbError::InvalidBookMetadata("合并操作 ID 无效".to_string()));
        }

        let mut connection = self.connection.lock().map_err(|_| DbError::Lock)?;
        let transaction = connection.transaction()?;
        let operation = transaction
            .query_row(
                "SELECT canonical_book_id, status, undo_until, plan_json
                 FROM book_merge_operations
                 WHERE id = ?1",
                params![operation_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                    ))
                },
            )
            .optional()?
            .ok_or(DbError::NotFound)?;
        if operation.1 != "committed" {
            return Err(DbError::InvalidBookMetadata(match operation.1.as_str() {
                "undone" => "该合并操作已经撤销".to_string(),
                "expired" => "该合并操作已超过 7 天撤销期限".to_string(),
                _ => "合并操作状态无效".to_string(),
            }));
        }

        let expired: i64 = transaction.query_row(
            "SELECT CASE WHEN unixepoch('now') > unixepoch(?1) THEN 1 ELSE 0 END",
            params![operation.2],
            |row| row.get(0),
        )?;
        if expired != 0 {
            transaction.execute(
                "UPDATE book_merge_operations
                 SET status = 'expired'
                 WHERE id = ?1 AND status = 'committed'",
                params![operation_id],
            )?;
            transaction.commit()?;
            return Err(DbError::InvalidBookMetadata(
                "该合并操作已超过 7 天撤销期限".to_string(),
            ));
        }

        let plan: MergeUndoPlan = serde_json::from_str(&operation.3).map_err(|error| {
            DbError::InvalidBookMetadata(format!("该合并操作不具备 d3 撤销快照：{error}"))
        })?;
        if plan.version < 2 || plan.canonical_book_id != operation.0 {
            return Err(DbError::InvalidBookMetadata(
                "该合并操作缺少可安全撤销的完整快照".to_string(),
            ));
        }

        let canonical =
            load_merge_snapshots_any_lifecycle(&transaction, &[plan.canonical_book_id.clone()])?
                .into_iter()
                .next()
                .ok_or(DbError::NotFound)?;
        if !merge_snapshot_matches(&canonical, &plan.canonical_after) {
            return Err(DbError::InvalidBookMetadata(
                "canonical 书籍在合并后发生外部修改，已拒绝撤销".to_string(),
            ));
        }
        let current_reading = load_reading_state_snapshot(&transaction, &plan.canonical_book_id)?;
        if current_reading
            .as_ref()
            .map(|state| (&state.read_state, state.position))
            != plan
                .canonical_reading_after
                .as_ref()
                .map(|state| (&state.read_state, state.position))
        {
            return Err(DbError::InvalidBookMetadata(
                "canonical 阅读状态在合并后发生外部修改，已拒绝撤销".to_string(),
            ));
        }

        let mut source_ids = Vec::new();
        let mut item_statement = transaction.prepare(
            "SELECT source_book_id, source_snapshot_json
             FROM book_merge_items
             WHERE operation_id = ?1
             ORDER BY source_book_id",
        )?;
        let items = item_statement
            .query_map(params![operation_id], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        if items.len() != plan.archived_book_ids.len() {
            return Err(DbError::InvalidBookMetadata(
                "合并来源快照数量已变化，已拒绝撤销".to_string(),
            ));
        }
        for (source_book_id, source_snapshot_json) in items {
            if !plan.archived_book_ids.contains(&source_book_id) {
                return Err(DbError::InvalidBookMetadata(
                    "合并来源列表已变化，已拒绝撤销".to_string(),
                ));
            }
            let expected_source = parse_stored_merge_snapshot(&source_snapshot_json)?;
            let current_source = load_merge_snapshots_any_lifecycle(
                &transaction,
                std::slice::from_ref(&source_book_id),
            )?
            .into_iter()
            .next()
            .ok_or(DbError::NotFound)?;
            if !merge_snapshot_matches(&current_source, &expected_source) {
                return Err(DbError::InvalidBookMetadata(
                    "来源书籍在合并后发生外部修改，已拒绝撤销".to_string(),
                ));
            }
            let lifecycle: String = transaction.query_row(
                "SELECT lifecycle_state FROM books WHERE id = ?1",
                params![source_book_id],
                |row| row.get(0),
            )?;
            if lifecycle != "merged" {
                return Err(DbError::InvalidBookMetadata(
                    "来源书籍生命周期已变化，已拒绝撤销".to_string(),
                ));
            }
            source_ids.push(source_book_id);
        }

        let alias_count: i64 = transaction.query_row(
            "SELECT COUNT(*) FROM book_aliases
             WHERE operation_id = ?1 AND canonical_book_id = ?2",
            params![operation_id, plan.canonical_book_id],
            |row| row.get(0),
        )?;
        if alias_count != i64::try_from(plan.archived_book_ids.len()).unwrap_or(i64::MAX) {
            return Err(DbError::InvalidBookMetadata(
                "合并别名已变化，已拒绝撤销".to_string(),
            ));
        }

        for appended in &plan.appended_chapters {
            let current = transaction
                .query_row(
                    "SELECT title, content
                     FROM chapters
                     WHERE id = ?1 AND book_id = ?2",
                    params![appended.id, plan.canonical_book_id],
                    |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
                )
                .optional()?
                .ok_or_else(|| {
                    DbError::InvalidBookMetadata("本次追加章节已被删除，已拒绝撤销".to_string())
                })?;
            if current.0 != appended.title || merge_text_digest(&current.1) != appended.digest {
                return Err(DbError::InvalidBookMetadata(
                    "本次追加章节在合并后发生外部修改，已拒绝撤销".to_string(),
                ));
            }
        }

        let mut removed_chapter_ids = Vec::with_capacity(plan.appended_chapters.len());
        for appended in &plan.appended_chapters {
            let changed = transaction.execute(
                "DELETE FROM chapters WHERE id = ?1 AND book_id = ?2",
                params![appended.id, plan.canonical_book_id],
            )?;
            if changed != 1 {
                return Err(DbError::InvalidBookMetadata(
                    "本次追加章节删除数量不一致，已回滚撤销".to_string(),
                ));
            }
            removed_chapter_ids.push(appended.id.clone());
        }

        let tags_json = serde_json::to_string(&plan.canonical_before.book.tags)
            .map_err(|error| DbError::InvalidBookMetadata(format!("标签恢复失败：{error}")))?;
        let changed = transaction.execute(
            "UPDATE books
             SET shelf_group = ?1,
                 tags_json = ?2,
                 progress = ?3,
                 current_chapter = ?4,
                 updated_at = CURRENT_TIMESTAMP
             WHERE id = ?5 AND lifecycle_state = 'active'",
            params![
                plan.canonical_before.book.shelf_group,
                tags_json,
                plan.canonical_before.book.progress,
                plan.canonical_before.book.current_chapter,
                plan.canonical_book_id
            ],
        )?;
        if changed != 1 {
            return Err(DbError::InvalidBookMetadata(
                "canonical 生命周期已变化，已拒绝撤销".to_string(),
            ));
        }

        if let Some(reading) = plan.canonical_reading_before {
            transaction.execute(
                "INSERT INTO book_reading_state (book_id, position, read_state, updated_at)
                 VALUES (?1, ?2, ?3, CURRENT_TIMESTAMP)
                 ON CONFLICT(book_id) DO UPDATE SET
                    position = excluded.position,
                    read_state = excluded.read_state,
                    updated_at = CURRENT_TIMESTAMP",
                params![plan.canonical_book_id, reading.position, reading.read_state],
            )?;
        } else {
            transaction.execute(
                "DELETE FROM book_reading_state WHERE book_id = ?1",
                params![plan.canonical_book_id],
            )?;
        }

        for source_book_id in &source_ids {
            let changed = transaction.execute(
                "UPDATE books
                 SET lifecycle_state = 'active', updated_at = CURRENT_TIMESTAMP
                 WHERE id = ?1 AND lifecycle_state = 'merged'",
                params![source_book_id],
            )?;
            if changed != 1 {
                return Err(DbError::InvalidBookMetadata(
                    "来源书籍生命周期恢复数量不一致，已回滚撤销".to_string(),
                ));
            }
        }
        transaction.execute(
            "DELETE FROM book_aliases WHERE operation_id = ?1",
            params![operation_id],
        )?;
        transaction.execute(
            "UPDATE book_merge_operations
             SET status = 'undone', undone_at = CURRENT_TIMESTAMP
             WHERE id = ?1 AND status = 'committed'",
            params![operation_id],
        )?;
        transaction.commit()?;

        let mut restored_book_ids = vec![plan.canonical_book_id.clone()];
        restored_book_ids.extend(source_ids);
        Ok(BookMergeUndoResult {
            operation_id: operation_id.to_string(),
            canonical_book_id: plan.canonical_book_id,
            restored_book_ids,
            removed_chapter_ids,
        })
    }

    pub fn resolve_book_alias(&self, book_id: &str) -> Result<BookAliasResolution, DbError> {
        let connection = self.connection.lock().map_err(|_| DbError::Lock)?;
        resolve_book_alias_with_connection(&connection, book_id)
    }

    pub fn get_book_cover(&self, book_id: &str) -> Result<Option<BookCoverSummary>, DbError> {
        let book_id = book_id.trim();
        if book_id.is_empty() {
            return Err(DbError::InvalidBookMetadata("书籍 ID 不能为空".to_string()));
        }
        let connection = self.connection.lock().map_err(|_| DbError::Lock)?;
        connection
            .query_row(
                "SELECT book_id, source_kind, source_value, source_fingerprint, cache_key,
                        state, mime, width, height, byte_size, fetched_at, last_error, updated_at
                 FROM book_covers
                 WHERE book_id = ?1",
                params![book_id],
                book_cover_from_row,
            )
            .optional()
            .map_err(DbError::from)
    }

    pub fn save_book_cover(&self, write: BookCoverWrite) -> Result<BookCoverSummary, DbError> {
        let book_id = write.book_id.trim();
        if book_id.is_empty() {
            return Err(DbError::InvalidBookMetadata("书籍 ID 不能为空".to_string()));
        }

        let source = normalize_cover_source(write.source_kind, &write.source_value)
            .map_err(|error| DbError::InvalidBookMetadata(format!("封面来源无效：{error}")))?;
        let fingerprint = write
            .source_fingerprint
            .unwrap_or_default()
            .trim()
            .to_string();
        if fingerprint.len() > 512 || fingerprint.chars().any(|character| character.is_control()) {
            return Err(DbError::InvalidBookMetadata("封面校验信息无效".to_string()));
        }
        let cache_key = cover_cache_key(&source, Some(&fingerprint))
            .map_err(|error| DbError::InvalidBookMetadata(format!("封面缓存键无效：{error}")))?;
        let source_kind = match write.source_kind {
            CoverSourceKind::None => "none",
            CoverSourceKind::LocalPath => "local_path",
            CoverSourceKind::RemoteUrl => "remote_url",
        };
        let state = if write.source_kind == CoverSourceKind::None {
            "missing"
        } else {
            "stale"
        };

        let mut connection = self.connection.lock().map_err(|_| DbError::Lock)?;
        let transaction = connection.transaction()?;
        ensure_book_active(&transaction, book_id)?;
        let exists: i64 = transaction.query_row(
            "SELECT COUNT(*) FROM books WHERE id = ?1",
            params![book_id],
            |row| row.get(0),
        )?;
        if exists == 0 {
            return Err(DbError::NotFound);
        }
        transaction.execute(
            "INSERT INTO book_covers (
                 book_id, source_kind, source_value, source_fingerprint, cache_key,
                 state, mime, width, height, byte_size, fetched_at, last_error, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, NULL, NULL, 0, NULL, NULL, CURRENT_TIMESTAMP)
             ON CONFLICT(book_id) DO UPDATE SET
                 source_kind = excluded.source_kind,
                 source_value = excluded.source_value,
                 source_fingerprint = excluded.source_fingerprint,
                 cache_key = excluded.cache_key,
                 state = excluded.state,
                 mime = NULL,
                 width = NULL,
                 height = NULL,
                 byte_size = 0,
                 fetched_at = NULL,
                 last_error = NULL,
                 updated_at = CURRENT_TIMESTAMP",
            params![
                book_id,
                source_kind,
                source.value,
                fingerprint,
                cache_key,
                state
            ],
        )?;
        transaction.commit()?;
        drop(connection);
        self.get_book_cover(book_id)?.ok_or(DbError::NotFound)
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
            warnings: _,
        } = parsed;

        let mut connection = self.connection.lock().map_err(|_| DbError::Lock)?;
        let transaction = connection.transaction()?;

        let merged_path_count: i64 = transaction.query_row(
            "SELECT COUNT(*) FROM books WHERE path = ?1 AND lifecycle_state = 'merged'",
            params![source_name],
            |row| row.get(0),
        )?;
        if merged_path_count > 0 {
            return Err(DbError::InvalidBookMetadata(
                "该文件对应的书籍已合并，不能覆盖；请先选择当前书籍或撤销合并".to_string(),
            ));
        }
        let canonical_path_count: i64 = transaction.query_row(
            "SELECT COUNT(*)
             FROM books b
             WHERE b.path = ?1
               AND EXISTS (
                   SELECT 1 FROM book_merge_operations op
                   WHERE op.canonical_book_id = b.id AND op.status = 'committed'
               )",
            params![source_name],
            |row| row.get(0),
        )?;
        if canonical_path_count > 0 {
            return Err(DbError::InvalidBookMetadata(
                "该文件对应的书籍是合并后的当前书籍，不能覆盖".to_string(),
            ));
        }

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

    pub fn update_book_metadata(&self, write: BookMetadataWrite) -> Result<BookSummary, DbError> {
        let shelf_group = write.shelf_group.trim();
        if shelf_group.len() > 128 {
            return Err(DbError::InvalidBookMetadata(
                "书架分组不能超过 128 字节".to_string(),
            ));
        }
        let tags = normalize_book_tags(&write.tags)?;
        let tags_json = serde_json::to_string(&tags)
            .map_err(|error| DbError::InvalidBookMetadata(format!("标签序列化失败：{error}")))?;
        let cover_path = write
            .cover_path
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty());
        let mut connection = self.connection.lock().map_err(|_| DbError::Lock)?;
        ensure_book_active(&connection, &write.book_id)?;
        let changed = connection.execute(
            "UPDATE books
             SET shelf_group = ?1,
                 tags_json = ?2,
                 cover_path = ?3,
                 custom_order = ?4,
                 updated_at = CURRENT_TIMESTAMP
             WHERE id = ?5",
            params![
                shelf_group,
                tags_json,
                cover_path,
                write.custom_order,
                write.book_id
            ],
        )?;
        if changed == 0 {
            return Err(DbError::NotFound);
        }
        drop(connection);
        self.get_book_summary(&write.book_id)
    }

    pub fn rename_book(&self, book_id: &str, title: &str) -> Result<BookSummary, DbError> {
        let title = title.trim();
        if title.is_empty() {
            return Err(DbError::InvalidBookMetadata("书名不能为空".to_string()));
        }
        if title.len() > 512 {
            return Err(DbError::InvalidBookMetadata(
                "书名不能超过 512 字节".to_string(),
            ));
        }
        let connection = self.connection.lock().map_err(|_| DbError::Lock)?;
        ensure_book_active(&connection, book_id)?;
        let changed = connection.execute(
            "UPDATE books
             SET title = ?1, updated_at = CURRENT_TIMESTAMP
             WHERE id = ?2",
            params![title, book_id],
        )?;
        if changed == 0 {
            return Err(DbError::NotFound);
        }
        drop(connection);
        self.get_book_summary(book_id)
    }

    pub fn delete_book(&self, book_id: &str) -> Result<(), DbError> {
        let mut connection = self.connection.lock().map_err(|_| DbError::Lock)?;
        let transaction = connection.transaction()?;
        ensure_book_active(&transaction, book_id)?;
        let merge_reference_count: i64 = transaction.query_row(
            "SELECT COUNT(*) FROM book_merge_operations
             WHERE canonical_book_id = ?1 AND status = 'committed'",
            params![book_id],
            |row| row.get(0),
        )?;
        if merge_reference_count > 0 {
            return Err(DbError::InvalidBookMetadata(
                "当前书籍已有合并记录，不能直接删除".to_string(),
            ));
        }
        let changed = transaction.execute("DELETE FROM books WHERE id = ?1", params![book_id])?;
        if changed == 0 {
            return Err(DbError::NotFound);
        }
        transaction.commit()?;
        Ok(())
    }

    pub fn update_books_metadata(
        &self,
        write: BookMetadataBatchWrite,
    ) -> Result<Vec<BookSummary>, DbError> {
        let mut book_ids = Vec::new();
        let mut seen = HashSet::new();
        for book_id in write.book_ids {
            let book_id = book_id.trim();
            if book_id.is_empty() {
                continue;
            }
            if seen.insert(book_id.to_string()) {
                book_ids.push(book_id.to_string());
            }
        }
        if book_ids.is_empty() {
            return Err(DbError::InvalidBookMetadata("至少选择一本书".to_string()));
        }
        if book_ids.len() > 256 {
            return Err(DbError::InvalidBookMetadata(
                "单次批量编辑不能超过 256 本书".to_string(),
            ));
        }

        let shelf_group = write.shelf_group.as_deref().map(str::trim);
        if let Some(group) = shelf_group {
            if group.len() > 128 {
                return Err(DbError::InvalidBookMetadata(
                    "书架分组不能超过 128 字节".to_string(),
                ));
            }
        }
        let tags_json = if let Some(tags) = write.tags.as_ref() {
            let tags = normalize_book_tags(tags)?;
            Some(serde_json::to_string(&tags).map_err(|error| {
                DbError::InvalidBookMetadata(format!("标签序列化失败：{error}"))
            })?)
        } else {
            None
        };
        if shelf_group.is_none() && tags_json.is_none() {
            return Err(DbError::InvalidBookMetadata(
                "批量编辑至少需要提供分组或标签".to_string(),
            ));
        }

        let mut connection = self.connection.lock().map_err(|_| DbError::Lock)?;
        let transaction = connection.transaction()?;
        for book_id in &book_ids {
            ensure_book_active(&transaction, book_id)?;
            let changed = transaction.execute(
                "UPDATE books
                 SET shelf_group = COALESCE(?1, shelf_group),
                     tags_json = COALESCE(?2, tags_json),
                     updated_at = CURRENT_TIMESTAMP
                 WHERE id = ?3",
                params![shelf_group, tags_json.as_deref(), book_id],
            )?;
            if changed == 0 {
                return Err(DbError::NotFound);
            }
        }
        transaction.commit()?;
        drop(connection);

        book_ids
            .into_iter()
            .map(|book_id| self.get_book_summary(&book_id))
            .collect()
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

        if write
            .book_id
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty())
        {
            ensure_book_active(&transaction, &book_id)?;
        }

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
             JOIN books b ON b.id = s.book_id AND b.lifecycle_state = 'active'
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
                 JOIN books b ON b.id = s.book_id AND b.lifecycle_state = 'active'
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
        self.preview_image_sequence_relink_with_cancel(book_id, new_root_path, None)
    }

    pub fn preview_image_sequence_relink_with_cancel(
        &self,
        book_id: &str,
        new_root_path: &str,
        cancel: Option<&AtomicBool>,
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
        preview_relink_with_cancel(
            book_id,
            &detail.sequence.root_path,
            new_root_path,
            &pages,
            cancel,
        )
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
        ensure_book_active(&transaction, book_id)?;
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
        ensure_book_active(&transaction, book_id)?;
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
        ensure_book_active(&transaction, book_id)?;
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
        ensure_book_active(&connection, book_id)?;
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
        let payload_bytes = payload_json.as_bytes().len();
        if payload_bytes > MAX_SOURCE_SNAPSHOT_BYTES {
            return Err(DbError::InvalidSourceSnapshot(format!(
                "快照内容不能超过 {} MiB",
                MAX_SOURCE_SNAPSHOT_BYTES / (1024 * 1024)
            )));
        }
        let id = generated_id("source-snapshot");
        let connection = self.connection.lock().map_err(|_| DbError::Lock)?;
        connection.execute(
            "INSERT INTO source_snapshots (id, label, payload_json, source_count)
             VALUES (?1, ?2, ?3, ?4)",
            params![id, label, payload_json, source_count],
        )?;
        connection.execute(
            "DELETE FROM source_snapshots
             WHERE id IN (
                 SELECT id
                 FROM source_snapshots
                 ORDER BY created_at DESC, id DESC
                 LIMIT -1 OFFSET ?1
             )",
            params![SOURCE_SNAPSHOT_RETENTION_COUNT],
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
             LIMIT ?1",
        )?;
        let rows = statement.query_map(params![SOURCE_SNAPSHOT_RETENTION_COUNT], |row| {
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
        let book_id = resolve_book_alias_with_connection(&connection, book_id)?.canonical_book_id;
        let book = connection
            .query_row(
                "SELECT b.id, b.title, b.author, b.format, b.content_kind, b.cover_path,
                    (SELECT c.state FROM book_covers c WHERE c.book_id = b.id),
                    b.shelf_group, b.tags_json, b.custom_order, COUNT(c.id),
                    b.current_chapter, b.progress, b.updated_at,
                    (SELECT s.state FROM image_sequences s WHERE s.book_id = b.id),
                    COALESCE((SELECT COUNT(*) FROM image_sequence_pages p
                              WHERE p.sequence_id = b.id AND p.state = 'missing'), 0),
                    COALESCE((SELECT COUNT(*) FROM image_sequence_pages p
                              WHERE p.sequence_id = b.id AND p.state = 'stale'), 0)
                 FROM books b
                 LEFT JOIN chapters c ON c.book_id = b.id
                 WHERE b.id = ?1 AND b.lifecycle_state = 'active'
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

        let reading_state = connection
            .query_row(
                "SELECT position, read_state, updated_at
                 FROM book_reading_state
                 WHERE book_id = ?1",
                params![book_id],
                |row| {
                    Ok(ReadingState {
                        position: row.get(0)?,
                        read_state: row.get(1)?,
                        updated_at: row.get(2)?,
                    })
                },
            )
            .optional()?
            .unwrap_or_else(|| ReadingState {
                position: 0.0,
                read_state: "unread".to_string(),
                updated_at: book.updated_at.clone(),
            });

        Ok(BookDetail {
            book,
            chapters,
            reading_state,
        })
    }

    pub fn get_chapter_content(
        &self,
        book_id: &str,
        chapter_id: &str,
    ) -> Result<ChapterContent, DbError> {
        let connection = self.connection.lock().map_err(|_| DbError::Lock)?;
        let book_id = resolve_book_alias_with_connection(&connection, book_id)?.canonical_book_id;
        ensure_book_active(&connection, &book_id)?;
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
        position: f64,
        read_state: Option<&str>,
    ) -> Result<(), DbError> {
        let connection = self.connection.lock().map_err(|_| DbError::Lock)?;
        let book_id = resolve_book_alias_with_connection(&connection, book_id)?.canonical_book_id;
        ensure_book_active(&connection, &book_id)?;
        let progress = progress.clamp(0.0, 1.0);
        let position = if position.is_finite() {
            position.max(0.0)
        } else {
            0.0
        };
        let read_state = match read_state {
            Some("unread") => "unread",
            Some("reading") => "reading",
            Some("finished") => "finished",
            _ if progress >= 0.999 => "finished",
            _ if progress > 0.0 || position > 0.0 => "reading",
            _ => "unread",
        };
        let changed = connection.execute(
            "UPDATE books
             SET current_chapter = ?1, progress = ?2, updated_at = CURRENT_TIMESTAMP
             WHERE id = ?3
               AND EXISTS (SELECT 1 FROM chapters WHERE id = ?4 AND book_id = ?3)",
            params![current_chapter, progress, book_id, chapter_id],
        )?;
        if changed == 0 {
            return Err(DbError::NotFound);
        }
        connection.execute(
            "INSERT INTO book_reading_state (book_id, position, read_state)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(book_id) DO UPDATE SET
                position = excluded.position,
                read_state = excluded.read_state,
                updated_at = CURRENT_TIMESTAMP",
            params![book_id, position, read_state],
        )?;
        Ok(())
    }

    fn get_book_summary(&self, book_id: &str) -> Result<BookSummary, DbError> {
        let connection = self.connection.lock().map_err(|_| DbError::Lock)?;
        connection
            .query_row(
                "SELECT b.id, b.title, b.author, b.format, b.content_kind, b.cover_path,
                    (SELECT c.state FROM book_covers c WHERE c.book_id = b.id),
                    b.shelf_group, b.tags_json, b.custom_order, COUNT(c.id),
                    b.current_chapter, b.progress, b.updated_at,
                    (SELECT s.state FROM image_sequences s WHERE s.book_id = b.id),
                    COALESCE((SELECT COUNT(*) FROM image_sequence_pages p
                              WHERE p.sequence_id = b.id AND p.state = 'missing'), 0),
                    COALESCE((SELECT COUNT(*) FROM image_sequence_pages p
                              WHERE p.sequence_id = b.id AND p.state = 'stale'), 0)
                 FROM books b
                 LEFT JOIN chapters c ON c.book_id = b.id
                 WHERE b.id = ?1 AND b.lifecycle_state = 'active'
                 GROUP BY b.id",
                params![book_id],
                book_from_row,
            )
            .optional()?
            .ok_or(DbError::NotFound)
    }
}

fn load_merge_snapshots(
    connection: &Connection,
    ordered_ids: &[String],
) -> Result<Vec<MergeBookSnapshot>, DbError> {
    load_merge_snapshots_with_lifecycle(connection, ordered_ids, false)
}

fn load_merge_snapshots_any_lifecycle(
    connection: &Connection,
    ordered_ids: &[String],
) -> Result<Vec<MergeBookSnapshot>, DbError> {
    load_merge_snapshots_with_lifecycle(connection, ordered_ids, true)
}

fn load_merge_snapshots_with_lifecycle(
    connection: &Connection,
    ordered_ids: &[String],
    include_merged: bool,
) -> Result<Vec<MergeBookSnapshot>, DbError> {
    let mut snapshots = Vec::with_capacity(ordered_ids.len());
    let book_query = if include_merged {
        "SELECT b.id, b.title, b.author, b.format, b.content_kind,
                b.progress, b.current_chapter, b.shelf_group, b.tags_json,
                (SELECT COUNT(*) FROM chapters c WHERE c.book_id = b.id),
                (SELECT c.state FROM book_covers c WHERE c.book_id = b.id),
                (SELECT c.source_kind FROM book_covers c WHERE c.book_id = b.id),
                (SELECT c.cache_key FROM book_covers c WHERE c.book_id = b.id),
                (SELECT s.state FROM image_sequences s WHERE s.book_id = b.id),
                (SELECT s.root_id FROM image_sequences s WHERE s.book_id = b.id),
                (SELECT s.page_count FROM image_sequences s WHERE s.book_id = b.id),
                b.updated_at
         FROM books b
         WHERE b.id = ?1"
    } else {
        "SELECT b.id, b.title, b.author, b.format, b.content_kind,
                b.progress, b.current_chapter, b.shelf_group, b.tags_json,
                (SELECT COUNT(*) FROM chapters c WHERE c.book_id = b.id),
                (SELECT c.state FROM book_covers c WHERE c.book_id = b.id),
                (SELECT c.source_kind FROM book_covers c WHERE c.book_id = b.id),
                (SELECT c.cache_key FROM book_covers c WHERE c.book_id = b.id),
                (SELECT s.state FROM image_sequences s WHERE s.book_id = b.id),
                (SELECT s.root_id FROM image_sequences s WHERE s.book_id = b.id),
                (SELECT s.page_count FROM image_sequences s WHERE s.book_id = b.id),
                b.updated_at
         FROM books b
         WHERE b.id = ?1 AND b.lifecycle_state = 'active'"
    };
    for book_id in ordered_ids {
        let mut snapshot = connection
            .query_row(book_query, params![book_id], |row| {
                let tags_json: String = row.get(8)?;
                Ok(MergeBookSnapshot {
                    book: BookMergeBookPreview {
                        id: row.get(0)?,
                        title: row.get(1)?,
                        author: row.get(2)?,
                        format: row.get(3)?,
                        content_kind: row.get(4)?,
                        progress: row.get(5)?,
                        current_chapter: row.get(6)?,
                        shelf_group: row.get(7)?,
                        tags: serde_json::from_str(&tags_json).unwrap_or_default(),
                        chapter_count: row.get(9)?,
                        cover_state: row.get(10)?,
                        image_sequence_state: row.get(13)?,
                        image_sequence_root_id: row.get(14)?,
                        image_sequence_page_count: row.get(15)?,
                    },
                    cover_source_kind: row.get(11)?,
                    cover_cache_key: row.get(12)?,
                    updated_at: row.get(16)?,
                    chapters: Vec::new(),
                })
            })
            .optional()?
            .ok_or(DbError::NotFound)?;

        let mut statement = connection.prepare(
            "SELECT id, title, content
             FROM chapters
             WHERE book_id = ?1
             ORDER BY chapter_index, id",
        )?;
        snapshot.chapters = statement
            .query_map(params![snapshot.book.id.clone()], |row| {
                let id: String = row.get(0)?;
                let title: String = row.get(1)?;
                let content: String = row.get(2)?;
                Ok(MergeChapterSnapshot {
                    id,
                    title,
                    digest: merge_text_digest(&content),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        snapshots.push(snapshot);
    }
    Ok(snapshots)
}

fn load_reading_state_snapshot(
    connection: &Connection,
    book_id: &str,
) -> Result<Option<MergeReadingStateSnapshot>, DbError> {
    connection
        .query_row(
            "SELECT position, read_state
             FROM book_reading_state
             WHERE book_id = ?1",
            params![book_id],
            |row| {
                Ok(MergeReadingStateSnapshot {
                    position: row.get(0)?,
                    read_state: row.get(1)?,
                })
            },
        )
        .optional()
        .map_err(DbError::from)
}

fn merge_snapshot_matches(current: &MergeBookSnapshot, expected: &MergeBookSnapshot) -> bool {
    current.book.id == expected.book.id
        && current.book.title == expected.book.title
        && current.book.author == expected.book.author
        && current.book.format == expected.book.format
        && current.book.content_kind == expected.book.content_kind
        && current.book.shelf_group == expected.book.shelf_group
        && current.book.tags == expected.book.tags
        && current.book.chapter_count == expected.book.chapter_count
        && (current.book.progress - expected.book.progress).abs() <= 1e-9
        && current.book.current_chapter == expected.book.current_chapter
        && current.book.cover_state == expected.book.cover_state
        && current.book.image_sequence_state == expected.book.image_sequence_state
        && current.book.image_sequence_root_id == expected.book.image_sequence_root_id
        && current.book.image_sequence_page_count == expected.book.image_sequence_page_count
        && current.cover_source_kind == expected.cover_source_kind
        && current.cover_cache_key == expected.cover_cache_key
        && current.chapters.len() == expected.chapters.len()
        && current
            .chapters
            .iter()
            .zip(expected.chapters.iter())
            .all(|(left, right)| {
                left.id == right.id && left.title == right.title && left.digest == right.digest
            })
}

fn parse_stored_merge_snapshot(value: &str) -> Result<MergeBookSnapshot, DbError> {
    let root: serde_json::Value = serde_json::from_str(value)
        .map_err(|error| DbError::InvalidBookMetadata(format!("书籍快照 JSON 无效：{error}")))?;
    let snapshot = root
        .get("merge_snapshot")
        .cloned()
        .ok_or_else(|| DbError::InvalidBookMetadata("合并记录缺少 d3 快照".to_string()))?;
    serde_json::from_value(snapshot)
        .map_err(|error| DbError::InvalidBookMetadata(format!("书籍快照格式无效：{error}")))
}

fn load_book_alias_target(
    connection: &Connection,
    book_id: &str,
) -> Result<Option<String>, DbError> {
    connection
        .query_row(
            "SELECT canonical_book_id FROM book_aliases WHERE alias_book_id = ?1",
            params![book_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(DbError::from)
}

fn resolve_book_alias_with_connection(
    connection: &Connection,
    book_id: &str,
) -> Result<BookAliasResolution, DbError> {
    let requested_book_id = book_id.trim();
    if requested_book_id.is_empty() {
        return Err(DbError::InvalidBookMetadata("书籍 ID 不能为空".to_string()));
    }
    let Some(canonical_book_id) = load_book_alias_target(connection, requested_book_id)? else {
        return Ok(BookAliasResolution {
            requested_book_id: requested_book_id.to_string(),
            canonical_book_id: requested_book_id.to_string(),
            redirected: false,
            hops: 0,
        });
    };
    if canonical_book_id == requested_book_id {
        return Err(DbError::InvalidBookMetadata(
            "检测到书籍别名环路，已拒绝解析".to_string(),
        ));
    }
    if load_book_alias_target(connection, &canonical_book_id)?.is_some() {
        return Err(DbError::InvalidBookMetadata(
            "检测到多跳书籍别名或别名环路，已拒绝解析".to_string(),
        ));
    }
    Ok(BookAliasResolution {
        requested_book_id: requested_book_id.to_string(),
        canonical_book_id,
        redirected: true,
        hops: 1,
    })
}

fn ensure_book_active(connection: &Connection, book_id: &str) -> Result<(), DbError> {
    let lifecycle_state: Option<String> = connection
        .query_row(
            "SELECT lifecycle_state FROM books WHERE id = ?1",
            params![book_id],
            |row| row.get(0),
        )
        .optional()?;
    match lifecycle_state.as_deref() {
        Some("active") => Ok(()),
        Some("merged") => Err(DbError::InvalidBookMetadata(
            "书籍已合并，不能直接修改；请先选择当前书籍或撤销合并".to_string(),
        )),
        Some(_) => Err(DbError::InvalidBookMetadata(
            "书籍生命周期状态无效".to_string(),
        )),
        None => Err(DbError::NotFound),
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
        (
            14_i64,
            include_str!("../migrations/0014_book_shelf_metadata.sql"),
        ),
        (15_i64, include_str!("../migrations/0015_book_covers.sql")),
        (16_i64, include_str!("../migrations/0016_book_merge.sql")),
        (17_i64, include_str!("../migrations/0017_reading_state.sql")),
        (
            18_i64,
            include_str!("../migrations/0018_book_merge_undo.sql"),
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

fn book_cover_from_row(row: &Row<'_>) -> rusqlite::Result<BookCoverSummary> {
    Ok(BookCoverSummary {
        book_id: row.get(0)?,
        source_kind: row.get(1)?,
        source_value: row.get(2)?,
        source_fingerprint: row.get(3)?,
        cache_key: row.get(4)?,
        state: row.get(5)?,
        mime: row.get(6)?,
        width: row.get(7)?,
        height: row.get(8)?,
        byte_size: row.get(9)?,
        fetched_at: row.get(10)?,
        last_error: row.get(11)?,
        updated_at: row.get(12)?,
    })
}

fn normalize_merge_text(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn merge_text_digest(value: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(value.as_bytes());
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn merge_preview_fingerprint(snapshots: &[MergeBookSnapshot]) -> String {
    let mut hasher = Sha256::new();
    for snapshot in snapshots {
        merge_hash_part(&mut hasher, &snapshot.book.id);
        merge_hash_part(&mut hasher, &snapshot.book.title);
        merge_hash_part(&mut hasher, snapshot.book.author.as_deref().unwrap_or(""));
        merge_hash_part(&mut hasher, &snapshot.book.format);
        merge_hash_part(&mut hasher, &snapshot.book.content_kind);
        merge_hash_part(&mut hasher, &snapshot.book.shelf_group);
        merge_hash_part(&mut hasher, &snapshot.book.tags.join("\u{1f}"));
        merge_hash_part(&mut hasher, &format!("{:.6}", snapshot.book.progress));
        merge_hash_part(&mut hasher, &snapshot.book.current_chapter.to_string());
        merge_hash_part(
            &mut hasher,
            snapshot.book.cover_state.as_deref().unwrap_or(""),
        );
        merge_hash_part(
            &mut hasher,
            snapshot.cover_source_kind.as_deref().unwrap_or(""),
        );
        merge_hash_part(
            &mut hasher,
            snapshot.cover_cache_key.as_deref().unwrap_or(""),
        );
        merge_hash_part(
            &mut hasher,
            snapshot.book.image_sequence_state.as_deref().unwrap_or(""),
        );
        merge_hash_part(
            &mut hasher,
            snapshot
                .book
                .image_sequence_root_id
                .as_deref()
                .unwrap_or(""),
        );
        merge_hash_part(
            &mut hasher,
            &snapshot
                .book
                .image_sequence_page_count
                .map(|count| count.to_string())
                .unwrap_or_default(),
        );
        merge_hash_part(&mut hasher, &snapshot.updated_at);
        for chapter in &snapshot.chapters {
            merge_hash_part(&mut hasher, &chapter.id);
            merge_hash_part(&mut hasher, &chapter.title);
            merge_hash_part(&mut hasher, &chapter.digest);
        }
    }
    let digest = hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!("merge-v1-{digest}")
}

fn merge_hash_part(hasher: &mut Sha256, value: &str) {
    hasher.update((value.len() as u64).to_le_bytes());
    hasher.update(value.as_bytes());
}

fn book_from_row(row: &Row<'_>) -> rusqlite::Result<BookSummary> {
    let tags_json: String = row.get(8)?;
    Ok(BookSummary {
        id: row.get(0)?,
        title: row.get(1)?,
        author: row.get(2)?,
        format: row.get(3)?,
        content_kind: row.get(4)?,
        cover_path: row.get(5)?,
        cover_state: row.get(6)?,
        shelf_group: row.get(7)?,
        tags: serde_json::from_str(&tags_json).unwrap_or_default(),
        custom_order: row.get(9)?,
        chapter_count: row.get(10)?,
        current_chapter: row.get(11)?,
        progress: row.get(12)?,
        updated_at: row.get(13)?,
        image_sequence_state: row.get(14)?,
        image_sequence_missing_pages: row.get(15)?,
        image_sequence_stale_pages: row.get(16)?,
    })
}

fn normalize_book_tags(tags: &[String]) -> Result<Vec<String>, DbError> {
    let mut normalized = Vec::new();
    let mut seen = HashSet::new();
    for tag in tags {
        let tag = tag.trim();
        if tag.is_empty() {
            continue;
        }
        if tag.len() > 64 {
            return Err(DbError::InvalidBookMetadata(
                "单个标签不能超过 64 字节".to_string(),
            ));
        }
        if seen.insert(tag.to_lowercase()) {
            normalized.push(tag.to_string());
        }
        if normalized.len() > 32 {
            return Err(DbError::InvalidBookMetadata(
                "标签数量不能超过 32 个".to_string(),
            ));
        }
    }
    Ok(normalized)
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
    fn persists_reading_position_and_explicit_read_state() {
        let directory = std::env::temp_dir().join(format!(
            "open-reader-reading-state-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        let database = Database::open(&directory).expect("database should open");
        {
            let connection = database.connection.lock().expect("database lock");
            connection
                .execute(
                    "INSERT INTO books (id, title, format) VALUES (?1, ?2, ?3)",
                    params!["reading-book", "Reading state", "txt"],
                )
                .expect("book should insert");
            connection
                .execute(
                    "INSERT INTO chapters (id, book_id, chapter_index, title, content)
                     VALUES (?1, ?2, 0, ?3, ?4)",
                    params!["reading-chapter", "reading-book", "第一章", "正文"],
                )
                .expect("chapter should insert");
        }

        database
            .save_progress(
                "reading-book",
                "reading-chapter",
                0,
                0.64,
                812.5,
                Some("reading"),
            )
            .expect("progress should save");
        let detail = database
            .get_book_detail("reading-book")
            .expect("book detail should load");
        assert!((detail.reading_state.position - 812.5).abs() < f64::EPSILON);
        assert_eq!(detail.reading_state.read_state, "reading");
        assert!((detail.book.progress - 0.64).abs() < f64::EPSILON);

        database
            .save_progress(
                "reading-book",
                "reading-chapter",
                0,
                1.0,
                1600.0,
                Some("finished"),
            )
            .expect("finished state should save");
        let finished = database
            .get_book_detail("reading-book")
            .expect("finished detail should load");
        assert_eq!(finished.reading_state.read_state, "finished");
        assert_eq!(finished.book.progress, 1.0);
    }

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
    #[test]
    fn prunes_source_snapshots_to_retention_limit() {
        let directory = std::env::temp_dir().join(format!(
            "open-reader-source-snapshot-retention-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        let database = Database::open(&directory).expect("database should open");
        for index in 0..(SOURCE_SNAPSHOT_RETENTION_COUNT as usize + 3) {
            database
                .create_source_snapshot(
                    &format!("snapshot-{index}"),
                    &format!(r#"{{"version":1,"index":{index}}}"#),
                    1,
                )
                .expect("snapshot should save");
        }
        let snapshots = database
            .list_source_snapshots()
            .expect("snapshots should list");
        assert_eq!(
            snapshots.len() as i64,
            SOURCE_SNAPSHOT_RETENTION_COUNT,
            "older snapshots should be pruned after creation"
        );

        let oversized = "x".repeat(MAX_SOURCE_SNAPSHOT_BYTES + 1);
        let error = database
            .create_source_snapshot("oversized", &oversized, 0)
            .expect_err("oversized snapshot should be rejected");
        assert!(error.to_string().contains("快照内容不能超过"));

        drop(database);
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn persists_book_shelf_metadata_and_filters() {
        let directory = std::env::temp_dir().join(format!(
            "open-reader-book-shelf-metadata-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        let database = Database::open(&directory).expect("database should open");
        {
            let connection = database.connection.lock().expect("database lock");
            connection
                .execute(
                    "INSERT INTO books (id, title, author, path, format)
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                    params!["book-alpha", "阿尔法", "作者甲", "alpha.txt", "txt"],
                )
                .expect("alpha book should insert");
            connection
                .execute(
                    "INSERT INTO books (id, title, author, path, format)
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                    params!["book-beta", "贝塔", "作者乙", "beta.txt", "txt"],
                )
                .expect("beta book should insert");
        }

        let alpha = database
            .update_book_metadata(BookMetadataWrite {
                book_id: "book-alpha".to_string(),
                shelf_group: "收藏".to_string(),
                tags: vec!["奇幻".to_string(), "奇幻".to_string(), "已读".to_string()],
                cover_path: Some("C:/covers/alpha.png".to_string()),
                custom_order: 5,
            })
            .expect("alpha metadata should save");
        assert_eq!(alpha.shelf_group, "收藏");
        assert_eq!(alpha.tags, vec!["奇幻".to_string(), "已读".to_string()]);
        assert_eq!(alpha.cover_path.as_deref(), Some("C:/covers/alpha.png"));

        database
            .update_book_metadata(BookMetadataWrite {
                book_id: "book-beta".to_string(),
                shelf_group: "待读".to_string(),
                tags: vec!["科幻".to_string()],
                cover_path: None,
                custom_order: 1,
            })
            .expect("beta metadata should save");

        let filtered = database
            .list_books_with_options(&BookListOptions {
                group: Some("收藏".to_string()),
                query: Some("奇幻".to_string()),
                sort: Some("title".to_string()),
                descending: Some(false),
            })
            .expect("filtered books should list");
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].id, "book-alpha");

        let sorted = database
            .list_books_with_options(&BookListOptions {
                group: None,
                query: None,
                sort: Some("custom_order".to_string()),
                descending: Some(false),
            })
            .expect("sorted books should list");
        assert_eq!(
            sorted
                .iter()
                .map(|book| book.id.as_str())
                .collect::<Vec<_>>(),
            vec!["book-beta", "book-alpha"]
        );

        let invalid = database.update_book_metadata(BookMetadataWrite {
            book_id: "book-alpha".to_string(),
            shelf_group: "x".repeat(129),
            tags: Vec::new(),
            cover_path: None,
            custom_order: 0,
        });
        assert!(invalid.is_err());

        drop(database);
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn batch_updates_book_metadata_transactionally() {
        let directory = std::env::temp_dir().join(format!(
            "open-reader-book-batch-metadata-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        let database = Database::open(&directory).expect("database should open");
        {
            let connection = database.connection.lock().expect("database lock");
            for (id, title) in [("book-alpha", "阿尔法"), ("book-beta", "贝塔")] {
                connection
                    .execute(
                        "INSERT INTO books (id, title, path, format)
                         VALUES (?1, ?2, ?3, 'txt')",
                        params![id, title, format!("{id}.txt")],
                    )
                    .expect("fixture book should insert");
            }
        }

        let updated = database
            .update_books_metadata(BookMetadataBatchWrite {
                book_ids: vec![
                    "book-alpha".to_string(),
                    "book-beta".to_string(),
                    "book-alpha".to_string(),
                ],
                shelf_group: Some("待读".to_string()),
                tags: Some(vec!["精选".to_string(), "精选".to_string()]),
            })
            .expect("batch metadata should save");
        assert_eq!(updated.len(), 2);
        assert!(updated.iter().all(|book| book.shelf_group == "待读"));
        assert!(updated
            .iter()
            .all(|book| book.tags == vec!["精选".to_string()]));

        let preserved = database
            .update_books_metadata(BookMetadataBatchWrite {
                book_ids: vec!["book-alpha".to_string()],
                shelf_group: Some("收藏".to_string()),
                tags: None,
            })
            .expect("partial batch metadata should save");
        assert_eq!(preserved[0].shelf_group, "收藏");
        assert_eq!(preserved[0].tags, vec!["精选".to_string()]);

        let cleared = database
            .update_books_metadata(BookMetadataBatchWrite {
                book_ids: vec!["book-alpha".to_string()],
                shelf_group: Some(String::new()),
                tags: Some(Vec::new()),
            })
            .expect("batch metadata should clear");
        assert!(cleared[0].shelf_group.is_empty());
        assert!(cleared[0].tags.is_empty());

        let invalid = database.update_books_metadata(BookMetadataBatchWrite {
            book_ids: vec!["book-alpha".to_string(), "missing".to_string()],
            shelf_group: Some("回滚".to_string()),
            tags: None,
        });
        assert!(invalid.is_err());
        let unchanged = database
            .get_book_summary("book-alpha")
            .expect("book should remain readable");
        assert!(unchanged.shelf_group.is_empty());

        drop(database);
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn persists_book_cover_source_without_network() {
        let directory = std::env::temp_dir().join(format!(
            "open-reader-book-cover-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        let database = Database::open(&directory).expect("database should open");
        {
            let connection = database.connection.lock().expect("database lock");
            connection
                .execute(
                    "INSERT INTO books (id, title, path, format)
                     VALUES (?1, ?2, ?3, 'txt')",
                    params!["cover-book", "封面测试", "cover.txt"],
                )
                .expect("book should insert");
        }

        let saved = database
            .save_book_cover(BookCoverWrite {
                book_id: "cover-book".to_string(),
                source_kind: CoverSourceKind::LocalPath,
                source_value: r"C:\书架\封面.png".to_string(),
                source_fingerprint: Some("size=12;mtime=34".to_string()),
            })
            .expect("local cover source should save");
        assert_eq!(saved.source_kind, "local_path");
        assert_eq!(saved.source_value, "C:/书架/封面.png");
        assert_eq!(saved.state, "stale");
        assert!(saved.cache_key.starts_with("cover-v1-"));

        let loaded = database
            .get_book_cover("cover-book")
            .expect("cover should load")
            .expect("cover should exist");
        assert_eq!(loaded.source_fingerprint, "size=12;mtime=34");

        let cleared = database
            .save_book_cover(BookCoverWrite {
                book_id: "cover-book".to_string(),
                source_kind: CoverSourceKind::None,
                source_value: String::new(),
                source_fingerprint: None,
            })
            .expect("cover source should clear");
        assert_eq!(cleared.source_kind, "none");
        assert_eq!(cleared.source_value, "");
        assert_eq!(cleared.state, "missing");

        let invalid = database.save_book_cover(BookCoverWrite {
            book_id: "cover-book".to_string(),
            source_kind: CoverSourceKind::RemoteUrl,
            source_value: "http://example.com/cover.png".to_string(),
            source_fingerprint: None,
        });
        assert!(invalid.is_err());
        assert_eq!(
            database
                .get_book_cover("cover-book")
                .expect("cover should remain readable")
                .expect("cover should remain")
                .state,
            "missing"
        );

        drop(database);
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn previews_book_merge_without_mutating_library() {
        let directory = std::env::temp_dir().join(format!(
            "open-reader-book-merge-preview-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        let database = Database::open(&directory).expect("database should open");
        {
            let connection = database.connection.lock().expect("database lock");
            for (id, title, progress) in [
                ("merge-canonical", "合并书", 0.4_f64),
                ("merge-source", " 合并书 ", 0.8_f64),
            ] {
                connection
                    .execute(
                        "INSERT INTO books (id, title, author, path, format, progress)
                         VALUES (?1, ?2, '作者', ?3, 'txt', ?4)",
                        params![id, title, format!("{id}.txt"), progress],
                    )
                    .expect("merge book should insert");
            }
            for (id, book_id, index, title, content) in [
                ("merge-c1", "merge-canonical", 0_i64, "第一章", "相同正文"),
                ("merge-s1", "merge-source", 0_i64, "第一章", "相同正文"),
                ("merge-s2", "merge-source", 1_i64, "第一章", "不同正文"),
                ("merge-s3", "merge-source", 2_i64, "第二章", "新增正文"),
            ] {
                connection
                    .execute(
                        "INSERT INTO chapters
                         (id, book_id, chapter_index, title, content, content_format)
                         VALUES (?1, ?2, ?3, ?4, ?5, 'text')",
                        params![id, book_id, index, title, content],
                    )
                    .expect("merge chapter should insert");
            }
        }

        let preview = database
            .preview_book_merge(BookMergePreviewRequest {
                book_ids: vec!["merge-source".to_string(), "merge-canonical".to_string()],
                canonical_book_id: "merge-canonical".to_string(),
            })
            .expect("merge preview should load");
        assert_eq!(preview.canonical_book_id, "merge-canonical");
        assert_eq!(preview.archived_book_ids, vec!["merge-source".to_string()]);
        assert_eq!(preview.books.len(), 2);
        assert_eq!(preview.identical_chapter_count, 1);
        assert_eq!(preview.append_candidates.len(), 1);
        assert_eq!(preview.chapter_conflicts.len(), 1);
        assert!(preview
            .blocked_reasons
            .iter()
            .any(|reason| reason.contains("章节正文冲突")));
        assert_eq!(preview.expires_at - preview.created_at, 5 * 60);
        assert!(preview.input_fingerprint.starts_with("merge-v1-"));
        assert!(preview.preview_id.starts_with("merge-preview-"));
        assert_eq!(
            database
                .get_book_summary("merge-canonical")
                .expect("canonical should remain readable")
                .progress,
            0.4
        );
        assert!(database
            .preview_book_merge(BookMergePreviewRequest {
                book_ids: vec!["merge-canonical".to_string()],
                canonical_book_id: "merge-canonical".to_string(),
            })
            .is_err());
        assert!(database
            .preview_book_merge(BookMergePreviewRequest {
                book_ids: vec!["merge-canonical".to_string(), "missing-book".to_string()],
                canonical_book_id: "merge-canonical".to_string(),
            })
            .is_err());

        let validated = database
            .revalidate_book_merge_preview(BookMergePreviewRevalidateRequest {
                preview: BookMergePreviewRequest {
                    book_ids: vec!["merge-source".to_string(), "merge-canonical".to_string()],
                    canonical_book_id: "merge-canonical".to_string(),
                },
                preview_id: preview.preview_id.clone(),
                created_at: preview.created_at,
                expires_at: preview.expires_at,
                input_fingerprint: preview.input_fingerprint.clone(),
            })
            .expect("unchanged preview should revalidate");
        assert_eq!(validated.preview_id, preview.preview_id);
        assert_eq!(validated.created_at, preview.created_at);
        assert_eq!(validated.expires_at, preview.expires_at);
        assert_eq!(validated.input_fingerprint, preview.input_fingerprint);

        {
            let connection = database.connection.lock().expect("database lock");
            connection
                .execute(
                    "UPDATE books SET progress = 0.9 WHERE id = ?1",
                    params!["merge-source"],
                )
                .expect("source mutation should succeed");
        }
        assert!(database
            .revalidate_book_merge_preview(BookMergePreviewRevalidateRequest {
                preview: BookMergePreviewRequest {
                    book_ids: vec!["merge-source".to_string(), "merge-canonical".to_string()],
                    canonical_book_id: "merge-canonical".to_string(),
                },
                preview_id: preview.preview_id.clone(),
                created_at: preview.created_at,
                expires_at: preview.expires_at,
                input_fingerprint: preview.input_fingerprint.clone(),
            })
            .is_err());

        assert!(database
            .revalidate_book_merge_preview(BookMergePreviewRevalidateRequest {
                preview: BookMergePreviewRequest {
                    book_ids: vec!["merge-source".to_string(), "merge-canonical".to_string()],
                    canonical_book_id: "merge-canonical".to_string(),
                },
                preview_id: preview.preview_id,
                created_at: 1,
                expires_at: 2,
                input_fingerprint: "merge-v1-expired".to_string(),
            })
            .is_err());

        drop(database);
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn commits_pure_text_book_merge_and_hides_archived_source() {
        let directory = std::env::temp_dir().join(format!(
            "open-reader-book-merge-commit-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        let database = Database::open(&directory).expect("database should open");
        {
            let connection = database.connection.lock().expect("database lock");
            for (id, title, progress) in [
                ("commit-canonical", "事务合并书", 0.2_f64),
                ("commit-source", "事务合并书", 0.75_f64),
            ] {
                connection
                    .execute(
                        "INSERT INTO books (id, title, author, path, format, progress)
                         VALUES (?1, ?2, '作者', ?3, 'txt', ?4)",
                        params![id, title, format!("{id}.txt"), progress],
                    )
                    .expect("merge book should insert");
            }
            connection
                .execute(
                    "INSERT INTO chapters
                     (id, book_id, chapter_index, title, content, content_format)
                     VALUES ('commit-c1', 'commit-canonical', 0, '第一章', '正文', 'text')",
                    [],
                )
                .expect("canonical chapter should insert");
            connection
                .execute(
                    "INSERT INTO chapters
                     (id, book_id, chapter_index, title, content, content_format)
                     VALUES ('commit-s1', 'commit-source', 0, '第一章', '正文', 'text')",
                    [],
                )
                .expect("identical source chapter should insert");
            connection
                .execute(
                    "INSERT INTO chapters
                     (id, book_id, chapter_index, title, content, content_format)
                     VALUES ('commit-s2', 'commit-source', 1, '第二章', '新增正文', 'text')",
                    [],
                )
                .expect("append source chapter should insert");
            connection
                .execute(
                    "INSERT INTO book_reading_state (book_id, position, read_state)
                     VALUES ('commit-source', 0.4, 'reading')",
                    [],
                )
                .expect("source reading state should insert");
        }

        let preview = database
            .preview_book_merge(BookMergePreviewRequest {
                book_ids: vec!["commit-source".to_string(), "commit-canonical".to_string()],
                canonical_book_id: "commit-canonical".to_string(),
            })
            .expect("merge preview should load");
        assert!(preview.conflicts.is_empty());
        assert!(preview.blocked_reasons.is_empty());
        assert_eq!(preview.append_candidates.len(), 1);

        let request = BookMergeCommitRequest {
            preview: BookMergePreviewRequest {
                book_ids: vec!["commit-source".to_string(), "commit-canonical".to_string()],
                canonical_book_id: "commit-canonical".to_string(),
            },
            preview_id: preview.preview_id.clone(),
            created_at: preview.created_at,
            expires_at: preview.expires_at,
            input_fingerprint: preview.input_fingerprint.clone(),
            progress_book_id: "commit-source".to_string(),
            final_shelf_group: Some("已合并".to_string()),
            final_tags: Some(vec!["测试".to_string(), "测试".to_string()]),
        };
        let result = database
            .commit_book_merge(request.clone())
            .expect("merge commit should succeed");
        assert!(result.operation_id.starts_with("merge-operation-"));
        assert_eq!(result.canonical_book_id, "commit-canonical");
        assert_eq!(result.archived_book_ids, vec!["commit-source".to_string()]);
        assert_eq!(result.appended_chapter_ids.len(), 1);
        assert!(database.get_book_summary("commit-source").is_err());
        assert_eq!(database.list_books().expect("shelf should load").len(), 1);
        let canonical = database
            .get_book_summary("commit-canonical")
            .expect("canonical should remain active");
        assert_eq!(canonical.chapter_count, 2);
        assert_eq!(canonical.progress, 0.75);
        assert_eq!(canonical.shelf_group, "已合并");
        assert_eq!(canonical.tags, vec!["测试".to_string()]);
        assert!(database.rename_book("commit-source", "不应修改").is_err());

        let connection = database.connection.lock().expect("database lock");
        let lifecycle: String = connection
            .query_row(
                "SELECT lifecycle_state FROM books WHERE id = 'commit-source'",
                [],
                |row| row.get(0),
            )
            .expect("archived lifecycle should read");
        assert_eq!(lifecycle, "merged");
        let item_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM book_merge_items WHERE operation_id = ?1",
                params![result.operation_id],
                |row| row.get(0),
            )
            .expect("merge item should exist");
        assert_eq!(item_count, 1);
        drop(connection);

        assert!(database.commit_book_merge(request).is_err());
        drop(database);
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn rejects_changed_merge_preview_and_rolls_back_alias_conflict() {
        let directory = std::env::temp_dir().join(format!(
            "open-reader-book-merge-rollback-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        let database = Database::open(&directory).expect("database should open");
        {
            let connection = database.connection.lock().expect("database lock");
            for id in ["rollback-canonical", "rollback-source"] {
                connection
                    .execute(
                        "INSERT INTO books (id, title, author, path, format)
                         VALUES (?1, '回滚合并书', '作者', ?2, 'txt')",
                        params![id, format!("{id}.txt")],
                    )
                    .expect("rollback book should insert");
            }
            connection
                .execute(
                    "INSERT INTO chapters
                     (id, book_id, chapter_index, title, content, content_format)
                     VALUES ('rollback-c1', 'rollback-canonical', 0, '第一章', '正文', 'text')",
                    [],
                )
                .expect("rollback canonical chapter should insert");
            connection
                .execute(
                    "INSERT INTO chapters
                     (id, book_id, chapter_index, title, content, content_format)
                     VALUES ('rollback-s1', 'rollback-source', 0, '第一章', '正文', 'text')",
                    [],
                )
                .expect("rollback source chapter should insert");
        }
        let preview = database
            .preview_book_merge(BookMergePreviewRequest {
                book_ids: vec![
                    "rollback-source".to_string(),
                    "rollback-canonical".to_string(),
                ],
                canonical_book_id: "rollback-canonical".to_string(),
            })
            .expect("rollback preview should load");
        let request = BookMergeCommitRequest {
            preview: BookMergePreviewRequest {
                book_ids: vec![
                    "rollback-source".to_string(),
                    "rollback-canonical".to_string(),
                ],
                canonical_book_id: "rollback-canonical".to_string(),
            },
            preview_id: preview.preview_id.clone(),
            created_at: preview.created_at,
            expires_at: preview.expires_at,
            input_fingerprint: preview.input_fingerprint.clone(),
            progress_book_id: "rollback-canonical".to_string(),
            final_shelf_group: None,
            final_tags: None,
        };

        {
            let connection = database.connection.lock().expect("database lock");
            connection
                .execute(
                    "UPDATE books SET progress = 0.9 WHERE id = 'rollback-source'",
                    [],
                )
                .expect("source mutation should succeed");
        }
        assert!(database.commit_book_merge(request.clone()).is_err());

        let fresh_preview = database
            .preview_book_merge(request.preview.clone())
            .expect("fresh preview should load");
        let request = BookMergeCommitRequest {
            preview: request.preview,
            preview_id: fresh_preview.preview_id.clone(),
            created_at: fresh_preview.created_at,
            expires_at: fresh_preview.expires_at,
            input_fingerprint: fresh_preview.input_fingerprint.clone(),
            progress_book_id: "rollback-canonical".to_string(),
            final_shelf_group: None,
            final_tags: None,
        };

        {
            let connection = database.connection.lock().expect("database lock");
            connection
                .execute(
                    "INSERT INTO book_merge_operations
                     (id, preview_id, canonical_book_id, status, plan_json, undo_until)
                     VALUES ('merge-operation-conflict', 'merge-preview-conflict',
                             'rollback-canonical', 'committed', '{}', datetime('now', '+7 days'))",
                    [],
                )
                .expect("conflicting operation should insert");
            connection
                .execute(
                    "INSERT INTO book_aliases (alias_book_id, canonical_book_id, operation_id)
                     VALUES ('rollback-source', 'rollback-canonical', 'merge-operation-conflict')",
                    [],
                )
                .expect("conflicting alias should insert");
        }
        assert!(database.commit_book_merge(request).is_err());

        let connection = database.connection.lock().expect("database lock");
        let lifecycle: String = connection
            .query_row(
                "SELECT lifecycle_state FROM books WHERE id = 'rollback-source'",
                [],
                |row| row.get(0),
            )
            .expect("source lifecycle should remain active");
        assert_eq!(lifecycle, "active");
        let appended_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM chapters WHERE book_id = 'rollback-canonical'",
                [],
                |row| row.get(0),
            )
            .expect("canonical chapter count should read");
        assert_eq!(appended_count, 1);
        drop(connection);

        drop(database);
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn undoes_book_merge_and_resolves_old_id() {
        let directory = std::env::temp_dir().join(format!(
            "open-reader-book-merge-undo-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        let database = Database::open(&directory).expect("database should open");
        {
            let connection = database.connection.lock().expect("database lock");
            connection
                .execute(
                    "INSERT INTO books (id, title, author, path, format, progress, shelf_group)
                     VALUES ('undo-canonical', '可撤销合并', '作者', 'undo-canonical.txt', 'txt', 0.2, '原组')",
                    [],
                )
                .expect("canonical should insert");
            connection
                .execute(
                    "INSERT INTO books (id, title, author, path, format, progress)
                     VALUES ('undo-source', '可撤销合并', '作者', 'undo-source.txt', 'txt', 0.75)",
                    [],
                )
                .expect("source should insert");
            connection
                .execute(
                    "INSERT INTO chapters
                     (id, book_id, chapter_index, title, content, content_format)
                     VALUES ('undo-c1', 'undo-canonical', 0, '第一章', '原正文', 'text')",
                    [],
                )
                .expect("canonical chapter should insert");
            connection
                .execute(
                    "INSERT INTO chapters
                     (id, book_id, chapter_index, title, content, content_format)
                     VALUES ('undo-s1', 'undo-source', 0, '第二章', '新增正文', 'text')",
                    [],
                )
                .expect("source chapter should insert");
            connection
                .execute(
                    "INSERT INTO book_reading_state (book_id, position, read_state)
                     VALUES ('undo-canonical', 0.15, 'reading')",
                    [],
                )
                .expect("canonical reading state should insert");
            connection
                .execute(
                    "INSERT INTO book_reading_state (book_id, position, read_state)
                     VALUES ('undo-source', 0.65, 'reading')",
                    [],
                )
                .expect("source reading state should insert");
        }

        let preview = database
            .preview_book_merge(BookMergePreviewRequest {
                book_ids: vec!["undo-source".to_string(), "undo-canonical".to_string()],
                canonical_book_id: "undo-canonical".to_string(),
            })
            .expect("undo preview should load");
        let result = database
            .commit_book_merge(BookMergeCommitRequest {
                preview: BookMergePreviewRequest {
                    book_ids: vec!["undo-source".to_string(), "undo-canonical".to_string()],
                    canonical_book_id: "undo-canonical".to_string(),
                },
                preview_id: preview.preview_id,
                created_at: preview.created_at,
                expires_at: preview.expires_at,
                input_fingerprint: preview.input_fingerprint,
                progress_book_id: "undo-source".to_string(),
                final_shelf_group: Some("合并后".to_string()),
                final_tags: Some(vec!["临时".to_string()]),
            })
            .expect("undo merge should commit");
        assert_eq!(
            database
                .get_book_detail("undo-source")
                .expect("old id should redirect")
                .book
                .id,
            "undo-canonical"
        );
        let undone = database
            .undo_book_merge(BookMergeUndoRequest {
                operation_id: result.operation_id.clone(),
            })
            .expect("merge should undo");
        assert_eq!(undone.canonical_book_id, "undo-canonical");
        assert_eq!(undone.restored_book_ids.len(), 2);
        assert_eq!(undone.removed_chapter_ids.len(), 1);
        assert_eq!(
            database
                .get_book_summary("undo-canonical")
                .expect("canonical should restore")
                .shelf_group,
            "原组"
        );
        assert_eq!(
            database
                .get_book_summary("undo-canonical")
                .expect("canonical should restore")
                .chapter_count,
            1
        );
        assert!(
            database
                .get_book_summary("undo-source")
                .expect("source should restore")
                .id
                == "undo-source"
        );
        assert!(database
            .undo_book_merge(BookMergeUndoRequest {
                operation_id: result.operation_id.clone(),
            })
            .is_err());

        let connection = database.connection.lock().expect("database lock");
        let status: String = connection
            .query_row(
                "SELECT status FROM book_merge_operations WHERE id = ?1",
                params![result.operation_id],
                |row| row.get(0),
            )
            .expect("undo status should read");
        assert_eq!(status, "undone");
        let alias_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM book_aliases WHERE alias_book_id = 'undo-source'",
                [],
                |row| row.get(0),
            )
            .expect("alias count should read");
        assert_eq!(alias_count, 0);
        drop(connection);

        drop(database);
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn rejects_merge_undo_after_external_change_or_expiry() {
        let directory = std::env::temp_dir().join(format!(
            "open-reader-book-merge-undo-conflict-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        let database = Database::open(&directory).expect("database should open");
        {
            let connection = database.connection.lock().expect("database lock");
            for (id, progress) in [
                ("undo-conflict-canonical", 0.2_f64),
                ("undo-conflict-source", 0.7_f64),
            ] {
                connection
                    .execute(
                        "INSERT INTO books (id, title, author, path, format, progress)
                         VALUES (?1, '外部修改检测', '作者', ?2, 'txt', ?3)",
                        params![id, format!("{id}.txt"), progress],
                    )
                    .expect("conflict book should insert");
            }
            for (id, book_id, title) in [
                ("undo-conflict-c1", "undo-conflict-canonical", "第一章"),
                ("undo-conflict-s1", "undo-conflict-source", "第二章"),
            ] {
                connection
                    .execute(
                        "INSERT INTO chapters
                         (id, book_id, chapter_index, title, content, content_format)
                         VALUES (?1, ?2, 0, ?3, '正文', 'text')",
                        params![id, book_id, title],
                    )
                    .expect("conflict chapter should insert");
            }
        }
        let preview = database
            .preview_book_merge(BookMergePreviewRequest {
                book_ids: vec![
                    "undo-conflict-source".to_string(),
                    "undo-conflict-canonical".to_string(),
                ],
                canonical_book_id: "undo-conflict-canonical".to_string(),
            })
            .expect("conflict preview should load");
        let result = database
            .commit_book_merge(BookMergeCommitRequest {
                preview: BookMergePreviewRequest {
                    book_ids: vec![
                        "undo-conflict-source".to_string(),
                        "undo-conflict-canonical".to_string(),
                    ],
                    canonical_book_id: "undo-conflict-canonical".to_string(),
                },
                preview_id: preview.preview_id,
                created_at: preview.created_at,
                expires_at: preview.expires_at,
                input_fingerprint: preview.input_fingerprint,
                progress_book_id: "undo-conflict-canonical".to_string(),
                final_shelf_group: None,
                final_tags: None,
            })
            .expect("conflict merge should commit");
        {
            let connection = database.connection.lock().expect("database lock");
            connection
                .execute(
                    "UPDATE books SET progress = 0.99 WHERE id = 'undo-conflict-canonical'",
                    [],
                )
                .expect("external canonical change should succeed");
        }
        assert!(database
            .undo_book_merge(BookMergeUndoRequest {
                operation_id: result.operation_id.clone(),
            })
            .is_err());
        {
            let connection = database.connection.lock().expect("database lock");
            connection
                .execute(
                    "UPDATE books SET progress = 0.2 WHERE id = 'undo-conflict-canonical'",
                    [],
                )
                .expect("canonical should restore for expiry check");
            connection
                .execute(
                    "UPDATE book_merge_operations
                     SET undo_until = datetime('now', '-1 second')
                     WHERE id = ?1",
                    params![result.operation_id],
                )
                .expect("undo window should expire");
        }
        assert!(database
            .undo_book_merge(BookMergeUndoRequest {
                operation_id: result.operation_id.clone(),
            })
            .is_err());
        let connection = database.connection.lock().expect("database lock");
        let status: String = connection
            .query_row(
                "SELECT status FROM book_merge_operations WHERE id = ?1",
                params![result.operation_id],
                |row| row.get(0),
            )
            .expect("expired status should read");
        assert_eq!(status, "expired");
        drop(connection);
        drop(database);
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn detects_book_alias_cycles_and_multi_hop_redirects() {
        let directory = std::env::temp_dir().join(format!(
            "open-reader-book-alias-cycle-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        let database = Database::open(&directory).expect("database should open");
        {
            let connection = database.connection.lock().expect("database lock");
            for id in ["alias-a", "alias-b", "alias-c"] {
                connection
                    .execute(
                        "INSERT INTO books (id, title, path, format)
                         VALUES (?1, ?1, ?2, 'txt')",
                        params![id, format!("{id}.txt")],
                    )
                    .expect("alias book should insert");
            }
            connection
                .execute(
                    "INSERT INTO book_merge_operations
                     (id, preview_id, canonical_book_id, plan_json, undo_until)
                     VALUES ('merge-operation-alias-a', 'merge-preview-alias-a', 'alias-c', '{}', datetime('now', '+7 days'))",
                    [],
                )
                .expect("alias operation should insert");
            connection
                .execute(
                    "INSERT INTO book_aliases (alias_book_id, canonical_book_id, operation_id)
                     VALUES ('alias-a', 'alias-b', 'merge-operation-alias-a')",
                    [],
                )
                .expect("first alias should insert");
            connection
                .execute(
                    "INSERT INTO book_aliases (alias_book_id, canonical_book_id, operation_id)
                     VALUES ('alias-b', 'alias-c', 'merge-operation-alias-a')",
                    [],
                )
                .expect("second alias should insert");
        }
        assert!(database.resolve_book_alias("alias-a").is_err());
        {
            let connection = database.connection.lock().expect("database lock");
            connection
                .execute(
                    "DELETE FROM book_aliases WHERE alias_book_id = 'alias-b'",
                    [],
                )
                .expect("multi-hop alias should remove");
            connection
                .execute(
                    "INSERT INTO book_aliases (alias_book_id, canonical_book_id, operation_id)
                     VALUES ('alias-b', 'alias-a', 'merge-operation-alias-a')",
                    [],
                )
                .expect("cycle alias should insert");
        }
        assert!(database.resolve_book_alias("alias-a").is_err());
        drop(database);
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn finds_duplicate_books_without_mutating_library() {
        let directory = std::env::temp_dir().join(format!(
            "open-reader-duplicate-books-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        let database = Database::open(&directory).expect("database should open");
        {
            let connection = database.connection.lock().expect("database lock");
            connection
                .execute(
                    "INSERT INTO books (id, title, author, path, format)
                     VALUES (?1, ?2, ?3, ?4, 'txt')",
                    params!["duplicate-a", "同一本书", "作者", "a.txt"],
                )
                .expect("first duplicate should insert");
            connection
                .execute(
                    "INSERT INTO books (id, title, author, path, format)
                     VALUES (?1, ?2, ?3, ?4, 'txt')",
                    params!["duplicate-b", " 同一本书 ", "作者", "b.txt"],
                )
                .expect("second duplicate should insert");
            connection
                .execute(
                    "INSERT INTO books (id, title, author, path, format)
                     VALUES (?1, ?2, ?3, ?4, 'epub')",
                    params!["different-format", "同一本书", "作者", "book.epub"],
                )
                .expect("different format should insert");
        }

        let groups = database
            .find_duplicate_books()
            .expect("duplicate groups should load");
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].books.len(), 2);
        assert!(groups[0].books.iter().any(|book| book.id == "duplicate-a"));
        assert!(groups[0].books.iter().any(|book| book.id == "duplicate-b"));
        assert_eq!(
            database
                .get_book_summary("duplicate-a")
                .expect("book should remain readable")
                .title,
            "同一本书"
        );

        drop(database);
        let _ = fs::remove_dir_all(directory);
    }
}
