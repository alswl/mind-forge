use std::path::Path;

use crate::error::Result;
use crate::service::index;

use super::ThinkingRecord;

/// List every thinking projection in the project, with binding status
/// computed fresh against the current `articles` set. Read-only.
/// Deterministically sorted by path.
pub fn list(project_path: &Path) -> Result<Vec<ThinkingRecord>> {
    let idx = index::load(project_path)?;

    let mut records: Vec<ThinkingRecord> = index::resolve_thinking_bindings(&idx)
        .into_iter()
        .map(|binding| ThinkingRecord {
            path: binding.thinking.path.clone(),
            article: binding.thinking.article.clone(),
            updated_at: binding.thinking.updated_at.clone(),
            binding_status: binding.status,
        })
        .collect();

    records.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(records)
}
