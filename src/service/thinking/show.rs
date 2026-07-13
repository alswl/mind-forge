use std::path::Path;

use crate::error::{MfError, Result};

use super::{list, ThinkingRecord};

/// Resolve a single thinking entry by the identity emitted from `list` (its
/// path). Read-only. Exits with a usage error (exit 2) when no match is
/// found, consistent with sibling `show` commands.
pub fn show(project_path: &Path, selector: &str) -> Result<ThinkingRecord> {
    let records = list(project_path)?;
    records.into_iter().find(|r| r.path == selector).ok_or_else(|| {
        MfError::usage(
            format!("thinking entry '{selector}' not found"),
            Some("use `mf thinking list` to see available thinking entries".to_string()),
        )
    })
}
