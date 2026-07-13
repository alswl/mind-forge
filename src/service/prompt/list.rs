use std::path::Path;

use crate::error::Result;
use crate::service::index;

use super::PromptRecord;

/// List every prompt projection in the project, with binding status computed
/// fresh against the current `articles` set. Read-only: does not write
/// `mind-index.yaml`. Deterministically sorted by path.
pub fn list(project_path: &Path) -> Result<Vec<PromptRecord>> {
    let idx = index::load(project_path)?;

    let mut records: Vec<PromptRecord> = index::resolve_prompt_bindings(&idx)
        .into_iter()
        .map(|binding| PromptRecord {
            path: binding.prompt.path.clone(),
            article: binding.prompt.article.clone(),
            mode: binding.prompt.mode,
            updated_at: binding.prompt.updated_at.clone(),
            binding_status: binding.status,
        })
        .collect();

    records.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(records)
}
