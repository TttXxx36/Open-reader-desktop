use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};
use thiserror::Error;

pub const MAX_IMAGE_RELATIVE_PATH_BYTES: usize = 4 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum ImagePathError {
    #[error("image relative path is empty")]
    Empty,
    #[error("image relative path is too long")]
    TooLong,
    #[error("image relative path must be relative")]
    Absolute,
    #[error("image relative path contains traversal")]
    Traversal,
    #[error("image relative path contains a control character")]
    ControlCharacter,
    #[error("image relative path contains an invalid segment")]
    InvalidSegment,
    #[error("image root path must be absolute")]
    RootNotAbsolute,
}

/// Normalizes the persisted path identity for an image page.
///
/// The database stores only this slash-separated relative form. Absolute paths,
/// drive prefixes, traversal segments, duplicate separators, and control characters
/// are rejected before a page can be persisted. The selected root is stored
/// separately in the library_roots table and is never mixed into a cache key.
pub fn normalize_relative_image_path(value: &str) -> Result<String, ImagePathError> {
    if value.is_empty() {
        return Err(ImagePathError::Empty);
    }
    if value.len() > MAX_IMAGE_RELATIVE_PATH_BYTES {
        return Err(ImagePathError::TooLong);
    }
    if value
        .chars()
        .any(|character| character == '\0' || character.is_control())
    {
        return Err(ImagePathError::ControlCharacter);
    }

    let normalized = value.replace('\\', "/");
    let bytes = normalized.as_bytes();
    if normalized.starts_with('/') || (bytes.len() >= 2 && bytes[1] == b':') {
        return Err(ImagePathError::Absolute);
    }

    let mut segments = Vec::new();
    for segment in normalized.split('/') {
        if segment.is_empty() {
            return Err(ImagePathError::InvalidSegment);
        }
        if segment == "." || segment == ".." {
            return Err(ImagePathError::Traversal);
        }
        if segment.contains(':') {
            return Err(ImagePathError::InvalidSegment);
        }
        segments.push(segment);
    }

    if segments.is_empty() {
        return Err(ImagePathError::Empty);
    }

    Ok(segments.join("/"))
}

/// Validates a user-approved root path without converting it into a cache key.
///
/// Windows drive paths, UNC paths, and POSIX absolute paths are accepted. Relative
/// roots are rejected so page records can never escape the selected root scope.
pub fn validate_image_root_path(value: &str) -> Result<String, ImagePathError> {
    let normalized = value.trim();
    if normalized.is_empty() {
        return Err(ImagePathError::Empty);
    }
    if normalized.len() > MAX_IMAGE_RELATIVE_PATH_BYTES {
        return Err(ImagePathError::TooLong);
    }
    if normalized
        .chars()
        .any(|character| character == '\0' || character.is_control())
    {
        return Err(ImagePathError::ControlCharacter);
    }

    let bytes = normalized.as_bytes();
    let is_absolute = normalized.starts_with('/')
        || normalized.starts_with(r"\\")
        || (bytes.len() >= 2 && bytes[1] == b':');
    if !is_absolute {
        return Err(ImagePathError::RootNotAbsolute);
    }
    Ok(normalized.to_string())
}

/// Resolves a persisted page path below the approved image root.
///
/// The relative path is normalized again at the filesystem boundary so callers
/// cannot accidentally bypass the database validation contract.
pub fn resolve_image_page_path(
    root_path: &str,
    relative_path: &str,
) -> Result<PathBuf, ImagePathError> {
    let root = validate_image_root_path(root_path)?;
    let relative = normalize_relative_image_path(relative_path)?;
    Ok(PathBuf::from(root).join(relative))
}

/// Converts a filesystem modification time to a bounded nanosecond timestamp.
pub fn modified_at_ns(metadata: &fs::Metadata) -> Option<i64> {
    metadata
        .modified()
        .ok()
        .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_nanos().min(i64::MAX as u128) as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_windows_separators_without_losing_order() {
        assert_eq!(
            normalize_relative_image_path(r"chapter\001\page 01.png")
                .expect("relative path should normalize"),
            "chapter/001/page 01.png"
        );
    }

    #[test]
    fn accepts_unicode_and_dots_inside_a_filename() {
        assert_eq!(
            normalize_relative_image_path("卷一/第01.话.png")
                .expect("unicode path should normalize"),
            "卷一/第01.话.png"
        );
    }

    #[test]
    fn rejects_absolute_drive_and_unc_paths() {
        for value in [
            r"C:\books\page.png",
            r"\\server\share\page.png",
            "/books/page.png",
        ] {
            assert_eq!(
                normalize_relative_image_path(value),
                Err(ImagePathError::Absolute)
            );
        }
    }

    #[test]
    fn rejects_traversal_and_ambiguous_separators() {
        for value in [
            "../page.png",
            "chapter/../page.png",
            "./page.png",
            "chapter//page.png",
        ] {
            assert!(matches!(
                normalize_relative_image_path(value),
                Err(ImagePathError::Traversal | ImagePathError::InvalidSegment)
            ));
        }
    }

    #[test]
    fn accepts_absolute_root_paths_and_rejects_relative_roots() {
        for value in [r"C:\books", r"\\server\share", "/home/user/books"] {
            assert_eq!(
                validate_image_root_path(value).expect("absolute root should pass"),
                value
            );
        }
        assert_eq!(
            validate_image_root_path("books"),
            Err(ImagePathError::RootNotAbsolute)
        );
    }

    #[test]
    fn resolves_pages_only_under_the_validated_root() {
        let resolved = resolve_image_page_path("/tmp/books", r"chapter\001.png")
            .expect("relative page should resolve");
        assert_eq!(resolved, PathBuf::from("/tmp/books").join("chapter/001.png"));
        assert!(matches!(
            resolve_image_page_path("/tmp/books", "../outside.png"),
            Err(ImagePathError::Traversal)
        ));
        assert!(matches!(
            resolve_image_page_path("/tmp/books", r"C:\outside.png"),
            Err(ImagePathError::Absolute)
        ));
    }

    #[test]
    fn rejects_control_characters_and_drive_like_segments() {
        assert_eq!(
            normalize_relative_image_path("chapter/\u{0000}page.png"),
            Err(ImagePathError::ControlCharacter)
        );
        assert_eq!(
            normalize_relative_image_path("chapter:ads/page.png"),
            Err(ImagePathError::InvalidSegment)
        );
    }
}
