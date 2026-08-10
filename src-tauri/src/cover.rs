use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use url::Url;

pub const MAX_COVER_SOURCE_BYTES: usize = 4096;
pub const MAX_COVER_VALIDATOR_BYTES: usize = 512;

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CoverSourceKind {
    None,
    LocalPath,
    RemoteUrl,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct CanonicalCoverSource {
    pub kind: CoverSourceKind,
    pub value: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CoverSourceError {
    #[error("封面来源不能为空")]
    Empty,
    #[error("封面来源超过 {0} 字节")]
    TooLong(usize),
    #[error("本地封面路径必须是绝对路径")]
    RelativePath,
    #[error("本地封面路径包含不允许的片段")]
    UnsafePath,
    #[error("远程封面只允许 HTTPS")]
    UnsupportedScheme,
    #[error("远程封面 URL 不能包含用户名或密码")]
    CredentialsNotAllowed,
    #[error("远程封面 URL 无效")]
    InvalidUrl,
    #[error("封面校验信息超过 {0} 字节")]
    ValidatorTooLong(usize),
}

pub fn normalize_cover_source(
    kind: CoverSourceKind,
    value: &str,
) -> Result<CanonicalCoverSource, CoverSourceError> {
    match kind {
        CoverSourceKind::None => Ok(CanonicalCoverSource {
            kind,
            value: String::new(),
        }),
        CoverSourceKind::LocalPath => Ok(CanonicalCoverSource {
            kind,
            value: normalize_local_path(value)?,
        }),
        CoverSourceKind::RemoteUrl => Ok(CanonicalCoverSource {
            kind,
            value: normalize_remote_url(value)?,
        }),
    }
}

pub fn cover_cache_key(
    source: &CanonicalCoverSource,
    validator: Option<&str>,
) -> Result<String, CoverSourceError> {
    let validator = validator.unwrap_or_default().trim();
    if validator.len() > MAX_COVER_VALIDATOR_BYTES {
        return Err(CoverSourceError::ValidatorTooLong(
            MAX_COVER_VALIDATOR_BYTES,
        ));
    }

    let kind = match source.kind {
        CoverSourceKind::None => "none",
        CoverSourceKind::LocalPath => "local_path",
        CoverSourceKind::RemoteUrl => "remote_url",
    };
    let payload = format!("cover-v1\n{kind}\n{}\n{validator}", source.value);
    let digest = Sha256::digest(payload.as_bytes());
    Ok(format!("cover-v1-{}", hex_lower(&digest)))
}

fn normalize_local_path(value: &str) -> Result<String, CoverSourceError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(CoverSourceError::Empty);
    }
    if value.len() > MAX_COVER_SOURCE_BYTES {
        return Err(CoverSourceError::TooLong(MAX_COVER_SOURCE_BYTES));
    }
    if value.chars().any(|character| character.is_control()) {
        return Err(CoverSourceError::UnsafePath);
    }

    let value = value.replace('\\', "/");
    let windows_drive = value.len() >= 3
        && value.as_bytes()[0].is_ascii_alphabetic()
        && value.as_bytes()[1] == b':'
        && value.as_bytes()[2] == b'/';
    let unc = value.starts_with("//");
    if !windows_drive && !unc && !value.starts_with('/') {
        return Err(CoverSourceError::RelativePath);
    }

    let mut segments = Vec::new();
    for segment in value.split('/') {
        if segment.is_empty() || segment == "." {
            continue;
        }
        if segment == ".."
            || (segment.contains(':')
                && !(segments.is_empty() && windows_drive && segment.len() == 2))
        {
            return Err(CoverSourceError::UnsafePath);
        }
        segments.push(segment);
    }

    if windows_drive {
        if segments.is_empty() {
            return Err(CoverSourceError::UnsafePath);
        }
        let mut normalized = String::with_capacity(value.len());
        normalized.push(segments[0].chars().next().unwrap().to_ascii_uppercase());
        normalized.push(':');
        for segment in segments.iter().skip(1) {
            normalized.push('/');
            normalized.push_str(segment);
        }
        return Ok(normalized);
    }

    if unc {
        if segments.len() < 2 {
            return Err(CoverSourceError::UnsafePath);
        }
        return Ok(format!("//{}", segments.join("/")));
    }

    Ok(format!("/{}", segments.join("/")))
}

fn normalize_remote_url(value: &str) -> Result<String, CoverSourceError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(CoverSourceError::Empty);
    }
    if value.len() > MAX_COVER_SOURCE_BYTES {
        return Err(CoverSourceError::TooLong(MAX_COVER_SOURCE_BYTES));
    }
    let mut url = Url::parse(value).map_err(|_| CoverSourceError::InvalidUrl)?;
    if url.scheme() != "https" {
        return Err(CoverSourceError::UnsupportedScheme);
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err(CoverSourceError::CredentialsNotAllowed);
    }
    url.set_fragment(None);
    Ok(url.to_string())
}

fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

#[cfg(test)]
mod tests {
    use super::{
        cover_cache_key, normalize_cover_source, CanonicalCoverSource, CoverSourceError,
        CoverSourceKind,
    };

    #[test]
    fn normalizes_windows_and_unc_paths_without_touching_filesystem() {
        let windows =
            normalize_cover_source(CoverSourceKind::LocalPath, r"C:\\Books\\.\\cover.png")
                .expect("windows path should normalize");
        assert_eq!(windows.value, "C:/Books/cover.png");

        let unc = normalize_cover_source(CoverSourceKind::LocalPath, r"\\server\\share\\cover.png")
            .expect("UNC path should normalize");
        assert_eq!(unc.value, "//server/share/cover.png");
    }

    #[test]
    fn rejects_relative_traversal_and_ads_paths() {
        assert_eq!(
            normalize_cover_source(CoverSourceKind::LocalPath, "covers/cover.png"),
            Err(CoverSourceError::RelativePath)
        );
        assert_eq!(
            normalize_cover_source(CoverSourceKind::LocalPath, r"C:\\Books\\..\\cover.png"),
            Err(CoverSourceError::UnsafePath)
        );
        assert_eq!(
            normalize_cover_source(CoverSourceKind::LocalPath, r"C:\\Books\\cover.png:secret"),
            Err(CoverSourceError::UnsafePath)
        );
    }

    #[test]
    fn accepts_https_without_credentials_and_rejects_unsafe_urls() {
        let source = normalize_cover_source(
            CoverSourceKind::RemoteUrl,
            "https://Example.com/cover.png#fragment",
        )
        .expect("https should normalize");
        assert_eq!(source.value, "https://Example.com/cover.png");

        assert_eq!(
            normalize_cover_source(CoverSourceKind::RemoteUrl, "http://example.com/cover.png"),
            Err(CoverSourceError::UnsupportedScheme)
        );
        assert_eq!(
            normalize_cover_source(
                CoverSourceKind::RemoteUrl,
                "https://user:password@example.com/cover.png"
            ),
            Err(CoverSourceError::CredentialsNotAllowed)
        );
    }

    #[test]
    fn cache_key_is_versioned_and_deterministic() {
        let source = CanonicalCoverSource {
            kind: CoverSourceKind::LocalPath,
            value: "C:/Books/cover.png".to_string(),
        };
        let first =
            cover_cache_key(&source, Some("size=123;mtime=456")).expect("cache key should build");
        let second =
            cover_cache_key(&source, Some("size=123;mtime=456")).expect("cache key should repeat");
        let changed = cover_cache_key(&source, Some("size=124;mtime=456"))
            .expect("changed validator should build");
        assert_eq!(first, second);
        assert_ne!(first, changed);
        assert!(first.starts_with("cover-v1-"));
        assert!(!first.contains("Books"));
    }

    #[test]
    fn none_source_has_empty_value_and_stable_key() {
        let source = normalize_cover_source(CoverSourceKind::None, "ignored")
            .expect("none source should normalize");
        assert_eq!(source.value, "");
        assert_eq!(
            cover_cache_key(&source, None).expect("none cache key should build"),
            cover_cache_key(
                &CanonicalCoverSource {
                    kind: CoverSourceKind::None,
                    value: String::new(),
                },
                Some("")
            )
            .expect("none cache key should repeat")
        );
    }
}
