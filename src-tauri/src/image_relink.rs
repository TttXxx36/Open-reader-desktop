use crate::image_sequence::{
    modified_at_ns, normalize_relative_image_path, validate_image_root_path,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
};

const MAX_RELINK_FILES: usize = 4096;
const MAX_RELINK_BYTES: u64 = 512 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct RelinkPage {
    pub page_index: i64,
    pub relative_path: String,
    pub file_size: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageRelinkAssignment {
    pub page_index: i64,
    pub old_relative_path: String,
    pub new_relative_path: Option<String>,
    pub status: String,
    pub match_kind: String,
    pub file_size: i64,
    pub modified_at_ns: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageRelinkPreview {
    pub book_id: String,
    pub old_root_path: String,
    pub new_root_path: String,
    pub matched_pages: i64,
    pub missing_pages: i64,
    pub added_files: i64,
    pub changed_pages: i64,
    pub reordered: bool,
    pub assignments: Vec<ImageRelinkAssignment>,
    pub missing_page_indices: Vec<i64>,
    pub added_paths: Vec<String>,
}

#[derive(Debug, Clone)]
struct CandidateFile {
    relative_path: String,
    file_size: i64,
    modified_at_ns: Option<i64>,
}

pub fn preview_relink(
    book_id: &str,
    old_root_path: &str,
    new_root_path: &str,
    pages: &[RelinkPage],
) -> Result<ImageRelinkPreview, String> {
    let old_root_path =
        validate_image_root_path(old_root_path).map_err(|error| format!("旧目录无效：{error}"))?;
    let new_root_path =
        validate_image_root_path(new_root_path).map_err(|error| format!("新目录无效：{error}"))?;
    let candidates = scan_image_root(&new_root_path)?;

    let mut exact_paths = HashMap::new();
    let mut basename_sizes: HashMap<(String, i64), Vec<usize>> = HashMap::new();
    for (index, candidate) in candidates.iter().enumerate() {
        exact_paths.insert(candidate.relative_path.clone(), index);
        let basename = Path::new(&candidate.relative_path)
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_string();
        basename_sizes
            .entry((basename, candidate.file_size))
            .or_default()
            .push(index);
    }

    let mut used = HashSet::new();
    let mut assignments = Vec::with_capacity(pages.len());
    let mut missing_page_indices = Vec::new();
    let mut matched_pages = 0_i64;
    let mut changed_pages = 0_i64;

    for page in pages {
        let mut assignment = None;
        if let Some(&candidate_index) = exact_paths.get(&page.relative_path) {
            if used.insert(candidate_index) {
                let candidate = &candidates[candidate_index];
                let status = if candidate.file_size == page.file_size {
                    matched_pages += 1;
                    "matched"
                } else {
                    changed_pages += 1;
                    "changed"
                };
                assignment = Some(ImageRelinkAssignment {
                    page_index: page.page_index,
                    old_relative_path: page.relative_path.clone(),
                    new_relative_path: Some(candidate.relative_path.clone()),
                    status: status.to_string(),
                    match_kind: "relative".to_string(),
                    file_size: candidate.file_size,
                    modified_at_ns: candidate.modified_at_ns,
                });
            }
        }

        if assignment.is_none() {
            let basename = Path::new(&page.relative_path)
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or_default()
                .to_string();
            if let Some(indices) = basename_sizes.get(&(basename, page.file_size)) {
                if let Some(&candidate_index) = indices.iter().find(|index| !used.contains(index)) {
                    used.insert(candidate_index);
                    let candidate = &candidates[candidate_index];
                    changed_pages += 1;
                    assignment = Some(ImageRelinkAssignment {
                        page_index: page.page_index,
                        old_relative_path: page.relative_path.clone(),
                        new_relative_path: Some(candidate.relative_path.clone()),
                        status: "changed".to_string(),
                        match_kind: "basename_size".to_string(),
                        file_size: candidate.file_size,
                        modified_at_ns: candidate.modified_at_ns,
                    });
                }
            }
        }

        if let Some(assignment) = assignment {
            assignments.push(assignment);
        } else {
            missing_page_indices.push(page.page_index);
            assignments.push(ImageRelinkAssignment {
                page_index: page.page_index,
                old_relative_path: page.relative_path.clone(),
                new_relative_path: None,
                status: "missing".to_string(),
                match_kind: "none".to_string(),
                file_size: 0,
                modified_at_ns: None,
            });
        }
    }

    let added_paths = candidates
        .iter()
        .enumerate()
        .filter_map(|(index, candidate)| {
            (!used.contains(&index)).then(|| candidate.relative_path.clone())
        })
        .collect::<Vec<_>>();
    let assigned_paths = assignments
        .iter()
        .filter_map(|assignment| assignment.new_relative_path.as_deref())
        .collect::<Vec<_>>();
    let mut sorted_paths = assigned_paths.clone();
    sorted_paths.sort_unstable();
    let reordered = assigned_paths.len() > 1 && assigned_paths != sorted_paths;

    Ok(ImageRelinkPreview {
        book_id: book_id.to_string(),
        old_root_path,
        new_root_path,
        matched_pages,
        missing_pages: missing_page_indices.len() as i64,
        added_files: added_paths.len() as i64,
        changed_pages,
        reordered,
        assignments,
        missing_page_indices,
        added_paths,
    })
}

fn scan_image_root(root_path: &str) -> Result<Vec<CandidateFile>, String> {
    let root =
        validate_image_root_path(root_path).map_err(|error| format!("图片根目录无效：{error}"))?;
    let root = PathBuf::from(root);
    if !fs::metadata(&root)
        .map(|metadata| metadata.is_dir())
        .unwrap_or(false)
    {
        return Err("图片根目录不存在或不是目录".to_string());
    }

    let mut directories = vec![root.clone()];
    let mut candidates = Vec::new();
    let mut total_bytes = 0_u64;
    while let Some(directory) = directories.pop() {
        let entries =
            fs::read_dir(&directory).map_err(|error| format!("读取图片目录失败：{error}"))?;
        for entry in entries {
            let entry = entry.map_err(|error| format!("读取图片目录项失败：{error}"))?;
            let file_type = entry
                .file_type()
                .map_err(|error| format!("读取图片目录项类型失败：{error}"))?;
            if file_type.is_symlink() {
                continue;
            }
            let path = entry.path();
            if file_type.is_dir() {
                directories.push(path);
                continue;
            }
            if !file_type.is_file() || !is_supported_image(&path) {
                continue;
            }
            if candidates.len() >= MAX_RELINK_FILES {
                return Err(format!("新目录图片数量超过 {MAX_RELINK_FILES} 页扫描上限"));
            }

            let metadata = entry
                .metadata()
                .map_err(|error| format!("读取图片元数据失败：{error}"))?;
            total_bytes = total_bytes.saturating_add(metadata.len());
            if total_bytes > MAX_RELINK_BYTES {
                return Err(format!(
                    "新目录图片总大小超过 {} MB 扫描上限",
                    MAX_RELINK_BYTES / (1024 * 1024)
                ));
            }

            let relative = path
                .strip_prefix(&root)
                .map_err(|_| "图片目录路径无法归一化".to_string())?
                .to_string_lossy()
                .replace('\\', "/");
            let relative = normalize_relative_image_path(&relative)
                .map_err(|error| format!("图片相对路径无效：{error}"))?;
            candidates.push(CandidateFile {
                relative_path: relative,
                file_size: i64::try_from(metadata.len()).unwrap_or(i64::MAX),
                modified_at_ns: modified_at_ns(&metadata),
            });
        }
    }

    candidates.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    Ok(candidates)
}

fn is_supported_image(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|extension| extension.to_str())
            .map(|extension| extension.to_ascii_lowercase())
            .as_deref(),
        Some("png" | "jpg" | "jpeg" | "gif" | "webp")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn previews_relative_and_basename_matches_without_following_symlinks() {
        let root = std::env::temp_dir().join(format!(
            "open-reader-relink-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        let old_root = root.join("old");
        let new_root = root.join("new");
        fs::create_dir_all(new_root.join("chapter")).expect("new root should exist");
        fs::write(new_root.join("001.png"), b"one").expect("first page should write");
        fs::write(new_root.join("chapter/002.png"), b"two22").expect("moved page should write");
        fs::write(new_root.join("extra.jpg"), b"extra").expect("added page should write");

        let preview = preview_relink(
            "book-1",
            &old_root.to_string_lossy(),
            &new_root.to_string_lossy(),
            &[
                RelinkPage {
                    page_index: 0,
                    relative_path: "001.png".to_string(),
                    file_size: 3,
                },
                RelinkPage {
                    page_index: 1,
                    relative_path: "002.png".to_string(),
                    file_size: 5,
                },
                RelinkPage {
                    page_index: 2,
                    relative_path: "003.png".to_string(),
                    file_size: 5,
                },
            ],
        )
        .expect("relink preview should succeed");

        assert_eq!(preview.matched_pages, 1);
        assert_eq!(preview.changed_pages, 1);
        assert_eq!(preview.missing_pages, 1);
        assert_eq!(preview.added_files, 1);
        assert_eq!(preview.assignments[1].match_kind, "basename_size");
        assert_eq!(preview.assignments[1].status, "changed");
        assert_eq!(preview.assignments[2].status, "missing");

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_missing_or_oversized_roots() {
        let missing = preview_relink("book", "/old", "/definitely/missing", &[]);
        assert!(missing.is_err());
    }
}
