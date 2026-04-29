//! Mind Repo detection runtime.
//!
//! Retains only `detect_repo_root` and `detect_repo_root_with_config` (runtime context).
//! Business logic (load/save manifest, scan, diff, reconcile) has been migrated to
//! `src/service/repo.rs`.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Mind Repo detection
// ---------------------------------------------------------------------------

/// Walk up from `start_dir` looking for `minds.yaml`. Returns the repo root directory.
/// `max_depth` limits upward recursion (safety guard).
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

/// Resolve Mind Repo root from a `--config` flag path (parent of config file).
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
}
