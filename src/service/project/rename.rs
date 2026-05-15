use std::path::Path;

use chrono::Utc;
use serde::Serialize;

use crate::error::{MfError, Result};
use crate::service::{repo, util};

/// Report from a successful project rename.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct ProjectRenameReport {
    pub old_name: String,
    pub new_name: String,
    pub from: String,
    pub to: String,
}

/// Rename a project: renames the directory, updates minds.yaml, and refreshes
/// the project's mind-index.yaml with the new name.
pub fn rename_project(repo_root: &Path, old_name: &str, new_name: &str) -> Result<ProjectRenameReport> {
    util::validate_project_name(new_name)?;

    let projects_dir = repo::projects_dir_for(repo_root)?;
    let trimmed = projects_dir.trim_matches('/');
    let parent = if trimmed.is_empty() || trimmed == "." {
        repo_root.to_path_buf()
    } else {
        let p = repo_root.join(trimmed);
        if !p.exists() {
            return Err(MfError::usage(
                format!("projects directory '{}' not found", trimmed),
                Some("use 'mf project list' to see available projects".to_string()),
            ));
        }
        p
    };

    let old_path = parent.join(old_name);
    if !old_path.exists() {
        return Err(MfError::usage(
            format!("project '{old_name}' not found at {}", old_path.display()),
            Some("use 'mf project list' to see available projects".to_string()),
        ));
    }

    let new_path = parent.join(new_name);
    if new_path.exists() {
        return Err(MfError::file_exists(new_path));
    }

    // Use git mv when inside a git repo, otherwise fall back to fs::rename
    let from_rel = old_path.strip_prefix(repo_root).unwrap_or(&old_path);
    let to_rel = new_path.strip_prefix(repo_root).unwrap_or(&new_path);

    let git_check =
        std::process::Command::new("git").args(["rev-parse", "--is-inside-work-tree"]).current_dir(repo_root).output();

    match git_check {
        Ok(output) if output.status.success() => {
            let mv_output = std::process::Command::new("git")
                .args(["mv", from_rel.to_string_lossy().as_ref(), to_rel.to_string_lossy().as_ref()])
                .current_dir(repo_root)
                .output()
                .map_err(|e| {
                    MfError::usage(format!("git mv failed: {e}"), Some("ensure git is installed".to_string()))
                })?;

            if !mv_output.status.success() {
                let stderr = String::from_utf8_lossy(&mv_output.stderr);
                return Err(MfError::usage(
                    format!("git mv failed: {stderr}"),
                    Some("check that there are no unstaged changes blocking the move".to_string()),
                ));
            }
        }
        Ok(_) => {
            std::fs::rename(&old_path, &new_path).map_err(MfError::Io)?;
        }
        Err(e) => {
            return Err(MfError::usage(
                format!("failed to check git repository: {e}"),
                Some("ensure git is installed".to_string()),
            ));
        }
    }

    // Update minds.yaml: rename the project entry
    let minds_path = repo_root.join("minds.yaml");
    if minds_path.exists() {
        let mut manifest = repo::load_manifest(&minds_path)?;
        if let Some(entry) = manifest.projects.iter_mut().find(|p| p.name == old_name) {
            entry.name = new_name.to_string();
            entry.path = repo::project_relpath(&projects_dir, new_name);
        }
        repo::save_manifest(&manifest, &minds_path)?;
    }

    // Update mind-index.yaml: rename the project field in every article
    let index_path = new_path.join("mind-index.yaml");
    if index_path.exists() {
        if let Ok(mut index) = crate::service::index::load(&new_path) {
            let now = Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
            if let Some(ref mut articles) = index.articles {
                for article in articles.iter_mut() {
                    article.project = new_name.to_string();
                    article.updated_at = now.clone();
                }
            }
            let _ = crate::service::index::save(&new_path, &index);
        }
    }

    Ok(ProjectRenameReport {
        old_name: old_name.to_string(),
        new_name: new_name.to_string(),
        from: from_rel.to_string_lossy().to_string(),
        to: to_rel.to_string_lossy().to_string(),
    })
}
