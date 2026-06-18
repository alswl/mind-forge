use std::path::Path;

use super::{find_term_by_correct, sort_terms_by_name};
use crate::error::{MfError, Result};
use crate::model::term::{Correction, FixKind, MatchKind, Term};
use crate::service::index;

/// Register a correction for an existing term.
pub fn learn_correction(project_root: &Path, original: &str, correct: &str) -> Result<(Term, bool)> {
    if original.trim().is_empty() {
        return Err(MfError::usage("--original cannot be empty", None));
    }
    if correct.trim().is_empty() {
        return Err(MfError::usage("--correct cannot be empty", None));
    }

    let mut index = index::load(project_root)?;
    let idx = find_term_by_correct(index.terms.as_deref().unwrap_or_default(), correct)?;
    let term_clone = {
        let terms = index
            .terms
            .as_mut()
            .ok_or_else(|| MfError::internal("index has no terms array despite successful find_term_by_correct"))?;
        let term = &mut terms[idx];

        // Check original doesn't equal term or any alias
        if original == term.term || term.aliases.iter().any(|a| a == original) {
            return Err(MfError::usage(
                format!("'{original}' is already a recognized form for term '{}'", term.term),
                None,
            ));
        }

        // Check idempotent
        let already_exists = term.corrections.iter().any(|c| c.original == original && c.correct == correct);
        if already_exists {
            return Ok((term.clone(), false));
        }

        term.corrections.push(Correction {
            original: original.to_string(),
            correct: correct.to_string(),
            r#match: MatchKind::Word,
            fix: FixKind::Required,
            pinyin: None,
        });
        term.clone()
    };

    // Sort terms before save (defensive)
    if let Some(ref mut terms) = index.terms {
        sort_terms_by_name(terms);
    }

    index::save(project_root, &index)?;
    Ok((term_clone, true))
}
