//! Global terms — repo-root `minds-terms.yaml`.
//!
//! When no `--project` flag is given, term commands operate on a single
//! global terms file at `<repo_root>/minds-terms.yaml` instead of a
//! project-scoped `mind-index.yaml`.

use std::fs;
use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::defaults;
use crate::error::{MfError, Result};
use crate::model::term::{Correction, Term};
use crate::service::util::{atomic_write, validate_schema_version};

const GLOBAL_TERMS_FILENAME: &str = "minds-terms.yaml";

// ── File model ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GlobalTermsFile {
    #[serde(alias = "schema")]
    schema_version: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    terms: Vec<Term>,
}

// ── Load / Save ───────────────────────────────────────────────────────────

fn path_for(repo_root: &Path) -> std::path::PathBuf {
    repo_root.join(GLOBAL_TERMS_FILENAME)
}

/// Load terms from `<repo_root>/minds-terms.yaml`.
pub fn load_terms(repo_root: &Path) -> Result<Vec<Term>> {
    let path = path_for(repo_root);
    let content = match fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            return Ok(Vec::new());
        }
        Err(e) => return Err(MfError::from(e)),
    };

    let file: GlobalTermsFile = serde_yaml::from_str(&content).map_err(|e| MfError::ParseError {
        kind: "yaml".to_string(),
        path: path.clone(),
        detail: e.to_string(),
    })?;

    validate_schema_version(&file.schema_version, &path)?;
    Ok(file.terms)
}

/// Atomically save terms to `<repo_root>/minds-terms.yaml`.
pub fn save_terms(repo_root: &Path, terms: &[Term]) -> Result<()> {
    let path = path_for(repo_root);
    let file = GlobalTermsFile { schema_version: defaults::SCHEMA_VERSION.to_string(), terms: terms.to_vec() };
    let yaml = serde_yaml::to_string(&file).map_err(|e| MfError::Internal(e.into()))?;
    atomic_write(&path, &yaml)
}

// ── Global term operations ────────────────────────────────────────────────

fn sort_terms(terms: &mut [Term]) {
    terms.sort_by(|a, b| a.term.cmp(&b.term));
}

fn dedup_preserve_first(items: &[String]) -> Vec<String> {
    let mut seen = std::collections::BTreeSet::new();
    let mut result = Vec::new();
    for item in items {
        if seen.insert(item.clone()) {
            result.push(item.clone());
        }
    }
    result
}

/// Register a new term in the global terms file.
pub fn new_term(
    repo_root: &Path,
    term: &str,
    definition: Option<&str>,
    aliases: &[String],
    tags: &[String],
) -> Result<Term> {
    if term.trim().is_empty() {
        return Err(MfError::usage("term name cannot be empty", None));
    }

    let mut terms = load_terms(repo_root)?;

    if terms.iter().any(|t| t.term == term) {
        return Err(MfError::usage(
            format!("term '{term}' already exists"),
            Some("use 'mf term fix' to modify the existing term".to_string()),
        ));
    }

    let aliases = dedup_preserve_first(aliases);
    let tags = dedup_preserve_first(tags);

    for alias in &aliases {
        for t in terms.iter() {
            if t.term == *alias || t.aliases.iter().any(|a| a == alias) {
                return Err(MfError::usage(format!("alias '{alias}' conflicts with existing term '{}'", t.term), None));
            }
        }
    }

    let new_entry = Term {
        term: term.to_string(),
        definition: definition.and_then(|s| if s.is_empty() { None } else { Some(s.to_string()) }),
        aliases,
        tags,
        corrections: vec![],
    };

    terms.push(new_entry.clone());
    sort_terms(&mut terms);
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

    sort_terms(&mut terms);
    Ok(terms)
}

/// Look up a single global term by canonical name (exact match).
pub fn show_term(repo_root: &Path, name: &str) -> Result<Term> {
    let terms = load_terms(repo_root)?;
    terms.iter().find(|t| t.term == name).cloned().ok_or_else(|| {
        MfError::usage(format!("term '{name}' not found"), Some("use 'mf term list' to see all terms".to_string()))
    })
}

/// Modify an existing global term's definition, aliases, or tags.
pub fn fix_term(
    repo_root: &Path,
    term_name: &str,
    definition: Option<&str>,
    aliases: &[String],
    tags: &[String],
) -> Result<Term> {
    if definition.is_none() && aliases.is_empty() && tags.is_empty() {
        return Err(MfError::usage("at least one of --definition, --alias, --tag must be provided", None));
    }

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

    let t = &mut terms[pos];
    if let Some(def) = definition {
        t.definition = if def.is_empty() { None } else { Some(def.to_string()) };
    }
    if !aliases.is_empty() {
        let existing: Vec<String> = t.aliases.clone();
        for alias in aliases {
            if !existing.contains(alias) {
                t.aliases.push(alias.clone());
            }
        }
    }
    if !tags.is_empty() {
        let existing: Vec<String> = t.tags.clone();
        for tag in tags {
            if !existing.contains(tag) {
                t.tags.push(tag.clone());
            }
        }
    }

    let result = t.clone();
    sort_terms(&mut terms);
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

    let term = &mut terms[idx];

    if original == term.term || term.aliases.iter().any(|a| a == original) {
        return Err(MfError::usage(
            format!("'{original}' is already a recognized form for term '{}'", term.term),
            None,
        ));
    }

    let already_exists = term.corrections.iter().any(|c| c.original == original && c.correct == correct);
    if already_exists {
        return Ok((term.clone(), false));
    }

    term.corrections.push(Correction { original: original.to_string(), correct: correct.to_string() });
    let result = term.clone();
    sort_terms(&mut terms);
    save_terms(repo_root, &terms)?;
    Ok((result, true))
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_and_list() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        let t = new_term(root, "api-gateway", Some("The API gateway service"), &[], &[]).unwrap();
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
        new_term(root, "foo", None, &[], &[]).unwrap();
        let err = new_term(root, "foo", None, &[], &[]).unwrap_err();
        assert!(err.to_string().contains("already exists"));
    }

    #[test]
    fn empty_term_name_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let err = new_term(dir.path(), "", None, &[], &[]).unwrap_err();
        assert!(err.to_string().contains("cannot be empty"));
    }

    #[test]
    fn show_term_found() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        new_term(root, "rust", Some("A systems programming language"), &[], &["lang".into()]).unwrap();

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
        new_term(root, "k8s", Some("Kubernetes"), &[], &[]).unwrap();

        let t = fix_term(root, "k8s", Some("Kubernetes (K8s)"), &[], &[]).unwrap();
        assert_eq!(t.definition.as_deref(), Some("Kubernetes (K8s)"));
    }

    #[test]
    fn learn_correction_adds() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        new_term(root, "Kubernetes", None, &["k8s".into()], &[]).unwrap();

        let (t, appended) = learn_correction(root, "kube", "Kubernetes").unwrap();
        assert!(appended);
        assert!(t.corrections.iter().any(|c| c.original == "kube" && c.correct == "Kubernetes"));
    }

    #[test]
    fn learn_correction_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        new_term(root, "Kubernetes", None, &[], &[]).unwrap();
        learn_correction(root, "kube", "Kubernetes").unwrap();
        let (_, appended) = learn_correction(root, "kube", "Kubernetes").unwrap();
        assert!(!appended);
    }

    #[test]
    fn filter_terms() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        new_term(root, "alpha", None, &[], &["test".into()]).unwrap();
        new_term(root, "beta", None, &[], &["prod".into()]).unwrap();
        new_term(root, "gamma", None, &[], &["test".into()]).unwrap();

        let list = list_terms(root, Some("test")).unwrap();
        assert_eq!(list.len(), 2);
    }

    #[test]
    fn alias_conflict_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        new_term(root, "alpha", None, &["a".into()], &[]).unwrap();
        let err = new_term(root, "beta", None, &["a".into()], &[]).unwrap_err();
        assert!(err.to_string().contains("conflicts"));
    }

    #[test]
    fn loads_empty_when_file_missing() {
        let dir = tempfile::tempdir().unwrap();
        let terms = load_terms(dir.path()).unwrap();
        assert!(terms.is_empty());
    }
}
