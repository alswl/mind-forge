use std::path::Path;

use crate::error::{MfError, Result};
use crate::model::asset::AssetRemoveReport;
use crate::service::index;

/// Remove an asset by path. Checks if the asset is referenced by articles.
pub fn remove(project_path: &Path, file: &str, force: bool) -> Result<AssetRemoveReport> {
    let mut index = index::load(project_path)?;

    let (pos, asset_path, asset_name, was_referenced) = {
        let assets = index.assets.as_mut().ok_or_else(|| {
            MfError::usage(
                format!("asset '{file}' not found"),
                Some("use 'mf asset list -p <project>' to see available assets".to_string()),
            )
        })?;

        let pos = assets.iter().position(|a| a.path == file || a.name == file).ok_or_else(|| {
            MfError::usage(
                format!("asset '{file}' not found"),
                Some("use 'mf asset list -p <project>' to see available assets".to_string()),
            )
        })?;

        let asset_path = assets[pos].path.clone();
        let asset_name = assets[pos].name.clone();
        let full_path = project_path.join(&asset_path);

        // Check if asset is referenced in article source files
        let was_referenced = if let Some(articles) = &index.articles {
            let name = asset_name.clone();
            articles.iter().any(|a| {
                let source_file = project_path.join(&a.source_path);
                if source_file.exists() {
                    match std::fs::read_to_string(&source_file) {
                        Ok(content) => content.contains(&name),
                        Err(_) => false,
                    }
                } else {
                    false
                }
            })
        } else {
            false
        };

        (pos, full_path, asset_name, was_referenced)
    };

    if was_referenced && !force {
        return Err(MfError::usage(
            format!("asset '{asset_name}' is referenced by articles; use --force to remove anyway"),
            Some("check which articles reference this asset before removal".to_string()),
        ));
    }

    // Remove file from disk
    if asset_path.exists() {
        std::fs::remove_file(&asset_path).map_err(MfError::Io)?;
    }

    // Remove from index
    {
        let assets = index.assets.as_mut().ok_or_else(|| MfError::usage(format!("asset '{file}' not found"), None))?;
        // Re-find position (may have shifted, but no other modifications should have happened)
        let new_pos = assets.iter().position(|a| a.name == asset_name).unwrap_or(pos);
        assets.remove(new_pos);
    }
    index::save(project_path, &index)?;

    let removed_path = asset_path.to_string_lossy().to_string();
    Ok(AssetRemoveReport { removed: removed_path, was_referenced })
}
