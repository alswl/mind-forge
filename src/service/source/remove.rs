use std::path::Path;

use crate::error::{MfError, Result};
use crate::model::source::{FileKind, SourceRemoveReport};
use crate::service::index;

/// Remove a source by name. If the source is a pdf/file type and `keep_file` is false,
/// the archive file is also deleted.
pub fn remove(project_path: &Path, name: &str, keep_file: bool) -> Result<SourceRemoveReport> {
    let mut index = index::load(project_path)?;
    let sources = index.sources.as_mut().ok_or_else(|| {
        MfError::usage(
            format!("source '{name}' not found"),
            Some("use 'mf source list' to see available sources".to_string()),
        )
    })?;

    let idx = sources.iter().position(|s| s.name == name).ok_or_else(|| {
        MfError::usage(
            format!("source '{name}' not found"),
            Some("use 'mf source list' to see available sources".to_string()),
        )
    })?;

    let entry = sources[idx].clone();
    let file_deleted = delete_file_if_needed(project_path, &entry, keep_file)?;

    sources.remove(idx);
    index::save(project_path, &index)?;

    Ok(SourceRemoveReport { source: entry, file_deleted })
}

/// Remove a source by its stored path (e.g. `sources/yuque/foo.md`).
/// This is the mind-primary form (FR-023).
pub fn remove_by_path(project_path: &Path, path: &str, keep_file: bool) -> Result<SourceRemoveReport> {
    let mut index = index::load(project_path)?;
    let sources = index.sources.as_mut().ok_or_else(|| {
        MfError::usage(
            format!("source with path '{path}' not found"),
            Some("use 'mf source list' to see available sources".to_string()),
        )
    })?;

    // Try path match first, then fall back to name match
    let idx = sources
        .iter()
        .position(|s| s.path.as_deref() == Some(path))
        .or_else(|| sources.iter().position(|s| s.name == path))
        .ok_or_else(|| {
            MfError::usage(
                format!("source with path or name '{path}' not found"),
                Some("use 'mf source list' to see available sources".to_string()),
            )
        })?;

    let entry = sources[idx].clone();
    let file_deleted = delete_file_if_needed(project_path, &entry, keep_file)?;

    sources.remove(idx);
    index::save(project_path, &index)?;

    Ok(SourceRemoveReport { source: entry, file_deleted })
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
