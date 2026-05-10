use std::path::Path;

use super::sort_terms_by_name;
use crate::error::{MfError, Result};
use crate::model::term::Term;
use crate::service::index;

/// Modify an existing term's definition, aliases, or tags.
pub fn fix_term(
    project_root: &Path,
    term_name: &str,
    definition: Option<&str>,
    aliases: &[String],
    tags: &[String],
) -> Result<Term> {
    if definition.is_none() && aliases.is_empty() && tags.is_empty() {
        return Err(MfError::usage("at least one of --definition, --alias, --tag must be provided", None));
    }

    let mut index = index::load(project_root)?;

    // Find term by name (strict case-sensitive) and extract data before mutation
    let term_clone = {
        let terms = index.terms.as_mut().ok_or_else(|| {
            MfError::usage(
                format!("term '{term_name}' not found"),
                Some("use 'mf term list' or 'mf term new'".to_string()),
            )
        })?;

        let pos = terms.iter().position(|t| t.term == term_name).ok_or_else(|| {
            MfError::usage(
                format!("term '{term_name}' not found"),
                Some("use 'mf term list' or 'mf term new'".to_string()),
            )
        })?;

        // Check for duplicate entries (dirty data)
        if terms.iter().filter(|t| t.term == term_name).count() > 1 {
            return Err(MfError::IncompatibleSchema {
                path: project_root.join("mind-index.yaml"),
                found: format!("duplicate term '{term_name}'"),
                expected: vec!["unique terms".to_string()],
            });
        }

        let t = &mut terms[pos];

        // Apply definition
        if let Some(def) = definition {
            t.definition = if def.is_empty() { None } else { Some(def.to_string()) };
        }

        // Append aliases with dedup
        if !aliases.is_empty() {
            let existing: Vec<String> = t.aliases.clone();
            for alias in aliases {
                if !existing.contains(alias) {
                    t.aliases.push(alias.clone());
                }
            }
        }

        // Append tags with dedup
        if !tags.is_empty() {
            let existing: Vec<String> = t.tags.clone();
            for tag in tags {
                if !existing.contains(tag) {
                    t.tags.push(tag.clone());
                }
            }
        }

        t.clone()
    };

    // Sort terms before save (defensive)
    if let Some(ref mut terms) = index.terms {
        sort_terms_by_name(terms);
    }

    index::save(project_root, &index)?;

    Ok(term_clone)
}
