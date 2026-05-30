use std::path::Path;

use super::{sort_terms_by_name, TermUpdate};
use crate::error::{MfError, Result};
use crate::model::term::Term;
use crate::service::index;

/// Modify an existing term's definition, aliases, tags, description, or confidence.
pub fn fix_term(project_root: &Path, term_name: &str, update: TermUpdate<'_>) -> Result<Term> {
    if !update.has_legacy_flags() && !update.has_metadata_flags() {
        return Err(MfError::usage(
            "at least one of --definition, --description, --confidence, --alias, --tag, --clear-description, --clear-confidence must be provided",
            None,
        ));
    }

    super::validate_confidence(update.confidence)?;

    let mut index = index::load(project_root)?;

    let term_clone = {
        let terms = index.terms.as_mut().ok_or_else(|| {
            MfError::usage(
                format!("term '{term_name}' not found"),
                Some("use `mf term list` or `mf term new`".to_string()),
            )
        })?;

        let pos = terms.iter().position(|t| t.term == term_name).ok_or_else(|| {
            MfError::usage(
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
    for tag in update.tags {
        if !t.tags.contains(tag) {
            t.tags.push(tag.clone());
        }
    }
}
