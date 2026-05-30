use std::path::Path;

use crate::error::{MfError, Result};
use crate::model::lifecycle::PlannedChange;
use crate::model::source::{FileKind, SourceRemoveReport};
use crate::service::index;
use crate::service::lifecycle;

/// Remove a source by name. If the source is a pdf/file type and `keep_file` is false,
/// the archive file is also deleted.
#[allow(dead_code)] // old API surface, remove_source with dry_run is canonical
pub fn remove(project_path: &Path, name: &str, keep_file: bool) -> Result<SourceRemoveReport> {
    lifecycle_remove(project_path, name, keep_file, false, false)
}

/// Remove a source by its stored path (e.g. `sources/yuque/foo.md`).
#[allow(dead_code)] // old API surface, remove_source with dry_run is canonical
pub fn remove_by_path(project_path: &Path, path: &str, keep_file: bool) -> Result<SourceRemoveReport> {
    lifecycle_remove_by_path(project_path, path, keep_file, false, false)
}

/// Remove a source by name (lifecycle-aware with dry-run and force).
pub fn remove_source(
    project_path: &Path,
    name_or_path: &str,
    keep_file: bool,
    force: bool,
    dry_run: bool,
) -> Result<SourceRemoveReport> {
    let is_path = name_or_path.contains('/') || name_or_path.starts_with("sources");
    if is_path {
        lifecycle_remove_by_path(project_path, name_or_path, keep_file, force, dry_run)
    } else {
        lifecycle_remove(project_path, name_or_path, keep_file, force, dry_run)
    }
}

fn lifecycle_remove(
    project_path: &Path,
    name: &str,
    keep_file: bool,
    force: bool,
    dry_run: bool,
) -> Result<SourceRemoveReport> {
    let mut index = index::load(project_path)?;
    let sources = index.sources.as_ref().ok_or_else(|| {
        MfError::usage(
            format!("source '{name}' not found"),
            Some("use `mf source list` to see available sources".to_string()),
        )
    })?;

    let entry = sources.iter().find(|s| s.name == name).cloned().ok_or_else(|| {
        MfError::usage(
            format!("source '{name}' not found"),
            Some("use `mf source list` to see available sources".to_string()),
        )
    })?;

    let refs = lifecycle::scan_references(project_path, &index, crate::model::lifecycle::ObjectKind::Source, name);

    if !refs.is_empty() && !force {
        let ref_ids: Vec<&str> = refs.iter().map(|r| r.from_id.as_str()).collect();
        return Err(MfError::usage(
            format!("source '{name}' is referenced by: {}. Use --force to remove anyway.", ref_ids.join(", ")),
            Some("check which articles reference this source before removal".to_string()),
        ));
    }

    let mut planned: Vec<PlannedChange> = Vec::new();
    if !keep_file && matches!(entry.kind, FileKind::Pdf | FileKind::File) {
        if let Some(ref rel_path) = entry.path {
            let abs_path = project_path.join(rel_path);
            if abs_path.exists() {
                planned.push(lifecycle::planned_remove_file(&abs_path.to_string_lossy()));
            }
        }
    }
    planned.push(lifecycle::planned_yaml_update(
        &project_path.join("mind-index.yaml").to_string_lossy(),
        Some(name),
        None,
    ));
    planned.push(lifecycle::planned_index_refresh(&project_path.join("mind-index.yaml").to_string_lossy()));

    if dry_run {
        let file_deleted = !keep_file
            && matches!(entry.kind, FileKind::Pdf | FileKind::File)
            && entry.path.as_ref().is_some_and(|p| project_path.join(p).exists());
        return Ok(SourceRemoveReport {
            source: entry,
            file_deleted,
            references: refs,
            side_effects: planned,
            force,
            dry_run: true,
        });
    }

    let file_deleted = delete_file_if_needed(project_path, &entry, keep_file)?;

    {
        let sources = index.sources.as_mut().expect("already checked");
        let idx = sources.iter().position(|s| s.name == name).expect("already checked");
        sources.remove(idx);
    }
    index::save(project_path, &index)?;

    Ok(SourceRemoveReport {
        source: entry,
        file_deleted,
        references: refs,
        side_effects: planned,
        force,
        dry_run: false,
    })
}

fn lifecycle_remove_by_path(
    project_path: &Path,
    path: &str,
    keep_file: bool,
    force: bool,
    dry_run: bool,
) -> Result<SourceRemoveReport> {
    let mut index = index::load(project_path)?;
    let sources = index.sources.as_ref().ok_or_else(|| {
        MfError::usage(
            format!("source with path '{path}' not found"),
            Some("use `mf source list` to see available sources".to_string()),
        )
    })?;

    let entry =
        sources.iter().find(|s| s.path.as_deref() == Some(path) || s.name == path).cloned().ok_or_else(|| {
            MfError::usage(
                format!("source with path or name '{path}' not found"),
                Some("use `mf source list` to see available sources".to_string()),
            )
        })?;

    let refs =
        lifecycle::scan_references(project_path, &index, crate::model::lifecycle::ObjectKind::Source, &entry.name);

    if !refs.is_empty() && !force {
        let ref_ids: Vec<&str> = refs.iter().map(|r| r.from_id.as_str()).collect();
        return Err(MfError::usage(
            format!("source '{}' is referenced by: {}. Use --force to remove anyway.", entry.name, ref_ids.join(", ")),
            Some("check which articles reference this source before removal".to_string()),
        ));
    }

    let mut planned: Vec<PlannedChange> = Vec::new();
    if !keep_file && matches!(entry.kind, FileKind::Pdf | FileKind::File) {
        if let Some(ref rel_path) = entry.path {
            let abs_path = project_path.join(rel_path);
            if abs_path.exists() {
                planned.push(lifecycle::planned_remove_file(&abs_path.to_string_lossy()));
            }
        }
    }
    planned.push(lifecycle::planned_yaml_update(
        &project_path.join("mind-index.yaml").to_string_lossy(),
        Some(&entry.name),
        None,
    ));
    planned.push(lifecycle::planned_index_refresh(&project_path.join("mind-index.yaml").to_string_lossy()));

    if dry_run {
        let file_deleted = !keep_file
            && matches!(entry.kind, FileKind::Pdf | FileKind::File)
            && entry.path.as_ref().is_some_and(|p| project_path.join(p).exists());
        return Ok(SourceRemoveReport {
            source: entry,
            file_deleted,
            references: refs,
            side_effects: planned,
            force,
            dry_run: true,
        });
    }

    let file_deleted = delete_file_if_needed(project_path, &entry, keep_file)?;

    {
        let sources = index.sources.as_mut().expect("already checked");
        let idx =
            sources.iter().position(|s| s.path.as_deref() == Some(path) || s.name == path).expect("already checked");
        sources.remove(idx);
    }
    index::save(project_path, &index)?;

    Ok(SourceRemoveReport {
        source: entry,
        file_deleted,
        references: refs,
        side_effects: planned,
        force,
        dry_run: false,
    })
}

fn delete_file_if_needed(project_path: &Path, entry: &crate::model::source::Source, keep_file: bool) -> Result<bool> {
    if !keep_file && matches!(entry.kind, FileKind::Pdf | FileKind::File) {
        if let Some(ref rel_path) = entry.path {
            let abs_path = project_path.join(rel_path);
            match std::fs::remove_file(&abs_path) {
                Ok(_) => return Ok(true),
                Err(ref e) if e.kind() == std::io::ErrorKind::NotFound => {
                    return Ok(false);
                }
                Err(e) => return Err(MfError::Io(e)),
            }
        }
    }
    Ok(false)
}
