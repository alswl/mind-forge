//! Shared lifecycle helpers: reference scanning, planned-change aggregation,
//! scope identity, and dry-run guards.

use std::path::Path;

use crate::error::{MfError, Result};
use crate::model::index::IndexFile;
use crate::model::lifecycle::{ObjectKind, PlannedChange, PlannedOp, Reference, ReferenceKind, ScopeRef};

/// Scan `mind-index.yaml` for references to a target object.
///
/// For assets, this also does a content scan of article files (matching the
/// existing `asset remove` precedent). For all other kinds, only the index
/// is consulted.
pub fn scan_references(
    project_path: &Path,
    index: &IndexFile,
    target_kind: ObjectKind,
    target_id: &str,
) -> Vec<Reference> {
    let mut refs = Vec::new();

    // Check if this object is referenced in any article's index entry
    if let Some(articles) = &index.articles {
        for article in articles {
            let from_path = Some(article.article_path.clone());
            let from_id = article.title.clone();

            // Index-level reference: the article exists in the index and is in this project
            refs.push(Reference {
                from_kind: ObjectKind::Article,
                from_id: from_id.clone(),
                from_path: from_path.clone(),
                line: None,
                kind: ReferenceKind::Index,
            });

            // For assets only: body-scan article files (matching asset remove precedent)
            if matches!(target_kind, ObjectKind::Asset) {
                let abs_path = project_path.join(&article.article_path);
                if abs_path.exists()
                    && let Ok(content) = std::fs::read_to_string(&abs_path)
                    && content.contains(target_id)
                {
                    refs.push(Reference {
                        from_kind: ObjectKind::Article,
                        from_id,
                        from_path,
                        line: None,
                        kind: ReferenceKind::Mention,
                    });
                }
            }
        }
    }

    // Check sources index for cross-references
    if let Some(sources) = &index.sources {
        for source in sources {
            if matches!(target_kind, ObjectKind::Source)
                && (source.name == target_id || source.path.as_deref() == Some(target_id))
            {
                continue; // self-reference
            }
            // Minimal: sources don't reference other objects in current index schema
        }
    }

    refs
}

/// Build a `PlannedChange` for an index refresh.
pub fn planned_index_refresh(index_path: &str) -> PlannedChange {
    PlannedChange { op: PlannedOp::RefreshIndex, path: index_path.to_string(), old: None, new: None }
}

/// Build a `PlannedChange` for a YAML update.
pub fn planned_yaml_update(path: &str, old: Option<&str>, new: Option<&str>) -> PlannedChange {
    PlannedChange {
        op: PlannedOp::UpdateYaml,
        path: path.to_string(),
        old: old.map(|s| s.to_string()),
        new: new.map(|s| s.to_string()),
    }
}

/// Build a `PlannedChange` for a file removal.
pub fn planned_remove_file(path: &str) -> PlannedChange {
    PlannedChange { op: PlannedOp::RemoveFile, path: path.to_string(), old: None, new: None }
}

/// Build a `PlannedChange` for a directory removal.
#[allow(dead_code)] // consumed by US3 move
pub fn planned_remove_dir(path: &str) -> PlannedChange {
    PlannedChange { op: PlannedOp::RemoveDir, path: path.to_string(), old: None, new: None }
}

/// Build a `PlannedChange` for a file rename.
#[allow(dead_code)] // consumed by US3 move
pub fn planned_rename_file(path: &str, old: &str, new: &str) -> PlannedChange {
    PlannedChange {
        op: PlannedOp::RenameFile,
        path: path.to_string(),
        old: Some(old.to_string()),
        new: Some(new.to_string()),
    }
}

/// Dry-run guard: if `dry_run` is true, return the planned changes without
/// performing any filesystem mutations. Callers should construct the full
/// plan, then call this guard.
#[allow(dead_code)] // consumed by US3 move
pub fn dry_run_guard(dry_run: bool, planned: Vec<PlannedChange>) -> Result<Option<Vec<PlannedChange>>> {
    if dry_run { Ok(Some(planned)) } else { Ok(None) }
}

/// Resolve the scope for an operation based on project path and global flag.
pub fn resolve_scope(project_path: Option<&Path>, global: bool) -> Result<ScopeRef> {
    match (project_path, global) {
        (Some(p), false) => {
            let name = p.file_name().and_then(|n| n.to_str()).map(|s| s.to_string());
            Ok(ScopeRef { project: name, global: false })
        }
        (None, true) => Ok(ScopeRef { project: None, global: true }),
        (Some(_), true) => Err(MfError::usage(
            "cannot specify both --project and --global",
            Some("choose either a project scope or global scope".to_string()),
        )),
        (None, false) => Err(MfError::usage(
            "must specify either --project or --global",
            Some("use -p <project> or --global to specify scope".to_string()),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_scope_project() {
        let scope = resolve_scope(Some(Path::new("/repo/projects/alpha")), false).unwrap();
        assert_eq!(scope.project.as_deref(), Some("alpha"));
        assert!(!scope.global);
    }

    #[test]
    fn resolve_scope_global() {
        let scope = resolve_scope(None, true).unwrap();
        assert_eq!(scope.project, None);
        assert!(scope.global);
    }

    #[test]
    fn resolve_scope_both_is_error() {
        assert!(resolve_scope(Some(Path::new("/repo/projects/alpha")), true).is_err());
    }

    #[test]
    fn resolve_scope_neither_is_error() {
        assert!(resolve_scope(None, false).is_err());
    }

    #[test]
    fn dry_run_guard_returns_plan() {
        let plan = vec![planned_index_refresh("projects/foo/mind-index.yaml")];
        let result = dry_run_guard(true, plan.clone()).unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap().len(), 1);
    }

    #[test]
    fn dry_run_guard_off_returns_none() {
        let plan = vec![planned_index_refresh("projects/foo/mind-index.yaml")];
        let result = dry_run_guard(false, plan).unwrap();
        assert!(result.is_none());
    }
}
