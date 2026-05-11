//! Repo service: manifest load/save, file-system scan, diff, reconcile, and diff rendering.
//!
//! Migrated from `src/runtime/repo.rs` (003). The runtime module now retains only
//! `detect_repo_root` and `detect_repo_root_with_config`.

use std::collections::HashSet;
use std::fs;
use std::path::Path;

use chrono::Utc;
use serde::Serialize;

use crate::error::{MfError, Result};
use crate::model::manifest::{default_projects_dir, MindsManifest, ProjectEntry};
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
pub fn load_manifest(path: &Path) -> Result<MindsManifest> {
    let content = fs::read_to_string(path).map_err(MfError::Io)?;
    if content.trim().is_empty() {
        return Ok(MindsManifest::create_default());
    }
    let manifest: MindsManifest = serde_yaml::from_str(&content).map_err(|e| MfError::ParseError {
        kind: "yaml".to_string(),
        path: path.to_path_buf(),
        detail: e.to_string(),
    })?;
    util::validate_schema_version(&manifest.schema_version, path)?;
    Ok(manifest)
}

/// Atomically write `MindsManifest` to a file (write-then-rename).
pub fn save_manifest(manifest: &MindsManifest, path: &Path) -> Result<()> {
    let content = serde_yaml::to_string(manifest).map_err(|e| MfError::Internal(e.into()))?;
    util::atomic_write(path, &content)
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
}
