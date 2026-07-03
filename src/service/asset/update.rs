use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{MfError, Result};
use crate::model::asset::AssetUpdateResult;
use crate::service::config as config_svc;
use crate::service::index;
use crate::service::util;
use crate::service::util::hash::sha256_file;

/// Outcome for setting the publish URL.
pub struct PublishUrlOutcome {
    pub url: String,
    pub channel: String,
}

/// Set the publish URL and channel for the project (mind form of asset update).
/// Stores the URL in the project's mind-index.yaml publish target config.
pub fn set_publish_url(project_path: &Path, url: &str, channel: &str) -> Result<PublishUrlOutcome> {
    // Validate input
    if url.is_empty() && channel.is_empty() {
        return Err(MfError::usage(
            "at least one of --set-url or --channel is required",
            Some("provide --set-url <URL> and/or --channel <NAME>".to_string()),
        ));
    }

    let mut index = index::load(project_path)?;
    let recs = index.publish_records.get_or_insert_with(Vec::new);

    // For now, store URL as a virtual record tagged by channel
    // A more complete implementation would update mind.yaml's publish targets
    if !url.is_empty() && !channel.is_empty() {
        recs.push(crate::model::index::PublishRecord {
            path: format!("_publish_settings/{channel}"),
            target_name: channel.to_string(),
            status: crate::model::index::PublishStatus::Published,
            target_url: Some(url.to_string()),
            published_at: Some(chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()),
        });
        index::save(project_path, &index)?;
    }

    Ok(PublishUrlOutcome { url: url.to_string(), channel: channel.to_string() })
}

/// Resolve a user-provided path to an asset path within `<project>/assets/`.
fn resolve_asset_path(project_root: &Path, cwd: &Path, input: &Path) -> Result<PathBuf> {
    let layout = config_svc::effective_layout(project_root)?;
    let assets_dir = project_root.join(&layout.assets);
    let candidates = [project_root.join(input), assets_dir.join(input), cwd.join(input)];
    for candidate in &candidates {
        if let Ok(canonical) = util::canonicalize_within(&assets_dir, candidate) {
            return Ok(canonical);
        }
    }
    Err(MfError::usage(
        format!("could not resolve '{}' within {}/", input.display(), layout.assets),
        Some("run `mf asset list`".to_string()),
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
                Some("run `mf asset list`".to_string()),
            )
        })?
        .to_string_lossy()
        .replace('\\', "/");

    let mut index = index::load(project_path)?;
    let entry_idx =
        index.assets.as_ref().and_then(|assets| assets.iter().position(|a| a.path == rel_path)).ok_or_else(|| {
            MfError::usage(format!("no index entry for '{rel_path}'"), Some("run `mf asset list`".to_string()))
        })?;

    let entry = &index.assets.as_ref().unwrap()[entry_idx];
    let old_size = entry.size;
    let old_hash = entry.hash.clone();

    if !resolved.exists() {
        return Err(MfError::usage(
            format!("file not found on disk: {rel_path}"),
            Some("run `mf asset index` to reconcile".to_string()),
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
