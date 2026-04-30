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

/// Validate that `name` is a legal Mind Project NAME:
/// - non-empty
/// - kebab-case (a-z, 0-9, hyphen)
/// - no path separators
/// - not `.` or `..`
pub fn validate_project_name(name: &str) -> Result<()> {
    if name.is_empty() {
        return Err(MfError::usage(
            "project name cannot be empty",
            Some("use a kebab-case NAME (e.g. my-project)".to_string()),
        ));
    }
    if name == "." || name == ".." {
        return Err(MfError::usage(
            format!("invalid project name: '{name}'"),
            Some("use a kebab-case NAME (e.g. my-project)".to_string()),
        ));
    }
    if name.contains('/') || name.contains('\\') {
        return Err(MfError::usage(
            format!("invalid project name '{name}': path separators are not allowed"),
            Some("use a kebab-case NAME (e.g. my-project)".to_string()),
        ));
    }
    for c in name.chars() {
        if !matches!(c, 'a'..='z' | '0'..='9' | '-') {
            return Err(MfError::usage(
                format!("invalid project name '{name}': only lowercase alphanumeric and hyphens allowed"),
                Some("use a kebab-case NAME (e.g. my-project)".to_string()),
            ));
        }
    }
    Ok(())
}

/// Canonicalize `target` and verify it lives under `root`.
///
/// If `target` does not exist on disk, canonicalizes the parent and checks
/// the leaf component is a single safe filename (no `/`, `\`, `..`, `.`).
pub fn canonicalize_within(root: &Path, target: &Path) -> Result<PathBuf> {
    let root_canonical = root.canonicalize().map_err(|e| {
        MfError::usage(
            format!("cannot resolve root path '{}': {e}", root.display()),
            Some("try --root <PATH>".to_string()),
        )
    })?;

    if target.exists() {
        let target_canonical = target.canonicalize().map_err(|e| {
            MfError::usage(
                format!("cannot resolve path '{}': {e}", target.display()),
                Some("try --project <NAME> or --root <PATH>".to_string()),
            )
        })?;
        if !target_canonical.starts_with(&root_canonical) {
            return Err(MfError::usage(
                format!(
                    "path '{}' is outside the Mind Repo root '{}'",
                    target.display(),
                    root_canonical.display(),
                ),
                Some("try --project <NAME> or --root <PATH>".to_string()),
            ));
        }
        Ok(target_canonical)
    } else {
        // Target doesn't exist yet — canonicalize parent, validate leaf
        let parent = target.parent().ok_or_else(|| {
            MfError::usage(
                format!("cannot determine parent of '{}'", target.display()),
                None as Option<String>,
            )
        })?;
        let leaf =
            target.file_name().map(|s| s.to_string_lossy().to_string()).ok_or_else(|| {
                MfError::usage(
                    format!("cannot extract filename from '{}'", target.display()),
                    None as Option<String>,
                )
            })?;
        validate_project_name(&leaf)?;

        let parent_canonical = parent.canonicalize().map_err(|e| {
            MfError::usage(
                format!("cannot resolve parent path '{}': {e}", parent.display()),
                Some("try --project <NAME> or --root <PATH>".to_string()),
            )
        })?;

        if !parent_canonical.starts_with(&root_canonical) {
            return Err(MfError::usage(
                format!(
                    "path '{}' is outside the Mind Repo root '{}'",
                    target.display(),
                    root_canonical.display(),
                ),
                Some("try --project <NAME> or --root <PATH>".to_string()),
            ));
        }
        Ok(parent_canonical.join(&leaf))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // --- validate_project_name ---

    #[test]
    fn test_validate_name_empty() {
        let err = validate_project_name("").unwrap_err();
        assert!(err.to_string().contains("empty"));
    }

    #[test]
    fn test_validate_name_dot() {
        assert!(validate_project_name(".").is_err());
    }

    #[test]
    fn test_validate_name_dotdot() {
        assert!(validate_project_name("..").is_err());
    }

    #[test]
    fn test_validate_name_contains_slash() {
        assert!(validate_project_name("foo/bar").is_err());
    }

    #[test]
    fn test_validate_name_contains_backslash() {
        assert!(validate_project_name("foo\\bar").is_err());
    }

    #[test]
    fn test_validate_name_uppercase() {
        assert!(validate_project_name("MyProject").is_err());
    }

    #[test]
    fn test_validate_name_valid_kebab() {
        assert!(validate_project_name("my-project").is_ok());
    }

    #[test]
    fn test_validate_name_valid_alphanumeric() {
        assert!(validate_project_name("project42").is_ok());
    }

    #[test]
    fn test_validate_name_with_underscore() {
        assert!(validate_project_name("my_project").is_err());
    }

    // --- canonicalize_within ---

    #[test]
    fn test_canonicalize_within_existing_target() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("repo");
        std::fs::create_dir_all(&root).unwrap();
        let sub = root.join("subdir");
        std::fs::create_dir_all(&sub).unwrap();

        let result = canonicalize_within(&root, &sub).unwrap();
        assert!(result.starts_with(root.canonicalize().unwrap()));
    }

    #[test]
    fn test_canonicalize_within_non_existent_leaf() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("repo");
        std::fs::create_dir_all(&root).unwrap();
        let target = root.join("new-project");

        let result = canonicalize_within(&root, &target).unwrap();
        assert!(result.starts_with(root.canonicalize().unwrap()));
    }

    #[test]
    fn test_canonicalize_within_escape_via_dotdot() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("repo");
        std::fs::create_dir_all(&root).unwrap();
        let target = root.join("../outside");

        let err = canonicalize_within(&root, &target).unwrap_err();
        assert!(err.to_string().contains("outside"));
    }

    #[test]
    fn test_canonicalize_within_symlink_escape() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("repo");
        std::fs::create_dir_all(&root).unwrap();
        let outside = dir.path().join("escape");
        std::fs::create_dir_all(&outside).unwrap();
        // Symlink inside repo pointing outside
        let link = root.join("evil-link");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&outside, &link).unwrap();

        let result = canonicalize_within(&root, &link);
        // On platforms where symlinks resolve, this should be an error
        // (the symlink target is outside the repo root)
        if let Ok(canonical) = result {
            // If it resolved inside the repo root it's fine
            assert!(canonical.starts_with(root.canonicalize().unwrap()));
        }
    }

    #[test]
    fn test_canonicalize_within_root_not_found() {
        let dir = tempfile::tempdir().unwrap();
        let nonexistent_root = dir.path().join("nonexistent");
        let target = dir.path().join("somewhere");

        let err = canonicalize_within(&nonexistent_root, &target).unwrap_err();
        assert!(err.to_string().contains("cannot resolve root"));
    }

    #[test]
    fn test_canonicalize_within_invalid_leaf_name() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("repo");
        std::fs::create_dir_all(&root).unwrap();
        let target = root.join("../escape");

        let err = canonicalize_within(&root, &target).unwrap_err();
        assert!(err.to_string().contains("invalid") || err.to_string().contains("outside"));
    }
}
