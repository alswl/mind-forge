//! Shared utilities: atomic write, schema version validation, markdown helpers.

pub mod filename_date;
pub mod hash;
pub mod markdown;
pub mod net;
pub mod path;
pub mod path_template;
pub mod yaml_splice;

use std::fs;
use std::path::{Path, PathBuf};

/// A file move that keeps an occupied destination recoverable until its
/// associated metadata write commits.
pub struct PendingFileRename {
    old: PathBuf,
    new: PathBuf,
    backup: Option<(PathBuf, PathBuf)>,
}

impl PendingFileRename {
    pub fn begin(old: &Path, new: &Path) -> Result<Self> {
        let mut backup = None;
        if new.exists() {
            let parent =
                new.parent().ok_or_else(|| MfError::usage("cannot determine target directory", None::<String>))?;
            let dir = create_rename_backup_dir(parent)?;
            let path = dir.join("replaced-file");
            if let Err(error) = std::fs::rename(new, &path) {
                let _ = std::fs::remove_dir(&dir);
                return Err(MfError::Io(error));
            }
            backup = Some((dir, path));
        }
        if let Err(error) = std::fs::rename(old, new) {
            if let Some((dir, path)) = backup.take() {
                if let Err(restore_error) = std::fs::rename(&path, new) {
                    return Err(MfError::Io(restore_error));
                }
                let _ = std::fs::remove_dir_all(dir);
            }
            return Err(MfError::Io(error));
        }
        Ok(Self { old: old.to_path_buf(), new: new.to_path_buf(), backup })
    }

    pub fn rollback(mut self) -> Result<()> {
        std::fs::rename(&self.new, &self.old).map_err(MfError::Io)?;
        if let Some((dir, path)) = self.backup.take() {
            std::fs::rename(&path, &self.new).map_err(MfError::Io)?;
            std::fs::remove_dir_all(dir).map_err(MfError::Io)?;
        }
        Ok(())
    }

    pub fn commit(self) {
        if let Some((dir, _)) = self.backup {
            let _ = std::fs::remove_dir_all(dir);
        }
    }
}

fn create_rename_backup_dir(parent: &Path) -> Result<PathBuf> {
    use std::time::{SystemTime, UNIX_EPOCH};

    let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos();
    for attempt in 0..100 {
        let path = parent.join(format!(".mf-rename-{}-{stamp}-{attempt}", std::process::id()));
        match std::fs::create_dir(&path) {
            Ok(()) => return Ok(path),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(MfError::Io(error)),
        }
    }
    Err(MfError::usage("could not reserve a temporary rename backup", None::<String>))
}
use std::time::{SystemTime, UNIX_EPOCH};

use crate::defaults;
use crate::error::{MfError, Result};

// ── Symlink helper ──────────────────────────────────────────────────────────

#[cfg(unix)]
pub(crate) fn create_symlink(src: &Path, dst: &Path) -> Result<()> {
    std::os::unix::fs::symlink(src, dst).map_err(MfError::Io)
}

#[cfg(not(unix))]
pub(crate) fn create_symlink(_src: &Path, _dst: &Path) -> Result<()> {
    Err(MfError::usage(
        "symlink is not supported on this platform",
        Some("use --copy or omit --link to copy the file".to_string()),
    ))
}

/// Atomically write content to a file using write-then-rename.
///
/// Writes to a temporary file (`{path}.yaml.tmp.{pid}.{rand}`) then
/// renames atomically, ensuring partial writes never land at `path`.
pub fn atomic_write(path: &Path, content: &str) -> Result<()> {
    let tmp_path = {
        let pid = std::process::id();
        let rand: u64 = {
            let t = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
            t.as_nanos() as u64
        };
        path.with_extension(format!("yaml.tmp.{pid}.{rand}"))
    };
    fs::write(&tmp_path, content).map_err(MfError::Io)?;
    fs::rename(&tmp_path, path).map_err(MfError::Io)?;
    Ok(())
}

/// Dry-run-aware variant of [`atomic_write`]. Returns whether a write would
/// have occurred, while preserving the original helper for existing callers.
pub fn atomic_write_dry_run(path: &Path, content: &str, dry_run: bool) -> Result<bool> {
    if dry_run {
        return Ok(true);
    }
    atomic_write(path, content)?;
    Ok(false)
}

/// Validate that `version` is among the compatible schema versions.
///
/// The compatible set is `["1"]` for the current generation.
pub fn validate_schema_version(version: &str, path: &Path) -> Result<()> {
    let compatible = [defaults::SCHEMA_VERSION];
    if !compatible.contains(&version) {
        return Err(MfError::IncompatibleSchema {
            path: path.to_path_buf(),
            found: version.to_string(),
            expected: compatible.iter().map(|s| s.to_string()).collect(),
        });
    }
    Ok(())
}

/// Atomically write a directory containing multiple files.
///
/// Builds the directory under a sibling tmp path
/// `parent/.{name}.tmp.{pid}.{rand}`, writes all files there, then
/// `fs::rename`s into `target`. On any error before the rename, the tmp
/// directory is removed.
pub fn atomic_write_directory(target: &Path, files: &[(&str, &str)]) -> Result<()> {
    let parent = target.parent().unwrap_or_else(|| Path::new("."));
    let name = target.file_name().unwrap_or_default();
    let pid = std::process::id();
    let rand: u64 = {
        let t = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
        t.as_nanos() as u64
    };
    let tmp_name = format!(".{}.tmp.{pid}.{rand}", name.to_string_lossy());
    let tmp_path = parent.join(&tmp_name);

    fs::create_dir(&tmp_path).map_err(MfError::Io)?;

    if let Err(e) = (|| -> Result<()> {
        for (filename, content) in files {
            fs::write(tmp_path.join(filename), content).map_err(MfError::Io)?;
        }
        Ok(())
    })() {
        let _ = fs::remove_dir_all(&tmp_path);
        return Err(e);
    }

    fs::rename(&tmp_path, target).map_err(|e| {
        let _ = fs::remove_dir_all(&tmp_path);
        MfError::Io(e)
    })?;
    Ok(())
}

/// Dry-run-aware directory write seam. Validation and planning remain the
/// caller's responsibility; this function guarantees no filesystem mutation
/// when `dry_run` is set.
#[allow(dead_code)]
pub fn atomic_write_directory_dry_run(target: &Path, files: &[(&str, &str)], dry_run: bool) -> Result<bool> {
    if dry_run {
        return Ok(true);
    }
    atomic_write_directory(target, files)?;
    Ok(false)
}

/// Return the modification time of `path` as RFC 3339 UTC string
/// (e.g. "2026-07-12T09:00:00Z").
pub fn file_mtime_rfc3339(path: &Path) -> Result<String> {
    let meta = fs::metadata(path).map_err(MfError::Io)?;
    let modified = meta.modified().map_err(MfError::Io)?;
    let datetime: chrono::DateTime<chrono::Utc> = modified.into();
    Ok(datetime.format("%Y-%m-%dT%H:%M:%SZ").to_string())
}

/// Canonicalize a path, falling back to the raw path on failure.
///
/// Canonicalization failures are logged as a debug warning so callers
/// are not silently operating on unchecked paths.
pub fn try_canonicalize(path: &Path) -> PathBuf {
    match path.canonicalize() {
        Ok(p) => p,
        Err(e) => {
            tracing::debug!("canonicalize failed for {}: {e}; using raw path", path.display());
            path.to_path_buf()
        }
    }
}

/// Compute a POSIX-separated relative path from `project_path` to `abs`.
pub fn rel_posix_path(project_path: &Path, abs: &Path) -> Result<String> {
    let rel = abs.strip_prefix(project_path).map_err(|_| {
        MfError::usage(
            format!("path '{}' is not under project root '{}'", abs.display(), project_path.display()),
            None as Option<String>,
        )
    })?;
    Ok(rel.to_string_lossy().replace('\\', "/"))
}

/// List direct `*.md` children of `dir`, sorted, as project-relative POSIX
/// paths. Tolerates a missing directory (empty result). Skips dot-files.
pub fn scan_md_paths(project_path: &Path, dir: &Path) -> Result<Vec<String>> {
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut paths = Vec::new();
    for entry in fs::read_dir(dir).map_err(MfError::Io)? {
        let entry = entry.map_err(MfError::Io)?;
        if !entry.file_type().map_err(MfError::Io)?.is_file() {
            continue;
        }
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if name_str.starts_with('.') || !name_str.ends_with(&format!(".{}", defaults::MARKDOWN_EXTENSION)) {
            continue;
        }
        paths.push(rel_posix_path(project_path, &entry.path())?);
    }
    paths.sort();
    Ok(paths)
}

/// Detect the current project from `cwd` within a `repo_root`.
///
/// Walks up from `cwd` looking for `mind.yaml`, stopping at the `repo_root`
/// boundary. Returns the repo-relative canonical project path, or `None`.
pub fn detect_current_project(repo_root: &Path, cwd: &Path) -> Option<String> {
    let repo = try_canonicalize(repo_root);
    let mut current = try_canonicalize(cwd);

    loop {
        if current.join("mind.yaml").exists() {
            return Some(repo_relative_path(&repo, &current));
        }
        if current == repo {
            return None;
        }
        current = current.parent()?.to_path_buf();
    }
}

/// Sanitize a string to kebab-case for use as a filename.
pub fn to_filename(raw: &str) -> String {
    let sanitized: String = raw
        .to_lowercase()
        .chars()
        .map(|c| match c {
            'a'..='z' | '0'..='9' => c,
            ' ' | '_' | '.' => '-',
            // Preserve non-ASCII letters/ideographs (e.g. CJK) so headings like
            // "本周进展" produce distinct, readable slugs instead of collapsing
            // to "untitled" and colliding (Bug #10 residual).
            _ if c.is_alphanumeric() => c,
            _ => '-',
        })
        .collect();

    let collapsed: String = sanitized
        .chars()
        .fold(String::new(), |mut acc, c| {
            let last_is_dash = acc.ends_with('-');
            if c == '-' && last_is_dash {
                /* skip consecutive dashes */
            } else {
                acc.push(c);
            }
            acc
        })
        .trim_matches('-')
        .to_string();

    if collapsed.is_empty() { "untitled".to_string() } else { collapsed }
}

/// Extract the directory name from a path as a string.
pub fn dir_name(path: &Path) -> String {
    path.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default()
}

/// Compute a repo-relative path string from an absolute path.
///
/// If `file_path` is under `repo_root`, returns the relative portion;
/// otherwise returns the path as-is.
pub fn repo_relative_path(repo_root: &Path, file_path: &Path) -> String {
    file_path.strip_prefix(repo_root).unwrap_or(file_path).to_string_lossy().to_string()
}

/// Resolve a project path within a repo root.
///
/// If `project` is `Some(selector)`, tries in order:
/// 1. If the selector is a path form (`.`, or contains `/` or `\`), resolve
///    it cwd-relative or repo-relative through the identity layer — this is
///    the primary form (spec 082 FR-001: it is what a shell completes).
/// 2. Otherwise (a bare name): try manifest lookup and cwd-relative
///    resolution independently. If both exist and point at different
///    directories, refuse as ambiguous rather than silently preferring one
///    (spec 082 FR-003, FR-010). If exactly one exists, use it.
/// 3. If neither resolved, fall back to `<repo_root>/<projects_dir>/<selector>`.
///
/// In every case, the final candidate MUST contain `mind.yaml`, or this
/// returns `not_found` naming the resolved location (spec 082 FR-008). This
/// check lives here — not in each of the ~30 call sites — so every
/// project-scoped command enforces it identically.
///
/// If `project` is `None`, auto-detects the current project from `cwd` by
/// walking up for `mind.yaml` — already guaranteed to exist by construction,
/// so no separate check is needed for that branch.
///
/// One caller — `asset new`/`asset add`, which lazily creates the project
/// directory when it doesn't exist yet — deliberately needs the unchecked
/// candidate instead; it calls [`resolve_project_allow_missing`] directly
/// rather than this function.
pub fn resolve_project(repo_root: &Path, project: Option<&str>, cwd: &Path) -> Result<PathBuf> {
    let projects_dir = crate::service::repo::projects_dir_for(repo_root)?;
    match project {
        Some(selector) => {
            let candidate = resolve_project_candidate(repo_root, &projects_dir, selector, cwd)?;
            if !candidate.join("mind.yaml").exists() {
                let repo_canonical = try_canonicalize(repo_root);
                let shown = repo_relative_path(&repo_canonical, &try_canonicalize(&candidate));
                // `usage` (exit 2), matching the pre-existing convention this
                // check replaces (project::index::resolve_project's own
                // now-removed duplicate check used the same kind) — spec 082
                // FR-008 accepts either usage or not_found, and FR-037 says
                // not to change an exit code an existing test already relies
                // on without a reason tied to this feature's requirements.
                return Err(MfError::usage(
                    format!("project '{selector}' not found (resolved to '{shown}')"),
                    Some("use `mf project list` to see available projects".to_string()),
                ));
            }
            Ok(candidate)
        }
        None => {
            let detected = detect_current_project(repo_root, cwd).ok_or_else(|| {
                MfError::usage(
                    "could not detect current project; run from a project directory or specify --project",
                    Some("use `mf project list` to see available projects".to_string()),
                )
            })?;
            // detected is a repo-relative path; join it directly under repo_root
            if let Some(path) = crate::service::repo::project_path_for(repo_root, &detected)? {
                Ok(path)
            } else {
                Ok(repo_root.join(&detected))
            }
        }
    }
}

/// Resolve a project selector the same way [`resolve_project`] does, but
/// without requiring `mind.yaml` to already exist at the result.
///
/// This exists for the one legitimate case where a project-scoped command
/// creates the project directory as a side effect instead of operating on
/// an existing one — `asset new`/`asset add`, which the `--project
/// <new-name>` positional both selects *and* creates. Every path-resolution
/// rule (path form vs. bare name, the ambiguity refusal, the `..` rejection)
/// still applies identically; only the final existence check is skipped.
///
/// Keep this exception limited to commands that create the selected project.
pub fn resolve_project_allow_missing(repo_root: &Path, project: Option<&str>, cwd: &Path) -> Result<PathBuf> {
    let projects_dir = crate::service::repo::projects_dir_for(repo_root)?;
    match project {
        Some(selector) => resolve_project_candidate(repo_root, &projects_dir, selector, cwd),
        None => resolve_project(repo_root, None, cwd),
    }
}

/// Compute the candidate path for a project selector, without checking
/// whether it actually contains `mind.yaml` — [`resolve_project`] does that
/// check once, uniformly, after calling this.
fn resolve_project_candidate(repo_root: &Path, projects_dir: &str, selector: &str, cwd: &Path) -> Result<PathBuf> {
    // 1. Path form: `.`, or anything containing a separator. Resolved
    // cwd-relative/repo-relative through the identity layer, which also
    // rejects `..` uniformly (spec 082 FR-004, FR-007).
    if selector == "." || selector == ".." || selector.contains('/') || selector.contains('\\') {
        return crate::service::identity::normalize_project_selector(repo_root, selector, cwd)
            .map(|id| id.resolved_path);
    }

    // 2. Bare name: manifest lookup and cwd-relative resolution are two
    // independent candidates for the same input. Prefer neither silently —
    // if both exist (as directories with `mind.yaml`) and differ, refuse
    // rather than guess (spec 082 FR-003, FR-010).
    let manifest_candidate = crate::service::repo::project_path_for(repo_root, selector)?;
    let cwd_candidate = {
        let candidate = cwd.join(selector);
        candidate.join("mind.yaml").exists().then_some(candidate)
    };
    match (&manifest_candidate, &cwd_candidate) {
        (Some(manifest_path), Some(cwd_path)) => {
            let manifest_canonical = try_canonicalize(manifest_path);
            let cwd_canonical = try_canonicalize(cwd_path);
            if manifest_canonical != cwd_canonical {
                let repo_canonical = try_canonicalize(repo_root);
                return Err(MfError::usage(
                    format!(
                        "project selector '{selector}' is ambiguous: it matches both '{}' and '{}'",
                        repo_relative_path(&repo_canonical, &manifest_canonical),
                        repo_relative_path(&repo_canonical, &cwd_canonical),
                    ),
                    Some(
                        "use a path (e.g. './name' for the cwd-relative one, or the full project path) to disambiguate"
                            .to_string(),
                    ),
                ));
            }
            Ok(manifest_path.clone())
        }
        (Some(manifest_path), None) => Ok(manifest_path.clone()),
        (None, Some(cwd_path)) => Ok(cwd_path.clone()),
        // 3. Neither resolved: fall back to projects_dir-relative. The
        // mind.yaml check in `resolve_project` reports not-found for this
        // candidate if it isn't a real project either.
        (None, None) => Ok(project_dir_for(repo_root, projects_dir, selector)),
    }
}

/// Compute the absolute project directory for a `name` under `projects_dir`.
/// `projects_dir == "."` (or empty) is the flat layout (`<repo>/<name>`).
pub fn project_dir_for(repo_root: &Path, projects_dir: &str, name: &str) -> PathBuf {
    let trimmed = projects_dir.trim_matches('/');
    if trimmed.is_empty() || trimmed == "." { repo_root.join(name) } else { repo_root.join(trimmed).join(name) }
}

/// Validate that `s` is non-empty after trimming.
pub fn require_nonempty(s: &str, label: &str) -> Result<()> {
    if s.trim().is_empty() {
        return Err(MfError::usage(format!("{label} cannot be empty"), Some("provide a non-empty value".to_string())));
    }
    Ok(())
}

/// Validate that `name` is a legal Mind Project NAME:
/// - non-empty
/// - kebab-case (a-z, 0-9, hyphen)
/// - no path separators
/// - not `.` or `..`
pub fn validate_project_name(name: &str) -> Result<()> {
    if name.is_empty() {
        return Err(MfError::usage(
            "project name cannot be empty",
            Some("use a kebab-case NAME (e.g. my-project)".to_string()),
        ));
    }
    if name == "." || name == ".." {
        return Err(MfError::usage(
            format!("invalid project name: '{name}'"),
            Some("use a kebab-case NAME (e.g. my-project)".to_string()),
        ));
    }
    if name.contains('/') || name.contains('\\') {
        // Spec 075 US7/FR-036: show the bare form of what was supplied so the
        // user does not have to strip the path themselves.
        let bare = name.rsplit(['/', '\\']).find(|segment| !segment.is_empty()).unwrap_or(name);
        return Err(MfError::usage(
            format!("invalid project name '{name}': path separators are not allowed"),
            Some(format!("use the bare name '{bare}'")),
        ));
    }
    for c in name.chars() {
        if !matches!(c, 'a'..='z' | '0'..='9' | '-') {
            return Err(MfError::usage(
                format!("invalid project name '{name}': only lowercase alphanumeric and hyphens allowed"),
                Some("use a kebab-case NAME (e.g. my-project)".to_string()),
            ));
        }
    }
    Ok(())
}

/// Canonicalize `target` and verify it lives under `root`.
///
/// If `target` does not exist on disk, canonicalizes the parent and checks
/// the leaf component is a single safe filename (no `/`, `\`, `..`, `.`).
pub fn canonicalize_within(root: &Path, target: &Path) -> Result<PathBuf> {
    let root_canonical = root.canonicalize().map_err(MfError::Io)?;

    if target.exists() {
        let target_canonical = target.canonicalize().map_err(MfError::Io)?;
        if !target_canonical.starts_with(&root_canonical) {
            return Err(MfError::usage(
                format!("path '{}' is outside the Mind Repo root '{}'", target.display(), root_canonical.display(),),
                Some("try --project <NAME> or --root <PATH>".to_string()),
            ));
        }
        Ok(target_canonical)
    } else {
        // Target doesn't exist yet — canonicalize parent, validate leaf
        let parent = target.parent().ok_or_else(|| {
            MfError::usage(format!("cannot determine parent of '{}'", target.display()), None as Option<String>)
        })?;
        let leaf = target.file_name().map(|s| s.to_string_lossy().to_string()).ok_or_else(|| {
            MfError::usage(format!("cannot extract filename from '{}'", target.display()), None as Option<String>)
        })?;
        validate_project_name(&leaf)?;

        let parent_canonical = parent.canonicalize().map_err(MfError::Io)?;

        if !parent_canonical.starts_with(&root_canonical) {
            return Err(MfError::usage(
                format!("path '{}' is outside the Mind Repo root '{}'", target.display(), root_canonical.display(),),
                Some("try --project <NAME> or --root <PATH>".to_string()),
            ));
        }
        Ok(parent_canonical.join(&leaf))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pending_file_rename_restores_replaced_target_on_rollback() {
        let dir = tempfile::tempdir().unwrap();
        let old = dir.path().join("old.md");
        let new = dir.path().join("new.md");
        std::fs::write(&old, "source contents").unwrap();
        std::fs::write(&new, "previous destination").unwrap();

        PendingFileRename::begin(&old, &new).unwrap().rollback().unwrap();

        assert_eq!(std::fs::read_to_string(old).unwrap(), "source contents");
        assert_eq!(std::fs::read_to_string(new).unwrap(), "previous destination");
    }

    // --- to_filename ---

    #[test]
    fn test_to_filename_ascii_unchanged() {
        assert_eq!(to_filename("Hello World"), "hello-world");
        assert_eq!(to_filename("Foo_Bar.Baz"), "foo-bar-baz");
        assert_eq!(to_filename("  --a--  "), "a");
    }

    #[test]
    fn test_to_filename_empty_is_untitled() {
        assert_eq!(to_filename("!!!"), "untitled");
        assert_eq!(to_filename(""), "untitled");
    }

    #[test]
    fn test_to_filename_preserves_cjk() {
        // CJK headings must produce distinct, readable slugs (Bug #10 residual).
        assert_eq!(to_filename("本周进展"), "本周进展");
        assert_eq!(to_filename("里程碑规划与进展"), "里程碑规划与进展");
        assert_ne!(to_filename("本周进展"), to_filename("里程碑规划与进展"));
        // Mixed CJK + ASCII keeps both, punctuation becomes a separator.
        assert_eq!(to_filename("Agent 建站周报"), "agent-建站周报");
    }

    // --- validate_schema_version & schema alias ---

    #[test]
    fn test_validate_schema_version_1_is_compatible() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.yaml");
        assert!(validate_schema_version("1", &path).is_ok());
    }

    #[test]
    fn test_validate_schema_version_unknown_is_incompatible() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.yaml");
        let err = validate_schema_version("2", &path).unwrap_err();
        assert!(matches!(err, MfError::IncompatibleSchema { .. }));
    }

    // --- validate_project_name ---

    #[test]
    fn test_validate_name_empty() {
        let err = validate_project_name("").unwrap_err();
        assert!(err.to_string().contains("empty"));
    }

    #[test]
    fn test_validate_name_dot() {
        assert!(validate_project_name(".").is_err());
    }

    #[test]
    fn test_validate_name_dotdot() {
        assert!(validate_project_name("..").is_err());
    }

    #[test]
    fn test_validate_name_contains_slash() {
        assert!(validate_project_name("foo/bar").is_err());
    }

    #[test]
    fn test_validate_name_contains_backslash() {
        assert!(validate_project_name("foo\\bar").is_err());
    }

    #[test]
    fn test_validate_name_uppercase() {
        assert!(validate_project_name("MyProject").is_err());
    }

    #[test]
    fn test_validate_name_valid_kebab() {
        assert!(validate_project_name("my-project").is_ok());
    }

    #[test]
    fn test_validate_name_valid_alphanumeric() {
        assert!(validate_project_name("project42").is_ok());
    }

    #[test]
    fn test_validate_name_with_underscore() {
        assert!(validate_project_name("my_project").is_err());
    }

    // --- canonicalize_within ---

    #[test]
    fn test_canonicalize_within_existing_target() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("repo");
        std::fs::create_dir_all(&root).unwrap();
        let sub = root.join("subdir");
        std::fs::create_dir_all(&sub).unwrap();

        let result = canonicalize_within(&root, &sub).unwrap();
        assert!(result.starts_with(root.canonicalize().unwrap()));
    }

    #[test]
    fn test_canonicalize_within_non_existent_leaf() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("repo");
        std::fs::create_dir_all(&root).unwrap();
        let target = root.join("new-project");

        let result = canonicalize_within(&root, &target).unwrap();
        assert!(result.starts_with(root.canonicalize().unwrap()));
    }

    #[test]
    fn test_canonicalize_within_escape_via_dotdot() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("repo");
        std::fs::create_dir_all(&root).unwrap();
        let target = root.join("../outside");

        let err = canonicalize_within(&root, &target).unwrap_err();
        assert!(err.to_string().contains("outside"));
    }

    #[test]
    fn test_canonicalize_within_symlink_escape() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("repo");
        std::fs::create_dir_all(&root).unwrap();
        let outside = dir.path().join("escape");
        std::fs::create_dir_all(&outside).unwrap();
        // Symlink inside repo pointing outside
        let link = root.join("evil-link");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&outside, &link).unwrap();

        let result = canonicalize_within(&root, &link);
        // On platforms where symlinks resolve, this should be an error
        // (the symlink target is outside the repo root)
        if let Ok(canonical) = result {
            // If it resolved inside the repo root it's fine
            assert!(canonical.starts_with(root.canonicalize().unwrap()));
        }
    }

    #[test]
    fn test_canonicalize_within_root_not_found() {
        let dir = tempfile::tempdir().unwrap();
        let nonexistent_root = dir.path().join("nonexistent");
        let target = dir.path().join("somewhere");

        let err = canonicalize_within(&nonexistent_root, &target).unwrap_err();
        assert!(matches!(err, MfError::Io(_)));
    }

    #[test]
    fn test_canonicalize_within_invalid_leaf_name() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("repo");
        std::fs::create_dir_all(&root).unwrap();
        let target = root.join("../escape");

        let err = canonicalize_within(&root, &target).unwrap_err();
        assert!(err.to_string().contains("invalid") || err.to_string().contains("outside"));
    }

    // --- atomic_write_directory ---

    #[test]
    fn atomic_write_directory_happy_path() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("my-article");

        let files: Vec<(&str, &str)> = vec![("01-opening.md", "# Title\n"), ("02-summary.md", "## Summary\nBody\n")];
        atomic_write_directory(&target, &files).unwrap();

        assert!(target.is_dir());
        assert_eq!(std::fs::read_to_string(target.join("01-opening.md")).unwrap(), "# Title\n");
        assert_eq!(std::fs::read_to_string(target.join("02-summary.md")).unwrap(), "## Summary\nBody\n");
    }

    #[test]
    fn atomic_write_directory_rollback_on_mid_write_failure() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("my-article");

        // Use an invalid filename to trigger a write error
        let result = atomic_write_directory(&target, &[("01-opening.md", "# Title\n"), ("bad/\x00name.md", "nope")]);
        assert!(result.is_err());
        // The tmp dir must not have leaked into target
        assert!(!target.exists());
        // No sibling tmp directory left behind
        let siblings: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().starts_with('.'))
            .collect();
        assert!(siblings.is_empty(), "no tmp dir left behind: {:?}", siblings);
    }

    #[test]
    fn atomic_write_directory_target_already_exists() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("existing");
        std::fs::create_dir(&target).unwrap();

        // Rename onto an existing directory fails on most platforms
        let result = atomic_write_directory(&target, &[("01-opening.md", "# Title\n")]);
        // The call may succeed (rename replaces on some fs) or fail; either is
        // acceptable. The important invariant is no tmp dir leak.
        let siblings: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().starts_with('.'))
            .collect();
        assert!(siblings.is_empty(), "no tmp dir left behind");
        let _ = result;
    }

    #[test]
    fn atomic_write_dry_run_does_not_create_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("new.yaml");
        assert!(atomic_write_dry_run(&path, "value: true\n", true).unwrap());
        assert!(!path.exists());
    }

    #[test]
    fn atomic_write_directory_dry_run_does_not_create_directory() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("article");
        assert!(atomic_write_directory_dry_run(&target, &[("01.md", "body\n")], true).unwrap());
        assert!(!target.exists());
    }
}
