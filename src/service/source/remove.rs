use std::path::Path;

use crate::error::{MfError, Result};
use crate::model::source::{SourceKind, SourceRemoveReport};
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
    let mut file_deleted = false;

    if !keep_file && matches!(entry.kind, SourceKind::Pdf | SourceKind::File) {
        if let Some(ref rel_path) = entry.path {
            let abs_path = project_path.join(rel_path);
            match std::fs::remove_file(&abs_path) {
                Ok(_) => file_deleted = true,
                Err(ref e) if e.kind() == std::io::ErrorKind::NotFound => {
                    file_deleted = false;
                }
                Err(e) => {
                    return Err(MfError::Io(e));
                }
            }
        }
    }

    sources.remove(idx);
    index::save(project_path, &index)?;

    Ok(SourceRemoveReport { source: entry, file_deleted })
}
