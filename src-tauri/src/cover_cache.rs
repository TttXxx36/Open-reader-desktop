use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use thiserror::Error;

pub const MAX_COVER_CACHE_FILE_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_COVER_CACHE_BYTES: u64 = 256 * 1024 * 1024;
const CACHE_KEY_PREFIX: &str = "cover-v1-";

#[derive(Debug, Error)]
pub enum CoverCacheError {
    #[error("封面缓存目录不可用：{0}")]
    Io(#[from] std::io::Error),
    #[error("封面缓存键无效")]
    InvalidKey,
    #[error("封面文件超过 {0} MiB 上限")]
    FileTooLarge(usize),
    #[error("封面缓存总量上限无效")]
    InvalidQuota,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CoverCacheSummary {
    pub entries: usize,
    pub bytes: u64,
    pub removed: usize,
}

pub fn install_cover_cache_file(
    cache_root: &Path,
    cache_key: &str,
    bytes: &[u8],
) -> Result<CoverCacheSummary, CoverCacheError> {
    validate_cache_key(cache_key)?;
    validate_cover_bytes(bytes.len())?;
    fs::create_dir_all(cache_root)?;

    let target = cache_path(cache_root, cache_key);
    let temp = temporary_cache_path(cache_root, cache_key);
    let result = (|| {
        let mut file = fs::File::create(&temp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        match fs::rename(&temp, &target) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                fs::remove_file(&target)?;
                fs::rename(&temp, &target)?;
                Ok(())
            }
            Err(error) => Err(error),
        }
    })();

    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result?;
    prune_cover_cache(cache_root, MAX_COVER_CACHE_BYTES)
}

pub fn read_cover_cache_file(
    cache_root: &Path,
    cache_key: &str,
) -> Result<Vec<u8>, CoverCacheError> {
    validate_cache_key(cache_key)?;
    let path = cache_path(cache_root, cache_key);
    let metadata = fs::metadata(&path)?;
    let length = usize::try_from(metadata.len()).unwrap_or(usize::MAX);
    validate_cover_bytes(length)?;
    Ok(fs::read(path)?)
}

pub fn prune_cover_cache(
    cache_root: &Path,
    max_total_bytes: u64,
) -> Result<CoverCacheSummary, CoverCacheError> {
    if max_total_bytes == 0 {
        return Err(CoverCacheError::InvalidQuota);
    }
    if !cache_root.exists() {
        return Ok(CoverCacheSummary {
            entries: 0,
            bytes: 0,
            removed: 0,
        });
    }

    let mut entries = Vec::new();
    for entry in fs::read_dir(cache_root)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.contains(".tmp-") {
            let _ = fs::remove_file(&path);
            continue;
        }
        if path.extension().and_then(|extension| extension.to_str()) != Some("bin") {
            continue;
        }
        let metadata = entry.metadata()?;
        let modified = metadata.modified().unwrap_or(UNIX_EPOCH);
        entries.push((path, metadata.len(), modified));
    }

    entries.sort_by_key(|(_, _, modified)| *modified);
    let mut bytes = entries.iter().map(|(_, size, _)| *size).sum::<u64>();
    let mut removed = 0;
    let limit = max_total_bytes.min(MAX_COVER_CACHE_BYTES);
    while bytes > limit {
        let Some((path, size, _)) = entries.first() else {
            break;
        };
        fs::remove_file(path)?;
        bytes = bytes.saturating_sub(*size);
        removed += 1;
        entries.remove(0);
    }

    Ok(CoverCacheSummary {
        entries: entries.len(),
        bytes,
        removed,
    })
}

fn validate_cache_key(cache_key: &str) -> Result<(), CoverCacheError> {
    if cache_key.len() > 128
        || !cache_key.starts_with(CACHE_KEY_PREFIX)
        || cache_key
            .bytes()
            .any(|byte| !(byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-'))
    {
        return Err(CoverCacheError::InvalidKey);
    }
    Ok(())
}

fn validate_cover_bytes(length: usize) -> Result<(), CoverCacheError> {
    if length > MAX_COVER_CACHE_FILE_BYTES {
        return Err(CoverCacheError::FileTooLarge(
            MAX_COVER_CACHE_FILE_BYTES / (1024 * 1024),
        ));
    }
    Ok(())
}

fn cache_path(cache_root: &Path, cache_key: &str) -> PathBuf {
    cache_root.join(format!("{cache_key}.bin"))
}

fn temporary_cache_path(cache_root: &Path, cache_key: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    cache_root.join(format!("{cache_key}.tmp-{nonce}"))
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::{
        install_cover_cache_file, prune_cover_cache, read_cover_cache_file, CoverCacheError,
        MAX_COVER_CACHE_FILE_BYTES,
    };

    fn test_root(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("open-reader-cover-cache-{label}-{nonce}"));
        fs::create_dir_all(&path).expect("cache root should create");
        path
    }

    #[test]
    fn installs_and_reads_cache_file_after_atomic_temp_write() {
        let root = test_root("install");
        let key = "cover-v1-abcdef";
        let bytes = b"png-fixture";
        let summary =
            install_cover_cache_file(&root, key, bytes).expect("cover cache should install");
        assert_eq!(summary.entries, 1);
        assert_eq!(
            read_cover_cache_file(&root, key).expect("cover cache should read"),
            bytes
        );
        assert!(!root.join(format!("{key}.tmp-leftover")).exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn prunes_oldest_entries_and_removes_leftover_temp_files() {
        let root = test_root("prune");
        install_cover_cache_file(&root, "cover-v1-first", b"1234").expect("first should install");
        install_cover_cache_file(&root, "cover-v1-second", b"5678").expect("second should install");
        fs::write(root.join("cover-v1-third.tmp-leftover"), b"temporary")
            .expect("temp file should write");

        let summary = prune_cover_cache(&root, 4).expect("cache should prune");
        assert!(summary.removed >= 1);
        assert!(summary.bytes <= 4);
        assert!(!root.join("cover-v1-third.tmp-leftover").exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_path_traversal_keys_and_oversized_files() {
        let root = test_root("reject");
        assert!(matches!(
            read_cover_cache_file(&root, "cover-v1-../escape"),
            Err(CoverCacheError::InvalidKey)
        ));
        let oversized = vec![0_u8; MAX_COVER_CACHE_FILE_BYTES + 1];
        assert!(matches!(
            install_cover_cache_file(&root, "cover-v1-large", &oversized),
            Err(CoverCacheError::FileTooLarge(_))
        ));
        let _ = fs::remove_dir_all(root);
    }
}
