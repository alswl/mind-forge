use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::path::PathBuf;

use chrono::Utc;
use walkdir::WalkDir;

use super::{infer_kind, sha256_file, update_all};
use crate::error::{MfError, Result};
use crate::model::asset::{Asset, AssetIndexEntry, AssetIndexReport};
use crate::service::config as config_svc;
use crate::service::index;

/// Recursively scan the project assets directory for regular files.
fn scan_assets_dir(assets_dir: &Path) -> Result<Vec<PathBuf>> {
    if !assets_dir.exists() {
        return Ok(Vec::new());
    }

    let mut files = Vec::new();
    for entry in WalkDir::new(assets_dir).follow_links(true).into_iter() {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };

        if let Some(name) = entry.file_name().to_str() {
            if name.starts_with('.') || name == "Thumbs.db" {
                continue;
            }
        }

        if entry.file_type().is_file() {
            files.push(entry.path().to_path_buf());
        }
    }

    Ok(files)
}

/// Reconcile the filesystem assets directory with the index.
pub fn reconcile(project_path: &Path, dry_run: bool, refresh_metadata: bool) -> Result<AssetIndexReport> {
    let layout = config_svc::effective_layout(project_path)?;
    let assets_dir = project_path.join(&layout.assets);
    if !assets_dir.exists() {
        return Err(MfError::usage(
            format!("assets directory not found at '{}'", assets_dir.display()),
            Some("run `mf project lint --fix` to create missing directories".to_string()),
        ));
    }

    let mut index = index::load(project_path)?;
    let existing_assets = index.assets.clone().unwrap_or_default();

    let disk_files = scan_assets_dir(&assets_dir)?;
    let project_canonical = project_path.canonicalize().map_err(MfError::Io)?;

    let indexed_paths: BTreeSet<String> = existing_assets.iter().map(|a| a.path.clone()).collect();
    let disk_paths: BTreeSet<String> = disk_files
        .iter()
        .filter_map(|f| f.strip_prefix(&project_canonical).ok().map(|p| p.to_string_lossy().replace('\\', "/")))
        .collect();

    let mut added: Vec<_> = disk_paths
        .difference(&indexed_paths)
        .map(|p| {
            let full_path = project_path.join(p);
            let name = full_path.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
            let size = fs::metadata(&full_path).map(|m| m.len()).unwrap_or(0);
            let hash = sha256_file(&full_path).unwrap_or_default();
            let ext = full_path.extension();
            let kind = infer_kind(ext);
            let now = Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();

            let asset =
                Asset { name: name.clone(), kind, path: p.clone(), size, hash, tags: Vec::new(), added_at: now };
            index.assets.get_or_insert_with(Vec::new).push(asset);

            AssetIndexEntry { name, path: p.clone() }
        })
        .collect();
    added.sort_by(|a, b| a.name.cmp(&b.name));

    let mut removed: Vec<_> = indexed_paths
        .difference(&disk_paths)
        .map(|p| {
            let name = existing_assets.iter().find(|a| a.path == *p).map(|a| a.name.clone()).unwrap_or_else(|| {
                Path::new(p).file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default()
            });
            AssetIndexEntry { name, path: p.clone() }
        })
        .collect();
    removed.sort_by(|a, b| a.name.cmp(&b.name));

    let remove_paths: std::collections::HashSet<String> = removed.iter().map(|r| r.path.clone()).collect();
    if let Some(assets) = &mut index.assets {
        assets.retain(|a| !remove_paths.contains(&a.path));
    }

    let kept_paths: BTreeSet<String> = disk_paths.intersection(&indexed_paths).cloned().collect();
    let kept_count = kept_paths.len() as u64;

    let refreshed = if refresh_metadata {
        let results = update_all(project_path)?;
        if results.is_empty() {
            None
        } else {
            Some(results)
        }
    } else {
        None
    };

    if let Some(assets) = &mut index.assets {
        assets.sort_by(|a, b| a.name.cmp(&b.name));
    }

    if !dry_run {
        index::save(project_path, &index)?;
    }

    Ok(AssetIndexReport { added, removed, kept_count, refreshed })
}
