mod db;
mod library;
mod source;
mod source_import;

use db::{
    BookDetail, BookSummary, ChapterContent, Database, SourceCacheStats, SourceMetadata,
    SourceSnapshotSummary, SourceSummary, SourceWrite,
};
use library::parse_book_bytes;
use serde::{Deserialize, Serialize};
use source::{
    MultiSourceSearchResult, SourceBookDetail, SourceDefinition, SourceEngine, SourcePreview,
    SourceSearchFailure, SourceValidation,
};
use std::collections::HashSet;
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
    let metadata = SourceMetadata::from(&source);
    database
        .save_source(source_id.as_deref(), &source.name, &config_json, &metadata)
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

fn update_source_metadata_impl(
    database: &Database,
    source_id: &str,
    group_name: Option<String>,
    weight: Option<i64>,
    custom_order: Option<i64>,
    enabled_explore: Option<bool>,
    comment: Option<String>,
) -> Result<SourceSummary, String> {
    let summary = database
        .list_sources()
        .map_err(|error| error.to_string())?
        .into_iter()
        .find(|source| source.id == source_id)
        .ok_or_else(|| "书源不存在".to_string())?;

    let mut value: serde_json::Value = serde_json::from_str(&summary.config_json)
        .map_err(|error| format!("书源配置解析失败：{error}"))?;
    let object = value
        .as_object_mut()
        .ok_or_else(|| "书源配置必须是 JSON 对象".to_string())?;

    if let Some(group_name) = group_name {
        object.remove("bookSourceGroup");
        object.remove("book_source_group");
        let group_name = group_name.trim();
        if group_name.is_empty() {
            object.remove("group");
        } else {
            object.insert(
                "group".to_string(),
                serde_json::Value::String(group_name.to_string()),
            );
        }
    }
    if let Some(weight) = weight {
        object.insert("weight".to_string(), serde_json::json!(weight));
    }
    if let Some(custom_order) = custom_order {
        object.remove("customOrder");
        object.insert("custom_order".to_string(), serde_json::json!(custom_order));
    }
    if let Some(enabled_explore) = enabled_explore {
        object.remove("enabledExplore");
        object.insert(
            "enabled_explore".to_string(),
            serde_json::json!(enabled_explore),
        );
    }
    if let Some(comment) = comment {
        object.remove("bookSourceComment");
        object.remove("book_source_comment");
        let comment = comment.trim();
        if comment.is_empty() {
            object.remove("comment");
        } else {
            object.insert(
                "comment".to_string(),
                serde_json::Value::String(comment.to_string()),
            );
        }
    }

    let config_json =
        serde_json::to_string(&value).map_err(|error| format!("书源配置序列化失败：{error}"))?;
    let validation = source::validate_source_json(&config_json);
    let source = validation
        .source
        .ok_or_else(|| validation.errors.join("；"))?;
    if !validation.valid {
        return Err(validation.errors.join("；"));
    }
    let metadata = SourceMetadata::from(&source);
    database
        .save_source(Some(source_id), &source.name, &config_json, &metadata)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn update_source_metadata(
    database: tauri::State<'_, Database>,
    source_id: String,
    group_name: Option<String>,
    weight: Option<i64>,
    custom_order: Option<i64>,
    enabled_explore: Option<bool>,
    comment: Option<String>,
) -> Result<SourceSummary, String> {
    update_source_metadata_impl(
        &database,
        &source_id,
        group_name,
        weight,
        custom_order,
        enabled_explore,
        comment,
    )
}

#[tauri::command]
fn set_source_explore_enabled(
    database: tauri::State<'_, Database>,
    source_id: String,
    enabled: bool,
) -> Result<SourceSummary, String> {
    update_source_metadata_impl(&database, &source_id, None, None, None, Some(enabled), None)
}

const MAX_SOURCE_BATCH: usize = 512;

fn validate_source_ids(database: &Database, source_ids: &[String]) -> Result<(), String> {
    if source_ids.is_empty() {
        return Err("请至少选择一个书源".to_string());
    }
    if source_ids.len() > MAX_SOURCE_BATCH {
        return Err(format!("单次最多操作 {MAX_SOURCE_BATCH} 个书源"));
    }

    let mut unique = HashSet::new();
    for source_id in source_ids {
        if source_id.trim().is_empty() || !unique.insert(source_id) {
            return Err("书源选择列表包含空 ID 或重复项".to_string());
        }
    }

    let existing = database.list_sources().map_err(|error| error.to_string())?;
    if source_ids
        .iter()
        .any(|source_id| !existing.iter().any(|source| source.id == *source_id))
    {
        return Err("书源选择列表包含不存在的条目".to_string());
    }
    Ok(())
}

#[tauri::command]
fn set_sources_enabled(
    database: tauri::State<'_, Database>,
    source_ids: Vec<String>,
    enabled: bool,
) -> Result<Vec<SourceSummary>, String> {
    validate_source_ids(&database, &source_ids)?;
    source_ids
        .iter()
        .map(|source_id| {
            database
                .set_source_enabled(source_id, enabled)
                .map_err(|error| error.to_string())
        })
        .collect()
}

#[tauri::command]
fn set_sources_explore_enabled(
    database: tauri::State<'_, Database>,
    source_ids: Vec<String>,
    enabled: bool,
) -> Result<Vec<SourceSummary>, String> {
    validate_source_ids(&database, &source_ids)?;
    for source_id in &source_ids {
        update_source_metadata_impl(&database, source_id, None, None, None, Some(enabled), None)?;
    }
    database.list_sources().map_err(|error| error.to_string())
}

#[tauri::command]
fn reorder_sources(
    database: tauri::State<'_, Database>,
    source_ids: Vec<String>,
) -> Result<Vec<SourceSummary>, String> {
    validate_source_ids(&database, &source_ids)?;
    for (index, source_id) in source_ids.iter().enumerate() {
        update_source_metadata_impl(
            &database,
            source_id,
            None,
            None,
            Some(index as i64),
            None,
            None,
        )?;
    }
    database.list_sources().map_err(|error| error.to_string())
}

#[tauri::command]
fn delete_sources(
    database: tauri::State<'_, Database>,
    source_ids: Vec<String>,
) -> Result<(), String> {
    validate_source_ids(&database, &source_ids)?;
    for source_id in &source_ids {
        database
            .delete_source(source_id)
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

#[tauri::command]
fn set_sources_group(
    database: tauri::State<'_, Database>,
    source_ids: Vec<String>,
    group_name: String,
) -> Result<Vec<SourceSummary>, String> {
    validate_source_ids(&database, &source_ids)?;
    let group_name = group_name.trim().to_string();
    if group_name.len() > 128 {
        return Err("分组名称不能超过 128 字节".to_string());
    }
    for source_id in &source_ids {
        update_source_metadata_impl(
            &database,
            source_id,
            Some(group_name.clone()),
            None,
            None,
            None,
            None,
        )?;
    }
    database.list_sources().map_err(|error| error.to_string())
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

#[derive(Debug, Clone, Serialize)]
struct SourceImportResult {
    imported: Vec<SourceSummary>,
    snapshot_id: String,
    skipped: usize,
}

fn export_sources_payload(database: &Database) -> Result<String, String> {
    let sources = database.list_sources().map_err(|error| error.to_string())?;
    let bundle = serde_json::json!({
        "version": 1,
        "sources": sources
            .into_iter()
            .map(|source| serde_json::json!({
                "id": source.id,
                "enabled": source.enabled,
                "config_json": source.config_json,
                "source_url": source.source_url,
                "group_name": source.group_name,
                "source_type": source.source_type,
                "weight": source.weight,
                "enabled_explore": source.enabled_explore,
                "custom_order": source.custom_order,
                "comment": source.comment,
                "book_url_pattern": source.book_url_pattern,
                "explore_url": source.explore_url,
            }))
            .collect::<Vec<_>>(),
    });
    serde_json::to_string_pretty(&bundle).map_err(|error| error.to_string())
}

fn find_existing_source<'a>(
    sources: &'a [SourceSummary],
    source_id: Option<&str>,
    source: &source::BookSource,
) -> Option<&'a SourceSummary> {
    if let Some(source_id) = source_id.filter(|value| !value.trim().is_empty()) {
        if let Some(found) = sources.iter().find(|item| item.id == source_id) {
            return Some(found);
        }
    }
    if let Some(source_url) = source.source_url.as_deref().filter(|value| !value.is_empty()) {
        if let Some(found) = sources
            .iter()
            .find(|item| item.source_url.as_deref() == Some(source_url))
        {
            return Some(found);
        }
    }
    let matches: Vec<&SourceSummary> = sources
        .iter()
        .filter(|item| item.name == source.name)
        .collect();
    (matches.len() == 1).then(|| matches[0])
}

fn generated_import_source_id(index: usize) -> String {
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("source-import-{index}-{timestamp}")
}

fn persist_imported_sources_with_strategy(
    database: &Database,
    bundle: Vec<source_import::ImportedSource>,
    conflict_strategy: &str,
) -> Result<SourceImportResult, String> {
    if !matches!(conflict_strategy, "update" | "skip-existing" | "new") {
        return Err("不支持的书源冲突策略".to_string());
    }

    let existing = database.list_sources().map_err(|error| error.to_string())?;
    let mut writes = Vec::with_capacity(bundle.len());
    let mut target_ids = Vec::with_capacity(bundle.len());
    let mut skipped = 0;

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

        let existing_source = find_existing_source(&existing, item.id.as_deref(), &source);
        if conflict_strategy == "skip-existing" && existing_source.is_some() {
            skipped += 1;
            continue;
        }

        let id = if conflict_strategy == "new" {
            None
        } else {
            existing_source
                .map(|source| source.id.clone())
                .or(item.id)
        }
        .unwrap_or_else(|| generated_import_source_id(index));
        target_ids.push(id.clone());
        writes.push(SourceWrite::from_source(
            id,
            &source,
            item.config_json,
            item.enabled,
        ));
    }

    let snapshot_payload = export_sources_payload(database)?;
    let source_count = existing.len() as i64;
    let snapshot = database
        .create_source_snapshot("导入前自动快照", &snapshot_payload, source_count)
        .map_err(|error| error.to_string())?;
    let saved = database
        .apply_sources_atomic(&writes, false)
        .map_err(|error| format!("书源导入未完成，已保留快照：{error}"))?;
    let imported = saved
        .into_iter()
        .filter(|source| target_ids.iter().any(|id| id == &source.id))
        .collect();

    Ok(SourceImportResult {
        imported,
        snapshot_id: snapshot.id,
        skipped,
    })
}

fn persist_imported_sources(
    database: &Database,
    bundle: Vec<source_import::ImportedSource>,
) -> Result<Vec<SourceSummary>, String> {
    Ok(persist_imported_sources_with_strategy(database, bundle, "update")?.imported)
}

fn import_source_payload(database: &Database, payload: &str) -> Result<Vec<SourceSummary>, String> {
    if payload.len() > MAX_SOURCE_BUNDLE_BYTES {
        return Err("书源文件超过 2 MB 限制".to_string());
    }

    let bundle = source_import::parse_import_bundle(payload)?;
    persist_imported_sources(database, bundle)
}

fn canonical_source_value(config_json: &str) -> Option<serde_json::Value> {
    let source = serde_json::from_str::<source::BookSource>(config_json).ok()?;
    serde_json::to_value(source).ok()
}

fn source_diff_fields(existing: &serde_json::Value, incoming: &serde_json::Value) -> Vec<String> {
    const FIELDS: &[&str] = &[
        "name",
        "source_url",
        "group",
        "source_type",
        "book_url_pattern",
        "explore_url",
        "enabled_explore",
        "custom_order",
        "weight",
        "comment",
        "search_url",
        "book_info_url",
        "toc_url",
        "content_url",
        "search",
        "book_info",
        "toc",
        "content",
        "permission",
        "headers",
        "replace_rules",
    ];
    FIELDS
        .iter()
        .filter(|field| existing.get(*field) != incoming.get(*field))
        .map(|field| (*field).to_string())
        .collect()
}

fn source_import_match<'a>(
    sources: &'a [SourceSummary],
    entry: &source_import::ImportPreviewEntry,
    incoming: &serde_json::Value,
) -> Option<&'a SourceSummary> {
    if let Some(source_id) = entry.source_id.as_deref() {
        if let Some(found) = sources.iter().find(|source| source.id == source_id) {
            return Some(found);
        }
    }

    if let Some(source_url) = incoming
        .get("source_url")
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.is_empty())
    {
        if let Some(found) = sources
            .iter()
            .find(|source| source.source_url.as_deref() == Some(source_url))
        {
            return Some(found);
        }
    }

    let name = incoming
        .get("name")
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.is_empty())?;
    let matches: Vec<&SourceSummary> = sources
        .iter()
        .filter(|source| source.name == name)
        .collect();
    (matches.len() == 1).then(|| matches[0])
}

fn enrich_source_import_preview(
    database: &Database,
    mut preview: source_import::ImportPreview,
) -> Result<source_import::ImportPreview, String> {
    let sources = database.list_sources().map_err(|error| error.to_string())?;
    for entry in &mut preview.entries {
        if !entry.valid {
            continue;
        }
        let Some(config_json) = entry.config_json.as_deref() else {
            continue;
        };
        let Some(incoming) = canonical_source_value(config_json) else {
            continue;
        };
        if let Some(existing) = source_import_match(&sources, entry, &incoming) {
            entry.existing_id = Some(existing.id.clone());
            entry.changed_fields = source_diff_fields(
                &canonical_source_value(&existing.config_json).unwrap_or(serde_json::Value::Null),
                &incoming,
            );
            if entry.enabled != existing.enabled {
                entry.changed_fields.push("enabled".to_string());
            }
            entry.action = if entry.changed_fields.is_empty() {
                "无变化".to_string()
            } else {
                "更新".to_string()
            };
        } else {
            entry.action = "新增".to_string();
        }
    }
    Ok(preview)
}

fn preview_source_payload(
    database: &Database,
    payload: &str,
) -> Result<source_import::ImportPreview, String> {
    if payload.len() > MAX_SOURCE_BUNDLE_BYTES {
        return Err("书源文件超过 2 MB 限制".to_string());
    }

    let preview = source_import::preview_import_bundle(payload)?;
    enrich_source_import_preview(database, preview)
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
fn preview_sources(
    database: tauri::State<'_, Database>,
    bundle_json: String,
) -> Result<source_import::ImportPreview, String> {
    preview_source_payload(&database, &bundle_json)
}

#[tauri::command]
fn import_sources_selected(
    database: tauri::State<'_, Database>,
    bundle_json: String,
    indices: Vec<usize>,
    conflict_strategy: String,
) -> Result<SourceImportResult, String> {
    if bundle_json.len() > MAX_SOURCE_BUNDLE_BYTES {
        return Err("书源文件超过 2 MB 限制".to_string());
    }

    let bundle = source_import::parse_selected_entries(&bundle_json, &indices)?;
    persist_imported_sources_with_strategy(&database, bundle, &conflict_strategy)
}

#[tauri::command]
fn export_sources(database: tauri::State<'_, Database>) -> Result<String, String> {
    export_sources_payload(&database)
}

#[tauri::command]
fn import_sources(
    database: tauri::State<'_, Database>,
    bundle_json: String,
) -> Result<Vec<SourceSummary>, String> {
    import_source_payload(&database, &bundle_json)
}

#[tauri::command]
fn list_source_snapshots(
    database: tauri::State<'_, Database>,
) -> Result<Vec<SourceSnapshotSummary>, String> {
    database
        .list_source_snapshots()
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn restore_source_snapshot(
    database: tauri::State<'_, Database>,
    snapshot_id: String,
) -> Result<Vec<SourceSummary>, String> {
    let payload = database
        .get_source_snapshot(&snapshot_id)
        .map_err(|error| format!("读取书源快照失败：{error}"))?;
    let bundle = source_import::parse_import_bundle(&payload)?;
    let mut writes = Vec::with_capacity(bundle.len());

    for (index, item) in bundle.into_iter().enumerate() {
        let validation = source::validate_source_json(&item.config_json);
        let source = validation.source.ok_or_else(|| {
            format!(
                "快照第 {} 个书源无法解析：{}",
                index + 1,
                validation.errors.join("；")
            )
        })?;
        if !validation.valid {
            return Err(format!(
                "快照第 {} 个书源校验失败：{}",
                index + 1,
                validation.errors.join("；")
            ));
        }
        let id = item
            .id
            .unwrap_or_else(|| generated_import_source_id(index));
        writes.push(SourceWrite::from_source(
            id,
            &source,
            item.config_json,
            item.enabled,
        ));
    }

    database
        .apply_sources_atomic(&writes, true)
        .map_err(|error| format!("恢复书源快照失败：{error}"))
}

#[tauri::command]
async fn preview_sources_from_url(
    database: tauri::State<'_, Database>,
    url: String,
) -> Result<RemoteSourceImportPreview, String> {
    let payload = fetch_source_import_payload(&url).await?;
    let preview = preview_source_payload(&database, &payload)?;
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
            update_source_metadata,
            set_source_explore_enabled,
            set_sources_enabled,
            set_sources_explore_enabled,
            set_sources_group,
            reorder_sources,
            delete_source,
            delete_sources,
            audit_sources,
            export_sources,
            import_sources,
            list_source_snapshots,
            restore_source_snapshot,
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
