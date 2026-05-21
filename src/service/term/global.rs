//! Global terms — repo-root `minds-terms.yaml`.
//!
//! When no `--project` flag is given, term commands operate on a single
//! global terms file at `<repo_root>/minds-terms.yaml`.

use std::path::Path;

use super::fix::apply_update;
use super::{dedup_preserve_first, sort_terms_by_name, TermInput, TermUpdate};
use crate::error::{MfError, Result};
use crate::model::term::{Correction, Term};
use crate::service::term::repo_format::{self, path_for};

// ── Load / Save ─────────────────────────────────────────────────────────────

/// Load terms from `<repo_root>/minds-terms.yaml`.
pub(crate) fn load_terms(repo_root: &Path) -> Result<Vec<Term>> {
    repo_format::load(repo_root)
}

/// Save terms to `<repo_root>/minds-terms.yaml`.
pub fn save_terms(repo_root: &Path, terms: &[Term]) -> Result<()> {
    repo_format::save(repo_root, terms)
}

// ── Global term operations ──────────────────────────────────────────────────

/// Register a new term in the global terms file.
pub fn new_term(repo_root: &Path, term: &str, input: TermInput<'_>, misrecognitions: &[String]) -> Result<Term> {
    if term.trim().is_empty() {
        return Err(MfError::usage("term name cannot be empty", None));
    }

    super::validate_confidence(input.confidence)?;

    let mut terms = load_terms(repo_root)?;
    let misrecognitions = dedup_preserve_first(misrecognitions);
    let aliases = dedup_preserve_first(input.aliases);
    let tags = dedup_preserve_first(input.tags);

    if terms.iter().any(|t| t.term == term) {
        return Err(MfError::usage(
            format!("term '{term}' already exists"),
            Some("use 'mf term fix' to modify the existing term".to_string()),
        ));
    }

    for alias in &aliases {
        for t in terms.iter() {
            if t.term == *alias || t.aliases.iter().any(|a| a == alias) {
                return Err(MfError::usage(format!("alias '{alias}' conflicts with existing term '{}'", t.term), None));
            }
        }
    }

    let corrections: Vec<Correction> =
        misrecognitions.iter().map(|m| Correction { original: m.clone(), correct: term.to_string() }).collect();

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
    sort_terms_by_name(&mut terms);
    save_terms(repo_root, &terms)?;

    Ok(new_entry)
}

/// List global terms, optionally filtered by substring.
pub fn list_terms(repo_root: &Path, filter: Option<&str>) -> Result<Vec<Term>> {
    let mut terms = load_terms(repo_root)?;

    if let Some(f) = filter {
        let lower = f.to_lowercase();
        terms.retain(|t| {
            t.term.to_lowercase().contains(&lower)
                || t.aliases.iter().any(|a| a.to_lowercase().contains(&lower))
                || t.tags.iter().any(|tag| tag.to_lowercase().contains(&lower))
        });
    }

    sort_terms_by_name(&mut terms);
    Ok(terms)
}

/// Look up a single global term by canonical name (exact match).
pub fn show_term(repo_root: &Path, name: &str) -> Result<Term> {
    let terms = load_terms(repo_root)?;
    terms.iter().find(|t| t.term == name).cloned().ok_or_else(|| {
        MfError::usage(format!("term '{name}' not found"), Some("use 'mf term list' to see all terms".to_string()))
    })
}

/// Modify an existing global term's definition, aliases, tags, description, or confidence.
pub fn fix_term(repo_root: &Path, term_name: &str, update: TermUpdate<'_>) -> Result<Term> {
    if !update.has_legacy_flags() && !update.has_metadata_flags() {
        return Err(MfError::usage(
            "at least one of --definition, --description, --confidence, --alias, --tag, --clear-description, --clear-confidence must be provided",
            None,
        ));
    }

    super::validate_confidence(update.confidence)?;

    let mut terms = load_terms(repo_root)?;

    let pos = terms.iter().position(|t| t.term == term_name).ok_or_else(|| {
        MfError::usage(format!("term '{term_name}' not found"), Some("use 'mf term list' or 'mf term new'".to_string()))
    })?;

    if terms.iter().filter(|t| t.term == term_name).count() > 1 {
        return Err(MfError::IncompatibleSchema {
            path: path_for(repo_root),
            found: format!("duplicate term '{term_name}'"),
            expected: vec!["unique terms".to_string()],
        });
    }

    apply_update(&mut terms[pos], &update);
    let result = terms[pos].clone();
    sort_terms_by_name(&mut terms);
    save_terms(repo_root, &terms)?;
    Ok(result)
}

/// Find a global term by its main name or alias. Returns the index.
fn find_term_index(terms: &[Term], correct: &str) -> Result<usize> {
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

/// Register a correction for an existing global term.
pub fn learn_correction(repo_root: &Path, original: &str, correct: &str) -> Result<(Term, bool)> {
    if original.trim().is_empty() {
        return Err(MfError::usage("--original cannot be empty", None));
    }
    if correct.trim().is_empty() {
        return Err(MfError::usage("--correct cannot be empty", None));
    }

    let mut terms = load_terms(repo_root)?;

    let idx = find_term_index(&terms, correct)?;
    let canonical_name = terms[idx].term.clone();

    if original == terms[idx].term || terms[idx].aliases.iter().any(|a| a == original) {
        return Err(MfError::usage(
            format!("'{original}' is already a recognized form for term '{}'", canonical_name),
            None,
        ));
    }

    let already_exists = terms[idx].corrections.iter().any(|c| c.original == original && c.correct == canonical_name);
    if already_exists {
        return Ok((terms[idx].clone(), false));
    }

    terms[idx].corrections.push(Correction { original: original.to_string(), correct: canonical_name.clone() });
    let result = terms[idx].clone();
    sort_terms_by_name(&mut terms);
    save_terms(repo_root, &terms)?;
    Ok((result, true))
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn seed(repo_root: &Path) {
        std::fs::write(path_for(repo_root), "schema_version: '1'\nterms: []\n").unwrap();
    }

    fn def_input(def: &str) -> TermInput<'_> {
        TermInput { definition: Some(def), ..TermInput::default() }
    }

    fn tags_input<'a>(def: Option<&'a str>, tags: &'a [String]) -> TermInput<'a> {
        TermInput { definition: def, tags, ..TermInput::default() }
    }

    fn aliases_input<'a>(aliases: &'a [String]) -> TermInput<'a> {
        TermInput { aliases, ..TermInput::default() }
    }

    #[test]
    fn new_and_list() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        seed(root);

        let t = new_term(root, "api-gateway", def_input("The API gateway service"), &[]).unwrap();
        assert_eq!(t.term, "api-gateway");
        assert_eq!(t.definition.as_deref(), Some("The API gateway service"));

        let list = list_terms(root, None).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].term, "api-gateway");
    }

    #[test]
    fn duplicate_term_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        seed(root);
        new_term(root, "foo", TermInput::default(), &[]).unwrap();
        let err = new_term(root, "foo", TermInput::default(), &[]).unwrap_err();
        assert!(err.to_string().contains("already exists"));
    }

    #[test]
    fn empty_term_name_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        seed(root);
        let err = new_term(root, "", TermInput::default(), &[]).unwrap_err();
        assert!(err.to_string().contains("cannot be empty"));
    }

    #[test]
    fn show_term_found() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        seed(root);
        let tags = vec!["lang".to_string()];
        new_term(root, "rust", tags_input(Some("A systems programming language"), &tags), &[]).unwrap();

        let t = show_term(root, "rust").unwrap();
        assert_eq!(t.term, "rust");
        assert_eq!(t.tags, vec!["lang"]);
    }

    #[test]
    fn show_term_not_found() {
        let dir = tempfile::tempdir().unwrap();
        let err = show_term(dir.path(), "nonexistent").unwrap_err();
        assert!(err.to_string().contains("not found"));
    }

    #[test]
    fn fix_term_definition() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        seed(root);
        new_term(root, "k8s", def_input("Kubernetes"), &[]).unwrap();

        let t = fix_term(root, "k8s", TermUpdate { definition: Some("Kubernetes (K8s)"), ..TermUpdate::default() })
            .unwrap();
        assert_eq!(t.definition.as_deref(), Some("Kubernetes (K8s)"));
    }

    #[test]
    fn learn_correction_adds() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        seed(root);
        let aliases = vec!["k8s".to_string()];
        new_term(root, "Kubernetes", aliases_input(&aliases), &[]).unwrap();

        let (t, appended) = learn_correction(root, "kube", "Kubernetes").unwrap();
        assert!(appended);
        assert!(t.corrections.iter().any(|c| c.original == "kube" && c.correct == "Kubernetes"));
    }

    #[test]
    fn learn_correction_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        seed(root);
        new_term(root, "Kubernetes", TermInput::default(), &[]).unwrap();
        learn_correction(root, "kube", "Kubernetes").unwrap();
        let (_, appended) = learn_correction(root, "kube", "Kubernetes").unwrap();
        assert!(!appended);
    }

    #[test]
    fn filter_terms() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        seed(root);
        let test_tag = vec!["test".to_string()];
        let prod_tag = vec!["prod".to_string()];
        new_term(root, "alpha", tags_input(None, &test_tag), &[]).unwrap();
        new_term(root, "beta", tags_input(None, &prod_tag), &[]).unwrap();
        new_term(root, "gamma", tags_input(None, &test_tag), &[]).unwrap();

        let list = list_terms(root, Some("test")).unwrap();
        assert_eq!(list.len(), 2);
    }

    #[test]
    fn alias_conflict_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        seed(root);
        let aliases = vec!["a".to_string()];
        new_term(root, "alpha", aliases_input(&aliases), &[]).unwrap();
        let err = new_term(root, "beta", aliases_input(&aliases), &[]).unwrap_err();
        assert!(err.to_string().contains("conflicts"));
    }

    #[test]
    fn loads_empty_when_file_missing() {
        let dir = tempfile::tempdir().unwrap();
        let terms = load_terms(dir.path()).unwrap();
        assert!(terms.is_empty());
    }
}
