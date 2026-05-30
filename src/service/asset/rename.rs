use std::path::Path;

use serde::Serialize;

use crate::error::{MfError, Result};
use crate::model::lifecycle::PlannedChange;
use crate::service::{lifecycle, util};

/// Report from a successful asset rename.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct AssetRenameReport {
    pub verb: String,
    pub kind: String,
    pub before: AssetRenameIdentity,
    pub after: AssetRenameIdentity,
    #[serde(default)]
    pub references: Vec<crate::model::lifecycle::Reference>,
    #[serde(default)]
    pub side_effects: Vec<PlannedChange>,
    pub force: bool,
    pub dry_run: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct AssetRenameIdentity {
    pub name: String,
    pub path: String,
}

/// Rename a project-scoped asset entry. The on-disk file is also renamed.
pub fn rename_asset(
    project_path: &Path,
    old_path: &str,
    new_path: &str,
    force: bool,
    dry_run: bool,
) -> Result<AssetRenameReport> {
    util::require_nonempty(old_path, "old asset path")?;
    util::require_nonempty(new_path, "new asset path")?;

    let mut index = crate::service::index::load(project_path)?;
    let assets = index.assets.as_ref().ok_or_else(|| {
        MfError::not_found(
            format!("asset at '{old_path}' not found"),
            Some("use `mf asset list` to see available assets".to_string()),
        )
    })?;

    let pos = assets.iter().position(|a| a.path == old_path || a.name == old_path).ok_or_else(|| {
        MfError::not_found(
            format!("asset at '{old_path}' not found"),
            Some("use `mf asset list` to see available assets".to_string()),
        )
    })?;

    // Check for duplicate new_path
    if old_path != new_path && assets.iter().any(|a| a.path == new_path) && !force {
        return Err(MfError::usage(
            format!("an asset at '{new_path}' already exists"),
            Some("use --force to overwrite".to_string()),
        ));
    }

    let entry = &assets[pos];
    let before = AssetRenameIdentity { name: entry.name.clone(), path: old_path.to_string() };
    let new_name = Path::new(new_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| new_path.to_string());

    let after = AssetRenameIdentity { name: new_name.clone(), path: new_path.to_string() };

    let mut planned: Vec<PlannedChange> = vec![lifecycle::planned_yaml_update(
        &project_path.join("mind-index.yaml").to_string_lossy(),
        Some(old_path),
        Some(new_path),
    )];
    planned.push(PlannedChange {
        op: crate::model::lifecycle::PlannedOp::RenameFile,
        path: project_path.join(old_path).to_string_lossy().to_string(),
        old: Some(old_path.to_string()),
        new: Some(new_path.to_string()),
    });

    if dry_run {
        return Ok(AssetRenameReport {
            verb: "rename".into(),
            kind: "asset".into(),
            before,
            after,
            references: vec![],
            side_effects: planned,
            force,
            dry_run: true,
        });
    }

    // Rename file on disk
    let old_full = project_path.join(old_path);
    let new_full = project_path.join(new_path);
    if old_full.exists() {
        if let Some(parent) = new_full.parent() {
            std::fs::create_dir_all(parent).map_err(MfError::Io)?;
        }
        if new_full.exists() && !force {
            return Err(MfError::file_exists(new_full));
        }
        std::fs::rename(&old_full, &new_full).map_err(MfError::Io)?;
    }

    // Update index
    let assets = index.assets.as_mut().unwrap();
    let entry = &mut assets[pos];
    entry.name = new_name;
    entry.path = new_path.to_string();

    crate::service::index::save(project_path, &index)?;

    Ok(AssetRenameReport {
        verb: "rename".into(),
        kind: "asset".into(),
        before,
        after,
        references: vec![],
        side_effects: planned,
        force,
        dry_run: false,
    })
}
