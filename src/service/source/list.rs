use std::path::Path;

use crate::error::Result;
use crate::model::source::{FileKind, Source};
use crate::service::index;

/// List sources in a project, with optional name substring filter and type filter.
pub fn list(project_path: &Path, filter: Option<&str>, kind: Option<FileKind>) -> Result<Vec<Source>> {
    let index = index::load(project_path)?;
    let mut sources = index.sources.unwrap_or_default();

    if let Some(f) = filter {
        let lower = f.to_lowercase();
        sources.retain(|s| s.name.to_lowercase().contains(&lower));
    }

    if let Some(k) = kind {
        sources.retain(|s| s.kind == k);
    }

    sources.sort_by(|a, b| a.name.cmp(&b.name));

    Ok(sources)
}
