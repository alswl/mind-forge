use std::fs;
use std::path::{Path, PathBuf};

use super::sha256_file;
use crate::error::{MfError, Result};
use crate::model::asset::AssetUpdateResult;
use crate::service::index;
use crate::service::util;

/// Resolve a user-provided path to an asset path within `<project>/assets/`.
fn resolve_asset_path(project_root: &Path, cwd: &Path, input: &Path) -> Result<PathBuf> {
    let assets_dir = project_root.join("assets");
    let candidates = [project_root.join(input), assets_dir.join(input), cwd.join(input)];
    for candidate in &candidates {
        if let Ok(canonical) = util::canonicalize_within(&assets_dir, candidate) {
            return Ok(canonical);
        }
    }
    Err(MfError::usage(
        format!("could not resolve '{}' within assets/", input.display()),
        Some("run 'mf asset list'".to_string()),
    ))
}

/// Update the size and hash for a single asset identified by `input`.
pub fn update_one(project_path: &Path, cwd: &Path, input: &Path) -> Result<AssetUpdateResult> {
    let resolved = resolve_asset_path(project_path, cwd, input)?;

    let project_canonical = project_path.canonicalize().map_err(MfError::Io)?;
    let rel_path = resolved
        .strip_prefix(&project_canonical)
        .map_err(|_| {
            MfError::usage(
                "resolved path is outside the project root".to_string(),
                Some("run 'mf asset list'".to_string()),
            )
        })?
        .to_string_lossy()
        .replace('\\', "/");

    let mut index = index::load(project_path)?;
    let entry_idx =
        index.assets.as_ref().and_then(|assets| assets.iter().position(|a| a.path == rel_path)).ok_or_else(|| {
            MfError::usage(format!("no index entry for '{rel_path}'"), Some("run 'mf asset list'".to_string()))
        })?;

    let entry = &index.assets.as_ref().unwrap()[entry_idx];
    let old_size = entry.size;
    let old_hash = entry.hash.clone();

    if !resolved.exists() {
        return Err(MfError::usage(
            format!("file not found on disk: {rel_path}"),
            Some("run 'mf asset index' to reconcile".to_string()),
        ));
    }

    let metadata = fs::metadata(&resolved).map_err(MfError::Io)?;
    let new_size = metadata.len();
    let new_hash = sha256_file(&resolved)?;
    let changed = new_size != old_size || new_hash != old_hash;

    if changed {
        if let Some(assets) = &mut index.assets {
            if let Some(entry) = assets.get_mut(entry_idx) {
                entry.size = new_size;
                entry.hash = new_hash.clone();
            }
        }
        index::save(project_path, &index)?;
    }

    Ok(AssetUpdateResult { path: rel_path, changed, old_size, new_size, old_hash, new_hash, error: None })
}

/// Update size and hash for all registered assets.
pub fn update_all(project_path: &Path) -> Result<Vec<AssetUpdateResult>> {
    let mut index = index::load(project_path)?;
    let assets = index.assets.get_or_insert_with(Vec::new);
    let mut results = Vec::new();
    let mut any_changed = false;

    for entry in assets.iter_mut() {
        let full_path = project_path.join(&entry.path);
        let old_size = entry.size;
        let old_hash = entry.hash.clone();

        if !full_path.exists() {
            results.push(AssetUpdateResult {
                path: entry.path.clone(),
                changed: false,
                old_size,
                new_size: old_size,
                old_hash: old_hash.clone(),
                new_hash: old_hash,
                error: Some("missing_file".to_string()),
            });
            continue;
        }

        let metadata = match fs::metadata(&full_path) {
            Ok(m) => m,
            Err(_) => {
                results.push(AssetUpdateResult {
                    path: entry.path.clone(),
                    changed: false,
                    old_size,
                    new_size: old_size,
                    old_hash: old_hash.clone(),
                    new_hash: old_hash,
                    error: Some("missing_file".to_string()),
                });
                continue;
            }
        };

        let new_size = metadata.len();
        let new_hash = match sha256_file(&full_path) {
            Ok(h) => h,
            Err(_) => {
                results.push(AssetUpdateResult {
                    path: entry.path.clone(),
                    changed: false,
                    old_size,
                    new_size: old_size,
                    old_hash: old_hash.clone(),
                    new_hash: old_hash,
                    error: Some("missing_file".to_string()),
                });
                continue;
            }
        };

        let changed = new_size != old_size || new_hash != old_hash;
        if changed {
            entry.size = new_size;
            entry.hash.clone_from(&new_hash);
            any_changed = true;
        }

        results.push(AssetUpdateResult {
            path: entry.path.clone(),
            changed,
            old_size,
            new_size,
            old_hash,
            new_hash,
            error: None,
        });
    }

    results.sort_by(|a, b| a.path.cmp(&b.path));

    if any_changed {
        index::save(project_path, &index)?;
    }

    Ok(results)
}
