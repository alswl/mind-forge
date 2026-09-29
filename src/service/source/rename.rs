use std::path::Path;

use serde::Serialize;

use crate::error::{MfError, Result};
use crate::model::lifecycle::PlannedChange;
use crate::model::source::FileKind;
use crate::service::{identity, lifecycle, util};

/// Report from a successful source rename.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct SourceRenameReport {
    pub verb: String,
    pub kind: String,
    pub before: SourceRenameIdentity,
    pub after: SourceRenameIdentity,
    #[serde(default)]
    pub references: Vec<crate::model::lifecycle::Reference>,
    #[serde(default)]
    pub side_effects: Vec<PlannedChange>,
    pub force: bool,
    pub dry_run: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct SourceRenameIdentity {
    pub name: String,
    pub path: Option<String>,
    pub url: Option<String>,
    pub file_kind: FileKind,
}

/// Rename a project-scoped source entry.
///
/// spec 082 (#54): `target` is a **path**, resolved cwd-relative or
/// absolute and checked against the project boundary — not a bare name
/// joined onto the old file's parent directory with its extension appended
/// (the historical behavior, which silently produced doubled extensions and
/// literal-dotted registration names for inputs like `renamed.md`). The
/// registration's new `name` is the target's file stem, matching the
/// existing convention that every source's `name` is the stem of its
/// `path`. If the source has an on-disk file, it is moved there; the file
/// move and the index update apply as one unit (spec FR-017) — a failure
/// partway through leaves neither applied.
///
/// A source with no on-disk file (a URL-only registration) has no file to
/// move; only its registered `name` changes, derived the same way from the
/// target's stem.
pub fn rename_source(
    project_path: &Path,
    cwd: &Path,
    old_name: &str,
    target: &str,
    force: bool,
    dry_run: bool,
) -> Result<SourceRenameReport> {
    util::require_nonempty(old_name, "old source name")?;
    util::require_nonempty(target, "rename target")?;

    let mut index = crate::service::index::load(project_path)?;
    let sources = index.sources.as_ref().ok_or_else(|| {
        MfError::not_found(
            format!("source '{old_name}' not found"),
            Some("use `mf source list` to see available sources".to_string()),
        )
    })?;

    let pos = sources.iter().position(|s| s.name == old_name).ok_or_else(|| {
        MfError::not_found(
            format!("source '{old_name}' not found"),
            Some("use `mf source list` to see available sources".to_string()),
        )
    })?;

    // Resolve the target as a path (spec FR-002, FR-012): cwd-relative or
    // absolute, always checked against the project boundary — `source
    // rename` is inherently project-scoped, so this check is unconditional
    // (not gated on whether `--project` was explicitly passed; by the time
    // this function runs, `project_path` is already resolved either way).
    let resolved_target = identity::resolve_path_selector(target, cwd, Some(project_path), "project")?;

    let new_name = resolved_target
        .file_stem()
        .and_then(|s| s.to_str())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| {
            MfError::usage(
                format!("target '{target}' has no valid file name"),
                Some("provide a path ending in a file name".to_string()),
            )
        })?
        .to_string();

    // Name-uniqueness in the index (pre-existing invariant, independent of
    // whether the target file path also collides — see the file-exists
    // check below).
    if old_name != new_name && sources.iter().any(|s| s.name == new_name) && !force {
        return Err(MfError::usage(
            format!("a source named '{new_name}' already exists"),
            Some("use --force to overwrite".to_string()),
        ));
    }

    let entry = &sources[pos];
    let before = SourceRenameIdentity {
        name: old_name.to_string(),
        path: entry.path.clone(),
        url: entry.url.clone(),
        file_kind: entry.kind.clone(),
    };

    // `resolved_target` is canonical (resolve_path_selector canonicalizes
    // internally); `project_path` may not be, if this function was called
    // directly rather than through the CLI's own `--root` canonicalization
    // — canonicalize it here too, or the prefix strip below silently fails
    // and leaks an absolute path into the stored, supposedly-relative field.
    let new_rel_path = util::repo_relative_path(&util::try_canonicalize(project_path), &resolved_target);
    let has_file = entry.path.is_some();
    let after = SourceRenameIdentity {
        name: new_name.clone(),
        path: if has_file { Some(new_rel_path.clone()) } else { None },
        url: entry.url.clone(),
        file_kind: entry.kind.clone(),
    };

    let mut planned = Vec::new();
    planned.push(lifecycle::planned_yaml_update(
        &project_path.join("mind-index.yaml").to_string_lossy(),
        Some(old_name),
        Some(&new_name),
    ));

    // Validate BEFORE any mutation (spec FR-015): the target's parent
    // directory must already exist, and — unless --force — the target must
    // not already be occupied by a different file. No directory is created
    // implicitly.
    let old_full = entry.path.as_ref().map(|p| project_path.join(p));
    if let Some(old_full) = &old_full
        && old_full.exists()
    {
        let parent = resolved_target.parent().ok_or_else(|| {
            MfError::usage(format!("cannot determine parent of target '{target}'"), None as Option<String>)
        })?;
        if !parent.exists() {
            let shown = util::repo_relative_path(&util::try_canonicalize(project_path), parent);
            return Err(MfError::not_found(
                format!("target directory does not exist: '{shown}'"),
                Some("create the directory first, or choose an existing one".to_string()),
            ));
        }
        if resolved_target.exists() && &resolved_target != old_full && !force {
            return Err(MfError::file_exists(resolved_target.clone()));
        }
        planned.push(PlannedChange {
            op: crate::model::lifecycle::PlannedOp::RenameFile,
            path: old_full.to_string_lossy().to_string(),
            old: Some(old_name.to_string()),
            new: Some(new_name.clone()),
        });
    }

    if dry_run {
        return Ok(SourceRenameReport {
            verb: "rename".into(),
            kind: "source".into(),
            before,
            after,
            references: vec![],
            side_effects: planned,
            force,
            dry_run: true,
        });
    }

    // Execute: move the file first (if any), then save the index. If the
    // index save fails, roll back the file move so neither half of the
    // operation is left applied on its own (spec FR-017).
    let mut pending_move = None;
    if let Some(old_full) = &old_full
        && old_full.exists()
        && &resolved_target != old_full
    {
        pending_move = Some(util::PendingFileRename::begin(old_full, &resolved_target)?);
    }

    let sources_mut = index.sources.as_mut().unwrap();
    let entry_mut = &mut sources_mut[pos];
    entry_mut.name = new_name.clone();
    if has_file {
        entry_mut.path = Some(new_rel_path.clone());
    }

    if let Err(error) = crate::service::index::save(project_path, &index) {
        if let Some(pending_move) = pending_move {
            let _ = pending_move.rollback();
        }
        return Err(error);
    }
    if let Some(pending_move) = pending_move {
        pending_move.commit();
    }

    Ok(SourceRenameReport {
        verb: "rename".into(),
        kind: "source".into(),
        before,
        after,
        references: vec![],
        side_effects: planned,
        force,
        dry_run: false,
    })
}
