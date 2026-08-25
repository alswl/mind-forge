// Correction subresource service — add, list, show, update, remove corrections
// within a term.
//
// Delegates to project-scoped index or global term storage per scope.

use std::path::Path;

use crate::error::{MfError, Result};
use crate::model::term::{Boundary, Correction, FixKind, MatchKind, validate_corrections};
use crate::service::index;
use crate::service::term::{find_correction_index, sort_terms_by_name};

fn validate_terms_before_project_save(terms: &[crate::model::term::Term]) -> Result<()> {
    validate_corrections(terms).map_err(|msg| MfError::usage(msg, None::<String>))
}

fn validate_terms_before_global_save(terms: &[crate::model::term::Term]) -> Result<()> {
    validate_corrections(terms).map_err(|msg| MfError::usage(msg, None::<String>))
}

/// Spec 075 US5/FR-030: `a` and `b` are in a prefix-or-equal relationship,
/// case-insensitively for ASCII. Used to detect cross-term shadowing — a new
/// correction's `original` that is a prefix of (or equal to) another term's
/// name or registered original will have lint map that occurrence to the
/// *other* term instead, silently misdirecting the correction.
fn is_prefix_or_equal_ci(a: &str, b: &str) -> bool {
    let a = a.to_ascii_lowercase();
    let b = b.to_ascii_lowercase();
    a == b || a.starts_with(&b) || b.starts_with(&a)
}

/// Find another term (by name) whose name or registered original is in a
/// prefix-or-equal relationship with `original`. `excluding_term` is the term
/// the new correction is being added to, which is never itself a collision.
fn find_shadowing_conflict<'a>(
    terms: &'a [crate::model::term::Term],
    excluding_term: &str,
    original: &str,
) -> Option<&'a str> {
    for t in terms {
        if t.term == excluding_term {
            continue;
        }
        if is_prefix_or_equal_ci(original, &t.term) {
            return Some(&t.term);
        }
        if t.corrections.iter().any(|c| is_prefix_or_equal_ci(original, &c.original)) {
            return Some(&t.term);
        }
    }
    None
}

fn shadowing_warning(original: &str, colliding_term: &str) -> String {
    // The relationship is symmetric (either string may be the prefix), so the
    // message must not claim a direction it did not check.
    format!(
        "original '{original}' overlaps term '{colliding_term}' (one is a prefix of the other); lint may map it to that term instead"
    )
}

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
    dry_run: bool,
) -> Result<(Correction, bool, Option<String>)> {
    let mut index = index::load(project_root)?;
    let terms = index.terms.as_mut().ok_or_else(|| {
        MfError::not_found(
            format!("term '{term_name}' not found"),
            Some("use `mf term list` or `mf term new`".to_string()),
        )
    })?;

    if !terms.iter().any(|t| t.term == term_name) {
        return Err(MfError::not_found(
            format!("term '{term_name}' not found"),
            Some("use `mf term list` or `mf term new`".to_string()),
        ));
    }

    // Idempotent: return the existing entry when the identical pair is present.
    let t = terms.iter().find(|t| t.term == term_name).expect("checked above");
    if let Some(existing) = t.corrections.iter().find(|c| c.original == original && c.correct == correct) {
        return Ok((existing.clone(), false, None));
    }

    let corr = Correction {
        original: original.to_string(),
        correct: correct.to_string(),
        r#match: match_kind.unwrap_or_default(),
        fix: fix_kind.unwrap_or_default(),
        boundary: boundary.unwrap_or_default(),
        pinyin: pinyin.unwrap_or(None),
    };

    // Spec 075 US5/FR-030: warn on cross-term shadowing, but still register —
    // this is a warn-and-proceed check, not a blocking one (D9).
    let warning = find_shadowing_conflict(terms, term_name, original).map(|other| shadowing_warning(original, other));

    let t = terms.iter_mut().find(|t| t.term == term_name).expect("checked above");
    t.corrections.push(corr.clone());
    sort_terms_by_name(terms);
    validate_terms_before_project_save(terms)?;
    if !dry_run {
        index::save(project_root, &index)?;
    }
    Ok((corr, true, warning))
}

/// List all corrections for a project-scoped term.
pub fn list_corrections(project_root: &Path, term_name: &str) -> Result<Vec<Correction>> {
    let index = index::load_lenient(project_root)?;
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
    dry_run: bool,
) -> Result<Correction> {
    let mut index = index::load_lenient(project_root)?;
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
    validate_terms_before_project_save(terms)?;
    if !dry_run {
        index::save(project_root, &index)?;
    }
    Ok(result)
}

/// Remove a correction from a project-scoped term.
pub fn remove_correction(project_root: &Path, term_name: &str, original: &str) -> Result<Correction> {
    let mut index = index::load_lenient(project_root)?;
    let terms =
        index.terms.as_mut().ok_or_else(|| MfError::not_found(format!("term '{term_name}' not found"), None))?;

    let t = terms
        .iter_mut()
        .find(|t| t.term == term_name)
        .ok_or_else(|| MfError::not_found(format!("term '{term_name}' not found"), None))?;

    let idx = find_correction_index(t, original)?;
    let removed = t.corrections.remove(idx);
    sort_terms_by_name(terms);
    validate_terms_before_project_save(terms)?;
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
    dry_run: bool,
) -> Result<(Correction, bool, Option<String>)> {
    let mut terms = crate::service::term::global::load_terms(repo_root)?;
    if !terms.iter().any(|t| t.term == term_name) {
        return Err(MfError::not_found(
            format!("term '{term_name}' not found"),
            Some("use `mf term list` or `mf term new`".to_string()),
        ));
    }

    // Idempotent: return the existing entry when the identical pair is present.
    let t = terms.iter().find(|t| t.term == term_name).expect("checked above");
    if let Some(existing) = t.corrections.iter().find(|c| c.original == original && c.correct == correct) {
        return Ok((existing.clone(), false, None));
    }

    let corr = Correction {
        original: original.to_string(),
        correct: correct.to_string(),
        r#match: match_kind.unwrap_or_default(),
        fix: fix_kind.unwrap_or_default(),
        boundary: boundary.unwrap_or_default(),
        pinyin: pinyin.unwrap_or(None),
    };

    // Spec 075 US5/FR-030: warn on cross-term shadowing, but still register.
    let warning = find_shadowing_conflict(&terms, term_name, original).map(|other| shadowing_warning(original, other));

    let t = terms.iter_mut().find(|t| t.term == term_name).expect("checked above");
    t.corrections.push(corr.clone());
    sort_terms_by_name(&mut terms);
    validate_terms_before_global_save(&terms)?;
    if !dry_run {
        crate::service::term::global::save_terms(repo_root, &terms)?;
    }
    Ok((corr, true, warning))
}

/// List all corrections for a global-scoped term.
pub fn list_corrections_global(repo_root: &Path, term_name: &str) -> Result<Vec<Correction>> {
    let terms = crate::service::term::repo_format::load_lenient(repo_root)?;
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
    dry_run: bool,
) -> Result<Correction> {
    let mut terms = crate::service::term::repo_format::load_lenient(repo_root)?;
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
    validate_terms_before_global_save(&terms)?;
    if !dry_run {
        crate::service::term::global::save_terms(repo_root, &terms)?;
    }
    Ok(result)
}

/// Remove a correction from a global-scoped term.
pub fn remove_correction_global(repo_root: &Path, term_name: &str, original: &str) -> Result<Correction> {
    let mut terms = crate::service::term::repo_format::load_lenient(repo_root)?;
    let t = terms
        .iter_mut()
        .find(|t| t.term == term_name)
        .ok_or_else(|| MfError::not_found(format!("term '{term_name}' not found"), None))?;

    let idx = find_correction_index(t, original)?;
    let removed = t.corrections.remove(idx);
    sort_terms_by_name(&mut terms);
    validate_terms_before_global_save(&terms)?;
    crate::service::term::global::save_terms(repo_root, &terms)?;
    Ok(removed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::term::Term;

    fn term(name: &str, originals: &[&str]) -> Term {
        Term {
            term: name.to_string(),
            definition: None,
            description: None,
            confidence: None,
            aliases: Vec::new(),
            tags: Vec::new(),
            corrections: originals.iter().map(|o| Correction::misrecognition(*o, format!("{o}-fixed"))).collect(),
        }
    }

    #[test]
    fn shadowing_conflict_is_found_in_both_prefix_directions() {
        let terms = vec![term("API", &[])];
        // The new original extends the other term's name...
        assert_eq!(find_shadowing_conflict(&terms, "gateway", "API网关"), Some("API"));
        // ...and the other term's name extends the new original.
        let terms = vec![term("API网关", &[])];
        assert_eq!(find_shadowing_conflict(&terms, "gateway", "API"), Some("API网关"));
    }

    #[test]
    fn shadowing_warning_does_not_claim_a_direction_it_did_not_check() {
        // `is_prefix_or_equal_ci` matches either way round, so the message must
        // state the relationship symmetrically: here the *term* is the prefix.
        let msg = shadowing_warning("API网关", "API");
        assert!(msg.contains("API网关"), "got {msg}");
        assert!(msg.contains("term 'API'"), "got {msg}");
        assert!(!msg.contains("is also a prefix of term"), "got {msg}");
        assert!(msg.contains("one is a prefix of the other"), "got {msg}");
    }

    #[test]
    fn the_term_being_edited_is_never_its_own_conflict() {
        let terms = vec![term("API", &["API网关"])];
        assert_eq!(find_shadowing_conflict(&terms, "API", "API网关"), None);
    }
}
