mod db;
mod library;
mod source;
mod source_import;

use db::{BookDetail, BookSummary, ChapterContent, Database, SourceCacheStats, SourceSummary};
use library::parse_book_bytes;
use serde::{Deserialize, Serialize};
use source::{
    MultiSourceSearchResult, SourceBookDetail, SourceDefinition, SourceEngine, SourcePreview,
    SourceSearchFailure, SourceValidation,
};
use tauri::Manager;

#[tauri::command]
fn list_books(database: tauri::State<'_, Database>) -> Result<Vec<BookSummary>, String> {
    database.list_books().map_err(|error| error.to_string())
}

#[tauri::command]
fn import_book(
    database: tauri::State<'_, Database>,
    file_name: String,
    bytes: Vec<u8>,
) -> Result<BookSummary, String> {
    const MAX_IMPORT_BYTES: usize = 64 * 1024 * 1024;
    if bytes.len() > MAX_IMPORT_BYTES {
        return Err("文件超过 64 MB 限制".to_string());
    }

    let parsed = parse_book_bytes(&file_name, &bytes).map_err(|error| error.to_string())?;
    database
        .import_book(&file_name, parsed)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn get_book_detail(
    database: tauri::State<'_, Database>,
    book_id: String,
) -> Result<BookDetail, String> {
    database
        .get_book_detail(&book_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn get_chapter_content(
    database: tauri::State<'_, Database>,
    book_id: String,
    chapter_id: String,
) -> Result<ChapterContent, String> {
    database
        .get_chapter_content(&book_id, &chapter_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn save_progress(
    database: tauri::State<'_, Database>,
    book_id: String,
    chapter_id: String,
    current_chapter: i64,
    progress: f64,
) -> Result<(), String> {
    database
        .save_progress(&book_id, &chapter_id, current_chapter, progress)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn validate_book_source(config_json: String) -> SourceValidation {
    source::validate_source_json(&config_json)
}

#[tauri::command]
fn list_sources(database: tauri::State<'_, Database>) -> Result<Vec<SourceSummary>, String> {
    database.list_sources().map_err(|error| error.to_string())
}

#[tauri::command]
fn save_source(
    database: tauri::State<'_, Database>,
    source_id: Option<String>,
    config_json: String,
) -> Result<SourceSummary, String> {
    let validation = source::validate_source_json(&config_json);
    let source = validation
        .source
        .ok_or_else(|| validation.errors.join("；"))?;
    if !validation.valid {
        return Err(validation.errors.join("；"));
    }
    database
        .save_source(source_id.as_deref(), &source.name, &config_json)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn set_source_enabled(
    database: tauri::State<'_, Database>,
    source_id: String,
    enabled: bool,
) -> Result<SourceSummary, String> {
    database
        .set_source_enabled(&source_id, enabled)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn delete_source(database: tauri::State<'_, Database>, source_id: String) -> Result<(), String> {
    database
        .delete_source(&source_id)
        .map_err(|error| error.to_string())
}

#[derive(Debug, Clone, Serialize)]
struct SourceAuditReport {
    source_id: String,
    source_name: String,
    enabled: bool,
    permission_status: String,
    permission_scope: Option<String>,
    reviewed_at: Option<String>,
    hosts: Vec<String>,
    sensitive_headers: Vec<String>,
    errors: Vec<String>,
    warnings: Vec<String>,
    pass: bool,
}

#[tauri::command]
fn audit_sources(database: tauri::State<'_, Database>) -> Result<Vec<SourceAuditReport>, String> {
    let sources = database.list_sources().map_err(|error| error.to_string())?;
    Ok(sources
        .into_iter()
        .map(|summary| {
            let audit = source::audit_source_json(&summary.config_json);
            SourceAuditReport {
                source_id: summary.id,
                source_name: summary.name,
                enabled: summary.enabled,
                permission_status: audit.permission_status,
                permission_scope: audit.permission_scope,
                reviewed_at: audit.reviewed_at,
                hosts: audit.hosts,
                sensitive_headers: audit.sensitive_headers,
                errors: audit.errors,
                warnings: audit.warnings,
                pass: audit.pass,
            }
        })
        .collect())
}

const MAX_SOURCE_BUNDLE_BYTES: usize = 2 * 1024 * 1024;
const MAX_SOURCE_IMPORT_URL_BYTES: usize = 2 * 1024;

#[derive(Debug, Clone, Serialize)]
struct RemoteSourceImportPreview {
    payload: String,
    preview: source_import::ImportPreview,
}

fn persist_imported_sources(
    database: &Database,
    bundle: Vec<source_import::ImportedSource>,
) -> Result<Vec<SourceSummary>, String> {
    let mut imported = Vec::with_capacity(bundle.len());

    for (index, item) in bundle.into_iter().enumerate() {
        let validation = source::validate_source_json(&item.config_json);
        let source = validation.source.ok_or_else(|| {
            format!(
                "第 {} 个书源无法解析：{}",
                index + 1,
                validation.errors.join("；")
            )
        })?;
        if !validation.valid {
            return Err(format!(
                "第 {} 个书源校验失败：{}",
                index + 1,
                validation.errors.join("；")
            ));
        }

        let saved = database
            .save_source(item.id.as_deref(), &source.name, &item.config_json)
            .map_err(|error| error.to_string())?;
        let saved = database
            .set_source_enabled(&saved.id, item.enabled)
            .map_err(|error| error.to_string())?;
        imported.push(saved);
    }

    Ok(imported)
}

fn import_source_payload(database: &Database, payload: &str) -> Result<Vec<SourceSummary>, String> {
    if payload.len() > MAX_SOURCE_BUNDLE_BYTES {
        return Err("书源文件超过 2 MB 限制".to_string());
    }

    let bundle = source_import::parse_import_bundle(payload)?;
    persist_imported_sources(database, bundle)
}

fn preview_source_payload(payload: &str) -> Result<source_import::ImportPreview, String> {
    if payload.len() > MAX_SOURCE_BUNDLE_BYTES {
        return Err("书源文件超过 2 MB 限制".to_string());
    }

    source_import::preview_import_bundle(payload)
}

fn validate_source_import_url(url: &str) -> Result<&str, String> {
    let url = url.trim();
    if url.is_empty() {
        return Err("书源 URL 不能为空".to_string());
    }
    if url.len() > MAX_SOURCE_IMPORT_URL_BYTES {
        return Err("书源 URL 不能超过 2 KB".to_string());
    }
    Ok(url)
}

async fn fetch_source_import_payload(url: &str) -> Result<String, String> {
    let url = validate_source_import_url(url)?;
    let engine = SourceEngine::default().map_err(|error| error.to_string())?;
    engine
        .fetch_text_document(url)
        .await
        .map_err(|error| format!("书源 URL 获取失败：{error}"))
}

#[tauri::command]
fn preview_sources(bundle_json: String) -> Result<source_import::ImportPreview, String> {
    preview_source_payload(&bundle_json)
}

#[tauri::command]
fn import_sources_selected(
    database: tauri::State<'_, Database>,
    bundle_json: String,
    indices: Vec<usize>,
) -> Result<Vec<SourceSummary>, String> {
    if bundle_json.len() > MAX_SOURCE_BUNDLE_BYTES {
        return Err("书源文件超过 2 MB 限制".to_string());
    }

    let bundle = source_import::parse_selected_entries(&bundle_json, &indices)?;
    persist_imported_sources(&database, bundle)
}

#[tauri::command]
fn export_sources(database: tauri::State<'_, Database>) -> Result<String, String> {
    let sources = database.list_sources().map_err(|error| error.to_string())?;
    let bundle = serde_json::json!({
        "version": 1,
        "sources": sources
            .into_iter()
            .map(|source| serde_json::json!({
                "id": source.id,
                "enabled": source.enabled,
                "config_json": source.config_json,
            }))
            .collect::<Vec<_>>(),
    });
    serde_json::to_string_pretty(&bundle).map_err(|error| error.to_string())
}

#[tauri::command]
fn import_sources(
    database: tauri::State<'_, Database>,
    bundle_json: String,
) -> Result<Vec<SourceSummary>, String> {
    import_source_payload(&database, &bundle_json)
}

#[tauri::command]
async fn preview_sources_from_url(url: String) -> Result<RemoteSourceImportPreview, String> {
    let payload = fetch_source_import_payload(&url).await?;
    let preview = preview_source_payload(&payload)?;
    Ok(RemoteSourceImportPreview { payload, preview })
}

#[tauri::command]
async fn import_sources_from_url(
    database: tauri::State<'_, Database>,
    url: String,
) -> Result<Vec<SourceSummary>, String> {
    let payload = fetch_source_import_payload(&url).await?;
    import_source_payload(&database, &payload)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RemoteBookDetail {
    source_id: String,
    source_name: String,
    book_info: source::BookInfo,
    chapters: Vec<source::SourceChapter>,
    debug_steps: Vec<source::SourceDebugStep>,
    #[serde(default)]
    chapter_fingerprint: String,
    #[serde(default)]
    chapter_update: Option<source::ChapterUpdateSummary>,
    #[serde(default)]
    stale: bool,
    #[serde(default)]
    refresh_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RemoteChapterContent {
    title: String,
    content: String,
    #[serde(default)]
    next_url: Option<String>,
    #[serde(default)]
    stale: bool,
    #[serde(default)]
    refresh_error: Option<String>,
}

impl From<source::SourceChapterContent> for RemoteChapterContent {
    fn from(content: source::SourceChapterContent) -> Self {
        Self {
            title: content.title,
            content: content.content,
            next_url: content.next_url,
            stale: false,
            refresh_error: None,
        }
    }
}

const SOURCE_BOOK_CACHE_TTL_SECS: u64 = 5 * 60;
const SOURCE_CHAPTER_CACHE_TTL_SECS: u64 = 10 * 60;
const SOURCE_CACHE_MAX_ENTRIES: usize = 256;
const SOURCE_CACHE_MAX_BYTES: usize = 32 * 1024 * 1024;

#[derive(Debug, Clone, Serialize)]
struct SourceCacheStatus {
    entries: usize,
    bytes: usize,
    expired_entries: usize,
    oldest_fetched_at: Option<i64>,
    max_entries: usize,
    max_bytes: usize,
}

#[tauri::command]
fn get_source_cache_status(
    database: tauri::State<'_, Database>,
) -> Result<SourceCacheStatus, String> {
    let stats: SourceCacheStats = database
        .source_cache_stats()
        .map_err(|error| error.to_string())?;
    Ok(SourceCacheStatus {
        entries: stats.entries,
        bytes: stats.bytes,
        expired_entries: stats.expired_entries,
        oldest_fetched_at: stats.oldest_fetched_at,
        max_entries: SOURCE_CACHE_MAX_ENTRIES,
        max_bytes: SOURCE_CACHE_MAX_BYTES,
    })
}

fn load_enabled_source(
    database: &Database,
    source_id: &str,
) -> Result<(SourceSummary, source::BookSource), String> {
    let summary = database
        .list_sources()
        .map_err(|error| error.to_string())?
        .into_iter()
        .find(|source| source.id == source_id)
        .ok_or_else(|| "书源不存在".to_string())?;

    if !summary.enabled {
        return Err("书源已停用，请先重新启用".to_string());
    }

    let source = serde_json::from_str::<source::BookSource>(&summary.config_json)
        .map_err(|error| format!("书源配置解析失败：{error}"))?;
    Ok((summary, source))
}

fn source_cache_key(kind: &str, summary: &SourceSummary, url: &str) -> String {
    format!("{kind}|{}|{}|{url}", summary.id, summary.updated_at)
}

fn log_cache_prune(database: &Database, context: &str) {
    match database.prune_source_cache(SOURCE_CACHE_MAX_ENTRIES, SOURCE_CACHE_MAX_BYTES) {
        Ok(removed) if removed > 0 => {
            eprintln!("source cache pruned {removed} entries ({context})");
        }
        Ok(_) => {}
        Err(error) => eprintln!("unable to prune source cache ({context}): {error}"),
    }
}

fn cached_remote_book(
    database: &Database,
    cache_key: &str,
) -> Result<Option<RemoteBookDetail>, String> {
    let payload = database
        .get_source_cache_any(cache_key)
        .map_err(|error| error.to_string())?;
    Ok(payload.and_then(|value| serde_json::from_str::<RemoteBookDetail>(&value).ok()))
}

fn cached_remote_chapter(
    database: &Database,
    cache_key: &str,
) -> Result<Option<RemoteChapterContent>, String> {
    let payload = database
        .get_source_cache_any(cache_key)
        .map_err(|error| error.to_string())?;
    Ok(payload.and_then(|value| serde_json::from_str::<RemoteChapterContent>(&value).ok()))
}

#[tauri::command]
async fn fetch_source_preview(url: String) -> Result<SourcePreview, String> {
    let engine = SourceEngine::default().map_err(|error| error.to_string())?;
    engine.fetch(&url).await.map_err(|error| error.to_string())
}

#[tauri::command]
async fn fetch_source_book(
    database: tauri::State<'_, Database>,
    source_id: String,
    book_url: String,
    force_refresh: bool,
) -> Result<RemoteBookDetail, String> {
    if book_url.trim().is_empty() {
        return Err("书籍链接不能为空".to_string());
    }

    let (summary, source) = load_enabled_source(&database, &source_id)?;
    let cache_key = source_cache_key("book", &summary, &book_url);
    let previous = cached_remote_book(&database, &cache_key)?;
    if !force_refresh {
        if let Some(payload) = database
            .get_source_cache(&cache_key)
            .map_err(|error| error.to_string())?
        {
            if let Ok(cached) = serde_json::from_str::<RemoteBookDetail>(&payload) {
                return Ok(cached);
            }
        }
    }

    let engine = SourceEngine::default().map_err(|error| error.to_string())?;
    let detail: SourceBookDetail = match engine.fetch_book_detail(&source, &book_url).await {
        Ok(detail) => detail,
        Err(error) => {
            if let Some(mut fallback) = previous.clone() {
                fallback.stale = true;
                fallback.refresh_error = Some(error.to_string());
                fallback.chapter_update = None;
                return Ok(fallback);
            }
            return Err(error.to_string());
        }
    };
    let chapter_update = previous
        .as_ref()
        .map(|cached| source::summarize_chapter_update(&cached.chapters, &detail.chapters));
    let result = RemoteBookDetail {
        source_id: summary.id.clone(),
        source_name: summary.name.clone(),
        book_info: detail.book_info,
        chapter_fingerprint: source::chapter_fingerprint(&detail.chapters),
        chapters: detail.chapters,
        debug_steps: detail.debug_steps,
        chapter_update,
        stale: false,
        refresh_error: None,
    };
    let mut cache_result = result.clone();
    cache_result.chapter_update = None;
    cache_result.stale = false;
    cache_result.refresh_error = None;
    let payload = serde_json::to_string(&cache_result).map_err(|error| error.to_string())?;
    database
        .save_source_cache(
            &cache_key,
            &summary.id,
            "book",
            &payload,
            SOURCE_BOOK_CACHE_TTL_SECS,
        )
        .map_err(|error| error.to_string())?;
    log_cache_prune(&database, "after write");
    Ok(result)
}

#[tauri::command]
async fn fetch_source_chapter(
    database: tauri::State<'_, Database>,
    source_id: String,
    chapter: source::SourceChapter,
    force_refresh: bool,
) -> Result<RemoteChapterContent, String> {
    if chapter.url.trim().is_empty() {
        return Err("章节链接不能为空".to_string());
    }

    let (summary, source) = load_enabled_source(&database, &source_id)?;
    let cache_key = source_cache_key("chapter", &summary, &chapter.url);
    let previous = cached_remote_chapter(&database, &cache_key)?;
    if !force_refresh {
        if let Some(payload) = database
            .get_source_cache(&cache_key)
            .map_err(|error| error.to_string())?
        {
            if let Ok(cached) = serde_json::from_str::<RemoteChapterContent>(&payload) {
                return Ok(cached);
            }
        }
    }

    let engine = SourceEngine::default().map_err(|error| error.to_string())?;
    let mut debug_steps = Vec::new();
    let result = match engine
        .fetch_chapter_content(&source, &chapter, &mut debug_steps)
        .await
    {
        Ok(content) => RemoteChapterContent::from(content),
        Err(error) => {
            if let Some(mut fallback) = previous {
                fallback.stale = true;
                fallback.refresh_error = Some(error.to_string());
                return Ok(fallback);
            }
            return Err(error.to_string());
        }
    };
    let mut cache_result = result.clone();
    cache_result.stale = false;
    cache_result.refresh_error = None;
    let payload = serde_json::to_string(&cache_result).map_err(|error| error.to_string())?;
    database
        .save_source_cache(
            &cache_key,
            &summary.id,
            "chapter",
            &payload,
            SOURCE_CHAPTER_CACHE_TTL_SECS,
        )
        .map_err(|error| error.to_string())?;
    log_cache_prune(&database, "after write");
    Ok(result)
}

#[tauri::command]
async fn search_sources(
    database: tauri::State<'_, Database>,
    keyword: String,
) -> Result<MultiSourceSearchResult, String> {
    let keyword = keyword.trim();
    if keyword.is_empty() {
        return Err("搜索关键词不能为空".to_string());
    }
    if keyword.chars().count() > 128 {
        return Err("搜索关键词不能超过 128 个字符".to_string());
    }

    let saved = database.list_sources().map_err(|error| error.to_string())?;
    let enabled_sources = saved.iter().filter(|source| source.enabled).count();
    let mut definitions = Vec::new();
    let mut failures = Vec::new();

    for summary in saved.into_iter().filter(|source| source.enabled) {
        match serde_json::from_str::<source::BookSource>(&summary.config_json) {
            Ok(source) => definitions.push(SourceDefinition {
                id: summary.id,
                name: summary.name,
                source,
            }),
            Err(error) => failures.push(SourceSearchFailure {
                source_id: summary.id,
                source_name: summary.name,
                message: format!("配置解析失败：{}", error),
            }),
        }
    }

    let engine = SourceEngine::default().map_err(|error| error.to_string())?;
    let mut result = engine.search_many(definitions, keyword).await;
    result.enabled_sources = enabled_sources;
    result.failures.splice(0..0, failures);
    Ok(result)
}

#[tauri::command]
async fn run_source_pipeline(
    config_json: String,
    keyword: String,
) -> Result<source::SourcePipelineResult, String> {
    let source: source::BookSource =
        serde_json::from_str(&config_json).map_err(|error| format!("JSON 解析失败：{error}"))?;
    let engine = SourceEngine::default().map_err(|error| error.to_string())?;
    engine
        .run_pipeline(&source, &keyword)
        .await
        .map_err(|error| error.to_string())
}

pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let app_data_dir = app
                .path()
                .app_data_dir()
                .expect("unable to resolve the application data directory");
            let database = Database::open(&app_data_dir).expect("unable to initialize SQLite");
            match database.clear_expired_source_cache() {
                Ok(removed) if removed > 0 => {
                    eprintln!("source cache expired cleanup removed {removed} entries");
                }
                Ok(_) => {}
                Err(error) => eprintln!("unable to clear expired source cache: {error}"),
            }
            log_cache_prune(&database, "startup");
            app.manage(database);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_books,
            import_book,
            get_book_detail,
            get_chapter_content,
            save_progress,
            validate_book_source,
            list_sources,
            save_source,
            set_source_enabled,
            delete_source,
            audit_sources,
            export_sources,
            import_sources,
            preview_sources,
            import_sources_selected,
            preview_sources_from_url,
            import_sources_from_url,
            fetch_source_preview,
            search_sources,
            fetch_source_book,
            fetch_source_chapter,
            run_source_pipeline,
            get_source_cache_status
        ])
        .run(tauri::generate_context!())
        .expect("error while running Open Reader Desktop");
}
