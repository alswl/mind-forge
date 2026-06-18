use std::path::Path;

use super::{sort_terms_by_name, TermUpdate};
use crate::error::{MfError, Result};
use crate::model::term::Term;
use crate::service::index;

/// Modify an existing term's definition, aliases, tags, description, or confidence.
pub fn fix_term(project_root: &Path, term_name: &str, update: TermUpdate<'_>) -> Result<Term> {
    if !update.has_legacy_flags()
        && !update.has_metadata_flags()
        && update.delete_aliases.is_empty()
        && update.delete_tags.is_empty()
        && update.delete_corrections.is_empty()
        && update.correction_match.is_empty()
        && update.correction_fix.is_empty()
        && update.correction_pinyin.is_empty()
        && update.correction_boundary.is_empty()
    {
        return Err(MfError::usage(
            "at least one of --definition, --description, --confidence, --alias, --tag, --delete-alias, --delete-tag, --delete-correction, --correction-match, --correction-fix, --correction-pinyin, --correction-boundary, --clear-description, --clear-confidence must be provided",
            None,
        ));
    }

    super::validate_confidence(update.confidence)?;

    let mut index = index::load(project_root)?;

    let term_clone = {
        let terms = index.terms.as_mut().ok_or_else(|| {
            MfError::not_found(
                format!("term '{term_name}' not found"),
                Some("use `mf term list` or `mf term new`".to_string()),
            )
        })?;

        let pos = terms.iter().position(|t| t.term == term_name).ok_or_else(|| {
            MfError::not_found(
                format!("term '{term_name}' not found"),
                Some("use `mf term list` or `mf term new`".to_string()),
            )
        })?;

        if terms.iter().filter(|t| t.term == term_name).count() > 1 {
            return Err(MfError::IncompatibleSchema {
                path: project_root.join("mind-index.yaml"),
                found: format!("duplicate term '{term_name}'"),
                expected: vec!["unique terms".to_string()],
            });
        }

        let t = &mut terms[pos];
        apply_update(t, &update);
        t.clone()
    };

    if let Some(ref mut terms) = index.terms {
        sort_terms_by_name(terms);
    }
    index::save(project_root, &index)?;

    Ok(term_clone)
}

/// Apply a `TermUpdate` to a term entry in place.
pub(crate) fn apply_update(t: &mut Term, update: &TermUpdate<'_>) {
    if let Some(def) = update.definition {
        t.definition = if def.is_empty() { None } else { Some(def.to_string()) };
    }
    if update.clear_description {
        t.description = None;
    } else if let Some(d) = update.description {
        t.description = if d.is_empty() { None } else { Some(d.to_string()) };
    }
    if update.clear_confidence {
        t.confidence = None;
    } else if let Some(c) = update.confidence {
        t.confidence = Some(c);
    }
    for alias in update.aliases {
        if !t.aliases.contains(alias) {
            t.aliases.push(alias.clone());
        }
    }
    for alias in update.delete_aliases {
        t.aliases.retain(|a| a != alias);
    }
    for tag in update.tags {
        if !t.tags.contains(tag) {
            t.tags.push(tag.clone());
        }
    }
    for tag in update.delete_tags {
        t.tags.retain(|t| t != tag);
    }
    for original in update.delete_corrections {
        t.corrections.retain(|c| &c.original != original);
    }
    for (original, mk) in update.correction_match {
        if let Some(c) = t.corrections.iter_mut().find(|c| c.original == *original) {
            c.r#match = *mk;
        }
    }
    for (original, fk) in update.correction_fix {
        if let Some(c) = t.corrections.iter_mut().find(|c| c.original == *original) {
            c.fix = *fk;
        }
    }
    for (original, pinyin) in update.correction_pinyin {
        if let Some(c) = t.corrections.iter_mut().find(|c| c.original == *original) {
            c.pinyin = if pinyin.is_empty() { None } else { Some(pinyin.clone()) };
        }
    }
    for (original, boundary) in update.correction_boundary {
        if let Some(c) = t.corrections.iter_mut().find(|c| c.original == *original) {
            c.boundary = *boundary;
        }
    }
}
