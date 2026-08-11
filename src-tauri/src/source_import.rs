use crate::source::{validate_source_json, BookSource};
use crate::xpath_poc;
use serde::Serialize;
use serde_json::{json, Map, Value};
use std::collections::HashSet;

const MAX_RULE_CHAIN_LENGTH: usize = 8;
const MAX_URL_CHAIN_LENGTH: usize = 8;
const MAX_UNSUPPORTED_RULES: usize = 8;
const MAX_UNSUPPORTED_RULE_BYTES: usize = 512;

#[derive(Debug, Clone)]
pub struct ImportedSource {
    pub id: Option<String>,
    pub enabled: bool,
    pub config_json: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct UnsupportedImportRule {
    pub context: String,
    pub value: String,
    pub reason: String,
    pub offline_accepted: bool,
    pub offline_syntax: String,
    pub offline_steps: usize,
    pub offline_estimated_work: usize,
    pub offline_elapsed_us: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImportPreviewEntry {
    pub index: usize,
    pub name: Option<String>,
    pub enabled: bool,
    pub valid: bool,
    pub error: Option<String>,
    pub action: String,
    pub existing_id: Option<String>,
    pub changed_fields: Vec<String>,
    pub unsupported_rules: Vec<UnsupportedImportRule>,
    #[serde(skip)]
    pub source_id: Option<String>,
    #[serde(skip)]
    pub config_json: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImportPreview {
    pub entries: Vec<ImportPreviewEntry>,
    pub valid_count: usize,
    pub invalid_count: usize,
}

/// Parse the native export format, a raw source object, or an array of
/// Legado-compatible source objects. Safe CSS/HTTP rules are mapped directly;
/// unsupported expressions are preserved as inert legacy rules so import never
/// silently drops the original source configuration.
pub fn parse_import_bundle(input: &str) -> Result<Vec<ImportedSource>, String> {
    let entries = extract_import_values(input)?;
    let imported = parse_entries(&entries)?;

    if imported.is_empty() {
        return Err("书源文件没有可导入的配置".to_string());
    }
    Ok(imported)
}

pub fn preview_import_bundle(input: &str) -> Result<ImportPreview, String> {
    let entries = extract_import_values(input)?;
    if entries.is_empty() {
        return Err("书源文件没有可导入的配置".to_string());
    }

    let mut preview_entries = Vec::with_capacity(entries.len());
    let mut valid_count = 0;
    for (index, entry) in entries.iter().enumerate() {
        let unsupported_rules = collect_unsupported_rules(entry);
        let parsed = parse_entry(entry, index);
        let (valid, error, source_id, config_json) = match parsed {
            Ok(source) => {
                let source_id = source.id.clone();
                let config_json = source.config_json;
                let validation = validate_source_json(&config_json);
                if validation.valid {
                    valid_count += 1;
                    (true, None, source_id, Some(config_json))
                } else {
                    (
                        false,
                        Some(validation.errors.join("；")),
                        source_id,
                        Some(config_json),
                    )
                }
            }
            Err(error) => (false, Some(error), None, None),
        };
        preview_entries.push(ImportPreviewEntry {
            index,
            name: entry_name(entry),
            enabled: entry_enabled(entry),
            valid,
            error,
            action: "新增".to_string(),
            existing_id: None,
            changed_fields: Vec::new(),
            unsupported_rules,
            source_id,
            config_json,
        });
    }

    Ok(ImportPreview {
        invalid_count: entries.len() - valid_count,
        entries: preview_entries,
        valid_count,
    })
}

pub fn parse_selected_entries(
    input: &str,
    indices: &[usize],
) -> Result<Vec<ImportedSource>, String> {
    if indices.is_empty() {
        return Err("没有选择可导入的书源".to_string());
    }

    let entries = extract_import_values(input)?;
    let mut selected = Vec::with_capacity(indices.len());
    let mut seen = HashSet::new();
    for index in indices {
        if !seen.insert(*index) {
            continue;
        }
        let entry = entries
            .get(*index)
            .ok_or_else(|| format!("书源序号超出范围：{}", index + 1))?;
        selected.push(parse_entry(entry, *index)?);
    }

    if selected.is_empty() {
        return Err("没有选择可导入的书源".to_string());
    }
    Ok(selected)
}

fn extract_import_values(input: &str) -> Result<Vec<Value>, String> {
    let input = input.trim_start_matches('\u{feff}');
    let value: Value =
        serde_json::from_str(input).map_err(|error| format!("书源文件 JSON 无效：{error}"))?;

    match value {
        Value::Array(entries) => Ok(entries),
        Value::Object(mut object)
            if object.contains_key("version") && object.contains_key("sources") =>
        {
            let version = object
                .get("version")
                .and_then(Value::as_u64)
                .ok_or_else(|| "书源文件 version 必须是数字".to_string())?;
            if version != 1 {
                return Err(format!("不支持的书源文件版本：{version}"));
            }
            let sources = object
                .remove("sources")
                .ok_or_else(|| "书源文件缺少 sources 数组".to_string())?;
            extract_value_entries(&sources, "sources")
        }
        Value::Object(object) => match extract_wrapper_entries(&object)? {
            Some(entries) => Ok(entries),
            None => Ok(vec![Value::Object(object)]),
        },
        _ => Err("书源文件必须是对象或数组".to_string()),
    }
}

fn extract_wrapper_entries(object: &Map<String, Value>) -> Result<Option<Vec<Value>>, String> {
    for key in ["sources", "bookSources", "booksources", "items", "data"] {
        let Some(value) = object.get(key) else {
            continue;
        };
        return extract_value_entries(value, key).map(Some);
    }
    Ok(None)
}

fn extract_value_entries(value: &Value, key: &str) -> Result<Vec<Value>, String> {
    match value {
        Value::Array(entries) => Ok(entries.clone()),
        Value::String(text) => {
            let text = text.trim_start_matches('\u{feff}');
            let parsed: Value = serde_json::from_str(text)
                .map_err(|error| format!("书源文件 {key} JSON 无效：{error}"))?;
            extract_value_entries(&parsed, key)
        }
        Value::Object(object) => {
            if let Some(entries) = extract_wrapper_entries(object)? {
                Ok(entries)
            } else {
                Ok(vec![value.clone()])
            }
        }
        _ => Err(format!("书源文件 {key} 必须是数组或 JSON 对象")),
    }
}

fn parse_entries(entries: &[Value]) -> Result<Vec<ImportedSource>, String> {
    entries
        .iter()
        .enumerate()
        .map(|(index, entry)| parse_entry(entry, index))
        .collect()
}

fn entry_enabled(value: &Value) -> bool {
    value
        .as_object()
        .and_then(|object| object.get("enabled"))
        .and_then(Value::as_bool)
        .unwrap_or(true)
}

fn entry_name(value: &Value) -> Option<String> {
    let object = value.as_object()?;
    if let Some(name) =
        optional_text(object.get("name")).or_else(|| optional_text(object.get("bookSourceName")))
    {
        return Some(name);
    }

    for key in ["config_json", "configJson"] {
        let Some(config) = object.get(key) else {
            continue;
        };
        let nested = match config {
            Value::String(text) => {
                serde_json::from_str::<Value>(text.trim_start_matches('\u{feff}')).ok()?
            }
            other => other.clone(),
        };
        if let Some(name) = entry_name(&nested) {
            return Some(name);
        }
    }
    None
}

fn collect_unsupported_rules(value: &Value) -> Vec<UnsupportedImportRule> {
    let mut findings = Vec::new();
    collect_unsupported_rules_at(value, "$", false, &mut findings);
    findings
}

fn collect_unsupported_rules_at(
    value: &Value,
    path: &str,
    in_rule_context: bool,
    findings: &mut Vec<UnsupportedImportRule>,
) {
    if findings.len() >= MAX_UNSUPPORTED_RULES {
        return;
    }

    match value {
        Value::String(raw) if in_rule_context => {
            let Some(reason) = unsupported_rule_reason(raw) else {
                return;
            };
            let is_xpath = is_xpath_expression(raw);
            let (
                offline_accepted,
                offline_syntax,
                offline_steps,
                offline_estimated_work,
                offline_elapsed_us,
            ) = if is_xpath {
                let offline = xpath_poc::analyze(raw, "<html></html>");
                (
                    offline.accepted,
                    offline.syntax,
                    offline.steps,
                    offline.estimated_work,
                    offline.elapsed_us,
                )
            } else {
                (false, "not_evaluated".to_string(), 0, 0, 1)
            };
            findings.push(UnsupportedImportRule {
                context: path.to_string(),
                value: truncate_preview_text(raw),
                reason: reason.to_string(),
                offline_accepted,
                offline_syntax,
                offline_steps,
                offline_estimated_work,
                offline_elapsed_us,
            });
        }
        Value::Array(entries) => {
            for (index, entry) in entries.iter().enumerate() {
                collect_unsupported_rules_at(
                    entry,
                    &format!("{path}[{index}]"),
                    in_rule_context,
                    findings,
                );
                if findings.len() >= MAX_UNSUPPORTED_RULES {
                    break;
                }
            }
        }
        Value::Object(object) => {
            for (key, child) in object {
                let child_path = format!("{path}.{key}");
                if key.eq_ignore_ascii_case("config_json") || key.eq_ignore_ascii_case("configJson")
                {
                    if let Some(raw) = child.as_str() {
                        if let Ok(nested) =
                            serde_json::from_str::<Value>(raw.trim_start_matches('\u{feff}'))
                        {
                            collect_unsupported_rules_at(
                                &nested,
                                &child_path,
                                in_rule_context,
                                findings,
                            );
                        }
                    }
                    continue;
                }

                let child_context = in_rule_context || is_rule_container_key(key);
                collect_unsupported_rules_at(child, &child_path, child_context, findings);
                if findings.len() >= MAX_UNSUPPORTED_RULES {
                    break;
                }
            }
        }
        _ => {}
    }
}

fn is_rule_container_key(key: &str) -> bool {
    matches!(
        key.trim().to_ascii_lowercase().as_str(),
        "search"
            | "rulesearch"
            | "bookinfo"
            | "rulebookinfo"
            | "toc"
            | "ruletoc"
            | "content"
            | "rulecontent"
            | "next"
            | "nexturl"
            | "next_url"
            | "nextpage"
            | "next_page"
    )
}

fn is_xpath_expression(value: &str) -> bool {
    let lowered = value.trim().to_ascii_lowercase();
    lowered.starts_with("//")
        || lowered.starts_with("xpath:")
        || lowered.starts_with("xpath=")
        || lowered.contains("@xpath")
}

fn is_script_expression(value: &str) -> bool {
    let lowered = value.trim().to_ascii_lowercase();
    lowered.contains("@js")
        || lowered.contains("@put:")
        || lowered.contains("@get:")
        || lowered.starts_with("javascript:")
        || lowered.contains("java.")
}

fn is_template_expression(value: &str) -> bool {
    value.contains("{{") || value.contains("}}")
}

fn is_legacy_selector_expression(value: &str) -> bool {
    let trimmed = value.trim();
    trimmed.starts_with("class.")
        || trimmed.starts_with("id.")
        || trimmed.starts_with("tag.")
        || trimmed.contains("!0")
        || trimmed.contains(".0@")
        || trimmed.contains(".1@")
        || (trimmed.contains("@") && trimmed.contains("tag."))
}

fn unsupported_rule_reason(value: &str) -> Option<&'static str> {
    if is_xpath_expression(value) {
        Some("XPath 规则已保留原表达式，当前不执行")
    } else if is_script_expression(value) {
        Some("JavaScript/脚本规则已保留原表达式，当前不执行")
    } else if is_template_expression(value) {
        Some("模板表达式已保留原文，当前不执行")
    } else if is_legacy_selector_expression(value) {
        Some("Legado 链式选择器已保留原表达式，当前不执行")
    } else {
        None
    }
}

fn truncate_preview_text(value: &str) -> String {
    let mut output = String::new();
    for character in value.trim().chars() {
        if output.len() + character.len_utf8() > MAX_UNSUPPORTED_RULE_BYTES {
            output.push('…');
            break;
        }
        output.push(character);
    }
    output
}

fn parse_entry(value: &Value, index: usize) -> Result<ImportedSource, String> {
    let object = value
        .as_object()
        .ok_or_else(|| format!("第 {} 个书源必须是对象", index + 1))?;
    let id = optional_text(object.get("id"))
        .or_else(|| optional_text(object.get("sourceId")))
        .or_else(|| optional_text(object.get("bookSourceUrl")))
        .or_else(|| optional_text(object.get("source_url")));
    let enabled = object
        .get("enabled")
        .and_then(Value::as_bool)
        .unwrap_or(true);

    if let Some(config) = object
        .get("config_json")
        .or_else(|| object.get("configJson"))
    {
        let config_value = match config {
            Value::String(text) => {
                serde_json::from_str::<Value>(text.trim_start_matches('\u{feff}'))
                    .map_err(|error| format!("第 {} 个书源 config_json 无效：{error}", index + 1))?
            }
            other => other.clone(),
        };
        let mut config_value = config_value;
        merge_wrapper_metadata(object, &mut config_value);
        let config_json = normalize_source_value(&config_value)
            .map_err(|error| format!("第 {} 个书源：{error}", index + 1))?;
        return Ok(ImportedSource {
            id,
            enabled,
            config_json,
        });
    }

    let config_json = normalize_source_value(value)
        .map_err(|error| format!("第 {} 个书源：{error}", index + 1))?;
    Ok(ImportedSource {
        id,
        enabled,
        config_json,
    })
}

fn merge_wrapper_metadata(wrapper: &Map<String, Value>, config: &mut Value) {
    let Some(config_object) = config.as_object_mut() else {
        return;
    };

    for (target, keys) in [
        (
            "source_url",
            &["sourceUrl", "bookSourceUrl", "source_url"][..],
        ),
        (
            "group",
            &[
                "group",
                "group_name",
                "bookSourceGroup",
                "book_source_group",
            ][..],
        ),
        (
            "source_type",
            &["source_type", "bookSourceType", "sourceType"][..],
        ),
        (
            "book_url_pattern",
            &["bookUrlPattern", "book_url_pattern"][..],
        ),
        ("explore_url", &["exploreUrl", "explore_url"][..]),
        (
            "enabled_explore",
            &["enabledExplore", "enabled_explore"][..],
        ),
        ("custom_order", &["customOrder", "custom_order"][..]),
        ("weight", &["weight"][..]),
        (
            "comment",
            &["comment", "bookSourceComment", "book_source_comment"][..],
        ),
    ] {
        if config_object.contains_key(target) {
            continue;
        }
        if let Some(value) = first_value(wrapper, keys) {
            config_object.insert(target.to_string(), value.clone());
        }
    }
}

fn normalize_source_value(value: &Value) -> Result<String, String> {
    let object = value
        .as_object()
        .ok_or_else(|| "书源配置必须是对象".to_string())?;
    let normalized = normalize_source_object(object)?;
    serde_json::to_string(&normalized).map_err(|error| format!("书源配置序列化失败：{error}"))
}

fn normalize_source_object(object: &Map<String, Value>) -> Result<Value, String> {
    if is_native_source(object) {
        let source: BookSource = serde_json::from_value(Value::Object(object.clone()))
            .map_err(|error| format!("原生书源配置无法解析：{error}"))?;
        return serde_json::to_value(source)
            .map_err(|error| format!("书源配置序列化失败：{error}"));
    }

    let name = required_text(object, &["name", "bookSourceName"], "name")?;
    let search_url = required_text(object, &["searchUrl", "search_url"], "searchUrl")?;
    let mut output = Map::new();
    output.insert("name".to_string(), Value::String(name));
    output.insert(
        "searchUrl".to_string(),
        Value::String(normalize_url(&search_url)?),
    );

    for (target, keys) in [
        (
            "source_url",
            &["sourceUrl", "bookSourceUrl", "source_url"][..],
        ),
        (
            "group",
            &["group", "bookSourceGroup", "book_source_group"][..],
        ),
        (
            "book_url_pattern",
            &["bookUrlPattern", "book_url_pattern"][..],
        ),
        ("explore_url", &["exploreUrl", "explore_url"][..]),
        (
            "comment",
            &["comment", "bookSourceComment", "book_source_comment"][..],
        ),
    ] {
        if let Some(value) = first_value(object, keys) {
            let text = value
                .as_str()
                .ok_or_else(|| format!("{target} 必须是字符串"))?
                .trim();
            if !text.is_empty() {
                let normalized = if matches!(target, "source_url" | "explore_url") {
                    normalize_url(text)?
                } else {
                    text.to_string()
                };
                output.insert(target.to_string(), Value::String(normalized));
            }
        }
    }

    if let Some(value) = first_value(object, &["source_type", "bookSourceType", "sourceType"][..]) {
        let source_type = value
            .as_i64()
            .ok_or_else(|| "bookSourceType 必须是数字".to_string())?;
        output.insert("source_type".to_string(), json!(source_type));
    }
    if let Some(value) = first_value(object, &["enabled_explore", "enabledExplore"][..]) {
        let enabled_explore = value
            .as_bool()
            .ok_or_else(|| "enabledExplore 必须是布尔值".to_string())?;
        output.insert("enabled_explore".to_string(), json!(enabled_explore));
    }
    for (target, keys) in [
        ("custom_order", &["customOrder", "custom_order"][..]),
        ("weight", &["weight"][..]),
    ] {
        if let Some(value) = first_value(object, keys) {
            let number = value
                .as_i64()
                .ok_or_else(|| format!("{target} 必须是整数"))?;
            output.insert(target.to_string(), json!(number));
        }
    }

    for (target, keys) in [
        ("bookInfoUrl", &["bookInfoUrl", "book_info_url"][..]),
        ("tocUrl", &["tocUrl", "toc_url"][..]),
        ("contentUrl", &["contentUrl", "content_url"][..]),
    ] {
        if let Some(value) = first_value(object, keys) {
            let url = value
                .as_str()
                .ok_or_else(|| format!("{target} 必须是字符串；当前只支持 HTTP/CSS 书源"))?;
            output.insert(target.to_string(), Value::String(normalize_url(url)?));
        }
    }

    if let Some(rules) = first_value(object, &["search", "ruleSearch"]) {
        output.insert(
            "search".to_string(),
            normalize_page_rules(rules, "ruleSearch", &["bookList", "book_list", "item"])?,
        );
    }
    if let Some(rules) = first_value(object, &["bookInfo", "ruleBookInfo"]) {
        output.insert(
            "bookInfo".to_string(),
            normalize_page_rules(rules, "ruleBookInfo", &["item"])?,
        );
    }
    if let Some(rules) = first_value(object, &["toc", "ruleToc"]) {
        output.insert(
            "toc".to_string(),
            normalize_page_rules(rules, "ruleToc", &["chapterList", "chapter_list", "item"])?,
        );
    }
    if let Some(rules) = first_value(object, &["content", "ruleContent"]) {
        output.insert(
            "content".to_string(),
            normalize_page_rules(rules, "ruleContent", &["item"])?,
        );
    }

    if let Some(headers) = first_value(object, &["headers", "header"]) {
        output.insert("headers".to_string(), normalize_headers(headers)?);
    }
    if let Some(permission) = first_value(object, &["permission", "permissions"]) {
        if permission.is_object() {
            output.insert("permission".to_string(), permission.clone());
        }
    }
    if let Some(replacements) = first_value(object, &["replaceRules", "replacements"]) {
        if replacements.is_array() {
            output.insert("replaceRules".to_string(), replacements.clone());
        }
    }

    Ok(Value::Object(output))
}

fn is_native_source(object: &Map<String, Value>) -> bool {
    object.contains_key("name")
        && !object.contains_key("bookSourceName")
        && !object.contains_key("ruleSearch")
        && !object.contains_key("ruleBookInfo")
        && !object.contains_key("ruleToc")
        && !object.contains_key("ruleContent")
        && (object.contains_key("search")
            || object.contains_key("searchUrl")
            || object.contains_key("search_url"))
}

fn normalize_page_rules(value: &Value, stage: &str, item_keys: &[&str]) -> Result<Value, String> {
    let object = value
        .as_object()
        .ok_or_else(|| format!("{stage} 必须是对象"))?;
    let mut output = Map::new();

    if let Some(item) = first_value(object, item_keys) {
        if let Some(selector) = optional_text(Some(item)) {
            match normalize_selector(&selector, &format!("{stage}.item")) {
                Ok(normalized) => {
                    output.insert("item".to_string(), Value::String(normalized));
                }
                Err(error) if !is_fatal_rule_error(&error) => {
                    output.insert(
                        "item_legacy".to_string(),
                        legacy_rule(Value::String(selector), error),
                    );
                }
                Err(error) => return Err(error),
            }
        }
    }

    for (target, keys) in [
        (
            "title",
            &["title", "name", "bookName", "bookTitle", "chapterName"][..],
        ),
        ("author", &["author", "bookAuthor"][..]),
        (
            "url",
            &[
                "url",
                "bookUrl",
                "book_url",
                "chapterUrl",
                "chapter_url",
                "coverUrl",
                "cover",
            ][..],
        ),
        ("intro", &["intro", "desc", "description", "bookIntro"][..]),
        ("content", &["content", "text", "bookContent"][..]),
        (
            "next",
            &[
                "next",
                "nextUrl",
                "next_url",
                "nextPage",
                "next_page",
                "nextContentUrl",
            ][..],
        ),
    ] {
        if let Some(rule) = first_value(object, keys) {
            let normalized = normalize_rule(rule, &format!("{stage}.{target}"))?;
            if !normalized.is_null() {
                output.insert(target.to_string(), normalized);
            }
        }
    }

    Ok(Value::Object(output))
}

fn normalize_rule(value: &Value, context: &str) -> Result<Value, String> {
    match value {
        Value::String(raw) => {
            if raw.trim().is_empty() {
                Ok(Value::Null)
            } else {
                match normalize_rule_parts(raw, context, None, None, None) {
                    Ok(normalized) => Ok(normalized),
                    Err(error) if !is_fatal_rule_error(&error) => {
                        Ok(legacy_rule(Value::String(raw.clone()), error))
                    }
                    Err(error) => Err(error),
                }
            }
        }
        Value::Object(object) => {
            if let Some(chain) = object.get("chain") {
                let entries = chain
                    .as_array()
                    .ok_or_else(|| format!("{context}.chain 必须是数组"))?;
                if entries.len() > MAX_RULE_CHAIN_LENGTH {
                    return Err(format!(
                        "{context}.chain 最多支持 {} 个候选",
                        MAX_RULE_CHAIN_LENGTH
                    ));
                }
                let normalized = entries
                    .iter()
                    .map(|entry| normalize_rule(entry, context))
                    .collect::<Result<Vec<_>, _>>()?
                    .into_iter()
                    .filter(|entry| !entry.is_null())
                    .collect::<Vec<_>>();
                if normalized.is_empty() {
                    return Ok(Value::Null);
                }
                if normalized.len() == 1 {
                    return Ok(normalized.into_iter().next().expect("one rule"));
                }
                return Ok(json!({ "chain": normalized }));
            }

            let Some(raw_selector) =
                first_value(object, &["selector", "rule", "value", "jsonPath", "path"])
                    .and_then(Value::as_str)
            else {
                return Ok(legacy_rule(
                    Value::Object(object.clone()),
                    format!("{context} 缺少 selector 或 JSONPath"),
                ));
            };
            if raw_selector.trim().is_empty() {
                return Ok(Value::Null);
            }
            let attr = object.get("attr").and_then(Value::as_str);
            let regex = object.get("regex").and_then(Value::as_str);
            let replacement = object.get("replacement").and_then(Value::as_str);
            match normalize_rule_parts(raw_selector, context, attr, regex, replacement) {
                Ok(normalized) => {
                    if normalized.is_string() {
                        Ok(json!({ "selector": normalized }))
                    } else {
                        Ok(normalized)
                    }
                }
                Err(error) if !is_fatal_rule_error(&error) => {
                    Ok(legacy_rule(Value::Object(object.clone()), error))
                }
                Err(error) => Err(error),
            }
        }
        other => Ok(legacy_rule(
            other.clone(),
            format!("{context} 必须是字符串或规则对象"),
        )),
    }
}

fn is_fatal_rule_error(error: &str) -> bool {
    error.contains("最多支持")
}

fn legacy_rule(value: Value, reason: String) -> Value {
    json!({
        "legacy": value,
        "reason": truncate_preview_text(&reason),
    })
}

fn normalize_rule_parts(
    raw_selector: &str,
    context: &str,
    override_attr: Option<&str>,
    override_regex: Option<&str>,
    override_replacement: Option<&str>,
) -> Result<Value, String> {
    let (raw_expression, parsed_regex, parsed_replacement) =
        split_rule_transform(raw_selector, context)?;
    let parts = split_rule_chain(&raw_expression, context)?;
    if parts.is_empty() {
        return Ok(Value::Null);
    }

    let mut normalized = Vec::with_capacity(parts.len());
    for part in parts {
        let join_parts = part
            .split("&&")
            .map(str::trim)
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>();
        if join_parts.is_empty() {
            continue;
        }
        if join_parts.len() > MAX_RULE_CHAIN_LENGTH {
            return Err(format!(
                "{context} 组合规则最多支持 {} 个候选",
                MAX_RULE_CHAIN_LENGTH
            ));
        }

        let mut joined = Vec::with_capacity(join_parts.len());
        for join_part in join_parts {
            let (selector, parsed_attr) = match parse_legado_rule(join_part, context) {
                Ok(parsed) => parsed,
                Err(error) if !is_fatal_rule_error(&error) => {
                    joined.push(legacy_rule(Value::String(join_part.to_string()), error));
                    continue;
                }
                Err(error) => return Err(error),
            };
            let attr = if let Some(raw_attr) = override_attr {
                if !is_supported_attr(raw_attr) {
                    joined.push(legacy_rule(
                        Value::String(join_part.to_string()),
                        format!("{context} 的属性后缀 @{} 不在安全子集内", raw_attr.trim()),
                    ));
                    continue;
                }
                normalize_attr(raw_attr)
            } else {
                parsed_attr
            };
            let effective_regex = override_regex
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .or(parsed_regex.as_deref());
            let effective_replacement = override_replacement
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .or(parsed_replacement.as_deref());
            if effective_replacement.is_some() && effective_regex.is_none() {
                joined.push(legacy_rule(
                    Value::String(join_part.to_string()),
                    format!("{context} replacement 不能脱离 regex 使用"),
                ));
                continue;
            }

            if attr.is_none() && effective_regex.is_none() && effective_replacement.is_none() {
                joined.push(Value::String(selector));
                continue;
            }

            let mut output = Map::new();
            output.insert("selector".to_string(), Value::String(selector));
            if let Some(attr) = attr {
                output.insert("attr".to_string(), Value::String(attr));
            }
            if let Some(regex) = effective_regex {
                output.insert("regex".to_string(), Value::String(regex.to_string()));
            }
            if let Some(replacement) = effective_replacement {
                output.insert(
                    "replacement".to_string(),
                    Value::String(replacement.to_string()),
                );
            }
            joined.push(Value::Object(output));
        }

        if joined.len() == 1 {
            normalized.push(joined.remove(0));
        } else if joined.len() > 1 {
            normalized.push(json!({ "join": joined }));
        }
    }

    if normalized.len() == 1 {
        Ok(normalized.remove(0))
    } else if normalized.len() > 1 {
        Ok(json!({ "chain": normalized }))
    } else {
        Ok(Value::Null)
    }
}

fn split_rule_chain<'a>(raw: &'a str, context: &str) -> Result<Vec<&'a str>, String> {
    let expression = raw.split_once("##").map(|(value, _)| value).unwrap_or(raw);
    let parts = expression
        .split("||")
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    if parts.len() > MAX_RULE_CHAIN_LENGTH {
        return Err(format!(
            "{context} 链式规则最多支持 {} 个候选",
            MAX_RULE_CHAIN_LENGTH
        ));
    }
    Ok(parts)
}

fn split_rule_transform(
    raw: &str,
    context: &str,
) -> Result<(String, Option<String>, Option<String>), String> {
    let mut pieces = raw.splitn(3, "##");
    let expression = pieces.next().unwrap_or_default().trim().to_string();
    if expression.is_empty() {
        return Err(format!("{context} selector 不能为空"));
    }
    let regex = pieces
        .next()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    let replacement = pieces
        .next()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    if replacement.is_some() && regex.is_none() {
        return Err(format!("{context} replacement 不能脱离 regex 使用"));
    }
    Ok((expression, regex, replacement))
}

fn parse_legado_rule(raw: &str, context: &str) -> Result<(String, Option<String>), String> {
    let raw = raw.trim();
    let lowered = raw.to_ascii_lowercase();
    if raw.is_empty() {
        return Err(format!("{context} 不能为空"));
    }
    if raw.contains("{{") || raw.contains("}}") {
        return Err(format!(
            "{context} 包含未实现的模板/脚本规则；请改写为单个 CSS 选择器"
        ));
    }
    if lowered.starts_with("//")
        || lowered.starts_with("xpath:")
        || lowered.starts_with("@xpath")
        || lowered.contains("@js")
        || lowered.contains("@put:")
        || lowered.contains("@get:")
    {
        return Err(format!(
            "{context} 使用了不受支持的 XPath/脚本规则；请改写为 CSS 选择器"
        ));
    }

    let first = if raw.starts_with('@') { &raw[1..] } else { raw };
    let segments = split_top_level_at(first);
    let (selector_text, suffix) = if segments.len() > 1 {
        let candidate = segments.last().copied().unwrap_or_default().trim();
        if is_supported_attr(candidate) || !looks_like_selector_segment(candidate) {
            (
                segments[..segments.len() - 1].join("@"),
                Some(candidate.to_string()),
            )
        } else {
            (first.to_string(), None)
        }
    } else {
        (first.to_string(), None)
    };
    let selector = normalize_selector(&selector_text, context)?;
    let attr = suffix.as_deref().and_then(normalize_attr);
    if let Some(suffix) = suffix.as_deref() {
        if !is_supported_attr(suffix) {
            return Err(format!("{context} 的属性后缀 @{suffix} 不在安全子集内"));
        }
    }
    Ok((selector, attr))
}

fn split_top_level_at(value: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0;
    let mut depth = 0usize;
    let mut quote: Option<char> = None;
    let mut escaped = false;
    for (index, character) in value.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if character == '\\' && quote.is_some() {
            escaped = true;
            continue;
        }
        if let Some(active) = quote {
            if character == active {
                quote = None;
            }
            continue;
        }
        match character {
            '\'' | '"' => quote = Some(character),
            '[' | '(' => depth = depth.saturating_add(1),
            ']' | ')' => depth = depth.saturating_sub(1),
            '@' if depth == 0 => {
                parts.push(value[start..index].trim());
                start = index + character.len_utf8();
            }
            _ => {}
        }
    }
    parts.push(value[start..].trim());
    parts
}

fn normalize_selector(value: &str, context: &str) -> Result<String, String> {
    let mut selector = value.trim().to_string();
    if selector.is_empty() {
        return Err(format!("{context} selector 不能为空"));
    }
    let lowered = selector.to_ascii_lowercase();
    if lowered.starts_with("xpath:")
        || lowered.starts_with("//")
        || lowered.contains("@js")
        || lowered.contains("@put:")
        || lowered.contains("@get:")
    {
        return Err(format!(
            "{context} 使用了不受支持的 XPath/脚本规则；请改写为 CSS 选择器"
        ));
    }
    if selector.starts_with("css:") {
        selector = selector.trim_start_matches("css:").trim().to_string();
    }
    if selector.contains("&&") || selector.contains("||") {
        return Err(format!(
            "{context} 包含多段未实现的规则表达式；请改写为单个 CSS 选择器"
        ));
    }

    let mut normalized_segments = Vec::new();
    for raw_segment in split_top_level_at(&selector) {
        if raw_segment.is_empty() {
            return Err(format!("{context} selector 包含空链式候选"));
        }
        let mut segment = raw_segment.to_string();
        for (prefix, replacement) in [("class.", "."), ("id.", "#"), ("tag.", "")] {
            if let Some(rest) = segment.strip_prefix(prefix) {
                segment = format!("{replacement}{rest}");
                break;
            }
        }
        segment = strip_legacy_indexes(&segment);
        if segment.trim().is_empty() {
            return Err(format!("{context} selector 不能为空"));
        }
        if segment.starts_with("text.") || segment.contains('!') {
            return Err(format!("{context} 使用了仅 Legado 支持的文本/索引选择器"));
        }
        normalized_segments.push(segment);
    }
    Ok(normalized_segments.join(" "))
}

fn strip_legacy_indexes(value: &str) -> String {
    let mut output = value.trim().to_string();
    loop {
        let mut changed = false;
        if let Some((base, suffix)) = output.rsplit_once('!') {
            if !suffix.is_empty() && suffix.chars().all(|character| character.is_ascii_digit()) {
                output = base.trim_end().to_string();
                changed = true;
            }
        }
        if !changed {
            if let Some((base, suffix)) = output.rsplit_once('.') {
                if !suffix.is_empty() && suffix.chars().all(|character| character.is_ascii_digit())
                {
                    output = base.trim_end().to_string();
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }
    if let Some((base, suffix)) = output.split_once(':') {
        if suffix.split(':').all(|part| {
            !part.is_empty() && part.chars().all(|character| character.is_ascii_digit())
        }) {
            output = base.trim_end().to_string();
        }
    }
    output
}

fn looks_like_selector_segment(value: &str) -> bool {
    let value = value.trim();
    value.starts_with('.')
        || value.starts_with('#')
        || value.starts_with('[')
        || value.starts_with("class.")
        || value.starts_with("id.")
        || value.starts_with("tag.")
        || value.contains('.')
        || value.contains('#')
        || value.contains(':')
        || matches!(
            value.to_ascii_lowercase().as_str(),
            "a" | "abbr"
                | "article"
                | "aside"
                | "blockquote"
                | "button"
                | "div"
                | "em"
                | "h1"
                | "h2"
                | "h3"
                | "h4"
                | "h5"
                | "h6"
                | "img"
                | "li"
                | "main"
                | "nav"
                | "p"
                | "section"
                | "span"
                | "strong"
                | "table"
                | "tbody"
                | "td"
                | "th"
                | "thead"
                | "tr"
                | "ul"
        )
}

fn is_supported_attr(value: &str) -> bool {
    let value = value.trim().to_ascii_lowercase();
    matches!(
        value.as_str(),
        "text"
            | "textnodes"
            | "owntext"
            | "all"
            | "content"
            | "html"
            | "href"
            | "src"
            | "title"
            | "alt"
            | "value"
            | "style"
            | "poster"
            | "rel"
            | "id"
            | "class"
            | "name"
            | "data-src"
            | "data-original"
            | "data-url"
            | "data-href"
            | "data-cover"
    ) || is_safe_custom_attr(&value)
}

fn is_safe_custom_attr(value: &str) -> bool {
    (value.starts_with("data-") || value.starts_with("aria-"))
        && value.len() <= 64
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
}

fn normalize_attr(value: &str) -> Option<String> {
    let value = value.trim().to_ascii_lowercase();
    match value.as_str() {
        "text" | "textnodes" | "owntext" | "all" => None,
        _ if is_supported_attr(&value) => Some(value),
        _ => None,
    }
}

fn normalize_headers(value: &Value) -> Result<Value, String> {
    match value {
        Value::Object(object) => {
            let mut headers = Map::new();
            for (key, value) in object {
                let text = value
                    .as_str()
                    .ok_or_else(|| format!("headers.{key} 必须是字符串"))?;
                insert_header(&mut headers, key, text)?;
            }
            Ok(Value::Object(headers))
        }
        Value::String(text) => {
            let text = text.trim();
            if text.is_empty() {
                return Ok(Value::Object(Map::new()));
            }
            if let Ok(parsed) = serde_json::from_str::<Value>(text) {
                if parsed.is_object() {
                    return normalize_headers(&parsed);
                }
            }

            let mut headers = Map::new();
            for line in text.lines().map(str::trim).filter(|line| !line.is_empty()) {
                let (key, value) = line
                    .split_once(':')
                    .ok_or_else(|| "header 字符串必须使用 Name: Value 格式".to_string())?;
                insert_header(&mut headers, key.trim(), value.trim())?;
            }
            if headers.is_empty() {
                return Err("header 字符串没有可用字段".to_string());
            }
            Ok(Value::Object(headers))
        }
        _ => Err("headers/header 目前只支持 JSON 对象或 Name: Value 字符串".to_string()),
    }
}

fn insert_header(headers: &mut Map<String, Value>, key: &str, value: &str) -> Result<(), String> {
    if key.is_empty() || value.is_empty() {
        return Err("header 名称和值不能为空".to_string());
    }
    if key
        .chars()
        .any(|character| matches!(character, '\r' | '\n'))
        || value
            .chars()
            .any(|character| matches!(character, '\r' | '\n'))
    {
        return Err("header 不能包含换行符".to_string());
    }
    headers.insert(key.to_string(), Value::String(value.to_string()));
    Ok(())
}

fn required_text(object: &Map<String, Value>, keys: &[&str], name: &str) -> Result<String, String> {
    first_value(object, keys)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .ok_or_else(|| format!("{name} 不能为空；Legado 书源需要提供对应字段"))
}

fn optional_text(value: Option<&Value>) -> Option<String> {
    value
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn first_value<'a>(object: &'a Map<String, Value>, keys: &[&str]) -> Option<&'a Value> {
    keys.iter().find_map(|key| object.get(*key))
}

fn normalize_url(value: &str) -> Result<String, String> {
    let parts = value.split("||").map(str::trim).collect::<Vec<_>>();
    if parts.is_empty() || parts.iter().any(|part| part.is_empty()) {
        return Err("URL 回退链包含空候选".to_string());
    }
    if parts.len() > MAX_URL_CHAIN_LENGTH {
        return Err(format!(
            "URL 回退链最多支持 {} 个候选",
            MAX_URL_CHAIN_LENGTH
        ));
    }
    Ok(parts
        .into_iter()
        .map(|part| {
            part.replace("{{page}}", "1")
                .replace("{{pageNum}}", "1")
                .replace("{{page+1}}", "2")
                .replace("{{page-1}}", "0")
        })
        .collect::<Vec<_>>()
        .join("||"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::SourceRule;

    #[test]
    fn imports_native_bundle() {
        let payload = json!({
            "version": 1,
            "sources": [{
                "id": "fixture",
                "enabled": false,
                "group_name": "Library",
                "weight": 7,
                "config_json": {
                    "name": "Fixture",
                    "searchUrl": "https://example.test/search?q={{keyword}}"
                }
            }]
        });
        let imported = parse_import_bundle(&payload.to_string()).expect("native bundle");
        assert_eq!(imported.len(), 1);
        assert_eq!(imported[0].id.as_deref(), Some("fixture"));
        assert!(!imported[0].enabled);
        let source: BookSource =
            serde_json::from_str(&imported[0].config_json).expect("canonical source");
        assert_eq!(source.name, "Fixture");
        assert_eq!(source.group.as_deref(), Some("Library"));
        assert_eq!(source.weight, 7);
    }

    #[test]
    fn normalizes_legado_css_subset() {
        let payload = json!({
            "bookSourceName": "Legado Fixture",
            "bookSourceUrl": "https://example.test/",
            "searchUrl": "https://example.test/search?q={{key}}",
            "ruleSearch": {
                "bookList": "class.book-list",
                "name": "h2 a@text",
                "author": ".author@text",
                "bookUrl": "h2 a@href"
            },
            "ruleBookInfo": {
                "name": "h1@text",
                "author": ".author@text",
                "intro": ".intro@text"
            },
            "ruleToc": {
                "chapterList": "ul.chapters li",
                "chapterName": "a@text",
                "chapterUrl": "a@href"
            },
            "ruleContent": {
                "content": ".content@text"
            }
        });
        let imported = parse_import_bundle(&payload.to_string()).expect("Legado source");
        let source: BookSource =
            serde_json::from_str(&imported[0].config_json).expect("canonical source");
        assert_eq!(source.name, "Legado Fixture");
        assert_eq!(
            source
                .search
                .as_ref()
                .and_then(|rules| rules.item.as_deref()),
            Some(".book-list")
        );
        assert!(matches!(
            source.search.as_ref().and_then(|rules| rules.url.as_ref()),
            Some(SourceRule::Detailed { attr: Some(attr), .. }) if attr == "href"
        ));
    }

    #[test]
    fn preserves_bounded_url_fallback_chain() {
        let payload = json!({
            "bookSourceName": "URL Chain Fixture",
            "searchUrl": "https://one.example.test/search?q={{key}}||https://two.example.test/search?q={{key}}",
            "ruleSearch": {
                "bookList": "li.book",
                "bookName": "h2 a",
                "bookUrl": "h2 a@href"
            }
        });
        let imported = parse_import_bundle(&payload.to_string()).expect("URL chain source");
        let source: BookSource =
            serde_json::from_str(&imported[0].config_json).expect("canonical URL chain source");
        assert_eq!(
            source.search_url,
            "https://one.example.test/search?q={{key}}||https://two.example.test/search?q={{key}}"
        );
    }

    #[test]
    fn normalizes_legado_aliases_and_header_string() {
        let payload = json!({
            "bookSourceName": "Alias Fixture",
            "searchUrl": "https://example.test/search?q={{key}}&page={{pageNum}}",
            "header": "User-Agent: OpenReaderTest\nAccept: text/html",
            "ruleSearch": {
                "bookList": "li.book",
                "bookName": "h2 a@text",
                "bookAuthor": ".author@text",
                "bookUrl": "h2 a@href"
            },
            "ruleBookInfo": {
                "bookName": "h1@text",
                "bookAuthor": ".author@text",
                "coverUrl": "img.cover@src",
                "bookIntro": ".intro@text"
            },
            "ruleContent": {
                "text": ".content@text"
            }
        });
        let imported = parse_import_bundle(&payload.to_string()).expect("alias source");
        let source: BookSource =
            serde_json::from_str(&imported[0].config_json).expect("canonical alias source");
        assert_eq!(source.name, "Alias Fixture");
        assert_eq!(
            source.headers.get("User-Agent").map(String::as_str),
            Some("OpenReaderTest")
        );
        assert!(matches!(
            source
                .search
                .as_ref()
                .and_then(|rules| rules.title.as_ref()),
            Some(SourceRule::Selector(selector)) if selector == "h2 a"
        ));
        assert!(matches!(
            source.book_info.as_ref().and_then(|rules| rules.url.as_ref()),
            Some(SourceRule::Detailed { attr: Some(attr), .. }) if attr == "src"
        ));
        assert!(source
            .content
            .as_ref()
            .and_then(|rules| rules.content.as_ref())
            .is_some());
    }

    #[test]
    fn normalizes_and_preserves_fallback_chain() {
        let payload = json!({
            "bookSourceName": "Chain Fixture",
            "searchUrl": "https://chain.example.test/search?q={{key}}",
            "ruleSearch": {
                "bookList": "li.book",
                "bookName": "h2 a@text||.title@text",
                "bookUrl": "h2 a@href||a@data-src"
            }
        });
        let imported = parse_import_bundle(&payload.to_string()).expect("chain source");
        let source: BookSource =
            serde_json::from_str(&imported[0].config_json).expect("canonical chain source");
        match source
            .search
            .as_ref()
            .and_then(|rules| rules.title.as_ref())
        {
            Some(SourceRule::Chain { chain }) => assert_eq!(chain.len(), 2),
            other => panic!("expected chain rule, got {other:?}"),
        }
    }

    #[test]
    fn normalizes_legado_metadata_and_uses_source_url_as_id() {
        let payload = json!({
            "bookSourceName": "Metadata Alias",
            "bookSourceUrl": "https://metadata.example.test/",
            "bookSourceGroup": "公开测试",
            "bookSourceType": 0,
            "bookUrlPattern": "https://metadata.example.test/book/{{bookId}}",
            "exploreUrl": "https://metadata.example.test/explore",
            "enabledExplore": true,
            "customOrder": 3,
            "weight": 50,
            "bookSourceComment": "授权夹具",
            "searchUrl": "https://metadata.example.test/search?q={{key}}"
        });
        let imported = parse_import_bundle(&payload.to_string()).expect("metadata source");
        assert_eq!(
            imported[0].id.as_deref(),
            Some("https://metadata.example.test/")
        );
        let source: BookSource =
            serde_json::from_str(&imported[0].config_json).expect("canonical metadata source");
        assert_eq!(
            source.source_url.as_deref(),
            Some("https://metadata.example.test/")
        );
        assert_eq!(source.group.as_deref(), Some("公开测试"));
        assert_eq!(source.source_type, 0);
        assert_eq!(
            source.book_url_pattern.as_deref(),
            Some("https://metadata.example.test/book/{{bookId}}")
        );
        assert_eq!(
            source.explore_url.as_deref(),
            Some("https://metadata.example.test/explore")
        );
        assert!(source.enabled_explore);
        assert_eq!(source.custom_order, 3);
        assert_eq!(source.weight, 50);
        assert_eq!(source.comment.as_deref(), Some("授权夹具"));
    }

    #[test]
    fn normalizes_jsonpath_rules() {
        let payload = json!({
            "bookSourceName": "JSON Alias Fixture",
            "searchUrl": "https://example.test/search?q={{key}}",
            "ruleSearch": {
                "bookList": "$.books[*]",
                "bookName": "$.title",
                "bookAuthor": { "jsonPath": "$.author" },
                "bookUrl": { "path": "$.url" }
            }
        });
        let imported = parse_import_bundle(&payload.to_string()).expect("JSONPath source");
        let source: BookSource =
            serde_json::from_str(&imported[0].config_json).expect("canonical JSONPath source");
        assert_eq!(
            source
                .search
                .as_ref()
                .and_then(|rules| rules.item.as_deref()),
            Some("$.books[*]")
        );
        assert!(matches!(
            source.search.as_ref().and_then(|rules| rules.title.as_ref()),
            Some(SourceRule::Selector(selector)) if selector == "$.title"
        ));
        assert!(matches!(
            source.search.as_ref().and_then(|rules| rules.author.as_ref()),
            Some(SourceRule::Detailed { selector, .. }) if selector == "$.author"
        ));
    }

    #[test]
    fn preserves_unsafe_legado_script_rule_as_inert_legacy() {
        let payload = json!({
            "bookSourceName": "Unsafe",
            "searchUrl": "https://example.test/search?q={{key}}",
            "ruleSearch": { "bookList": "li", "name": "@js:java.return 'x'" }
        });
        let imported = parse_import_bundle(&payload.to_string()).expect("legacy import");
        let source: BookSource =
            serde_json::from_str(&imported[0].config_json).expect("legacy source");
        assert!(matches!(
            source.search.as_ref().and_then(|rules| rules.title.as_ref()),
            Some(SourceRule::Legacy { legacy, .. })
                if legacy.as_str() == Some("@js:java.return 'x'")
        ));
    }

    #[test]
    fn preserves_template_rule_as_inert_legacy() {
        let payload = json!({
            "bookSourceName": "Template rule",
            "searchUrl": "https://example.test/search?q={{key}}",
            "ruleBookInfo": {
                "intro": "{{@#novel_intro@html}}##展开收起|飞卢小说(.|\\n)*"
            }
        });
        let imported = parse_import_bundle(&payload.to_string()).expect("template import");
        let source: BookSource =
            serde_json::from_str(&imported[0].config_json).expect("template source");
        assert!(matches!(
            source.book_info.as_ref().and_then(|rules| rules.intro.as_ref()),
            Some(SourceRule::Legacy { legacy, .. }) if legacy.as_str().is_some()
        ));
    }

    #[test]
    fn rejects_invalid_and_empty_documents() {
        let invalid = parse_import_bundle("{").expect_err("invalid json");
        assert!(invalid.contains("JSON 无效"));

        let empty = parse_import_bundle("[]").expect_err("empty array");
        assert!(empty.contains("没有可导入"));
    }

    #[test]
    fn accepts_html_and_rejects_unknown_rule_attributes() {
        let html_payload = json!({
            "bookSourceName": "HTML attribute",
            "searchUrl": "https://example.test/search?q={{key}}",
            "ruleSearch": { "bookList": "li", "name": "h2@html" }
        });
        let imported = parse_import_bundle(&html_payload.to_string()).expect("html attr");
        let source: BookSource =
            serde_json::from_str(&imported[0].config_json).expect("html source");
        assert!(matches!(
            source.search.as_ref().and_then(|rules| rules.title.as_ref()),
            Some(SourceRule::Detailed { attr: Some(attr), .. }) if attr == "html"
        ));

        let invalid_payload = json!({
            "bookSourceName": "Unknown attribute",
            "searchUrl": "https://example.test/search?q={{key}}",
            "ruleSearch": { "bookList": "li", "name": "h2@onclick" }
        });
        let imported = parse_import_bundle(&invalid_payload.to_string()).expect("legacy attr");
        let source: BookSource =
            serde_json::from_str(&imported[0].config_json).expect("legacy attr source");
        assert!(matches!(
            source
                .search
                .as_ref()
                .and_then(|rules| rules.title.as_ref()),
            Some(SourceRule::Legacy { .. })
        ));
    }

    #[test]
    fn omits_empty_optional_legado_rules() {
        let payload = json!({
            "bookSourceName": "Empty optional rules",
            "searchUrl": "https://example.test/search?q={{key}}",
            "ruleSearch": {
                "bookList": "li.book",
                "name": "h2 a",
                "intro": "",
                "author": "||"
            },
            "ruleBookInfo": {
                "name": "",
                "url": "",
                "intro": ""
            },
            "ruleToc": {
                "chapterList": "li.chapter",
                "chapterName": "",
                "chapterUrl": ""
            }
        });
        let imported = parse_import_bundle(&payload.to_string()).expect("empty rules");
        let source: BookSource =
            serde_json::from_str(&imported[0].config_json).expect("empty-rule source");
        assert!(source
            .search
            .as_ref()
            .and_then(|rules| rules.intro.as_ref())
            .is_none());
        assert!(source
            .search
            .as_ref()
            .and_then(|rules| rules.author.as_ref())
            .is_none());
        assert!(source
            .book_info
            .as_ref()
            .and_then(|rules| rules.title.as_ref())
            .is_none());
        assert!(source
            .toc
            .as_ref()
            .and_then(|rules| rules.url.as_ref())
            .is_none());
    }

    #[test]
    fn normalizes_legado_attr_regex_replacement_and_join() {
        let payload = json!({
            "bookSourceName": "Legado compatibility fixture",
            "searchUrl": "https://example.test/search?q={{key}}",
            "ruleSearch": {
                "bookList": "li.book",
                "author": "div.-1@text##\\s*丨.+$"
            },
            "ruleBookInfo": {
                "intro": "[name=\"og:novel:update_time\"]@content&&[property=\"og:description\"]@content##(^|[。！？]+[”」）】]?)##$1<br>"
            }
        });
        let imported = parse_import_bundle(&payload.to_string()).expect("Legado transforms");
        let source: BookSource =
            serde_json::from_str(&imported[0].config_json).expect("canonical source");
        assert!(matches!(
            source.search.as_ref().and_then(|rules| rules.author.as_ref()),
            Some(SourceRule::Detailed {
                attr: None,
                regex: Some(regex),
                replacement: None,
                ..
            }) if regex == r"\s*丨.+$"
        ));
        match source
            .book_info
            .as_ref()
            .and_then(|rules| rules.intro.as_ref())
        {
            Some(SourceRule::Join { join }) => {
                assert_eq!(join.len(), 2);
                assert!(join.iter().all(|rule| matches!(
                    rule,
                    SourceRule::Detailed {
                        attr: Some(attr),
                        regex: Some(_),
                        replacement: Some(replacement),
                        ..
                    } if attr == "content" && replacement == "$1<br>"
                )));
            }
            other => panic!("expected join rule, got {other:?}"),
        }
    }

    #[test]
    fn accepts_bom_and_nested_wrappers() {
        let source = json!({
            "bookSourceName": "Wrapped",
            "searchUrl": "https://wrapped.test/search?q={{key}}"
        });
        let payload = json!({ "data": { "bookSources": [source] } });
        let input = format!("\u{feff}{}", payload);
        let imported = parse_import_bundle(&input).expect("BOM and wrapper");
        assert_eq!(imported.len(), 1);
        let source: BookSource =
            serde_json::from_str(&imported[0].config_json).expect("wrapped source");
        assert_eq!(source.name, "Wrapped");
    }

    #[test]
    fn previews_valid_and_invalid_entries() {
        let payload = json!([
            {
                "bookSourceName": "Valid",
                "searchUrl": "https://valid.test?q={{key}}"
            },
            {
                "bookSourceName": "Unsafe",
                "searchUrl": "https://unsafe.test?q={{key}}",
                "ruleSearch": { "bookList": "li", "name": "@js:return 'x'" }
            }
        ]);
        let preview = preview_import_bundle(&payload.to_string()).expect("preview");
        assert_eq!(preview.entries.len(), 2);
        assert_eq!(preview.valid_count, 2);
        assert_eq!(preview.invalid_count, 0);
        assert!(preview.entries[0].valid);
        assert!(preview.entries[1].valid);
        assert!(preview.entries[1].error.is_none());
        assert!(preview.entries[1]
            .unsupported_rules
            .iter()
            .any(|rule| rule.reason.contains("脚本规则")));
    }

    #[test]
    fn previews_xpath_rule_with_read_only_reason() {
        let payload = json!([{
            "bookSourceName": "XPath fixture",
            "searchUrl": "https://xpath.test/search?q={{key}}",
            "ruleSearch": {
                "bookList": "//article",
                "name": "@xpath=//h2"
            }
        }]);
        let preview = preview_import_bundle(&payload.to_string()).expect("preview");
        assert_eq!(preview.valid_count, 1);
        assert_eq!(preview.invalid_count, 0);
        let entry = &preview.entries[0];
        assert!(entry.valid);
        assert_eq!(entry.unsupported_rules.len(), 2);
        assert!(entry
            .unsupported_rules
            .iter()
            .any(|rule| rule.value == "//article"));
        assert!(entry.unsupported_rules.iter().any(|rule| {
            rule.value == "//article" && rule.offline_accepted && rule.offline_steps == 1
        }));
        assert!(entry
            .unsupported_rules
            .iter()
            .any(|rule| rule.value == "@xpath=//h2" && !rule.offline_accepted));
        assert!(entry
            .unsupported_rules
            .iter()
            .all(|rule| rule.reason.contains("已保留原表达式")));
        assert!(entry
            .unsupported_rules
            .iter()
            .all(|rule| rule.offline_elapsed_us >= 1));
        assert!(entry.error.is_none());
    }

    #[test]
    fn accepts_string_wrappers() {
        let source = json!({
            "bookSourceName": "String wrapper",
            "searchUrl": "https://string.test?q={{key}}"
        });
        let payload = json!({ "data": source.to_string() });
        let imported = parse_import_bundle(&payload.to_string()).expect("string wrapper");
        assert_eq!(imported.len(), 1);
        assert_eq!(
            entry_name(&json!({ "config_json": imported[0].config_json })),
            Some("String wrapper".to_string())
        );
    }

    #[test]
    fn accepts_a_raw_source_array() {
        let payload = json!([
            {
                "bookSourceName": "One",
                "searchUrl": "https://one.test?q={{key}}"
            },
            {
                "bookSourceName": "Two",
                "searchUrl": "https://two.test?q={{key}}"
            }
        ]);
        let imported = parse_import_bundle(&payload.to_string()).expect("source array");
        assert_eq!(imported.len(), 2);
        assert_eq!(imported[1].enabled, true);
    }
}
