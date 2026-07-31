mod db;
mod library;
mod source;

use db::{BookDetail, BookSummary, ChapterContent, Database, SourceSummary};
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

const MAX_SOURCE_BUNDLE_BYTES: usize = 2 * 1024 * 1024;

#[tauri::command]
fn export_sources(database: tauri::State<'_, Database>) -> Result<String, String> {
    let sources = database.list_sources().map_err(|error| error.to_string())?;
    let bundle = SourceBundle {
        version: 1,
        sources: sources
            .into_iter()
            .map(|source| SourceBundleItem {
                id: Some(source.id),
                enabled: source.enabled,
                config_json: source.config_json,
            })
            .collect(),
    };
    serde_json::to_string_pretty(&bundle).map_err(|error| error.to_string())
}

#[tauri::command]
fn import_sources(
    database: tauri::State<'_, Database>,
    bundle_json: String,
) -> Result<Vec<SourceSummary>, String> {
    if bundle_json.len() > MAX_SOURCE_BUNDLE_BYTES {
        return Err("书源文件超过 2 MB 限制".to_string());
    }

    let bundle: SourceBundle = serde_json::from_str(&bundle_json)
        .map_err(|error| format!("书源文件 JSON 无效：{error}"))?;
    if bundle.version != 1 {
        return Err(format!("不支持的书源文件版本：{}", bundle.version));
    }
    if bundle.sources.is_empty() {
        return Err("书源文件没有可导入的配置".to_string());
    }

    let mut imported = Vec::with_capacity(bundle.sources.len());
    for (index, item) in bundle.sources.into_iter().enumerate() {
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

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SourceBundle {
    version: u32,
    sources: Vec<SourceBundleItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SourceBundleItem {
    id: Option<String>,
    enabled: bool,
    config_json: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RemoteBookDetail {
    source_id: String,
    source_name: String,
    book_info: source::BookInfo,
    chapters: Vec<source::SourceChapter>,
    debug_steps: Vec<source::SourceDebugStep>,
}

const SOURCE_BOOK_CACHE_TTL_SECS: u64 = 5 * 60;
const SOURCE_CHAPTER_CACHE_TTL_SECS: u64 = 10 * 60;

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
    let detail: SourceBookDetail = engine
        .fetch_book_detail(&source, &book_url)
        .await
        .map_err(|error| error.to_string())?;
    let result = RemoteBookDetail {
        source_id: summary.id.clone(),
        source_name: summary.name.clone(),
        book_info: detail.book_info,
        chapters: detail.chapters,
        debug_steps: detail.debug_steps,
    };
    let payload = serde_json::to_string(&result).map_err(|error| error.to_string())?;
    database
        .save_source_cache(
            &cache_key,
            &summary.id,
            "book",
            &payload,
            SOURCE_BOOK_CACHE_TTL_SECS,
        )
        .map_err(|error| error.to_string())?;
    Ok(result)
}

#[tauri::command]
async fn fetch_source_chapter(
    database: tauri::State<'_, Database>,
    source_id: String,
    chapter: source::SourceChapter,
    force_refresh: bool,
) -> Result<source::SourceChapterContent, String> {
    if chapter.url.trim().is_empty() {
        return Err("章节链接不能为空".to_string());
    }

    let (summary, source) = load_enabled_source(&database, &source_id)?;
    let cache_key = source_cache_key("chapter", &summary, &chapter.url);
    if !force_refresh {
        if let Some(payload) = database
            .get_source_cache(&cache_key)
            .map_err(|error| error.to_string())?
        {
            if let Ok(cached) = serde_json::from_str::<source::SourceChapterContent>(&payload) {
                return Ok(cached);
            }
        }
    }

    let engine = SourceEngine::default().map_err(|error| error.to_string())?;
    let mut debug_steps = Vec::new();
    let result = engine
        .fetch_chapter_content(&source, &chapter, &mut debug_steps)
        .await
        .map_err(|error| error.to_string())?;
    let payload = serde_json::to_string(&result).map_err(|error| error.to_string())?;
    database
        .save_source_cache(
            &cache_key,
            &summary.id,
            "chapter",
            &payload,
            SOURCE_CHAPTER_CACHE_TTL_SECS,
        )
        .map_err(|error| error.to_string())?;
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
            if let Err(error) = database.clear_expired_source_cache() {
                eprintln!("unable to clear expired source cache: {error}");
            }
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
            export_sources,
            import_sources,
            fetch_source_preview,
            search_sources,
            fetch_source_book,
            fetch_source_chapter,
            run_source_pipeline
        ])
        .run(tauri::generate_context!())
        .expect("error while running Open Reader Desktop");
}
