use std::path::{Path, PathBuf};

use crate::error::{MfError, Result};
use crate::service::util;

/// Resolve a project path within the repo root, with boundary checking.
///
/// Delegates to [`util::resolve_project`] for path identity resolution, then
/// verifies the project exists and is within the repo boundary.
pub fn resolve_project(repo_root: &Path, name: Option<&str>, cwd: &Path) -> Result<PathBuf> {
    let target = util::resolve_project(repo_root, name, cwd)?;
    if let Some(n) = name
        && !target.join("mind.yaml").exists()
    {
        return Err(MfError::usage(
            format!("project '{n}' not found in Mind Repo"),
            Some("use `mf project list` to see available projects".to_string()),
        ));
    }
    util::canonicalize_within(repo_root, &target)
}
