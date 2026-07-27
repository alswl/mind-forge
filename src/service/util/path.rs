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
        // Walk up from cwd looking for mind.yaml; fall back to repo root.
        let mut anchor = repo_root;
        let mut dir = Some(cwd);
        while let Some(d) = dir {
            if d.join("mind.yaml").exists() {
                anchor = d;
                break;
            }
            dir = d.parent();
        }
        anchor.join(p)
    };

    if resolved.exists() {
        Ok(resolved)
    } else {
        Err(MfError::usage(format!("file not found\n  input:    {input}\n  resolved: {}", resolved.display()), None))
    }
}

/// Resolve a `source new` local-file input against the anchors a caller may
/// reasonably mean, in precedence order (first existing wins):
///   1. absolute path, used as-is
///   2. `project_path` + input     (project-relative, e.g. `sources/x/f.md`)
///   3. `sources_dir` + input      (sources-relative, e.g. `x/f.md`)
///   4. `repo_root` + input        (repo-relative)
///   5. `cwd` + input              (legacy process-relative fallback)
///
/// On none-found, returns a `MfError::usage` (exit 2) naming the input and the
/// anchors tried — never a raw filesystem `MfError::Io` (exit 1). This fixes
/// spec 069 #23, where a relative input in a worktree hit `canonicalize` and
/// surfaced as an internal error.
pub fn resolve_source_input(
    input: &str,
    project_path: &Path,
    sources_dir: &Path,
    cwd: &Path,
    repo_root: &Path,
) -> Result<PathBuf> {
    let p = Path::new(input);
    if p.is_absolute() {
        if p.exists() {
            return Ok(p.to_path_buf());
        }
        return Err(MfError::usage(
            format!("file not found\n  input: {input}\n  resolved: {}", p.display()),
            Some("pass an existing file path".to_string()),
        ));
    }

    let anchors = [project_path, sources_dir, repo_root, cwd];
    for anchor in anchors {
        let candidate = anchor.join(p);
        if candidate.exists() {
            return Ok(candidate);
        }
    }

    let tried: Vec<String> = anchors.iter().map(|a| a.join(p).display().to_string()).collect();
    Err(MfError::usage(
        format!("file not found\n  input: {input}\n  tried:\n    {}", tried.join("\n    ")),
        Some("pass a path relative to the project, its sources/ dir, the repo root, or an absolute path".to_string()),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn touch(path: &Path) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, b"x").unwrap();
    }

    #[test]
    fn resolves_each_anchor_and_reports_precedence() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = tmp.path();
        let project = repo.join("projects/demo");
        let sources = project.join("sources");
        let cwd = repo.join("cwd");
        fs::create_dir_all(&cwd).unwrap();

        // sources-relative: `self-eval/x.md` lives under sources/
        touch(&sources.join("self-eval/x.md"));
        let got = resolve_source_input("self-eval/x.md", &project, &sources, &cwd, repo).unwrap();
        assert_eq!(got, sources.join("self-eval/x.md"));

        // project-relative: `sources/self-eval/x.md` under the project root
        let got = resolve_source_input("sources/self-eval/x.md", &project, &sources, &cwd, repo).unwrap();
        assert_eq!(got, project.join("sources/self-eval/x.md"));

        // absolute
        let abs = sources.join("self-eval/x.md");
        let got = resolve_source_input(abs.to_str().unwrap(), &project, &sources, &cwd, repo).unwrap();
        assert_eq!(got, abs);
    }

    #[test]
    fn cwd_is_the_final_fallback() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = tmp.path();
        let project = repo.join("projects/demo");
        let sources = project.join("sources");
        let cwd = repo.join("elsewhere");
        touch(&cwd.join("only-here.md"));
        let got = resolve_source_input("only-here.md", &project, &sources, &cwd, repo).unwrap();
        assert_eq!(got, cwd.join("only-here.md"));
    }

    #[test]
    fn not_found_is_a_usage_error_naming_anchors() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = tmp.path();
        let project = repo.join("projects/demo");
        let sources = project.join("sources");
        let err = resolve_source_input("nope/missing.md", &project, &sources, repo, repo).unwrap_err();
        assert!(matches!(err, MfError::Usage { .. }), "expected usage error, got {err:?}");
        let msg = err.message();
        assert!(msg.contains("file not found"), "{msg}");
        assert!(msg.contains("tried"), "{msg}");
    }

    #[test]
    fn missing_absolute_is_usage_not_io() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = tmp.path();
        let abs = repo.join("does/not/exist.md");
        let err = resolve_source_input(abs.to_str().unwrap(), repo, repo, repo, repo).unwrap_err();
        assert!(matches!(err, MfError::Usage { .. }), "expected usage error, got {err:?}");
    }
}
