//! Shared utilities: atomic write, schema version validation.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::{MfError, Result};

/// Atomically write content to a file using write-then-rename.
///
/// Writes to a temporary file (`{path}.yaml.tmp.{pid}.{rand}`) then
/// renames atomically, ensuring partial writes never land at `path`.
pub fn atomic_write(path: &Path, content: &str) -> Result<()> {
    let tmp_path = {
        let pid = std::process::id();
        let rand: u64 = {
            let t = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
            t.as_nanos() as u64
        };
        path.with_extension(format!("yaml.tmp.{pid}.{rand}"))
    };
    fs::write(&tmp_path, content).map_err(MfError::Io)?;
    fs::rename(&tmp_path, path).map_err(MfError::Io)?;
    Ok(())
}

/// Validate that `version` is among the compatible schema versions.
///
/// The compatible set is `["1"]` for the current generation.
pub fn validate_schema_version(version: &str, path: &Path) -> Result<()> {
    let compatible = ["1"];
    if !compatible.contains(&version) {
        return Err(MfError::IncompatibleSchema {
            path: path.to_path_buf(),
            found: version.to_string(),
            expected: compatible.iter().map(|s| s.to_string()).collect(),
        });
    }
    Ok(())
}

/// Canonicalize a path, falling back to the raw path on failure.
///
/// Canonicalization failures are logged as a debug warning so callers
/// are not silently operating on unchecked paths.
pub fn try_canonicalize(path: &Path) -> PathBuf {
    match path.canonicalize() {
        Ok(p) => p,
        Err(e) => {
            tracing::debug!("canonicalize failed for {}: {e}; using raw path", path.display());
            path.to_path_buf()
        }
    }
}

/// Detect the current project name from `cwd` within a `repo_root`.
///
/// Walks up from `cwd` looking for `mind.yaml`, stopping at the `repo_root`
/// boundary. Returns the directory name containing `mind.yaml`, or `None`.
pub fn detect_current_project(repo_root: &Path, cwd: &Path) -> Option<String> {
    let repo = try_canonicalize(repo_root);
    let mut current = try_canonicalize(cwd);

    loop {
        if current.join("mind.yaml").exists() {
            return current.file_name().map(|s| s.to_string_lossy().to_string());
        }
        if current == repo {
            return None;
        }
        match current.parent() {
            Some(parent) => current = parent.to_path_buf(),
            None => return None,
        }
    }
}

/// Sanitize a string to kebab-case for use as a filename.
pub fn to_filename(raw: &str) -> String {
    let sanitized: String = raw
        .to_lowercase()
        .chars()
        .map(|c| match c {
            'a'..='z' | '0'..='9' => c,
            ' ' | '_' | '.' => '-',
            _ => '-',
        })
        .collect();

    let collapsed: String = sanitized
        .chars()
        .fold(String::new(), |mut acc, c| {
            let last_is_dash = acc.ends_with('-');
            if c == '-' && last_is_dash {
                /* skip consecutive dashes */
            } else {
                acc.push(c);
            }
            acc
        })
        .trim_matches('-')
        .to_string();

    if collapsed.is_empty() {
        "untitled".to_string()
    } else {
        collapsed
    }
}

/// Extract the directory name from a path as a string.
pub fn dir_name(path: &Path) -> String {
    path.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default()
}

/// Resolve a project path within a repo root.
///
/// If `project` is `Some(name)`, joins it under `repo_root`.
/// If `None`, detects the current project from `cwd` by walking up for `mind.yaml`.
pub fn resolve_project(repo_root: &Path, project: Option<&str>, cwd: &Path) -> Result<PathBuf> {
    match project {
        Some(name) => Ok(repo_root.join(name)),
        None => {
            detect_current_project(repo_root, cwd)
                .ok_or_else(|| MfError::usage(
                    "could not detect current project; run from a project directory or specify --project",
                    Some("use `mf project list` to see available projects".to_string()),
                ))
                .map(|name| repo_root.join(name))
        }
    }
}
