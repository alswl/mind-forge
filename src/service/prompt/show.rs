use std::path::Path;

use crate::error::{MfError, Result};

use super::{list, PromptRecord};

/// Resolve a single prompt by the identity emitted from `list` (its path).
/// Read-only. Exits with a usage error (exit 2) when no match is found,
/// consistent with sibling `show` commands.
pub fn show(project_path: &Path, selector: &str) -> Result<PromptRecord> {
    let records = list(project_path)?;
    records.into_iter().find(|r| r.path == selector).ok_or_else(|| {
        MfError::usage(
            format!("prompt '{selector}' not found"),
            Some("use `mf prompt list` to see available prompts".to_string()),
        )
    })
}
