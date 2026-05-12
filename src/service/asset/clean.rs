use std::path::Path;

use crate::error::Result;
use crate::model::asset::AssetCleanReport;
use crate::service::index;

/// Scan index for stale entries (files that no longer exist on disk) and remove them.
pub fn clean(project_path: &Path, dry_run: bool) -> Result<AssetCleanReport> {
    let mut index = index::load(project_path)?;
    let assets = match index.assets.as_mut() {
        Some(a) => a,
        None => {
            return Ok(AssetCleanReport { stale_entries: Vec::new(), removed_count: 0, dry_run });
        }
    };

    let mut stale_entries: Vec<String> = Vec::new();
    assets.retain(|a| {
        let full_path = project_path.join(&a.path);
        let exists = full_path.exists();
        if !exists {
            stale_entries.push(a.path.clone());
        }
        exists
    });

    let removed_count = stale_entries.len() as u64;

    if !dry_run && removed_count > 0 {
        index::save(project_path, &index)?;
    }

    Ok(AssetCleanReport { stale_entries, removed_count, dry_run })
}
