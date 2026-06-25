// Term move service — relocate terms between project and global scopes.
//
// Supports: project→global, global→project, project→project.

use std::path::Path;

use crate::error::{MfError, Result};
use crate::model::term::{MoveSideEffect, Term};
use crate::service::index;
use crate::service::term::{self, sort_terms_by_name};

/// Result of a term move operation.
pub struct MoveOutcome {
    pub term: Term,
    pub from_scope: String,
    pub to_scope: String,
    pub side_effects: Vec<MoveSideEffect>,
}

/// Move a term from a project scope to the global scope.
pub fn move_project_to_global(
    project_root: &Path,
    repo_root: &Path,
    term_name: &str,
    force: bool,
) -> Result<MoveOutcome> {
    let mut index = index::load(project_root)?;
    let terms =
        index.terms.as_mut().ok_or_else(|| MfError::not_found(format!("term '{term_name}' not found"), None))?;

    let pos = terms
        .iter()
        .position(|t| t.term == term_name)
        .ok_or_else(|| MfError::not_found(format!("term '{term_name}' not found in project"), None))?;

    let removed = terms.remove(pos);
    sort_terms_by_name(terms);
    index::save(project_root, &index)?;

    // Add to global — with conflict check
    let mut global_terms = term::global::load_terms(repo_root)?;
    if !force && global_terms.iter().any(|t| t.term == term_name) {
        return Err(MfError::usage(
            format!("term '{term_name}' already exists in global scope; use --force to overwrite"),
            None,
        ));
    }
    // Remove existing global copy if forcing
    if force {
        global_terms.retain(|t| t.term != term_name);
    }
    global_terms.push(removed.clone());
    sort_terms_by_name(&mut global_terms);
    term::global::save_terms(repo_root, &global_terms)?;

    let side_effects = vec![
        MoveSideEffect {
            action: "remove".into(),
            scope: "project".into(),
            description: "removed from project".to_string(),
        },
        MoveSideEffect {
            action: "add".into(),
            scope: "global".into(),
            description: "added to global scope".to_string(),
        },
    ];

    Ok(MoveOutcome { term: removed, from_scope: "project".into(), to_scope: "global".into(), side_effects })
}

/// Move a term from the global scope to a project scope.
pub fn move_global_to_project(
    repo_root: &Path,
    project_root: &Path,
    term_name: &str,
    force: bool,
) -> Result<MoveOutcome> {
    let mut global_terms = term::global::load_terms(repo_root)?;
    let pos = global_terms
        .iter()
        .position(|t| t.term == term_name)
        .ok_or_else(|| MfError::not_found(format!("term '{term_name}' not found in global scope"), None))?;

    let removed = global_terms.remove(pos);
    sort_terms_by_name(&mut global_terms);
    term::global::save_terms(repo_root, &global_terms)?;

    // Add to project — with conflict check
    let mut index = index::load(project_root)?;
    let dst_terms = index.terms.get_or_insert_with(Vec::new);
    if !force && dst_terms.iter().any(|t| t.term == term_name) {
        return Err(MfError::usage(
            format!("term '{term_name}' already exists in project scope; use --force to overwrite"),
            None,
        ));
    }
    if force {
        dst_terms.retain(|t| t.term != term_name);
    }
    dst_terms.push(removed.clone());
    sort_terms_by_name(dst_terms);
    index::save(project_root, &index)?;

    let side_effects = vec![
        MoveSideEffect {
            action: "remove".into(),
            scope: "global".into(),
            description: "removed from global scope".to_string(),
        },
        MoveSideEffect { action: "add".into(), scope: "project".into(), description: "added to project".to_string() },
    ];

    Ok(MoveOutcome { term: removed, from_scope: "global".into(), to_scope: "project".into(), side_effects })
}

/// Move a term from one project to another.
pub fn move_project_to_project(src_root: &Path, dst_root: &Path, term_name: &str, force: bool) -> Result<MoveOutcome> {
    // Remove from source
    let mut src_index = index::load(src_root)?;
    let src_terms =
        src_index.terms.as_mut().ok_or_else(|| MfError::not_found(format!("term '{term_name}' not found"), None))?;
    let pos = src_terms
        .iter()
        .position(|t| t.term == term_name)
        .ok_or_else(|| MfError::not_found(format!("term '{term_name}' not found in source project"), None))?;
    let removed = src_terms.remove(pos);
    sort_terms_by_name(src_terms);
    index::save(src_root, &src_index)?;

    // Add to destination
    let mut dst_index = index::load(dst_root)?;
    let dst_terms = dst_index.terms.get_or_insert_with(Vec::new);
    if !force && dst_terms.iter().any(|t| t.term == term_name) {
        // Rollback: restore source term
        let mut src_index2 = index::load(src_root)?;
        src_index2.terms.get_or_insert_with(Vec::new).push(removed.clone());
        sort_terms_by_name(src_index2.terms.as_mut().unwrap());
        index::save(src_root, &src_index2)?;
        return Err(MfError::usage(
            format!("term '{term_name}' already exists in destination project; use --force to overwrite"),
            None,
        ));
    }
    if force {
        dst_terms.retain(|t| t.term != term_name);
    }
    dst_terms.push(removed.clone());
    sort_terms_by_name(dst_terms);
    index::save(dst_root, &dst_index)?;

    let side_effects = vec![
        MoveSideEffect {
            action: "remove".into(),
            scope: "project".into(),
            description: "removed from source project".into(),
        },
        MoveSideEffect {
            action: "add".into(),
            scope: "project".into(),
            description: "added to destination project".into(),
        },
    ];

    Ok(MoveOutcome { term: removed, from_scope: "project".into(), to_scope: "project".into(), side_effects })
}
