use std::path::Path;
use std::time::UNIX_EPOCH;

use super::read_index_counts;
use crate::error::Result;
use crate::model::project::ProjectListEntry;
use crate::service::repo;

/// List all projects in the Mind Repo with document counts and last activity.
///
/// When a project entry is missing `created_at` (shorthand string entries),
/// the filesystem creation time of the project directory is used as a fallback.
/// This is a read-only operation and does not mutate `minds.yaml`.
pub fn list_projects(repo_root: &Path) -> Result<Vec<ProjectListEntry>> {
    let minds_path = repo_root.join("minds.yaml");
    let manifest = if minds_path.exists() {
        repo::load_manifest(&minds_path)?
    } else {
        return Ok(Vec::new());
    };

    let mut entries: Vec<ProjectListEntry> = Vec::with_capacity(manifest.projects.len());
    for p in &manifest.projects {
        let (count, last_activity) = read_index_counts(&repo_root.join(&p.path));
        let created_at =
            if p.created_at.is_empty() { dir_created_iso(&repo_root.join(&p.path)) } else { p.created_at.clone() };
        entries.push(ProjectListEntry {
            name: p.name.clone(),
            path: p.path.clone(),
            created_at,
            archived_at: p.archived_at.clone(),
            document_count: count,
            last_activity_at: last_activity,
        });
    }

    entries.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(entries)
}

fn dir_created_iso(dir: &Path) -> String {
    match std::fs::metadata(dir) {
        Ok(meta) => match meta.created().or_else(|_| meta.modified()) {
            Ok(time) => match time.duration_since(UNIX_EPOCH) {
                Ok(dur) => chrono::DateTime::from_timestamp(dur.as_secs() as i64, 0)
                    .map(|dt| dt.format("%Y-%m-%dT%H:%M:%SZ").to_string())
                    .unwrap_or_default(),
                Err(_) => String::new(),
            },
            Err(_) => String::new(),
        },
        Err(_) => String::new(),
    }
}
