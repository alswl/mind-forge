use std::path::{Path, PathBuf};

use crate::error::Result;
use crate::service::util;

/// Resolve a project path within the repo root, with boundary checking.
///
/// Delegates to [`util::resolve_project`] for path identity resolution and
/// the manifest-existence check (spec 082 FR-008: `util::resolve_project`
/// itself now refuses a selector with no `mind.yaml`, uniformly for every
/// caller), then applies the repo-boundary check.
pub fn resolve_project(repo_root: &Path, name: Option<&str>, cwd: &Path) -> Result<PathBuf> {
    let target = util::resolve_project(repo_root, name, cwd)?;
    util::canonicalize_within(repo_root, &target)
}
