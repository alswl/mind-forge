use crate::error::{MfError, Result};
use crate::model::term::Term;

/// Find a term by its main name or alias.
/// Returns the index into the `terms` slice.
pub(crate) fn find_term_by_correct(terms: &[Term], correct: &str) -> Result<usize> {
    let matches: Vec<usize> = terms
        .iter()
        .enumerate()
        .filter(|(_, t)| t.term == correct || t.aliases.iter().any(|a| a == correct))
        .map(|(i, _)| i)
        .collect();

    match matches.len() {
        0 => Err(MfError::usage(
            format!("no term registers '{correct}' as its main name or alias"),
            Some(format!("register first with 'mf term new {correct}' or 'mf term fix --alias {correct}'")),
        )),
        1 => Ok(matches[0]),
        _ => {
            let candidates: Vec<&str> = matches.iter().map(|&i| terms[i].term.as_str()).collect();
            Err(MfError::usage(
                format!("multiple terms claim '{correct}': {candidates:?}"),
                Some("disambiguate by editing 'mf term fix' for the chosen term".to_string()),
            ))
        }
    }
}
