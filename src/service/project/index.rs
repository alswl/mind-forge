use std::path::{Path, PathBuf};

use crate::error::{MfError, Result};
use crate::model::index::IndexFile;
use crate::model::project::ProjectStatusSnapshot;
use crate::service::{repo, util};

/// Resolve a project path within the repo root, with boundary checking.
pub fn resolve_project(repo_root: &Path, name: Option<&str>, cwd: &Path) -> Result<PathBuf> {
    let projects_dir = repo::projects_dir_for(repo_root)?;
    match name {
        Some(name) => {
            util::validate_project_name(name)?;
            let target = util::project_dir_for(repo_root, &projects_dir, name);
            // Check existence first so a missing project (or missing projects_dir
            // entirely) yields a clean usage error rather than an IO error from
            // canonicalize_within walking a non-existent parent.
            if !target.join("mind.yaml").exists() {
                return Err(MfError::usage(
                    format!("project '{name}' not found in Mind Repo"),
                    Some("use `mf project list` to see available projects".to_string()),
                ));
            }
            util::canonicalize_within(repo_root, &target)
        }
        None => {
            let detected = util::detect_current_project(repo_root, cwd).ok_or_else(|| {
                MfError::usage(
                    "could not detect current project; run from a project directory or specify --project",
                    Some("use `mf project list` to see available projects".to_string()),
                )
            })?;
            let target = util::project_dir_for(repo_root, &projects_dir, &detected);
            util::canonicalize_within(repo_root, &target)
        }
    }
}

/// Get status snapshot for a project path. `repo_root` is used to compute the
/// repo-root-relative `path` field on the snapshot.
pub fn status_for(repo_root: &Path, project_path: &Path) -> Result<ProjectStatusSnapshot> {
    let name = project_path.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let rel_path = match project_path.strip_prefix(repo_root) {
        Ok(rel) => format!("./{}", rel.display()),
        Err(_) => format!("./{}", name),
    };

    let index_path = project_path.join("mind-index.yaml");
    let index = if index_path.exists() {
        match std::fs::read_to_string(&index_path) {
            Ok(content) if !content.trim().is_empty() => {
                serde_yaml::from_str::<IndexFile>(&content).unwrap_or_else(|_| {
                    tracing::warn!("failed to parse {}", index_path.display());
                    IndexFile::create_default()
                })
            }
            _ => IndexFile::create_default(),
        }
    } else {
        IndexFile::create_default()
    };

    let articles = index.articles.as_ref().map(|v| v.len() as u64).unwrap_or(0);
    let assets = index.assets.as_ref().map(|v| v.len() as u64).unwrap_or(0);
    let sources = index.sources.as_ref().map(|v| v.len() as u64).unwrap_or(0);
    let terms = index.terms.as_ref().map(|v| v.len() as u64).unwrap_or(0);

    let mut max_ts: Option<String> = None;
    for entry in index.articles.iter().flatten() {
        if max_ts.as_ref().map_or(true, |m| &entry.updated_at > m) {
            max_ts = Some(entry.updated_at.clone());
        }
    }
    for entry in index.assets.iter().flatten() {
        if max_ts.as_ref().map_or(true, |m| &entry.added_at > m) {
            max_ts = Some(entry.added_at.clone());
        }
    }
    for entry in index.sources.iter().flatten() {
        if max_ts.as_ref().map_or(true, |m| &entry.updated_at > m) {
            max_ts = Some(entry.updated_at.clone());
        }
    }

    Ok(ProjectStatusSnapshot { name, path: rel_path, articles, assets, sources, terms, updated_at: max_ts })
}
