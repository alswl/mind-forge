use std::path::Path;

use crate::error::{MfError, Result};
use crate::model::term::Term;
use crate::service::index;

/// Look up a single term by canonical name (exact match).
pub fn show_term(project_root: &Path, name: &str) -> Result<Term> {
    let index = index::load(project_root)?;
    let terms = index.terms.as_deref().unwrap_or_default();

    let term = terms
        .iter()
        .find(|t| t.term == name)
        .ok_or_else(|| {
            MfError::usage(format!("term '{name}' not found"), Some("use `mf term list` to see all terms".to_string()))
        })?
        .clone();

    Ok(term)
}
