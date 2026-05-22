use std::path::{Path, PathBuf};

use crate::error::{MfError, Result};
use crate::model::index::IndexFile;
use crate::model::project::ProjectStatusSnapshot;
use crate::service::{config, util};

/// Resolve a project path within the repo root, with boundary checking.
///
/// Delegates to [`util::resolve_project`] for path identity resolution, then
/// verifies the project exists and is within the repo boundary.
pub fn resolve_project(repo_root: &Path, name: Option<&str>, cwd: &Path) -> Result<PathBuf> {
    let target = util::resolve_project(repo_root, name, cwd)?;
    if let Some(n) = name {
        if !target.join("mind.yaml").exists() {
            return Err(MfError::usage(
                format!("project '{n}' not found in Mind Repo"),
                Some("use `mf project list` to see available projects".to_string()),
            ));
        }
    }
    util::canonicalize_within(repo_root, &target)
}

/// Get status snapshot for a project path. `repo_root` is used to compute the
/// repo-root-relative `path` field on the snapshot.
pub fn status_for(repo_root: &Path, project_path: &Path) -> Result<ProjectStatusSnapshot> {
    let dir_name = project_path.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let name = config::load_project(project_path, Some(repo_root))?
        .and_then(|cfg| cfg.project.map(|project| project.name))
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| dir_name.clone());
    let rel_path = match project_path.strip_prefix(repo_root) {
        Ok(rel) => format!("./{}", rel.display()),
        Err(_) => format!("./{}", dir_name),
    };

    let index = match crate::service::index::load(project_path) {
        Ok(idx) => idx,
        Err(e) => {
            let detail = match &e {
                crate::error::MfError::ParseError { detail, .. } => format!(": {detail}"),
                _ => String::new(),
            };
            tracing::warn!("failed to load index for {}{detail}", project_path.display());
            IndexFile::create_default()
        }
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
