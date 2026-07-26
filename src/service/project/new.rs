use std::path::{Path, PathBuf};

use chrono::Utc;

use crate::defaults;
use crate::error::{MfError, Result};
use crate::model::manifest::ProjectEntry;
use crate::service::{repo, util};

/// Report from a successful scaffold operation.
pub struct ScaffoldReport {
    pub requested_path: String,
    pub path: String,
    #[allow(dead_code)]
    pub resolved_path: PathBuf,
    pub created_at: String,
    pub scaffolded: Vec<String>,
}

/// Create the standard project skeleton for the given resolved project path.
///
/// `requested_path` is the raw user input. `canonical_path` is the
/// repo-relative canonical project identity. `resolved_path` is the
/// absolute path where directories and files will be created.
pub fn scaffold(
    repo_root: &Path,
    requested_path: &str,
    canonical_path: &str,
    resolved_path: &Path,
    force: bool,
) -> Result<ScaffoldReport> {
    // Verify the resolved path is within the repo boundary.
    // Use a simple canonicalization check (not canonicalize_within which
    // enforces kebab-case naming — the identity module already validated
    // the path segments).
    let repo_canonical = util::try_canonicalize(repo_root);
    let resolved = util::try_canonicalize(resolved_path);
    if !resolved.starts_with(&repo_canonical) {
        return Err(MfError::usage(
            format!("path '{}' is outside the Mind Repo root '{}'", resolved_path.display(), repo_canonical.display()),
            Some("use a path under the repo root".to_string()),
        ));
    }

    if resolved.exists() && !force {
        // Check if the directory is non-empty (not just a leftover empty dir)
        let has_content = std::fs::read_dir(&resolved).ok().is_some_and(|mut d| d.next().is_some());
        if has_content || resolved.join("mind.yaml").exists() {
            return Err(MfError::file_exists(resolved));
        }
    }

    let now = Utc::now();
    let created_at = now.format("%Y-%m-%dT%H:%M:%SZ").to_string();
    let mut scaffolded: Vec<String> = Vec::new();

    // Create parent directories first
    if let Some(parent) = resolved.parent()
        && !parent.exists()
    {
        std::fs::create_dir_all(parent).map_err(MfError::Io)?;
    }

    for dir in defaults::REQUIRED_PROJECT_DIRS {
        let dir_path = resolved.join(dir);
        if !dir_path.exists() {
            std::fs::create_dir_all(&dir_path).map_err(MfError::Io)?;
            scaffolded.push(dir.to_string());
        }
    }

    let mind_path = resolved.join("mind.yaml");
    if !mind_path.exists() {
        let mind_yaml = format!("schema: '{}'\n", defaults::SCHEMA_VERSION);
        util::atomic_write(&mind_path, &mind_yaml)?;
        scaffolded.push("mind.yaml".to_string());
    }

    let mind_index_path = resolved.join("mind-index.yaml");
    if !mind_index_path.exists() {
        let mind_index_yaml = format!("schema: '{}'\n", defaults::SCHEMA_VERSION);
        util::atomic_write(&mind_index_path, &mind_index_yaml)?;
        scaffolded.push("mind-index.yaml".to_string());
    }

    Ok(ScaffoldReport {
        requested_path: requested_path.to_string(),
        path: canonical_path.to_string(),
        resolved_path: resolved,
        created_at,
        scaffolded,
    })
}

/// Upsert a project entry into `minds.yaml` by canonical path identity.
///
/// The project is registered using its repo-relative canonical path as both
/// the `name` and `path` in the manifest. When `force` is false, duplicate
/// path identities are rejected before writes. When `force` is true, an
/// existing matching entry is returned unchanged (idempotent re-creation).
pub fn upsert_project_entry(
    repo_root: &Path,
    canonical_path: &str,
    created_at: &str,
    force: bool,
) -> Result<ProjectEntry> {
    let minds_path = repo_root.join("minds.yaml");
    let mut manifest = if minds_path.exists() {
        repo::load_manifest(&minds_path)?
    } else {
        crate::model::manifest::MindsManifest::create_default()
    };

    // Check for duplicate path identity
    let existing = manifest
        .projects
        .iter()
        .position(|p| p.name == canonical_path || p.path == canonical_path || p.path == format!("./{canonical_path}"));
    if let Some(idx) = existing {
        if force {
            return Ok(manifest.projects[idx].clone());
        }
        return Err(MfError::usage(
            format!("project path '{}' is already registered as '{}'", canonical_path, manifest.projects[idx].name),
            Some("project paths must be unique; use a different path".to_string()),
        ));
    }

    let entry_path = format!("./{canonical_path}");
    let entry = ProjectEntry {
        name: canonical_path.to_string(),
        path: entry_path,
        created_at: created_at.to_string(),
        archived_at: None,
    };
    manifest.projects.push(entry.clone());

    repo::save_manifest(&manifest, &minds_path)?;
    Ok(entry)
}
