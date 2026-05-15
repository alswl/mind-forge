use std::path::Path;

use chrono::Utc;

use crate::defaults;
use crate::error::{MfError, Result};
use crate::model::manifest::ProjectEntry;
use crate::service::{repo, util};

/// Report from a successful scaffold operation.
pub struct ScaffoldReport {
    pub name: String,
    pub project_path: String,
    pub created_at: String,
    pub scaffolded: Vec<String>,
}

/// Create the standard project skeleton under `<repo_root>/<projects_dir>/<name>`.
pub fn scaffold(repo_root: &Path, name: &str, force: bool) -> Result<ScaffoldReport> {
    util::validate_project_name(name)?;

    let projects_dir = repo::projects_dir_for(repo_root)?;
    let trimmed = projects_dir.trim_matches('/');
    let parent = if trimmed.is_empty() || trimmed == "." {
        repo_root.to_path_buf()
    } else {
        let p = repo_root.join(trimmed);
        if !p.exists() {
            std::fs::create_dir_all(&p).map_err(MfError::Io)?;
        }
        p
    };
    let target = parent.join(name);
    let resolved = util::canonicalize_within(repo_root, &target)?;

    if resolved.exists() && !force {
        return Err(MfError::file_exists(resolved));
    }

    let now = Utc::now();
    let created_at = now.format("%Y-%m-%dT%H:%M:%SZ").to_string();
    let mut scaffolded: Vec<String> = Vec::new();

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
        name: name.to_string(),
        project_path: repo::project_relpath(&projects_dir, name),
        created_at,
        scaffolded,
    })
}

/// Upsert a project entry into `minds.yaml`.
pub fn upsert_project_entry(repo_root: &Path, name: &str, created_at: &str) -> Result<ProjectEntry> {
    let minds_path = repo_root.join("minds.yaml");
    let mut manifest = if minds_path.exists() {
        repo::load_manifest(&minds_path)?
    } else {
        crate::model::manifest::MindsManifest::create_default()
    };

    let entry_path = repo::project_relpath(&manifest.projects_dir, name);
    let existing = manifest.projects.iter_mut().find(|p| p.name == name);
    let entry = match existing {
        Some(existing_entry) => {
            if existing_entry.path != entry_path {
                existing_entry.path = entry_path;
            }
            existing_entry.clone()
        }
        None => {
            let entry = ProjectEntry {
                name: name.to_string(),
                path: entry_path,
                created_at: created_at.to_string(),
                archived_at: None,
            };
            manifest.projects.push(entry.clone());
            entry
        }
    };

    repo::save_manifest(&manifest, &minds_path)?;
    Ok(entry)
}
