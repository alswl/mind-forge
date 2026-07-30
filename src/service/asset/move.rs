use std::fs;
use std::path::Path;

use serde::Serialize;

use crate::error::{MfError, Result};
use crate::service::{config, index};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct AssetMoveReport {
    pub name: String,
    pub old_path: String,
    pub new_path: String,
    pub dry_run: bool,
}

/// Move one local asset between projects, updating both project indexes.
pub fn move_asset(
    source_project: &Path,
    target_project: &Path,
    selector: &str,
    dry_run: bool,
) -> Result<AssetMoveReport> {
    if source_project == target_project {
        return Err(MfError::usage("asset source and destination projects must differ", None::<String>));
    }
    let mut source_index = index::load(source_project)?;
    let original_source_index = source_index.clone();
    let source_assets = source_index.assets.as_ref().ok_or_else(|| {
        MfError::not_found(
            format!("asset '{selector}' not found"),
            Some("use `mf asset list` to see available assets".to_string()),
        )
    })?;
    let source_pos =
        source_assets.iter().position(|asset| asset.name == selector || asset.path == selector).ok_or_else(|| {
            MfError::not_found(
                format!("asset '{selector}' not found"),
                Some("use `mf asset list` to see available assets".to_string()),
            )
        })?;
    let asset = source_assets[source_pos].clone();
    let old_path = asset.path.clone();
    let filename = Path::new(&old_path)
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| MfError::usage("asset path has no filename", None::<String>))?;
    let target_layout = config::effective_layout(target_project)?;
    let new_path = format!("{}/{}", target_layout.assets, filename);
    let old_full = source_project.join(&old_path);
    let new_full = target_project.join(&new_path);
    let mut target_index = index::load(target_project)?;
    if target_index
        .assets
        .as_ref()
        .is_some_and(|assets| assets.iter().any(|candidate| candidate.name == asset.name || candidate.path == new_path))
    {
        return Err(MfError::usage(
            format!("destination already contains asset '{}'", asset.name),
            Some("choose a different asset name or remove the destination entry".to_string()),
        ));
    }
    let report =
        AssetMoveReport { name: asset.name.clone(), old_path: old_path.clone(), new_path: new_path.clone(), dry_run };
    if dry_run {
        return Ok(report);
    }
    if !old_full.exists() {
        return Err(MfError::not_found(format!("asset file '{}' not found", old_full.display()), None::<String>));
    }
    if new_full.exists() {
        return Err(MfError::usage(
            format!("destination asset path '{}' already exists", new_path),
            Some("remove the destination path before moving".to_string()),
        ));
    }
    if let Some(parent) = new_full.parent() {
        fs::create_dir_all(parent).map_err(MfError::Io)?;
    }
    fs::rename(&old_full, &new_full).map_err(MfError::Io)?;

    source_index.assets.as_mut().unwrap().remove(source_pos);
    if let Err(error) = index::save(source_project, &source_index) {
        let _ = fs::rename(&new_full, &old_full);
        return Err(error);
    }
    let mut moved = asset;
    moved.path = new_path.clone();
    target_index.assets.get_or_insert_with(Vec::new).push(moved);
    if let Err(error) = index::save(target_project, &target_index) {
        let _ = fs::rename(&new_full, &old_full);
        let _ = index::save(source_project, &original_source_index);
        return Err(error);
    }
    Ok(report)
}
