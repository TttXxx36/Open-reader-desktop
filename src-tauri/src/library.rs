use encoding_rs::{GB18030, UTF_16BE, UTF_16LE};
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

#[derive(Debug, Clone)]
pub struct ParsedChapter {
    pub title: String,
    pub content: String,
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

    chapters.push(ParsedChapter { title, content });
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
        let content = strip_html(&html);
        if content.is_empty() {
            continue;
        }

        let title = ["h1", "h2", "h3"]
            .iter()
            .find_map(|tag| extract_element_text(&html, tag))
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| format!("第 {} 章", chapters.len() + 1));

        chapters.push(ParsedChapter { title, content });
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

fn read_zip_text<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
    path: &str,
) -> Result<String, ImportError> {
    let mut file = archive.by_name(path)?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
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

fn strip_html(html: &str) -> String {
    let mut text = String::with_capacity(html.len());
    let mut in_tag = false;
    let mut tag = String::new();

    for character in html.chars() {
        if in_tag {
            if character == '>' {
                let normalized = tag.trim().to_ascii_lowercase();
                if normalized.starts_with("br")
                    || normalized.starts_with("/p")
                    || normalized.starts_with("/div")
                    || normalized.starts_with("/li")
                    || normalized.starts_with("/h")
                    || normalized.starts_with("/blockquote")
                {
                    text.push('\n');
                }
                tag.clear();
                in_tag = false;
            } else {
                tag.push(character);
            }
        } else if character == '<' {
            in_tag = true;
        } else {
            text.push(character);
        }
    }

    let decoded = decode_entities(&text);
    decoded
        .lines()
        .map(|line| line.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn decode_entities(text: &str) -> String {
    text.replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
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
            "第一章\n  首行缩进\n\n第二章\n正文".as_bytes().to_vec(),
        ]
        .concat();
        let book = parse_book_bytes("bom.txt", &bytes).expect("BOM txt should parse");

        assert_eq!(book.chapters.len(), 2, "{:?}", book.chapters);
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
}
