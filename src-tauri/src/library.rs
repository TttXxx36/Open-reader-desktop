use base64::{engine::general_purpose::STANDARD, Engine as _};
use encoding_rs::{GB18030, UTF_16BE, UTF_16LE};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    io::{Cursor, Read, Seek},
    path::Path,
};
use thiserror::Error;
use zip::ZipArchive;

#[derive(Debug, Error)]
pub enum ImportError {
    #[error("unsupported book format: {0}")]
    UnsupportedFormat(String),
    #[error("unable to decode text file")]
    TextDecode,
    #[error("file I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid EPUB: {0}")]
    InvalidEpub(String),
    #[error("EPUB archive error: {0}")]
    Archive(#[from] zip::result::ZipError),
}

#[derive(Debug, Clone)]
pub struct ParsedBook {
    pub title: String,
    pub author: Option<String>,
    pub format: String,
    pub chapters: Vec<ParsedChapter>,
}

pub const CONTENT_FORMAT_TEXT: &str = "text";
pub const CONTENT_FORMAT_BLOCKS_V1: &str = "blocks-v1";
const MAX_EMBEDDED_IMAGE_BYTES: usize = 2 * 1024 * 1024;
const MAX_EMBEDDED_IMAGE_TOTAL_BYTES: usize = 8 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct ParsedChapter {
    pub title: String,
    pub content: String,
    pub content_format: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentDocument {
    pub version: u8,
    pub blocks: Vec<ContentBlock>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentBlock {
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub level: Option<u8>,
    #[serde(default)]
    pub spans: Vec<ContentSpan>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alt: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub src: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentSpan {
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub emphasis: Option<String>,
}

pub fn parse_book_bytes(file_name: &str, bytes: &[u8]) -> Result<ParsedBook, ImportError> {
    let extension = Path::new(file_name)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();

    match extension.as_str() {
        "txt" => parse_txt(bytes, file_name),
        "epub" => parse_epub(bytes, file_name),
        _ => Err(ImportError::UnsupportedFormat(extension)),
    }
}

fn parse_txt(bytes: &[u8], file_name: &str) -> Result<ParsedBook, ImportError> {
    let text = decode_text(bytes)?;
    let chapters = split_txt(&text);
    if chapters.is_empty() {
        return Err(ImportError::TextDecode);
    }

    Ok(ParsedBook {
        title: title_from_file_name(file_name),
        author: None,
        format: "txt".to_string(),
        chapters,
    })
}

fn decode_text(bytes: &[u8]) -> Result<String, ImportError> {
    if bytes.starts_with(&[0xFF, 0xFE]) {
        return Ok(UTF_16LE.decode(&bytes[2..]).0.into_owned());
    }
    if bytes.starts_with(&[0xFE, 0xFF]) {
        return Ok(UTF_16BE.decode(&bytes[2..]).0.into_owned());
    }

    let bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
    if let Ok(text) = String::from_utf8(bytes.to_vec()) {
        return Ok(text.trim_start_matches('\u{feff}').to_string());
    }

    let (text, _, had_errors) = GB18030.decode(bytes);
    if had_errors {
        return Err(ImportError::TextDecode);
    }
    Ok(text.into_owned().trim_start_matches('\u{feff}').to_string())
}

fn split_txt(text: &str) -> Vec<ParsedChapter> {
    let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
    let mut chapters = Vec::new();
    let mut current_title = String::new();
    let mut lines = Vec::new();

    for line in normalized.lines() {
        let trimmed = line.trim();
        if looks_like_chapter(trimmed) {
            if !current_title.is_empty() || !lines.is_empty() {
                push_text_chapter(&mut chapters, &current_title, &lines);
                lines.clear();
            }
            current_title = trimmed.to_string();
        } else {
            lines.push(line.trim_end().to_string());
        }
    }

    if !current_title.is_empty() || !lines.is_empty() {
        push_text_chapter(&mut chapters, &current_title, &lines);
    }

    chapters
}

fn push_text_chapter(chapters: &mut Vec<ParsedChapter>, title: &str, lines: &[String]) {
    let Some(first) = lines.iter().position(|line| !line.trim().is_empty()) else {
        return;
    };
    let Some(last) = lines.iter().rposition(|line| !line.trim().is_empty()) else {
        return;
    };
    let content = lines[first..=last].join("\n");

    let title = if title.is_empty() {
        format!("正文 {}", chapters.len() + 1)
    } else {
        title.to_string()
    };

    chapters.push(ParsedChapter {
        title,
        content,
        content_format: CONTENT_FORMAT_TEXT.to_string(),
    });
}

fn looks_like_chapter(line: &str) -> bool {
    if line.is_empty() || line.chars().count() > 80 {
        return false;
    }

    if ["序章", "楔子", "番外", "正文"]
        .iter()
        .any(|prefix| line.starts_with(prefix))
    {
        return true;
    }

    line.starts_with('第')
        && ["章", "节", "回", "卷", "篇"]
            .iter()
            .any(|marker| line.contains(marker))
}

fn title_from_file_name(file_name: &str) -> String {
    Path::new(file_name)
        .file_stem()
        .and_then(|value| value.to_str())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("未命名书籍")
        .to_string()
}

#[derive(Debug, Clone)]
struct ManifestItem {
    href: String,
    media_type: String,
}

fn parse_epub(bytes: &[u8], file_name: &str) -> Result<ParsedBook, ImportError> {
    let mut archive = ZipArchive::new(Cursor::new(bytes))?;
    let container = read_zip_text(&mut archive, "META-INF/container.xml")?;
    let opf_path = extract_attribute_from_xml(&container, "rootfile", "full-path")
        .ok_or_else(|| ImportError::InvalidEpub("缺少 OPF 根文件".to_string()))?;
    let opf = read_zip_text(&mut archive, &opf_path)?;
    let base_path = opf_path
        .rsplit_once('/')
        .map(|(base, _)| base)
        .unwrap_or_default();

    let title = extract_element_text(&opf, "dc:title")
        .or_else(|| extract_element_text(&opf, "title"))
        .unwrap_or_else(|| title_from_file_name(file_name));
    let author =
        extract_element_text(&opf, "dc:creator").or_else(|| extract_element_text(&opf, "creator"));

    let mut manifest = HashMap::new();
    for tag in find_tags(&opf, "item") {
        let Some(id) = extract_attribute(&tag, "id") else {
            continue;
        };
        let Some(href) = extract_attribute(&tag, "href") else {
            continue;
        };
        let media_type = extract_attribute(&tag, "media-type").unwrap_or_default();
        manifest.insert(id, ManifestItem { href, media_type });
    }

    let mut image_sources = HashMap::new();
    let mut embedded_image_bytes = 0usize;
    for item in manifest.values() {
        let media_type = item.media_type.to_ascii_lowercase();
        if !is_safe_epub_image_type(&media_type) {
            continue;
        }
        let path = join_zip_path(base_path, &item.href);
        let Ok(bytes) = read_zip_bytes(&mut archive, &path) else {
            continue;
        };
        if bytes.len() > MAX_EMBEDDED_IMAGE_BYTES
            || embedded_image_bytes.saturating_add(bytes.len()) > MAX_EMBEDDED_IMAGE_TOTAL_BYTES
        {
            continue;
        }
        embedded_image_bytes = embedded_image_bytes.saturating_add(bytes.len());
        image_sources.insert(
            path,
            format!("data:{media_type};base64,{}", STANDARD.encode(bytes)),
        );
    }

    let spine: Vec<String> = find_tags(&opf, "itemref")
        .into_iter()
        .filter_map(|tag| extract_attribute(&tag, "idref"))
        .collect();

    let mut chapters = Vec::new();
    for id in spine {
        let Some(item) = manifest.get(&id) else {
            continue;
        };
        if !item.media_type.contains("html") && !item.media_type.contains("xhtml") {
            continue;
        }

        let path = join_zip_path(base_path, &item.href);
        let html = read_zip_text(&mut archive, &path)?;
        let mut document = parse_html_document(&html);
        resolve_epub_images(&mut document, &path, &image_sources);
        if document.blocks.is_empty() {
            continue;
        }
        let content = serde_json::to_string(&document)
            .map_err(|error| ImportError::InvalidEpub(format!("内容块编码失败：{error}")))?;

        let title = ["h1", "h2", "h3"]
            .iter()
            .find_map(|tag| extract_element_text(&html, tag))
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| format!("第 {} 章", chapters.len() + 1));

        chapters.push(ParsedChapter {
            title,
            content,
            content_format: CONTENT_FORMAT_BLOCKS_V1.to_string(),
        });
    }

    if chapters.is_empty() {
        return Err(ImportError::InvalidEpub("未找到可阅读章节".to_string()));
    }

    Ok(ParsedBook {
        title,
        author,
        format: "epub".to_string(),
        chapters,
    })
}

fn read_zip_bytes<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
    path: &str,
) -> Result<Vec<u8>, ImportError> {
    let mut file = archive.by_name(path)?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    Ok(bytes)
}

fn read_zip_text<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
    path: &str,
) -> Result<String, ImportError> {
    let bytes = read_zip_bytes(archive, path)?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

fn join_zip_path(base: &str, href: &str) -> String {
    let href = href
        .split('#')
        .next()
        .unwrap_or_default()
        .replace("%20", " ");
    let combined = if base.is_empty() {
        href
    } else {
        format!("{base}/{href}")
    };
    let mut parts = Vec::new();

    for part in combined.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            value => parts.push(value),
        }
    }

    parts.join("/")
}

fn is_safe_epub_image_type(media_type: &str) -> bool {
    matches!(
        media_type,
        "image/png" | "image/jpeg" | "image/gif" | "image/webp" | "image/bmp"
    )
}

fn resolve_epub_images(
    document: &mut ContentDocument,
    chapter_path: &str,
    image_sources: &HashMap<String, String>,
) {
    let chapter_base = chapter_path
        .rsplit_once('/')
        .map(|(base, _)| base)
        .unwrap_or_default();

    for block in &mut document.blocks {
        if block.kind != "image" {
            continue;
        }
        let Some(source) = block.src.take() else {
            continue;
        };
        if source.contains("://") || source.starts_with("data:") || source.starts_with('#') {
            continue;
        }
        let image_path = join_zip_path(chapter_base, &source);
        block.src = image_sources.get(&image_path).cloned();
    }
}

fn find_tags(xml: &str, name: &str) -> Vec<String> {
    let needle = format!("<{name}");
    let bytes = xml.as_bytes();
    let mut cursor = 0;
    let mut tags = Vec::new();

    while let Some(relative_start) = xml[cursor..].find(&needle) {
        let start = cursor + relative_start;
        let boundary = bytes.get(start + needle.len()).copied();
        if !matches!(boundary, Some(b' ' | b'\t' | b'\r' | b'\n' | b'/' | b'>')) {
            cursor = start + needle.len();
            continue;
        }

        let Some(relative_end) = xml[start..].find('>') else {
            break;
        };
        let end = start + relative_end + 1;
        tags.push(xml[start..end].to_string());
        cursor = end;
    }

    tags
}

fn extract_attribute(tag: &str, name: &str) -> Option<String> {
    for quote in ['"', '\''] {
        let needle = format!("{name}={quote}");
        let Some(start) = tag.find(&needle) else {
            continue;
        };
        let start = start + needle.len();
        let end = tag[start..].find(quote)?;
        return Some(tag[start..start + end].to_string());
    }
    None
}

fn extract_attribute_from_xml(xml: &str, tag_name: &str, attribute: &str) -> Option<String> {
    find_tags(xml, tag_name)
        .into_iter()
        .find_map(|tag| extract_attribute(&tag, attribute))
}

fn extract_element_text(xml: &str, name: &str) -> Option<String> {
    let start_tag = format!("<{name}");
    let start = xml.find(&start_tag)?;
    let open_end = start + xml[start..].find('>')? + 1;
    let close_tag = format!("</{name}>");
    let close = xml[open_end..].find(&close_tag)? + open_end;
    let value = strip_html(&xml[open_end..close]);
    (!value.is_empty()).then_some(value)
}

fn parse_html_document(html: &str) -> ContentDocument {
    let chars: Vec<char> = html.chars().collect();
    let mut blocks = Vec::new();
    let mut spans = Vec::new();
    let mut current = String::new();
    let mut pending_space = false;
    let mut quote_depth = 0usize;
    let mut heading_level = None;
    let mut emphasis_stack: Vec<String> = Vec::new();
    let mut ignored_tag: Option<String> = None;
    let mut index = 0usize;

    while index < chars.len() {
        if chars[index] != '<' {
            if ignored_tag.is_none() {
                current.push(chars[index]);
            }
            index += 1;
            continue;
        }

        if ignored_tag.is_none() && is_html_comment_start(&chars, index) {
            index = skip_html_comment(&chars, index);
            continue;
        }

        let Some(tag_end) = find_html_tag_end(&chars, index) else {
            if ignored_tag.is_none() {
                current.push(chars[index]);
            }
            index += 1;
            continue;
        };

        let raw_tag: String = chars[index + 1..tag_end].iter().collect();
        let (closing, name, self_closing) = parse_html_tag(&raw_tag);

        if let Some(ignored) = ignored_tag.as_deref() {
            if closing && name == ignored {
                ignored_tag = None;
            }
            index = tag_end + 1;
            continue;
        }

        if name.is_empty() {
            index = tag_end + 1;
            continue;
        }

        if closing {
            if is_emphasis_html_tag(&name) {
                push_html_span(
                    &mut spans,
                    &mut current,
                    &emphasis_stack,
                    &mut pending_space,
                );
                emphasis_stack.pop();
            }
            if name == "blockquote" {
                push_html_block(
                    &mut blocks,
                    &mut spans,
                    &mut current,
                    &mut pending_space,
                    quote_depth,
                    heading_level,
                );
                quote_depth = quote_depth.saturating_sub(1);
            } else if is_block_html_tag(&name) {
                push_html_block(
                    &mut blocks,
                    &mut spans,
                    &mut current,
                    &mut pending_space,
                    quote_depth,
                    heading_level,
                );
            }
        } else if name == "script" || name == "style" || name == "noscript" {
            if !self_closing {
                ignored_tag = Some(name);
            }
        } else if name == "blockquote" {
            push_html_block(
                &mut blocks,
                &mut spans,
                &mut current,
                &mut pending_space,
                quote_depth,
                heading_level,
            );
            quote_depth = quote_depth.saturating_add(1);
        } else if name == "img" {
            push_html_block(
                &mut blocks,
                &mut spans,
                &mut current,
                &mut pending_space,
                quote_depth,
                heading_level,
            );
            let alt = extract_html_attribute(&raw_tag, "alt")
                .map(|value| decode_entities(&value).trim().to_string())
                .filter(|value| !value.is_empty());
            let src = extract_html_attribute(&raw_tag, "src")
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty());
            blocks.push(ContentBlock {
                kind: "image".to_string(),
                level: None,
                spans: Vec::new(),
                alt,
                src,
            });
        } else if name == "br" {
            push_html_block(
                &mut blocks,
                &mut spans,
                &mut current,
                &mut pending_space,
                quote_depth,
                heading_level,
            );
        } else if is_emphasis_html_tag(&name) {
            push_html_span(
                &mut spans,
                &mut current,
                &emphasis_stack,
                &mut pending_space,
            );
            emphasis_stack.push(emphasis_name(&name).to_string());
        } else if is_block_html_tag(&name) {
            push_html_block(
                &mut blocks,
                &mut spans,
                &mut current,
                &mut pending_space,
                quote_depth,
                heading_level,
            );
            heading_level = heading_level_from_tag(&name);
        }

        index = tag_end + 1;
    }

    push_html_block(
        &mut blocks,
        &mut spans,
        &mut current,
        &mut pending_space,
        quote_depth,
        heading_level,
    );

    ContentDocument { version: 1, blocks }
}

fn is_emphasis_html_tag(name: &str) -> bool {
    matches!(name, "strong" | "b" | "em" | "i")
}

fn emphasis_name(name: &str) -> &'static str {
    match name {
        "strong" | "b" => "strong",
        _ => "em",
    }
}

fn heading_level_from_tag(name: &str) -> Option<u8> {
    match name {
        "h1" => Some(1),
        "h2" => Some(2),
        "h3" => Some(3),
        "h4" => Some(4),
        "h5" => Some(5),
        "h6" => Some(6),
        _ => None,
    }
}

fn push_html_span(
    spans: &mut Vec<ContentSpan>,
    current: &mut String,
    emphasis_stack: &[String],
    pending_space: &mut bool,
) {
    let raw = std::mem::take(current);
    let leading = raw
        .chars()
        .next()
        .map(|value| value.is_whitespace())
        .unwrap_or(false);
    let trailing = raw
        .chars()
        .last()
        .map(|value| value.is_whitespace())
        .unwrap_or(false);
    let normalized = decode_entities(&raw)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");

    if normalized.is_empty() {
        if leading || trailing {
            *pending_space = true;
        }
        return;
    }

    if *pending_space || leading {
        if let Some(previous) = spans.last_mut() {
            if !previous.text.ends_with(' ') {
                previous.text.push(' ');
            }
        }
    }

    let emphasis = emphasis_stack.last().cloned();
    if let Some(previous) = spans.last_mut() {
        if previous.emphasis == emphasis {
            previous.text.push_str(&normalized);
        } else {
            spans.push(ContentSpan {
                text: normalized,
                emphasis,
            });
        }
    } else {
        spans.push(ContentSpan {
            text: normalized,
            emphasis,
        });
    }

    *pending_space = trailing;
}

fn push_html_block(
    blocks: &mut Vec<ContentBlock>,
    spans: &mut Vec<ContentSpan>,
    current: &mut String,
    pending_space: &mut bool,
    quote_depth: usize,
    heading_level: Option<u8>,
) {
    push_html_span(spans, current, &[], pending_space);
    if spans.is_empty() {
        return;
    }

    let kind = if quote_depth > 0 {
        "quote"
    } else if heading_level.is_some() {
        "heading"
    } else {
        "paragraph"
    };
    blocks.push(ContentBlock {
        kind: kind.to_string(),
        level: heading_level,
        spans: std::mem::take(spans),
        alt: None,
        src: None,
    });
    *pending_space = false;
}

fn strip_html(html: &str) -> String {
    let chars: Vec<char> = html.chars().collect();
    let mut lines = Vec::new();
    let mut current = String::new();
    let mut quote_depth = 0usize;
    let mut ignored_tag: Option<String> = None;
    let mut index = 0usize;

    while index < chars.len() {
        if chars[index] != '<' {
            if ignored_tag.is_none() {
                current.push(chars[index]);
            }
            index += 1;
            continue;
        }

        if ignored_tag.is_none() && is_html_comment_start(&chars, index) {
            index = skip_html_comment(&chars, index);
            continue;
        }

        let Some(tag_end) = find_html_tag_end(&chars, index) else {
            if ignored_tag.is_none() {
                current.push(chars[index]);
            }
            index += 1;
            continue;
        };

        let raw_tag: String = chars[index + 1..tag_end].iter().collect();
        let (closing, name, self_closing) = parse_html_tag(&raw_tag);

        if let Some(ignored) = ignored_tag.as_deref() {
            if closing && name == ignored {
                ignored_tag = None;
            }
            index = tag_end + 1;
            continue;
        }

        if name.is_empty() {
            index = tag_end + 1;
            continue;
        }

        if closing {
            if name == "blockquote" {
                push_html_line(&mut lines, &mut current, quote_depth);
                quote_depth = quote_depth.saturating_sub(1);
            } else if is_block_html_tag(&name) {
                push_html_line(&mut lines, &mut current, quote_depth);
            }
        } else if name == "script" || name == "style" || name == "noscript" {
            if !self_closing {
                ignored_tag = Some(name);
            }
        } else if name == "blockquote" {
            push_html_line(&mut lines, &mut current, quote_depth);
            quote_depth = quote_depth.saturating_add(1);
        } else if name == "img" {
            push_html_line(&mut lines, &mut current, quote_depth);
            let alt = extract_html_attribute(&raw_tag, "alt")
                .map(|value| decode_entities(&value).trim().to_string())
                .filter(|value| !value.is_empty());
            let label = alt
                .map(|value| format!("图片：{value}"))
                .unwrap_or_else(|| "图片".to_string());
            let line = format!("[{label}]");
            lines.push(if quote_depth > 0 {
                format!("> {line}")
            } else {
                line
            });
        } else if name == "br" {
            push_html_line(&mut lines, &mut current, quote_depth);
        } else if is_block_html_tag(&name) {
            push_html_line(&mut lines, &mut current, quote_depth);
        }

        index = tag_end + 1;
    }

    push_html_line(&mut lines, &mut current, quote_depth);
    lines.join("\n\n")
}

fn is_html_comment_start(chars: &[char], index: usize) -> bool {
    matches!(
        (
            chars.get(index),
            chars.get(index + 1),
            chars.get(index + 2),
            chars.get(index + 3)
        ),
        (Some('<'), Some('!'), Some('-'), Some('-'))
    )
}

fn skip_html_comment(chars: &[char], index: usize) -> usize {
    let mut cursor = index + 4;
    while cursor + 2 < chars.len() {
        if chars[cursor] == '-' && chars[cursor + 1] == '-' && chars[cursor + 2] == '>' {
            return cursor + 3;
        }
        cursor += 1;
    }
    chars.len()
}

fn find_html_tag_end(chars: &[char], start: usize) -> Option<usize> {
    let mut quote = None;
    for (index, character) in chars.iter().enumerate().skip(start + 1) {
        match quote {
            Some(expected) if *character == expected => quote = None,
            Some(_) => {}
            None if *character == '"' || *character == '\'' => quote = Some(*character),
            None if *character == '>' => return Some(index),
            None => {}
        }
    }
    None
}

fn parse_html_tag(raw_tag: &str) -> (bool, String, bool) {
    let trimmed = raw_tag.trim();
    let closing = trimmed.starts_with('/');
    let mut body = if closing {
        trimmed[1..].trim_start()
    } else {
        trimmed
    };
    let self_closing = body.ends_with('/');
    if self_closing {
        body = body[..body.len() - 1].trim_end();
    }
    let name = body
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .trim_end_matches('/')
        .to_ascii_lowercase();
    (closing, name, self_closing)
}

fn is_block_html_tag(name: &str) -> bool {
    matches!(
        name,
        "address"
            | "article"
            | "aside"
            | "dd"
            | "div"
            | "dl"
            | "dt"
            | "figcaption"
            | "figure"
            | "footer"
            | "h1"
            | "h2"
            | "h3"
            | "h4"
            | "h5"
            | "h6"
            | "header"
            | "hr"
            | "li"
            | "main"
            | "nav"
            | "ol"
            | "p"
            | "pre"
            | "section"
            | "table"
            | "tbody"
            | "td"
            | "tfoot"
            | "th"
            | "thead"
            | "tr"
            | "ul"
    )
}

fn push_html_line(lines: &mut Vec<String>, current: &mut String, quote_depth: usize) {
    let normalized = decode_entities(current)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if !normalized.is_empty() {
        lines.push(if quote_depth > 0 {
            format!("> {normalized}")
        } else {
            normalized
        });
    }
    current.clear();
}

fn extract_html_attribute(tag: &str, name: &str) -> Option<String> {
    let bytes = tag.as_bytes();
    let mut cursor = 0usize;

    while cursor < bytes.len() {
        while cursor < bytes.len() && (bytes[cursor].is_ascii_whitespace() || bytes[cursor] == b'/')
        {
            cursor += 1;
        }
        let key_start = cursor;
        while cursor < bytes.len()
            && !bytes[cursor].is_ascii_whitespace()
            && bytes[cursor] != b'='
            && bytes[cursor] != b'/'
        {
            cursor += 1;
        }
        if key_start == cursor {
            cursor += 1;
            continue;
        }
        let key = &tag[key_start..cursor];

        while cursor < bytes.len() && bytes[cursor].is_ascii_whitespace() {
            cursor += 1;
        }
        if cursor >= bytes.len() || bytes[cursor] != b'=' {
            continue;
        }

        cursor += 1;
        while cursor < bytes.len() && bytes[cursor].is_ascii_whitespace() {
            cursor += 1;
        }
        let value = if cursor < bytes.len() && (bytes[cursor] == b'"' || bytes[cursor] == b'\'') {
            let quote = bytes[cursor];
            cursor += 1;
            let value_start = cursor;
            while cursor < bytes.len() && bytes[cursor] != quote {
                cursor += 1;
            }
            let value = tag[value_start..cursor].to_string();
            if cursor < bytes.len() {
                cursor += 1;
            }
            value
        } else {
            let value_start = cursor;
            while cursor < bytes.len() && !bytes[cursor].is_ascii_whitespace() {
                cursor += 1;
            }
            tag[value_start..cursor].to_string()
        };

        if key.eq_ignore_ascii_case(name) {
            return Some(value);
        }
    }

    None
}

fn decode_entities(text: &str) -> String {
    let mut decoded = String::with_capacity(text.len());
    let mut cursor = 0usize;

    while cursor < text.len() {
        let Some(relative_ampersand) = text[cursor..].find('&') else {
            decoded.push_str(&text[cursor..]);
            break;
        };
        let ampersand = cursor + relative_ampersand;
        decoded.push_str(&text[cursor..ampersand]);

        let Some(relative_end) = text[ampersand + 1..].find(';') else {
            decoded.push_str(&text[ampersand..]);
            break;
        };
        let end = ampersand + 1 + relative_end;
        let entity = &text[ampersand + 1..end];
        if let Some(value) = decode_entity(entity) {
            decoded.push(value);
            cursor = end + 1;
        } else {
            decoded.push('&');
            cursor = ampersand + 1;
        }
    }

    decoded
}

fn decode_entity(entity: &str) -> Option<char> {
    match entity.to_ascii_lowercase().as_str() {
        "nbsp" => Some(' '),
        "amp" => Some('&'),
        "quot" => Some('"'),
        "apos" => Some('\''),
        "lt" => Some('<'),
        "gt" => Some('>'),
        "mdash" => Some('—'),
        "ndash" => Some('–'),
        "hellip" => Some('…'),
        "ldquo" => Some('“'),
        "rdquo" => Some('”'),
        "lsquo" => Some('‘'),
        "rsquo" => Some('’'),
        "middot" => Some('·'),
        value if value.starts_with("#x") => u32::from_str_radix(&value[2..], 16)
            .ok()
            .and_then(char::from_u32),
        value if value.starts_with('#') => value[1..].parse::<u32>().ok().and_then(char::from_u32),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_common_chapter_headings() {
        assert!(looks_like_chapter("第一章 初见"));
        assert!(looks_like_chapter("番外：春日"));
        assert!(!looks_like_chapter("这是一个很长很长的普通段落"));
    }

    #[test]
    fn parses_utf8_txt() {
        let book = parse_book_bytes("demo.txt", "第一章\n你好\n\n第二章\n世界".as_bytes())
            .expect("txt should parse");
        assert_eq!(book.title, "demo");
        assert_eq!(book.chapters.len(), 2);
        assert_eq!(book.chapters[1].content, "世界");
    }

    #[test]
    fn strips_utf8_bom_and_preserves_indentation() {
        let bytes = [
            vec![0xEF, 0xBB, 0xBF],
            "第一章\n  首行缩进\n\n第二章\n第二段内容"
                .as_bytes()
                .to_vec(),
        ]
        .concat();
        let book = parse_book_bytes("bom.txt", &bytes).expect("BOM txt should parse");

        assert_eq!(book.chapters.len(), 2);
        assert_eq!(book.chapters[0].title, "第一章");
        assert_eq!(book.chapters[0].content, "  首行缩进");
    }

    #[test]
    fn strips_html_blocks() {
        assert_eq!(
            strip_html("<h1>标题</h1><p>第一段</p><p>第二段</p>"),
            "标题\n\n第一段\n\n第二段"
        );
    }

    #[test]
    fn resolves_local_image_sources_and_rejects_external_urls() {
        let mut document = ContentDocument {
            version: 1,
            blocks: vec![
                ContentBlock {
                    kind: "image".to_string(),
                    level: None,
                    spans: Vec::new(),
                    alt: Some("封面".to_string()),
                    src: Some("../Images/cover.jpg".to_string()),
                },
                ContentBlock {
                    kind: "image".to_string(),
                    level: None,
                    spans: Vec::new(),
                    alt: None,
                    src: Some("https://example.test/cover.jpg".to_string()),
                },
            ],
        };
        let mut image_sources = HashMap::new();
        image_sources.insert(
            "OPS/Images/cover.jpg".to_string(),
            "data:image/jpeg;base64,AAAA".to_string(),
        );

        resolve_epub_images(&mut document, "OPS/Text/chapter.xhtml", &image_sources);

        assert_eq!(
            document.blocks[0].src.as_deref(),
            Some("data:image/jpeg;base64,AAAA")
        );
        assert!(document.blocks[1].src.is_none());
    }

    #[test]
    fn parses_content_blocks_with_heading_quote_and_emphasis() {
        let document = parse_html_document(
            "<h1>标题</h1><p>正文 <strong>重点</strong></p><blockquote>引用</blockquote>",
        );

        assert_eq!(document.version, 1);
        assert_eq!(document.blocks.len(), 3);
        assert_eq!(document.blocks[0].kind, "heading");
        assert_eq!(document.blocks[0].level, Some(1));
        assert_eq!(
            document.blocks[1]
                .spans
                .iter()
                .map(|span| span.text.as_str())
                .collect::<String>(),
            "正文 重点"
        );
        assert_eq!(
            document.blocks[1].spans[1].emphasis.as_deref(),
            Some("strong")
        );
        assert_eq!(document.blocks[2].kind, "quote");
    }

    #[test]
    fn ignores_script_and_style_content() {
        assert_eq!(
            strip_html("<script>alert(1)</script><style>.danger{color:red}</style><p>正文</p>"),
            "正文"
        );
    }

    #[test]
    fn preserves_blockquotes_and_image_placeholders() {
        assert_eq!(
            strip_html(
                "<blockquote><p>引用&nbsp;内容</p></blockquote><p>正文 <strong>重点</strong></p><img alt=\"封面 &amp; 目录\" src=\"https://example.test/cover.jpg\">"
            ),
            "> 引用 内容\n\n正文 重点\n\n[图片：封面 & 目录]"
        );
    }
}
