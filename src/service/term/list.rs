use std::path::Path;

use crate::error::Result;
use crate::model::term::Term;
use crate::service::index;

/// List all registered terms, optionally filtered by substring.
pub fn list_terms(project_root: &Path, filter: Option<&str>) -> Result<Vec<Term>> {
    let index = index::load(project_root)?;
    let mut terms = match index.terms {
        Some(t) => t,
        None => return Ok(vec![]),
    };

    if let Some(f) = filter {
        let lower = f.to_lowercase();
        terms.retain(|t| {
            t.term.to_lowercase().contains(&lower)
                || t.aliases.iter().any(|a| a.to_lowercase().contains(&lower))
                || t.tags.iter().any(|tag| tag.to_lowercase().contains(&lower))
        });
    }

    terms.sort_by(|a, b| a.term.cmp(&b.term));
    Ok(terms)
}
