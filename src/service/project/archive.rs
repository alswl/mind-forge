use std::path::Path;

use chrono::Utc;

use crate::error::{MfError, Result};
use crate::model::project::ProjectArchiveReport;
use crate::service::repo;

/// Archive a project: `git mv projects/<P> _archived/<P>`.
pub fn archive_project(repo_root: &Path, project_name: &str) -> Result<ProjectArchiveReport> {
    // Pre-check: must be a git repo
    let git_check =
        std::process::Command::new("git").args(["rev-parse", "--is-inside-work-tree"]).current_dir(repo_root).output();

    match git_check {
        Ok(output) if output.status.success() => {}
        Ok(_) => {
            return Err(MfError::usage(
                "not a git repository",
                Some("project archive requires a git repository; use 'git init' first".to_string()),
            ));
        }
        Err(e) => {
            return Err(MfError::usage(
                format!("failed to check git repository: {e}"),
                Some("ensure git is installed".to_string()),
            ));
        }
    }

    let projects_dir = repo::projects_dir_for(repo_root)?;
    let trimmed = projects_dir.trim_matches('/');
    let parent = if trimmed.is_empty() || trimmed == "." { repo_root.to_path_buf() } else { repo_root.join(trimmed) };

    let from_path = parent.join(project_name);
    if !from_path.exists() {
        return Err(MfError::usage(
            format!("project '{project_name}' not found at {}", from_path.display()),
            Some("use 'mf project list' to see available projects".to_string()),
        ));
    }

    let archived_parent = repo_root.join("_archived");
    let to_path = archived_parent.join(project_name);

    if to_path.exists() {
        return Err(MfError::usage(
            format!("_archived/{project_name} already exists"),
            Some("remove or rename the existing archived project first".to_string()),
        ));
    }

    // Ensure _archived dir exists
    std::fs::create_dir_all(&archived_parent).map_err(MfError::Io)?;

    // Run git mv
    let from_rel = from_path.strip_prefix(repo_root).unwrap_or(&from_path);
    let to_rel = to_path.strip_prefix(repo_root).unwrap_or(&to_path);

    let mv_output = std::process::Command::new("git")
        .args(["mv", from_rel.to_string_lossy().as_ref(), to_rel.to_string_lossy().as_ref()])
        .current_dir(repo_root)
        .output()
        .map_err(|e| MfError::usage(format!("git mv failed: {e}"), Some("ensure git is installed".to_string())))?;

    if !mv_output.status.success() {
        let stderr = String::from_utf8_lossy(&mv_output.stderr);
        return Err(MfError::usage(
            format!("git mv failed: {stderr}"),
            Some("check that there are no unstaged changes blocking the move".to_string()),
        ));
    }

    // Refresh top-level index (minds.yaml)
    let minds_path = repo_root.join("minds.yaml");
    if minds_path.exists() {
        let mut manifest = repo::load_manifest(&minds_path)?;
        if let Some(entry) = manifest.projects.iter_mut().find(|p| p.name == project_name) {
            entry.archived_at = Some(Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string());
        }
        repo::save_manifest(&manifest, &minds_path)?;
    }

    Ok(ProjectArchiveReport {
        name: project_name.to_string(),
        from: from_rel.to_string_lossy().to_string(),
        to: to_rel.to_string_lossy().to_string(),
    })
}
