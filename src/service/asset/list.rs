use std::path::Path;

use crate::error::Result;
use crate::model::asset::{Asset, AssetKind};
use crate::service::index;

/// List assets in a project, with optional substring filter and type filter.
pub fn list(project_path: &Path, filter: Option<&str>, kind: Option<AssetKind>) -> Result<Vec<Asset>> {
    let index = index::load(project_path)?;
    let mut assets = index.assets.unwrap_or_default();

    if let Some(f) = filter {
        let lower = f.to_lowercase();
        assets.retain(|a| {
            a.name.to_lowercase().contains(&lower) || a.tags.iter().any(|t| t.to_lowercase().contains(&lower))
        });
    }

    if let Some(k) = kind {
        assets.retain(|a| a.kind == k);
    }

    assets.sort_by(|a, b| a.name.cmp(&b.name));

    Ok(assets)
}
