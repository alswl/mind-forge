use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;

use crate::error::{MfError, Result};
use crate::model::manifest::{MindsManifest, ProjectEntry};

// ---------------------------------------------------------------------------
// Mind Repo 检测
// ---------------------------------------------------------------------------

/// 从指定目录开始向上递归查找 minds.yaml，返回 Mind Repo 根目录。
/// max_depth 限制最大向上递归层数。
pub fn detect_repo_root(start_dir: &Path, max_depth: usize) -> Option<PathBuf> {
    let mut current = match start_dir.canonicalize() {
        Ok(p) => p,
        Err(_) => start_dir.to_path_buf(),
    };
    let mut depth = 0usize;
    let mut visited = HashSet::new();
    loop {
        if !visited.insert(current.clone()) {
            return None;
        }
        if current.join("minds.yaml").exists() {
            return Some(current);
        }
        if depth >= max_depth {
            return None;
        }
        match current.parent() {
            Some(parent) => {
                current = parent.to_path_buf();
                depth += 1;
            }
            None => return None,
        }
    }
}

/// 当用户通过 --config 显式指定配置文件路径时，
/// 使用该路径的父目录作为 Mind Repo 根（跳过目录搜索）。
pub fn detect_repo_root_with_config(config_path: &Path) -> Option<PathBuf> {
    if !config_path.exists() {
        return None;
    }
    let path = if config_path.is_dir() {
        config_path.to_path_buf()
    } else {
        config_path.parent()?.to_path_buf()
    };
    if path.join("minds.yaml").exists() {
        Some(path)
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// MindsManifest 管理
// ---------------------------------------------------------------------------

/// 从文件加载 MindsManifest（含 schema_version 校验）
pub fn load_manifest(path: &Path) -> Result<MindsManifest> {
    let content = fs::read_to_string(path).map_err(MfError::Io)?;
    let manifest: MindsManifest =
        serde_yaml::from_str(&content).map_err(|e| MfError::ParseError {
            kind: "yaml".to_string(),
            path: path.to_path_buf(),
            detail: e.to_string(),
        })?;
    validate_schema_version(&manifest, path)?;
    Ok(manifest)
}

/// 原子写入 MindsManifest 到文件
pub fn save_manifest(manifest: &MindsManifest, path: &Path) -> Result<()> {
    let content = serde_yaml::to_string(manifest).map_err(|e| MfError::Internal(e.into()))?;
    let tmp_path = {
        let pid = std::process::id();
        let rand: u64 = {
            let t = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
            t.as_nanos() as u64
        };
        path.with_extension(format!("yaml.tmp.{pid}.{rand}"))
    };
    fs::write(&tmp_path, &content).map_err(MfError::Io)?;
    fs::rename(&tmp_path, path).map_err(MfError::Io)?;
    Ok(())
}

fn validate_schema_version(manifest: &MindsManifest, path: &Path) -> Result<()> {
    let compatible = ["1"];
    if !compatible.contains(&manifest.schema_version.as_str()) {
        return Err(MfError::IncompatibleSchema {
            path: path.to_path_buf(),
            found: manifest.schema_version.clone(),
            expected: compatible.iter().map(|s| s.to_string()).collect(),
        });
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// 文件系统扫描
// ---------------------------------------------------------------------------

/// 扫描结果：文件系统上的一个项目候选
#[derive(Debug, Clone, Serialize)]
pub struct ScannedProject {
    pub name: String,
    pub path: String,
}

/// 扫描 repo_root 下一级子目录，识别含 mind.yaml 的项目
pub fn scan_project_dirs(repo_root: &Path) -> Vec<ScannedProject> {
    let mut projects = Vec::new();
    let entries = match fs::read_dir(repo_root) {
        Ok(e) => e,
        Err(_) => return projects,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        // 权限不足时跳过
        if fs::metadata(&path).is_err() {
            continue;
        }
        if path.join("mind.yaml").exists() {
            let name = entry.file_name().to_string_lossy().to_string();
            let rel_path = format!("./{}", name);
            projects.push(ScannedProject { name, path: rel_path });
        }
    }
    projects
}

// ---------------------------------------------------------------------------
// Diff 计算与 Reconcile
// ---------------------------------------------------------------------------

/// index diff 结果
#[derive(Debug, Clone, Serialize)]
pub struct IndexDiff {
    pub added: Vec<ProjectEntry>,
    pub removed: Vec<ProjectEntry>,
    pub updated: Vec<UpdatedProject>,
}

/// 属性变更的项目
#[derive(Debug, Clone, Serialize)]
pub struct UpdatedProject {
    pub before: ProjectEntry,
    pub after: ProjectEntry,
}

/// 计算当前 manifest 与文件系统扫描结果的差异
pub fn compute_diff(manifest: &MindsManifest, scanned: &[ScannedProject]) -> IndexDiff {
    let now = iso_now();

    let manifest_map: std::collections::HashMap<&str, &ProjectEntry> =
        manifest.projects.iter().map(|p| (p.name.as_str(), p)).collect();

    let scanned_map: std::collections::HashMap<&str, &ScannedProject> =
        scanned.iter().map(|p| (p.name.as_str(), p)).collect();

    let manifest_names: HashSet<&str> = manifest_map.keys().copied().collect();
    let scanned_names: HashSet<&str> = scanned_map.keys().copied().collect();

    // Added: on disk but not in manifest
    let added: Vec<ProjectEntry> = scanned_names
        .difference(&manifest_names)
        .map(|name| {
            let sp = scanned_map[name];
            ProjectEntry {
                name: sp.name.clone(),
                path: sp.path.clone(),
                created_at: now.clone(),
                archived_at: None,
            }
        })
        .collect();

    // Removed: in manifest but not on disk
    let removed: Vec<ProjectEntry> = manifest_names
        .difference(&scanned_names)
        .map(|name| (*manifest_map[name]).clone())
        .collect();

    // Updated: in both but attrs changed
    let updated: Vec<UpdatedProject> = manifest_names
        .intersection(&scanned_names)
        .filter_map(|name| {
            let entry = manifest_map[name];
            let sp = scanned_map[name];
            if entry.path != sp.path {
                let mut after = (*entry).clone();
                after.path = sp.path.clone();
                Some(UpdatedProject { before: (*entry).clone(), after })
            } else {
                None
            }
        })
        .collect();

    IndexDiff { added, removed, updated }
}

/// 应用 diff 到 manifest，返回更新后的 manifest
pub fn reconcile(mut manifest: MindsManifest, diff: IndexDiff) -> MindsManifest {
    // 移除
    let remove_names: HashSet<&str> = diff.removed.iter().map(|p| p.name.as_str()).collect();
    manifest.projects.retain(|p| !remove_names.contains(p.name.as_str()));

    // 更新
    let update_map: std::collections::HashMap<&str, &ProjectEntry> =
        diff.updated.iter().map(|u| (u.after.name.as_str(), &u.after)).collect();
    for p in &mut manifest.projects {
        if let Some(after) = update_map.get(p.name.as_str()) {
            p.path = after.path.clone();
        }
    }

    // 新增
    for added in diff.added {
        manifest.projects.push(added);
    }

    manifest
}

// ---------------------------------------------------------------------------
// IndexDiff 渲染
// ---------------------------------------------------------------------------

/// 将 IndexDiff 渲染为人类可读的文本
pub fn render_diff_text(diff: &IndexDiff) -> String {
    let mut lines = Vec::new();
    if diff.added.is_empty() && diff.removed.is_empty() && diff.updated.is_empty() {
        return "No changes detected.".to_string();
    }
    for p in &diff.added {
        lines.push(format!("+ {}", p.name));
    }
    for p in &diff.removed {
        lines.push(format!("- {}", p.name));
    }
    for u in &diff.updated {
        lines.push(format!("~ {} (path: {} -> {})", u.after.name, u.before.path, u.after.path));
    }
    lines.join("\n")
}

fn iso_now() -> String {
    // 简化的 ISO 8601 时间戳，不含时区
    use std::time::{SystemTime, UNIX_EPOCH};
    let dur = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
    let secs = dur.as_secs();
    // 转换成可读格式
    let days = secs / 86400;
    let time_secs = secs % 86400;
    let hours = time_secs / 3600;
    let minutes = (time_secs % 3600) / 60;
    let seconds = time_secs % 60;

    // 从 1970-01-01 推算日期（简化，仅用于测试和开发）
    let mut y = 1970i64;
    let mut remaining_days = days as i64;
    loop {
        let days_in_year = if is_leap(y) { 366 } else { 365 };
        if remaining_days < days_in_year {
            break;
        }
        remaining_days -= days_in_year;
        y += 1;
    }
    let month_days = if is_leap(y) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    let mut m = 1usize;
    for &md in month_days.iter() {
        if remaining_days < md {
            break;
        }
        remaining_days -= md;
        m += 1;
    }
    let d = remaining_days + 1;
    format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z", y, m, d, hours, minutes, seconds)
}

fn is_leap(y: i64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || (y % 400 == 0)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_detect_repo_root_found() {
        let dir = tempfile::tempdir().unwrap();
        let repo_root = dir.path().join("repo");
        fs::create_dir_all(&repo_root).unwrap();
        fs::write(repo_root.join("minds.yaml"), "").unwrap();
        let sub_dir = repo_root.join("a").join("b");
        fs::create_dir_all(&sub_dir).unwrap();
        assert_eq!(detect_repo_root(&sub_dir, 50), Some(repo_root.canonicalize().unwrap()));
    }

    #[test]
    fn test_detect_repo_root_not_found() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(detect_repo_root(dir.path(), 50), None);
    }

    #[test]
    fn test_detect_repo_root_max_depth() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(detect_repo_root(dir.path(), 0), None);
    }

    #[test]
    fn test_detect_repo_root_with_config_found() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("minds.yaml"), "").unwrap();
        let config_file = dir.path().join("mf.yaml");
        fs::write(&config_file, "").unwrap();
        let result = detect_repo_root_with_config(&config_file);
        assert!(result.is_some());
    }

    #[test]
    fn test_detect_repo_root_with_config_not_found() {
        let dir = tempfile::tempdir().unwrap();
        let config_file = dir.path().join("mf.yaml");
        fs::write(&config_file, "").unwrap();
        assert_eq!(detect_repo_root_with_config(&config_file), None);
    }

    #[test]
    fn test_load_manifest_valid() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("minds.yaml");
        fs::write(&path, "schema_version: '1'\nprojects: []\n").unwrap();
        let manifest = load_manifest(&path).unwrap();
        assert_eq!(manifest.schema_version, "1");
    }

    #[test]
    fn test_load_manifest_incompatible_schema() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("minds.yaml");
        fs::write(&path, "schema_version: '2'\nprojects: []\n").unwrap();
        let result = load_manifest(&path);
        assert!(result.is_err());
        match result.unwrap_err() {
            MfError::IncompatibleSchema { .. } => {}
            _ => panic!("expected IncompatibleSchema error"),
        }
    }

    #[test]
    fn test_save_and_load_manifest() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("minds.yaml");
        let manifest = MindsManifest {
            schema_version: "1".to_string(),
            projects: vec![ProjectEntry {
                name: "test".to_string(),
                path: "./test".to_string(),
                created_at: "2026-04-27T00:00:00Z".to_string(),
                archived_at: None,
            }],
        };
        save_manifest(&manifest, &path).unwrap();
        let loaded = load_manifest(&path).unwrap();
        assert_eq!(loaded.projects.len(), 1);
        assert_eq!(loaded.projects[0].name, "test");
    }

    #[test]
    fn test_load_manifest_parse_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("minds.yaml");
        fs::write(&path, "invalid: yaml: [[[").unwrap();
        let result = load_manifest(&path);
        assert!(result.is_err());
    }

    #[test]
    fn test_scan_project_dirs() {
        let dir = tempfile::tempdir().unwrap();
        // 创建 minds.yaml 标识 repo
        fs::write(dir.path().join("minds.yaml"), "").unwrap();
        // 创建含 mind.yaml 的项目目录
        let p1 = dir.path().join("project-a");
        fs::create_dir_all(&p1).unwrap();
        fs::write(p1.join("mind.yaml"), "").unwrap();
        // 创建不含 mind.yaml 的目录（应被忽略）
        let p2 = dir.path().join("not-a-project");
        fs::create_dir_all(&p2).unwrap();
        // 创建含 mind.yaml 的另一个项目
        let p3 = dir.path().join("project-b");
        fs::create_dir_all(&p3).unwrap();
        fs::write(p3.join("mind.yaml"), "").unwrap();

        let scanned = scan_project_dirs(dir.path());
        let mut names: Vec<&str> = scanned.iter().map(|s| s.name.as_str()).collect();
        names.sort();
        assert_eq!(names, vec!["project-a", "project-b"]);
    }

    #[test]
    fn test_compute_diff_add_remove() {
        let manifest = MindsManifest {
            schema_version: "1".to_string(),
            projects: vec![ProjectEntry {
                name: "old-project".to_string(),
                path: "./old-project".to_string(),
                created_at: "2026-01-01T00:00:00Z".to_string(),
                archived_at: None,
            }],
        };
        let scanned = vec![ScannedProject {
            name: "new-project".to_string(),
            path: "./new-project".to_string(),
        }];
        let diff = compute_diff(&manifest, &scanned);
        assert_eq!(diff.added.len(), 1);
        assert_eq!(diff.added[0].name, "new-project");
        assert_eq!(diff.removed.len(), 1);
        assert_eq!(diff.removed[0].name, "old-project");
    }

    #[test]
    fn test_reconcile_add_remove() {
        let manifest = MindsManifest {
            schema_version: "1".to_string(),
            projects: vec![
                ProjectEntry {
                    name: "keep".to_string(),
                    path: "./keep".to_string(),
                    created_at: "2026-01-01T00:00:00Z".to_string(),
                    archived_at: None,
                },
                ProjectEntry {
                    name: "remove-me".to_string(),
                    path: "./remove-me".to_string(),
                    created_at: "2026-01-01T00:00:00Z".to_string(),
                    archived_at: None,
                },
            ],
        };
        let diff = IndexDiff {
            added: vec![ProjectEntry {
                name: "new".to_string(),
                path: "./new".to_string(),
                created_at: "2026-04-27T00:00:00Z".to_string(),
                archived_at: None,
            }],
            removed: vec![ProjectEntry {
                name: "remove-me".to_string(),
                path: "./remove-me".to_string(),
                created_at: "2026-01-01T00:00:00Z".to_string(),
                archived_at: None,
            }],
            updated: vec![],
        };
        let result = reconcile(manifest, diff);
        let mut names: Vec<&str> = result.projects.iter().map(|p| p.name.as_str()).collect();
        names.sort();
        assert_eq!(names, vec!["keep", "new"]);
    }

    #[test]
    fn test_render_diff_text_no_changes() {
        let diff = IndexDiff { added: vec![], removed: vec![], updated: vec![] };
        assert_eq!(render_diff_text(&diff), "No changes detected.");
    }

    #[test]
    fn test_render_diff_text_with_changes() {
        let diff = IndexDiff {
            added: vec![ProjectEntry {
                name: "new-p".to_string(),
                path: "./new-p".to_string(),
                created_at: "".to_string(),
                archived_at: None,
            }],
            removed: vec![],
            updated: vec![],
        };
        let text = render_diff_text(&diff);
        assert!(text.contains("+ new-p"));
    }

    #[test]
    fn test_create_default() {
        let m = MindsManifest::create_default();
        assert_eq!(m.schema_version, "1");
        assert!(m.projects.is_empty());
    }
}
