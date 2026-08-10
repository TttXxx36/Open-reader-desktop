use base64::{engine::general_purpose::STANDARD, Engine as _};
use encoding_rs::{GB18030, UTF_16BE, UTF_16LE};
use image::{ImageDecoder, ImageReader, Limits};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::{
    borrow::Cow,
    collections::HashMap,
    fs::{self, OpenOptions},
    io::{Cursor, Read, Seek, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};
use thiserror::Error;
use zip::ZipArchive;

#[derive(Debug, Error)]
pub enum ImportError {
    #[error("unsupported book format: {0}")]
    UnsupportedFormat(String),
    #[error("unable to decode text file")]
    TextDecode,
    #[error("invalid TXT parsing options: {0}")]
    InvalidTxtOptions(String),
    #[error("file I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid EPUB: {0}")]
    InvalidEpub(String),
    #[error("EPUB archive error: {0}")]
    Archive(#[from] zip::result::ZipError),
    #[error("invalid image: {0}")]
    InvalidImage(String),
}

#[derive(Debug, Clone)]
pub struct ParsedBook {
    pub title: String,
    pub author: Option<String>,
    pub format: String,
    pub chapters: Vec<ParsedChapter>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TxtChapterRule {
    Auto,
    Disabled,
    Regex,
}

impl Default for TxtChapterRule {
    fn default() -> Self {
        Self::Auto
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TxtReplacement {
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TxtParseOptions {
    #[serde(default)]
    pub chapter_rule: TxtChapterRule,
    #[serde(default)]
    pub custom_pattern: Option<String>,
    #[serde(default)]
    pub normalize_full_width_space: bool,
    #[serde(default)]
    pub replacements: Vec<TxtReplacement>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BookImportPreview {
    pub title: String,
    pub format: String,
    pub encoding: Option<String>,
    pub chapter_count: usize,
    pub first_chapter_title: Option<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BookFormatKind {
    Txt,
    Epub,
    Mobi,
    Azw,
    Azw3,
    Pdf,
    Image,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FormatSupport {
    Importable,
    ProbeOnly,
    Unsupported,
    SignatureMismatch,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FormatProbeMetadata {
    Pdf {
        version: String,
    },
    Image {
        mime: String,
        width: Option<u32>,
        height: Option<u32>,
    },
    Mobi {
        record_offset: u32,
        header_length: Option<u32>,
    },
}

#[derive(Debug, Clone, Serialize)]
pub struct BookFormatProbe {
    pub format: BookFormatKind,
    pub support: FormatSupport,
    pub signature_match: bool,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<FormatProbeMetadata>,
}

/// Rejects formats that are only probed or whose extension conflicts with a known signature.
///
/// This is the single import boundary used by desktop commands. In particular, MOBI/AZW
/// remain probe-only and are never routed to a parser or a DRM-bypass implementation.
pub fn require_importable_format(file_name: &str, bytes: &[u8]) -> Result<(), ImportError> {
    let probe = probe_book_format(file_name, bytes);
    if probe.support == FormatSupport::Importable {
        return Ok(());
    }
    Err(ImportError::UnsupportedFormat(format!(
        "{file_name}: {}",
        probe.message
    )))
}

pub fn probe_book_format(file_name: &str, bytes: &[u8]) -> BookFormatProbe {
    let extension = Path::new(file_name)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let extension_kind = match extension.as_str() {
        "txt" => BookFormatKind::Txt,
        "epub" => BookFormatKind::Epub,
        "mobi" => BookFormatKind::Mobi,
        "azw" => BookFormatKind::Azw,
        "azw3" => BookFormatKind::Azw3,
        "pdf" => BookFormatKind::Pdf,
        "png" | "jpg" | "jpeg" | "gif" | "webp" => BookFormatKind::Image,
        _ => BookFormatKind::Unknown,
    };
    let magic_kind = detect_magic_format(bytes);
    let format = if extension_kind == BookFormatKind::Unknown {
        magic_kind
    } else {
        extension_kind
    };
    let signature_match = formats_compatible(extension_kind, magic_kind)
        && match format {
            BookFormatKind::Txt => !bytes.is_empty(),
            BookFormatKind::Epub => bytes.starts_with(b"PK\x03\x04"),
            BookFormatKind::Mobi | BookFormatKind::Azw | BookFormatKind::Azw3 => {
                has_mobi_header(bytes)
            }
            BookFormatKind::Pdf => bytes.starts_with(b"%PDF-"),
            BookFormatKind::Image => has_image_signature(bytes),
            BookFormatKind::Unknown => false,
        };
    let support = match (format, signature_match) {
        (BookFormatKind::Txt | BookFormatKind::Epub, true) => FormatSupport::Importable,
        (
            BookFormatKind::Mobi
            | BookFormatKind::Azw
            | BookFormatKind::Azw3
            | BookFormatKind::Pdf
            | BookFormatKind::Image,
            true,
        ) => FormatSupport::ProbeOnly,
        (BookFormatKind::Unknown, _) => FormatSupport::Unsupported,
        (_, false) => FormatSupport::SignatureMismatch,
    };
    let message = match (format, support) {
        (BookFormatKind::Txt, FormatSupport::Importable) => "TXT 可进入现有导入流程".to_string(),
        (BookFormatKind::Epub, FormatSupport::Importable) => "EPUB 可进入现有导入流程".to_string(),
        (
            BookFormatKind::Mobi | BookFormatKind::Azw | BookFormatKind::Azw3,
            FormatSupport::ProbeOnly,
        ) => "已识别 MOBI/AZW 容器；当前仅做只读探测，尚未导入且不会绕过 DRM".to_string(),
        (BookFormatKind::Pdf, FormatSupport::ProbeOnly) => {
            "已识别 PDF；需要独立的渲染、搜索和目录模型".to_string()
        }
        (BookFormatKind::Image, FormatSupport::ProbeOnly) => {
            "已识别图片；需要独立的缓存、缩放和阅读方向模型".to_string()
        }
        (_, FormatSupport::SignatureMismatch) => "文件扩展名与内容签名不匹配".to_string(),
        (_, FormatSupport::Unsupported) => "暂不支持该文件格式".to_string(),
        _ => "格式已识别".to_string(),
    };

    let metadata = signature_match
        .then(|| format_probe_metadata(format, bytes))
        .flatten();

    BookFormatProbe {
        format,
        support,
        signature_match,
        message,
        metadata,
    }
}

pub const MAX_IMAGE_INPUT_BYTES: usize = 64 * 1024 * 1024;
const MAX_IMAGE_DIMENSION: u32 = 20_000;
const MAX_IMAGE_PIXELS: u64 = 32_000_000;
const MAX_IMAGE_DECODED_BYTES: u64 = 128 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageDocumentPreview {
    pub file_name: String,
    pub mime: String,
    pub width: u32,
    pub height: u32,
    pub color_type: String,
    pub decoded_bytes: u64,
}

/// Decodes exactly one image while enforcing input, dimension, pixel, and decoded-buffer quotas.
/// The decoded pixels are deliberately dropped after validation; the UI keeps the original
/// local file as a short-lived object URL for the first single-page preview.
pub fn preview_image_bytes(
    file_name: &str,
    bytes: &[u8],
) -> Result<ImageDocumentPreview, ImportError> {
    if bytes.len() > MAX_IMAGE_INPUT_BYTES {
        return Err(ImportError::InvalidImage(format!(
            "{file_name}: 文件超过 {} MB 图片输入上限",
            MAX_IMAGE_INPUT_BYTES / (1024 * 1024)
        )));
    }

    let probe = probe_book_format(file_name, bytes);
    if probe.format != BookFormatKind::Image || !probe.signature_match {
        return Err(ImportError::InvalidImage(format!(
            "{file_name}: 图片签名无效或与扩展名不匹配"
        )));
    }

    let reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|error| ImportError::InvalidImage(format!("无法识别图片格式：{error}")))?;
    let image_format = reader
        .format()
        .ok_or_else(|| ImportError::InvalidImage("无法识别图片格式".to_string()))?;
    let mime = match image_format {
        image::ImageFormat::Png => "image/png",
        image::ImageFormat::Jpeg => "image/jpeg",
        image::ImageFormat::Gif => "image/gif",
        image::ImageFormat::WebP => "image/webp",
        _ => {
            return Err(ImportError::InvalidImage(format!(
                "{file_name}: 仅允许 PNG、JPEG、GIF、WebP"
            )))
        }
    };

    let mut decoder = reader
        .into_decoder()
        .map_err(|error| ImportError::InvalidImage(format!("图片解码器初始化失败：{error}")))?;
    let (width, height) = decoder.dimensions();
    let pixels = u64::from(width)
        .checked_mul(u64::from(height))
        .ok_or_else(|| ImportError::InvalidImage("图片像素数量溢出".to_string()))?;
    if width == 0
        || height == 0
        || width > MAX_IMAGE_DIMENSION
        || height > MAX_IMAGE_DIMENSION
        || pixels > MAX_IMAGE_PIXELS
    {
        return Err(ImportError::InvalidImage(format!(
            "{file_name}: 图片尺寸或像素数超过受限预览配额（最大 {}×{}、{} 像素）",
            MAX_IMAGE_DIMENSION, MAX_IMAGE_DIMENSION, MAX_IMAGE_PIXELS
        )));
    }

    let decoded_bytes = decoder.total_bytes();
    if decoded_bytes > MAX_IMAGE_DECODED_BYTES {
        return Err(ImportError::InvalidImage(format!(
            "{file_name}: 解码缓冲区超过 {} MiB 上限",
            MAX_IMAGE_DECODED_BYTES / (1024 * 1024)
        )));
    }
    let color_type = format!("{:?}", decoder.color_type());

    let mut limits = Limits::default();
    limits.max_alloc = Some(MAX_IMAGE_DECODED_BYTES);
    decoder
        .set_limits(limits)
        .map_err(|error| ImportError::InvalidImage(format!("图片解码配额不受支持：{error}")))?;

    let buffer_length = usize::try_from(decoded_bytes)
        .map_err(|_| ImportError::InvalidImage("解码缓冲区超过当前平台可用大小".to_string()))?;
    let mut decoded = vec![0_u8; buffer_length];
    decoder
        .read_image(&mut decoded)
        .map_err(|error| ImportError::InvalidImage(format!("图片解码失败：{error}")))?;
    drop(decoded);

    Ok(ImageDocumentPreview {
        file_name: file_name.to_string(),
        mime: mime.to_string(),
        width,
        height,
        color_type,
        decoded_bytes,
    })
}

const MAX_IMAGE_SEQUENCE_PAGES: usize = 2_048;
const MAX_IMAGE_SEQUENCE_PIXELS: u64 = 128_000_000;
const MAX_IMAGE_SEQUENCE_DECODED_BYTES: u64 = 512 * 1024 * 1024;
pub const MAX_IMAGE_SEQUENCE_INPUT_BYTES: usize = 256 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageReadingDirection {
    Ltr,
    Rtl,
    Vertical,
}

impl Default for ImageReadingDirection {
    fn default() -> Self {
        Self::Ltr
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageSpreadMode {
    Single,
    Double,
    LongStrip,
}

impl Default for ImageSpreadMode {
    fn default() -> Self {
        Self::Single
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageSequencePage {
    pub index: usize,
    pub file_name: String,
    pub mime: String,
    pub width: u32,
    pub height: u32,
    pub decoded_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageSequencePreview {
    pub direction: ImageReadingDirection,
    pub spread: ImageSpreadMode,
    pub page_count: usize,
    pub total_pixels: u64,
    pub total_decoded_bytes: u64,
    pub pages: Vec<ImageSequencePage>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ImageSequenceInput {
    pub file_name: String,
    pub bytes: Vec<u8>,
}

/// Decodes a bounded set of local image files and then builds the serializable sequence model.
/// Each page passes the single-image decoder gate before sequence totals are evaluated.
pub fn preview_image_sequence_bytes(
    inputs: Vec<ImageSequenceInput>,
    direction: ImageReadingDirection,
    spread: ImageSpreadMode,
) -> Result<ImageSequencePreview, ImportError> {
    if inputs.is_empty() {
        return Err(ImportError::InvalidImage("图片序列不能为空".to_string()));
    }
    if inputs.len() > MAX_IMAGE_SEQUENCE_PAGES {
        return Err(ImportError::InvalidImage(format!(
            "图片序列页数超过 {} 页上限",
            MAX_IMAGE_SEQUENCE_PAGES
        )));
    }

    let mut input_bytes = 0_usize;
    let mut previews = Vec::with_capacity(inputs.len());
    for input in inputs {
        input_bytes = input_bytes
            .checked_add(input.bytes.len())
            .ok_or_else(|| ImportError::InvalidImage("图片序列输入大小溢出".to_string()))?;
        if input_bytes > MAX_IMAGE_SEQUENCE_INPUT_BYTES {
            return Err(ImportError::InvalidImage(format!(
                "图片序列原始输入超过 {} MiB 上限",
                MAX_IMAGE_SEQUENCE_INPUT_BYTES / (1024 * 1024)
            )));
        }
        previews.push(preview_image_bytes(&input.file_name, &input.bytes)?);
    }

    build_image_sequence_preview(previews, direction, spread)
}

/// Builds the serializable page/sequence contract after each page has passed the decoder gate.
/// It does not decode or trust arbitrary paths, URLs, or raw bytes; callers must provide
/// ImageDocumentPreview values returned by preview_image_bytes.
pub fn build_image_sequence_preview(
    pages: Vec<ImageDocumentPreview>,
    direction: ImageReadingDirection,
    spread: ImageSpreadMode,
) -> Result<ImageSequencePreview, ImportError> {
    if pages.is_empty() {
        return Err(ImportError::InvalidImage("图片序列不能为空".to_string()));
    }
    if pages.len() > MAX_IMAGE_SEQUENCE_PAGES {
        return Err(ImportError::InvalidImage(format!(
            "图片序列页数超过 {} 页上限",
            MAX_IMAGE_SEQUENCE_PAGES
        )));
    }

    let mut total_pixels = 0_u64;
    let mut total_decoded_bytes = 0_u64;
    let mut sequence_pages = Vec::with_capacity(pages.len());
    for (index, page) in pages.into_iter().enumerate() {
        if page.width == 0 || page.height == 0 || page.mime.is_empty() {
            return Err(ImportError::InvalidImage(format!(
                "第 {} 页图片元数据无效",
                index + 1
            )));
        }
        let pixels = u64::from(page.width)
            .checked_mul(u64::from(page.height))
            .ok_or_else(|| ImportError::InvalidImage("图片序列像素数溢出".to_string()))?;
        total_pixels = total_pixels
            .checked_add(pixels)
            .ok_or_else(|| ImportError::InvalidImage("图片序列总像素数溢出".to_string()))?;
        total_decoded_bytes = total_decoded_bytes
            .checked_add(page.decoded_bytes)
            .ok_or_else(|| ImportError::InvalidImage("图片序列解码内存溢出".to_string()))?;
        if total_pixels > MAX_IMAGE_SEQUENCE_PIXELS {
            return Err(ImportError::InvalidImage(format!(
                "图片序列总像素数超过 {} 上限",
                MAX_IMAGE_SEQUENCE_PIXELS
            )));
        }
        if total_decoded_bytes > MAX_IMAGE_SEQUENCE_DECODED_BYTES {
            return Err(ImportError::InvalidImage(format!(
                "图片序列解码内存超过 {} MiB 上限",
                MAX_IMAGE_SEQUENCE_DECODED_BYTES / (1024 * 1024)
            )));
        }

        let ImageDocumentPreview {
            file_name,
            mime,
            width,
            height,
            decoded_bytes,
            ..
        } = page;
        sequence_pages.push(ImageSequencePage {
            index,
            file_name,
            mime,
            width,
            height,
            decoded_bytes,
        });
    }

    Ok(ImageSequencePreview {
        direction,
        spread,
        page_count: sequence_pages.len(),
        total_pixels,
        total_decoded_bytes,
        pages: sequence_pages,
    })
}
pub const MAX_IMAGE_THUMBNAIL_BYTES: u64 = 8 * 1024 * 1024;
pub const MAX_IMAGE_CACHE_BYTES: u64 = 512 * 1024 * 1024;
const MAX_IMAGE_THUMBNAIL_DIMENSION: u32 = 1_600;
const IMAGE_CACHE_KEY_PREFIX: &str = "imgseq-v1-";
const PNG_SIGNATURE: [u8; 8] = [137, 80, 78, 71, 13, 10, 26, 10];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageThumbnailCacheEntry {
    pub cache_key: String,
    pub page_index: usize,
    pub byte_len: u64,
    pub cache_hit: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageThumbnailCacheSummary {
    pub cache_key: String,
    pub page_count: usize,
    pub cache_hits: usize,
    pub cache_writes: usize,
    pub evicted_files: usize,
    pub cleaned_temp_files: usize,
    pub cache_bytes: u64,
    pub entries: Vec<ImageThumbnailCacheEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageThumbnailPageBytes {
    pub page_index: usize,
    pub mime: String,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone)]
struct CachedImageFile {
    path: PathBuf,
    size: u64,
    modified: SystemTime,
}

#[derive(Debug, Default, Clone, Copy)]
struct ImageCachePruneResult {
    remaining_bytes: u64,
    evicted_files: usize,
    cleaned_temp_files: usize,
}

const MAX_IMAGE_THUMBNAIL_READ_PAGES: usize = 3;

fn is_safe_image_cache_key(cache_key: &str) -> bool {
    let Some(suffix) = cache_key.strip_prefix(IMAGE_CACHE_KEY_PREFIX) else {
        return false;
    };
    (8..=64).contains(&suffix.len())
        && suffix.bytes().all(|byte| {
            matches!(
                byte,
                b'0'..=b'9' | b'a'..=b'f' | b'A'..=b'F'
            )
        })
}

fn cached_thumbnail_size(path: &Path) -> Result<Option<u64>, ImportError> {
    let metadata = match fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(ImportError::Io(error)),
    };
    let size = metadata.len();
    if size == 0 || size > MAX_IMAGE_THUMBNAIL_BYTES {
        return Ok(None);
    }

    let mut file = fs::File::open(path)?;
    let mut signature = [0_u8; PNG_SIGNATURE.len()];
    if file.read_exact(&mut signature).is_err() || signature != PNG_SIGNATURE {
        return Ok(None);
    }

    Ok(Some(size))
}

fn encode_image_thumbnail(file_name: &str, bytes: &[u8]) -> Result<Vec<u8>, ImportError> {
    preview_image_bytes(file_name, bytes)?;

    let mut reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|error| ImportError::InvalidImage(format!("无法识别图片格式：{error}")))?;
    let mut limits = Limits::default();
    limits.max_alloc = Some(MAX_IMAGE_DECODED_BYTES);
    reader.limits(limits);
    let decoded = reader
        .decode()
        .map_err(|error| ImportError::InvalidImage(format!("图片缩略图解码失败：{error}")))?;
    let thumbnail = decoded.thumbnail(MAX_IMAGE_THUMBNAIL_DIMENSION, MAX_IMAGE_THUMBNAIL_DIMENSION);

    let mut output = Cursor::new(Vec::new());
    thumbnail
        .write_to(&mut output, image::ImageFormat::Png)
        .map_err(|error| ImportError::InvalidImage(format!("图片缩略图编码失败：{error}")))?;
    let encoded = output.into_inner();
    if encoded.len() as u64 > MAX_IMAGE_THUMBNAIL_BYTES {
        return Err(ImportError::InvalidImage(format!(
            "{file_name}: 缩略图超过 {} MiB 上限",
            MAX_IMAGE_THUMBNAIL_BYTES / (1024 * 1024)
        )));
    }
    Ok(encoded)
}

fn check_image_cache_cancellation(cancelled: Option<&AtomicBool>) -> Result<(), ImportError> {
    if cancelled.is_some_and(|token| token.load(Ordering::Relaxed)) {
        return Err(ImportError::InvalidImage("图片缓存操作已取消".to_string()));
    }
    Ok(())
}

fn write_thumbnail_atomically(
    path: &Path,
    bytes: &[u8],
    cancelled: Option<&AtomicBool>,
) -> Result<(), ImportError> {
    check_image_cache_cancellation(cancelled)?;
    let parent = path
        .parent()
        .ok_or_else(|| ImportError::InvalidImage("缩略图缓存路径无父目录".to_string()))?;
    fs::create_dir_all(parent)?;

    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| ImportError::Io(std::io::Error::other(error)))?
        .as_nanos();
    let temp_path = parent.join(format!(
        ".{}.tmp-{}-{}",
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("thumbnail"),
        std::process::id(),
        nonce
    ));

    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_path)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        check_image_cache_cancellation(cancelled)
            .map_err(|error| std::io::Error::other(error.to_string()))?;

        // Cache keys are content-derived. A normal write installs into an absent target
        // atomically; invalidation removes a corrupt target before this function is called.
        fs::rename(&temp_path, path)?;
        Ok::<(), std::io::Error>(())
    })();

    if result.is_err() {
        let _ = fs::remove_file(&temp_path);
    }
    result.map_err(ImportError::Io)
}

fn is_stale_thumbnail_temp(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
        return false;
    };
    let Some((_, suffix)) = name.rsplit_once(".tmp-") else {
        return false;
    };
    let Some((pid, _nonce)) = suffix.split_once('-') else {
        return true;
    };
    pid.parse::<u32>()
        .map_or(true, |value| value != std::process::id())
}

fn remove_stale_thumbnail_temps(root: &Path) -> Result<usize, ImportError> {
    if !root.exists() {
        return Ok(0);
    }

    let mut removed = 0;
    for cache_dir in fs::read_dir(root)? {
        let cache_dir = cache_dir?;
        if !cache_dir.file_type()?.is_dir() {
            continue;
        }
        for file in fs::read_dir(cache_dir.path())? {
            let file = file?;
            if file.file_type()?.is_file() && is_stale_thumbnail_temp(&file.path()) {
                if fs::remove_file(file.path()).is_ok() {
                    removed += 1;
                }
            }
        }
    }
    Ok(removed)
}

fn prune_image_thumbnail_cache(
    root: &Path,
    max_bytes: u64,
) -> Result<ImageCachePruneResult, ImportError> {
    if !root.exists() {
        return Ok(ImageCachePruneResult::default());
    }

    let cleaned_temp_files = remove_stale_thumbnail_temps(root)?;
    let mut files = Vec::new();
    for cache_dir in fs::read_dir(root)? {
        let cache_dir = cache_dir?;
        if !cache_dir.file_type()?.is_dir() {
            continue;
        }
        for file in fs::read_dir(cache_dir.path())? {
            let file = file?;
            if !file.file_type()?.is_file() {
                continue;
            }
            let metadata = file.metadata()?;
            files.push(CachedImageFile {
                path: file.path(),
                size: metadata.len(),
                modified: metadata.modified().unwrap_or(UNIX_EPOCH),
            });
        }
    }

    let mut total_bytes = files
        .iter()
        .fold(0_u64, |total, file| total.saturating_add(file.size));
    if total_bytes > max_bytes {
        files.sort_by(|left, right| {
            left.modified
                .cmp(&right.modified)
                .then_with(|| left.path.cmp(&right.path))
        });
        let mut evicted_files = 0;
        for file in files {
            if total_bytes <= max_bytes {
                break;
            }
            if fs::remove_file(&file.path).is_ok() {
                total_bytes = total_bytes.saturating_sub(file.size);
                evicted_files += 1;
            }
        }
        for cache_dir in fs::read_dir(root)? {
            let cache_dir = cache_dir?;
            if cache_dir.file_type()?.is_dir() && fs::read_dir(cache_dir.path())?.next().is_none() {
                let _ = fs::remove_dir(cache_dir.path());
            }
        }
        return Ok(ImageCachePruneResult {
            remaining_bytes: total_bytes,
            evicted_files,
            cleaned_temp_files,
        });
    }

    Ok(ImageCachePruneResult {
        remaining_bytes: total_bytes,
        evicted_files: 0,
        cleaned_temp_files,
    })
}

pub fn cache_image_sequence_files(
    root: &Path,
    cache_key: &str,
    inputs: &[ImageSequenceInput],
    direction: ImageReadingDirection,
    spread: ImageSpreadMode,
    force_refresh: bool,
) -> Result<ImageThumbnailCacheSummary, ImportError> {
    cache_image_sequence_files_with_cancel(
        root,
        cache_key,
        inputs,
        direction,
        spread,
        force_refresh,
        None,
    )
}

pub fn cache_image_sequence_files_with_cancel(
    root: &Path,
    cache_key: &str,
    inputs: &[ImageSequenceInput],
    direction: ImageReadingDirection,
    spread: ImageSpreadMode,
    force_refresh: bool,
    cancelled: Option<&AtomicBool>,
) -> Result<ImageThumbnailCacheSummary, ImportError> {
    check_image_cache_cancellation(cancelled)?;
    if !is_safe_image_cache_key(cache_key) {
        return Err(ImportError::InvalidImage("图片缓存键格式无效".to_string()));
    }
    if inputs.is_empty() {
        return Err(ImportError::InvalidImage("图片序列不能为空".to_string()));
    }
    if inputs.len() > MAX_IMAGE_SEQUENCE_PAGES {
        return Err(ImportError::InvalidImage(format!(
            "图片序列页数超过 {} 页上限",
            MAX_IMAGE_SEQUENCE_PAGES
        )));
    }

    let mut input_bytes = 0_usize;
    let mut previews = Vec::with_capacity(inputs.len());
    for input in inputs {
        check_image_cache_cancellation(cancelled)?;
        input_bytes = input_bytes
            .checked_add(input.bytes.len())
            .ok_or_else(|| ImportError::InvalidImage("图片序列输入大小溢出".to_string()))?;
        if input_bytes > MAX_IMAGE_SEQUENCE_INPUT_BYTES {
            return Err(ImportError::InvalidImage(format!(
                "图片序列原始输入超过 {} MiB 上限",
                MAX_IMAGE_SEQUENCE_INPUT_BYTES / (1024 * 1024)
            )));
        }
        previews.push(preview_image_bytes(&input.file_name, &input.bytes)?);
    }
    build_image_sequence_preview(previews, direction, spread)?;
    check_image_cache_cancellation(cancelled)?;

    let cache_dir = root.join(cache_key);
    fs::create_dir_all(&cache_dir)?;
    let mut entries = Vec::with_capacity(inputs.len());
    let mut cache_hits = 0;
    let mut cache_writes = 0;

    for (page_index, input) in inputs.iter().enumerate() {
        check_image_cache_cancellation(cancelled)?;
        let path = cache_dir.join(format!("page-{page_index:04}.png"));
        if !force_refresh {
            if let Some(byte_len) = cached_thumbnail_size(&path)? {
                cache_hits += 1;
                entries.push(ImageThumbnailCacheEntry {
                    cache_key: cache_key.to_string(),
                    page_index,
                    byte_len,
                    cache_hit: true,
                });
                continue;
            }
        }
        if path.exists() {
            let _ = fs::remove_file(&path);
        }
        let encoded = encode_image_thumbnail(&input.file_name, &input.bytes)?;
        check_image_cache_cancellation(cancelled)?;
        let byte_len = encoded.len() as u64;
        write_thumbnail_atomically(&path, &encoded, cancelled)?;
        cache_writes += 1;
        entries.push(ImageThumbnailCacheEntry {
            cache_key: cache_key.to_string(),
            page_index,
            byte_len,
            cache_hit: false,
        });
    }

    let prune = prune_image_thumbnail_cache(root, MAX_IMAGE_CACHE_BYTES)?;
    Ok(ImageThumbnailCacheSummary {
        cache_key: cache_key.to_string(),
        page_count: inputs.len(),
        cache_hits,
        cache_writes,
        evicted_files: prune.evicted_files,
        cleaned_temp_files: prune.cleaned_temp_files,
        cache_bytes: prune.remaining_bytes,
        entries,
    })
}

pub fn read_image_thumbnail_files(
    root: &Path,
    cache_key: &str,
    page_indices: &[usize],
) -> Result<Vec<ImageThumbnailPageBytes>, ImportError> {
    if !is_safe_image_cache_key(cache_key) {
        return Err(ImportError::InvalidImage("图片缓存键格式无效".to_string()));
    }
    if page_indices.len() > MAX_IMAGE_THUMBNAIL_READ_PAGES {
        return Err(ImportError::InvalidImage(format!(
            "单次最多读取 {} 张缓存缩略图",
            MAX_IMAGE_THUMBNAIL_READ_PAGES
        )));
    }

    let cache_dir = root.join(cache_key);
    let mut result = Vec::with_capacity(page_indices.len());
    for &page_index in page_indices {
        if page_index >= MAX_IMAGE_SEQUENCE_PAGES {
            return Err(ImportError::InvalidImage("图片页码超出上限".to_string()));
        }
        if result.iter().any(|page: &ImageThumbnailPageBytes| page.page_index == page_index) {
            continue;
        }
        let path = cache_dir.join(format!("page-{page_index:04}.png"));
        let Some(byte_len) = cached_thumbnail_size(&path)? else {
            continue;
        };
        let bytes = fs::read(&path)?;
        if bytes.len() as u64 != byte_len {
            continue;
        }
        result.push(ImageThumbnailPageBytes {
            page_index,
            mime: "image/png".to_string(),
            bytes,
        });
    }
    Ok(result)
}

fn formats_compatible(extension_kind: BookFormatKind, magic_kind: BookFormatKind) -> bool {
    if extension_kind == BookFormatKind::Unknown || magic_kind == BookFormatKind::Unknown {
        return true;
    }
    if extension_kind == magic_kind {
        return true;
    }
    matches!(
        (extension_kind, magic_kind),
        (
            BookFormatKind::Mobi | BookFormatKind::Azw | BookFormatKind::Azw3,
            BookFormatKind::Mobi | BookFormatKind::Azw | BookFormatKind::Azw3
        )
    )
}

fn detect_magic_format(bytes: &[u8]) -> BookFormatKind {
    if bytes.starts_with(b"%PDF-") {
        return BookFormatKind::Pdf;
    }
    if bytes.starts_with(b"PK\x03\x04") {
        return BookFormatKind::Epub;
    }
    if has_mobi_header(bytes) {
        return BookFormatKind::Mobi;
    }
    if has_image_signature(bytes) {
        return BookFormatKind::Image;
    }
    BookFormatKind::Unknown
}

fn has_mobi_header(bytes: &[u8]) -> bool {
    if bytes.len() < 82 {
        return false;
    }
    let record_offset = u32::from_be_bytes([bytes[78], bytes[79], bytes[80], bytes[81]]) as usize;
    record_offset
        .checked_add(20)
        .is_some_and(|end| end <= bytes.len())
        && bytes
            .get(record_offset + 16..record_offset + 20)
            .is_some_and(|marker| marker == b"MOBI")
}

fn has_image_signature(bytes: &[u8]) -> bool {
    bytes.starts_with(b"\x89PNG\r\n\x1A\n")
        || bytes.starts_with(b"\xFF\xD8\xFF")
        || bytes.starts_with(b"GIF87a")
        || bytes.starts_with(b"GIF89a")
        || (bytes.starts_with(b"RIFF") && bytes.len() >= 12 && &bytes[8..12] == b"WEBP")
}

fn format_probe_metadata(format: BookFormatKind, bytes: &[u8]) -> Option<FormatProbeMetadata> {
    match format {
        BookFormatKind::Pdf => parse_pdf_version(bytes).map(|version| FormatProbeMetadata::Pdf {
            version: version.to_string(),
        }),
        BookFormatKind::Image => {
            let (mime, dimensions) = if bytes.starts_with(b"\x89PNG\r\n\x1A\n") {
                ("image/png", png_dimensions(bytes))
            } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
                ("image/gif", gif_dimensions(bytes))
            } else if bytes.starts_with(b"RIFF") && bytes.len() >= 16 && &bytes[8..12] == b"WEBP" {
                ("image/webp", webp_dimensions(bytes))
            } else {
                ("image/jpeg", jpeg_dimensions(bytes))
            };
            Some(FormatProbeMetadata::Image {
                mime: mime.to_string(),
                width: dimensions.map(|(width, _)| width),
                height: dimensions.map(|(_, height)| height),
            })
        }
        BookFormatKind::Mobi | BookFormatKind::Azw | BookFormatKind::Azw3 => mobi_metadata(bytes)
            .map(|(record_offset, header_length)| FormatProbeMetadata::Mobi {
                record_offset,
                header_length,
            }),
        _ => None,
    }
}

fn parse_pdf_version(bytes: &[u8]) -> Option<&str> {
    let version = bytes.get(5..8)?;
    (version[0].is_ascii_digit() && version[1] == b'.' && version[2].is_ascii_digit())
        .then(|| std::str::from_utf8(version).ok())
        .flatten()
}

fn png_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.len() < 24 || &bytes[12..16] != b"IHDR" {
        return None;
    }
    let width = u32::from_be_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]);
    let height = u32::from_be_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]);
    valid_image_dimensions(width, height)
}

fn gif_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.len() < 10 {
        return None;
    }
    let width = u16::from_le_bytes([bytes[6], bytes[7]]) as u32;
    let height = u16::from_le_bytes([bytes[8], bytes[9]]) as u32;
    valid_image_dimensions(width, height)
}

fn webp_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.len() < 30 || &bytes[12..16] != b"VP8X" {
        return None;
    }
    let width =
        1 + u32::from(bytes[24]) + (u32::from(bytes[25]) << 8) + (u32::from(bytes[26]) << 16);
    let height =
        1 + u32::from(bytes[27]) + (u32::from(bytes[28]) << 8) + (u32::from(bytes[29]) << 16);
    valid_image_dimensions(width, height)
}

fn jpeg_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    if !bytes.starts_with(b"\xFF\xD8\xFF") {
        return None;
    }
    const MAX_SCAN_BYTES: usize = 1024 * 1024;
    let limit = bytes.len().min(MAX_SCAN_BYTES);
    let mut cursor = 2;
    while cursor + 1 < limit {
        while cursor < limit && bytes[cursor] != 0xFF {
            cursor += 1;
        }
        while cursor < limit && bytes[cursor] == 0xFF {
            cursor += 1;
        }
        if cursor >= limit {
            break;
        }
        let marker = bytes[cursor];
        cursor += 1;
        if marker == 0xD9 || marker == 0xDA {
            break;
        }
        if marker == 0x01 || (0xD0..=0xD7).contains(&marker) {
            continue;
        }
        if cursor + 2 > limit {
            break;
        }
        let segment_length = u16::from_be_bytes([bytes[cursor], bytes[cursor + 1]]) as usize;
        if segment_length < 2 || cursor + segment_length > limit {
            break;
        }
        let is_sof = matches!(
            marker,
            0xC0..=0xC3
                | 0xC5..=0xC7
                | 0xC9..=0xCB
                | 0xCD..=0xCF
        );
        if is_sof && segment_length >= 7 {
            let height = u16::from_be_bytes([bytes[cursor + 3], bytes[cursor + 4]]) as u32;
            let width = u16::from_be_bytes([bytes[cursor + 5], bytes[cursor + 6]]) as u32;
            return valid_image_dimensions(width, height);
        }
        cursor += segment_length;
    }
    None
}

fn valid_image_dimensions(width: u32, height: u32) -> Option<(u32, u32)> {
    (width > 0 && height > 0 && width <= 100_000 && height <= 100_000).then_some((width, height))
}

fn mobi_metadata(bytes: &[u8]) -> Option<(u32, Option<u32>)> {
    if !has_mobi_header(bytes) {
        return None;
    }
    let record_offset = u32::from_be_bytes([bytes[78], bytes[79], bytes[80], bytes[81]]);
    let header_offset = record_offset as usize + 20;
    let header_length = bytes
        .get(header_offset..header_offset + 4)
        .map(|value| u32::from_be_bytes([value[0], value[1], value[2], value[3]]));
    Some((record_offset, header_length))
}

pub const CONTENT_FORMAT_TEXT: &str = "text";
pub const CONTENT_FORMAT_BLOCKS_V1: &str = "blocks-v1";
const MAX_EMBEDDED_IMAGE_BYTES: usize = 2 * 1024 * 1024;
const MAX_EMBEDDED_IMAGE_TOTAL_BYTES: usize = 8 * 1024 * 1024;
const MAX_EPUB_ARCHIVE_ENTRIES: usize = 2_048;
const MAX_EPUB_ENTRY_BYTES: u64 = 16 * 1024 * 1024;
const MAX_EPUB_UNCOMPRESSED_BYTES: u64 = 64 * 1024 * 1024;
const MAX_TXT_REPLACEMENTS: usize = 32;
const MAX_TXT_REPLACEMENT_FROM_BYTES: usize = 128;
const MAX_TXT_REPLACEMENT_TO_BYTES: usize = 256;

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
    #[serde(default)]
    pub links: Vec<ContentLink>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentLink {
    pub label: String,
    pub href: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_chapter: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentBlock {
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub level: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anchor: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style: Option<String>,
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
    parse_book_bytes_with_options(file_name, bytes, &TxtParseOptions::default())
}

pub fn parse_book_bytes_with_options(
    file_name: &str,
    bytes: &[u8],
    txt_options: &TxtParseOptions,
) -> Result<ParsedBook, ImportError> {
    let extension = Path::new(file_name)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();

    match extension.as_str() {
        "txt" => parse_txt_with_options(bytes, file_name, txt_options),
        "epub" => parse_epub(bytes, file_name),
        _ => Err(ImportError::UnsupportedFormat(extension)),
    }
}

pub fn preview_book_bytes(
    file_name: &str,
    bytes: &[u8],
    txt_options: &TxtParseOptions,
) -> Result<BookImportPreview, ImportError> {
    let parsed = parse_book_bytes_with_options(file_name, bytes, txt_options)?;
    let encoding = if parsed.format == "txt" {
        Some(detect_text_encoding(bytes).to_string())
    } else {
        None
    };
    let mut warnings = Vec::new();
    if parsed.chapters.len() > 10_000 {
        warnings.push("章节数量较多，导入后建议分批阅读".to_string());
    }
    Ok(BookImportPreview {
        title: parsed.title,
        format: parsed.format,
        encoding,
        chapter_count: parsed.chapters.len(),
        first_chapter_title: parsed.chapters.first().map(|chapter| chapter.title.clone()),
        warnings,
    })
}

fn parse_txt(bytes: &[u8], file_name: &str) -> Result<ParsedBook, ImportError> {
    parse_txt_with_options(bytes, file_name, &TxtParseOptions::default())
}

fn parse_txt_with_options(
    bytes: &[u8],
    file_name: &str,
    options: &TxtParseOptions,
) -> Result<ParsedBook, ImportError> {
    let chapters = if options.replacements.is_empty() {
        split_txt_bytes_streaming(bytes, options)?
    } else {
        let text = decode_text(bytes)?;
        split_txt_with_options(text.as_ref(), options)?
    };
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

fn detect_text_encoding(bytes: &[u8]) -> &'static str {
    if bytes.starts_with(&[0xFF, 0xFE]) {
        return "UTF-16LE";
    }
    if bytes.starts_with(&[0xFE, 0xFF]) {
        return "UTF-16BE";
    }
    if bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).is_some() || std::str::from_utf8(bytes).is_ok() {
        "UTF-8"
    } else {
        "GB18030"
    }
}

fn decode_text(bytes: &[u8]) -> Result<Cow<'_, str>, ImportError> {
    if bytes.starts_with(&[0xFF, 0xFE]) {
        return Ok(Cow::Owned(UTF_16LE.decode(&bytes[2..]).0.into_owned()));
    }
    if bytes.starts_with(&[0xFE, 0xFF]) {
        return Ok(Cow::Owned(UTF_16BE.decode(&bytes[2..]).0.into_owned()));
    }

    let bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
    if let Ok(text) = std::str::from_utf8(bytes) {
        return Ok(Cow::Borrowed(text.trim_start_matches('\u{feff}')));
    }

    let (text, _, had_errors) = GB18030.decode(bytes);
    if had_errors {
        return Err(ImportError::TextDecode);
    }
    Ok(Cow::Owned(
        text.into_owned().trim_start_matches('\u{feff}').to_string(),
    ))
}

fn validate_txt_replacements(options: &TxtParseOptions) -> Result<(), ImportError> {
    if options.replacements.len() > MAX_TXT_REPLACEMENTS {
        return Err(ImportError::InvalidTxtOptions(format!(
            "替换规则不能超过 {} 条",
            MAX_TXT_REPLACEMENTS
        )));
    }

    for replacement in &options.replacements {
        if replacement.from.trim().is_empty() {
            return Err(ImportError::InvalidTxtOptions(
                "替换规则的原文本不能为空".to_string(),
            ));
        }
        if replacement.from.len() > MAX_TXT_REPLACEMENT_FROM_BYTES {
            return Err(ImportError::InvalidTxtOptions(format!(
                "替换规则原文本不能超过 {} 字节",
                MAX_TXT_REPLACEMENT_FROM_BYTES
            )));
        }
        if replacement.to.len() > MAX_TXT_REPLACEMENT_TO_BYTES {
            return Err(ImportError::InvalidTxtOptions(format!(
                "替换规则目标文本不能超过 {} 字节",
                MAX_TXT_REPLACEMENT_TO_BYTES
            )));
        }
    }

    Ok(())
}

fn normalize_txt_text(text: &str, options: &TxtParseOptions) -> Result<String, ImportError> {
    validate_txt_replacements(options)?;

    let mut normalized = String::with_capacity(text.len());
    let mut characters = text.chars().peekable();
    while let Some(character) = characters.next() {
        if character == '\r' {
            if characters.peek().is_some_and(|next| *next == '\n') {
                characters.next();
            }
            normalized.push('\n');
        } else if options.normalize_full_width_space && character == '\u{3000}' {
            normalized.push(' ');
        } else {
            normalized.push(character);
        }
    }

    for replacement in &options.replacements {
        normalized = normalized.replace(&replacement.from, &replacement.to);
    }

    Ok(normalized)
}

fn split_txt(text: &str) -> Vec<ParsedChapter> {
    split_txt_with_options(text, &TxtParseOptions::default()).unwrap_or_default()
}

fn compile_txt_chapter_pattern(options: &TxtParseOptions) -> Result<Option<Regex>, ImportError> {
    match options.chapter_rule {
        TxtChapterRule::Regex => {
            let pattern = options
                .custom_pattern
                .as_deref()
                .filter(|value| !value.trim().is_empty())
                .ok_or_else(|| {
                    ImportError::InvalidTxtOptions("自定义章节规则不能为空".to_string())
                })?;
            if pattern.len() > 256 {
                return Err(ImportError::InvalidTxtOptions(
                    "自定义章节规则不能超过 256 字节".to_string(),
                ));
            }
            Ok(Some(Regex::new(pattern).map_err(|error| {
                ImportError::InvalidTxtOptions(format!("自定义章节规则无效：{error}"))
            })?))
        }
        _ => Ok(None),
    }
}

struct StreamingTxtReplacementStage {
    from: String,
    to: String,
    pending: String,
}

impl StreamingTxtReplacementStage {
    fn new(replacement: &TxtReplacement) -> Self {
        Self {
            from: replacement.from.clone(),
            to: replacement.to.clone(),
            pending: String::new(),
        }
    }

    fn push(&mut self, input: &str, output: &mut String) {
        self.pending.push_str(input);
        self.drain(output, false);
    }

    fn finish(&mut self, output: &mut String) {
        self.drain(output, true);
    }

    fn drain(&mut self, output: &mut String, final_chunk: bool) {
        loop {
            if let Some(index) = self.pending.find(&self.from) {
                output.push_str(&self.pending[..index]);
                output.push_str(&self.to);
                self.pending.drain(..index + self.from.len());
                continue;
            }

            if final_chunk {
                output.push_str(&self.pending);
                self.pending.clear();
            } else {
                let keep = self.from.len().saturating_sub(1);
                if self.pending.len() > keep {
                    let mut split = self.pending.len() - keep;
                    while split > 0 && !self.pending.is_char_boundary(split) {
                        split -= 1;
                    }
                    output.push_str(&self.pending[..split]);
                    self.pending.drain(..split);
                }
            }
            break;
        }
    }
}

struct StreamingTxtReplacements {
    stages: Vec<StreamingTxtReplacementStage>,
}

impl StreamingTxtReplacements {
    fn new(replacements: &[TxtReplacement]) -> Self {
        Self {
            stages: replacements
                .iter()
                .map(StreamingTxtReplacementStage::new)
                .collect(),
        }
    }

    fn push(&mut self, input: &str, output: &mut String) {
        let mut carry = input.to_string();
        for stage in &mut self.stages {
            let mut next = String::new();
            stage.push(&carry, &mut next);
            carry = next;
        }
        output.push_str(&carry);
    }

    fn finish(&mut self, output: &mut String) {
        let mut carry = String::new();
        for index in 0..self.stages.len() {
            let mut next = String::new();
            if index == 0 {
                self.stages[index].finish(&mut next);
            } else {
                self.stages[index].push(&carry, &mut next);
                let mut tail = String::new();
                self.stages[index].finish(&mut tail);
                next.push_str(&tail);
            }
            carry = next;
        }
        output.push_str(&carry);
    }
}

fn for_each_text_chunk<F>(text: &str, mut callback: F)
where
    F: FnMut(&str),
{
    const CHUNK_SIZE: usize = 64 * 1024;
    let mut start = 0;
    while start < text.len() {
        let mut end = (start + CHUNK_SIZE).min(text.len());
        while end > start && !text.is_char_boundary(end) {
            end -= 1;
        }
        if end == start {
            end = text[start..]
                .char_indices()
                .nth(1)
                .map_or(text.len(), |(offset, _)| start + offset);
        }
        callback(&text[start..end]);
        start = end;
    }
}

fn normalize_txt_chunk(
    chunk: &str,
    options: &TxtParseOptions,
    pending_cr: &mut bool,
    normalized: &mut String,
) {
    for character in chunk.chars() {
        if *pending_cr {
            normalized.push('\n');
            *pending_cr = false;
            if character == '\n' {
                continue;
            }
        }

        if character == '\r' {
            *pending_cr = true;
        } else if options.normalize_full_width_space && character == '\u{3000}' {
            normalized.push(' ');
        } else {
            normalized.push(character);
        }
    }
}

fn consume_txt_replacement_chunk<F>(chunk: &str, line: &mut String, callback: &mut F)
where
    F: FnMut(&str),
{
    for character in chunk.chars() {
        if character == '\n' {
            if line.ends_with('\r') {
                line.pop();
            }
            callback(line);
            line.clear();
        } else {
            line.push(character);
        }
    }
}

fn consume_replaced_txt_chunk(
    chunk: &str,
    options: &TxtParseOptions,
    custom_pattern: Option<&Regex>,
    line: &mut String,
    chapters: &mut Vec<ParsedChapter>,
    current_title: &mut String,
    current_content: &mut String,
) {
    consume_txt_replacement_chunk(chunk, line, &mut |value| {
        append_txt_line(
            value,
            options,
            custom_pattern,
            chapters,
            current_title,
            current_content,
        );
    });
}

fn split_txt_with_replacements_streaming(
    text: &str,
    options: &TxtParseOptions,
) -> Result<Vec<ParsedChapter>, ImportError> {
    validate_txt_replacements(options)?;
    let custom_pattern = compile_txt_chapter_pattern(options)?;
    let mut replacements = StreamingTxtReplacements::new(&options.replacements);
    let mut normalized_pending_cr = false;
    let mut line = String::new();
    let mut chapters = Vec::new();
    let mut current_title = String::new();
    let mut current_content = String::new();

    for_each_text_chunk(text, |chunk| {
        let mut normalized = String::new();
        normalize_txt_chunk(chunk, options, &mut normalized_pending_cr, &mut normalized);
        if normalized.is_empty() {
            return;
        }

        let mut replaced = String::new();
        replacements.push(&normalized, &mut replaced);
        consume_replaced_txt_chunk(
            &replaced,
            options,
            custom_pattern.as_ref(),
            &mut line,
            &mut chapters,
            &mut current_title,
            &mut current_content,
        );
    });

    if normalized_pending_cr {
        let mut replaced = String::new();
        replacements.push("\n", &mut replaced);
        consume_replaced_txt_chunk(
            &replaced,
            options,
            custom_pattern.as_ref(),
            &mut line,
            &mut chapters,
            &mut current_title,
            &mut current_content,
        );
    }

    let mut replaced = String::new();
    replacements.finish(&mut replaced);
    consume_replaced_txt_chunk(
        &replaced,
        options,
        custom_pattern.as_ref(),
        &mut line,
        &mut chapters,
        &mut current_title,
        &mut current_content,
    );

    if !line.is_empty() {
        append_txt_line(
            &line,
            options,
            custom_pattern.as_ref(),
            &mut chapters,
            &mut current_title,
            &mut current_content,
        );
    }
    if !current_title.is_empty() || !current_content.is_empty() {
        push_text_chapter(&mut chapters, &current_title, &current_content);
    }

    Ok(chapters)
}

fn split_txt_with_options(
    text: &str,
    options: &TxtParseOptions,
) -> Result<Vec<ParsedChapter>, ImportError> {
    let custom_pattern = compile_txt_chapter_pattern(options)?;
    let mut chapters = Vec::new();
    let mut current_title = String::new();
    let mut current_content = String::new();

    if options.replacements.is_empty() {
        for_each_normalized_txt_line(text, options, |line| {
            append_txt_line(
                line,
                options,
                custom_pattern.as_ref(),
                &mut chapters,
                &mut current_title,
                &mut current_content,
            );
        });
    } else {
        return split_txt_with_replacements_streaming(text, options);
    }

    if !current_title.is_empty() || !current_content.is_empty() {
        push_text_chapter(&mut chapters, &current_title, &current_content);
    }

    Ok(chapters)
}

fn split_txt_bytes_streaming(
    bytes: &[u8],
    options: &TxtParseOptions,
) -> Result<Vec<ParsedChapter>, ImportError> {
    let custom_pattern = compile_txt_chapter_pattern(options)?;
    let mut chapters = Vec::new();
    let mut current_title = String::new();
    let mut current_content = String::new();

    for_each_decoded_txt_line(bytes, options, |line| {
        append_txt_line(
            line,
            options,
            custom_pattern.as_ref(),
            &mut chapters,
            &mut current_title,
            &mut current_content,
        );
    })?;

    if !current_title.is_empty() || !current_content.is_empty() {
        push_text_chapter(&mut chapters, &current_title, &current_content);
    }

    Ok(chapters)
}

fn for_each_normalized_txt_line<F>(text: &str, options: &TxtParseOptions, mut callback: F)
where
    F: FnMut(&str),
{
    let mut line = String::new();
    let mut pending_cr = false;
    consume_normalized_txt_chunk(text, options, &mut line, &mut pending_cr, &mut callback);
    finish_normalized_txt_lines(&mut line, &mut pending_cr, &mut callback);
}

fn for_each_decoded_txt_line<F>(
    bytes: &[u8],
    options: &TxtParseOptions,
    mut callback: F,
) -> Result<(), ImportError>
where
    F: FnMut(&str),
{
    let mut line = String::new();
    let mut pending_cr = false;
    let mut bytes = bytes;

    if let Some(stripped) = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
        bytes = stripped;
    }
    if let Ok(text) = std::str::from_utf8(bytes) {
        consume_normalized_txt_chunk(
            text.trim_start_matches('\u{feff}'),
            options,
            &mut line,
            &mut pending_cr,
            &mut callback,
        );
        finish_normalized_txt_lines(&mut line, &mut pending_cr, &mut callback);
        return Ok(());
    }

    let (encoding, encoded) = if bytes.starts_with(&[0xFF, 0xFE]) {
        (UTF_16LE, &bytes[2..])
    } else if bytes.starts_with(&[0xFE, 0xFF]) {
        (UTF_16BE, &bytes[2..])
    } else {
        (GB18030, bytes)
    };
    let mut decoder = encoding.new_decoder_without_bom_handling();
    const CHUNK_SIZE: usize = 64 * 1024;
    let chunk_count = encoded.chunks(CHUNK_SIZE).len();

    for (index, chunk) in encoded.chunks(CHUNK_SIZE).enumerate() {
        let last = index + 1 == chunk_count;
        let capacity = decoder
            .max_utf8_buffer_length(chunk.len())
            .ok_or(ImportError::TextDecode)?
            .max(4);
        let mut decoded = String::with_capacity(capacity);
        let (_, _, had_errors) = decoder.decode_to_string(chunk, &mut decoded, last);
        if had_errors {
            return Err(ImportError::TextDecode);
        }
        consume_normalized_txt_chunk(&decoded, options, &mut line, &mut pending_cr, &mut callback);
    }

    finish_normalized_txt_lines(&mut line, &mut pending_cr, &mut callback);
    Ok(())
}

fn consume_normalized_txt_chunk<F>(
    chunk: &str,
    options: &TxtParseOptions,
    line: &mut String,
    pending_cr: &mut bool,
    callback: &mut F,
) where
    F: FnMut(&str),
{
    for character in chunk.chars() {
        if *pending_cr {
            callback(line);
            line.clear();
            *pending_cr = false;
            if character == '\n' {
                continue;
            }
        }

        if character == '\r' {
            *pending_cr = true;
        } else if character == '\n' {
            callback(line);
            line.clear();
        } else if options.normalize_full_width_space && character == '\u{3000}' {
            line.push(' ');
        } else {
            line.push(character);
        }
    }
}

fn finish_normalized_txt_lines<F>(line: &mut String, pending_cr: &mut bool, callback: &mut F)
where
    F: FnMut(&str),
{
    if *pending_cr {
        callback(line);
        line.clear();
        *pending_cr = false;
    }
    if !line.is_empty() {
        callback(line);
        line.clear();
    }
}

fn append_txt_line(
    line: &str,
    options: &TxtParseOptions,
    custom_pattern: Option<&Regex>,
    chapters: &mut Vec<ParsedChapter>,
    current_title: &mut String,
    current_content: &mut String,
) {
    let trimmed = line.trim();
    let is_chapter = match options.chapter_rule {
        TxtChapterRule::Auto => looks_like_chapter(trimmed),
        TxtChapterRule::Disabled => false,
        TxtChapterRule::Regex => custom_pattern.is_some_and(|pattern| pattern.is_match(trimmed)),
    };
    if is_chapter {
        if !current_title.is_empty() || !current_content.is_empty() {
            push_text_chapter(chapters, current_title, current_content);
            current_content.clear();
        }
        *current_title = trimmed.to_string();
    } else {
        if !current_content.is_empty() {
            current_content.push('\n');
        }
        current_content.push_str(line.trim_end());
    }
}

fn push_text_chapter(chapters: &mut Vec<ParsedChapter>, title: &str, content: &str) {
    let mut offset = 0usize;
    let mut first_offset = None;
    let mut last_offset = 0usize;

    for segment in content.split_inclusive('\n') {
        let line = segment.strip_suffix('\n').unwrap_or(segment);
        if !line.trim().is_empty() {
            if first_offset.is_none() {
                first_offset = Some(offset);
            }
            last_offset = offset + line.len();
        }
        offset += segment.len();
    }

    let Some(first_offset) = first_offset else {
        return;
    };
    let content = content[first_offset..last_offset].to_string();
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
    let line = line.trim();
    if line.is_empty() || line.chars().count() > 80 {
        return false;
    }

    for prefix in [
        "序章", "楔子", "番外", "正文", "后记", "尾声", "终章", "引子", "卷首", "卷末",
    ] {
        if line == prefix
            || line
                .strip_prefix(prefix)
                .is_some_and(|rest| rest.is_empty() || is_title_separator(rest))
        {
            return true;
        }
    }

    let lower = line.to_ascii_lowercase();
    if lower.starts_with("chapter ")
        && lower[8..]
            .chars()
            .next()
            .is_some_and(|character| character.is_ascii_digit())
    {
        return true;
    }

    if !line.starts_with('第') {
        return false;
    }

    let Some((marker_index, _)) = ["章", "节", "回", "卷", "篇"]
        .iter()
        .filter_map(|marker| line.find(marker).map(|index| (index, *marker)))
        .min_by_key(|(index, _)| *index)
    else {
        return false;
    };
    if marker_index > 24 {
        return false;
    }

    let number = &line['第'.len_utf8()..marker_index];
    let compact: String = number
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect();
    !compact.is_empty() && compact.chars().all(is_chapter_number_character)
}

fn is_title_separator(rest: &str) -> bool {
    rest.chars()
        .next()
        .is_some_and(|character| matches!(character, ' ' | '\t' | ':' | '：' | '-' | '—' | '·'))
}

fn is_chapter_number_character(character: char) -> bool {
    character.is_ascii_digit() || "零〇一二两三四五六七八九十百千万".contains(character)
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
    let mut archive = ZipArchive::new(Cursor::new(bytes))
        .map_err(|error| ImportError::InvalidEpub(format!("EPUB ZIP 容器损坏：{error}")))?;
    validate_epub_archive(&mut archive)?;
    let container =
        read_required_zip_text(&mut archive, "META-INF/container.xml", "container.xml")?;
    let opf_path = extract_attribute_from_xml(&container, "rootfile", "full-path")
        .ok_or_else(|| ImportError::InvalidEpub("缺少 OPF 根文件".to_string()))?;
    let opf = read_required_zip_text(&mut archive, &opf_path, "OPF 文件")?;
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

    let mut parsed_chapters: Vec<(String, String, ContentDocument)> = Vec::new();
    for id in spine {
        let Some(item) = manifest.get(&id) else {
            continue;
        };
        if !item.media_type.contains("html") && !item.media_type.contains("xhtml") {
            continue;
        }

        let path = join_zip_path(base_path, &item.href);
        let Ok(html) = read_zip_text(&mut archive, &path) else {
            continue;
        };
        let mut document = parse_html_document(&html);
        resolve_epub_images(&mut document, &path, &image_sources);
        if document.blocks.is_empty() {
            continue;
        }

        let chapter_title = ["h1", "h2", "h3"]
            .iter()
            .find_map(|tag| extract_element_text(&html, tag))
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| format!("第 {} 章", parsed_chapters.len() + 1));
        parsed_chapters.push((path, chapter_title, document));
    }

    if parsed_chapters.is_empty() {
        return Err(ImportError::InvalidEpub("未找到可阅读章节".to_string()));
    }

    let mut chapter_indices = HashMap::new();
    for (index, (path, _, _)) in parsed_chapters.iter().enumerate() {
        chapter_indices.insert(path.clone(), index);
    }

    let mut chapters = Vec::with_capacity(parsed_chapters.len());
    for (path, chapter_title, mut document) in parsed_chapters {
        resolve_epub_link_targets(&mut document, &path, &chapter_indices);
        let content = serde_json::to_string(&document)
            .map_err(|error| ImportError::InvalidEpub(format!("内容块编码失败：{error}")))?;
        chapters.push(ParsedChapter {
            title: chapter_title,
            content,
            content_format: CONTENT_FORMAT_BLOCKS_V1.to_string(),
        });
    }

    Ok(ParsedBook {
        title,
        author,
        format: "epub".to_string(),
        chapters,
    })
}

fn validate_epub_archive<R: Read + Seek>(archive: &mut ZipArchive<R>) -> Result<(), ImportError> {
    if archive.len() > MAX_EPUB_ARCHIVE_ENTRIES {
        return Err(ImportError::InvalidEpub(format!(
            "EPUB 条目数量超过 {} 个上限",
            MAX_EPUB_ARCHIVE_ENTRIES
        )));
    }

    let mut total_uncompressed = 0_u64;
    for index in 0..archive.len() {
        let entry = archive.by_index(index)?;
        let size = entry.size();
        if size > MAX_EPUB_ENTRY_BYTES {
            return Err(ImportError::InvalidEpub(format!(
                "EPUB 单个条目超过 {} MiB 上限",
                MAX_EPUB_ENTRY_BYTES / (1024 * 1024)
            )));
        }
        total_uncompressed = total_uncompressed.saturating_add(size);
        if total_uncompressed > MAX_EPUB_UNCOMPRESSED_BYTES {
            return Err(ImportError::InvalidEpub(format!(
                "EPUB 解压后总大小超过 {} MiB 上限",
                MAX_EPUB_UNCOMPRESSED_BYTES / (1024 * 1024)
            )));
        }
    }
    Ok(())
}

fn is_safe_zip_entry_path(path: &str) -> bool {
    !path.is_empty()
        && !path.starts_with('/')
        && !path.bytes().any(|byte| byte == 0)
        && !path.split('/').any(|part| part == "..")
}

fn read_zip_bytes<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
    path: &str,
) -> Result<Vec<u8>, ImportError> {
    if !is_safe_zip_entry_path(path) {
        return Err(ImportError::InvalidEpub("EPUB 条目路径不安全".to_string()));
    }
    let mut file = archive.by_name(path)?;
    if file.size() > MAX_EPUB_ENTRY_BYTES {
        return Err(ImportError::InvalidEpub(
            "EPUB 单个条目超过大小上限".to_string(),
        ));
    }
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    Ok(bytes)
}

fn read_required_zip_text<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
    path: &str,
    label: &str,
) -> Result<String, ImportError> {
    read_zip_text(archive, path)
        .map_err(|error| ImportError::InvalidEpub(format!("缺少或无法读取 {label}：{error}")))
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

fn safe_epub_internal_href(raw: &str) -> Option<String> {
    let href = decode_entities(raw).trim().to_string();
    if href.is_empty() || href.len() > 512 || href.chars().any(char::is_control) {
        return None;
    }

    let lower = href.to_ascii_lowercase();
    if lower.starts_with("javascript:")
        || lower.starts_with("data:")
        || lower.starts_with("http:")
        || lower.starts_with("https:")
        || lower.starts_with("//")
        || href.starts_with('/')
        || href.contains(':')
    {
        return None;
    }

    let path = href.split('#').next().unwrap_or_default();
    if path.split('/').any(|part| part == "..") {
        return None;
    }

    Some(href)
}

fn extract_epub_internal_links(html: &str) -> Vec<ContentLink> {
    let lower = html.to_ascii_lowercase();
    let bytes = lower.as_bytes();
    let mut links = Vec::new();
    let mut cursor = 0usize;

    while let Some(relative_start) = lower[cursor..].find("<a") {
        let start = cursor + relative_start;
        let boundary = bytes.get(start + 2).copied();
        if !matches!(boundary, Some(b' ' | b'\t' | b'\r' | b'\n' | b'/' | b'>')) {
            cursor = start + 2;
            continue;
        }

        let Some(relative_end) = lower[start..].find('>') else {
            break;
        };
        let open_end = start + relative_end + 1;
        let tag = &html[start..open_end];
        let Some(href) =
            extract_html_attribute(tag, "href").and_then(|value| safe_epub_internal_href(&value))
        else {
            cursor = open_end;
            continue;
        };

        let Some(relative_close) = lower[open_end..].find("</a>") else {
            break;
        };
        let close_start = open_end + relative_close;
        let label = strip_html(&html[open_end..close_start]).trim().to_string();
        if !label.is_empty()
            && !links
                .iter()
                .any(|link: &ContentLink| link.href == href && link.label == label)
        {
            links.push(ContentLink {
                label,
                href,
                target_chapter: None,
            });
        }
        cursor = close_start + "</a>".len();
    }

    links
}

fn resolve_epub_link_targets(
    document: &mut ContentDocument,
    chapter_path: &str,
    chapter_indices: &HashMap<String, usize>,
) {
    let chapter_base = chapter_path
        .rsplit_once('/')
        .map(|(base, _)| base)
        .unwrap_or_default();

    for link in &mut document.links {
        let target_path = link.href.split('#').next().unwrap_or_default();
        let target_path = if target_path.is_empty() {
            chapter_path.to_string()
        } else {
            join_zip_path(chapter_base, target_path)
        };
        link.target_chapter = chapter_indices.get(&target_path).copied();
    }
}

fn safe_epub_anchor_id(raw: &str) -> Option<String> {
    let value = decode_entities(raw).trim().to_string();
    if value.is_empty()
        || value.len() > 128
        || value
            .chars()
            .any(|character| character.is_control() || character.is_whitespace())
        || value
            .chars()
            .any(|character| matches!(character, '/' | '\\' | '"' | '\'' | '<' | '>'))
    {
        return None;
    }

    Some(value)
}

fn safe_epub_inline_style(raw: &str) -> Option<String> {
    let raw = raw.trim();
    if raw.is_empty() || raw.len() > 512 {
        return None;
    }

    let mut declarations = Vec::new();
    for declaration in raw.split(';').take(8) {
        let Some((property, value)) = declaration.split_once(':') else {
            continue;
        };
        let property = property.trim().to_ascii_lowercase();
        let value = value.trim().to_ascii_lowercase();
        if value.is_empty()
            || value.len() > 64
            || value
                .bytes()
                .any(|byte| matches!(byte, b'{' | b'}' | b'<' | b'>' | b'"' | b'\''))
        {
            continue;
        }

        let allowed = match property.as_str() {
            "text-align" => matches!(value.as_str(), "left" | "right" | "center" | "justify"),
            "font-style" => matches!(value.as_str(), "normal" | "italic" | "oblique"),
            "font-weight" => {
                matches!(value.as_str(), "normal" | "bold" | "bolder" | "lighter")
                    || (value.len() == 3
                        && value.chars().all(|character| character.is_ascii_digit()))
            }
            "text-decoration" => {
                matches!(value.as_str(), "none" | "underline" | "line-through")
            }
            _ => false,
        };
        if allowed && declarations.len() < 4 {
            declarations.push(format!("{property}: {value}"));
        }
    }

    (!declarations.is_empty()).then(|| declarations.join("; "))
}

fn parse_html_document(html: &str) -> ContentDocument {
    let chars: Vec<char> = html.chars().collect();
    let mut blocks = Vec::new();
    let mut spans = Vec::new();
    let mut current = String::new();
    let mut pending_space = false;
    let mut quote_depth = 0usize;
    let mut heading_level = None;
    let mut block_anchor: Option<String> = None;
    let mut block_style: Option<String> = None;
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
                    block_anchor.take(),
                    block_style.take(),
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
                    block_anchor.take(),
                    block_style.take(),
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
                block_anchor.take(),
                block_style.take(),
            );
            quote_depth = quote_depth.saturating_add(1);
            block_anchor = extract_html_attribute(&raw_tag, "id")
                .and_then(|value| safe_epub_anchor_id(&value));
            block_style = extract_html_attribute(&raw_tag, "style")
                .and_then(|value| safe_epub_inline_style(&value));
        } else if name == "img" {
            push_html_block(
                &mut blocks,
                &mut spans,
                &mut current,
                &mut pending_space,
                quote_depth,
                heading_level,
                block_anchor.take(),
                block_style.take(),
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
                anchor: None,
                style: None,
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
                block_anchor.take(),
                block_style.take(),
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
                block_anchor.take(),
                block_style.take(),
            );
            heading_level = heading_level_from_tag(&name);
            block_anchor = extract_html_attribute(&raw_tag, "id")
                .and_then(|value| safe_epub_anchor_id(&value));
            block_style = extract_html_attribute(&raw_tag, "style")
                .and_then(|value| safe_epub_inline_style(&value));
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
        block_anchor.take(),
        block_style.take(),
    );

    ContentDocument {
        version: 1,
        blocks,
        links: extract_epub_internal_links(html),
    }
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
    anchor: Option<String>,
    style: Option<String>,
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
        anchor,
        style,
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

    #[cfg(target_os = "linux")]
    fn peak_rss_bytes() -> Option<u64> {
        let status = std::fs::read_to_string("/proc/self/status").ok()?;
        status.lines().find_map(|line| {
            let value = line.strip_prefix("VmHWM:")?.split_whitespace().next()?;
            value
                .parse::<u64>()
                .ok()
                .map(|kilobytes| kilobytes.saturating_mul(1024))
        })
    }

    #[cfg(target_os = "windows")]
    #[allow(non_snake_case)]
    #[repr(C)]
    struct ProcessMemoryCounters {
        cb: u32,
        PageFaultCount: u32,
        PeakWorkingSetSize: usize,
        WorkingSetSize: usize,
        QuotaPeakPagedPoolUsage: usize,
        QuotaPagedPoolUsage: usize,
        QuotaPeakNonPagedPoolUsage: usize,
        QuotaNonPagedPoolUsage: usize,
        PagefileUsage: usize,
        PeakPagefileUsage: usize,
    }

    #[cfg(target_os = "windows")]
    #[link(name = "psapi")]
    unsafe extern "system" {
        fn GetProcessMemoryInfo(
            process: *mut std::ffi::c_void,
            counters: *mut ProcessMemoryCounters,
            size: u32,
        ) -> i32;
    }

    #[cfg(target_os = "windows")]
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetCurrentProcess() -> *mut std::ffi::c_void;
    }

    #[cfg(target_os = "windows")]
    fn peak_rss_bytes() -> Option<u64> {
        let mut counters = ProcessMemoryCounters {
            cb: std::mem::size_of::<ProcessMemoryCounters>() as u32,
            PageFaultCount: 0,
            PeakWorkingSetSize: 0,
            WorkingSetSize: 0,
            QuotaPeakPagedPoolUsage: 0,
            QuotaPagedPoolUsage: 0,
            QuotaPeakNonPagedPoolUsage: 0,
            QuotaNonPagedPoolUsage: 0,
            PagefileUsage: 0,
            PeakPagefileUsage: 0,
        };
        let counter_size = counters.cb;
        let result =
            unsafe { GetProcessMemoryInfo(GetCurrentProcess(), &mut counters, counter_size) };
        (result != 0).then_some(counters.PeakWorkingSetSize as u64)
    }

    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    fn peak_rss_bytes() -> Option<u64> {
        None
    }

    fn tiny_png_fixture() -> Vec<u8> {
        STANDARD
            .decode("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=")
            .expect("fixture PNG should decode")
    }

    fn image_page_fixture(file_name: &str, width: u32, height: u32) -> ImageDocumentPreview {
        ImageDocumentPreview {
            file_name: file_name.to_string(),
            mime: "image/png".to_string(),
            width,
            height,
            color_type: "L8".to_string(),
            decoded_bytes: u64::from(width) * u64::from(height),
        }
    }

    #[test]
    fn previews_bounded_image_sequence_from_local_inputs() {
        let fixture = tiny_png_fixture();
        let sequence = preview_image_sequence_bytes(
            vec![
                ImageSequenceInput {
                    file_name: "001.png".to_string(),
                    bytes: fixture.clone(),
                },
                ImageSequenceInput {
                    file_name: "002.png".to_string(),
                    bytes: fixture,
                },
            ],
            ImageReadingDirection::Rtl,
            ImageSpreadMode::Double,
        )
        .expect("local image inputs should decode");

        assert_eq!(sequence.page_count, 2);
        assert_eq!(sequence.direction, ImageReadingDirection::Rtl);
        assert_eq!(sequence.spread, ImageSpreadMode::Double);
        assert_eq!(sequence.pages[0].file_name, "001.png");
        assert_eq!(sequence.pages[1].index, 1);
    }
    fn temporary_image_cache_root() -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be valid")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "open-reader-image-cache-{}-{nonce}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("temporary cache root should be created");
        root
    }

    #[test]
    fn writes_and_reuses_persistent_image_thumbnail_cache() {
        let root = temporary_image_cache_root();
        let key = "imgseq-v1-01234567";
        let inputs = vec![ImageSequenceInput {
            file_name: "001.png".to_string(),
            bytes: tiny_png_fixture(),
        }];

        let first = cache_image_sequence_files(
            &root,
            key,
            &inputs,
            ImageReadingDirection::Ltr,
            ImageSpreadMode::Single,
            false,
        )
        .expect("first cache write should succeed");
        assert_eq!(first.cache_writes, 1);
        assert_eq!(first.cache_hits, 0);
        assert!(root.join(key).join("page-0000.png").is_file());

        let second = cache_image_sequence_files(
            &root,
            key,
            &inputs,
            ImageReadingDirection::Ltr,
            ImageSpreadMode::Single,
            false,
        )
        .expect("second cache read should succeed");
        assert_eq!(second.cache_writes, 0);
        assert_eq!(second.cache_hits, 1);
        assert!(second.cache_bytes > 0);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn rebuilds_invalidated_thumbnail_cache_entry() {
        let root = temporary_image_cache_root();
        let key = "imgseq-v1-89abcdef";
        let inputs = vec![ImageSequenceInput {
            file_name: "001.png".to_string(),
            bytes: tiny_png_fixture(),
        }];
        cache_image_sequence_files(
            &root,
            key,
            &inputs,
            ImageReadingDirection::Ltr,
            ImageSpreadMode::Single,
            false,
        )
        .expect("initial cache write should succeed");
        let cache_path = root.join(key).join("page-0000.png");
        fs::write(&cache_path, b"corrupt").expect("corrupt entry should be writable");

        let repaired = cache_image_sequence_files(
            &root,
            key,
            &inputs,
            ImageReadingDirection::Ltr,
            ImageSpreadMode::Single,
            false,
        )
        .expect("invalid cache entry should be rebuilt");
        assert_eq!(repaired.cache_hits, 0);
        assert_eq!(repaired.cache_writes, 1);
        assert_eq!(
            fs::read(&cache_path)
                .expect("repaired cache should be readable")
                .get(..PNG_SIGNATURE.len()),
            Some(PNG_SIGNATURE.as_slice())
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn reads_current_and_adjacent_cached_thumbnails() {
        let root = temporary_image_cache_root();
        let key = "imgseq-v1-13572468";
        let inputs = vec![ImageSequenceInput {
            file_name: "001.png".to_string(),
            bytes: tiny_png_fixture(),
        }];
        cache_image_sequence_files(
            &root,
            key,
            &inputs,
            ImageReadingDirection::Ltr,
            ImageSpreadMode::Single,
            false,
        )
        .expect("cache write should succeed");

        let pages =
            read_image_thumbnail_files(&root, key, &[0, 0, 1]).expect("cached pages should read");
        assert_eq!(pages.len(), 1);
        assert_eq!(pages[0].page_index, 0);
        assert_eq!(pages[0].mime, "image/png");
        assert_eq!(pages[0].bytes.get(..PNG_SIGNATURE.len()), Some(PNG_SIGNATURE.as_slice()));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn cancels_image_thumbnail_cache_before_writing() {
        let root = temporary_image_cache_root();
        let key = "imgseq-v1-24681357";
        let inputs = vec![ImageSequenceInput {
            file_name: "001.png".to_string(),
            bytes: tiny_png_fixture(),
        }];
        let cancelled = AtomicBool::new(true);

        let error = cache_image_sequence_files_with_cancel(
            &root,
            key,
            &inputs,
            ImageReadingDirection::Ltr,
            ImageSpreadMode::Single,
            false,
            Some(&cancelled),
        )
        .expect_err("cancelled cache should stop before writing");
        assert!(error.to_string().contains("取消"));
        assert!(!root.join(key).exists());

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn removes_orphaned_thumbnail_temp_files_on_prune() {
        let root = temporary_image_cache_root();
        let cache_dir = root.join("imgseq-v1-abcdef01");
        fs::create_dir_all(&cache_dir).expect("cache directory should be created");
        let temp = cache_dir.join(".page-0000.png.tmp-0-1");
        fs::write(&temp, [1_u8; 4]).expect("orphan temp should be writable");

        let result = prune_image_thumbnail_cache(&root, 0).expect("pruning should succeed");
        assert_eq!(result.cleaned_temp_files, 1);
        assert!(!temp.exists());

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn prunes_image_thumbnail_cache_by_oldest_file_first() {
        let root = temporary_image_cache_root();
        let cache_dir = root.join("imgseq-v1-01234567");
        fs::create_dir_all(&cache_dir).expect("cache directory should be created");
        fs::write(cache_dir.join("page-0000.png"), [1_u8; 8]).expect("first file");
        std::thread::sleep(std::time::Duration::from_millis(5));
        fs::write(cache_dir.join("page-0001.png"), [2_u8; 8]).expect("second file");

        let result = prune_image_thumbnail_cache(&root, 8).expect("pruning should succeed");
        assert!(result.evicted_files >= 1);
        assert!(result.remaining_bytes <= 8);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn builds_bounded_image_sequence_with_direction_and_spread() {
        let sequence = build_image_sequence_preview(
            vec![
                image_page_fixture("001.png", 2, 3),
                image_page_fixture("002.png", 4, 5),
            ],
            ImageReadingDirection::Rtl,
            ImageSpreadMode::Double,
        )
        .expect("valid image sequence should build");

        assert_eq!(sequence.direction, ImageReadingDirection::Rtl);
        assert_eq!(sequence.spread, ImageSpreadMode::Double);
        assert_eq!(sequence.page_count, 2);
        assert_eq!(sequence.total_pixels, 26);
        assert_eq!(sequence.total_decoded_bytes, 26);
        assert_eq!(sequence.pages[0].index, 0);
        assert_eq!(sequence.pages[1].file_name, "002.png");
    }

    #[test]
    fn rejects_image_sequence_over_total_pixel_quota() {
        let error = build_image_sequence_preview(
            vec![
                image_page_fixture("001.png", 10_000, 10_000),
                image_page_fixture("002.png", 10_000, 10_000),
            ],
            ImageReadingDirection::Ltr,
            ImageSpreadMode::Single,
        )
        .expect_err("sequence pixel quota should be enforced");
        assert!(error.to_string().contains("总像素数超过"));
    }

    #[test]
    fn rejects_empty_image_sequence() {
        let error = build_image_sequence_preview(
            Vec::new(),
            ImageReadingDirection::Vertical,
            ImageSpreadMode::LongStrip,
        )
        .expect_err("empty sequence should be rejected");
        assert!(error.to_string().contains("不能为空"));
    }

    #[test]
    fn decodes_tiny_png_with_restricted_preview() {
        let preview =
            preview_image_bytes("cover.png", &tiny_png_fixture()).expect("tiny PNG should decode");
        assert_eq!(preview.file_name, "cover.png");
        assert_eq!(preview.mime, "image/png");
        assert_eq!(preview.width, 1);
        assert_eq!(preview.height, 1);
        assert!(preview.decoded_bytes > 0);
    }

    fn png_crc32(bytes: &[u8]) -> u32 {
        let mut crc = 0xffff_ffff_u32;
        for byte in bytes {
            crc ^= u32::from(*byte);
            for _ in 0..8 {
                crc = if crc & 1 == 1 {
                    (crc >> 1) ^ 0xedb8_8320
                } else {
                    crc >> 1
                };
            }
        }
        !crc
    }

    #[test]
    fn rejects_image_that_exceeds_pixel_quota_before_decode() {
        let mut oversized = tiny_png_fixture();
        oversized[16..20].copy_from_slice(&10_000_u32.to_be_bytes());
        oversized[20..24].copy_from_slice(&10_000_u32.to_be_bytes());
        let ihdr_crc = png_crc32(&oversized[12..29]);
        oversized[29..33].copy_from_slice(&ihdr_crc.to_be_bytes());

        let error = preview_image_bytes("oversized.png", &oversized)
            .expect_err("oversized image should be rejected");
        assert!(error.to_string().contains("像素数超过"));
    }

    #[test]
    fn rejects_truncated_image() {
        let error = preview_image_bytes("broken.png", b"\x89PNG\r\n\x1A\n")
            .expect_err("truncated image should fail");
        assert!(error.to_string().contains("图片"));
    }

    #[test]
    fn probes_format_signatures_without_parsing() {
        let mut mobi = vec![0_u8; 128];
        mobi[78..82].copy_from_slice(&80_u32.to_be_bytes());
        mobi[96..100].copy_from_slice(b"MOBI");
        mobi[100..104].copy_from_slice(&232_u32.to_be_bytes());

        let mobi_probe = probe_book_format("book.mobi", &mobi);
        assert_eq!(mobi_probe.format, BookFormatKind::Mobi);
        assert_eq!(mobi_probe.support, FormatSupport::ProbeOnly);
        assert!(mobi_probe.signature_match);
        assert!(mobi_probe.message.contains("DRM"));
        assert_eq!(
            mobi_probe.metadata,
            Some(FormatProbeMetadata::Mobi {
                record_offset: 80,
                header_length: Some(232),
            })
        );

        let azw_probe = probe_book_format("book.azw3", &mobi);
        assert_eq!(azw_probe.format, BookFormatKind::Azw3);
        assert_eq!(azw_probe.support, FormatSupport::ProbeOnly);

        let pdf_probe = probe_book_format("book.pdf", b"%PDF-1.7");
        assert_eq!(pdf_probe.format, BookFormatKind::Pdf);
        assert_eq!(pdf_probe.support, FormatSupport::ProbeOnly);
        assert_eq!(
            pdf_probe.metadata,
            Some(FormatProbeMetadata::Pdf {
                version: "1.7".to_string(),
            })
        );

        let mut png = b"\x89PNG\r\n\x1A\n".to_vec();
        png.extend_from_slice(&13_u32.to_be_bytes());
        png.extend_from_slice(b"IHDR");
        png.extend_from_slice(&1_u32.to_be_bytes());
        png.extend_from_slice(&2_u32.to_be_bytes());
        png.extend_from_slice(&[8, 6, 0, 0, 0]);
        let image_probe = probe_book_format("cover.png", &png);
        assert_eq!(image_probe.format, BookFormatKind::Image);
        assert_eq!(image_probe.support, FormatSupport::ProbeOnly);
        assert_eq!(
            image_probe.metadata,
            Some(FormatProbeMetadata::Image {
                mime: "image/png".to_string(),
                width: Some(1),
                height: Some(2),
            })
        );

        let txt_probe = probe_book_format("book.txt", "第一章\\n正文".as_bytes());
        assert_eq!(txt_probe.support, FormatSupport::Importable);

        let mismatch = probe_book_format("book.pdf", b"not a pdf");
        assert_eq!(mismatch.support, FormatSupport::SignatureMismatch);

        let renamed_pdf = probe_book_format("book.bin", b"%PDF-1.7");
        assert_eq!(renamed_pdf.format, BookFormatKind::Pdf);
        assert_eq!(renamed_pdf.support, FormatSupport::ProbeOnly);

        let mismatched_txt = probe_book_format("book.txt", b"%PDF-1.7");
        assert_eq!(mismatched_txt.format, BookFormatKind::Txt);
        assert_eq!(mismatched_txt.support, FormatSupport::SignatureMismatch);
    }

    #[test]
    fn reports_peak_rss_on_supported_platforms() {
        #[cfg(any(target_os = "linux", target_os = "windows"))]
        assert!(peak_rss_bytes().is_some());

        #[cfg(not(any(target_os = "linux", target_os = "windows")))]
        assert!(peak_rss_bytes().is_none());
    }

    #[test]
    fn rejects_unsafe_epub_entry_paths() {
        assert!(!is_safe_zip_entry_path("../META-INF/container.xml"));
        assert!(!is_safe_zip_entry_path("/OPS/content.xhtml"));
        assert!(!is_safe_zip_entry_path("OPS/\0content.xhtml"));
        assert!(is_safe_zip_entry_path("OPS/Text/chapter.xhtml"));
    }

    #[test]
    fn accepts_empty_epub_archive_within_limits() {
        let writer = zip::ZipWriter::new(Cursor::new(Vec::<u8>::new()));
        let cursor = writer.finish().expect("empty archive should finish");
        let mut archive =
            ZipArchive::new(Cursor::new(cursor.into_inner())).expect("archive should open");

        validate_epub_archive(&mut archive).expect("empty archive should be within limits");
    }

    fn zip_text_entries(entries: &[(&str, &str)]) -> Vec<u8> {
        use std::io::Write;

        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::<u8>::new()));
        let options = zip::write::SimpleFileOptions::default();
        for (path, content) in entries {
            writer
                .start_file(*path, options)
                .expect("fixture entry should start");
            writer
                .write_all(content.as_bytes())
                .expect("fixture entry should write");
        }
        writer
            .finish()
            .expect("fixture archive should finish")
            .into_inner()
    }

    #[test]
    fn rejects_corrupt_epub_zip_with_recoverable_error() {
        let error =
            parse_book_bytes("broken.epub", b"not a zip").expect_err("broken ZIP must fail");
        assert!(error.to_string().contains("ZIP"));
    }

    #[test]
    fn reports_missing_epub_container_and_opf() {
        let missing_container = parse_book_bytes(
            "missing-container.epub",
            &zip_text_entries(&[("OPS/content.opf", "<package/>")]),
        )
        .expect_err("missing container should fail");
        assert!(missing_container.to_string().contains("container.xml"));

        let missing_opf = parse_book_bytes(
            "missing-opf.epub",
            &zip_text_entries(&[(
                "META-INF/container.xml",
                r#"<container><rootfile full-path="OPS/content.opf"/></container>"#,
            )]),
        )
        .expect_err("missing OPF should fail");
        assert!(missing_opf.to_string().contains("OPF"));
    }

    #[test]
    fn skips_missing_spine_chapters_and_keeps_readable_content() {
        let bytes = zip_text_entries(&[
            (
                "META-INF/container.xml",
                r#"<container><rootfile full-path="OPS/content.opf"/></container>"#,
            ),
            (
                "OPS/content.opf",
                r#"<package><metadata><dc:title>演示书</dc:title></metadata><manifest><item id="missing" href="Text/missing.xhtml" media-type="application/xhtml+xml"/><item id="good" href="Text/good.xhtml" media-type="application/xhtml+xml"/></manifest><spine><itemref idref="missing"/><itemref idref="good"/></spine></package>"#,
            ),
            ("OPS/Text/good.xhtml", "<h1>可读章节</h1><p>正文</p>"),
        ]);

        let book =
            parse_book_bytes("recover.epub", &bytes).expect("readable spine item should survive");
        assert_eq!(book.chapters.len(), 1);
        assert_eq!(book.chapters[0].title, "可读章节");
    }

    #[test]
    fn parses_txt_with_custom_chapter_regex() {
        let options = TxtParseOptions {
            chapter_rule: TxtChapterRule::Regex,
            custom_pattern: Some(r"^卷\d+".to_string()),
            normalize_full_width_space: false,
            replacements: Vec::new(),
        };
        let book = parse_book_bytes_with_options(
            "custom.txt",
            "卷1 开始\n正文\n卷2 继续\n更多".as_bytes(),
            &options,
        )
        .expect("custom TXT rule should parse");

        assert_eq!(book.chapters.len(), 2);
        assert_eq!(book.chapters[0].title, "卷1 开始");
        assert_eq!(book.chapters[1].title, "卷2 继续");
    }

    #[test]
    fn disables_automatic_txt_chapter_detection() {
        let options = TxtParseOptions {
            chapter_rule: TxtChapterRule::Disabled,
            custom_pattern: None,
            normalize_full_width_space: false,
            replacements: Vec::new(),
        };
        let book = parse_book_bytes_with_options(
            "plain.txt",
            "第一章\n正文\n第二章\n更多".as_bytes(),
            &options,
        )
        .expect("disabled TXT rule should parse");

        assert_eq!(book.chapters.len(), 1);
        assert_eq!(book.chapters[0].title, "正文 1");
    }

    #[test]
    fn rejects_invalid_txt_chapter_options() {
        let options = TxtParseOptions {
            chapter_rule: TxtChapterRule::Regex,
            custom_pattern: Some("[".to_string()),
            normalize_full_width_space: false,
            replacements: Vec::new(),
        };
        let error = parse_book_bytes_with_options("bad.txt", "正文".as_bytes(), &options)
            .expect_err("invalid regex should be rejected");
        assert!(error.to_string().contains("自定义章节规则无效"));
    }

    #[test]
    fn detects_extended_chapter_headings() {
        assert!(looks_like_chapter("第一卷 风起"));
        assert!(looks_like_chapter("第2篇 远行"));
        assert!(looks_like_chapter("后记"));
        assert!(looks_like_chapter("Chapter 3"));
        assert!(!looks_like_chapter("第二个章节内容"));
    }

    #[test]
    fn normalizes_full_width_spaces_and_applies_replacements() {
        let options = TxtParseOptions {
            chapter_rule: TxtChapterRule::Auto,
            custom_pattern: None,
            normalize_full_width_space: true,
            replacements: vec![TxtReplacement {
                from: "旧词".to_string(),
                to: "新词".to_string(),
            }],
        };
        let book = parse_book_bytes_with_options(
            "normalized.txt",
            "第一章\n　旧词\n第二章\n旧词".as_bytes(),
            &options,
        )
        .expect("TXT normalization should parse");

        assert_eq!(book.chapters.len(), 2);
        assert_eq!(book.chapters[0].content, " 新词");
        assert_eq!(book.chapters[1].content, "新词");
    }

    #[test]
    fn applies_txt_replacements_across_stream_chunks() {
        let mut input = String::from("第一章\n");
        while input.len() < 64 * 1024 - 2 {
            input.push('a');
        }
        input.push_str("旧词\n尾");

        let options = TxtParseOptions {
            chapter_rule: TxtChapterRule::Auto,
            custom_pattern: None,
            normalize_full_width_space: false,
            replacements: vec![TxtReplacement {
                from: "旧词".to_string(),
                to: "新词".to_string(),
            }],
        };
        let book =
            parse_book_bytes_with_options("chunked-replacements.txt", input.as_bytes(), &options)
                .expect("replacement should cross the streaming chunk boundary");

        assert_eq!(book.chapters.len(), 1);
        assert!(book.chapters[0].content.ends_with("新词\n尾"));
    }

    #[test]
    fn normalizes_mixed_line_endings_in_one_pass() {
        let input = "第一章\r\n首行\r第二行\n\n第二章\n正文内容";
        let normalized = normalize_txt_text(input, &TxtParseOptions::default())
            .expect("mixed line endings should normalize");
        assert_eq!(normalized, "第一章\n首行\n第二行\n\n第二章\n正文内容");
        assert!(looks_like_chapter("第一章"));
        assert!(looks_like_chapter("第二章"));

        let chapters = split_txt_with_options(input, &TxtParseOptions::default())
            .expect("mixed line endings should split");
        assert_eq!(chapters.len(), 2);
        assert_eq!(chapters[0].content, "首行\n第二行");
    }

    #[test]
    fn borrows_valid_utf8_without_decode_copy() {
        let decoded = decode_text("第一章\n正文内容".as_bytes()).expect("UTF-8 should decode");
        assert!(matches!(decoded, Cow::Borrowed(_)));
    }

    #[test]
    fn rejects_empty_txt_replacement_source() {
        let options = TxtParseOptions {
            chapter_rule: TxtChapterRule::Auto,
            custom_pattern: None,
            normalize_full_width_space: false,
            replacements: vec![TxtReplacement {
                from: " ".to_string(),
                to: "新词".to_string(),
            }],
        };
        let error = parse_book_bytes_with_options("bad.txt", "正文".as_bytes(), &options)
            .expect_err("empty replacement should be rejected");
        assert!(error.to_string().contains("替换规则"));
    }

    #[test]
    fn detects_common_chapter_headings() {
        assert!(looks_like_chapter("第一章 初见"));
        assert!(looks_like_chapter("番外：春日"));
        assert!(!looks_like_chapter("这是一个很长很长的普通段落"));
    }

    #[test]
    fn parses_large_txt_fixture_with_line_accumulator() {
        let mut text = String::with_capacity(1024 * 1024);
        for index in 0..512 {
            text.push_str(&format!("第 {index} 章\n"));
            text.push_str("正文内容。\n\n第二段内容。\n");
        }

        let book = parse_book_bytes("large.txt", text.as_bytes()).expect("large TXT should parse");
        assert_eq!(book.chapters.len(), 512);
        assert_eq!(book.chapters[0].title, "第 0 章");
        assert!(book.chapters[511].content.contains("第二段内容"));
    }

    #[test]
    fn records_txt_size_matrix_baseline() {
        for size in [1_usize << 20, 16_usize << 20, 64_usize << 20] {
            let mut text = String::with_capacity(size);
            text.push_str("第一章\n");
            while text.len() < size {
                text.push_str("这是用于 CI 性能基线的连续正文行。\n");
            }

            let started = std::time::Instant::now();
            let book = parse_book_bytes("size-matrix.txt", text.as_bytes())
                .expect("size fixture should parse");
            let elapsed = started.elapsed();
            eprintln!(
                "txt_perf size_bytes={} elapsed_ms={} peak_rss_bytes={:?} chapters={} content_bytes={}",
                size,
                elapsed.as_millis(),
                peak_rss_bytes(),
                book.chapters.len(),
                book.chapters
                    .first()
                    .map_or(0, |chapter| chapter.content.len())
            );
            #[cfg(target_os = "linux")]
            assert!(peak_rss_bytes().is_some());

            assert_eq!(book.chapters.len(), 1);
            assert!(book.chapters[0].content.len() > size / 2);
            assert!(
                elapsed < std::time::Duration::from_secs(20),
                "TXT {} MiB parse exceeded 20 seconds: {:?}",
                size / (1024 * 1024),
                elapsed
            );
        }
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
    fn decodes_utf16_and_gb18030_txt() {
        let input = "第一章\n内容";

        let mut utf16le = vec![0xFF, 0xFE];
        for unit in input.encode_utf16() {
            utf16le.extend_from_slice(&unit.to_le_bytes());
        }
        let le_book = parse_book_bytes("le.txt", &utf16le).expect("UTF-16LE should parse");
        assert_eq!(le_book.chapters[0].content, "内容");

        let mut utf16be = vec![0xFE, 0xFF];
        for unit in input.encode_utf16() {
            utf16be.extend_from_slice(&unit.to_be_bytes());
        }
        let be_book = parse_book_bytes("be.txt", &utf16be).expect("UTF-16BE should parse");
        assert_eq!(be_book.chapters[0].content, "内容");

        let (gb18030, _, had_errors) = GB18030.encode(input);
        assert!(!had_errors);
        let gb_book = parse_book_bytes("gb.txt", gb18030.as_ref()).expect("GB18030 should parse");
        assert_eq!(gb_book.chapters[0].content, "内容");
    }

    #[test]
    fn decodes_txt_across_multibyte_chunk_boundaries() {
        let mut input = String::from("第一章\r\n");
        while input.len() < 128 * 1024 {
            input.push_str("跨边界的中文正文行。\r\n");
        }
        let (encoded, _, had_errors) = GB18030.encode(&input);
        assert!(!had_errors);

        let book = parse_book_bytes("chunked.txt", encoded.as_ref())
            .expect("chunked GB18030 text should parse");
        assert_eq!(book.chapters.len(), 1);
        assert!(book.chapters[0].content.contains("跨边界的中文正文行"));
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
    fn extracts_safe_epub_internal_links() {
        let links = extract_epub_internal_links(
            r##"<p><a href="#toc">目录</a> <a href="chapter-2.xhtml#part">第二章</a>
               <a href="https://example.test/out">外链</a>
               <a href="javascript:alert(1)">脚本</a></p>"##,
        );

        assert_eq!(links.len(), 2);
        assert_eq!(links[0].label, "目录");
        assert_eq!(links[0].href, "#toc");
        assert_eq!(links[1].href, "chapter-2.xhtml#part");
    }

    #[test]
    fn resolves_local_image_sources_and_rejects_external_urls() {
        let mut document = ContentDocument {
            version: 1,
            blocks: vec![
                ContentBlock {
                    kind: "image".to_string(),
                    level: None,
                    anchor: None,
                    style: None,
                    spans: Vec::new(),
                    alt: Some("封面".to_string()),
                    src: Some("../Images/cover.jpg".to_string()),
                },
                ContentBlock {
                    kind: "image".to_string(),
                    level: None,
                    anchor: None,
                    style: None,
                    spans: Vec::new(),
                    alt: None,
                    src: Some("https://example.test/cover.jpg".to_string()),
                },
            ],
            links: Vec::new(),
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
    fn resolves_epub_links_to_readable_chapters() {
        let mut document = ContentDocument {
            version: 1,
            blocks: Vec::new(),
            links: vec![
                ContentLink {
                    label: "本章".to_string(),
                    href: "#intro".to_string(),
                    target_chapter: None,
                },
                ContentLink {
                    label: "下一章".to_string(),
                    href: "chapter-2.xhtml#part".to_string(),
                    target_chapter: None,
                },
            ],
        };
        let mut chapter_indices = HashMap::new();
        chapter_indices.insert("OPS/Text/chapter-1.xhtml".to_string(), 0);
        chapter_indices.insert("OPS/Text/chapter-2.xhtml".to_string(), 1);

        resolve_epub_link_targets(&mut document, "OPS/Text/chapter-1.xhtml", &chapter_indices);

        assert_eq!(document.links[0].target_chapter, Some(0));
        assert_eq!(document.links[1].target_chapter, Some(1));
    }

    #[test]
    fn keeps_only_whitelisted_epub_inline_styles() {
        let document = parse_html_document(
            r##"<p id="body" style="text-align: center; color: red; font-weight: 700">正文</p>"##,
        );

        assert_eq!(
            document.blocks[0].style.as_deref(),
            Some("text-align: center; font-weight: 700")
        );
        assert!(safe_epub_inline_style("position: fixed").is_none());
    }

    #[test]
    fn keeps_safe_epub_block_anchors() {
        let document = parse_html_document(
            r##"<h1 id="intro">标题</h1><p id="body">正文</p><p id="bad value">忽略</p>"##,
        );

        assert_eq!(document.blocks[0].anchor.as_deref(), Some("intro"));
        assert_eq!(document.blocks[1].anchor.as_deref(), Some("body"));
        assert_eq!(document.blocks[2].anchor, None);
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
