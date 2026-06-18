//! Path resolution helpers for `term lint` / `term fix`.

use std::path::{Path, PathBuf};

use crate::error::{MfError, Result};

/// Resolve an input path for `term lint` / `term fix` according to the rules:
/// - Absolute path → used as-is.
/// - Relative with `project_root` → resolved against the project root directory.
/// - Relative without project → resolved against cwd (walk up for mind.yaml), fallback to repo root.
///
/// On NotFound, the error message names both the literal input and the attempted
/// absolute path (FR-011).
pub fn resolve_lint_path(input: &str, project_root: Option<&Path>, cwd: &Path, repo_root: &Path) -> Result<PathBuf> {
    let p = Path::new(input);

    let resolved = if p.is_absolute() {
        p.to_path_buf()
    } else if let Some(proj_root) = project_root {
        proj_root.join(p)
    } else {
        // Walk up from cwd looking for mind.yaml
        let mut dir = Some(cwd);
        while let Some(d) = dir {
            if d.join("mind.yaml").exists() {
                return Ok(d.join(p));
            }
            dir = d.parent();
        }
        // Fallback to repo root
        repo_root.join(p)
    };

    if resolved.exists() {
        Ok(resolved)
    } else {
        Err(MfError::usage(format!("file not found\n  input:    {input}\n  resolved: {}", resolved.display()), None))
    }
}
