use crate::source::BookSource;
use serde_json::{json, Map, Value};

#[derive(Debug, Clone)]
pub struct ImportedSource {
    pub id: Option<String>,
    pub enabled: bool,
    pub config_json: String,
}

/// Parse the native export format, a raw source object, or an array of
/// Legado-compatible source objects. Only the safe CSS/HTTP subset is mapped.
pub fn parse_import_bundle(input: &str) -> Result<Vec<ImportedSource>, String> {
    let input = input.trim_start_matches('\u{feff}');
    let value: Value =
        serde_json::from_str(input).map_err(|error| format!("书源文件 JSON 无效：{error}"))?;

    let imported = match value {
        Value::Array(entries) => parse_entries(&entries)?,
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
            let entries = object
                .remove("sources")
                .and_then(|value| value.as_array().cloned())
                .ok_or_else(|| "书源文件缺少 sources 数组".to_string())?;
            parse_entries(&entries)?
        }
        Value::Object(object) => match extract_wrapper_entries(&object)? {
            Some(entries) => parse_entries(&entries)?,
            None => vec![parse_entry(&Value::Object(object), 0)?],
        },
        _ => return Err("书源文件必须是对象或数组".to_string()),
    };

    if imported.is_empty() {
        return Err("书源文件没有可导入的配置".to_string());
    }
    Ok(imported)
}

fn extract_wrapper_entries(object: &Map<String, Value>) -> Result<Option<Vec<Value>>, String> {
    for key in ["sources", "bookSources", "booksources", "items", "data"] {
        let Some(value) = object.get(key) else {
            continue;
        };
        if let Some(entries) = value.as_array() {
            return Ok(Some(entries.clone()));
        }
        if key == "data" {
            if let Some(nested) = value.as_object() {
                return extract_wrapper_entries(nested);
            }
        }
        return Err(format!("书源文件 {key} 必须是数组"));
    }
    Ok(None)
}

fn parse_entries(entries: &[Value]) -> Result<Vec<ImportedSource>, String> {
    entries
        .iter()
        .enumerate()
        .map(|(index, entry)| parse_entry(entry, index))
        .collect()
}

fn parse_entry(value: &Value, index: usize) -> Result<ImportedSource, String> {
    let object = value
        .as_object()
        .ok_or_else(|| format!("第 {} 个书源必须是对象", index + 1))?;
    let id = optional_text(object.get("id")).or_else(|| optional_text(object.get("sourceId")));
    let enabled = object
        .get("enabled")
        .and_then(Value::as_bool)
        .unwrap_or(true);

    if let Some(config) = object
        .get("config_json")
        .or_else(|| object.get("configJson"))
    {
        let config_value = match config {
            Value::String(text) => serde_json::from_str::<Value>(text)
                .map_err(|error| format!("第 {} 个书源 config_json 无效：{error}", index + 1))?,
            other => other.clone(),
        };
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
        Value::String(normalize_url(&search_url)),
    );

    for (target, keys) in [
        ("bookInfoUrl", &["bookInfoUrl", "book_info_url"][..]),
        ("tocUrl", &["tocUrl", "toc_url"][..]),
        ("contentUrl", &["contentUrl", "content_url"][..]),
    ] {
        if let Some(value) = first_value(object, keys) {
            let url = value
                .as_str()
                .ok_or_else(|| format!("{target} 必须是字符串；当前只支持 HTTP/CSS 书源"))?;
            output.insert(target.to_string(), Value::String(normalize_url(url)));
        }
    }

    if let Some(rules) = first_value(object, &["search", "ruleSearch"]) {
        output.insert(
            "search".to_string(),
            normalize_page_rules(rules, "ruleSearch", &["bookList", "item"])?,
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
            normalize_page_rules(rules, "ruleToc", &["chapterList", "item"])?,
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
        let selector = item
            .as_str()
            .ok_or_else(|| format!("{stage}.item 必须是 CSS 选择器字符串"))?;
        output.insert(
            "item".to_string(),
            Value::String(normalize_selector(selector, &format!("{stage}.item"))?),
        );
    }

    for (target, keys) in [
        ("title", &["title", "name", "chapterName"][..]),
        ("author", &["author"][..]),
        ("url", &["url", "bookUrl", "chapterUrl"][..]),
        ("intro", &["intro", "desc", "description"][..]),
        ("content", &["content"][..]),
    ] {
        if let Some(rule) = first_value(object, keys) {
            output.insert(
                target.to_string(),
                normalize_rule(rule, &format!("{stage}.{target}"))?,
            );
        }
    }

    Ok(Value::Object(output))
}

fn normalize_rule(value: &Value, context: &str) -> Result<Value, String> {
    match value {
        Value::String(raw) => {
            let (selector, attr) = parse_legado_rule(raw, context)?;
            if let Some(attr) = attr {
                Ok(json!({ "selector": selector, "attr": attr }))
            } else {
                Ok(Value::String(selector))
            }
        }
        Value::Object(object) => {
            let raw_selector = first_value(object, &["selector", "rule", "value"])
                .and_then(Value::as_str)
                .ok_or_else(|| format!("{context} 缺少 selector"))?;
            let (selector, parsed_attr) = parse_legado_rule(raw_selector, context)?;
            let attr = if let Some(raw_attr) = object.get("attr").and_then(Value::as_str) {
                let normalized = raw_attr.trim().to_ascii_lowercase();
                if !matches!(
                    normalized.as_str(),
                    "text" | "content" | "href" | "src" | "title" | "alt" | "data-src"
                ) {
                    return Err(format!(
                        "{context} 的属性后缀 @{} 不在安全子集内",
                        raw_attr.trim()
                    ));
                }
                normalize_attr(raw_attr)
            } else {
                parsed_attr
            };
            let mut output = Map::new();
            output.insert("selector".to_string(), Value::String(selector));
            if let Some(attr) = attr {
                output.insert("attr".to_string(), Value::String(attr));
            }
            if let Some(regex) = object.get("regex").and_then(Value::as_str) {
                output.insert("regex".to_string(), Value::String(regex.to_string()));
            }
            Ok(Value::Object(output))
        }
        _ => Err(format!("{context} 必须是字符串或规则对象")),
    }
}

fn parse_legado_rule(raw: &str, context: &str) -> Result<(String, Option<String>), String> {
    let first = raw.split("||").next().unwrap_or_default().trim();
    if first.is_empty() {
        return Err(format!("{context} 不能为空"));
    }
    let lowered = first.to_ascii_lowercase();
    if lowered.starts_with("//")
        || lowered.starts_with("xpath:")
        || lowered.contains("@js")
        || lowered.contains("@put:")
        || lowered.contains("@get:")
    {
        return Err(format!(
            "{context} 使用了不受支持的 XPath/脚本规则；请改写为 CSS 选择器"
        ));
    }

    let (selector, suffix) = match first.rsplit_once('@') {
        Some((selector, suffix)) if !selector.trim().is_empty() => {
            (selector.trim(), Some(suffix.trim()))
        }
        _ => (first, None),
    };
    let selector = normalize_selector(selector, context)?;
    let attr = suffix.and_then(normalize_attr);
    if let Some(suffix) = suffix {
        if attr.is_none() && !suffix.eq_ignore_ascii_case("text") {
            return Err(format!("{context} 的属性后缀 @{suffix} 不在安全子集内"));
        }
    }
    Ok((selector, attr))
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
    for (prefix, replacement) in [("class.", "."), ("id.", "#"), ("tag.", "")] {
        if let Some(rest) = selector.strip_prefix(prefix) {
            selector = format!("{replacement}{rest}");
            break;
        }
    }
    if selector.starts_with("css:") {
        selector = selector.trim_start_matches("css:").trim().to_string();
    }
    if selector.contains("&&") || selector.contains("||") {
        return Err(format!(
            "{context} 包含多段未实现的规则表达式；请改写为单个 CSS 选择器"
        ));
    }
    Ok(selector)
}

fn normalize_attr(value: &str) -> Option<String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "text" | "content" => None,
        "href" => Some("href".to_string()),
        "src" => Some("src".to_string()),
        "title" => Some("title".to_string()),
        "alt" => Some("alt".to_string()),
        "data-src" => Some("data-src".to_string()),
        _ => None,
    }
}

fn normalize_headers(value: &Value) -> Result<Value, String> {
    let object = value
        .as_object()
        .ok_or_else(|| "headers/header 目前只支持 JSON 对象".to_string())?;
    let mut headers = Map::new();
    for (key, value) in object {
        let text = value
            .as_str()
            .ok_or_else(|| format!("headers.{key} 必须是字符串"))?;
        headers.insert(key.clone(), Value::String(text.to_string()));
    }
    Ok(Value::Object(headers))
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

fn normalize_url(value: &str) -> String {
    value
        .split("||")
        .next()
        .unwrap_or_default()
        .trim()
        .replace("{{page}}", "1")
        .replace("{{pageNum}}", "1")
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
    fn rejects_unsafe_legado_script_rule() {
        let payload = json!({
            "bookSourceName": "Unsafe",
            "searchUrl": "https://example.test/search?q={{key}}",
            "ruleSearch": { "bookList": "li", "name": "@js:java.return 'x'" }
        });
        let error = parse_import_bundle(&payload.to_string()).expect_err("unsafe rule");
        assert!(error.contains("不受支持"));
    }

    #[test]
    fn rejects_invalid_and_empty_documents() {
        let invalid = parse_import_bundle("{").expect_err("invalid json");
        assert!(invalid.contains("JSON 无效"));

        let empty = parse_import_bundle("[]").expect_err("empty array");
        assert!(empty.contains("没有可导入"));
    }

    #[test]
    fn rejects_unknown_rule_attributes() {
        let payload = json!({
            "bookSourceName": "Unknown attribute",
            "searchUrl": "https://example.test/search?q={{key}}",
            "ruleSearch": { "bookList": "li", "name": "h2@html" }
        });
        let error = parse_import_bundle(&payload.to_string()).expect_err("unknown attr");
        assert!(error.contains("安全子集"));
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
