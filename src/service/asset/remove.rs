use std::path::Path;

use crate::error::{MfError, Result};
use crate::model::asset::AssetRemoveReport;
use crate::model::lifecycle::PlannedChange;
use crate::service::index;
use crate::service::lifecycle;

#[allow(dead_code)] // old API surface, remove_asset with dry_run is canonical
pub fn remove(project_path: &Path, file: &str, force: bool) -> Result<AssetRemoveReport> {
    remove_impl(project_path, file, force, false)
}

/// Remove an asset (with dry-run support).
pub fn remove_asset(project_path: &Path, file: &str, force: bool, dry_run: bool) -> Result<AssetRemoveReport> {
    remove_impl(project_path, file, force, dry_run)
}

fn remove_impl(project_path: &Path, file: &str, force: bool, dry_run: bool) -> Result<AssetRemoveReport> {
    let mut index = index::load(project_path)?;

    let (pos, asset_path, asset_name, was_referenced) = {
        let assets = index.assets.as_ref().ok_or_else(|| {
            MfError::usage(
                format!("asset '{file}' not found"),
                Some("use `mf asset list -p <project>` to see available assets".to_string()),
            )
        })?;

        let entry = assets.iter().find(|a| a.path == file || a.name == file).ok_or_else(|| {
            MfError::usage(
                format!("asset '{file}' not found"),
                Some("use `mf asset list -p <project>` to see available assets".to_string()),
            )
        })?;

        let pos = assets.iter().position(|a| a.path == file || a.name == file).unwrap();
        let asset_path = entry.path.clone();
        let asset_name = entry.name.clone();
        let full_path = project_path.join(&asset_path);

        // Check if asset is referenced in article files
        let was_referenced = if let Some(articles) = &index.articles {
            let name = asset_name.clone();
            articles.iter().any(|a| {
                let source_file = project_path.join(&a.article_path);
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

    // Reference scan from index
    let refs =
        lifecycle::scan_references(project_path, &index, crate::model::lifecycle::ObjectKind::Asset, &asset_name);

    if was_referenced && !force {
        return Err(MfError::usage(
            format!("asset '{asset_name}' is referenced by articles; use --force to remove anyway"),
            Some("check which articles reference this asset before removal".to_string()),
        ));
    }

    let mut planned: Vec<PlannedChange> = Vec::new();
    if asset_path.exists() {
        planned.push(lifecycle::planned_remove_file(&project_path.join(&asset_path).to_string_lossy()));
    }
    planned.push(lifecycle::planned_yaml_update(
        &project_path.join("mind-index.yaml").to_string_lossy(),
        Some(&asset_name),
        None,
    ));
    planned.push(lifecycle::planned_index_refresh(&project_path.join("mind-index.yaml").to_string_lossy()));

    if dry_run {
        return Ok(AssetRemoveReport {
            removed: asset_path.to_string_lossy().to_string(),
            was_referenced,
            references: refs,
            side_effects: planned,
            force,
            dry_run: true,
        });
    }

    // Remove file from disk
    if asset_path.exists() {
        std::fs::remove_file(&asset_path).map_err(MfError::Io)?;
    }

    // Remove from index
    {
        let assets = index.assets.as_mut().ok_or_else(|| MfError::usage(format!("asset '{file}' not found"), None))?;
        let new_pos = assets.iter().position(|a| a.name == asset_name).unwrap_or(pos);
        assets.remove(new_pos);
    }
    index::save(project_path, &index)?;

    let removed_path = asset_path.to_string_lossy().to_string();
    Ok(AssetRemoveReport {
        removed: removed_path,
        was_referenced,
        references: refs,
        side_effects: planned,
        force,
        dry_run: false,
    })
}
