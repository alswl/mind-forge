//! Path-based entity identity model and selector normalization.
//!
//! All path-backed entities (project, article, asset, source) use canonical
//! paths as their user-facing identity. Input may be cwd-relative or
//! repo-relative, but canonical identity is always expressed relative to the
//! entity's ownership boundary (repo root for projects, project root for
//! articles/assets/sources).

use std::path::{Path, PathBuf};

use crate::error::{MfError, Result};
use crate::service::util;

// ── Enums ─────────────────────────────────────────────────────────────────

/// The kind of a project-local path-backed entity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum PathEntityKind {
    Article,
    Asset,
    Source,
}

#[allow(dead_code)]
impl PathEntityKind {
    fn layout_dir(self) -> &'static str {
        match self {
            Self::Article => "docs",
            Self::Asset => "assets",
            Self::Source => "sources",
        }
    }
}

// ── Identity types ────────────────────────────────────────────────────────

/// Canonical identity for a project.
///
/// `requested_path` is the raw user input. `path` is the repo-relative
/// canonical project path. `resolved_path` is the canonical absolute path
/// used for boundary checks and writes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectIdentity {
    pub requested_path: String,
    pub path: String,
    pub resolved_path: PathBuf,
}

/// Canonical identity for a project-local entity (article, asset, source).
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub struct ProjectLocalIdentity {
    pub kind: PathEntityKind,
    pub project_identity: String,
    pub path: String,
    pub resolved_path: PathBuf,
}

// ── Project selector normalization (T008) ─────────────────────────────────

/// Normalize a user-provided project selector into a [`ProjectIdentity`].
///
/// `input` may be:
/// - An absolute path (canonicalized and verified to be under `repo_root`).
/// - A path containing `/` (treated as cwd-relative, resolved against `cwd`).
/// - A simple name without path separators: first tries manifest lookup
///   (via the caller's project_path_for), then falls back to cwd-relative
///   resolution (for non-repo-root cwd), and finally to projects_dir-relative.
///
/// Returns the repo-relative canonical `path` and absolute `resolved_path`.
pub fn normalize_project_selector(repo_root: &Path, input: &str, cwd: &Path) -> Result<ProjectIdentity> {
    validate_project_input(input)?;

    let repo_canonical = util::try_canonicalize(repo_root);
    let cwd_canonical = util::try_canonicalize(cwd);
    let has_separator = input.contains('/') || input.contains('\\');

    let resolved = if has_separator {
        canonicalize_within_boundary(&repo_canonical, &cwd_canonical.join(input))?
    } else if cwd_canonical != repo_canonical {
        // Simple name, cwd inside the repo: try cwd-relative first for
        // ergonomic workspace workflows, then fall back to projects_dir.
        canonicalize_within_boundary(&repo_canonical, &cwd_canonical.join(input))
            .or_else(|_| canonicalize_within_boundary(&repo_canonical, &projects_dir_target(repo_root, input)?))?
    } else {
        canonicalize_within_boundary(&repo_canonical, &projects_dir_target(repo_root, input)?)?
    };

    let rel = util::repo_relative_path(&repo_canonical, &resolved);
    Ok(ProjectIdentity { requested_path: input.to_string(), path: rel, resolved_path: resolved })
}

fn projects_dir_target(repo_root: &Path, name: &str) -> Result<PathBuf> {
    let projects_dir = crate::service::repo::projects_dir_for(repo_root)?;
    Ok(util::project_dir_for(repo_root, &projects_dir, name))
}

/// Reject inputs whose components contain `..` (path traversal).
///
/// A bare substring match on `..` is not enough — file names may legitimately
/// contain dots — so this splits on both POSIX and Windows separators.
fn reject_traversal(input: &str, hint: &str) -> Result<()> {
    if !input.contains("..") {
        return Ok(());
    }
    let components: Vec<&str> = input.split(&['/', '\\'][..]).collect();
    if components.contains(&"..") {
        return Err(MfError::usage(
            format!("path '{input}' contains '..' which would escape the {hint} root"),
            Some(format!("use a path under the {hint} root")),
        ));
    }
    Ok(())
}

/// Validate the raw project selector input for basic safety.
fn validate_project_input(input: &str) -> Result<()> {
    if input.is_empty() {
        return Err(MfError::usage(
            "project path cannot be empty",
            Some("provide a path under the repo root".to_string()),
        ));
    }
    if input == "." || input == ".." {
        return Err(MfError::usage(
            format!("invalid project path: '{input}'"),
            Some("use a path under the repo root".to_string()),
        ));
    }
    reject_traversal(input, "repo")
}

/// Canonicalize `target` and verify it lives under `boundary_root`.
/// Unlike `util::canonicalize_within`, this does NOT enforce kebab-case naming.
fn canonicalize_within_boundary(boundary_root: &Path, target: &Path) -> Result<PathBuf> {
    let root_canonical = util::try_canonicalize(boundary_root);

    let outside_err = || {
        MfError::usage(
            format!("path '{}' is outside the boundary root '{}'", target.display(), root_canonical.display()),
            Some("use a path under the repo root".to_string()),
        )
    };

    if target.exists() {
        let canonical = util::try_canonicalize(target);
        if !canonical.starts_with(&root_canonical) {
            return Err(outside_err());
        }
        return Ok(canonical);
    }

    let parent = target.parent().ok_or_else(|| {
        MfError::usage(format!("cannot determine parent of '{}'", target.display()), None as Option<String>)
    })?;
    let leaf = target.file_name().map(|s| s.to_string_lossy().to_string()).ok_or_else(|| {
        MfError::usage(format!("cannot extract filename from '{}'", target.display()), None as Option<String>)
    })?;
    if leaf.is_empty() || leaf == "." || leaf == ".." || leaf.contains('/') || leaf.contains('\\') {
        return Err(MfError::usage(
            format!("invalid path segment '{leaf}'"),
            Some("use a path under the repo root".to_string()),
        ));
    }

    let parent_resolved = if parent.exists() { util::try_canonicalize(parent) } else { parent.to_path_buf() };
    if !parent_resolved.starts_with(&root_canonical) {
        return Err(outside_err());
    }
    Ok(parent_resolved.join(&leaf))
}

// ── Project-local entity selector normalization (T009) ────────────────────

/// Validate that a project-local entity selector stays inside `project_root`.
///
/// Used by CLI handlers that retain legacy title/name matching but still need
/// to reject `..` traversal and symlink escapes before any read or write
/// (spec FR-012, edge case: traversal/symlinks rejected before I/O).
pub fn validate_entity_path(project_root: &Path, input: &str) -> Result<()> {
    reject_traversal(input, "project")?;
    if input.contains('/') || input.contains('\\') {
        let project_canonical = util::try_canonicalize(project_root);
        let target = project_canonical.join(input);
        canonicalize_within_boundary(&project_canonical, &target)?;
    }
    Ok(())
}

/// Normalize a project-local entity selector (article, asset, source) into a
/// [`ProjectLocalIdentity`].
///
/// `input` is treated as a project-relative path. Simple names without path
/// separators are accepted as path shorthand (legacy title/name compat).
/// The resolved path must stay within the project root.
#[allow(dead_code)]
pub fn normalize_entity_selector(
    project_root: &Path,
    project_identity: &str,
    kind: PathEntityKind,
    input: &str,
) -> Result<ProjectLocalIdentity> {
    reject_traversal(input, "project")?;

    let project_canonical = util::try_canonicalize(project_root);
    let has_separator = input.contains('/') || input.contains('\\');
    let relative = if has_separator { input.to_string() } else { format!("{}/{}", kind.layout_dir(), input) };

    let target = project_canonical.join(&relative);
    canonicalize_within_boundary(&project_canonical, &target)?;

    Ok(ProjectLocalIdentity {
        kind,
        project_identity: project_identity.to_string(),
        path: relative,
        resolved_path: target,
    })
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn setup_repo() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let repo_root = dir.path().join("repo");
        fs::create_dir_all(&repo_root).unwrap();
        fs::write(repo_root.join("minds.yaml"), "schema_version: '1'\nprojects_dir: '.'\nprojects: []\n").unwrap();
        (dir, repo_root)
    }

    // ── T007: Project selector normalization ──────────────────────────

    #[test]
    fn normalize_cwd_relative_with_slash() {
        let (_dir, repo_root) = setup_repo();
        let nested = repo_root.join("workspaces/E_团队周报/projects/2026-W21_iSee团队周报");
        fs::create_dir_all(&nested).unwrap();
        fs::write(nested.join("mind.yaml"), "schema_version: '1'\n").unwrap();

        let cwd = repo_root.join("workspaces/E_团队周报/projects");
        // select from cwd
        let id = normalize_project_selector(&repo_root, "2026-W21_iSee团队周报", &cwd).unwrap();
        assert_eq!(id.path, "workspaces/E_团队周报/projects/2026-W21_iSee团队周报");
        assert_eq!(id.requested_path, "2026-W21_iSee团队周报");
    }

    #[test]
    fn normalize_repo_relative_with_slash_from_root() {
        let (_dir, repo_root) = setup_repo();
        let nested = repo_root.join("workspaces/📊周报/projects/2026-W21_团队复盘🚀");
        fs::create_dir_all(&nested).unwrap();
        fs::write(nested.join("mind.yaml"), "schema_version: '1'\n").unwrap();

        let id = normalize_project_selector(&repo_root, "workspaces/📊周报/projects/2026-W21_团队复盘🚀", &repo_root)
            .unwrap();
        assert_eq!(id.path, "workspaces/📊周报/projects/2026-W21_团队复盘🚀");
    }

    #[test]
    fn normalize_simple_name_legacy() {
        let (_dir, repo_root) = setup_repo();
        let id = normalize_project_selector(&repo_root, "simple-report", &repo_root).unwrap();
        assert_eq!(id.path, "simple-report");
        assert_eq!(id.requested_path, "simple-report");
    }

    #[test]
    fn normalize_rejects_empty_input() {
        let (_dir, repo_root) = setup_repo();
        let err = normalize_project_selector(&repo_root, "", &repo_root).unwrap_err();
        assert!(err.to_string().contains("empty"), "got: {err}");
    }

    #[test]
    fn normalize_rejects_dot_input() {
        let (_dir, repo_root) = setup_repo();
        let err = normalize_project_selector(&repo_root, ".", &repo_root).unwrap_err();
        assert!(err.to_string().contains("invalid"), "got: {err}");
    }

    #[test]
    fn normalize_rejects_dotdot_escape() {
        let (_dir, repo_root) = setup_repo();
        let err = normalize_project_selector(&repo_root, "../escape", &repo_root).unwrap_err();
        assert!(
            err.to_string().contains("outside")
                || err.to_string().contains("invalid")
                || err.to_string().contains(".."),
            "got: {err}"
        );
    }

    // ── T007: cwd-relative project selector normalization ──────────────

    #[test]
    fn normalize_unicode_path_from_cwd() {
        let (_dir, repo_root) = setup_repo();
        let nested = repo_root.join("workspaces/E_团队周报/projects/2026-W21_iSee团队周报");
        fs::create_dir_all(&nested).unwrap();

        let cwd = repo_root.join("workspaces/E_团队周报/projects");
        let id = normalize_project_selector(&repo_root, "2026-W21_iSee团队周报", &cwd).unwrap();
        assert_eq!(id.path, "workspaces/E_团队周报/projects/2026-W21_iSee团队周报");
    }

    #[test]
    fn normalize_emoji_path_from_root() {
        let (_dir, repo_root) = setup_repo();
        let nested = repo_root.join("workspaces/📊周报/projects/2026-W21_团队复盘🚀");
        fs::create_dir_all(&nested).unwrap();

        let id = normalize_project_selector(&repo_root, "workspaces/📊周报/projects/2026-W21_团队复盘🚀", &repo_root)
            .unwrap();
        // Repo-root-relative path should match
        assert!(id.path.contains("📊周报"), "got: {}", id.path);
        assert!(id.path.contains("团队复盘🚀"), "got: {}", id.path);
    }

    // ── T007: project-relative entity selector normalization ───────────

    #[test]
    fn normalize_article_selector_with_slash() {
        let (_dir, repo_root) = setup_repo();
        let project = repo_root.join("my-project");
        fs::create_dir_all(&project).unwrap();
        fs::create_dir_all(project.join("docs")).unwrap();
        fs::write(project.join("mind.yaml"), "schema_version: '1'\n").unwrap();

        let id = normalize_entity_selector(&project, "my-project", PathEntityKind::Article, "docs/weekly.md").unwrap();
        assert_eq!(id.path, "docs/weekly.md");
        assert_eq!(id.project_identity, "my-project");
    }

    #[test]
    fn normalize_article_selector_simple_name() {
        let (_dir, repo_root) = setup_repo();
        let project = repo_root.join("my-project");
        fs::create_dir_all(&project).unwrap();
        fs::write(project.join("mind.yaml"), "schema_version: '1'\n").unwrap();

        let id = normalize_entity_selector(&project, "my-project", PathEntityKind::Article, "weekly").unwrap();
        assert_eq!(id.path, "docs/weekly");
    }

    #[test]
    fn normalize_asset_selector_with_slash() {
        let (_dir, repo_root) = setup_repo();
        let project = repo_root.join("my-project");
        fs::create_dir_all(&project).unwrap();
        fs::create_dir_all(project.join("assets")).unwrap();
        fs::write(project.join("mind.yaml"), "schema_version: '1'\n").unwrap();

        let id = normalize_entity_selector(&project, "my-project", PathEntityKind::Asset, "assets/chart.png").unwrap();
        assert_eq!(id.path, "assets/chart.png");
    }

    #[test]
    fn normalize_source_selector_with_slash() {
        let (_dir, repo_root) = setup_repo();
        let project = repo_root.join("my-project");
        fs::create_dir_all(&project).unwrap();
        fs::create_dir_all(project.join("sources")).unwrap();
        fs::write(project.join("mind.yaml"), "schema_version: '1'\n").unwrap();

        let id = normalize_entity_selector(&project, "my-project", PathEntityKind::Source, "sources/meeting/notes.md")
            .unwrap();
        assert_eq!(id.path, "sources/meeting/notes.md");
    }

    #[test]
    fn normalize_entity_rejects_project_root_escape() {
        let (_dir, repo_root) = setup_repo();
        let project = repo_root.join("my-project");
        fs::create_dir_all(&project).unwrap();
        fs::write(project.join("mind.yaml"), "schema_version: '1'\n").unwrap();

        let err =
            normalize_entity_selector(&project, "my-project", PathEntityKind::Article, "../outside.md").unwrap_err();
        assert!(err.to_string().contains("outside") || err.to_string().contains("invalid"), "got: {err}");
    }

    // ── T014: project auto-detection tests ────────────────────────────

    #[test]
    fn detect_project_from_nested_unicode_path() {
        let (_dir, repo_root) = setup_repo();
        let project = repo_root.join("workspaces/E_团队周报/projects/2026-W21_iSee团队周报");
        fs::create_dir_all(&project).unwrap();
        fs::write(project.join("mind.yaml"), "schema_version: '1'\n").unwrap();

        let detected = util::detect_current_project(&repo_root, &project).unwrap();
        assert_eq!(detected, "workspaces/E_团队周报/projects/2026-W21_iSee团队周报");
    }

    #[test]
    fn detect_project_from_nested_emoji_path() {
        let (_dir, repo_root) = setup_repo();
        let project = repo_root.join("workspaces/📊周报/projects/2026-W21_团队复盘🚀");
        fs::create_dir_all(&project).unwrap();
        fs::write(project.join("mind.yaml"), "schema_version: '1'\n").unwrap();

        let detected = util::detect_current_project(&repo_root, &project).unwrap();
        assert_eq!(detected, "workspaces/📊周报/projects/2026-W21_团队复盘🚀");
    }

    #[test]
    fn detect_project_from_subdir() {
        let (_dir, repo_root) = setup_repo();
        let project = repo_root.join("my-project");
        fs::create_dir_all(project.join("docs")).unwrap();
        fs::write(project.join("mind.yaml"), "schema_version: '1'\n").unwrap();

        let cwd = project.join("docs");
        let detected = util::detect_current_project(&repo_root, &cwd).unwrap();
        assert_eq!(detected, "my-project");
    }

    #[test]
    fn detect_project_returns_none_outside_project() {
        let (_dir, repo_root) = setup_repo();
        let cwd = repo_root.clone();
        let detected = util::detect_current_project(&repo_root, &cwd);
        assert!(detected.is_none());
    }

    // ── T015: rejection of escape selectors ───────────────────────────

    #[test]
    fn reject_project_selector_escape_via_dotdot() {
        let (_dir, repo_root) = setup_repo();
        let err = normalize_project_selector(&repo_root, "../outside", &repo_root).unwrap_err();
        assert!(
            err.to_string().contains("outside")
                || err.to_string().contains("invalid")
                || err.to_string().contains(".."),
            "got: {err}"
        );
    }

    #[test]
    fn reject_entity_selector_escape_via_dotdot() {
        let (_dir, repo_root) = setup_repo();
        let project = repo_root.join("my-project");
        fs::create_dir_all(&project).unwrap();
        fs::write(project.join("mind.yaml"), "schema_version: '1'\n").unwrap();

        let err =
            normalize_entity_selector(&project, "my-project", PathEntityKind::Article, "../../etc/passwd").unwrap_err();
        assert!(
            err.to_string().contains("outside")
                || err.to_string().contains("invalid")
                || err.to_string().contains(".."),
            "got: {err}"
        );
    }
}
