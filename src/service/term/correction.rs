// Correction subresource service — add, list, show, update, remove corrections
// within a term.
//
// Delegates to project-scoped index or global term storage per scope.

use std::path::Path;

use crate::error::{MfError, Result};
use crate::model::term::{Boundary, Correction, FixKind, MatchKind};
use crate::service::index;
use crate::service::term::{find_correction_index, sort_terms_by_name};

// ── Project-scoped correction operations ──────────────────────────────────────

/// Add a correction to an existing project-scoped term.
/// Idempotent: returns Ok if the identical (original, correct) pair already exists.
#[allow(clippy::too_many_arguments)]
pub fn add_correction(
    project_root: &Path,
    term_name: &str,
    original: &str,
    correct: &str,
    match_kind: Option<MatchKind>,
    fix_kind: Option<FixKind>,
    boundary: Option<Boundary>,
    pinyin: Option<Option<String>>,
) -> Result<(Correction, bool)> {
    let mut index = index::load(project_root)?;
    let terms = index.terms.as_mut().ok_or_else(|| {
        MfError::not_found(
            format!("term '{term_name}' not found"),
            Some("use `mf term list` or `mf term new`".to_string()),
        )
    })?;

    let t = terms.iter_mut().find(|t| t.term == term_name).ok_or_else(|| {
        MfError::not_found(
            format!("term '{term_name}' not found"),
            Some("use `mf term list` or `mf term new`".to_string()),
        )
    })?;

    // Idempotent: return the existing entry when the identical pair is present.
    if let Some(existing) = t.corrections.iter().find(|c| c.original == original && c.correct == correct) {
        return Ok((existing.clone(), false));
    }

    let corr = Correction {
        original: original.to_string(),
        correct: correct.to_string(),
        r#match: match_kind.unwrap_or_default(),
        fix: fix_kind.unwrap_or_default(),
        boundary: boundary.unwrap_or_default(),
        pinyin: pinyin.unwrap_or(None),
    };

    t.corrections.push(corr.clone());
    sort_terms_by_name(terms);
    index::save(project_root, &index)?;
    Ok((corr, true))
}

/// List all corrections for a project-scoped term.
pub fn list_corrections(project_root: &Path, term_name: &str) -> Result<Vec<Correction>> {
    let index = index::load(project_root)?;
    let terms = index.terms.as_ref().ok_or_else(|| {
        MfError::not_found(
            format!("term '{term_name}' not found"),
            Some("use `mf term list` or `mf term new`".to_string()),
        )
    })?;
    let t = terms.iter().find(|t| t.term == term_name).ok_or_else(|| {
        MfError::not_found(
            format!("term '{term_name}' not found"),
            Some("use `mf term list` or `mf term new`".to_string()),
        )
    })?;
    Ok(t.corrections.clone())
}

/// Show a single correction for a project-scoped term.
pub fn show_correction(project_root: &Path, term_name: &str, original: &str) -> Result<Correction> {
    let terms = list_corrections(project_root, term_name)?;
    terms.into_iter().find(|c| c.original == original).ok_or_else(|| {
        MfError::not_found(
            format!("correction \"{original}\" not found on term \"{term_name}\""),
            Some("use `mf term correction list <TERM>` to see available corrections".to_string()),
        )
    })
}

/// Update attributes on a correction of a project-scoped term.
#[allow(clippy::too_many_arguments)]
pub fn update_correction(
    project_root: &Path,
    term_name: &str,
    original: &str,
    correct: Option<String>,
    match_kind: Option<MatchKind>,
    fix_kind: Option<FixKind>,
    boundary: Option<Boundary>,
    pinyin: Option<Option<String>>,
) -> Result<Correction> {
    let mut index = index::load(project_root)?;
    let terms =
        index.terms.as_mut().ok_or_else(|| MfError::not_found(format!("term '{term_name}' not found"), None))?;

    let t = terms
        .iter_mut()
        .find(|t| t.term == term_name)
        .ok_or_else(|| MfError::not_found(format!("term '{term_name}' not found"), None))?;

    let idx = find_correction_index(t, original)?;
    let c = &mut t.corrections[idx];

    if let Some(v) = correct {
        c.correct = v;
    }
    if let Some(v) = match_kind {
        c.r#match = v;
    }
    if let Some(v) = fix_kind {
        c.fix = v;
    }
    if let Some(v) = boundary {
        c.boundary = v;
    }
    if let Some(v) = pinyin {
        c.pinyin = v;
    }

    let result = c.clone();
    sort_terms_by_name(terms);
    index::save(project_root, &index)?;
    Ok(result)
}

/// Remove a correction from a project-scoped term.
pub fn remove_correction(project_root: &Path, term_name: &str, original: &str) -> Result<Correction> {
    let mut index = index::load(project_root)?;
    let terms =
        index.terms.as_mut().ok_or_else(|| MfError::not_found(format!("term '{term_name}' not found"), None))?;

    let t = terms
        .iter_mut()
        .find(|t| t.term == term_name)
        .ok_or_else(|| MfError::not_found(format!("term '{term_name}' not found"), None))?;

    let idx = find_correction_index(t, original)?;
    let removed = t.corrections.remove(idx);
    sort_terms_by_name(terms);
    index::save(project_root, &index)?;
    Ok(removed)
}

// ── Global-scoped correction operations ───────────────────────────────────────

/// Add a correction to an existing global-scoped term.
#[allow(clippy::too_many_arguments)]
pub fn add_correction_global(
    repo_root: &Path,
    term_name: &str,
    original: &str,
    correct: &str,
    match_kind: Option<MatchKind>,
    fix_kind: Option<FixKind>,
    boundary: Option<Boundary>,
    pinyin: Option<Option<String>>,
) -> Result<(Correction, bool)> {
    let mut terms = crate::service::term::global::load_terms(repo_root)?;
    let t = terms.iter_mut().find(|t| t.term == term_name).ok_or_else(|| {
        MfError::not_found(
            format!("term '{term_name}' not found"),
            Some("use `mf term list` or `mf term new`".to_string()),
        )
    })?;

    // Idempotent: return the existing entry when the identical pair is present.
    if let Some(existing) = t.corrections.iter().find(|c| c.original == original && c.correct == correct) {
        return Ok((existing.clone(), false));
    }

    let corr = Correction {
        original: original.to_string(),
        correct: correct.to_string(),
        r#match: match_kind.unwrap_or_default(),
        fix: fix_kind.unwrap_or_default(),
        boundary: boundary.unwrap_or_default(),
        pinyin: pinyin.unwrap_or(None),
    };

    t.corrections.push(corr.clone());
    sort_terms_by_name(&mut terms);
    crate::service::term::global::save_terms(repo_root, &terms)?;
    Ok((corr, true))
}

/// List all corrections for a global-scoped term.
pub fn list_corrections_global(repo_root: &Path, term_name: &str) -> Result<Vec<Correction>> {
    let terms = crate::service::term::global::load_terms(repo_root)?;
    let t = terms
        .iter()
        .find(|t| t.term == term_name)
        .ok_or_else(|| MfError::not_found(format!("term '{term_name}' not found"), None))?;
    Ok(t.corrections.clone())
}

/// Show a single correction for a global-scoped term.
pub fn show_correction_global(repo_root: &Path, term_name: &str, original: &str) -> Result<Correction> {
    let corrections = list_corrections_global(repo_root, term_name)?;
    corrections
        .into_iter()
        .find(|c| c.original == original)
        .ok_or_else(|| MfError::not_found(format!("correction \"{original}\" not found on term \"{term_name}\""), None))
}

/// Update attributes on a correction of a global-scoped term.
#[allow(clippy::too_many_arguments)]
pub fn update_correction_global(
    repo_root: &Path,
    term_name: &str,
    original: &str,
    correct: Option<String>,
    match_kind: Option<MatchKind>,
    fix_kind: Option<FixKind>,
    boundary: Option<Boundary>,
    pinyin: Option<Option<String>>,
) -> Result<Correction> {
    let mut terms = crate::service::term::global::load_terms(repo_root)?;
    let t = terms
        .iter_mut()
        .find(|t| t.term == term_name)
        .ok_or_else(|| MfError::not_found(format!("term '{term_name}' not found"), None))?;

    let idx = find_correction_index(t, original)?;
    let c = &mut t.corrections[idx];

    if let Some(v) = correct {
        c.correct = v;
    }
    if let Some(v) = match_kind {
        c.r#match = v;
    }
    if let Some(v) = fix_kind {
        c.fix = v;
    }
    if let Some(v) = boundary {
        c.boundary = v;
    }
    if let Some(v) = pinyin {
        c.pinyin = v;
    }

    let result = c.clone();
    sort_terms_by_name(&mut terms);
    crate::service::term::global::save_terms(repo_root, &terms)?;
    Ok(result)
}

/// Remove a correction from a global-scoped term.
pub fn remove_correction_global(repo_root: &Path, term_name: &str, original: &str) -> Result<Correction> {
    let mut terms = crate::service::term::global::load_terms(repo_root)?;
    let t = terms
        .iter_mut()
        .find(|t| t.term == term_name)
        .ok_or_else(|| MfError::not_found(format!("term '{term_name}' not found"), None))?;

    let idx = find_correction_index(t, original)?;
    let removed = t.corrections.remove(idx);
    sort_terms_by_name(&mut terms);
    crate::service::term::global::save_terms(repo_root, &terms)?;
    Ok(removed)
}
