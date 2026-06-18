use std::path::Path;

use super::{dedup_preserve_first, sort_terms_by_name, TermInput};
use crate::error::{MfError, Result};
use crate::model::term::{Correction, Term};
use crate::service::index;

/// Result of a `term new` operation, distinguishing created vs appended.
#[derive(Debug, Clone)]
pub struct NewTermResult {
    pub term: Term,
    pub created: bool,
    pub added_aliases: Vec<String>,
    pub added_tags: Vec<String>,
    pub added_misrecognitions: Vec<String>,
}

/// Register a new term, or append aliases/tags/misrecognitions when it already exists.
pub fn new_term(
    project_root: &Path,
    term: &str,
    input: TermInput<'_>,
    misrecognitions: &[String],
) -> Result<NewTermResult> {
    if term.trim().is_empty() {
        return Err(MfError::usage("term name cannot be empty", None));
    }

    super::validate_confidence(input.confidence)?;

    let mut index = index::load(project_root)?;
    let terms = index.terms.get_or_insert_with(Vec::new);

    let aliases = dedup_preserve_first(input.aliases);
    let tags = dedup_preserve_first(input.tags);
    let misrecognitions = dedup_preserve_first(misrecognitions);

    // Check if the term already exists (by index to avoid borrow conflicts)
    let existing_idx = terms.iter().position(|t| t.term == term);

    if let Some(idx) = existing_idx {
        // Collect the data we need, then modify by index
        let existing_term_name = terms[idx].term.clone();
        let mut added_aliases = Vec::new();
        let mut added_tags = Vec::new();
        let mut added_misrecognitions = Vec::new();

        for alias in &aliases {
            if terms[idx].aliases.contains(alias) || terms[idx].term == *alias {
                continue;
            }
            for t in terms.iter() {
                if t.term == existing_term_name {
                    continue;
                }
                if t.term == *alias || t.aliases.iter().any(|a| a == alias) {
                    return Err(MfError::not_found(
                        format!("alias '{alias}' already belongs to term '{}'", t.term),
                        None,
                    ));
                }
            }
            terms[idx].aliases.push(alias.clone());
            added_aliases.push(alias.clone());
        }

        for tag in &tags {
            if !terms[idx].tags.contains(tag) {
                terms[idx].tags.push(tag.clone());
                added_tags.push(tag.clone());
            }
        }

        for misrec in &misrecognitions {
            let parts: Vec<&str> = misrec.splitn(2, ':').collect();
            if parts.len() != 2 {
                continue;
            }
            let original = parts[0];
            let correct = parts[1];
            if terms[idx].corrections.iter().any(|c| c.original == original && c.correct == correct) {
                continue;
            }
            terms[idx].corrections.push(Correction::misrecognition(original, correct));
            added_misrecognitions.push(misrec.clone());
        }

        let out = terms[idx].clone();
        sort_terms_by_name(terms);
        index::save(project_root, &index)?;

        return Ok(NewTermResult { term: out, created: false, added_aliases, added_tags, added_misrecognitions });
    }

    // Check alias uniqueness across existing terms
    for alias in &aliases {
        for t in terms.iter() {
            if t.term == *alias || t.aliases.iter().any(|a| a == alias) {
                return Err(MfError::usage(format!("alias '{alias}' conflicts with existing term '{}'", t.term), None));
            }
        }
    }

    let corrections: Vec<Correction> =
        misrecognitions.iter().map(|m| Correction::misrecognition(m.clone(), term)).collect();

    let new_entry = Term {
        term: term.to_string(),
        definition: input.definition.and_then(|s| if s.is_empty() { None } else { Some(s.to_string()) }),
        description: input.description.map(String::from),
        confidence: input.confidence,
        aliases: aliases.clone(),
        tags: tags.clone(),
        corrections: corrections.clone(),
    };

    terms.push(new_entry.clone());
    sort_terms_by_name(terms);
    index::save(project_root, &index)?;

    Ok(NewTermResult {
        term: new_entry,
        created: true,
        added_aliases: aliases,
        added_tags: tags,
        added_misrecognitions: misrecognitions,
    })
}
