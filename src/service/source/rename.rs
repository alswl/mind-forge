use std::path::Path;

use serde::Serialize;

use crate::error::{MfError, Result};
use crate::model::lifecycle::PlannedChange;
use crate::model::source::FileKind;
use crate::service::{lifecycle, util};

/// Report from a successful source rename.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct SourceRenameReport {
    pub verb: String,
    pub kind: String,
    pub before: SourceRenameIdentity,
    pub after: SourceRenameIdentity,
    #[serde(default)]
    pub references: Vec<crate::model::lifecycle::Reference>,
    #[serde(default)]
    pub side_effects: Vec<PlannedChange>,
    pub force: bool,
    pub dry_run: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct SourceRenameIdentity {
    pub name: String,
    pub path: Option<String>,
    pub url: Option<String>,
    pub file_kind: FileKind,
}

/// Rename a project-scoped source entry. If the source has an on-disk file,
/// the file is also renamed.
pub fn rename_source(
    project_path: &Path,
    old_name: &str,
    new_name: &str,
    force: bool,
    dry_run: bool,
) -> Result<SourceRenameReport> {
    util::require_nonempty(old_name, "old source name")?;
    util::require_nonempty(new_name, "new source name")?;

    let mut index = crate::service::index::load(project_path)?;
    let sources = index.sources.as_ref().ok_or_else(|| {
        MfError::not_found(
            format!("source '{old_name}' not found"),
            Some("use `mf source list` to see available sources".to_string()),
        )
    })?;

    let pos = sources.iter().position(|s| s.name == old_name).ok_or_else(|| {
        MfError::not_found(
            format!("source '{old_name}' not found"),
            Some("use `mf source list` to see available sources".to_string()),
        )
    })?;

    // Check for duplicate new_name
    if old_name != new_name && sources.iter().any(|s| s.name == new_name) && !force {
        return Err(MfError::usage(
            format!("a source named '{new_name}' already exists"),
            Some("use --force to overwrite".to_string()),
        ));
    }

    let entry = &sources[pos];
    let before = SourceRenameIdentity {
        name: old_name.to_string(),
        path: entry.path.clone(),
        url: entry.url.clone(),
        file_kind: entry.kind.clone(),
    };
    let after = SourceRenameIdentity {
        name: new_name.to_string(),
        path: entry.path.clone(),
        url: entry.url.clone(),
        file_kind: entry.kind.clone(),
    };

    let mut planned = Vec::new();
    planned.push(lifecycle::planned_yaml_update(
        &project_path.join("mind-index.yaml").to_string_lossy(),
        Some(old_name),
        Some(new_name),
    ));

    // Plan file rename if source has an on-disk file
    if let Some(ref file_path) = entry.path {
        let old_full = project_path.join(file_path);
        if old_full.exists() {
            if let Some(parent) = old_full.parent() {
                if old_full.file_stem().and_then(|s| s.to_str()).is_some() {
                    if let Some(ext) = old_full.extension().and_then(|e| e.to_str()) {
                        let new_path = format!(
                            "{}/{}.{}",
                            parent.strip_prefix(project_path).unwrap_or(parent).display(),
                            new_name,
                            ext
                        );
                        planned.push(PlannedChange {
                            op: crate::model::lifecycle::PlannedOp::RenameFile,
                            path: old_full.to_string_lossy().to_string(),
                            old: Some(old_name.to_string()),
                            new: Some(new_name.to_string()),
                        });
                        let _ = new_path; // used when executing
                    }
                }
            }
        }
    }

    if dry_run {
        return Ok(SourceRenameReport {
            verb: "rename".into(),
            kind: "source".into(),
            before,
            after,
            references: vec![],
            side_effects: planned,
            force,
            dry_run: true,
        });
    }

    // Execute: rename file on disk
    let sources = index.sources.as_mut().unwrap();
    let entry = &mut sources[pos];

    if let Some(ref file_path) = entry.path {
        let old_full = project_path.join(file_path);
        if old_full.exists() {
            if let Some(parent) = old_full.parent() {
                if let Some(ext) = old_full.extension().and_then(|e| e.to_str()) {
                    let new_file_name = format!("{}.{}", new_name, ext);
                    let new_full = parent.join(&new_file_name);
                    if new_full.exists() && !force {
                        return Err(MfError::file_exists(new_full));
                    }
                    std::fs::rename(&old_full, &new_full).map_err(MfError::Io)?;
                    // Update path relative to project
                    let new_rel =
                        new_full.strip_prefix(project_path).unwrap_or(&new_full).to_string_lossy().to_string();
                    entry.path = Some(new_rel);
                }
            }
        }
    }

    entry.name = new_name.to_string();

    crate::service::index::save(project_path, &index)?;

    Ok(SourceRenameReport {
        verb: "rename".into(),
        kind: "source".into(),
        before,
        after,
        references: vec![],
        side_effects: planned,
        force,
        dry_run: false,
    })
}
