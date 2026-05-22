//! Repo service: manifest load/save, file-system scan, diff, reconcile, and diff rendering.
//!
//! Migrated from `src/runtime/repo.rs` (003). The runtime module now retains only
//! `detect_repo_root` and `detect_repo_root_with_config`.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use chrono::Utc;
use serde::Serialize;

use crate::error::{MfError, Result};
use crate::model::manifest::{default_projects_dir, MindsManifest, ProjectEntry};
use crate::runtime::repo::detect_repo_root;
use crate::service::util;

/// Build a repo-root-relative project path from a `projects_dir` and project name.
///
/// `projects_dir == "."` (or empty) yields `./<name>` (flat layout); otherwise
/// `./<projects_dir>/<name>`. Trailing/leading slashes on `projects_dir` are tolerated.
pub fn project_relpath(projects_dir: &str, name: &str) -> String {
    let trimmed = projects_dir.trim_matches('/');
    if trimmed.is_empty() || trimmed == "." {
        format!("./{name}")
    } else {
        format!("./{trimmed}/{name}")
    }
}

/// Read `projects_dir` from `<repo_root>/minds.yaml`, falling back to default
/// when the file is missing or empty.
pub fn projects_dir_for(repo_root: &Path) -> Result<String> {
    let minds_path = repo_root.join("minds.yaml");
    if !minds_path.exists() {
        return Ok(default_projects_dir());
    }
    Ok(load_manifest(&minds_path)?.projects_dir)
}

// ---------------------------------------------------------------------------
// MindsManifest management
// ---------------------------------------------------------------------------

/// Load `MindsManifest` from file with schema version validation.
///
/// An empty (or whitespace-only) file is treated as a fresh repo and yields
/// the default manifest, matching the convention that `minds.yaml` only needs
/// to exist to mark a repo root.
///
/// **Compatibility**: When `projects` entries are plain path strings (the
/// Python `mind` 0.3.0 shape), they are resolved into `ProjectEntry` values
/// using the manifest's `projects_dir`.
pub fn load_manifest(path: &Path) -> Result<MindsManifest> {
    let content = fs::read_to_string(path).map_err(MfError::Io)?;
    if content.trim().is_empty() {
        return Ok(MindsManifest::create_default());
    }
    let mut manifest: MindsManifest = serde_yaml::from_str(&content).map_err(|e| MfError::ParseError {
        kind: "yaml".to_string(),
        path: path.to_path_buf(),
        detail: e.to_string(),
    })?;
    util::validate_schema_version(&manifest.schema_version, path)?;

    // Resolve path-string project entries. Bare names stay compatible with the
    // legacy projects_dir default; path strings remain repo-relative paths.
    let pd = &manifest.projects_dir;
    for entry in &mut manifest.projects {
        if entry.created_at.is_empty() {
            let raw_path = entry.path.clone();
            entry.name = project_name_from_relpath(&raw_path);
            entry.path = if raw_path.contains('/') || raw_path.starts_with('.') {
                normalize_manifest_path(&raw_path)
            } else {
                project_relpath(pd, &raw_path)
            };
        }
    }

    Ok(manifest)
}

/// Atomically write `MindsManifest` to a file (write-then-rename).
pub fn save_manifest(manifest: &MindsManifest, path: &Path) -> Result<()> {
    let content = serialize_mind_manifest(manifest).map_err(|e| MfError::Internal(e.into()))?;
    util::atomic_write(path, &content)
}

fn serialize_mind_manifest(manifest: &MindsManifest) -> std::result::Result<String, serde_yaml::Error> {
    let mut map = serde_yaml::Mapping::new();
    map.insert(
        serde_yaml::Value::String("schema".to_string()),
        serde_yaml::Value::String(manifest.schema_version.clone()),
    );
    map.insert(
        serde_yaml::Value::String("projects_dir".to_string()),
        serde_yaml::Value::String(manifest.projects_dir.clone()),
    );
    let projects = manifest
        .projects
        .iter()
        .map(|project| serde_yaml::Value::String(project_path_for_mind_manifest(&project.path)))
        .collect();
    map.insert(serde_yaml::Value::String("projects".to_string()), serde_yaml::Value::Sequence(projects));
    serde_yaml::to_string(&serde_yaml::Value::Mapping(map))
}

pub fn project_path_for(repo_root: &Path, name: &str) -> Result<Option<PathBuf>> {
    let minds_path = repo_root.join("minds.yaml");
    if !minds_path.exists() {
        return Ok(None);
    }
    let manifest = load_manifest(&minds_path)?;
    for project in &manifest.projects {
        let stripped = strip_dot_prefix(&project.path);
        if project.name == name || project_name_from_relpath(&project.path) == name || stripped == name {
            return Ok(Some(repo_root.join(stripped)));
        }
    }
    Ok(None)
}

fn normalize_manifest_path(path: &str) -> String {
    let stripped = strip_dot_prefix(path).trim_matches('/');
    format!("./{stripped}")
}

fn strip_dot_prefix(path: &str) -> &str {
    path.strip_prefix("./").unwrap_or(path)
}

fn project_path_for_mind_manifest(path: &str) -> String {
    let stripped = strip_dot_prefix(path);
    if path.starts_with("./") && !stripped.contains('/') {
        path.to_string()
    } else {
        stripped.to_string()
    }
}

fn project_name_from_relpath(path: &str) -> String {
    Path::new(strip_dot_prefix(path))
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string())
}

// ---------------------------------------------------------------------------
// Filesystem scan
// ---------------------------------------------------------------------------

/// A project candidate discovered on the filesystem.
#[derive(Debug, Clone, Serialize)]
pub struct ScannedProject {
    pub name: String,
    pub path: String,
}

/// Scan immediate subdirectories of `<repo_root>/<projects_dir>` for those containing
/// `mind.yaml`. `projects_dir == "."` scans the repo root directly (flat layout).
///
/// Returned `ScannedProject.path` is repo-root-relative (e.g. `"./projects/foo"`).
pub fn scan_project_dirs(repo_root: &Path, projects_dir: &str) -> Vec<ScannedProject> {
    let mut projects = Vec::new();
    let trimmed = projects_dir.trim_matches('/');
    let scan_root =
        if trimmed.is_empty() || trimmed == "." { repo_root.to_path_buf() } else { repo_root.join(trimmed) };
    let entries = match fs::read_dir(&scan_root) {
        Ok(e) => e,
        Err(e) => {
            tracing::debug!("scan_project_dirs: cannot read {scan_root:?}: {e}");
            return projects;
        }
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        if fs::metadata(&path).is_err() {
            continue;
        }
        if path.join("mind.yaml").exists() {
            let name = entry.file_name().to_string_lossy().to_string();
            let rel_path = project_relpath(projects_dir, &name);
            projects.push(ScannedProject { name, path: rel_path });
        }
    }
    projects
}

// ---------------------------------------------------------------------------
// Diff computation and reconciliation
// ---------------------------------------------------------------------------

/// Result of comparing manifest entries against filesystem scan results.
#[derive(Debug, Clone, Serialize)]
pub struct IndexDiff {
    pub added: Vec<ProjectEntry>,
    pub removed: Vec<ProjectEntry>,
    pub updated: Vec<UpdatedProject>,
}

/// A project whose attributes have changed.
#[derive(Debug, Clone, Serialize)]
pub struct UpdatedProject {
    pub before: ProjectEntry,
    pub after: ProjectEntry,
}

/// Compute the diff between the current manifest and a filesystem scan.
pub fn compute_diff(manifest: &MindsManifest, scanned: &[ScannedProject]) -> IndexDiff {
    let now = iso_now();

    let manifest_map: std::collections::HashMap<&str, &ProjectEntry> =
        manifest.projects.iter().map(|p| (p.name.as_str(), p)).collect();

    let scanned_map: std::collections::HashMap<&str, &ScannedProject> =
        scanned.iter().map(|p| (p.name.as_str(), p)).collect();

    let manifest_names: HashSet<&str> = manifest_map.keys().copied().collect();
    let scanned_names: HashSet<&str> = scanned_map.keys().copied().collect();

    let added: Vec<ProjectEntry> = scanned_names
        .difference(&manifest_names)
        .map(|name| {
            let sp = scanned_map[name];
            ProjectEntry { name: sp.name.clone(), path: sp.path.clone(), created_at: now.clone(), archived_at: None }
        })
        .collect();

    let removed: Vec<ProjectEntry> =
        manifest_names.difference(&scanned_names).map(|name| (*manifest_map[name]).clone()).collect();

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

/// Apply a diff to a manifest, returning the updated manifest.
pub fn reconcile(mut manifest: MindsManifest, diff: IndexDiff) -> MindsManifest {
    let remove_names: HashSet<&str> = diff.removed.iter().map(|p| p.name.as_str()).collect();
    manifest.projects.retain(|p| !remove_names.contains(p.name.as_str()));

    let update_map: std::collections::HashMap<&str, &ProjectEntry> =
        diff.updated.iter().map(|u| (u.after.name.as_str(), &u.after)).collect();
    for p in &mut manifest.projects {
        if let Some(after) = update_map.get(p.name.as_str()) {
            p.path = after.path.clone();
        }
    }

    for added in diff.added {
        manifest.projects.push(added);
    }

    manifest
}

// ---------------------------------------------------------------------------
// Diff rendering
// ---------------------------------------------------------------------------

/// Render an `IndexDiff` as human-readable text.
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
    Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

// ---------------------------------------------------------------------------
// Repo lifecycle: init command
// ---------------------------------------------------------------------------

/// Serializable result returned by a successful `mf init` invocation.
#[derive(Debug, Clone, Serialize)]
pub struct RepoInitReport {
    /// Display path of the initialized repository.
    pub path: String,
    /// True when this invocation created a new Mind Repo.
    pub created: bool,
    /// True when a valid repo already existed and nothing was rewritten.
    pub already_existed: bool,
    /// Repo-relative files created (sorted).
    pub created_files: Vec<String>,
    /// Repo-relative directories created (sorted).
    pub created_directories: Vec<String>,
    /// Resources intentionally left unchanged (sorted).
    pub skipped: Vec<String>,
}

/// Classification of a target path supplied to `mf init`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RepoTargetKind {
    /// Target exists as an empty directory (no unsafe content).
    ExistingEmptyDirectory,
    /// Target does not exist and has a valid parent directory.
    NewDirectory,
    /// Target already contains a valid, compatible `minds.yaml`.
    ExistingRepo,
    /// Target contains `minds.yaml` that cannot be loaded (parse or schema error).
    MalformedManifest,
    /// Target is a regular file, not a directory.
    InvalidFileTarget,
    /// Target path is invalid (no extractable leaf, e.g. `..` or `foo/..`).
    InvalidPath,
    /// Target exists as a non-empty directory without a compatible `minds.yaml`.
    UnsafeNonEmptyDirectory,
}

/// Classify a target path for `mf init` purposes. Does not modify the filesystem.
pub fn classify_repo_target(target: &Path) -> Result<RepoTargetKind> {
    // `.` is the only special form without a `file_name()` that we accept;
    // anything else without a real leaf (`..`, `foo/..`, etc.) is rejected.
    if target != Path::new(".") && target.file_name().is_none() {
        return Ok(RepoTargetKind::InvalidPath);
    }

    if !target.exists() {
        let parent = parent_or_cwd(target);
        if !parent.exists() {
            return Err(MfError::usage(
                format!("parent directory '{}' does not exist", parent.display()),
                Some("create the parent directory first, or choose a different path".to_string()),
            ));
        }
        if !parent.is_dir() {
            return Err(MfError::usage(format!("parent path '{}' is not a directory", parent.display()), None));
        }
        return Ok(RepoTargetKind::NewDirectory);
    }

    if target.is_file() {
        return Ok(RepoTargetKind::InvalidFileTarget);
    }
    if !target.is_dir() {
        return Ok(RepoTargetKind::InvalidPath);
    }

    let minds_path = target.join("minds.yaml");
    if minds_path.exists() {
        return match load_manifest(&minds_path) {
            Ok(_) => Ok(RepoTargetKind::ExistingRepo),
            Err(MfError::ParseError { .. } | MfError::IncompatibleSchema { .. }) => {
                Ok(RepoTargetKind::MalformedManifest)
            }
            Err(e) => Err(e),
        };
    }

    if is_directory_empty(target)? {
        Ok(RepoTargetKind::ExistingEmptyDirectory)
    } else {
        Ok(RepoTargetKind::UnsafeNonEmptyDirectory)
    }
}

/// Resolve `path.parent()` to a non-empty path, falling back to `.` for
/// bare leaf paths like `Path::new("foo")` whose parent is the empty path.
fn parent_or_cwd(path: &Path) -> &Path {
    let parent = path.parent().unwrap_or(Path::new("."));
    if parent.as_os_str().is_empty() {
        Path::new(".")
    } else {
        parent
    }
}

fn is_directory_empty(path: &Path) -> Result<bool> {
    let mut entries = fs::read_dir(path).map_err(MfError::Io)?;
    Ok(entries.next().is_none())
}

/// Refuse initialization inside an existing Mind Repo (nested repo guard).
///
/// Walks up from `target`'s parent looking for `minds.yaml`.
pub fn validate_not_nested(target: &Path) -> Result<()> {
    if let Some(root) = detect_repo_root(parent_or_cwd(target), 50) {
        return Err(MfError::usage(
            format!("cannot initialize '{}' inside an existing Mind Repo at '{}'", target.display(), root.display()),
            Some("use 'mf project new' to create a project inside a Mind Repo".to_string()),
        ));
    }
    Ok(())
}

/// Run the repo initialization sequence against a target directory.
///
/// Writes `minds.yaml` atomically and creates the projects container.
/// Handles new directory creation with cleanup on failure, and idempotent
/// already-repo.
pub fn init_repo(target: &Path, target_kind: &RepoTargetKind) -> Result<RepoInitReport> {
    match target_kind {
        RepoTargetKind::ExistingRepo => Ok(RepoInitReport {
            path: canonical_display(target),
            created: false,
            already_existed: true,
            created_files: vec![],
            created_directories: vec![],
            skipped: vec!["minds.yaml".to_string()],
        }),
        RepoTargetKind::ExistingEmptyDirectory => init_repo_in_empty_dir(target),
        RepoTargetKind::NewDirectory => {
            fs::create_dir(target).map_err(|e| {
                MfError::usage(
                    format!("cannot create directory '{}': {}", target.display(), e),
                    Some("check parent directory permissions".to_string()),
                )
            })?;
            match init_repo_in_empty_dir(target) {
                Ok(report) => Ok(report),
                Err(e) => {
                    let _ = fs::remove_dir(target);
                    Err(e)
                }
            }
        }
        RepoTargetKind::MalformedManifest => Err(MfError::usage(
            format!("cannot initialize '{}': existing minds.yaml is malformed or incompatible", target.display()),
            Some("fix or remove the existing minds.yaml first, then try again".to_string()),
        )),
        RepoTargetKind::InvalidFileTarget => Err(MfError::usage(
            format!("cannot initialize '{}': path is a file, not a directory", target.display()),
            Some("choose a directory path instead".to_string()),
        )),
        RepoTargetKind::InvalidPath => Err(MfError::usage(
            format!("invalid target path '{}'", target.display()),
            Some("use a valid directory name without path traversal".to_string()),
        )),
        RepoTargetKind::UnsafeNonEmptyDirectory => Err(MfError::usage(
            format!("cannot initialize '{}': directory is not empty", target.display()),
            Some("choose an empty directory, a new path, or run from an existing Mind Repo".to_string()),
        )),
    }
}

fn init_repo_in_empty_dir(repo_root: &Path) -> Result<RepoInitReport> {
    let manifest = MindsManifest::create_default();
    let yaml = serialize_mind_manifest(&manifest).expect("default manifest serialization is infallible");

    util::atomic_write(&repo_root.join("minds.yaml"), &yaml)?;
    fs::create_dir(repo_root.join(&manifest.projects_dir)).map_err(MfError::Io)?;

    Ok(RepoInitReport {
        path: canonical_display(repo_root),
        created: true,
        already_existed: false,
        created_files: vec!["minds.yaml".to_string()],
        created_directories: vec![manifest.projects_dir.clone()],
        skipped: vec![],
    })
}

fn canonical_display(path: &Path) -> String {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf()).display().to_string()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

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
            projects_dir: default_projects_dir(),
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
    fn test_load_manifest_empty_file_returns_default() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("minds.yaml");
        fs::write(&path, "").unwrap();
        let manifest = load_manifest(&path).unwrap();
        assert_eq!(manifest.schema_version, "1");
        assert!(manifest.projects.is_empty());
    }

    #[test]
    fn test_load_manifest_whitespace_only_returns_default() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("minds.yaml");
        fs::write(&path, "   \n\t\n").unwrap();
        let manifest = load_manifest(&path).unwrap();
        assert_eq!(manifest.schema_version, "1");
        assert!(manifest.projects.is_empty());
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
    fn test_scan_project_dirs_default_projects_dir() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("minds.yaml"), "").unwrap();
        let projects = dir.path().join("projects");
        fs::create_dir_all(&projects).unwrap();
        let p1 = projects.join("project-a");
        fs::create_dir_all(&p1).unwrap();
        fs::write(p1.join("mind.yaml"), "").unwrap();
        let p2 = projects.join("not-a-project");
        fs::create_dir_all(&p2).unwrap();
        let p3 = projects.join("project-b");
        fs::create_dir_all(&p3).unwrap();
        fs::write(p3.join("mind.yaml"), "").unwrap();
        // A directory at the repo root that contains mind.yaml must be IGNORED
        // because we're scanning under projects/.
        let stray = dir.path().join("flat-project");
        fs::create_dir_all(&stray).unwrap();
        fs::write(stray.join("mind.yaml"), "").unwrap();

        let scanned = scan_project_dirs(dir.path(), "projects");
        let mut names: Vec<&str> = scanned.iter().map(|s| s.name.as_str()).collect();
        names.sort();
        assert_eq!(names, vec!["project-a", "project-b"]);
        let path_a = scanned.iter().find(|s| s.name == "project-a").unwrap();
        assert_eq!(path_a.path, "./projects/project-a");
    }

    #[test]
    fn test_scan_project_dirs_flat_layout() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("minds.yaml"), "").unwrap();
        let p1 = dir.path().join("flat-project");
        fs::create_dir_all(&p1).unwrap();
        fs::write(p1.join("mind.yaml"), "").unwrap();

        let scanned = scan_project_dirs(dir.path(), ".");
        assert_eq!(scanned.len(), 1);
        assert_eq!(scanned[0].name, "flat-project");
        assert_eq!(scanned[0].path, "./flat-project");
    }

    #[test]
    fn test_project_relpath_default() {
        assert_eq!(project_relpath("projects", "foo"), "./projects/foo");
        assert_eq!(project_relpath(".", "foo"), "./foo");
        assert_eq!(project_relpath("", "foo"), "./foo");
        assert_eq!(project_relpath("/projects/", "foo"), "./projects/foo");
    }

    #[test]
    fn test_load_manifest_string_project_paths() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("minds.yaml");
        fs::write(&path, "schema_version: '1'\nprojects:\n  - alpha\n  - beta\n  - gamma\n").unwrap();
        let manifest = load_manifest(&path).unwrap();
        assert_eq!(manifest.projects.len(), 3);
        // Path strings should be resolved to ./projects/<name>
        assert_eq!(manifest.projects[0].path, "./projects/alpha");
        assert_eq!(manifest.projects[1].path, "./projects/beta");
        assert_eq!(manifest.projects[2].path, "./projects/gamma");
    }

    #[test]
    fn test_load_manifest_mixed_project_entries() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("minds.yaml");
        fs::write(
            &path,
            r#"schema_version: '1'
projects:
  - alpha
  - name: beta
    path: ./projects/beta
    created_at: "2026-01-01T00:00:00Z"
"#,
        )
        .unwrap();
        let manifest = load_manifest(&path).unwrap();
        assert_eq!(manifest.projects.len(), 2);
        // String entry gets resolved, object entry is kept as-is
        assert_eq!(manifest.projects[0].path, "./projects/alpha");
        assert_eq!(manifest.projects[1].path, "./projects/beta");
    }

    #[test]
    fn test_load_manifest_schema_alias() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("minds.yaml");
        fs::write(&path, "schema: '1'\nprojects: []\n").unwrap();
        let manifest = load_manifest(&path).unwrap();
        assert_eq!(manifest.schema_version, "1");
    }

    #[test]
    fn test_projects_dir_for_default_when_missing() {
        let dir = tempfile::tempdir().unwrap();
        // No minds.yaml — falls back to default.
        assert_eq!(projects_dir_for(dir.path()).unwrap(), "projects");
    }

    #[test]
    fn test_projects_dir_for_reads_explicit_value() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("minds.yaml"), "schema_version: '1'\nprojects_dir: work\nprojects: []\n").unwrap();
        assert_eq!(projects_dir_for(dir.path()).unwrap(), "work");
    }

    #[test]
    fn test_compute_diff_add_remove() {
        let manifest = MindsManifest {
            schema_version: "1".to_string(),
            projects_dir: default_projects_dir(),
            projects: vec![ProjectEntry {
                name: "old-project".to_string(),
                path: "./old-project".to_string(),
                created_at: "2026-01-01T00:00:00Z".to_string(),
                archived_at: None,
            }],
        };
        let scanned = vec![ScannedProject { name: "new-project".to_string(), path: "./new-project".to_string() }];
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
            projects_dir: default_projects_dir(),
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

    // ── Init command unit tests ──

    #[test]
    fn test_classify_new_directory_valid_parent() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("new-dir");
        let kind = classify_repo_target(&target).unwrap();
        assert_eq!(kind, RepoTargetKind::NewDirectory);
    }

    #[test]
    fn test_classify_new_directory_parent_missing() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("missing-parent").join("child");
        let result = classify_repo_target(&target);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("parent directory"));
    }

    #[test]
    fn test_classify_new_directory_parent_is_file() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("not-a-dir");
        fs::write(&file_path, "content").unwrap();
        let target = file_path.join("child");
        let result = classify_repo_target(&target);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not a directory"));
    }

    #[test]
    fn test_classify_existing_repo() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("minds.yaml"), "schema_version: '1'\nprojects: []\n").unwrap();
        let kind = classify_repo_target(dir.path()).unwrap();
        assert_eq!(kind, RepoTargetKind::ExistingRepo);
    }

    #[test]
    fn test_classify_existing_empty_directory() {
        let dir = tempfile::tempdir().unwrap();
        let kind = classify_repo_target(dir.path()).unwrap();
        assert_eq!(kind, RepoTargetKind::ExistingEmptyDirectory);
    }

    #[test]
    fn test_classify_unsafe_non_empty_directory() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("readme.txt"), "notes").unwrap();
        let kind = classify_repo_target(dir.path()).unwrap();
        assert_eq!(kind, RepoTargetKind::UnsafeNonEmptyDirectory);
    }

    #[test]
    fn test_classify_invalid_file_target() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("a-file.txt");
        fs::write(&file_path, "content").unwrap();
        let kind = classify_repo_target(&file_path).unwrap();
        assert_eq!(kind, RepoTargetKind::InvalidFileTarget);
    }

    #[test]
    fn test_classify_malformed_manifest() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("minds.yaml"), "invalid: yaml: [[[").unwrap();
        let kind = classify_repo_target(dir.path()).unwrap();
        assert_eq!(kind, RepoTargetKind::MalformedManifest);
    }

    #[test]
    fn test_classify_incompatible_schema_manifest() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("minds.yaml"), "schema_version: '99'\nprojects: []\n").unwrap();
        let kind = classify_repo_target(dir.path()).unwrap();
        assert_eq!(kind, RepoTargetKind::MalformedManifest);
    }

    #[test]
    fn test_classify_invalid_path_dotdot() {
        let kind = classify_repo_target(Path::new("..")).unwrap();
        assert_eq!(kind, RepoTargetKind::InvalidPath);
    }

    #[test]
    fn test_validate_not_nested_detects_parent_repo() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("minds.yaml"), "schema: '1'\nprojects: []\n").unwrap();
        let sub = dir.path().join("subdir");
        fs::create_dir(&sub).unwrap();
        let result = validate_not_nested(&sub);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("inside an existing Mind Repo"));
    }

    #[test]
    fn test_validate_not_nested_ok_when_no_parent_repo() {
        let dir = tempfile::tempdir().unwrap();
        let sub = dir.path().join("subdir");
        fs::create_dir(&sub).unwrap();
        assert!(validate_not_nested(&sub).is_ok());
    }

    #[test]
    fn test_init_repo_existing_repo_is_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("minds.yaml"), "schema: '1'\nprojects: []\n").unwrap();
        fs::create_dir(dir.path().join("projects")).unwrap();
        let report = init_repo(dir.path(), &RepoTargetKind::ExistingRepo).unwrap();
        assert!(!report.created);
        assert!(report.already_existed);
        assert!(report.created_files.is_empty());
        assert!(report.skipped.contains(&"minds.yaml".to_string()));
    }

    #[test]
    fn test_init_repo_in_empty_directory_creates_files() {
        let dir = tempfile::tempdir().unwrap();
        let report = init_repo(dir.path(), &RepoTargetKind::ExistingEmptyDirectory).unwrap();
        assert!(report.created);
        assert!(!report.already_existed);
        assert!(report.created_files.contains(&"minds.yaml".to_string()));
        assert!(report.created_directories.contains(&"projects".to_string()));
        assert!(dir.path().join("minds.yaml").exists());
        assert!(dir.path().join("projects").is_dir());
    }

    #[test]
    fn test_init_repo_unsafe_non_empty_refused() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("readme.txt"), "notes").unwrap();
        let result = init_repo(dir.path(), &RepoTargetKind::UnsafeNonEmptyDirectory);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not empty"));
    }

    #[test]
    fn test_init_repo_invalid_file_target_refused() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("a-file.txt");
        fs::write(&file_path, "content").unwrap();
        let result = init_repo(&file_path, &RepoTargetKind::InvalidFileTarget);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("file"));
    }

    #[test]
    fn test_init_repo_new_directory_with_cleanup() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("new-repo");
        let report = init_repo(&target, &RepoTargetKind::NewDirectory).unwrap();
        assert!(report.created);
        assert!(target.join("minds.yaml").exists());
        assert!(target.join("projects").is_dir());
    }
}
