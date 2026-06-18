use std::path::Path;

use super::{dedup_preserve_first, sort_terms_by_name, TermInput};
use crate::error::{MfError, Result};
use crate::model::term::Term;
use crate::service::index;

/// Register a new term.
pub fn new_term(project_root: &Path, term: &str, input: TermInput<'_>, misrecognitions: &[String]) -> Result<Term> {
    if term.trim().is_empty() {
        return Err(MfError::usage("term name cannot be empty", None));
    }

    super::validate_confidence(input.confidence)?;

    let mut index = index::load(project_root)?;
    let terms = index.terms.get_or_insert_with(Vec::new);

    // Check for duplicate (strict case-sensitive)
    if terms.iter().any(|t| t.term == term) {
        return Err(MfError::usage(
            format!("term '{term}' already exists"),
            Some("use `mf term fix` to modify the existing term".to_string()),
        ));
    }

    // Dedup before checking alias conflicts
    let aliases = dedup_preserve_first(input.aliases);
    let tags = dedup_preserve_first(input.tags);
    let misrecognitions = dedup_preserve_first(misrecognitions);

    // Check alias uniqueness across existing terms
    for alias in &aliases {
        for t in terms.iter() {
            if t.term == *alias || t.aliases.iter().any(|a| a == alias) {
                return Err(MfError::usage(format!("alias '{alias}' conflicts with existing term '{}'", t.term), None));
            }
        }
    }

    let corrections: Vec<crate::model::term::Correction> = misrecognitions
        .iter()
        .map(|m| crate::model::term::Correction {
            original: m.clone(),
            correct: term.to_string(),
            r#match: crate::model::term::MatchKind::Word,
            fix: crate::model::term::FixKind::Required,
            boundary: crate::model::term::Boundary::Loose,
            pinyin: None,
        })
        .collect();

    let new_entry = Term {
        term: term.to_string(),
        definition: input.definition.and_then(|s| if s.is_empty() { None } else { Some(s.to_string()) }),
        description: input.description.map(String::from),
        confidence: input.confidence,
        aliases,
        tags,
        corrections,
    };

    terms.push(new_entry.clone());
    sort_terms_by_name(terms);
    index::save(project_root, &index)?;

    Ok(new_entry)
}
