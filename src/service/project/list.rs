use std::path::Path;

use super::read_index_counts;
use crate::error::Result;
use crate::model::project::ProjectListEntry;
use crate::service::repo;

/// List all projects in the Mind Repo with document counts and last activity.
pub fn list_projects(repo_root: &Path) -> Result<Vec<ProjectListEntry>> {
    let minds_path = repo_root.join("minds.yaml");
    let manifest = if minds_path.exists() {
        repo::load_manifest(&minds_path)?
    } else {
        return Ok(Vec::new());
    };

    let mut entries: Vec<ProjectListEntry> = manifest
        .projects
        .iter()
        .map(|p| {
            let (count, last_activity) = read_index_counts(&repo_root.join(&p.name));
            ProjectListEntry {
                name: p.name.clone(),
                path: p.path.clone(),
                created_at: p.created_at.clone(),
                archived_at: p.archived_at.clone(),
                document_count: count,
                last_activity_at: last_activity,
            }
        })
        .collect();

    entries.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(entries)
}
