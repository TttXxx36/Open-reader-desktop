use encoding_rs::{GB18030, UTF_16BE, UTF_16LE, WINDOWS_1252};
use regex::Regex;
use reqwest::header::CONTENT_TYPE;
use scraper::{ElementRef, Html, Selector};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    time::{Duration, Instant},
};
use thiserror::Error;
use url::{form_urlencoded, Url};

const DEFAULT_TIMEOUT_SECS: u64 = 15;
const DEFAULT_MAX_BODY_BYTES: usize = 2 * 1024 * 1024;
const MAX_SEARCH_RESULTS: usize = 100;
const MAX_SOURCE_SEARCH_PAGES: usize = 20;
const MAX_RULE_CHAIN_LENGTH: usize = 8;
const MAX_URL_CHAIN_LENGTH: usize = 8;
const MAX_JSON_PATH_BYTES: usize = 512;
const MAX_JSON_MATCHES: usize = 256;
const MAX_JSON_FILTER_FIELD_BYTES: usize = 128;
const MAX_JSON_FILTER_VALUE_BYTES: usize = 256;
const MAX_REPLACE_RULES: usize = 32;
const MAX_REPLACE_PATTERN_BYTES: usize = 512;
const MAX_REPLACE_REPLACEMENT_BYTES: usize = 4 * 1024;
const MAX_PERMISSION_SCOPE_BYTES: usize = 512;
const MAX_PERMISSION_REVIEWED_AT_BYTES: usize = 64;
const MAX_SOURCE_GROUP_BYTES: usize = 128;
const MAX_SOURCE_COMMENT_BYTES: usize = 2 * 1024;
const MAX_BOOK_URL_PATTERN_BYTES: usize = 512;
const MAX_NEXT_URL_BYTES: usize = 2 * 1024;
const MAX_SOURCE_WEIGHT: i64 = 1_000_000;
const MAX_SOURCE_CUSTOM_ORDER: i64 = 1_000_000;
const MAX_REDIRECTS: usize = 5;
const MAX_STAGE_BUDGET_SECS: u64 = 60;
const MAX_PIPELINE_BUDGET_SECS: u64 = 120;
const MAX_CHARSET_SCAN_BYTES: usize = 16 * 1024;
const CONTENT_FALLBACK_SELECTORS: &[(&str, Option<&str>)] = &[
    (".content", Some("html")),
    ("#content", Some("html")),
    ("article.content", Some("html")),
    (".read-content", Some("html")),
    (".chapter-content", Some("html")),
    ("[itemprop=\"articleBody\"]", Some("html")),
    ("article", Some("html")),
    ("main", Some("html")),
];

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
    #[error("request exceeded stage time budget: {0}")]
    TimeoutBudget(String),
    #[error("no value matched the source rule")]
    NoMatch,
    #[error("{stage} 规则 {rule} 失败：{message}")]
    Rule {
        stage: String,
        rule: String,
        message: String,
    },
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
    #[serde(default, alias = "bookSourceUrl")]
    pub source_url: Option<String>,
    #[serde(default, alias = "bookSourceGroup")]
    pub group: Option<String>,
    #[serde(default, alias = "bookSourceType")]
    pub source_type: i64,
    #[serde(default, alias = "bookUrlPattern")]
    pub book_url_pattern: Option<String>,
    #[serde(default, alias = "exploreUrl")]
    pub explore_url: Option<String>,
    #[serde(default, alias = "enabledExplore")]
    pub enabled_explore: bool,
    #[serde(default, alias = "customOrder")]
    pub custom_order: i64,
    #[serde(default)]
    pub weight: i64,
    #[serde(default, alias = "bookSourceComment")]
    pub comment: Option<String>,
    #[serde(alias = "searchUrl")]
    pub search_url: String,
    #[serde(default)]
    pub legacy_urls: HashMap<String, String>,
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
    #[serde(default, alias = "permissions")]
    pub permission: SourcePermission,
    #[serde(default)]
    pub headers: HashMap<String, String>,
    /// Original dynamic or non-standard header expression retained for round-tripping.
    /// It is intentionally inert at runtime.
    #[serde(default)]
    pub legacy_headers: Option<Value>,
    #[serde(default, alias = "replaceRules", alias = "replacements")]
    pub replace_rules: Vec<ReplaceRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplaceRule {
    pub pattern: String,
    #[serde(default)]
    pub replacement: String,
    #[serde(default = "default_replace_enabled")]
    pub enabled: bool,
}

fn default_replace_enabled() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourcePermission {
    #[serde(default = "default_permission_status", alias = "permissionStatus")]
    pub status: String,
    #[serde(default)]
    pub scope: Option<String>,
    #[serde(default, alias = "reviewedAt")]
    pub reviewed_at: Option<String>,
}

impl Default for SourcePermission {
    fn default() -> Self {
        Self {
            status: default_permission_status(),
            scope: None,
            reviewed_at: None,
        }
    }
}

fn default_permission_status() -> String {
    "unknown".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PageRules {
    #[serde(default)]
    pub item: Option<String>,
    /// Original item selector retained when it cannot be safely converted.
    #[serde(default)]
    pub item_legacy: Option<Value>,
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
    #[serde(
        default,
        alias = "nextUrl",
        alias = "next_url",
        alias = "nextPage",
        alias = "next_page"
    )]
    pub next: Option<SourceRule>,
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
        #[serde(default)]
        replacement: Option<String>,
    },
    JsonPath {
        #[serde(alias = "jsonPath", alias = "path")]
        json_path: String,
        #[serde(default)]
        attr: Option<String>,
        #[serde(default)]
        regex: Option<String>,
        #[serde(default)]
        replacement: Option<String>,
    },
    Join {
        join: Vec<SourceRule>,
    },
    Chain {
        chain: Vec<SourceRule>,
    },
    /// Original Legado/XPath/JavaScript/template expression preserved for
    /// round-tripping. It is intentionally inert at runtime.
    Legacy {
        legacy: Value,
        #[serde(default)]
        reason: Option<String>,
    },
}

impl SourceRule {
    fn selector(&self) -> &str {
        match self {
            Self::Selector(value) => value,
            Self::Detailed { selector, .. } => selector,
            Self::JsonPath { json_path, .. } => json_path,
            Self::Join { join } => join.first().map(Self::selector).unwrap_or_default(),
            Self::Chain { chain } => chain.first().map(Self::selector).unwrap_or_default(),
            Self::Legacy { .. } => "",
        }
    }

    fn attr(&self) -> Option<&str> {
        match self {
            Self::Selector(_) | Self::Join { .. } | Self::Chain { .. } | Self::Legacy { .. } => {
                None
            }
            Self::Detailed { attr, .. } | Self::JsonPath { attr, .. } => attr.as_deref(),
        }
    }

    fn regex(&self) -> Option<&str> {
        match self {
            Self::Selector(_) | Self::Join { .. } | Self::Chain { .. } | Self::Legacy { .. } => {
                None
            }
            Self::Detailed { regex, .. } | Self::JsonPath { regex, .. } => regex.as_deref(),
        }
    }

    fn replacement(&self) -> Option<&str> {
        match self {
            Self::Selector(_) | Self::Join { .. } | Self::Chain { .. } | Self::Legacy { .. } => {
                None
            }
            Self::Detailed { replacement, .. } | Self::JsonPath { replacement, .. } => {
                replacement.as_deref()
            }
        }
    }

    fn json_path(&self) -> Option<&str> {
        match self {
            Self::JsonPath { json_path, .. } => Some(json_path),
            Self::Selector(value) if is_json_rule_path(value) => Some(value),
            Self::Detailed { selector, .. } if is_json_rule_path(selector) => Some(selector),
            Self::Join { join } => {
                let usable = join
                    .iter()
                    .filter(|rule| !rule.is_legacy())
                    .collect::<Vec<_>>();
                if !usable.is_empty() && usable.iter().all(|rule| rule.is_json_path()) {
                    usable.first().and_then(|rule| rule.json_path())
                } else {
                    None
                }
            }
            Self::Chain { chain } => {
                let usable = chain
                    .iter()
                    .filter(|rule| !rule.is_legacy())
                    .collect::<Vec<_>>();
                if !usable.is_empty() && usable.iter().all(|rule| rule.is_json_path()) {
                    usable.first().and_then(|rule| rule.json_path())
                } else {
                    None
                }
            }
            Self::Legacy { .. } => None,
            _ => None,
        }
    }

    fn is_legacy(&self) -> bool {
        matches!(self, Self::Legacy { .. })
    }

    fn is_json_path(&self) -> bool {
        match self {
            Self::Join { join } => {
                let usable = join
                    .iter()
                    .filter(|rule| !rule.is_legacy())
                    .collect::<Vec<_>>();
                !usable.is_empty() && usable.iter().all(|rule| rule.is_json_path())
            }
            Self::Chain { chain } => {
                let usable = chain
                    .iter()
                    .filter(|rule| !rule.is_legacy())
                    .collect::<Vec<_>>();
                !usable.is_empty() && usable.iter().all(|rule| rule.is_json_path())
            }
            Self::Legacy { .. } => false,
            _ => self.json_path().is_some(),
        }
    }
}

impl PageRules {
    fn is_json(&self) -> bool {
        self.item.as_deref().is_some_and(is_json_rule_path)
            || [
                self.title.as_ref(),
                self.author.as_ref(),
                self.url.as_ref(),
                self.intro.as_ref(),
                self.content.as_ref(),
                self.next.as_ref(),
            ]
            .into_iter()
            .flatten()
            .any(SourceRule::is_json_path)
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
pub struct SourceSecurityAudit {
    pub permission_status: String,
    pub permission_scope: Option<String>,
    pub reviewed_at: Option<String>,
    pub hosts: Vec<String>,
    pub sensitive_headers: Vec<String>,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    pub pass: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct SearchResult {
    pub title: String,
    pub author: Option<String>,
    pub intro: Option<String>,
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
    pub intro: Option<String>,
    pub book_url: Option<String>,
    pub can_open: bool,
    pub can_read: bool,
    pub unavailable_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SourceSearchFailure {
    pub source_id: String,
    pub source_name: String,
    pub message: String,
    #[serde(default)]
    pub rule_evaluations: Vec<SourceRuleEvaluation>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceRuleEvaluationStatus {
    Success,
    NoMatch,
    Failure,
    Skipped,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceRuleEvaluation {
    pub stage: String,
    pub rule_key: String,
    pub status: SourceRuleEvaluationStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SourceSearchDiagnostics {
    pub source_id: String,
    pub source_name: String,
    pub pages_scanned: usize,
    pub parsed_items: usize,
    pub stop_reason: String,
    #[serde(default)]
    pub rule_evaluations: Vec<SourceRuleEvaluation>,
}

pub fn rule_evaluation_for_output(
    stage: &str,
    rule_key: &str,
    has_output: bool,
) -> SourceRuleEvaluation {
    SourceRuleEvaluation {
        stage: stage.to_string(),
        rule_key: rule_key.to_string(),
        status: if has_output {
            SourceRuleEvaluationStatus::Success
        } else {
            SourceRuleEvaluationStatus::NoMatch
        },
        detail: None,
    }
}

fn rule_evaluation_for_rule(
    stage: &str,
    rule_key: &str,
    configured: bool,
    has_output: bool,
) -> SourceRuleEvaluation {
    if !configured {
        return SourceRuleEvaluation {
            stage: stage.to_string(),
            rule_key: rule_key.to_string(),
            status: SourceRuleEvaluationStatus::Skipped,
            detail: None,
        };
    }
    rule_evaluation_for_output(stage, rule_key, has_output)
}

fn search_rule_evaluations(
    source: &BookSource,
    results: &[SearchResult],
) -> Vec<SourceRuleEvaluation> {
    let rules = source.search.as_ref();
    vec![
        rule_evaluation_for_rule("search", "item", rules.is_some(), !results.is_empty()),
        rule_evaluation_for_rule(
            "search",
            "title",
            rules.and_then(|value| value.title.as_ref()).is_some(),
            results.iter().any(|item| !item.title.trim().is_empty()),
        ),
        rule_evaluation_for_rule(
            "search",
            "author",
            rules.and_then(|value| value.author.as_ref()).is_some(),
            results.iter().any(|item| {
                item.author
                    .as_deref()
                    .is_some_and(|value| !value.trim().is_empty())
            }),
        ),
        rule_evaluation_for_rule(
            "search",
            "url",
            rules.and_then(|value| value.url.as_ref()).is_some(),
            results.iter().any(|item| item.book_url.is_some()),
        ),
    ]
}

fn book_rule_evaluations(source: &BookSource, book_info: &BookInfo) -> Vec<SourceRuleEvaluation> {
    let rules = source.book_info.as_ref();
    vec![
        rule_evaluation_for_rule(
            "book_info",
            "title",
            rules.and_then(|value| value.title.as_ref()).is_some(),
            book_info.title.trim() != "未命名书籍" && !book_info.title.trim().is_empty(),
        ),
        rule_evaluation_for_rule(
            "book_info",
            "author",
            rules.and_then(|value| value.author.as_ref()).is_some(),
            book_info
                .author
                .as_deref()
                .is_some_and(|value| !value.trim().is_empty()),
        ),
        rule_evaluation_for_rule(
            "book_info",
            "intro",
            rules.and_then(|value| value.intro.as_ref()).is_some(),
            book_info
                .intro
                .as_deref()
                .is_some_and(|value| !value.trim().is_empty()),
        ),
        rule_evaluation_for_rule(
            "book_info",
            "cover",
            rules.and_then(|value| value.url.as_ref()).is_some(),
            book_info
                .cover_url
                .as_deref()
                .is_some_and(|value| !value.trim().is_empty()),
        ),
    ]
}

fn toc_rule_evaluations(
    source: &BookSource,
    chapters: &[SourceChapter],
) -> Vec<SourceRuleEvaluation> {
    let rules = source.toc.as_ref();
    vec![
        rule_evaluation_for_rule("toc", "item", rules.is_some(), !chapters.is_empty()),
        rule_evaluation_for_rule(
            "toc",
            "title",
            rules.and_then(|value| value.title.as_ref()).is_some(),
            chapters
                .iter()
                .any(|chapter| !chapter.title.trim().is_empty()),
        ),
        rule_evaluation_for_rule(
            "toc",
            "url",
            rules.and_then(|value| value.url.as_ref()).is_some(),
            chapters
                .iter()
                .any(|chapter| !chapter.url.trim().is_empty()),
        ),
    ]
}

fn content_rule_evaluations(
    source: &BookSource,
    content: &str,
    next_url: Option<&str>,
) -> Vec<SourceRuleEvaluation> {
    let rules = source.content.as_ref();
    vec![
        rule_evaluation_for_rule(
            "content",
            "content",
            rules.and_then(|value| value.content.as_ref()).is_some(),
            !content.trim().is_empty(),
        ),
        rule_evaluation_for_rule(
            "content",
            "next",
            rules.and_then(|value| value.next.as_ref()).is_some(),
            next_url.is_some_and(|value| !value.trim().is_empty()),
        ),
    ]
}

pub fn rule_evaluation_from_error(
    stage: &str,
    rule_key: &str,
    message: &str,
) -> Option<SourceRuleEvaluation> {
    let lower = message.to_ascii_lowercase();
    let is_rule_error = lower.contains("parse")
        || (lower.contains("invalid")
            && (lower.contains("selector") || lower.contains("json") || lower.contains("regex")))
        || lower.contains("no value matched the source rule");
    if !is_rule_error {
        return None;
    }
    let status = if lower.contains("no value matched the source rule") {
        SourceRuleEvaluationStatus::NoMatch
    } else {
        SourceRuleEvaluationStatus::Failure
    };
    Some(SourceRuleEvaluation {
        stage: stage.to_string(),
        rule_key: rule_key.to_string(),
        status,
        detail: Some(if status == SourceRuleEvaluationStatus::NoMatch {
            "no_match".to_string()
        } else {
            "rule_error".to_string()
        }),
    })
}

#[derive(Debug, Clone)]
struct PagedSearchResult {
    results: Vec<SearchResult>,
    pages_scanned: usize,
    parsed_items: usize,
    stop_reason: String,
    rule_evaluations: Vec<SourceRuleEvaluation>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MultiSourceSearchResult {
    pub results: Vec<UnifiedSearchResult>,
    pub failures: Vec<SourceSearchFailure>,
    pub diagnostics: Vec<SourceSearchDiagnostics>,
    pub enabled_sources: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookInfo {
    pub title: String,
    pub author: Option<String>,
    pub intro: Option<String>,
    pub cover_url: Option<String>,
    pub book_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceChapter {
    pub title: String,
    pub url: String,
    pub index: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChapterUpdateSummary {
    pub changed: bool,
    pub fingerprint: String,
    pub added: usize,
    pub removed: usize,
    pub retained: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceChapterContent {
    pub title: String,
    pub content: String,
    pub next_url: Option<String>,
    #[serde(default)]
    pub rule_evaluations: Vec<SourceRuleEvaluation>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SourcePreview {
    pub status: u16,
    pub content_type: Option<String>,
    pub bytes: usize,
    pub body_preview: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceDebugStep {
    pub stage: String,
    pub url: String,
    pub duration_ms: u64,
    pub status: Option<u16>,
    pub bytes: Option<usize>,
    pub error: Option<String>,
    #[serde(default)]
    pub variables: BTreeMap<String, String>,
    #[serde(default)]
    pub cache_hit: bool,
}

#[derive(Debug, Clone, Default)]
struct SourceRequestContext {
    keyword: Option<String>,
    page: usize,
    book_url: Option<String>,
    book_id: Option<String>,
    chapter_id: Option<String>,
}

impl SourceRequestContext {
    fn search(keyword: &str, page: usize) -> Self {
        Self {
            keyword: Some(keyword.to_string()),
            page: page.max(1),
            ..Self::default()
        }
    }

    fn book(book_url: &str) -> Self {
        Self {
            book_url: Some(book_url.to_string()),
            book_id: Some(last_path_segment(book_url).to_string()),
            page: 1,
            ..Self::default()
        }
    }

    fn chapter(chapter_url: &str) -> Self {
        Self {
            chapter_id: Some(last_path_segment(chapter_url).to_string()),
            page: 1,
            ..Self::default()
        }
    }

    fn variables(&self) -> BTreeMap<String, String> {
        let page = self.page.max(1);
        let mut variables = BTreeMap::from([
            ("page".to_string(), page.to_string()),
            ("pageNum".to_string(), page.to_string()),
            ("pageIndex".to_string(), page.saturating_sub(1).to_string()),
            ("page+1".to_string(), page.saturating_add(1).to_string()),
            ("page-1".to_string(), page.saturating_sub(1).to_string()),
        ]);
        if self.keyword.is_some() {
            variables.insert("keyword".to_string(), "<redacted>".to_string());
            variables.insert("key".to_string(), "<redacted>".to_string());
        }
        if self.book_url.is_some() {
            variables.insert("bookUrl".to_string(), "<redacted-url>".to_string());
            variables.insert("book_url".to_string(), "<redacted-url>".to_string());
        }
        if self.book_id.is_some() {
            variables.insert("bookId".to_string(), "<redacted>".to_string());
            variables.insert("book_id".to_string(), "<redacted>".to_string());
        }
        if self.chapter_id.is_some() {
            variables.insert("chapterId".to_string(), "<redacted>".to_string());
            variables.insert("chapter_id".to_string(), "<redacted>".to_string());
        }
        variables
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct SourcePipelineResult {
    pub search_results: Vec<SearchResult>,
    pub book_info: BookInfo,
    pub chapters: Vec<SourceChapter>,
    pub first_chapter: SourceChapterContent,
    pub debug_steps: Vec<SourceDebugStep>,
    #[serde(default)]
    pub rule_evaluations: Vec<SourceRuleEvaluation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceBookDetail {
    pub book_info: BookInfo,
    pub chapters: Vec<SourceChapter>,
    pub debug_steps: Vec<SourceDebugStep>,
    #[serde(default)]
    pub rule_evaluations: Vec<SourceRuleEvaluation>,
}

struct FetchedText {
    body: String,
    status: u16,
    bytes: usize,
    encoding: String,
    had_decode_errors: bool,
}

struct DecodedResponse {
    body: String,
    encoding: String,
    had_decode_errors: bool,
}

#[derive(Clone)]
pub struct SourceEngine {
    client: reqwest::Client,
    max_body_bytes: usize,
    stage_budget: Duration,
    pipeline_budget: Duration,
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
            .redirect(reqwest::redirect::Policy::limited(MAX_REDIRECTS))
            .build()?;

        Ok(Self {
            client,
            max_body_bytes,
            stage_budget: bounded_stage_timeout(timeout_secs),
            pipeline_budget: bounded_pipeline_timeout(timeout_secs),
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

        let decoded = decode_response_body(&body, content_type.as_deref());
        let preview = decoded.body.chars().take(2_000).collect();

        Ok(SourcePreview {
            status,
            content_type,
            bytes: body.len(),
            body_preview: preview,
        })
    }

    pub async fn fetch_text_document(&self, url: &str) -> Result<String, SourceError> {
        self.fetch_text(url, &HashMap::new())
            .await
            .map(|fetched| fetched.body)
    }

    pub async fn search(
        &self,
        source: &BookSource,
        keyword: &str,
    ) -> Result<Vec<SearchResult>, SourceError> {
        self.search_page(source, keyword, 1).await
    }

    pub async fn search_page(
        &self,
        source: &BookSource,
        keyword: &str,
        page: usize,
    ) -> Result<Vec<SearchResult>, SourceError> {
        ensure_runtime_endpoint(source, "searchUrl")?;
        let context = SourceRequestContext::search(keyword, page);
        let search_url = render_url_context(&source.search_url, &context);
        let fetched = self
            .fetch_text(&search_url, &source.headers)
            .await
            .map_err(|error| pipeline_error("search_fetch", error))?;
        let mut results = self
            .parse_search_response(source, &fetched.body)
            .map_err(|error| pipeline_error("search_parse", error))?;
        for result in &mut results {
            if let Some(url) = &result.book_url {
                result.book_url = Some(absolutize_url(&search_url, url));
            }
        }
        Ok(results)
    }

    pub async fn search_pages(
        &self,
        source: &BookSource,
        keyword: &str,
        requested_pages: usize,
    ) -> Result<Vec<SearchResult>, SourceError> {
        Ok(self
            .search_pages_with_diagnostics(source, keyword, requested_pages)
            .await?
            .results)
    }

    async fn search_pages_with_diagnostics(
        &self,
        source: &BookSource,
        keyword: &str,
        requested_pages: usize,
    ) -> Result<PagedSearchResult, SourceError> {
        let limit = bounded_search_pages(requested_pages);
        let mut results = Vec::new();
        let mut seen = HashSet::new();
        let mut pages_scanned = 0;
        let mut parsed_items = 0;
        let mut stop_reason = "max_pages";
        let mut rule_evaluations = Vec::new();

        for page in 1..=limit {
            pages_scanned += 1;
            let page_results = self.search_page(source, keyword, page).await?;
            rule_evaluations.extend(search_rule_evaluations(source, &page_results));
            if page_results.is_empty() {
                stop_reason = "empty_page";
                break;
            }

            parsed_items += page_results.len();
            let mut added = 0;
            for item in page_results {
                if seen.insert(search_result_identity(&item)) {
                    results.push(item);
                    added += 1;
                }
            }
            if added == 0 {
                stop_reason = "no_new_results";
                break;
            }
        }

        Ok(PagedSearchResult {
            results,
            pages_scanned,
            parsed_items,
            stop_reason: stop_reason.to_string(),
            rule_evaluations,
        })
    }

    pub async fn search_many(
        &self,
        sources: Vec<SourceDefinition>,
        keyword: &str,
    ) -> MultiSourceSearchResult {
        self.search_many_with_pages(sources, keyword, 1).await
    }

    pub async fn search_many_with_pages(
        &self,
        sources: Vec<SourceDefinition>,
        keyword: &str,
        max_pages: usize,
    ) -> MultiSourceSearchResult {
        let enabled_sources = sources.len();
        let mut tasks = tokio::task::JoinSet::new();

        for definition in sources {
            let engine = self.clone();
            let keyword = keyword.to_string();
            tasks.spawn(async move {
                let result = engine
                    .search_pages_with_diagnostics(&definition.source, &keyword, max_pages)
                    .await;
                (definition, result)
            });
        }

        let mut results = Vec::new();
        let mut failures = Vec::new();
        let mut diagnostics = Vec::new();

        while let Some(joined) = tasks.join_next().await {
            match joined {
                Ok((definition, Ok(paged))) => {
                    diagnostics.push(SourceSearchDiagnostics {
                        source_id: definition.id.clone(),
                        source_name: definition.name.clone(),
                        pages_scanned: paged.pages_scanned,
                        parsed_items: paged.parsed_items,
                        stop_reason: paged.stop_reason,
                        rule_evaluations: paged.rule_evaluations,
                    });
                    results.extend(paged.results.into_iter().map(|item| {
                        let (can_open, can_read, unavailable_reason) =
                            source_capabilities(&definition.source, item.book_url.as_deref());
                        UnifiedSearchResult {
                            source_id: definition.id.clone(),
                            source_name: definition.name.clone(),
                            title: item.title,
                            author: item.author,
                            intro: item.intro,
                            book_url: item.book_url,
                            can_open,
                            can_read,
                            unavailable_reason,
                        }
                    }));
                }
                Ok((definition, Err(error))) => {
                    let message = error.to_string();
                    let rule_evaluations: Vec<SourceRuleEvaluation> =
                        rule_evaluation_from_error("search", "item", &message)
                            .into_iter()
                            .collect();
                    diagnostics.push(SourceSearchDiagnostics {
                        source_id: definition.id.clone(),
                        source_name: definition.name.clone(),
                        pages_scanned: 0,
                        parsed_items: 0,
                        stop_reason: "request_failed".to_string(),
                        rule_evaluations: rule_evaluations.clone(),
                    });
                    failures.push(SourceSearchFailure {
                        source_id: definition.id,
                        source_name: definition.name,
                        message,
                        rule_evaluations,
                    });
                }
                Err(error) => failures.push(SourceSearchFailure {
                    source_id: "unknown".to_string(),
                    source_name: "未知书源".to_string(),
                    message: format!("搜索任务异常：{}", error),
                    rule_evaluations: Vec::new(),
                }),
            }
        }

        failures.sort_by(|left, right| {
            left.source_name
                .cmp(&right.source_name)
                .then_with(|| left.source_id.cmp(&right.source_id))
        });
        diagnostics.sort_by(|left, right| {
            left.source_name
                .cmp(&right.source_name)
                .then_with(|| left.source_id.cmp(&right.source_id))
        });

        MultiSourceSearchResult {
            results: dedupe_search_results(results),
            failures,
            diagnostics,
            enabled_sources,
        }
    }

    pub async fn fetch_book_detail(
        &self,
        source: &BookSource,
        book_url: &str,
    ) -> Result<SourceBookDetail, SourceError> {
        let mut debug_steps = Vec::new();
        let mut rule_evaluations = Vec::new();
        let book_info = self
            .fetch_book_info(source, book_url, &mut debug_steps)
            .await?;
        let chapters = self.fetch_toc(source, book_url, &mut debug_steps).await?;
        if chapters.is_empty() {
            return Err(rule_error("toc", "item", SourceError::NoMatch));
        }
        rule_evaluations.extend(book_rule_evaluations(source, &book_info));
        rule_evaluations.extend(toc_rule_evaluations(source, &chapters));

        Ok(SourceBookDetail {
            book_info,
            chapters,
            debug_steps,
            rule_evaluations,
        })
    }

    pub async fn run_pipeline(
        &self,
        source: &BookSource,
        keyword: &str,
    ) -> Result<SourcePipelineResult, SourceError> {
        match tokio::time::timeout(
            self.pipeline_budget,
            self.run_pipeline_inner(source, keyword),
        )
        .await
        {
            Ok(result) => result,
            Err(_) => Err(SourceError::TimeoutBudget(format!(
                "pipeline 超过 {} 秒总时间预算",
                self.pipeline_budget.as_secs()
            ))),
        }
    }

    async fn run_pipeline_inner(
        &self,
        source: &BookSource,
        keyword: &str,
    ) -> Result<SourcePipelineResult, SourceError> {
        ensure_runtime_endpoint(source, "searchUrl")?;
        let mut debug_steps = Vec::new();
        let search_context = SourceRequestContext::search(keyword, 1);
        let (search_body, search_url) = self
            .fetch_stage_chain(
                "search",
                &source.search_url,
                &source.headers,
                &search_context,
                &mut debug_steps,
            )
            .await?;
        let mut search_results = self
            .parse_search_response(source, &search_body)
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

        let mut rule_evaluations = Vec::new();
        rule_evaluations.extend(first_chapter_content.rule_evaluations.iter().cloned());
        Ok(SourcePipelineResult {
            search_results,
            book_info,
            chapters,
            first_chapter: first_chapter_content,
            debug_steps,
            rule_evaluations,
        })
    }

    async fn fetch_stage(
        &self,
        stage: &str,
        url: &str,
        headers: &HashMap<String, String>,
        context: &SourceRequestContext,
        debug_steps: &mut Vec<SourceDebugStep>,
    ) -> Result<String, SourceError> {
        let started = Instant::now();
        match self.fetch_text(url, headers).await {
            Ok(response) => {
                let mut variables = context.variables();
                variables.insert("encoding".to_string(), response.encoding.clone());
                if response.had_decode_errors {
                    variables.insert(
                        "encoding_warning".to_string(),
                        "响应包含无法按声明字符集解码的字节，已使用兼容回退".to_string(),
                    );
                }
                debug_steps.push(SourceDebugStep {
                    stage: stage.to_string(),
                    url: redact_url(url),
                    duration_ms: started.elapsed().as_millis() as u64,
                    status: Some(response.status),
                    bytes: Some(response.bytes),
                    error: None,
                    variables,
                    cache_hit: false,
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
                    variables: context.variables(),
                    cache_hit: false,
                });
                Err(pipeline_error(stage, error))
            }
        }
    }

    async fn fetch_stage_chain(
        &self,
        stage: &str,
        template: &str,
        headers: &HashMap<String, String>,
        context: &SourceRequestContext,
        debug_steps: &mut Vec<SourceDebugStep>,
    ) -> Result<(String, String), SourceError> {
        let urls = render_url_chain(template, context)?;
        let total = urls.len();
        let deadline = Instant::now() + self.stage_budget;
        let mut last_error = None;

        for (index, url) in urls.iter().enumerate() {
            let stage_name = if total == 1 {
                stage.to_string()
            } else {
                format!("{stage}[{}]", index + 1)
            };
            let remaining = deadline
                .checked_duration_since(Instant::now())
                .unwrap_or_default();
            if remaining.is_zero() {
                let error = SourceError::TimeoutBudget(format!(
                    "{stage} 阶段超过 {} 秒时间预算",
                    self.stage_budget.as_secs()
                ));
                debug_steps.push(SourceDebugStep {
                    stage: stage_name,
                    url: redact_url(url),
                    duration_ms: 0,
                    status: None,
                    bytes: None,
                    error: Some(error.to_string()),
                    variables: context.variables(),
                    cache_hit: false,
                });
                last_error = Some(pipeline_error(stage, error));
                break;
            }

            match tokio::time::timeout(
                remaining,
                self.fetch_stage(&stage_name, url, headers, context, debug_steps),
            )
            .await
            {
                Ok(Ok(body)) => return Ok((body, url.clone())),
                Ok(Err(error)) => last_error = Some(error),
                Err(_) => {
                    let error = SourceError::TimeoutBudget(format!(
                        "{stage_name} 超过 {} 秒时间预算",
                        self.stage_budget.as_secs()
                    ));
                    debug_steps.push(SourceDebugStep {
                        stage: stage_name,
                        url: redact_url(url),
                        duration_ms: self.stage_budget.as_millis() as u64,
                        status: None,
                        bytes: None,
                        error: Some(error.to_string()),
                        variables: context.variables(),
                        cache_hit: false,
                    });
                    last_error = Some(pipeline_error(stage, error));
                    break;
                }
            }
        }

        Err(last_error
            .unwrap_or_else(|| SourceError::InvalidUrl("URL 回退链没有可用候选".to_string())))
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
        let content_type = response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(str::to_string);
        let mut response = response.error_for_status()?;
        let mut body = Vec::new();

        while let Some(chunk) = response.chunk().await? {
            if body.len() + chunk.len() > self.max_body_bytes {
                return Err(SourceError::BodyTooLarge(self.max_body_bytes));
            }
            body.extend_from_slice(&chunk);
        }

        let decoded = decode_response_body(&body, content_type.as_deref());
        Ok(FetchedText {
            status,
            bytes: body.len(),
            encoding: decoded.encoding,
            had_decode_errors: decoded.had_decode_errors,
            body: decoded.body,
        })
    }

    async fn fetch_book_info(
        &self,
        source: &BookSource,
        book_url: &str,
        debug_steps: &mut Vec<SourceDebugStep>,
    ) -> Result<BookInfo, SourceError> {
        let rules = source
            .book_info
            .as_ref()
            .ok_or_else(|| SourceError::InvalidConfig("bookInfo rules are required".to_string()))?;
        let template = runtime_endpoint_or_fallback(
            source,
            "bookInfoUrl",
            source.book_info_url.as_deref(),
            book_url,
        );
        let context = SourceRequestContext::book(book_url);
        let (body, fetched_url) = self
            .fetch_stage_chain(
                "book_info",
                template,
                &source.headers,
                &context,
                debug_steps,
            )
            .await?;
        if rules.is_json() {
            let mut book_info = parse_book_info_json(rules, &body, book_url)
                .map_err(|error| rule_error("book_info", "document", error))?;
            let downgraded_fields = downgrade_garbled_book_fields(&mut book_info);
            append_text_quality_debug_step(debug_steps, &fetched_url, &downgraded_fields);
            return Ok(book_info);
        }
        let document = Html::parse_document(&body);

        let mut book_info = BookInfo {
            title: extract_document_rule_with_fallback(
                &document,
                rules.title.as_ref(),
                &[
                    ("h1", None),
                    ("h2", None),
                    (".book-title", None),
                    (".bookname", None),
                    (".title", None),
                ],
            )
            .map_err(|error| rule_error("book_info", "title", error))?
            .unwrap_or_else(|| "未命名书籍".to_string()),
            author: non_empty(
                extract_document_rule_with_fallback(
                    &document,
                    rules.author.as_ref(),
                    &[
                        (".author", None),
                        (".book-author", None),
                        (r#"[itemprop="author"]"#, None),
                    ],
                )
                .map_err(|error| rule_error("book_info", "author", error))?,
            ),
            intro: non_empty(
                extract_document_rule_with_fallback(
                    &document,
                    rules.intro.as_ref(),
                    &[
                        (".intro", None),
                        (".book-intro", None),
                        (".description", None),
                        (r#"[itemprop="description"]"#, None),
                    ],
                )
                .map_err(|error| rule_error("book_info", "intro", error))?,
            ),
            cover_url: non_empty(
                extract_document_rule_with_fallback(
                    &document,
                    rules.url.as_ref(),
                    &[
                        ("img.cover", Some("src")),
                        (".book-cover img", Some("src")),
                        ("img[data-src]", Some("data-src")),
                        ("img[data-original]", Some("data-original")),
                        (r#"[itemprop="image"]"#, Some("content")),
                    ],
                )
                .map_err(|error| rule_error("book_info", "cover", error))?,
            ),
            book_url: book_url.to_string(),
        };
        let downgraded_fields = downgrade_garbled_book_fields(&mut book_info);
        append_text_quality_debug_step(debug_steps, &fetched_url, &downgraded_fields);
        Ok(book_info)
    }

    async fn fetch_toc(
        &self,
        source: &BookSource,
        book_url: &str,
        debug_steps: &mut Vec<SourceDebugStep>,
    ) -> Result<Vec<SourceChapter>, SourceError> {
        let rules = source
            .toc
            .as_ref()
            .ok_or_else(|| SourceError::InvalidConfig("toc rules are required".to_string()))?;
        let template =
            runtime_endpoint_or_fallback(source, "tocUrl", source.toc_url.as_deref(), book_url);
        let context = SourceRequestContext::book(book_url);
        let (body, url) = self
            .fetch_stage_chain("toc", template, &source.headers, &context, debug_steps)
            .await?;
        if rules.is_json() {
            return parse_chapter_list_json(rules, &body, &url)
                .map_err(|error| rule_error("toc", "document", error));
        }
        parse_chapter_list(rules, &body, &url).map_err(|error| rule_error("toc", "item", error))
    }

    pub async fn fetch_chapter_content(
        &self,
        source: &BookSource,
        chapter: &SourceChapter,
        debug_steps: &mut Vec<SourceDebugStep>,
    ) -> Result<SourceChapterContent, SourceError> {
        let rules = source
            .content
            .as_ref()
            .ok_or_else(|| SourceError::InvalidConfig("content rules are required".to_string()))?;
        let template = runtime_endpoint_or_fallback(
            source,
            "contentUrl",
            source.content_url.as_deref(),
            chapter.url.as_str(),
        );
        let context = SourceRequestContext::chapter(&chapter.url);
        let (body, content_url) = self
            .fetch_stage_chain("content", template, &source.headers, &context, debug_steps)
            .await?;
        let content = if rules.is_json() {
            parse_json_rule_document(&body, rules.item.as_deref(), rules.content.as_ref())
                .map_err(|error| rule_error("content", "content", error))?
        } else {
            let document = Html::parse_document(&body);
            extract_document_rule_with_fallback(
                &document,
                rules.content.as_ref(),
                CONTENT_FALLBACK_SELECTORS,
            )
            .map_err(|error| rule_error("content", "content", error))?
        }
        .ok_or_else(|| rule_error("content", "content", SourceError::NoMatch))?;
        let next_url = if let Some(next_rule) = rules.next.as_ref() {
            let next_value = if rules.is_json() {
                parse_json_rule_document(&body, rules.item.as_deref(), Some(next_rule))
                    .map_err(|error| rule_error("content", "next", error))?
            } else {
                let document = Html::parse_document(&body);
                extract_document_rule(&document, Some(next_rule))
                    .map_err(|error| rule_error("content", "next", error))?
            };
            next_value
                .filter(|value| !value.trim().is_empty())
                .map(|value| {
                    let absolute = absolutize_url(&content_url, value.trim());
                    bounded_next_url(&absolute)
                })
                .transpose()?
                .flatten()
        } else {
            None
        };
        let content = apply_replace_rules(&content, &source.replace_rules)?;
        let rule_evaluations = content_rule_evaluations(source, &content, next_url.as_deref());

        Ok(SourceChapterContent {
            title: chapter.title.clone(),
            content,
            next_url,
            rule_evaluations,
        })
    }

    pub async fn fetch_chapter_content_with_policy(
        &self,
        source: &BookSource,
        chapter: &SourceChapter,
        policy: &NextPagePolicy,
        debug_steps: &mut Vec<SourceDebugStep>,
    ) -> Result<SourceChapterContent, SourceError> {
        if !policy.enabled {
            return self
                .fetch_chapter_content(source, chapter, debug_steps)
                .await;
        }

        let template = runtime_endpoint_or_fallback(
            source,
            "contentUrl",
            source.content_url.as_deref(),
            chapter.url.as_str(),
        );
        let rules = source
            .content
            .as_ref()
            .ok_or_else(|| SourceError::InvalidConfig("content rules are required".to_string()))?;

        let started = Instant::now();
        let mut depth: usize = 0;
        let mut pages_used: usize = 0;
        let mut bytes_used: usize = 0;
        let mut current_url = chapter.url.clone();
        let mut visited_urls = vec![current_url.clone()];
        let mut combined = String::new();
        let mut pending_next_url = None;
        let mut rule_evaluations = Vec::new();

        loop {
            let context = SourceRequestContext::chapter(&current_url);
            let stage = if depth == 0 {
                "content".to_string()
            } else {
                format!("content.next.depth-{depth}")
            };
            let fetched = match self
                .fetch_stage_chain(&stage, template, &source.headers, &context, debug_steps)
                .await
            {
                Ok(fetched) => fetched,
                Err(error) if pages_used > 0 => {
                    debug_steps.push(SourceDebugStep {
                        stage: "content.next.policy".to_string(),
                        url: redact_url(&current_url),
                        duration_ms: started.elapsed().as_millis() as u64,
                        status: None,
                        bytes: None,
                        error: Some(format!("next URL request_error: {error}")),
                        variables: context.variables(),
                        cache_hit: false,
                    });
                    pending_next_url = Some(current_url);
                    break;
                }
                Err(error) => return Err(error),
            };

            let (body, content_url) = fetched;
            let max_bytes = policy.max_bytes.min(default_next_page_bytes());
            if bytes_used.saturating_add(body.len()) > max_bytes {
                let error = SourceError::BodyTooLarge(max_bytes);
                debug_steps.push(SourceDebugStep {
                    stage: "content.next.policy".to_string(),
                    url: redact_url(&content_url),
                    duration_ms: started.elapsed().as_millis() as u64,
                    status: None,
                    bytes: Some(body.len()),
                    error: Some(format!("next URL byte_limit: {error}")),
                    variables: context.variables(),
                    cache_hit: false,
                });
                if pages_used == 0 {
                    return Err(error);
                }
                pending_next_url = Some(content_url);
                break;
            }

            let (page_content, next_url) = match parse_chapter_page(rules, &body, &content_url) {
                Ok(page) => page,
                Err(error) if pages_used > 0 => {
                    debug_steps.push(SourceDebugStep {
                        stage: "content.next.policy".to_string(),
                        url: redact_url(&content_url),
                        duration_ms: started.elapsed().as_millis() as u64,
                        status: None,
                        bytes: Some(body.len()),
                        error: Some(format!("next URL parse_error: {error}")),
                        variables: context.variables(),
                        cache_hit: false,
                    });
                    pending_next_url = Some(content_url);
                    break;
                }
                Err(error) => return Err(error),
            };

            let page_content = apply_replace_rules(&page_content, &source.replace_rules)?;
            rule_evaluations.extend(content_rule_evaluations(
                source,
                &page_content,
                next_url.as_deref(),
            ));
            if !combined.is_empty() {
                combined.push_str("\n\n");
            }
            combined.push_str(&page_content);
            pages_used = pages_used.saturating_add(1);
            bytes_used = bytes_used.saturating_add(body.len());

            let Some(next_url) = next_url else {
                break;
            };
            let decision = evaluate_next_page_policy(
                policy,
                &content_url,
                &next_url,
                depth,
                pages_used,
                bytes_used,
                started.elapsed().as_secs(),
                &visited_urls,
            );
            if !decision.allowed {
                debug_steps.push(SourceDebugStep {
                    stage: "content.next.policy".to_string(),
                    url: redact_url(&next_url),
                    duration_ms: started.elapsed().as_millis() as u64,
                    status: None,
                    bytes: None,
                    error: Some(format!("next URL {}", decision.reason)),
                    variables: context.variables(),
                    cache_hit: false,
                });
                pending_next_url = Some(next_url);
                break;
            }

            depth = decision.next_depth;
            current_url = next_url;
            visited_urls.push(current_url.clone());
        }

        Ok(SourceChapterContent {
            title: chapter.title.clone(),
            content: combined,
            next_url: pending_next_url,
            rule_evaluations,
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

    fn parse_search_response(
        &self,
        source: &BookSource,
        body: &str,
    ) -> Result<Vec<SearchResult>, SourceError> {
        if source.search.as_ref().is_some_and(PageRules::is_json) {
            self.parse_search_json(source, body)
        } else {
            self.parse_search_html(source, body)
        }
    }

    pub fn parse_search_json(
        &self,
        source: &BookSource,
        body: &str,
    ) -> Result<Vec<SearchResult>, SourceError> {
        let rules = source
            .search
            .as_ref()
            .ok_or_else(|| SourceError::InvalidConfig("search rules are required".to_string()))?;
        if !rules.is_json() {
            return Err(SourceError::InvalidConfig(
                "search rules are not JSONPath rules".to_string(),
            ));
        }

        let value: Value = serde_json::from_str(body)
            .map_err(|error| SourceError::InvalidJson(error.to_string()))?;
        let items = extract_json_nodes(&value, rules.item.as_deref().unwrap_or("$"))?;
        let mut results = Vec::new();

        for item in items {
            let title = rules
                .title
                .as_ref()
                .map(|rule| extract_json_rule_optional(item, Some(rule)))
                .transpose()?
                .flatten()
                .or_else(|| json_object_text(item, &["title", "name", "bookName"]))
                .unwrap_or_default();
            let author = rules
                .author
                .as_ref()
                .map(|rule| extract_json_rule_optional(item, Some(rule)))
                .transpose()?
                .flatten()
                .or_else(|| json_object_text(item, &["author", "bookAuthor"]));
            let intro = rules
                .intro
                .as_ref()
                .map(|rule| extract_json_rule_optional(item, Some(rule)))
                .transpose()?
                .flatten()
                .or_else(|| json_object_text(item, &["intro", "description", "desc", "summary"]));
            let book_url = rules
                .url
                .as_ref()
                .map(|rule| extract_json_rule_optional(item, Some(rule)))
                .transpose()?
                .flatten()
                .or_else(|| json_object_text(item, &["url", "href", "link", "bookUrl"]));

            if !title.is_empty() || book_url.is_some() {
                results.push(SearchResult {
                    title,
                    author: non_empty(author),
                    intro: non_empty(intro),
                    book_url: non_empty(book_url),
                    source_name: source.name.clone(),
                });
            }
        }

        Ok(results)
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
            let fallback_link = fallback_link_from_element(item);
            let title = rules
                .title
                .as_ref()
                .map(|rule| extract_from_element_optional(item, rule))
                .transpose()?
                .flatten()
                .or_else(|| {
                    fallback_link
                        .as_ref()
                        .and_then(|(_, title)| non_empty(Some(title.clone())))
                })
                .or_else(|| fallback_text_from_element(item))
                .unwrap_or_default();
            let author = rules
                .author
                .as_ref()
                .map(|rule| extract_from_element_optional(item, rule))
                .transpose()?;
            let intro = rules
                .intro
                .as_ref()
                .map(|rule| extract_from_element_optional(item, rule))
                .transpose()?
                .flatten()
                .or_else(|| {
                    extract_from_element_optional(item, &SourceRule::Selector(".intro".to_string()))
                        .ok()
                        .flatten()
                })
                .or_else(|| {
                    extract_from_element_optional(
                        item,
                        &SourceRule::Selector(".description".to_string()),
                    )
                    .ok()
                    .flatten()
                });
            let book_url = rules
                .url
                .as_ref()
                .map(|rule| extract_from_element_optional(item, rule))
                .transpose()?;
            let book_url = book_url
                .flatten()
                .or_else(|| fallback_link.as_ref().map(|(url, _)| url.clone()));

            if !title.is_empty() || book_url.is_some() {
                results.push(SearchResult {
                    title,
                    author: non_empty(author.flatten()),
                    intro: non_empty(intro),
                    book_url: non_empty(book_url),
                    source_name: source.name.clone(),
                });
            }
        }

        Ok(results)
    }
}

fn parse_chapter_page(
    rules: &PageRules,
    body: &str,
    content_url: &str,
) -> Result<(String, Option<String>), SourceError> {
    let content = if rules.is_json() {
        parse_json_rule_document(body, rules.item.as_deref(), rules.content.as_ref())
            .map_err(|error| rule_error("content", "content", error))?
    } else {
        let document = Html::parse_document(body);
        extract_document_rule_with_fallback(
            &document,
            rules.content.as_ref(),
            CONTENT_FALLBACK_SELECTORS,
        )
        .map_err(|error| rule_error("content", "content", error))?
    }
    .ok_or_else(|| rule_error("content", "content", SourceError::NoMatch))?;

    let next_url = if let Some(next_rule) = rules.next.as_ref() {
        let next_value = if rules.is_json() {
            parse_json_rule_document(body, rules.item.as_deref(), Some(next_rule))
                .map_err(|error| rule_error("content", "next", error))?
        } else {
            let document = Html::parse_document(body);
            extract_document_rule(&document, Some(next_rule))
                .map_err(|error| rule_error("content", "next", error))?
        };
        next_value
            .filter(|value| !value.trim().is_empty())
            .map(|value| {
                let absolute = absolutize_url(content_url, value.trim());
                bounded_next_url(&absolute)
            })
            .transpose()?
            .flatten()
    } else {
        None
    };

    Ok((content, next_url))
}

fn dedupe_search_results(mut results: Vec<UnifiedSearchResult>) -> Vec<UnifiedSearchResult> {
    results.sort_by(|left, right| {
        (
            normalize_search_text(&left.title),
            normalize_search_text(left.author.as_deref().unwrap_or_default()),
            !left.can_read,
            !left.can_open,
            left.book_url.is_none(),
            normalize_search_text(&left.source_name),
            left.source_id.clone(),
        )
            .cmp(&(
                normalize_search_text(&right.title),
                normalize_search_text(right.author.as_deref().unwrap_or_default()),
                !right.can_read,
                !right.can_open,
                right.book_url.is_none(),
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

fn source_capabilities(
    source: &BookSource,
    book_url: Option<&str>,
) -> (bool, bool, Option<String>) {
    if book_url.map_or(true, |value| value.trim().is_empty()) {
        return (false, false, Some("搜索结果没有可用的书籍链接".to_string()));
    }

    let can_open = source.book_info.is_some() && source.toc.is_some();
    let can_read = can_open && source.content.is_some();
    let reason = if !can_open {
        Some("该书源仅支持搜索，未配置书籍详情或目录规则".to_string())
    } else if !can_read {
        Some("该书源缺少正文规则，暂时无法阅读".to_string())
    } else {
        None
    };
    (can_open, can_read, reason)
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

fn rule_error(stage: &str, rule: &str, error: SourceError) -> SourceError {
    match error {
        SourceError::Rule { .. } => error,
        other => SourceError::Rule {
            stage: stage.to_string(),
            rule: rule.to_string(),
            message: other.to_string(),
        },
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

fn decode_response_body(bytes: &[u8], content_type: Option<&str>) -> DecodedResponse {
    let (payload, bom_encoding) = if bytes.starts_with(&[0xef, 0xbb, 0xbf]) {
        (&bytes[3..], Some("utf-8"))
    } else if bytes.starts_with(&[0xff, 0xfe]) {
        (&bytes[2..], Some("utf-16le"))
    } else if bytes.starts_with(&[0xfe, 0xff]) {
        (&bytes[2..], Some("utf-16be"))
    } else {
        (bytes, None)
    };

    let declared = bom_encoding
        .map(ToOwned::to_owned)
        .or_else(|| content_type.and_then(extract_charset_hint))
        .or_else(|| {
            let limit = payload.len().min(MAX_CHARSET_SCAN_BYTES);
            extract_charset_hint(&String::from_utf8_lossy(&payload[..limit]))
        });

    if let Some(label) = declared.as_deref().and_then(normalize_charset_label) {
        if let Some(decoded) = decode_with_charset(payload, &label) {
            if decoded.had_decode_errors {
                if let Some(mut fallback) = decode_with_charset(payload, "gb18030") {
                    if replacement_char_count(&fallback.body)
                        < replacement_char_count(&decoded.body)
                    {
                        fallback.encoding = "gb18030-fallback".to_string();
                        fallback.had_decode_errors = true;
                        return fallback;
                    }
                }
            }
            return decoded;
        }
    }

    if let Ok(text) = std::str::from_utf8(payload) {
        return DecodedResponse {
            body: text.to_string(),
            encoding: "utf-8".to_string(),
            had_decode_errors: false,
        };
    }

    let (text, _, _) = GB18030.decode(payload);
    DecodedResponse {
        body: text.into_owned(),
        encoding: "gb18030-fallback".to_string(),
        had_decode_errors: true,
    }
}

fn decode_with_charset(bytes: &[u8], label: &str) -> Option<DecodedResponse> {
    let (text, encoding, had_decode_errors) = match label {
        "utf-8" | "utf8" => {
            let had_decode_errors = std::str::from_utf8(bytes).is_err();
            (
                String::from_utf8_lossy(bytes).into_owned(),
                "utf-8",
                had_decode_errors,
            )
        }
        "utf-16le" | "utf16le" | "unicode" => {
            let (text, _, had_decode_errors) = UTF_16LE.decode(bytes);
            (text.into_owned(), "utf-16le", had_decode_errors)
        }
        "utf-16be" | "utf16be" => {
            let (text, _, had_decode_errors) = UTF_16BE.decode(bytes);
            (text.into_owned(), "utf-16be", had_decode_errors)
        }
        "gbk" | "gb2312" | "gb18030" | "x-gbk" => {
            let (text, _, had_decode_errors) = GB18030.decode(bytes);
            (text.into_owned(), "gb18030", had_decode_errors)
        }
        "windows-1252" | "cp1252" | "iso-8859-1" | "latin1" => {
            let (text, _, had_decode_errors) = WINDOWS_1252.decode(bytes);
            (text.into_owned(), "windows-1252", had_decode_errors)
        }
        _ => return None,
    };

    Some(DecodedResponse {
        body: text,
        encoding: encoding.to_string(),
        had_decode_errors,
    })
}

fn extract_charset_hint(value: &str) -> Option<String> {
    let lowered = value.to_ascii_lowercase();
    let marker = "charset";
    let mut cursor = 0;
    while let Some(relative) = lowered[cursor..].find(marker) {
        let start = cursor + relative + marker.len();
        let remainder = &lowered[start..];
        let Some(equals) = remainder.find('=') else {
            cursor = start;
            continue;
        };
        let token_start = start + equals + 1;
        let raw = &value[token_start..];
        let token = raw
            .trim_start_matches(|character: char| {
                character.is_ascii_whitespace() || character == '\'' || character == '"'
            })
            .split(|character: char| {
                character.is_ascii_whitespace() || matches!(character, ';' | '\'' | '"' | '>' | '/')
            })
            .next()
            .unwrap_or_default();
        if !token.is_empty() {
            return Some(token.to_string());
        }
        cursor = token_start.min(lowered.len());
    }
    None
}

fn normalize_charset_label(value: &str) -> Option<String> {
    let normalized = value
        .trim()
        .trim_matches(|character| character == '\'' || character == '"')
        .to_ascii_lowercase()
        .replace('_', "-");
    (!normalized.is_empty()).then_some(normalized)
}

fn replacement_char_count(value: &str) -> usize {
    value
        .chars()
        .filter(|character| *character == '\u{fffd}')
        .count()
}

fn is_suspicious_decoded_text(value: &str) -> bool {
    let value = value.trim();
    if value.is_empty() {
        return false;
    }
    if value.contains('\u{fffd}')
        || value
            .chars()
            .any(|character| character.is_control() && !matches!(character, '\n' | '\r' | '\t'))
    {
        return true;
    }
    ["Ã", "Â", "Ð", "Ñ", "â€", "ä¸", "æ–", "ï¿½"]
        .iter()
        .any(|marker| value.contains(marker))
}

fn downgrade_garbled_book_fields(book_info: &mut BookInfo) -> Vec<&'static str> {
    let mut downgraded = Vec::new();
    if is_suspicious_decoded_text(&book_info.title) {
        book_info.title = "未命名书籍".to_string();
        downgraded.push("title");
    }
    if book_info
        .author
        .as_deref()
        .is_some_and(is_suspicious_decoded_text)
    {
        book_info.author = None;
        downgraded.push("author");
    }
    if book_info
        .intro
        .as_deref()
        .is_some_and(is_suspicious_decoded_text)
    {
        book_info.intro = None;
        downgraded.push("intro");
    }
    downgraded
}

fn append_text_quality_debug_step(
    debug_steps: &mut Vec<SourceDebugStep>,
    url: &str,
    downgraded_fields: &[&str],
) {
    if downgraded_fields.is_empty() {
        return;
    }
    debug_steps.push(SourceDebugStep {
        stage: "book_info.text_quality".to_string(),
        url: redact_url(url),
        duration_ms: 0,
        status: None,
        bytes: None,
        error: None,
        variables: BTreeMap::from([
            ("reason".to_string(), "garbled_text_downgraded".to_string()),
            ("fields".to_string(), downgraded_fields.join(",")),
        ]),
        cache_hit: false,
    });
}

fn extract_document_rule(
    document: &Html,
    rule: Option<&SourceRule>,
) -> Result<Option<String>, SourceError> {
    let Some(rule) = rule else {
        return Ok(None);
    };
    match rule {
        SourceRule::Legacy { .. } => Ok(None),
        SourceRule::Chain { chain } => {
            for child in chain {
                match extract_document_rule(document, Some(child)) {
                    Ok(Some(value)) => return Ok(Some(value)),
                    Ok(None) | Err(SourceError::NoMatch) => continue,
                    Err(error) => return Err(error),
                }
            }
            Ok(None)
        }
        SourceRule::Join { join } => {
            let mut values = Vec::new();
            for child in join {
                match extract_document_rule(document, Some(child)) {
                    Ok(Some(value)) if !value.trim().is_empty() => values.push(value),
                    Ok(Some(_)) | Ok(None) | Err(SourceError::NoMatch) => {}
                    Err(error) => return Err(error),
                }
            }
            if values.is_empty() {
                Ok(None)
            } else {
                Ok(Some(values.join(" ")))
            }
        }
        _ => {
            let selector = parse_selector(rule.selector())?;
            let Some(element) = document.select(&selector).next() else {
                return Ok(None);
            };
            match extract_selected_element(element, rule) {
                Ok(value) if !value.trim().is_empty() => Ok(Some(value)),
                Ok(_) | Err(SourceError::NoMatch) => Ok(None),
                Err(error) => Err(error),
            }
        }
    }
}

fn extract_document_rule_with_fallback(
    document: &Html,
    rule: Option<&SourceRule>,
    candidates: &[(&str, Option<&str>)],
) -> Result<Option<String>, SourceError> {
    if let Some(value) = extract_document_rule(document, rule)? {
        return Ok(Some(value));
    }

    for (selector_text, attr) in candidates {
        let Ok(selector) = parse_selector(selector_text) else {
            continue;
        };
        let Some(element) = document.select(&selector).next() else {
            continue;
        };
        let value = match *attr {
            Some("html") => element.inner_html(),
            Some(attribute) => element
                .value()
                .attr(attribute)
                .unwrap_or_default()
                .to_string(),
            None => element.text().collect::<Vec<_>>().join(" "),
        };
        if let Some(value) = non_empty(Some(value.trim().to_string())) {
            return Ok(Some(value));
        }
    }

    Ok(None)
}

fn fallback_link_from_element(element: ElementRef<'_>) -> Option<(String, String)> {
    let selector = parse_selector("a[href]").ok()?;
    std::iter::once(element)
        .filter(|candidate| selector.matches(candidate))
        .chain(element.select(&selector))
        .find_map(|link| {
            let href = link.value().attr("href")?.trim();
            if !is_navigable_reference(href) {
                return None;
            }
            let title = link.text().collect::<Vec<_>>().join(" ");
            Some((href.to_string(), title.trim().to_string()))
        })
}

fn fallback_text_from_element(element: ElementRef<'_>) -> Option<String> {
    let text = element
        .text()
        .collect::<Vec<_>>()
        .join(" ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    non_empty(Some(text))
}

fn json_object_text(value: &Value, keys: &[&str]) -> Option<String> {
    let object = value.as_object()?;
    keys.iter().find_map(|key| {
        object
            .get(*key)
            .map(json_value_to_text)
            .and_then(|text| non_empty(Some(text.trim().to_string())))
    })
}

fn is_navigable_reference(value: &str) -> bool {
    let value = value.trim();
    if value.is_empty() || value.starts_with('#') {
        return false;
    }
    let lowered = value.to_ascii_lowercase();
    !["javascript:", "data:", "mailto:", "tel:"]
        .iter()
        .any(|prefix| lowered.starts_with(prefix))
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
        let fallback_link = fallback_link_from_element(item);
        let title = extract_document_rule_from_element(item, rules.title.as_ref())?
            .or_else(|| {
                fallback_link
                    .as_ref()
                    .and_then(|(_, title)| non_empty(Some(title.clone())))
            })
            .or_else(|| fallback_text_from_element(item));
        let url = extract_document_rule_from_element(item, rules.url.as_ref())?
            .or_else(|| fallback_link.as_ref().map(|(url, _)| url.clone()))
            .filter(|value| is_navigable_reference(value));
        let (Some(title), Some(url)) = (title, url) else {
            continue;
        };
        let url = absolutize_url(base_url, &url);
        chapters.push(SourceChapter { title, url, index });
    }

    if chapters.is_empty() {
        chapters = fallback_chapters_from_links(&document, base_url);
    }

    Ok(chapters)
}

fn fallback_chapters_from_links(document: &Html, base_url: &str) -> Vec<SourceChapter> {
    let Ok(selector) = parse_selector("a[href]") else {
        return Vec::new();
    };
    document
        .select(&selector)
        .take(256)
        .enumerate()
        .filter_map(|(index, link)| {
            let href = link.value().attr("href")?.trim();
            if !is_navigable_reference(href) {
                return None;
            }
            let title = link
                .text()
                .collect::<Vec<_>>()
                .join(" ")
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ");
            if title.is_empty() || !is_likely_chapter_link(link, &title, href) {
                return None;
            }
            Some(SourceChapter {
                title,
                url: absolutize_url(base_url, href),
                index,
            })
        })
        .collect()
}

fn is_likely_chapter_link(link: ElementRef<'_>, title: &str, href: &str) -> bool {
    let metadata = [
        href,
        link.value().attr("class").unwrap_or_default(),
        link.value().attr("id").unwrap_or_default(),
    ]
    .join(" ")
    .to_ascii_lowercase();
    let title = title.to_ascii_lowercase();
    metadata.contains("chapter")
        || metadata.contains("chap")
        || metadata.contains("section")
        || (title.contains('第')
            && ["章", "节", "回", "卷", "集"]
                .iter()
                .any(|marker| title.contains(marker)))
}

fn extract_document_rule_from_element(
    element: ElementRef<'_>,
    rule: Option<&SourceRule>,
) -> Result<Option<String>, SourceError> {
    let Some(rule) = rule else {
        return Ok(None);
    };
    extract_from_element_optional(element, rule)
}

pub fn chapter_fingerprint(chapters: &[SourceChapter]) -> String {
    let mut hash = 0xcbf29ce484222325_u64;
    for chapter in chapters {
        let fingerprint_value = format!(
            "index:{}|title:{}|url:{}",
            chapter.index,
            chapter.title.trim(),
            chapter.url.trim()
        );
        for byte in fingerprint_value.as_bytes() {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
        hash ^= 0xff;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

pub fn summarize_chapter_update(
    previous: &[SourceChapter],
    current: &[SourceChapter],
) -> ChapterUpdateSummary {
    let previous_keys: HashSet<String> = previous.iter().map(chapter_identity).collect();
    let current_keys: HashSet<String> = current.iter().map(chapter_identity).collect();
    let added = current_keys.difference(&previous_keys).count();
    let removed = previous_keys.difference(&current_keys).count();
    let retained = current_keys.intersection(&previous_keys).count();

    ChapterUpdateSummary {
        changed: chapter_fingerprint(previous) != chapter_fingerprint(current),
        fingerprint: chapter_fingerprint(current),
        added,
        removed,
        retained,
    }
}

fn chapter_identity(chapter: &SourceChapter) -> String {
    let url = chapter.url.trim();
    if url.is_empty() {
        format!("index:{}|title:{}", chapter.index, chapter.title.trim())
    } else {
        format!("url:{url}")
    }
}

fn render_url(
    template: &str,
    keyword: Option<&str>,
    book_url: Option<&str>,
    book_id: Option<&str>,
    chapter_id: Option<&str>,
) -> String {
    let mut context = SourceRequestContext {
        keyword: keyword.map(ToOwned::to_owned),
        page: 1,
        book_url: book_url.map(ToOwned::to_owned),
        book_id: book_id.map(ToOwned::to_owned),
        chapter_id: chapter_id.map(ToOwned::to_owned),
    };
    context.page = context.page.max(1);
    render_url_context(template, &context)
}

fn render_url_context(template: &str, context: &SourceRequestContext) -> String {
    let page = context.page.max(1);
    let mut result = template.to_string();
    if let Some(keyword) = context.keyword.as_deref() {
        result = result
            .replace("{{keyword}}", &encode_keyword(keyword))
            .replace("{{key}}", &encode_keyword(keyword));
    }
    result = result
        .replace("{{page}}", &page.to_string())
        .replace("{{pageNum}}", &page.to_string())
        .replace("{{pageIndex}}", &page.saturating_sub(1).to_string())
        .replace("{{page_index}}", &page.saturating_sub(1).to_string())
        .replace("{{page+1}}", &page.saturating_add(1).to_string())
        .replace("{{page-1}}", &page.saturating_sub(1).to_string());
    if let Some(book_url) = context.book_url.as_deref() {
        result = result
            .replace("{{bookUrl}}", book_url)
            .replace("{{book_url}}", book_url);
    }
    if let Some(book_id) = context.book_id.as_deref() {
        result = result
            .replace("{{bookId}}", book_id)
            .replace("{{book_id}}", book_id);
    }
    if let Some(chapter_id) = context.chapter_id.as_deref() {
        result = result
            .replace("{{chapterId}}", chapter_id)
            .replace("{{chapter_id}}", chapter_id);
    }
    result
}

fn bounded_stage_timeout(timeout_secs: u64) -> Duration {
    Duration::from_secs(
        timeout_secs
            .saturating_mul(MAX_URL_CHAIN_LENGTH as u64)
            .min(MAX_STAGE_BUDGET_SECS),
    )
}

fn bounded_pipeline_timeout(timeout_secs: u64) -> Duration {
    Duration::from_secs(timeout_secs.saturating_mul(4).min(MAX_PIPELINE_BUDGET_SECS))
}

fn bounded_search_pages(requested_pages: usize) -> usize {
    requested_pages.clamp(1, MAX_SOURCE_SEARCH_PAGES)
}

fn search_result_identity(item: &SearchResult) -> String {
    format!(
        "{}|{}|{}",
        normalize_search_text(&item.title),
        normalize_search_text(item.author.as_deref().unwrap_or_default()),
        normalize_search_text(item.book_url.as_deref().unwrap_or_default())
    )
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

fn bounded_next_url(url: &str) -> Result<Option<String>, SourceError> {
    let url = url.trim();
    if url.is_empty() {
        return Ok(None);
    }
    if url.len() > MAX_NEXT_URL_BYTES {
        return Err(SourceError::InvalidUrl(format!(
            "next URL 不能超过 {} 字节",
            MAX_NEXT_URL_BYTES
        )));
    }
    if url.contains("||") {
        return Err(SourceError::InvalidUrl("next URL 不支持回退链".to_string()));
    }
    validate_url(url)?;
    Ok(Some(url.to_string()))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NextPagePolicy {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_next_page_depth")]
    pub max_depth: usize,
    #[serde(default = "default_next_page_count")]
    pub max_pages: usize,
    #[serde(default = "default_next_page_bytes")]
    pub max_bytes: usize,
    #[serde(default = "default_next_page_duration")]
    pub max_duration_secs: u64,
    #[serde(default = "default_next_page_same_host")]
    pub same_host_only: bool,
}

fn default_next_page_depth() -> usize {
    2
}

fn default_next_page_count() -> usize {
    3
}

fn default_next_page_bytes() -> usize {
    2 * 1024 * 1024
}

fn default_next_page_duration() -> u64 {
    15
}

fn default_next_page_same_host() -> bool {
    true
}

impl Default for NextPagePolicy {
    fn default() -> Self {
        Self {
            enabled: false,
            max_depth: default_next_page_depth(),
            max_pages: default_next_page_count(),
            max_bytes: default_next_page_bytes(),
            max_duration_secs: default_next_page_duration(),
            same_host_only: default_next_page_same_host(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NextPagePolicyDecision {
    pub allowed: bool,
    pub reason: String,
    pub next_depth: usize,
    pub pages_remaining: usize,
    pub bytes_remaining: usize,
}

pub fn evaluate_next_page_policy(
    policy: &NextPagePolicy,
    base_url: &str,
    candidate_url: &str,
    depth: usize,
    pages_used: usize,
    bytes_used: usize,
    elapsed_secs: u64,
    visited_urls: &[String],
) -> NextPagePolicyDecision {
    let bounded = |allowed: bool,
                   reason: &str,
                   next_depth: usize,
                   pages_remaining: usize,
                   bytes_remaining: usize| {
        NextPagePolicyDecision {
            allowed,
            reason: reason.to_string(),
            next_depth,
            pages_remaining,
            bytes_remaining,
        }
    };

    let max_depth = policy.max_depth.min(default_next_page_depth());
    let max_pages = policy.max_pages.min(default_next_page_count());
    let max_bytes = policy.max_bytes.min(default_next_page_bytes());
    let max_duration_secs = policy.max_duration_secs.min(default_next_page_duration());

    if !policy.enabled {
        return bounded(
            false,
            "disabled",
            depth,
            max_pages.saturating_sub(pages_used),
            max_bytes.saturating_sub(bytes_used),
        );
    }
    if max_depth == 0 || max_pages == 0 {
        return bounded(false, "quota_zero", depth, 0, 0);
    }
    if depth >= max_depth {
        return bounded(
            false,
            "depth_limit",
            depth,
            0,
            max_bytes.saturating_sub(bytes_used),
        );
    }
    if pages_used >= max_pages {
        return bounded(
            false,
            "page_limit",
            depth,
            0,
            max_bytes.saturating_sub(bytes_used),
        );
    }
    if bytes_used >= max_bytes {
        return bounded(
            false,
            "byte_limit",
            depth,
            max_pages.saturating_sub(pages_used),
            0,
        );
    }
    if elapsed_secs >= max_duration_secs {
        return bounded(
            false,
            "time_limit",
            depth,
            max_pages.saturating_sub(pages_used),
            max_bytes.saturating_sub(bytes_used),
        );
    }

    let Some(candidate) = bounded_next_url(candidate_url).ok().flatten() else {
        return bounded(
            false,
            "invalid_next_url",
            depth,
            max_pages.saturating_sub(pages_used),
            max_bytes.saturating_sub(bytes_used),
        );
    };
    if visited_urls.iter().any(|visited| visited == &candidate) {
        return bounded(
            false,
            "cycle",
            depth,
            max_pages.saturating_sub(pages_used),
            max_bytes.saturating_sub(bytes_used),
        );
    }

    let base = match Url::parse(base_url) {
        Ok(base) => base,
        Err(_) => {
            return bounded(
                false,
                "invalid_base_url",
                depth,
                max_pages.saturating_sub(pages_used),
                max_bytes.saturating_sub(bytes_used),
            );
        }
    };
    let candidate = match Url::parse(&candidate) {
        Ok(candidate) => candidate,
        Err(_) => {
            return bounded(
                false,
                "invalid_next_url",
                depth,
                max_pages.saturating_sub(pages_used),
                max_bytes.saturating_sub(bytes_used),
            );
        }
    };
    if policy.same_host_only && !same_url_origin(&base, &candidate) {
        return bounded(
            false,
            "same_origin",
            depth,
            max_pages.saturating_sub(pages_used),
            max_bytes.saturating_sub(bytes_used),
        );
    }

    bounded(
        true,
        "allowed",
        depth.saturating_add(1),
        max_pages.saturating_sub(pages_used.saturating_add(1)),
        max_bytes.saturating_sub(bytes_used),
    )
}

fn same_url_origin(left: &Url, right: &Url) -> bool {
    left.scheme() == right.scheme()
        && left.host_str() == right.host_str()
        && left.port_or_known_default() == right.port_or_known_default()
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
    if let Some(value) = source.source_url.as_deref() {
        validate_endpoint("bookSourceUrl", value, &mut errors);
    }
    if let Some(value) = source.explore_url.as_deref() {
        validate_endpoint("exploreUrl", value, &mut errors);
    }
    if source.source_type != 0 {
        warnings.push(
            "bookSourceType 不是文本类型；元数据会保存，但当前阅读执行器不会执行音频/TTS 规则"
                .to_string(),
        );
    }
    for (key, value) in &source.legacy_urls {
        warnings.push(format!(
            "{key} 已保留原始动态/相对表达式，当前不会执行：{value}"
        ));
    }
    if source.legacy_headers.is_some() {
        warnings
            .push("headers 已保留原始动态或非标准对象，当前不会执行其中的兼容表达式".to_string());
    }
    if source.enabled_explore && source.explore_url.is_none() {
        warnings.push("enabledExplore 已开启，但未配置 exploreUrl".to_string());
    }
    if source
        .group
        .as_ref()
        .is_some_and(|value| value.len() > MAX_SOURCE_GROUP_BYTES)
    {
        errors.push(format!(
            "bookSourceGroup 不能超过 {} 字节",
            MAX_SOURCE_GROUP_BYTES
        ));
    }
    if source
        .comment
        .as_ref()
        .is_some_and(|value| value.len() > MAX_SOURCE_COMMENT_BYTES)
    {
        errors.push(format!(
            "bookSourceComment 不能超过 {} 字节",
            MAX_SOURCE_COMMENT_BYTES
        ));
    }
    if source
        .book_url_pattern
        .as_ref()
        .is_some_and(|value| value.len() > MAX_BOOK_URL_PATTERN_BYTES)
    {
        errors.push(format!(
            "bookUrlPattern 不能超过 {} 字节",
            MAX_BOOK_URL_PATTERN_BYTES
        ));
    }
    if source.weight < -MAX_SOURCE_WEIGHT || source.weight > MAX_SOURCE_WEIGHT {
        errors.push(format!("weight 必须在 ±{} 范围内", MAX_SOURCE_WEIGHT));
    }
    if source.custom_order < -MAX_SOURCE_CUSTOM_ORDER
        || source.custom_order > MAX_SOURCE_CUSTOM_ORDER
    {
        errors.push(format!(
            "customOrder 必须在 ±{} 范围内",
            MAX_SOURCE_CUSTOM_ORDER
        ));
    }
    validate_permission(&source.permission, &mut errors);
    if !source.legacy_urls.contains_key("searchUrl") {
        validate_endpoint("searchUrl", &source.search_url, &mut errors);
    }
    for (name, value) in [
        ("bookInfoUrl", source.book_info_url.as_deref()),
        ("tocUrl", source.toc_url.as_deref()),
        ("contentUrl", source.content_url.as_deref()),
    ] {
        if let Some(value) = value {
            if !source.legacy_urls.contains_key(name) {
                validate_endpoint(name, value, &mut errors);
            }
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
    validate_replace_rules(&source.replace_rules, &mut errors, &mut warnings);

    if source.book_info_url.is_none() {
        warnings.push("未配置 bookInfoUrl，将使用搜索结果详情页作为回退地址".to_string());
    }
    if source.toc_url.is_none() {
        warnings.push("未配置 tocUrl，将使用书籍详情页作为回退地址".to_string());
    }
    if source.content_url.is_none() {
        warnings.push("未配置 contentUrl，将使用章节链接作为回退地址".to_string());
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

pub fn audit_source_json(input: &str) -> SourceSecurityAudit {
    let validation = validate_source_json(input);
    let Some(source) = validation.source else {
        return SourceSecurityAudit {
            permission_status: "unknown".to_string(),
            permission_scope: None,
            reviewed_at: None,
            hosts: Vec::new(),
            sensitive_headers: Vec::new(),
            errors: validation.errors,
            warnings: validation.warnings,
            pass: false,
        };
    };

    let mut warnings = validation.warnings;
    let errors = validation.errors;
    let status = source.permission.status.trim().to_ascii_lowercase();
    let scope = source
        .permission
        .scope
        .as_ref()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    if status == "unknown" {
        warnings.push("permission.status 尚未确认，发布前请补充授权范围".to_string());
    }
    if scope.is_none() {
        warnings.push("permission.scope 未填写，无法追踪来源授权范围".to_string());
    }
    if source.permission.reviewed_at.is_none() {
        warnings.push("permission.reviewedAt 未填写，建议记录最近审核日期".to_string());
    }

    let sensitive_headers = sensitive_header_names(&source);
    SourceSecurityAudit {
        permission_status: status,
        permission_scope: scope,
        reviewed_at: source.permission.reviewed_at.clone(),
        hosts: source_endpoint_hosts(&source),
        sensitive_headers,
        pass: errors.is_empty(),
        errors,
        warnings,
    }
}

fn validate_permission(permission: &SourcePermission, errors: &mut Vec<String>) {
    let status = permission.status.trim().to_ascii_lowercase();
    if !matches!(
        status.as_str(),
        "unknown" | "authorized" | "public_domain" | "personal"
    ) {
        errors.push(format!("permission.status 不支持：{}", permission.status));
    }
    if permission
        .scope
        .as_ref()
        .is_some_and(|value| value.len() > MAX_PERMISSION_SCOPE_BYTES)
    {
        errors.push(format!(
            "permission.scope 不能超过 {} 字节",
            MAX_PERMISSION_SCOPE_BYTES
        ));
    }
    if permission
        .reviewed_at
        .as_ref()
        .is_some_and(|value| value.len() > MAX_PERMISSION_REVIEWED_AT_BYTES)
    {
        errors.push(format!(
            "permission.reviewedAt 不能超过 {} 字节",
            MAX_PERMISSION_REVIEWED_AT_BYTES
        ));
    }
}

fn source_endpoint_hosts(source: &BookSource) -> Vec<String> {
    let mut hosts = HashSet::new();
    for endpoint in [
        source.source_url.as_deref(),
        source.explore_url.as_deref(),
        (!source.legacy_urls.contains_key("searchUrl")).then_some(source.search_url.as_str()),
        source.book_info_url.as_deref(),
        source.toc_url.as_deref(),
        source.content_url.as_deref(),
    ]
    .into_iter()
    .flatten()
    {
        let candidates = split_url_chain(endpoint).unwrap_or_else(|_| vec![endpoint]);
        for candidate in candidates {
            if let Ok(parsed) = Url::parse(&expand_url_template(candidate)) {
                if let Some(host) = parsed.host_str() {
                    hosts.insert(host.to_string());
                }
            }
        }
    }
    let mut hosts = hosts.into_iter().collect::<Vec<_>>();
    hosts.sort();
    hosts
}

fn sensitive_header_names(source: &BookSource) -> Vec<String> {
    let mut headers = source
        .headers
        .keys()
        .filter(|name| {
            matches!(
                name.to_ascii_lowercase().as_str(),
                "authorization" | "cookie" | "proxy-authorization"
            )
        })
        .cloned()
        .collect::<Vec<_>>();
    headers.sort();
    headers
}

fn validate_replace_rules(
    rules: &[ReplaceRule],
    errors: &mut Vec<String>,
    warnings: &mut Vec<String>,
) {
    if rules.len() > MAX_REPLACE_RULES {
        errors.push(format!(
            "replaceRules 最多支持 {} 条规则",
            MAX_REPLACE_RULES
        ));
    }

    for (index, rule) in rules.iter().enumerate() {
        let label = format!("replaceRules[{index}]");
        if rule.pattern.trim().is_empty() {
            errors.push(format!("{label}.pattern 不能为空"));
        }
        if rule.pattern.len() > MAX_REPLACE_PATTERN_BYTES {
            errors.push(format!(
                "{label}.pattern 不能超过 {} 字节",
                MAX_REPLACE_PATTERN_BYTES
            ));
        }
        if rule.replacement.len() > MAX_REPLACE_REPLACEMENT_BYTES {
            errors.push(format!(
                "{label}.replacement 不能超过 {} 字节",
                MAX_REPLACE_REPLACEMENT_BYTES
            ));
        }
        if let Err(error) = Regex::new(&rule.pattern) {
            errors.push(format!("{label}.pattern：{error}"));
        }
        if !rule.enabled {
            warnings.push(format!("{label} 当前已停用"));
        }
    }
}

fn apply_replace_rules(content: &str, rules: &[ReplaceRule]) -> Result<String, SourceError> {
    let mut result = content.to_string();
    for rule in rules.iter().filter(|rule| rule.enabled) {
        let regex = Regex::new(&rule.pattern)
            .map_err(|error| SourceError::InvalidRegex(error.to_string()))?;
        result = regex
            .replace_all(&result, rule.replacement.as_str())
            .into_owned();
    }
    Ok(result)
}

fn ensure_runtime_endpoint(source: &BookSource, key: &str) -> Result<(), SourceError> {
    if source.legacy_urls.contains_key(key) {
        return Err(SourceError::InvalidConfig(format!(
            "{key} 仅保留原始兼容表达式，当前不会执行"
        )));
    }
    Ok(())
}

fn runtime_endpoint_or_fallback<'a>(
    source: &BookSource,
    key: &str,
    configured: Option<&'a str>,
    fallback: &'a str,
) -> &'a str {
    if source.legacy_urls.contains_key(key) {
        return fallback;
    }
    configured
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(fallback)
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
        let result = if is_json_rule_path(item) {
            validate_json_path(item)
        } else {
            parse_selector(item).map(|_| ())
        };
        if let Err(error) = result {
            errors.push(format!("{name}.item：{error}"));
        }
    }
    if rules.item_legacy.is_some() {
        warnings.push(format!(
            "{name}.item 已保留原始兼容选择器，当前不会执行该 item 规则"
        ));
    }

    for (field, rule) in [
        ("title", rules.title.as_ref()),
        ("author", rules.author.as_ref()),
        ("url", rules.url.as_ref()),
        ("intro", rules.intro.as_ref()),
        ("content", rules.content.as_ref()),
        ("next", rules.next.as_ref()),
    ] {
        if let Some(rule) = rule {
            validate_source_rule(&format!("{name}.{field}"), rule, errors, warnings);
        }
    }
}

fn validate_source_rule(
    name: &str,
    rule: &SourceRule,
    errors: &mut Vec<String>,
    warnings: &mut Vec<String>,
) {
    if let SourceRule::Legacy { reason, .. } = rule {
        warnings.push(format!(
            "{name} 已保留原始兼容规则，当前不执行{}",
            reason
                .as_deref()
                .map(|value| format!("：{value}"))
                .unwrap_or_default()
        ));
        return;
    }

    let compound = match rule {
        SourceRule::Chain { chain } => Some(("链式规则", chain)),
        SourceRule::Join { join } => Some(("组合规则", join)),
        _ => None,
    };
    if let Some((label, children)) = compound {
        if children.is_empty() {
            errors.push(format!("{name} {label}不能为空"));
            return;
        }
        if children.len() > MAX_RULE_CHAIN_LENGTH {
            errors.push(format!(
                "{name} {label}最多支持 {} 个候选",
                MAX_RULE_CHAIN_LENGTH
            ));
        }
        let usable = children
            .iter()
            .filter(|child| !child.is_legacy())
            .collect::<Vec<_>>();
        let json_flags = usable
            .iter()
            .map(|rule| rule.is_json_path())
            .collect::<Vec<_>>();
        if json_flags.iter().any(|flag| *flag) && json_flags.iter().any(|flag| !*flag) {
            errors.push(format!("{name} {label}不能混用 CSS 和 JSONPath"));
        }
        for (index, child) in children.iter().enumerate() {
            validate_source_rule(&format!("{name}[{index}]"), child, errors, warnings);
        }
        return;
    }

    if rule.selector().trim().is_empty() {
        errors.push(format!("{name} selector 不能为空"));
        return;
    }
    let result = if let Some(path) = rule.json_path() {
        validate_json_path(path)
    } else {
        parse_selector(rule.selector()).map(|_| ())
    };
    if let Err(error) = result {
        errors.push(format!("{name}：{error}"));
    }
    if let Some(regex) = rule.regex() {
        if let Err(error) = Regex::new(regex) {
            errors.push(format!("{name} regex：{error}"));
        }
    }
    if rule.replacement().is_some() && rule.regex().is_none() {
        errors.push(format!("{name} replacement 必须与 regex 一起使用"));
    }
    if rule
        .replacement()
        .is_some_and(|replacement| replacement.len() > MAX_REPLACE_REPLACEMENT_BYTES)
    {
        errors.push(format!(
            "{name} replacement 不能超过 {} 字节",
            MAX_REPLACE_REPLACEMENT_BYTES
        ));
    }
}

fn split_url_chain(value: &str) -> Result<Vec<&str>, SourceError> {
    let parts = value.split("||").map(str::trim).collect::<Vec<_>>();
    if parts.is_empty() || parts.iter().any(|part| part.is_empty()) {
        return Err(SourceError::InvalidUrl("URL 回退链包含空候选".to_string()));
    }
    if parts.len() > MAX_URL_CHAIN_LENGTH {
        return Err(SourceError::InvalidUrl(format!(
            "URL 回退链最多支持 {} 个候选",
            MAX_URL_CHAIN_LENGTH
        )));
    }
    Ok(parts)
}

fn render_url_chain(
    template: &str,
    context: &SourceRequestContext,
) -> Result<Vec<String>, SourceError> {
    Ok(split_url_chain(template)?
        .into_iter()
        .map(|candidate| render_url_context(candidate, context))
        .collect())
}

fn validate_url(url: &str) -> Result<(), SourceError> {
    for candidate in split_url_chain(url)? {
        let parsed = Url::parse(&expand_url_template(candidate))
            .map_err(|error| SourceError::InvalidUrl(error.to_string()))?;
        match parsed.scheme() {
            "http" | "https" => {}
            scheme => return Err(SourceError::UnsupportedScheme(scheme.to_string())),
        }
    }
    Ok(())
}

fn expand_url_template(url: &str) -> String {
    let context = SourceRequestContext {
        keyword: Some("open-reader".to_string()),
        page: 1,
        ..SourceRequestContext::default()
    };
    render_url_context(url, &context)
}

fn is_json_rule_path(value: &str) -> bool {
    let value = value.trim();
    value.starts_with('$') || value.starts_with("json:")
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum JsonPathSegment {
    Field(String),
    ObjectWildcard,
    ArrayWildcard {
        key: Option<String>,
    },
    ArrayIndex {
        key: Option<String>,
        index: usize,
    },
    ArrayFilter {
        key: Option<String>,
        field: String,
        expected: String,
    },
}

fn split_json_path_segments(path: &str) -> Result<Vec<String>, SourceError> {
    let mut segments = Vec::new();
    let mut current = String::new();
    let mut bracket_depth = 0usize;
    let mut characters = path.chars().peekable();

    while let Some(character) = characters.next() {
        match character {
            '[' => {
                bracket_depth = bracket_depth
                    .checked_add(1)
                    .ok_or_else(|| SourceError::InvalidJsonPath(path.to_string()))?;
                current.push(character);
            }
            ']' => {
                if bracket_depth == 0 {
                    return Err(SourceError::InvalidJsonPath(path.to_string()));
                }
                bracket_depth -= 1;
                current.push(character);
                if bracket_depth == 0 && characters.peek() == Some(&'[') {
                    if current.is_empty() {
                        return Err(SourceError::InvalidJsonPath(path.to_string()));
                    }
                    segments.push(std::mem::take(&mut current));
                }
            }
            '.' if bracket_depth == 0 => {
                if current.is_empty() {
                    return Err(SourceError::InvalidJsonPath(path.to_string()));
                }
                segments.push(std::mem::take(&mut current));
            }
            _ => current.push(character),
        }
    }

    if bracket_depth != 0 || current.is_empty() {
        return Err(SourceError::InvalidJsonPath(path.to_string()));
    }
    segments.push(current);
    Ok(segments)
}

fn validate_json_field(field: &str, path: &str) -> Result<String, SourceError> {
    let field = field.trim();
    if field.is_empty()
        || field.len() > MAX_JSON_FILTER_FIELD_BYTES
        || field.chars().any(|character| {
            matches!(
                character,
                '[' | ']' | '?' | '(' | ')' | '=' | '!' | '&' | '|' | '\\' | '\'' | '"'
            )
        })
    {
        return Err(SourceError::InvalidJsonPath(path.to_string()));
    }
    Ok(field.to_string())
}

fn parse_json_filter(inner: &str, path: &str) -> Result<(String, String), SourceError> {
    let (raw_field, raw_value) = inner
        .trim()
        .split_once("==")
        .ok_or_else(|| SourceError::InvalidJsonPath(path.to_string()))?;
    if raw_value.contains("==")
        || raw_value.contains("!=")
        || raw_value.contains("&&")
        || raw_value.contains("||")
        || raw_value.contains('@')
    {
        return Err(SourceError::InvalidJsonPath(path.to_string()));
    }

    let field = raw_field
        .trim()
        .strip_prefix("@.")
        .ok_or_else(|| SourceError::InvalidJsonPath(path.to_string()))?;
    let field = validate_json_field(field, path)?;
    let value = raw_value.trim();
    let quote = value
        .chars()
        .next()
        .filter(|character| *character == '\'' || *character == '"')
        .ok_or_else(|| SourceError::InvalidJsonPath(path.to_string()))?;
    if value.len() < 2 || !value.ends_with(quote) {
        return Err(SourceError::InvalidJsonPath(path.to_string()));
    }
    let expected = &value[1..value.len() - 1];
    if expected.len() > MAX_JSON_FILTER_VALUE_BYTES
        || expected.contains('\\')
        || expected.contains('\n')
        || expected.contains('\r')
    {
        return Err(SourceError::InvalidJsonPath(path.to_string()));
    }
    Ok((field, expected.to_string()))
}

fn parse_json_bracket(
    bracket: &str,
    key: Option<String>,
    path: &str,
) -> Result<JsonPathSegment, SourceError> {
    if bracket == "[*]" {
        return Ok(JsonPathSegment::ArrayWildcard { key });
    }

    let inner = bracket
        .strip_prefix('[')
        .and_then(|value| value.strip_suffix(']'))
        .ok_or_else(|| SourceError::InvalidJsonPath(path.to_string()))?
        .trim();

    if let Ok(index) = inner.parse::<usize>() {
        return Ok(JsonPathSegment::ArrayIndex { key, index });
    }

    if let Some(filter) = inner
        .strip_prefix("?(")
        .and_then(|value| value.strip_suffix(')'))
    {
        let (field, expected) = parse_json_filter(filter, path)?;
        return Ok(JsonPathSegment::ArrayFilter {
            key,
            field,
            expected,
        });
    }

    if key.is_none()
        && inner.len() >= 2
        && ((inner.starts_with('\'') && inner.ends_with('\''))
            || (inner.starts_with('"') && inner.ends_with('"')))
    {
        let field = validate_json_field(&inner[1..inner.len() - 1], path)?;
        return Ok(JsonPathSegment::Field(field));
    }

    Err(SourceError::InvalidJsonPath(path.to_string()))
}

fn parse_json_path_segment(segment: &str, path: &str) -> Result<JsonPathSegment, SourceError> {
    if segment == "*" {
        return Ok(JsonPathSegment::ObjectWildcard);
    }
    if segment == "[*]" {
        return Ok(JsonPathSegment::ArrayWildcard { key: None });
    }

    if let Some(open) = segment.find('[') {
        let base = &segment[..open];
        let bracket = &segment[open..];
        if !bracket.ends_with(']') || bracket.contains('[') && bracket[1..].contains('[') {
            return Err(SourceError::InvalidJsonPath(path.to_string()));
        }
        let key = if base.is_empty() {
            None
        } else {
            Some(validate_json_field(base, path)?)
        };
        return parse_json_bracket(bracket, key, path);
    }

    if let Ok(index) = segment.parse::<usize>() {
        return Ok(JsonPathSegment::ArrayIndex { key: None, index });
    }

    Ok(JsonPathSegment::Field(validate_json_field(segment, path)?))
}

fn normalize_json_path(path: &str) -> Result<String, SourceError> {
    let trimmed = path.trim();
    let path = trimmed.strip_prefix("json:").unwrap_or(trimmed).trim();

    if path.is_empty() || path.len() > MAX_JSON_PATH_BYTES || !path.starts_with('$') {
        return Err(SourceError::InvalidJsonPath(path.to_string()));
    }
    if path == "$" {
        return Ok(path.to_string());
    }

    let relative = path
        .strip_prefix("$.")
        .or_else(|| path.strip_prefix('$'))
        .unwrap_or(path);
    if relative.is_empty() {
        return Ok("$".to_string());
    }

    for segment in split_json_path_segments(relative)? {
        parse_json_path_segment(&segment, path)?;
    }

    Ok(path.to_string())
}

fn validate_json_path(path: &str) -> Result<(), SourceError> {
    normalize_json_path(path).map(|_| ())
}

fn extract_json_nodes<'a>(value: &'a Value, path: &str) -> Result<Vec<&'a Value>, SourceError> {
    let path = normalize_json_path(path)?;
    if path == "$" {
        return Ok(vec![value]);
    }

    let relative = path
        .strip_prefix("$.")
        .or_else(|| path.strip_prefix('$'))
        .unwrap_or(&path);
    let segments = split_json_path_segments(relative)?
        .into_iter()
        .map(|segment| parse_json_path_segment(&segment, &path))
        .collect::<Result<Vec<_>, _>>()?;
    let mut current = vec![value];

    for segment in segments {
        let mut next = Vec::new();
        for item in current {
            match &segment {
                JsonPathSegment::Field(key) => {
                    if let Some(child) = item.get(key) {
                        next.push(child);
                    }
                }
                JsonPathSegment::ObjectWildcard => match item {
                    Value::Object(object) => next.extend(object.values()),
                    Value::Array(array) => next.extend(array.iter()),
                    Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
                },
                JsonPathSegment::ArrayWildcard { key } => {
                    let candidate = key.as_deref().and_then(|key| item.get(key)).unwrap_or(item);
                    if let Some(array) = candidate.as_array() {
                        next.extend(array.iter());
                    }
                }
                JsonPathSegment::ArrayIndex { key, index } => {
                    let candidate = key.as_deref().and_then(|key| item.get(key)).unwrap_or(item);
                    if let Some(child) = candidate.as_array().and_then(|array| array.get(*index)) {
                        next.push(child);
                    }
                }
                JsonPathSegment::ArrayFilter {
                    key,
                    field,
                    expected,
                } => {
                    let candidate = key.as_deref().and_then(|key| item.get(key)).unwrap_or(item);
                    if let Some(array) = candidate.as_array() {
                        next.extend(array.iter().filter(|entry| {
                            entry
                                .get(field)
                                .map(|value| json_value_to_text(value) == *expected)
                                .unwrap_or(false)
                        }));
                    }
                }
            }
        }
        if next.len() > MAX_JSON_MATCHES {
            return Err(SourceError::InvalidJsonPath(format!(
                "JSONPath 匹配结果超过 {} 项上限",
                MAX_JSON_MATCHES
            )));
        }
        current = next;
    }

    if current.is_empty() {
        return Err(SourceError::NoMatch);
    }

    Ok(current)
}

fn parse_selector(value: &str) -> Result<Selector, SourceError> {
    Selector::parse(value).map_err(|error| SourceError::InvalidSelector(format!("{error:?}")))
}

fn extract_from_element(element: ElementRef<'_>, rule: &SourceRule) -> Result<String, SourceError> {
    match rule {
        SourceRule::Legacy { .. } => Err(SourceError::NoMatch),
        SourceRule::Chain { chain } => {
            for child in chain {
                match extract_from_element(element.clone(), child) {
                    Ok(value) => return Ok(value),
                    Err(SourceError::NoMatch) => continue,
                    Err(error) => return Err(error),
                }
            }
            Err(SourceError::NoMatch)
        }
        SourceRule::Join { join } => {
            let mut values = Vec::new();
            for child in join {
                match extract_from_element(element.clone(), child) {
                    Ok(value) if !value.trim().is_empty() => values.push(value),
                    Ok(_) | Err(SourceError::NoMatch) => {}
                    Err(error) => return Err(error),
                }
            }
            if values.is_empty() {
                Err(SourceError::NoMatch)
            } else {
                Ok(values.join(" "))
            }
        }
        _ => {
            let selector = parse_selector(rule.selector())?;
            let target =
                select_first_including_self(element, &selector).ok_or(SourceError::NoMatch)?;
            extract_selected_element(target, rule)
        }
    }
}

fn select_first_including_self<'a>(
    element: ElementRef<'a>,
    selector: &Selector,
) -> Option<ElementRef<'a>> {
    if selector.matches(&element) {
        Some(element)
    } else {
        element.select(selector).next()
    }
}

fn extract_from_element_optional(
    element: ElementRef<'_>,
    rule: &SourceRule,
) -> Result<Option<String>, SourceError> {
    match extract_from_element(element, rule) {
        Ok(value) if !value.trim().is_empty() => Ok(Some(value)),
        Ok(_) | Err(SourceError::NoMatch) => Ok(None),
        Err(error) => Err(error),
    }
}

fn extract_selected_element(
    element: ElementRef<'_>,
    rule: &SourceRule,
) -> Result<String, SourceError> {
    let value = match rule.attr() {
        Some("html") => element.inner_html(),
        Some(attribute) => element
            .value()
            .attr(attribute)
            .unwrap_or_default()
            .to_string(),
        None => element.text().collect::<Vec<_>>().join(" "),
    };

    apply_regex(value.trim(), rule.regex(), rule.replacement())
}

fn apply_regex(
    value: &str,
    pattern: Option<&str>,
    replacement: Option<&str>,
) -> Result<String, SourceError> {
    let Some(pattern) = pattern else {
        return Ok(value.to_string());
    };
    let regex =
        Regex::new(pattern).map_err(|error| SourceError::InvalidRegex(error.to_string()))?;
    if let Some(replacement) = replacement {
        if !regex.is_match(value) {
            return Err(SourceError::NoMatch);
        }
        return Ok(regex.replace_all(value, replacement).into_owned());
    }
    let captures = regex.captures(value).ok_or(SourceError::NoMatch)?;
    Ok(captures
        .get(1)
        .or_else(|| captures.get(0))
        .map(|capture| capture.as_str().to_string())
        .unwrap_or_default())
}

fn json_value_to_text(value: &Value) -> String {
    match value {
        Value::String(value) => value.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

fn extract_json_path(value: &Value, path: &str) -> Result<Vec<String>, SourceError> {
    extract_json_nodes(value, path)
        .map(|values| values.into_iter().map(json_value_to_text).collect())
}

fn extract_json_rule(value: &Value, rule: &SourceRule) -> Result<String, SourceError> {
    match rule {
        SourceRule::Legacy { .. } => Err(SourceError::NoMatch),
        SourceRule::Chain { chain } => {
            for child in chain {
                match extract_json_rule(value, child) {
                    Ok(value) => return Ok(value),
                    Err(SourceError::NoMatch) => continue,
                    Err(error) => return Err(error),
                }
            }
            Err(SourceError::NoMatch)
        }
        SourceRule::Join { join } => {
            let mut values = Vec::new();
            for child in join {
                match extract_json_rule(value, child) {
                    Ok(value) if !value.trim().is_empty() => values.push(value),
                    Ok(_) | Err(SourceError::NoMatch) => {}
                    Err(error) => return Err(error),
                }
            }
            if values.is_empty() {
                Err(SourceError::NoMatch)
            } else {
                Ok(values.join(" "))
            }
        }
        _ => {
            let path = rule.json_path().ok_or_else(|| {
                SourceError::InvalidConfig("JSON 文档中的字段必须使用 JSONPath 规则".to_string())
            })?;
            let selected = extract_json_nodes(value, path)?
                .into_iter()
                .next()
                .ok_or(SourceError::NoMatch)?;
            let selected = if let Some(attribute) = rule.attr() {
                selected.get(attribute).ok_or(SourceError::NoMatch)?
            } else {
                selected
            };
            let text = json_value_to_text(selected);
            apply_regex(text.trim(), rule.regex(), rule.replacement())
        }
    }
}

fn extract_json_rule_optional(
    value: &Value,
    rule: Option<&SourceRule>,
) -> Result<Option<String>, SourceError> {
    let Some(rule) = rule else {
        return Ok(None);
    };
    match extract_json_rule(value, rule) {
        Ok(value) => Ok(Some(value)),
        Err(SourceError::NoMatch) => Ok(None),
        Err(error) => Err(error),
    }
}

fn first_json_context<'a>(
    value: &'a Value,
    item_path: Option<&str>,
) -> Result<&'a Value, SourceError> {
    extract_json_nodes(value, item_path.unwrap_or("$"))?
        .into_iter()
        .next()
        .ok_or(SourceError::NoMatch)
}

fn parse_json_rule_document(
    body: &str,
    item_path: Option<&str>,
    rule: Option<&SourceRule>,
) -> Result<Option<String>, SourceError> {
    let value: Value =
        serde_json::from_str(body).map_err(|error| SourceError::InvalidJson(error.to_string()))?;
    let context = first_json_context(&value, item_path)?;
    extract_json_rule_optional(context, rule)
}

fn parse_book_info_json(
    rules: &PageRules,
    body: &str,
    book_url: &str,
) -> Result<BookInfo, SourceError> {
    let value: Value =
        serde_json::from_str(body).map_err(|error| SourceError::InvalidJson(error.to_string()))?;
    let context = first_json_context(&value, rules.item.as_deref())?;
    Ok(BookInfo {
        title: extract_json_rule_optional(context, rules.title.as_ref())?
            .or_else(|| json_object_text(context, &["title", "name", "bookName"]))
            .unwrap_or_else(|| "未命名书籍".to_string()),
        author: non_empty(
            extract_json_rule_optional(context, rules.author.as_ref())?
                .or_else(|| json_object_text(context, &["author", "bookAuthor"])),
        ),
        intro: non_empty(
            extract_json_rule_optional(context, rules.intro.as_ref())?
                .or_else(|| json_object_text(context, &["intro", "description", "desc"])),
        ),
        cover_url: non_empty(
            extract_json_rule_optional(context, rules.url.as_ref())?
                .or_else(|| json_object_text(context, &["coverUrl", "cover", "image", "img"])),
        ),
        book_url: book_url.to_string(),
    })
}

fn parse_chapter_list_json(
    rules: &PageRules,
    body: &str,
    base_url: &str,
) -> Result<Vec<SourceChapter>, SourceError> {
    let value: Value =
        serde_json::from_str(body).map_err(|error| SourceError::InvalidJson(error.to_string()))?;
    let items = extract_json_nodes(&value, rules.item.as_deref().unwrap_or("$"))?;
    let mut chapters = Vec::new();

    for (index, item) in items.into_iter().enumerate() {
        let title = extract_json_rule_optional(item, rules.title.as_ref())?
            .or_else(|| json_object_text(item, &["title", "name", "chapterName"]));
        let url = extract_json_rule_optional(item, rules.url.as_ref())?
            .or_else(|| json_object_text(item, &["url", "href", "link", "chapterUrl"]));
        let (Some(title), Some(url)) = (title, url.filter(|value| is_navigable_reference(value)))
        else {
            continue;
        };
        let url = absolutize_url(base_url, &url);
        chapters.push(SourceChapter { title, url, index });
    }

    Ok(chapters)
}
fn non_empty(value: Option<String>) -> Option<String> {
    value.filter(|value| !value.trim().is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn classifies_rule_evaluation_boundaries() {
        assert_eq!(
            rule_evaluation_for_output("search", "item", true).status,
            SourceRuleEvaluationStatus::Success
        );
        assert_eq!(
            rule_evaluation_for_output("search", "item", false).status,
            SourceRuleEvaluationStatus::NoMatch
        );
        assert_eq!(
            rule_evaluation_for_rule("search", "author", false, false).status,
            SourceRuleEvaluationStatus::Skipped
        );
        assert_eq!(
            rule_evaluation_from_error("content", "content", "no value matched the source rule")
                .expect("no-match error should be classified")
                .status,
            SourceRuleEvaluationStatus::NoMatch
        );
        assert_eq!(
            rule_evaluation_from_error("book_info", "title", "invalid CSS selector: h2[")
                .expect("selector error should be classified")
                .status,
            SourceRuleEvaluationStatus::Failure
        );
        assert!(
            rule_evaluation_from_error("content", "content", "request failed: timeout").is_none()
        );
    }

    #[test]
    fn validates_the_public_fixture() {
        let result = validate_source_json(include_str!("../fixtures/sample_source.json"));
        assert!(result.valid, "{:?}", result.errors);
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn preserves_legado_source_metadata() {
        let result = validate_source_json(
            r#"{
              "name": "Metadata fixture",
              "bookSourceUrl": "https://metadata.example.test/",
              "bookSourceGroup": "公开测试",
              "bookSourceType": 0,
              "bookUrlPattern": "https://metadata.example.test/book/{{bookId}}",
              "exploreUrl": "https://metadata.example.test/explore",
              "enabledExplore": true,
              "customOrder": 12,
              "weight": 80,
              "bookSourceComment": "仅用于授权夹具",
              "searchUrl": "https://metadata.example.test/search?q={{keyword}}"
            }"#,
        );
        assert!(result.valid, "{:?}", result.errors);
        let source = result.source.expect("source should parse");
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
        assert_eq!(source.custom_order, 12);
        assert_eq!(source.weight, 80);
        assert_eq!(source.comment.as_deref(), Some("仅用于授权夹具"));
    }

    #[test]
    fn uses_direct_page_fallback_for_missing_runtime_endpoints() {
        let source: BookSource = serde_json::from_str(
            r#"{
              "name": "Fallback endpoint",
              "searchUrl": "https://example.test/search?q={{keyword}}"
            }"#,
        )
        .expect("fallback endpoint source");

        assert_eq!(
            runtime_endpoint_or_fallback(
                &source,
                "bookInfoUrl",
                source.book_info_url.as_deref(),
                "https://example.test/book/1",
            ),
            "https://example.test/book/1"
        );
        assert_eq!(
            runtime_endpoint_or_fallback(
                &source,
                "tocUrl",
                source.toc_url.as_deref(),
                "https://example.test/book/1",
            ),
            "https://example.test/book/1"
        );
        assert_eq!(
            runtime_endpoint_or_fallback(
                &source,
                "contentUrl",
                source.content_url.as_deref(),
                "https://example.test/chapter/1",
            ),
            "https://example.test/chapter/1"
        );

        let mut legacy_source = source.clone();
        legacy_source.legacy_urls.insert(
            "bookInfoUrl".to_string(),
            "@js:return 'dynamic'".to_string(),
        );
        assert_eq!(
            runtime_endpoint_or_fallback(
                &legacy_source,
                "bookInfoUrl",
                Some("https://example.test/ignored"),
                "https://example.test/book/1",
            ),
            "https://example.test/book/1"
        );
    }

    #[test]
    fn skips_legacy_endpoint_at_runtime() {
        let source: BookSource = serde_json::from_str(
            r#"{
              "name": "Legacy endpoint",
              "searchUrl": "https://legacy.invalid/",
              "legacy_urls": { "searchUrl": "@js:return 'dynamic'" }
            }"#,
        )
        .expect("legacy endpoint source");
        assert!(ensure_runtime_endpoint(&source, "searchUrl").is_err());
    }

    #[test]
    fn rejects_unsupported_audio_source_type() {
        let result = validate_source_json(
            r#"{
              "name": "Audio fixture",
              "bookSourceType": 1,
              "searchUrl": "https://example.test/search?q={{keyword}}"
            }"#,
        );
        assert!(result.valid);
        assert!(result.errors.is_empty());
        assert!(result
            .warnings
            .iter()
            .any(|warning| warning.contains("音频/TTS")));
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
    fn extracts_html_fallback_chain() {
        let source: BookSource = serde_json::from_str(
            r#"{
              "name": "Chain Fixture",
              "searchUrl": "https://example.test/search",
              "search": {
                "item": "article.book",
                "title": {
                  "chain": [
                    { "selector": "h2 a" },
                    { "selector": ".title" }
                  ]
                }
              }
            }"#,
        )
        .expect("source should parse");
        let engine = SourceEngine::new(1, 1024).expect("engine should build");
        let results = engine
            .parse_search_html(
                &source,
                r#"<article class="book"><span class="title">备用标题</span></article>"#,
            )
            .expect("html should parse");

        assert_eq!(results[0].title, "备用标题");
    }

    #[test]
    fn deduplicates_search_results_by_title_and_author() {
        let results = dedupe_search_results(vec![
            UnifiedSearchResult {
                source_id: "source-b".to_string(),
                source_name: "书源 B".to_string(),
                title: " 测试 书 ".to_string(),
                author: Some(" 作者甲 ".to_string()),
                intro: None,
                book_url: Some("https://b.test/book".to_string()),
                can_open: true,
                can_read: true,
                unavailable_reason: None,
            },
            UnifiedSearchResult {
                source_id: "source-a".to_string(),
                source_name: "书源 A".to_string(),
                title: "测试书".to_string(),
                author: Some("作者甲".to_string()),
                intro: None,
                book_url: Some("https://a.test/book".to_string()),
                can_open: true,
                can_read: true,
                unavailable_reason: None,
            },
            UnifiedSearchResult {
                source_id: "source-a".to_string(),
                source_name: "书源 A".to_string(),
                title: "另一本".to_string(),
                author: None,
                intro: None,
                book_url: None,
                can_open: false,
                can_read: false,
                unavailable_reason: Some("搜索结果没有可用的书籍链接".to_string()),
            },
        ]);

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].title, "另一本");
        assert_eq!(results[1].source_name, "书源 A");
    }

    #[test]
    fn exposes_search_result_capabilities_without_blocking_search_only_sources() {
        let source: BookSource = serde_json::from_str(
            r#"{
              "name": "Search only",
              "searchUrl": "https://example.test/search?q={{keyword}}",
              "search": { "item": "article", "title": { "selector": "h2" } }
            }"#,
        )
        .expect("source should parse");
        let (can_open, can_read, reason) =
            source_capabilities(&source, Some("https://example.test/book/1"));
        assert!(!can_open);
        assert!(!can_read);
        assert_eq!(
            reason.as_deref(),
            Some("该书源仅支持搜索，未配置书籍详情或目录规则")
        );

        let missing_url = source_capabilities(&source, None);
        assert!(!missing_url.0);
        assert_eq!(missing_url.2.as_deref(), Some("搜索结果没有可用的书籍链接"));
    }

    #[test]
    fn applies_enabled_replace_rules_in_order() {
        let rules = vec![
            ReplaceRule {
                pattern: r"\s+".to_string(),
                replacement: " ".to_string(),
                enabled: true,
            },
            ReplaceRule {
                pattern: "内部标记".to_string(),
                replacement: String::new(),
                enabled: true,
            },
            ReplaceRule {
                pattern: "正文".to_string(),
                replacement: "不应改变".to_string(),
                enabled: false,
            },
        ];

        let content =
            apply_replace_rules("  正文   内部标记  ", &rules).expect("replace should work");
        assert_eq!(content, " 正文  ");
    }

    #[test]
    fn rejects_invalid_replace_rules() {
        let result = validate_source_json(
            r#"{
              "name": "Broken replace",
              "searchUrl": "https://example.test/search?q={{keyword}}",
              "replaceRules": [{ "pattern": "[", "replacement": "" }]
            }"#,
        );
        assert!(!result.valid);
        assert!(result
            .errors
            .iter()
            .any(|error| error.contains("replaceRules[0]")));
    }

    #[test]
    fn audits_permission_and_hosts() {
        let audit = audit_source_json(
            r#"{ 
              "name": "Owned fixture",
              "searchUrl": "https://example.test/search?q={{keyword}}",
              "permission": {
                "status": "authorized",
                "scope": "internal fixture",
                "reviewedAt": "2026-08-01"
              }
            }"#,
        );
        assert!(audit.pass, "{:?}", audit.errors);
        assert_eq!(audit.permission_status, "authorized");
        assert_eq!(audit.hosts, vec!["example.test"]);
        assert!(audit
            .warnings
            .iter()
            .all(|warning| !warning.contains("permission")));
    }

    #[test]
    fn audits_sensitive_headers() {
        let audit = audit_source_json(
            r#"{ 
              "name": "Unsafe fixture",
              "searchUrl": "https://example.test/search",
              "headers": { "Cookie": "session=secret" }
            }"#,
        );
        assert!(!audit.pass);
        assert_eq!(audit.sensitive_headers, vec!["Cookie"]);
        assert!(audit
            .errors
            .iter()
            .any(|error| error.contains("敏感认证头")));
    }

    #[test]
    fn summarizes_chapter_updates() {
        let previous = vec![
            SourceChapter {
                title: "第一章".to_string(),
                url: "https://example.test/chapter/1".to_string(),
                index: 0,
            },
            SourceChapter {
                title: "第二章".to_string(),
                url: "https://example.test/chapter/2".to_string(),
                index: 1,
            },
        ];
        let current = vec![
            previous[0].clone(),
            SourceChapter {
                title: "第三章".to_string(),
                url: "https://example.test/chapter/3".to_string(),
                index: 1,
            },
        ];

        let summary = summarize_chapter_update(&previous, &current);
        assert!(summary.changed);
        assert_eq!(summary.added, 1);
        assert_eq!(summary.removed, 1);
        assert_eq!(summary.retained, 1);
        assert_eq!(summary.fingerprint, chapter_fingerprint(&current));
        assert_ne!(chapter_fingerprint(&previous), summary.fingerprint);
    }

    #[test]
    fn bounds_url_chain_time_budget() {
        assert_eq!(bounded_stage_timeout(1), Duration::from_secs(8));
        assert_eq!(
            bounded_stage_timeout(DEFAULT_TIMEOUT_SECS),
            Duration::from_secs(MAX_STAGE_BUDGET_SECS)
        );
    }

    #[test]
    fn bounds_pipeline_time_budget() {
        assert_eq!(bounded_pipeline_timeout(1), Duration::from_secs(4));
        assert_eq!(
            bounded_pipeline_timeout(DEFAULT_TIMEOUT_SECS),
            Duration::from_secs(DEFAULT_TIMEOUT_SECS * 4)
        );
        assert_eq!(
            bounded_pipeline_timeout(999),
            Duration::from_secs(MAX_PIPELINE_BUDGET_SECS)
        );
    }

    #[test]
    fn renders_bounded_request_template_variables() {
        let context = SourceRequestContext {
            keyword: Some("测试 书".to_string()),
            page: 2,
            book_url: Some("https://example.test/book/42".to_string()),
            book_id: Some("42".to_string()),
            chapter_id: Some("7".to_string()),
        };
        let rendered = render_url_context(
            "https://example.test/search?q={{keyword}}&page={{page}}&next={{page+1}}&prev={{page-1}}&index={{pageIndex}}&book={{bookId}}&chapter={{chapterId}}",
            &context,
        );
        assert!(rendered.contains("q=%E6%B5%8B%E8%AF%95+%E4%B9%A6"));
        assert!(rendered.contains("page=2"));
        assert!(rendered.contains("next=3"));
        assert!(rendered.contains("prev=1"));
        assert!(rendered.contains("index=1"));
        assert!(rendered.contains("book=42"));
        assert!(rendered.contains("chapter=7"));
        assert_eq!(bounded_search_pages(0), 1);
        assert_eq!(bounded_search_pages(3), 3);
        assert_eq!(bounded_search_pages(999), MAX_SOURCE_SEARCH_PAGES);
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
    async fn retries_url_chain_after_failed_candidate() {
        let (base_url, server) = spawn_search_fixture_server();
        let engine = SourceEngine::new(1, 1024 * 1024).expect("engine should build");
        let context = SourceRequestContext::search("demo", 1);
        let template =
            format!("http://127.0.0.1:1/unreachable||{base_url}/search?q={{{{keyword}}}}");
        let mut debug_steps = Vec::new();

        let (body, successful_url) = engine
            .fetch_stage_chain(
                "search",
                &template,
                &HashMap::new(),
                &context,
                &mut debug_steps,
            )
            .await
            .expect("second URL candidate should succeed");

        assert!(body.contains("测试书"));
        assert_eq!(successful_url, format!("{base_url}/search?q=demo"));
        assert_eq!(debug_steps.len(), 2);
        assert!(debug_steps[0].error.is_some());
        assert!(debug_steps[1].error.is_none());
        server.join().expect("fixture server should stop");
    }

    #[test]
    fn keeps_next_page_policy_disabled_by_default() {
        let policy = NextPagePolicy::default();
        let decision = evaluate_next_page_policy(
            &policy,
            "https://example.test/chapter/1",
            "https://example.test/chapter/2",
            0,
            1,
            0,
            0,
            &[],
        );
        assert!(!decision.allowed);
        assert_eq!(decision.reason, "disabled");
    }

    #[test]
    fn allows_same_origin_next_page_with_remaining_budget() {
        let policy = NextPagePolicy {
            enabled: true,
            ..NextPagePolicy::default()
        };
        let decision = evaluate_next_page_policy(
            &policy,
            "https://example.test/chapter/1",
            "https://example.test/chapter/2",
            0,
            1,
            100,
            1,
            &["https://example.test/chapter/1".to_string()],
        );
        assert!(decision.allowed);
        assert_eq!(decision.reason, "allowed");
        assert_eq!(decision.next_depth, 1);
        assert_eq!(decision.pages_remaining, 1);
        assert_eq!(decision.bytes_remaining, policy.max_bytes - 100);
    }

    #[test]
    fn rejects_next_page_policy_limits_cycles_and_cross_origin() {
        let policy = NextPagePolicy {
            enabled: true,
            ..NextPagePolicy::default()
        };
        let cross_origin = evaluate_next_page_policy(
            &policy,
            "https://example.test/chapter/1",
            "https://other.test/chapter/2",
            0,
            1,
            0,
            0,
            &[],
        );
        assert!(!cross_origin.allowed);
        assert_eq!(cross_origin.reason, "same_origin");

        let depth_limit = evaluate_next_page_policy(
            &policy,
            "https://example.test/chapter/1",
            "https://example.test/chapter/2",
            policy.max_depth,
            1,
            0,
            0,
            &[],
        );
        assert!(!depth_limit.allowed);
        assert_eq!(depth_limit.reason, "depth_limit");

        let cycle = evaluate_next_page_policy(
            &policy,
            "https://example.test/chapter/1",
            "https://example.test/chapter/2",
            0,
            1,
            0,
            0,
            &["https://example.test/chapter/2".to_string()],
        );
        assert!(!cycle.allowed);
        assert_eq!(cycle.reason, "cycle");
    }

    #[test]
    fn covers_next_page_policy_stop_reason_matrix() {
        let policy = NextPagePolicy {
            enabled: true,
            ..NextPagePolicy::default()
        };
        let cases = [
            (
                "page_limit",
                "https://example.test/chapter/2",
                "https://example.test/chapter/1",
                0,
                policy.max_pages,
                0,
                0,
            ),
            (
                "byte_limit",
                "https://example.test/chapter/2",
                "https://example.test/chapter/1",
                0,
                1,
                policy.max_bytes,
                0,
            ),
            (
                "time_limit",
                "https://example.test/chapter/2",
                "https://example.test/chapter/1",
                0,
                1,
                0,
                policy.max_duration_secs,
            ),
            (
                "invalid_next_url",
                "javascript:alert(1)",
                "https://example.test/chapter/1",
                0,
                1,
                0,
                0,
            ),
            (
                "invalid_base_url",
                "https://example.test/chapter/2",
                "not-a-url",
                0,
                1,
                0,
                0,
            ),
        ];

        for (reason, candidate, base, depth, pages, bytes, elapsed) in cases {
            let decision = evaluate_next_page_policy(
                &policy,
                base,
                candidate,
                depth,
                pages,
                bytes,
                elapsed,
                &[],
            );
            assert_eq!(decision.reason, reason, "{reason}: {decision:?}");
            assert!(!decision.allowed);
        }

        let zero_quota = NextPagePolicy {
            enabled: true,
            max_depth: 0,
            ..NextPagePolicy::default()
        };
        let decision = evaluate_next_page_policy(
            &zero_quota,
            "https://example.test/chapter/1",
            "https://example.test/chapter/2",
            0,
            1,
            0,
            0,
            &[],
        );
        assert_eq!(decision.reason, "quota_zero");

        let unbounded = NextPagePolicy {
            enabled: true,
            max_depth: usize::MAX,
            max_pages: usize::MAX,
            max_bytes: usize::MAX,
            max_duration_secs: u64::MAX,
            ..NextPagePolicy::default()
        };
        let decision = evaluate_next_page_policy(
            &unbounded,
            "https://example.test/chapter/1",
            "https://example.test/chapter/2",
            0,
            1,
            0,
            0,
            &[],
        );
        assert!(decision.allowed);
        assert_eq!(decision.next_depth, 1);
        assert_eq!(decision.pages_remaining, default_next_page_count() - 2);
        assert_eq!(decision.bytes_remaining, default_next_page_bytes());
    }

    #[test]
    fn accumulates_next_page_budgets_without_resetting() {
        let policy = NextPagePolicy {
            enabled: true,
            ..NextPagePolicy::default()
        };
        let base = "https://example.test/chapter/1";
        let mut depth = 0;
        let mut pages_used = 1;
        let mut bytes_used = 0;
        let mut elapsed_secs = 0;
        let mut visited = vec![base.to_string()];

        for (url, page_bytes, page_seconds) in [
            ("https://example.test/chapter/2", 100, 5),
            ("https://example.test/chapter/3", 100, 5),
        ] {
            let decision = evaluate_next_page_policy(
                &policy,
                base,
                url,
                depth,
                pages_used,
                bytes_used,
                elapsed_secs,
                &visited,
            );
            assert!(decision.allowed, "{url}: {decision:?}");
            depth = decision.next_depth;
            pages_used += 1;
            bytes_used += page_bytes;
            elapsed_secs += page_seconds;
            visited.push(url.to_string());
        }

        let page_limit = evaluate_next_page_policy(
            &policy,
            base,
            "https://example.test/chapter/4",
            depth,
            pages_used,
            bytes_used,
            elapsed_secs,
            &visited,
        );
        assert!(!page_limit.allowed);
        assert_eq!(page_limit.reason, "depth_limit");

        let time_limited = NextPagePolicy {
            enabled: true,
            max_duration_secs: 5,
            ..NextPagePolicy::default()
        };
        let time_limit = evaluate_next_page_policy(
            &time_limited,
            base,
            "https://example.test/chapter/2",
            0,
            1,
            0,
            5,
            &[base.to_string()],
        );
        assert!(!time_limit.allowed);
        assert_eq!(time_limit.reason, "time_limit");
    }

    #[test]
    fn validates_bounded_nested_content_url() {
        assert_eq!(
            bounded_next_url("https://example.test/chapter/2")
                .expect("next URL should be accepted"),
            Some("https://example.test/chapter/2".to_string())
        );
        assert!(bounded_next_url("javascript:alert(1)").is_err());
        let oversized = format!("https://example.test/{}", "a".repeat(MAX_NEXT_URL_BYTES));
        assert!(bounded_next_url(&oversized).is_err());
    }

    #[test]
    fn validates_bounded_url_fallback_chain() {
        let context = SourceRequestContext::search("demo", 2);
        let urls = render_url_chain(
            "https://one.example.test/search?q={{keyword}}||https://two.example.test/search?page={{pageNum}}",
            &context,
        )
        .expect("URL chain should render");
        assert_eq!(urls[0], "https://one.example.test/search?q=demo");
        assert_eq!(urls[1], "https://two.example.test/search?page=2");

        let too_many = (0..9)
            .map(|index| format!("https://{index}.example.test"))
            .collect::<Vec<_>>()
            .join("||");
        assert!(split_url_chain(&too_many).is_err());
    }

    #[test]
    fn extracts_bounded_jsonpath_filter_and_bracket_alias() {
        let engine = SourceEngine::new(1, 1024).expect("engine should build");
        let values = engine
            .extract_json_values(
                r#"{ "books": [
                    { "kind": "novel", "title": "第一本" },
                    { "kind": "comic", "title": "第二本" }
                ] }"#,
                "$.books[?(@.kind == 'novel')]['title']",
            )
            .expect("safe JSONPath should parse");
        assert_eq!(values, vec!["第一本"]);
    }

    #[test]
    fn rejects_unsafe_jsonpath_filter_expression() {
        let engine = SourceEngine::new(1, 1024).expect("engine should build");
        let error = engine
            .extract_json_values(
                r#"{ "books": [{ "kind": "novel", "title": "第一本" }] }"#,
                "$.books[?(@.kind != 'novel')]",
            )
            .expect_err("unsupported filter should be rejected");
        assert!(matches!(error, SourceError::InvalidJsonPath(_)));
    }

    #[test]
    fn parses_json_search_results_with_jsonpath_rules() {
        let source: BookSource = serde_json::from_str(
            r#"{
              "name": "JSON Fixture",
              "searchUrl": "https://example.test/search",
              "search": {
                "item": "$.books[*]",
                "title": "$.title",
                "author": { "jsonPath": "$.author" },
                "url": { "path": "$.url" }
              }
            }"#,
        )
        .expect("JSON source should parse");
        let engine = SourceEngine::new(1, 1024).expect("engine should build");
        let results = engine
            .parse_search_json(
                &source,
                r#"{ "books": [
                    { "title": "第一本", "author": "作者甲", "url": "/book/1" },
                    { "title": "第二本", "author": "作者乙", "url": "/book/2" }
                ] }"#,
            )
            .expect("JSON should parse");

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].title, "第一本");
        assert_eq!(results[0].author.as_deref(), Some("作者甲"));
        assert_eq!(results[0].book_url.as_deref(), Some("/book/1"));
    }

    #[test]
    fn rejects_invalid_jsonpath_rules() {
        let result = validate_source_json(
            r#"{
              "name": "Broken JSON",
              "searchUrl": "https://example.test/search",
              "search": {
                "item": "$.books[",
                "title": "$.title"
              }
            }"#,
        );
        assert!(!result.valid);
        assert!(result
            .errors
            .iter()
            .any(|error| error.contains("JSON path")));
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
        assert_eq!(result.diagnostics.len(), 2);
        assert_eq!(
            result
                .diagnostics
                .iter()
                .find(|item| item.source_id == "fixture")
                .map(|item| item.stop_reason.as_str()),
            Some("max_pages")
        );
        assert_eq!(
            result
                .diagnostics
                .iter()
                .find(|item| item.source_id == "broken")
                .map(|item| item.stop_reason.as_str()),
            Some("request_failed")
        );
        server.join().expect("fixture server should stop");
    }

    #[tokio::test]
    async fn follows_authorized_next_pages_with_opt_in_policy() {
        let (base_url, server) = spawn_next_page_fixture_server();
        let mut source: BookSource = serde_json::from_str(
            r#"{
              "name": "Next page fixture",
              "searchUrl": "https://example.test/search",
              "content": {
                "content": { "selector": "article.content" },
                "next": { "selector": "a.next", "attr": "href" }
              }
            }"#,
        )
        .expect("next page source should parse");
        source.content_url = Some(format!("{base_url}/chapter/{{{{chapterId}}}}"));

        let chapter = SourceChapter {
            title: "第一章".to_string(),
            url: format!("{base_url}/chapter/1"),
            index: 0,
        };
        let policy = NextPagePolicy {
            enabled: true,
            ..NextPagePolicy::default()
        };
        let engine = SourceEngine::new(3, 1024 * 1024).expect("engine should build");
        let mut debug_steps = Vec::new();
        let result = engine
            .fetch_chapter_content_with_policy(&source, &chapter, &policy, &mut debug_steps)
            .await
            .expect("next page fixture should succeed");

        assert!(result.content.contains("第一页"));
        assert!(result.content.contains("第二页"));
        assert!(result.content.contains("第三页"));
        assert!(result.next_url.is_none());
        assert!(debug_steps
            .iter()
            .any(|step| step.stage == "content.next.depth-1"));
        assert!(debug_steps
            .iter()
            .any(|step| step.stage == "content.next.depth-2"));
        server.join().expect("next page fixture server should stop");
    }

    #[tokio::test]
    async fn preserves_partial_content_after_next_page_request_failure() {
        let (base_url, server) = spawn_next_page_edge_fixture_server("partial");
        let source = next_page_fixture_source(&base_url);
        let chapter = next_page_fixture_chapter(&base_url);
        let policy = NextPagePolicy {
            enabled: true,
            ..NextPagePolicy::default()
        };
        let engine = SourceEngine::new(3, 1024 * 1024).expect("engine should build");
        let mut debug_steps = Vec::new();

        let result = engine
            .fetch_chapter_content_with_policy(&source, &chapter, &policy, &mut debug_steps)
            .await
            .expect("a later request failure should preserve partial content");

        assert!(result.content.contains("第一页"));
        assert_eq!(
            result.next_url.as_deref(),
            Some(format!("{base_url}/chapter/2").as_str())
        );
        assert!(debug_steps.iter().any(|step| {
            step.stage == "content.next.depth-1"
                && step
                    .error
                    .as_deref()
                    .is_some_and(|error| error.contains("503"))
        }));
        assert!(debug_steps.iter().any(|step| {
            step.stage == "content.next.policy"
                && step
                    .error
                    .as_deref()
                    .is_some_and(|error| error.contains("request_error"))
        }));
        server.join().expect("partial fixture server should stop");
    }

    #[tokio::test]
    async fn stops_next_page_chain_on_cycle_without_requesting_again() {
        let (base_url, server) = spawn_next_page_edge_fixture_server("cycle");
        let source = next_page_fixture_source(&base_url);
        let chapter = next_page_fixture_chapter(&base_url);
        let policy = NextPagePolicy {
            enabled: true,
            ..NextPagePolicy::default()
        };
        let engine = SourceEngine::new(3, 1024 * 1024).expect("engine should build");
        let mut debug_steps = Vec::new();

        let result = engine
            .fetch_chapter_content_with_policy(&source, &chapter, &policy, &mut debug_steps)
            .await
            .expect("cycle should return accumulated content");

        assert!(result.content.contains("第一页"));
        assert!(result.content.contains("第二页"));
        assert_eq!(
            result.next_url.as_deref(),
            Some(format!("{base_url}/chapter/1").as_str())
        );
        assert!(debug_steps.iter().any(|step| {
            step.stage == "content.next.policy"
                && step
                    .error
                    .as_deref()
                    .is_some_and(|error| error.contains("cycle"))
        }));
        server.join().expect("cycle fixture server should stop");
    }

    #[tokio::test]
    async fn refuses_cross_origin_next_page_before_request() {
        let (base_url, server) = spawn_next_page_edge_fixture_server("cross-origin");
        let source = next_page_fixture_source(&base_url);
        let chapter = next_page_fixture_chapter(&base_url);
        let policy = NextPagePolicy {
            enabled: true,
            ..NextPagePolicy::default()
        };
        let engine = SourceEngine::new(3, 1024 * 1024).expect("engine should build");
        let mut debug_steps = Vec::new();

        let result = engine
            .fetch_chapter_content_with_policy(&source, &chapter, &policy, &mut debug_steps)
            .await
            .expect("cross-origin candidate should keep first page");

        assert!(result.content.contains("第一页"));
        assert_eq!(
            result.next_url.as_deref(),
            Some("https://example.invalid/chapter/2")
        );
        assert!(debug_steps.iter().any(|step| {
            step.stage == "content.next.policy"
                && step
                    .error
                    .as_deref()
                    .is_some_and(|error| error.contains("same_origin"))
        }));
        server
            .join()
            .expect("cross-origin fixture server should stop");
    }

    #[tokio::test]
    async fn preserves_partial_content_when_cumulative_bytes_are_exhausted() {
        let (base_url, server) = spawn_next_page_edge_fixture_server("byte");
        let source = next_page_fixture_source(&base_url);
        let chapter = next_page_fixture_chapter(&base_url);
        let policy = NextPagePolicy {
            enabled: true,
            max_bytes: 180,
            ..NextPagePolicy::default()
        };
        let engine = SourceEngine::new(3, 1024 * 1024).expect("engine should build");
        let mut debug_steps = Vec::new();

        let result = engine
            .fetch_chapter_content_with_policy(&source, &chapter, &policy, &mut debug_steps)
            .await
            .expect("byte limit should preserve the first page");

        assert!(result.content.contains("第一页"));
        assert_eq!(
            result.next_url.as_deref(),
            Some(format!("{base_url}/chapter/2").as_str())
        );
        assert!(debug_steps.iter().any(|step| {
            step.stage == "content.next.policy"
                && step
                    .error
                    .as_deref()
                    .is_some_and(|error| error.contains("byte_limit"))
        }));
        server.join().expect("byte fixture server should stop");
    }

    #[tokio::test]
    async fn preserves_partial_content_when_next_page_times_out() {
        let (base_url, server) = spawn_next_page_edge_fixture_server("timeout");
        let source = next_page_fixture_source(&base_url);
        let chapter = next_page_fixture_chapter(&base_url);
        let policy = NextPagePolicy {
            enabled: true,
            ..NextPagePolicy::default()
        };
        let engine = SourceEngine::new(1, 1024 * 1024).expect("engine should build");
        let mut debug_steps = Vec::new();

        let result = engine
            .fetch_chapter_content_with_policy(&source, &chapter, &policy, &mut debug_steps)
            .await
            .expect("a timed-out later request should preserve partial content");

        assert!(result.content.contains("第一页"));
        assert_eq!(
            result.next_url.as_deref(),
            Some(format!("{base_url}/chapter/2").as_str())
        );
        // reqwest versions may redact the transport timeout wording; the stable
        // contract is that the later-page stage records an error and partial content remains.
        assert!(debug_steps
            .iter()
            .any(|step| { step.stage == "content.next.depth-1" && step.error.is_some() }));
        server.join().expect("timeout fixture server should stop");
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
        assert_eq!(
            result.debug_steps[0]
                .variables
                .get("page")
                .map(String::as_str),
            Some("1")
        );
        assert_eq!(
            result.debug_steps[0]
                .variables
                .get("keyword")
                .map(String::as_str),
            Some("<redacted>")
        );
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

    fn spawn_next_page_fixture_server() -> (String, std::thread::JoinHandle<()>) {
        use std::{
            io::{Read, Write},
            net::TcpListener,
            thread,
        };

        let listener = TcpListener::bind("127.0.0.1:0").expect("next page fixture listener");
        let address = listener.local_addr().expect("next page fixture address");
        let server = thread::spawn(move || {
            for stream in listener.incoming().take(3) {
                let mut stream = stream.expect("next page fixture stream");
                let mut buffer = [0_u8; 2048];
                let size = stream.read(&mut buffer).expect("next page fixture request");
                let request = String::from_utf8_lossy(&buffer[..size]);
                let path = request.split_whitespace().nth(1).unwrap_or("/");
                let body = match path {
                    "/chapter/1" => {
                        r#"<article class="content">第一页</article><a class="next" href="/chapter/2">下一页</a>"#
                    }
                    "/chapter/2" => {
                        r#"<article class="content">第二页</article><a class="next" href="/chapter/3">下一页</a>"#
                    }
                    _ => r#"<article class="content">第三页</article>"#,
                };
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.as_bytes().len(),
                    body
                );
                stream
                    .write_all(response.as_bytes())
                    .expect("next page fixture response");
            }
        });

        (format!("http://{address}"), server)
    }

    fn next_page_fixture_source(base_url: &str) -> BookSource {
        let mut source: BookSource = serde_json::from_str(
            r#"{
              "name": "Next page edge fixture",
              "searchUrl": "https://example.test/search",
              "content": {
                "content": { "selector": "article.content" },
                "next": { "selector": "a.next", "attr": "href" }
              }
            }"#,
        )
        .expect("next page source should parse");
        source.content_url = Some(format!("{base_url}/chapter/{{{{chapterId}}}}"));
        source
    }

    fn next_page_fixture_chapter(base_url: &str) -> SourceChapter {
        SourceChapter {
            title: "第一章".to_string(),
            url: format!("{base_url}/chapter/1"),
            index: 0,
        }
    }

    fn spawn_next_page_edge_fixture_server(
        mode: &'static str,
    ) -> (String, std::thread::JoinHandle<()>) {
        use std::{
            io::{Read, Write},
            net::TcpListener,
            thread,
        };

        let listener = TcpListener::bind("127.0.0.1:0").expect("edge fixture listener");
        let address = listener.local_addr().expect("edge fixture address");
        let connections = if mode == "cross-origin" { 1 } else { 2 };
        let server = thread::spawn(move || {
            for stream in listener.incoming().take(connections) {
                let mut stream = stream.expect("edge fixture stream");
                let mut buffer = [0_u8; 4096];
                let size = stream.read(&mut buffer).expect("edge fixture request");
                let request = String::from_utf8_lossy(&buffer[..size]);
                let path = request.split_whitespace().nth(1).unwrap_or("/");
                let (status, body) = match (mode, path) {
                    ("partial", "/chapter/1") => (
                        "HTTP/1.1 200 OK",
                        r#"<article class="content">第一页</article><a class="next" href="/chapter/2">下一页</a>"#.to_string(),
                    ),
                    ("partial", "/chapter/2") => (
                        "HTTP/1.1 503 Service Unavailable",
                        "暂时不可用".to_string(),
                    ),
                    ("cycle", "/chapter/1") => (
                        "HTTP/1.1 200 OK",
                        r#"<article class="content">第一页</article><a class="next" href="/chapter/2">下一页</a>"#.to_string(),
                    ),
                    ("cycle", "/chapter/2") => (
                        "HTTP/1.1 200 OK",
                        r#"<article class="content">第二页</article><a class="next" href="/chapter/1">上一页</a>"#.to_string(),
                    ),
                    ("cross-origin", "/chapter/1") => (
                        "HTTP/1.1 200 OK",
                        r#"<article class="content">第一页</article><a class="next" href="https://example.invalid/chapter/2">跨源</a>"#.to_string(),
                    ),
                    ("byte", "/chapter/1") => (
                        "HTTP/1.1 200 OK",
                        r#"<article class="content">第一页</article><a class="next" href="/chapter/2">下一页</a>"#.to_string(),
                    ),
                    ("byte", "/chapter/2") => (
                        "HTTP/1.1 200 OK",
                        format!(r#"<article class="content">{}</article>"#, "长".repeat(256)),
                    ),
                    ("timeout", "/chapter/1") => (
                        "HTTP/1.1 200 OK",
                        r#"<article class="content">第一页</article><a class="next" href="/chapter/2">下一页</a>"#.to_string(),
                    ),
                    ("timeout", "/chapter/2") => {
                        std::thread::sleep(std::time::Duration::from_secs(2));
                        (
                            "HTTP/1.1 200 OK",
                            r#"<article class="content">迟到的第二页</article>"#.to_string(),
                        )
                    }
                    _ => ("HTTP/1.1 404 Not Found", "not found".to_string()),
                };
                let response = format!(
                    "{status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.as_bytes().len(),
                    body
                );
                stream
                    .write_all(response.as_bytes())
                    .expect("edge fixture response");
            }
        });

        (format!("http://{address}"), server)
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

    #[test]
    fn extracts_join_html_and_replacement_rules() {
        let document = Html::parse_document(
            r#"<div class="meta">
                <meta name="updated" content="更新时间">
                <meta name="summary" content="简介">
            </div>
            <article class="intro"><p>第一段</p><p>第二段</p></article>"#,
        );
        let joined = SourceRule::Join {
            join: vec![
                SourceRule::Detailed {
                    selector: r#"meta[name="updated"]"#.to_string(),
                    attr: Some("content".to_string()),
                    regex: None,
                    replacement: None,
                },
                SourceRule::Detailed {
                    selector: r#"meta[name="summary"]"#.to_string(),
                    attr: Some("content".to_string()),
                    regex: None,
                    replacement: None,
                },
            ],
        };
        assert_eq!(
            extract_document_rule(&document, Some(&joined)).expect("join should parse"),
            Some("更新时间 简介".to_string())
        );

        let html_rule = SourceRule::Detailed {
            selector: "article.intro".to_string(),
            attr: Some("html".to_string()),
            regex: Some("第二段".to_string()),
            replacement: Some("第二段（已替换）".to_string()),
        };
        assert_eq!(
            extract_document_rule(&document, Some(&html_rule)).expect("html should parse"),
            Some("<p>第一段</p><p>第二段（已替换）</p>".to_string())
        );
    }
    #[test]
    fn legacy_rules_are_inert_and_do_not_fail_runtime() {
        let document = Html::parse_document("<article><h2>safe</h2></article>");
        let legacy = SourceRule::Legacy {
            legacy: Value::String("@js:return 'unsafe'".to_string()),
            reason: Some("脚本规则".to_string()),
        };
        assert_eq!(
            extract_document_rule(&document, Some(&legacy)).expect("legacy is skipped"),
            None
        );
        let article = document
            .select(&Selector::parse("article").expect("selector"))
            .next()
            .expect("article");
        assert!(matches!(
            extract_from_element(article, &legacy),
            Err(SourceError::NoMatch)
        ));
    }

    #[test]
    fn rule_errors_keep_stage_and_rule_context() {
        let message = rule_error("toc", "item", SourceError::NoMatch).to_string();
        assert!(message.contains("toc 规则 item"));
        assert!(message.contains("no value matched the source rule"));
        assert_eq!(
            rule_evaluation_from_error("toc", "item", &message)
                .expect("rule errors should be classified")
                .status,
            SourceRuleEvaluationStatus::NoMatch
        );
    }

    #[test]
    fn optional_rule_mismatch_does_not_abort_html_extraction() {
        let document = Html::parse_document("<article><h2>safe</h2></article>");
        let missing = SourceRule::Selector(".missing".to_string());
        assert_eq!(
            extract_document_rule(&document, Some(&missing)).expect("missing optional rule"),
            None
        );
        let article = document
            .select(&Selector::parse("article").expect("selector"))
            .next()
            .expect("article");
        assert_eq!(
            extract_from_element_optional(article, &missing).expect("missing item rule"),
            None
        );
    }

    #[test]
    fn html_search_uses_safe_link_fallback_for_legacy_url_rules() {
        let source: BookSource = serde_json::from_value(json!({
            "name": "Fallback search",
            "searchUrl": "https://example.test/search",
            "search": {
                "item": "li.book",
                "title": {"legacy": "//h2", "reason": "XPath"},
                "url": {"legacy": "//a/@href", "reason": "XPath"}
            }
        }))
        .expect("fallback source");
        let engine = SourceEngine::default().expect("source engine");
        let results = engine
            .parse_search_html(
                &source,
                r#"<ul><li class="book"><a href="/book/1">Book one</a><span>Author</span></li></ul>"#,
            )
            .expect("fallback search");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Book one");
        assert_eq!(results[0].book_url.as_deref(), Some("/book/1"));
    }

    #[test]
    fn html_toc_uses_safe_link_fallback_for_legacy_rules() {
        let rules = PageRules {
            item: Some("li.chapter".to_string()),
            title: Some(SourceRule::Legacy {
                legacy: json!("//a/text()"),
                reason: Some("XPath".to_string()),
            }),
            url: Some(SourceRule::Legacy {
                legacy: json!("//a/@href"),
                reason: Some("XPath".to_string()),
            }),
            ..PageRules::default()
        };
        let chapters = parse_chapter_list(
            &rules,
            r#"<ul><li class="chapter"><a href="/chapter/1">Chapter one</a></li></ul>"#,
            "https://example.test/book/",
        )
        .expect("fallback toc");

        assert_eq!(chapters.len(), 1);
        assert_eq!(chapters[0].title, "Chapter one");
        assert_eq!(chapters[0].url, "https://example.test/chapter/1");
    }

    #[test]
    fn html_toc_falls_back_to_chapter_links_when_item_rule_misses() {
        let rules = PageRules {
            item: Some(".missing-chapter-item".to_string()),
            title: Some(SourceRule::Selector(".missing-title".to_string())),
            url: Some(SourceRule::Selector(".missing-url".to_string())),
            ..PageRules::default()
        };
        let chapters = parse_chapter_list(
            &rules,
            r#"<nav><a class="chapter-link" href="/chapter/1">第一章 初见</a></nav>"#,
            "https://example.test/book/",
        )
        .expect("fallback chapter links");
        assert_eq!(chapters.len(), 1);
        assert_eq!(chapters[0].title, "第一章 初见");
        assert_eq!(chapters[0].url, "https://example.test/chapter/1");
    }

    #[test]
    fn html_content_falls_back_to_safe_content_selectors() {
        let rules = PageRules {
            content: Some(SourceRule::Selector(".missing-content".to_string())),
            ..PageRules::default()
        };
        let (content, next_url) = parse_chapter_page(
            &rules,
            r#"<article class="content"><p>正文内容</p></article>"#,
            "https://example.test/chapter/1",
        )
        .expect("fallback content");
        assert_eq!(content, "<p>正文内容</p>");
        assert_eq!(next_url, None);
    }

    #[test]
    fn json_search_and_book_info_use_common_field_fallbacks() {
        let search_source: BookSource = serde_json::from_value(json!({
            "name": "JSON fallback",
            "searchUrl": "https://example.test/search",
            "search": {
                "item": "$.items[*]",
                "title": {"legacy": "@js:title", "reason": "script"},
                "url": {"legacy": "@js:url", "reason": "script"}
            }
        }))
        .expect("json search source");
        let engine = SourceEngine::default().expect("source engine");
        let results = engine
            .parse_search_json(
                &search_source,
                r#"{"items":[{"name":"Book two","href":"/book/2","author":"Author two"}]}"#,
            )
            .expect("json fallback search");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Book two");
        assert_eq!(results[0].book_url.as_deref(), Some("/book/2"));

        let book_rules = PageRules {
            item: Some("$.book".to_string()),
            title: Some(SourceRule::Legacy {
                legacy: json!("@js:title"),
                reason: Some("script".to_string()),
            }),
            author: Some(SourceRule::Legacy {
                legacy: json!("@js:author"),
                reason: Some("script".to_string()),
            }),
            ..PageRules::default()
        };
        let info = parse_book_info_json(
            &book_rules,
            r#"{"book":{"name":"Book two","author":"Author two"}}"#,
            "https://example.test/book/2",
        )
        .expect("json fallback book info");
        assert_eq!(info.title, "Book two");
        assert_eq!(info.author.as_deref(), Some("Author two"));
    }

    #[test]
    fn fallback_ignores_non_navigable_links() {
        let document = Html::parse_document(
            r#"<li><a href="javascript:alert(1)">Bad</a><a href="/safe">Safe</a></li>"#,
        );
        let item = document
            .select(&Selector::parse("li").expect("selector"))
            .next()
            .expect("list item");
        assert_eq!(
            fallback_link_from_element(item),
            Some(("/safe".to_string(), "Safe".to_string()))
        );
    }

    #[test]
    fn html_rules_can_match_the_item_element_itself() {
        let engine = SourceEngine::default().expect("source engine");
        let source: BookSource = serde_json::from_value(json!({
            "name": "Self matching",
            "searchUrl": "https://example.test/search",
            "search": {
                "item": "a.result",
                "title": "a.result",
                "url": {"selector": "a.result", "attr": "href"}
            }
        }))
        .expect("self-matching search source");
        let results = engine
            .parse_search_html(&source, r#"<a class="result" href="/book/1">Book one</a>"#)
            .expect("self-matching search");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Book one");
        assert_eq!(results[0].book_url.as_deref(), Some("/book/1"));

        let rules = PageRules {
            item: Some("a.chapter".to_string()),
            title: Some(SourceRule::Selector("a.chapter".to_string())),
            url: Some(SourceRule::Detailed {
                selector: "a.chapter".to_string(),
                attr: Some("href".to_string()),
                regex: None,
                replacement: None,
            }),
            ..PageRules::default()
        };
        let chapters = parse_chapter_list(
            &rules,
            r#"<a class="chapter" href="/chapter/1">第一章</a>"#,
            "https://example.test/book/",
        )
        .expect("self-matching toc");
        assert_eq!(chapters.len(), 1);
        assert_eq!(chapters[0].title, "第一章");
        assert_eq!(chapters[0].url, "https://example.test/chapter/1");
    }

    #[test]
    fn decodes_declared_non_utf8_source_responses() {
        let (gbk, _, _) = GB18030.encode("测试书名\n作者甲");
        let decoded = decode_response_body(gbk.as_ref(), Some("text/html; charset=gbk"));
        assert_eq!(decoded.body, "测试书名\n作者甲");
        assert_eq!(decoded.encoding, "gb18030");
        assert!(!decoded.had_decode_errors);

        let utf16 = "第一章"
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect::<Vec<_>>();
        let decoded = decode_response_body(&utf16, Some("text/html; charset=utf-16le"));
        assert_eq!(decoded.body, "第一章");
        assert_eq!(decoded.encoding, "utf-16le");
        assert!(!decoded.had_decode_errors);

        let invalid_utf8 = vec![0xe4, 0xb8, 0xad, 0xff];
        let decoded = decode_response_body(&invalid_utf8, None);
        assert_eq!(decoded.encoding, "gb18030-fallback");
        assert!(decoded.had_decode_errors);
    }

    #[test]
    fn downgrades_garbled_book_fields_and_records_safe_reason() {
        let mut info = BookInfo {
            title: "ä¸­æ–‡ä¹¦å".to_string(),
            author: Some("正常作者".to_string()),
            intro: Some("���".to_string()),
            cover_url: None,
            book_url: "https://example.test/book/1".to_string(),
        };
        let downgraded = downgrade_garbled_book_fields(&mut info);
        assert_eq!(info.title, "未命名书籍");
        assert_eq!(info.author.as_deref(), Some("正常作者"));
        assert_eq!(info.intro, None);
        assert_eq!(downgraded, vec!["title", "intro"]);

        let mut steps = Vec::new();
        append_text_quality_debug_step(
            &mut steps,
            "https://example.test/book/1?token=secret",
            &downgraded,
        );
        assert_eq!(steps[0].stage, "book_info.text_quality");
        assert_eq!(steps[0].url, "https://example.test/book/1");
        assert_eq!(
            steps[0].variables.get("reason").map(String::as_str),
            Some("garbled_text_downgraded")
        );
    }

    #[test]
    fn serializes_field_level_rule_diagnostics_without_response_body() {
        let payload = serde_json::to_value(SourceRuleEvaluation {
            stage: "toc".to_string(),
            rule_key: "url".to_string(),
            status: SourceRuleEvaluationStatus::NoMatch,
            detail: Some("no_match".to_string()),
        })
        .expect("rule evaluation should serialize");
        assert_eq!(payload["stage"], "toc");
        assert_eq!(payload["rule_key"], "url");
        assert_eq!(payload["status"], "no_match");
        assert_eq!(payload["detail"], "no_match");
        assert!(!payload.to_string().contains("response"));
    }
}
