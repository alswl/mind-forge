//! Global terms — repo-root `minds-terms.yaml`.
//!
//! When no `--project` flag is given, term commands operate on a single
//! global terms file at `<repo_root>/minds-terms.yaml` instead of a
//! project-scoped `mind-index.yaml`.
//!
//! Load/save delegates to `repo_format` for format detection and
//! dual-shape read/write. Read-only callers use the format-unaware
//! `load_terms` wrapper. Write paths must use `save_terms_with_format`
//! with the loaded format — there is no format-defaulting save, since
//! that would silently rewrite repo-format files as schema-version and
//! destroy comments.

use std::path::Path;

use super::{dedup_preserve_first, sort_terms_by_name};
use crate::error::{MfError, Result};
use crate::model::term::{Correction, Term};
use crate::service::term::repo_format::{self, path_for, TermsFileFormat};

// ── Load / Save ───────────────────────────────────────────────────────────

/// Load terms from `<repo_root>/minds-terms.yaml`.
///
/// Delegates to `repo_format::load`; discards the format tag so existing
/// read-only callers stay unchanged.
pub fn load_terms(repo_root: &Path) -> Result<Vec<Term>> {
    repo_format::load(repo_root).map(|(terms, _format)| terms)
}

/// Load terms with format tag for callers that need to branch on write.
pub fn load_terms_with_format(repo_root: &Path) -> Result<(Vec<Term>, TermsFileFormat)> {
    repo_format::load(repo_root)
}

/// Atomically save terms with explicit format control.
///
/// `on_disk_content` is required for repository-format writes (surgical
/// edits). Schema-version writes ignore it. Callers MUST pass the format
/// they loaded — there is intentionally no format-defaulting wrapper,
/// because defaulting to schema-version on a repo-format file would
/// silently destroy comments and reformat the file.
pub fn save_terms_with_format(
    repo_root: &Path,
    terms: &[Term],
    format: TermsFileFormat,
    on_disk_content: Option<&str>,
) -> Result<()> {
    repo_format::save(repo_root, terms, format, on_disk_content)
}

// ── Repo-format field guard ───────────────────────────────────────────────

/// Reject any field that the repository format does not carry.
///
/// `definition.is_some()` triggers regardless of the value (including
/// `""` and whitespace-only), because the only correct response on a
/// repo-format file is to refuse the edit, not to silently accept and
/// later rewrite the file as schema-version.
fn assert_no_unsupported_repo_fields(definition: Option<&str>, aliases: &[String], tags: &[String]) -> Result<()> {
    if definition.is_some() {
        return Err(repo_field_error("--definition"));
    }
    if !aliases.is_empty() {
        return Err(repo_field_error("--alias"));
    }
    if !tags.is_empty() {
        return Err(repo_field_error("--tag"));
    }
    Ok(())
}

fn repo_field_error(field_name: &str) -> MfError {
    MfError::usage(
        format!("{field_name} is not supported on repository-format minds-terms.yaml"),
        Some("use --misrecognition or convert the file to schema-version format by hand".to_string()),
    )
}

// ── Global term operations ────────────────────────────────────────────────

/// Register a new term in the global terms file.
pub fn new_term(
    repo_root: &Path,
    term: &str,
    definition: Option<&str>,
    aliases: &[String],
    tags: &[String],
    misrecognitions: &[String],
) -> Result<Term> {
    if term.trim().is_empty() {
        return Err(MfError::usage("term name cannot be empty", None));
    }

    let (mut terms, format) = load_terms_with_format(repo_root)?;
    let on_disk_content = std::fs::read_to_string(path_for(repo_root)).ok();
    let misrecognitions = dedup_preserve_first(misrecognitions);

    match format {
        TermsFileFormat::Repository => {
            assert_no_unsupported_repo_fields(definition, aliases, tags)?;

            if terms.iter().any(|t| t.term == term) {
                return Err(MfError::usage(
                    format!("term '{term}' already exists"),
                    Some("use 'mf term fix' to modify the existing term".to_string()),
                ));
            }

            let new_content =
                repo_format::append_term_repo_format(on_disk_content.as_deref().unwrap_or(""), term, &misrecognitions)?;

            let new_entry = Term {
                term: term.to_string(),
                definition: None,
                aliases: vec![],
                tags: vec![],
                corrections: misrecognitions
                    .iter()
                    .map(|m| Correction { original: m.clone(), correct: term.to_string() })
                    .collect(),
            };

            terms.push(new_entry.clone());
            sort_terms_by_name(&mut terms);
            save_terms_with_format(repo_root, &terms, TermsFileFormat::Repository, Some(&new_content))?;

            Ok(new_entry)
        }
        TermsFileFormat::SchemaVersion => {
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
                        return Err(MfError::usage(
                            format!("alias '{alias}' conflicts with existing term '{}'", t.term),
                            None,
                        ));
                    }
                }
            }

            let corrections: Vec<Correction> =
                misrecognitions.iter().map(|m| Correction { original: m.clone(), correct: term.to_string() }).collect();

            let new_entry = Term {
                term: term.to_string(),
                definition: definition.and_then(|s| if s.is_empty() { None } else { Some(s.to_string()) }),
                aliases,
                tags,
                corrections,
            };

            terms.push(new_entry.clone());
            sort_terms_by_name(&mut terms);
            save_terms_with_format(repo_root, &terms, TermsFileFormat::SchemaVersion, None)?;

            Ok(new_entry)
        }
    }
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

    let (mut terms, format) = load_terms_with_format(repo_root)?;

    if format == TermsFileFormat::Repository {
        // Any flag is unsupported on repo-format; the "at least one"
        // check above guarantees we hit one of the rejection branches.
        assert_no_unsupported_repo_fields(definition, aliases, tags)?;
        return Err(MfError::internal("fix_term: repo-format guard accepted a write — this is a bug"));
    }

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
    sort_terms_by_name(&mut terms);
    save_terms_with_format(repo_root, &terms, TermsFileFormat::SchemaVersion, None)?;
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

    let (mut terms, format) = load_terms_with_format(repo_root)?;
    let on_disk_content = std::fs::read_to_string(path_for(repo_root)).ok();

    // Validate the correct target exists and resolve the canonical term name
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

    match format {
        TermsFileFormat::Repository => {
            let content = on_disk_content.unwrap_or_default();
            let result = repo_format::append_misrecognition_repo_format(&content, &canonical_name, original)?;
            if !result.appended {
                return Ok((terms[idx].clone(), false));
            }

            terms[idx].corrections.push(Correction { original: original.to_string(), correct: canonical_name.clone() });
            let term_result = terms[idx].clone();
            sort_terms_by_name(&mut terms);
            save_terms_with_format(repo_root, &terms, TermsFileFormat::Repository, Some(&result.content))?;
            Ok((term_result, true))
        }
        TermsFileFormat::SchemaVersion => {
            terms[idx].corrections.push(Correction { original: original.to_string(), correct: canonical_name.clone() });
            let result = terms[idx].clone();
            sort_terms_by_name(&mut terms);
            save_terms_with_format(repo_root, &terms, TermsFileFormat::SchemaVersion, None)?;
            Ok((result, true))
        }
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    /// Write a minimal schema-version file so format detection returns SchemaVersion.
    fn seed_schema_version(root: &Path) {
        std::fs::write(path_for(root), "schema_version: '1'\nterms: []\n").unwrap();
    }

    #[test]
    fn new_and_list() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        seed_schema_version(root);

        let t = new_term(root, "api-gateway", Some("The API gateway service"), &[], &[], &[]).unwrap();
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
        seed_schema_version(root);
        new_term(root, "foo", None, &[], &[], &[]).unwrap();
        let err = new_term(root, "foo", None, &[], &[], &[]).unwrap_err();
        assert!(err.to_string().contains("already exists"));
    }

    #[test]
    fn empty_term_name_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        seed_schema_version(root);
        let err = new_term(root, "", None, &[], &[], &[]).unwrap_err();
        assert!(err.to_string().contains("cannot be empty"));
    }

    #[test]
    fn show_term_found() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        seed_schema_version(root);
        new_term(root, "rust", Some("A systems programming language"), &[], &["lang".into()], &[]).unwrap();

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
        seed_schema_version(root);
        new_term(root, "k8s", Some("Kubernetes"), &[], &[], &[]).unwrap();

        let t = fix_term(root, "k8s", Some("Kubernetes (K8s)"), &[], &[]).unwrap();
        assert_eq!(t.definition.as_deref(), Some("Kubernetes (K8s)"));
    }

    #[test]
    fn learn_correction_adds() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        seed_schema_version(root);
        new_term(root, "Kubernetes", None, &["k8s".into()], &[], &[]).unwrap();

        let (t, appended) = learn_correction(root, "kube", "Kubernetes").unwrap();
        assert!(appended);
        assert!(t.corrections.iter().any(|c| c.original == "kube" && c.correct == "Kubernetes"));
    }

    #[test]
    fn learn_correction_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        seed_schema_version(root);
        new_term(root, "Kubernetes", None, &[], &[], &[]).unwrap();
        learn_correction(root, "kube", "Kubernetes").unwrap();
        let (_, appended) = learn_correction(root, "kube", "Kubernetes").unwrap();
        assert!(!appended);
    }

    #[test]
    fn filter_terms() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        seed_schema_version(root);
        new_term(root, "alpha", None, &[], &["test".into()], &[]).unwrap();
        new_term(root, "beta", None, &[], &["prod".into()], &[]).unwrap();
        new_term(root, "gamma", None, &[], &["test".into()], &[]).unwrap();

        let list = list_terms(root, Some("test")).unwrap();
        assert_eq!(list.len(), 2);
    }

    #[test]
    fn alias_conflict_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        seed_schema_version(root);
        new_term(root, "alpha", None, &["a".into()], &[], &[]).unwrap();
        let err = new_term(root, "beta", None, &["a".into()], &[], &[]).unwrap_err();
        assert!(err.to_string().contains("conflicts"));
    }

    #[test]
    fn loads_empty_when_file_missing() {
        let dir = tempfile::tempdir().unwrap();
        let terms = load_terms(dir.path()).unwrap();
        assert!(terms.is_empty());
    }
}
