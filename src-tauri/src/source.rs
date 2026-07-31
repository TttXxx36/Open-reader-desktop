use regex::Regex;
use reqwest::header::CONTENT_TYPE;
use scraper::{ElementRef, Html, Selector};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::{HashMap, HashSet},
    time::{Duration, Instant},
};
use thiserror::Error;
use url::{form_urlencoded, Url};

const DEFAULT_TIMEOUT_SECS: u64 = 15;
const DEFAULT_MAX_BODY_BYTES: usize = 2 * 1024 * 1024;
const MAX_SEARCH_RESULTS: usize = 100;

#[derive(Debug, Error)]
pub enum SourceError {
    #[error("invalid source configuration: {0}")]
    InvalidConfig(String),
    #[error("invalid URL: {0}")]
    InvalidUrl(String),
    #[error("unsupported URL scheme: {0}")]
    UnsupportedScheme(String),
    #[error("invalid CSS selector: {0}")]
    InvalidSelector(String),
    #[error("invalid regular expression: {0}")]
    InvalidRegex(String),
    #[error("request failed: {0}")]
    Request(#[from] reqwest::Error),
    #[error("response body exceeds {0} bytes")]
    BodyTooLarge(usize),
    #[error("no value matched the source rule")]
    NoMatch,
    #[error("invalid JSON path: {0}")]
    InvalidJsonPath(String),
    #[error("invalid JSON response: {0}")]
    InvalidJson(String),
    #[error("{stage} 阶段失败：{message}")]
    Pipeline { stage: String, message: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookSource {
    pub name: String,
    #[serde(alias = "searchUrl")]
    pub search_url: String,
    #[serde(default, alias = "bookInfoUrl")]
    pub book_info_url: Option<String>,
    #[serde(default, alias = "tocUrl")]
    pub toc_url: Option<String>,
    #[serde(default, alias = "contentUrl")]
    pub content_url: Option<String>,
    #[serde(default, alias = "ruleSearch")]
    pub search: Option<PageRules>,
    #[serde(default, alias = "ruleBookInfo", alias = "bookInfo")]
    pub book_info: Option<PageRules>,
    #[serde(default, alias = "ruleToc", alias = "toc")]
    pub toc: Option<PageRules>,
    #[serde(default, alias = "ruleContent", alias = "content")]
    pub content: Option<PageRules>,
    #[serde(default)]
    pub headers: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PageRules {
    #[serde(default)]
    pub item: Option<String>,
    #[serde(default)]
    pub title: Option<SourceRule>,
    #[serde(default)]
    pub author: Option<SourceRule>,
    #[serde(default, alias = "bookUrl")]
    pub url: Option<SourceRule>,
    #[serde(default)]
    pub intro: Option<SourceRule>,
    #[serde(default)]
    pub content: Option<SourceRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SourceRule {
    Selector(String),
    Detailed {
        selector: String,
        #[serde(default)]
        attr: Option<String>,
        #[serde(default)]
        regex: Option<String>,
    },
}

impl SourceRule {
    fn selector(&self) -> &str {
        match self {
            Self::Selector(value) => value,
            Self::Detailed { selector, .. } => selector,
        }
    }

    fn attr(&self) -> Option<&str> {
        match self {
            Self::Selector(_) => None,
            Self::Detailed { attr, .. } => attr.as_deref(),
        }
    }

    fn regex(&self) -> Option<&str> {
        match self {
            Self::Selector(_) => None,
            Self::Detailed { regex, .. } => regex.as_deref(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct SourceValidation {
    pub valid: bool,
    pub source: Option<BookSource>,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SearchResult {
    pub title: String,
    pub author: Option<String>,
    pub book_url: Option<String>,
    pub source_name: String,
}

#[derive(Debug, Clone)]
pub struct SourceDefinition {
    pub id: String,
    pub name: String,
    pub source: BookSource,
}

#[derive(Debug, Clone, Serialize)]
pub struct UnifiedSearchResult {
    pub source_id: String,
    pub source_name: String,
    pub title: String,
    pub author: Option<String>,
    pub book_url: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SourceSearchFailure {
    pub source_id: String,
    pub source_name: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct MultiSourceSearchResult {
    pub results: Vec<UnifiedSearchResult>,
    pub failures: Vec<SourceSearchFailure>,
    pub enabled_sources: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct BookInfo {
    pub title: String,
    pub author: Option<String>,
    pub intro: Option<String>,
    pub cover_url: Option<String>,
    pub book_url: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SourceChapter {
    pub title: String,
    pub url: String,
    pub index: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct SourceChapterContent {
    pub title: String,
    pub content: String,
    pub next_url: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SourcePreview {
    pub status: u16,
    pub content_type: Option<String>,
    pub bytes: usize,
    pub body_preview: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SourceDebugStep {
    pub stage: String,
    pub url: String,
    pub duration_ms: u64,
    pub status: Option<u16>,
    pub bytes: Option<usize>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SourcePipelineResult {
    pub search_results: Vec<SearchResult>,
    pub book_info: BookInfo,
    pub chapters: Vec<SourceChapter>,
    pub first_chapter: SourceChapterContent,
    pub debug_steps: Vec<SourceDebugStep>,
}

struct FetchedText {
    body: String,
    status: u16,
    bytes: usize,
}

#[derive(Clone)]
pub struct SourceEngine {
    client: reqwest::Client,
    max_body_bytes: usize,
}

impl SourceEngine {
    pub fn new(timeout_secs: u64, max_body_bytes: usize) -> Result<Self, SourceError> {
        if timeout_secs == 0 {
            return Err(SourceError::InvalidConfig(
                "timeout must be greater than zero".to_string(),
            ));
        }
        if max_body_bytes == 0 {
            return Err(SourceError::InvalidConfig(
                "max_body_bytes must be greater than zero".to_string(),
            ));
        }

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(timeout_secs))
            .user_agent("OpenReaderDesktop/0.1")
            .build()?;

        Ok(Self {
            client,
            max_body_bytes,
        })
    }

    pub fn default() -> Result<Self, SourceError> {
        Self::new(DEFAULT_TIMEOUT_SECS, DEFAULT_MAX_BODY_BYTES)
    }

    pub async fn fetch(&self, url: &str) -> Result<SourcePreview, SourceError> {
        validate_url(url)?;
        let response = self.client.get(expand_url_template(url)).send().await?;
        let status = response.status().as_u16();
        let content_type = response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(str::to_string);

        let mut body = Vec::new();
        let mut response = response.error_for_status()?;
        while let Some(chunk) = response.chunk().await? {
            if body.len() + chunk.len() > self.max_body_bytes {
                return Err(SourceError::BodyTooLarge(self.max_body_bytes));
            }
            body.extend_from_slice(&chunk);
        }

        let preview = String::from_utf8_lossy(&body).chars().take(2_000).collect();

        Ok(SourcePreview {
            status,
            content_type,
            bytes: body.len(),
            body_preview: preview,
        })
    }

    pub async fn search(
        &self,
        source: &BookSource,
        keyword: &str,
    ) -> Result<Vec<SearchResult>, SourceError> {
        let search_url = render_url(&source.search_url, Some(keyword), None, None, None);
        let fetched = self
            .fetch_text(&search_url, &source.headers)
            .await
            .map_err(|error| pipeline_error("search_fetch", error))?;
        let mut results = self
            .parse_search_html(source, &fetched.body)
            .map_err(|error| pipeline_error("search_parse", error))?;
        for result in &mut results {
            if let Some(url) = &result.book_url {
                result.book_url = Some(absolutize_url(&search_url, url));
            }
        }
        Ok(results)
    }

    pub async fn search_many(
        &self,
        sources: Vec<SourceDefinition>,
        keyword: &str,
    ) -> MultiSourceSearchResult {
        let enabled_sources = sources.len();
        let mut tasks = tokio::task::JoinSet::new();

        for definition in sources {
            let engine = self.clone();
            let keyword = keyword.to_string();
            tasks.spawn(async move {
                let result = engine.search(&definition.source, &keyword).await;
                (definition, result)
            });
        }

        let mut results = Vec::new();
        let mut failures = Vec::new();

        while let Some(joined) = tasks.join_next().await {
            match joined {
                Ok((definition, Ok(items))) => {
                    results.extend(items.into_iter().map(|item| UnifiedSearchResult {
                        source_id: definition.id.clone(),
                        source_name: definition.name.clone(),
                        title: item.title,
                        author: item.author,
                        book_url: item.book_url,
                    }));
                }
                Ok((definition, Err(error))) => failures.push(SourceSearchFailure {
                    source_id: definition.id,
                    source_name: definition.name,
                    message: error.to_string(),
                }),
                Err(error) => failures.push(SourceSearchFailure {
                    source_id: "unknown".to_string(),
                    source_name: "未知书源".to_string(),
                    message: format!("搜索任务异常：{}", error),
                }),
            }
        }

        MultiSourceSearchResult {
            results: dedupe_search_results(results),
            failures,
            enabled_sources,
        }
    }

    pub async fn run_pipeline(
        &self,
        source: &BookSource,
        keyword: &str,
    ) -> Result<SourcePipelineResult, SourceError> {
        let mut debug_steps = Vec::new();
        let search_url = render_url(&source.search_url, Some(keyword), None, None, None);
        let search_body = self
            .fetch_stage("search", &search_url, &source.headers, &mut debug_steps)
            .await?;
        let mut search_results = self
            .parse_search_html(source, &search_body)
            .map_err(|error| pipeline_error("search_parse", error))?;
        for result in &mut search_results {
            if let Some(url) = &result.book_url {
                result.book_url = Some(absolutize_url(&search_url, url));
            }
        }

        let first_result = search_results
            .first()
            .ok_or_else(|| pipeline_error("search_parse", SourceError::NoMatch))?;
        let book_url = first_result.book_url.clone().ok_or_else(|| {
            pipeline_error(
                "search_parse",
                SourceError::InvalidConfig("search result has no book URL".to_string()),
            )
        })?;
        let book_info = self
            .fetch_book_info(source, &book_url, &mut debug_steps)
            .await?;
        let chapters = self.fetch_toc(source, &book_url, &mut debug_steps).await?;
        let first_chapter = chapters
            .first()
            .ok_or_else(|| pipeline_error("toc_parse", SourceError::NoMatch))?;
        let first_chapter_content = self
            .fetch_chapter_content(source, first_chapter, &mut debug_steps)
            .await?;

        Ok(SourcePipelineResult {
            search_results,
            book_info,
            chapters,
            first_chapter: first_chapter_content,
            debug_steps,
        })
    }

    async fn fetch_stage(
        &self,
        stage: &str,
        url: &str,
        headers: &HashMap<String, String>,
        debug_steps: &mut Vec<SourceDebugStep>,
    ) -> Result<String, SourceError> {
        let started = Instant::now();
        match self.fetch_text(url, headers).await {
            Ok(response) => {
                debug_steps.push(SourceDebugStep {
                    stage: stage.to_string(),
                    url: redact_url(url),
                    duration_ms: started.elapsed().as_millis() as u64,
                    status: Some(response.status),
                    bytes: Some(response.bytes),
                    error: None,
                });
                Ok(response.body)
            }
            Err(error) => {
                let message = error.to_string();
                debug_steps.push(SourceDebugStep {
                    stage: stage.to_string(),
                    url: redact_url(url),
                    duration_ms: started.elapsed().as_millis() as u64,
                    status: None,
                    bytes: None,
                    error: Some(message.clone()),
                });
                Err(pipeline_error(stage, error))
            }
        }
    }

    async fn fetch_text(
        &self,
        url: &str,
        headers: &HashMap<String, String>,
    ) -> Result<FetchedText, SourceError> {
        validate_url(url)?;
        let mut request = self.client.get(expand_url_template(url));
        for (name, value) in headers {
            request = request.header(name, value);
        }
        let response = request.send().await?;
        let status = response.status().as_u16();
        let mut response = response.error_for_status()?;
        let mut body = Vec::new();

        while let Some(chunk) = response.chunk().await? {
            if body.len() + chunk.len() > self.max_body_bytes {
                return Err(SourceError::BodyTooLarge(self.max_body_bytes));
            }
            body.extend_from_slice(&chunk);
        }

        Ok(FetchedText {
            status,
            bytes: body.len(),
            body: String::from_utf8_lossy(&body).into_owned(),
        })
    }

    async fn fetch_book_info(
        &self,
        source: &BookSource,
        book_url: &str,
        debug_steps: &mut Vec<SourceDebugStep>,
    ) -> Result<BookInfo, SourceError> {
        let template = source
            .book_info_url
            .as_deref()
            .ok_or_else(|| SourceError::InvalidConfig("bookInfoUrl is required".to_string()))?;
        let url = render_url(
            template,
            None,
            Some(book_url),
            Some(last_path_segment(book_url)),
            None,
        );
        let body = self
            .fetch_stage("book_info", &url, &source.headers, debug_steps)
            .await?;
        let rules = source
            .book_info
            .as_ref()
            .ok_or_else(|| SourceError::InvalidConfig("bookInfo rules are required".to_string()))?;
        let document = Html::parse_document(&body);

        Ok(BookInfo {
            title: extract_document_rule(&document, rules.title.as_ref())?
                .unwrap_or_else(|| "未命名书籍".to_string()),
            author: non_empty(extract_document_rule(&document, rules.author.as_ref())?),
            intro: non_empty(extract_document_rule(&document, rules.intro.as_ref())?),
            cover_url: non_empty(extract_document_rule(&document, rules.url.as_ref())?),
            book_url: book_url.to_string(),
        })
    }

    async fn fetch_toc(
        &self,
        source: &BookSource,
        book_url: &str,
        debug_steps: &mut Vec<SourceDebugStep>,
    ) -> Result<Vec<SourceChapter>, SourceError> {
        let template = source
            .toc_url
            .as_deref()
            .ok_or_else(|| SourceError::InvalidConfig("tocUrl is required".to_string()))?;
        let url = render_url(
            template,
            None,
            Some(book_url),
            Some(last_path_segment(book_url)),
            None,
        );
        let body = self
            .fetch_stage("toc", &url, &source.headers, debug_steps)
            .await?;
        let rules = source
            .toc
            .as_ref()
            .ok_or_else(|| SourceError::InvalidConfig("toc rules are required".to_string()))?;
        parse_chapter_list(rules, &body, &url)
    }

    async fn fetch_chapter_content(
        &self,
        source: &BookSource,
        chapter: &SourceChapter,
        debug_steps: &mut Vec<SourceDebugStep>,
    ) -> Result<SourceChapterContent, SourceError> {
        let template = source
            .content_url
            .as_deref()
            .ok_or_else(|| SourceError::InvalidConfig("contentUrl is required".to_string()))?;
        let url = render_url(
            template,
            None,
            None,
            None,
            Some(last_path_segment(&chapter.url)),
        );
        let body = self
            .fetch_stage("content", &url, &source.headers, debug_steps)
            .await?;
        let rules = source
            .content
            .as_ref()
            .ok_or_else(|| SourceError::InvalidConfig("content rules are required".to_string()))?;
        let document = Html::parse_document(&body);
        let content = extract_document_rule(&document, rules.content.as_ref())?
            .ok_or(SourceError::NoMatch)?;

        Ok(SourceChapterContent {
            title: chapter.title.clone(),
            content,
            next_url: None,
        })
    }

    pub fn extract_html_value(&self, html: &str, rule: &SourceRule) -> Result<String, SourceError> {
        let document = Html::parse_document(html);
        let selector = parse_selector(rule.selector())?;
        let element = document
            .select(&selector)
            .next()
            .ok_or(SourceError::NoMatch)?;
        extract_selected_element(element, rule)
    }

    pub fn extract_json_values(&self, json: &str, path: &str) -> Result<Vec<String>, SourceError> {
        let value: Value = serde_json::from_str(json)
            .map_err(|error| SourceError::InvalidJson(error.to_string()))?;
        extract_json_path(&value, path)
    }

    pub fn parse_search_html(
        &self,
        source: &BookSource,
        html: &str,
    ) -> Result<Vec<SearchResult>, SourceError> {
        let rules = source
            .search
            .as_ref()
            .ok_or_else(|| SourceError::InvalidConfig("search rules are required".to_string()))?;
        let item_selector = parse_selector(rules.item.as_deref().unwrap_or("body"))?;
        let document = Html::parse_document(html);
        let mut results = Vec::new();

        for item in document.select(&item_selector).take(MAX_SEARCH_RESULTS) {
            let title = rules
                .title
                .as_ref()
                .map(|rule| extract_from_element(item, rule))
                .transpose()?
                .unwrap_or_default();
            let author = rules
                .author
                .as_ref()
                .map(|rule| extract_from_element(item, rule))
                .transpose()?;
            let book_url = rules
                .url
                .as_ref()
                .map(|rule| extract_from_element(item, rule))
                .transpose()?;

            if !title.is_empty() || book_url.is_some() {
                results.push(SearchResult {
                    title,
                    author: non_empty(author),
                    book_url: non_empty(book_url),
                    source_name: source.name.clone(),
                });
            }
        }

        Ok(results)
    }
}

fn dedupe_search_results(mut results: Vec<UnifiedSearchResult>) -> Vec<UnifiedSearchResult> {
    results.sort_by(|left, right| {
        (
            normalize_search_text(&left.title),
            normalize_search_text(left.author.as_deref().unwrap_or_default()),
            normalize_search_text(&left.source_name),
            left.source_id.clone(),
        )
            .cmp(&(
                normalize_search_text(&right.title),
                normalize_search_text(right.author.as_deref().unwrap_or_default()),
                normalize_search_text(&right.source_name),
                right.source_id.clone(),
            ))
    });

    let mut seen = HashSet::new();
    results
        .into_iter()
        .filter(|item| seen.insert(search_result_key(item)))
        .collect()
}

fn search_result_key(item: &UnifiedSearchResult) -> String {
    let title = normalize_search_text(&item.title);
    let author = normalize_search_text(item.author.as_deref().unwrap_or_default());
    if title.is_empty() && author.is_empty() {
        return format!(
            "url:{}",
            normalize_search_text(item.book_url.as_deref().unwrap_or_default())
        );
    }
    format!("{}|{}", title, author)
}

fn normalize_search_text(value: &str) -> String {
    value.split_whitespace().collect::<String>().to_lowercase()
}

fn pipeline_error(stage: &str, error: SourceError) -> SourceError {
    SourceError::Pipeline {
        stage: stage.to_string(),
        message: error.to_string(),
    }
}

fn redact_url(value: &str) -> String {
    let Ok(mut parsed) = Url::parse(value) else {
        return value.to_string();
    };
    parsed.set_query(None);
    parsed.set_fragment(None);
    parsed.to_string()
}

fn extract_document_rule(
    document: &Html,
    rule: Option<&SourceRule>,
) -> Result<Option<String>, SourceError> {
    let Some(rule) = rule else {
        return Ok(None);
    };
    let selector = parse_selector(rule.selector())?;
    let Some(element) = document.select(&selector).next() else {
        return Ok(None);
    };
    Ok(Some(extract_selected_element(element, rule)?))
}

fn parse_chapter_list(
    rules: &PageRules,
    body: &str,
    base_url: &str,
) -> Result<Vec<SourceChapter>, SourceError> {
    let document = Html::parse_document(body);
    let item_selector = parse_selector(rules.item.as_deref().unwrap_or("body"))?;
    let mut chapters = Vec::new();

    for (index, item) in document.select(&item_selector).enumerate() {
        let Some(title) = extract_document_rule_from_element(item, rules.title.as_ref())? else {
            continue;
        };
        let url = extract_document_rule_from_element(item, rules.url.as_ref())?
            .map(|value| absolutize_url(base_url, &value))
            .unwrap_or_else(|| format!("{base_url}#chapter-{index}"));
        chapters.push(SourceChapter { title, url, index });
    }

    Ok(chapters)
}

fn extract_document_rule_from_element(
    element: ElementRef<'_>,
    rule: Option<&SourceRule>,
) -> Result<Option<String>, SourceError> {
    let Some(rule) = rule else {
        return Ok(None);
    };
    Ok(Some(extract_from_element(element, rule)?))
}

fn render_url(
    template: &str,
    keyword: Option<&str>,
    book_url: Option<&str>,
    book_id: Option<&str>,
    chapter_id: Option<&str>,
) -> String {
    let mut result = template.to_string();
    if let Some(keyword) = keyword {
        result = result
            .replace("{{keyword}}", &encode_keyword(keyword))
            .replace("{{key}}", &encode_keyword(keyword));
    }
    if let Some(book_url) = book_url {
        result = result
            .replace("{{bookUrl}}", book_url)
            .replace("{{book_url}}", book_url);
    }
    if let Some(book_id) = book_id {
        result = result
            .replace("{{bookId}}", book_id)
            .replace("{{book_id}}", book_id);
    }
    if let Some(chapter_id) = chapter_id {
        result = result
            .replace("{{chapterId}}", chapter_id)
            .replace("{{chapter_id}}", chapter_id);
    }
    result
}

fn encode_keyword(value: &str) -> String {
    form_urlencoded::byte_serialize(value.as_bytes()).collect()
}

fn last_path_segment(value: &str) -> &str {
    value
        .trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or_default()
        .split('?')
        .next()
        .unwrap_or_default()
}

fn absolutize_url(base_url: &str, value: &str) -> String {
    if value.starts_with("http://") || value.starts_with("https://") {
        return value.to_string();
    }
    Url::parse(base_url)
        .ok()
        .and_then(|base| base.join(value).ok())
        .map(|url| url.to_string())
        .unwrap_or_else(|| value.to_string())
}

pub fn validate_source_json(input: &str) -> SourceValidation {
    let source = match serde_json::from_str::<BookSource>(input) {
        Ok(source) => source,
        Err(error) => {
            return SourceValidation {
                valid: false,
                source: None,
                errors: vec![format!("JSON 解析失败：{error}")],
                warnings: Vec::new(),
            };
        }
    };

    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    if source.name.trim().is_empty() {
        errors.push("name 不能为空".to_string());
    }
    validate_endpoint("searchUrl", &source.search_url, &mut errors);
    for (name, value) in [
        ("bookInfoUrl", source.book_info_url.as_deref()),
        ("tocUrl", source.toc_url.as_deref()),
        ("contentUrl", source.content_url.as_deref()),
    ] {
        if let Some(value) = value {
            validate_endpoint(name, value, &mut errors);
        }
    }

    validate_page_rules(
        "ruleSearch",
        source.search.as_ref(),
        &mut errors,
        &mut warnings,
    );
    validate_page_rules(
        "ruleBookInfo",
        source.book_info.as_ref(),
        &mut errors,
        &mut warnings,
    );
    validate_page_rules("ruleToc", source.toc.as_ref(), &mut errors, &mut warnings);
    validate_page_rules(
        "ruleContent",
        source.content.as_ref(),
        &mut errors,
        &mut warnings,
    );

    if source.book_info_url.is_none() {
        warnings.push("未配置 bookInfoUrl，端到端流程无法完成详情链路".to_string());
    }
    if source.toc_url.is_none() {
        warnings.push("未配置 tocUrl，端到端流程无法完成目录链路".to_string());
    }
    if source.content_url.is_none() {
        warnings.push("未配置 contentUrl，端到端流程无法完成正文链路".to_string());
    }
    for header in source.headers.keys() {
        let normalized = header.to_ascii_lowercase();
        if matches!(
            normalized.as_str(),
            "authorization" | "cookie" | "proxy-authorization"
        ) {
            errors.push(format!("headers 不允许携带敏感认证头：{header}"));
        }
    }

    SourceValidation {
        valid: errors.is_empty(),
        source: Some(source),
        errors,
        warnings,
    }
}

fn validate_endpoint(name: &str, value: &str, errors: &mut Vec<String>) {
    if value.trim().is_empty() {
        errors.push(format!("{name} 不能为空"));
        return;
    }

    if let Err(error) = validate_url(value) {
        errors.push(format!("{name}：{error}"));
    }
}

fn validate_page_rules(
    name: &str,
    rules: Option<&PageRules>,
    errors: &mut Vec<String>,
    warnings: &mut Vec<String>,
) {
    let Some(rules) = rules else {
        warnings.push(format!("未配置 {name}"));
        return;
    };

    if let Some(item) = &rules.item {
        if let Err(error) = parse_selector(item) {
            errors.push(format!("{name}.item：{error}"));
        }
    }

    for (field, rule) in [
        ("title", rules.title.as_ref()),
        ("author", rules.author.as_ref()),
        ("url", rules.url.as_ref()),
        ("intro", rules.intro.as_ref()),
        ("content", rules.content.as_ref()),
    ] {
        if let Some(rule) = rule {
            if rule.selector().trim().is_empty() {
                errors.push(format!("{name}.{field} selector 不能为空"));
                continue;
            }
            if let Err(error) = parse_selector(rule.selector()) {
                errors.push(format!("{name}.{field}：{error}"));
            }
            if let Some(regex) = rule.regex() {
                if let Err(error) = Regex::new(regex) {
                    errors.push(format!("{name}.{field} regex：{error}"));
                }
            }
        }
    }
}

fn validate_url(url: &str) -> Result<(), SourceError> {
    let parsed = Url::parse(&expand_url_template(url))
        .map_err(|error| SourceError::InvalidUrl(error.to_string()))?;
    match parsed.scheme() {
        "http" | "https" => Ok(()),
        scheme => Err(SourceError::UnsupportedScheme(scheme.to_string())),
    }
}

fn expand_url_template(url: &str) -> String {
    url.replace("{{keyword}}", "open-reader")
        .replace("{{page}}", "1")
}

fn parse_selector(value: &str) -> Result<Selector, SourceError> {
    Selector::parse(value).map_err(|error| SourceError::InvalidSelector(format!("{error:?}")))
}

fn extract_from_element(element: ElementRef<'_>, rule: &SourceRule) -> Result<String, SourceError> {
    let selector = parse_selector(rule.selector())?;
    let target = element
        .select(&selector)
        .next()
        .ok_or(SourceError::NoMatch)?;
    extract_selected_element(target, rule)
}

fn extract_selected_element(
    element: ElementRef<'_>,
    rule: &SourceRule,
) -> Result<String, SourceError> {
    let value = if let Some(attribute) = rule.attr() {
        element
            .value()
            .attr(attribute)
            .unwrap_or_default()
            .to_string()
    } else {
        element.text().collect::<Vec<_>>().join(" ")
    };

    apply_regex(value.trim(), rule.regex())
}

fn apply_regex(value: &str, pattern: Option<&str>) -> Result<String, SourceError> {
    let Some(pattern) = pattern else {
        return Ok(value.to_string());
    };
    let regex =
        Regex::new(pattern).map_err(|error| SourceError::InvalidRegex(error.to_string()))?;
    let captures = regex.captures(value).ok_or(SourceError::NoMatch)?;
    Ok(captures
        .get(1)
        .or_else(|| captures.get(0))
        .map(|capture| capture.as_str().to_string())
        .unwrap_or_default())
}

fn extract_json_path(value: &Value, path: &str) -> Result<Vec<String>, SourceError> {
    let path = path.trim().trim_start_matches("$.").trim_start_matches('$');
    if path.is_empty() {
        return Err(SourceError::InvalidJsonPath(path.to_string()));
    }

    let mut current = vec![value];
    for segment in path.split('.') {
        if segment.is_empty() {
            return Err(SourceError::InvalidJsonPath(path.to_string()));
        }

        let wildcard = segment.ends_with("[*]");
        let key = segment.trim_end_matches("[*]");
        let mut next = Vec::new();

        for item in current {
            if wildcard {
                let Some(array) = item.get(key).and_then(Value::as_array) else {
                    continue;
                };
                next.extend(array);
            } else if let Some(child) = item.get(key) {
                next.push(child);
            } else if let Ok(index) = key.parse::<usize>() {
                if let Some(child) = item.get(index) {
                    next.push(child);
                }
            }
        }
        current = next;
    }

    if current.is_empty() {
        return Err(SourceError::NoMatch);
    }

    Ok(current
        .into_iter()
        .map(|item| match item {
            Value::String(value) => value.clone(),
            other => other.to_string(),
        })
        .collect())
}

fn non_empty(value: Option<String>) -> Option<String> {
    value.filter(|value| !value.trim().is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_the_public_fixture() {
        let result = validate_source_json(include_str!("../fixtures/sample_source.json"));
        assert!(result.valid, "{:?}", result.errors);
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn rejects_invalid_selector_and_regex() {
        let result = validate_source_json(
            r#"{
              "name": "Broken",
              "searchUrl": "https://example.test/search?q={{keyword}}",
              "search": {
                "item": "article[",
                "title": { "selector": "h2", "regex": "[" }
              }
            }"#,
        );
        assert!(!result.valid);
        assert!(result.errors.iter().any(|error| error.contains("selector")));
        assert!(result.errors.iter().any(|error| error.contains("regex")));
    }

    #[test]
    fn extracts_html_search_results() {
        let source: BookSource = serde_json::from_str(
            r#"{
              "name": "Fixture",
              "searchUrl": "https://example.test/search",
              "search": {
                "item": "article.book",
                "title": { "selector": "h2 a" },
                "author": { "selector": ".author" },
                "url": { "selector": "h2 a", "attr": "href" }
              }
            }"#,
        )
        .expect("source should parse");
        let engine = SourceEngine::new(1, 1024).expect("engine should build");
        let results = engine
            .parse_search_html(
                &source,
                r#"<article class="book"><h2><a href="/book/1">第一本</a></h2><span class="author">作者甲</span></article>"#,
            )
            .expect("html should parse");

        assert_eq!(results[0].title, "第一本");
        assert_eq!(results[0].author.as_deref(), Some("作者甲"));
        assert_eq!(results[0].book_url.as_deref(), Some("/book/1"));
    }

    #[test]
    fn deduplicates_search_results_by_title_and_author() {
        let results = dedupe_search_results(vec![
            UnifiedSearchResult {
                source_id: "source-b".to_string(),
                source_name: "书源 B".to_string(),
                title: " 测试 书 ".to_string(),
                author: Some(" 作者甲 ".to_string()),
                book_url: Some("https://b.test/book".to_string()),
            },
            UnifiedSearchResult {
                source_id: "source-a".to_string(),
                source_name: "书源 A".to_string(),
                title: "测试书".to_string(),
                author: Some("作者甲".to_string()),
                book_url: Some("https://a.test/book".to_string()),
            },
            UnifiedSearchResult {
                source_id: "source-a".to_string(),
                source_name: "书源 A".to_string(),
                title: "另一本".to_string(),
                author: None,
                book_url: None,
            },
        ]);

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].title, "另一本");
        assert_eq!(results[1].source_name, "书源 A");
    }

    #[test]
    fn extracts_json_wildcard_values() {
        let engine = SourceEngine::new(1, 1024).expect("engine should build");
        let values = engine
            .extract_json_values(
                r#"{ "books": [{ "title": "第一本" }, { "title": "第二本" }] }"#,
                "$.books[*].title",
            )
            .expect("json should parse");
        assert_eq!(values, vec!["第一本", "第二本"]);
    }

    #[tokio::test]
    async fn searches_multiple_sources_with_failure_isolation() {
        let (base_url, server) = spawn_search_fixture_server();
        let mut valid_source: BookSource = serde_json::from_str(
            r#"{
              "name": "Fixture",
              "searchUrl": "https://example.test/search",
              "search": {
                "item": "article.book",
                "title": { "selector": "h2 a" },
                "author": { "selector": ".author" },
                "url": { "selector": "h2 a", "attr": "href" }
              }
            }"#,
        )
        .expect("source should parse");
        valid_source.search_url = format!("{}/search?q={{{{keyword}}}}", base_url);

        let broken_source: BookSource = serde_json::from_str(
            r#"{
              "name": "Broken",
              "searchUrl": "http://127.0.0.1:1/search?q={{keyword}}",
              "search": {
                "item": "article.book",
                "title": { "selector": "h2 a" }
              }
            }"#,
        )
        .expect("source should parse");

        let engine = SourceEngine::new(1, 1024 * 1024).expect("engine should build");
        let result = engine
            .search_many(
                vec![
                    SourceDefinition {
                        id: "fixture".to_string(),
                        name: "Fixture".to_string(),
                        source: valid_source,
                    },
                    SourceDefinition {
                        id: "broken".to_string(),
                        name: "Broken".to_string(),
                        source: broken_source,
                    },
                ],
                "demo",
            )
            .await;

        assert_eq!(result.enabled_sources, 2);
        assert_eq!(result.results.len(), 1);
        assert_eq!(result.failures.len(), 1);
        assert_eq!(result.results[0].title, "测试书");
        server.join().expect("fixture server should stop");
    }

    #[tokio::test]
    async fn runs_authorized_fixture_pipeline() {
        let (base_url, server) = spawn_fixture_server();
        let mut source: BookSource =
            serde_json::from_str(include_str!("../fixtures/sample_source.json"))
                .expect("fixture source should parse");
        source.search_url = format!("{base_url}/search?q={{{{keyword}}}}");
        source.book_info_url = Some(format!("{base_url}/book/{{{{bookId}}}}"));
        source.toc_url = Some(format!("{base_url}/book/{{{{bookId}}}}/toc"));
        source.content_url = Some(format!("{base_url}/chapter/{{{{chapterId}}}}"));

        let engine = SourceEngine::new(3, 1024 * 1024).expect("engine should build");
        let result = engine
            .run_pipeline(&source, "demo")
            .await
            .expect("fixture pipeline should succeed");

        assert_eq!(result.search_results.len(), 1);
        assert_eq!(result.book_info.title, "测试书");
        assert_eq!(result.chapters[0].title, "第一章");
        assert!(result.first_chapter.content.contains("这是正文"));
        assert_eq!(result.debug_steps.len(), 4);
        assert!(result.debug_steps.iter().all(|step| step.error.is_none()));
        server.join().expect("fixture server should stop");
    }

    fn spawn_search_fixture_server() -> (String, std::thread::JoinHandle<()>) {
        use std::{
            io::{Read, Write},
            net::TcpListener,
            thread,
        };

        let listener = TcpListener::bind("127.0.0.1:0").expect("fixture listener");
        let address = listener.local_addr().expect("fixture address");
        let server = thread::spawn(move || {
            for stream in listener.incoming().take(1) {
                let mut stream = stream.expect("fixture stream");
                let mut buffer = [0_u8; 2048];
                let size = stream.read(&mut buffer).expect("fixture request");
                let request = String::from_utf8_lossy(&buffer[..size]);
                let path = request.split_whitespace().nth(1).unwrap_or("/");
                let body = if path.starts_with("/search") {
                    r#"<article class="book"><h2><a href="/book/1">测试书</a></h2><span class="author">作者甲</span></article>"#
                } else {
                    r#"<article class="book"><h2><a href="/book/2">意外</a></h2></article>"#
                };
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.as_bytes().len(),
                    body
                );
                stream
                    .write_all(response.as_bytes())
                    .expect("fixture response");
            }
        });

        (format!("http://{}", address), server)
    }

    fn spawn_fixture_server() -> (String, std::thread::JoinHandle<()>) {
        use std::{
            io::{Read, Write},
            net::TcpListener,
            thread,
        };

        let listener = TcpListener::bind("127.0.0.1:0").expect("fixture listener");
        let address = listener.local_addr().expect("fixture address");
        let server = thread::spawn(move || {
            for stream in listener.incoming().take(4) {
                let mut stream = stream.expect("fixture stream");
                let mut buffer = [0_u8; 2048];
                let size = stream.read(&mut buffer).expect("fixture request");
                let request = String::from_utf8_lossy(&buffer[..size]);
                let path = request.split_whitespace().nth(1).unwrap_or("/");
                let body = if path.starts_with("/search") {
                    r#"<article class="book"><h2><a href="/book/1">测试书</a></h2><span class="author">作者甲</span></article>"#
                } else if path == "/book/1" {
                    r#"<h1>测试书</h1><div class="author">作者甲</div><p class="intro">一本用于 M4 的公开测试书</p>"#
                } else if path == "/book/1/toc" {
                    r#"<ol class="chapters"><li><a href="/chapter/1">第一章</a></li></ol>"#
                } else {
                    r#"<article class="content">这是正文，用于验证搜索到正文的完整链路。</article>"#
                };
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.as_bytes().len(),
                    body
                );
                stream
                    .write_all(response.as_bytes())
                    .expect("fixture response");
            }
        });

        (format!("http://{address}"), server)
    }
}
