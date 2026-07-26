//! Global terms — repo-root `minds-terms.yaml`.
//!
//! When no `--project` flag is given, term commands operate on a single
//! global terms file at `<repo_root>/minds-terms.yaml`.

use std::path::Path;

use super::fix::apply_update;
use super::{TermInput, TermUpdate, dedup_preserve_first, sort_terms_by_name};
use crate::defaults;
use crate::error::{MfError, Result};
use crate::model::index::IndexFile;
use crate::model::lifecycle::{PlannedChange, ScopeRef};
use crate::model::term::{Correction, FixSelection, Term, TermLintReport, validate_corrections};
use crate::service::lifecycle;
use crate::service::term::lint;
use crate::service::term::new::append_to_existing_term;
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

/// Register a new term in the global terms file, or append aliases/tags when it exists.
pub fn new_term(
    repo_root: &Path,
    term: &str,
    input: TermInput<'_>,
    misrecognitions: &[String],
) -> Result<super::new::NewTermResult> {
    if term.trim().is_empty() {
        return Err(MfError::usage("term name cannot be empty", None));
    }

    super::validate_confidence(input.confidence)?;

    let mut terms = load_terms(repo_root)?;
    let misrecognitions = dedup_preserve_first(misrecognitions);
    let aliases = dedup_preserve_first(input.aliases);
    let tags = dedup_preserve_first(input.tags);

    if let Some(idx) = terms.iter().position(|t| t.term == term) {
        let outcome = append_to_existing_term(&mut terms, idx, &aliases, &tags, &misrecognitions)?;
        let out = terms[idx].clone();
        sort_terms_by_name(&mut terms);
        save_terms(repo_root, &terms)?;
        return Ok(super::new::NewTermResult {
            term: out,
            created: false,
            added_aliases: outcome.added_aliases,
            added_tags: outcome.added_tags,
            added_misrecognitions: outcome.added_misrecognitions,
        });
    }

    // FR-006: alias collision exits 1 (Failure), not 2 (UsageError).
    // NotFound is the closest existing variant for that exit code.
    for alias in &aliases {
        for t in terms.iter() {
            if t.term == *alias || t.aliases.iter().any(|a| a == alias) {
                return Err(MfError::not_found(
                    format!("alias '{alias}' conflicts with existing term '{}'", t.term),
                    None,
                ));
            }
        }
    }

    let mut corrections: Vec<Correction> = misrecognitions
        .iter()
        .map(|m| {
            if let Some((original, correct)) = m.split_once(':') {
                Correction::misrecognition(original, correct)
            } else {
                Correction::misrecognition(m.clone(), term)
            }
        })
        .collect();
    // Also create corrections for aliases so the scanner finds them.
    for alias in &aliases {
        if !corrections.iter().any(|c| c.original == *alias) {
            corrections.push(Correction::misrecognition(alias.clone(), term));
        }
    }

    let new_entry = Term {
        term: term.to_string(),
        definition: input.definition.filter(|s| !s.is_empty()).map(String::from),
        description: input.description.map(String::from),
        confidence: input.confidence,
        aliases: aliases.clone(),
        tags: tags.clone(),
        corrections: corrections.clone(),
    };

    terms.push(new_entry.clone());
    sort_terms_by_name(&mut terms);
    save_terms(repo_root, &terms)?;

    Ok(super::new::NewTermResult {
        term: new_entry,
        created: true,
        added_aliases: aliases,
        added_tags: tags,
        added_misrecognitions: misrecognitions,
    })
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
///
/// Lenient load so a term with invalid corrections can still be inspected.
pub fn show_term(repo_root: &Path, name: &str) -> Result<Term> {
    let terms = repo_format::load_lenient(repo_root)?;
    terms.iter().find(|t| t.term == name).cloned().ok_or_else(|| {
        MfError::usage(format!("term '{name}' not found"), Some("use `mf term list` to see all terms".to_string()))
    })
}

/// Modify an existing global term's definition, aliases, tags, description, confidence,
/// or corrections.
pub fn fix_term(repo_root: &Path, term_name: &str, update: TermUpdate<'_>) -> Result<Term> {
    update.ensure_non_empty()?;

    super::validate_confidence(update.confidence)?;

    // Lenient load so an invalid correction stays repairable via the CLI.
    let mut terms = repo_format::load_lenient(repo_root)?;

    let pos = terms.iter().position(|t| t.term == term_name).ok_or_else(|| {
        MfError::usage(format!("term '{term_name}' not found"), Some("use `mf term list` or `mf term new`".to_string()))
    })?;

    if terms.iter().filter(|t| t.term == term_name).count() > 1 {
        return Err(MfError::IncompatibleSchema {
            path: path_for(repo_root),
            found: format!("duplicate term '{term_name}'"),
            expected: vec!["unique terms".to_string()],
        });
    }

    apply_update(&mut terms[pos], &update)?;
    let result = terms[pos].clone();
    sort_terms_by_name(&mut terms);
    validate_corrections(&terms).map_err(|msg| MfError::usage(msg, None::<String>))?;
    save_terms(repo_root, &terms)?;
    Ok(result)
}

// ── Global term lint ──────────────────────────────────────────────────────────

fn global_index(terms: Vec<Term>) -> IndexFile {
    IndexFile {
        terms: Some(terms),
        schema_version: defaults::SCHEMA_VERSION.to_string(),
        sources: None,
        assets: None,
        articles: None,
        prompts: None,
        thinking: None,
        publish_records: None,
        extra: None,
    }
}

pub fn lint_file_selection(
    repo_root: &Path,
    file_path: &str,
    fix: bool,
    dry_run: bool,
    selection: &FixSelection,
) -> Result<TermLintReport> {
    let terms = load_terms(repo_root)?;
    lint::validate_fix_selection(&terms, selection)?;
    if terms.is_empty() {
        return Ok(lint::empty_report(fix, dry_run));
    }
    lint::lint_single_file_with_selection(&global_index(terms), repo_root, file_path, fix, dry_run, selection)
}

pub fn lint_path_selection(
    repo_root: &Path,
    path: &str,
    fix: bool,
    dry_run: bool,
    selection: &FixSelection,
) -> Result<TermLintReport> {
    if repo_root.join(path).is_dir() {
        lint_dir_selection(repo_root, path, fix, dry_run, selection)
    } else {
        lint_file_selection(repo_root, path, fix, dry_run, selection)
    }
}

pub fn lint_dir_selection(
    repo_root: &Path,
    dir_path: &str,
    fix: bool,
    dry_run: bool,
    selection: &FixSelection,
) -> Result<TermLintReport> {
    let terms = load_terms(repo_root)?;
    lint::validate_fix_selection(&terms, selection)?;
    if terms.is_empty() {
        return Ok(lint::empty_report(fix, dry_run));
    }
    let target = repo_root.join(dir_path).canonicalize().map_err(MfError::Io)?;
    let canonical_repo = repo_root.canonicalize().map_err(MfError::Io)?;
    let index = global_index(terms);
    if target.starts_with(&canonical_repo) {
        let relative = target
            .strip_prefix(&canonical_repo)
            .map_err(|_| MfError::usage(format!("directory is outside repo root: {dir_path}"), None))?;
        lint::lint_dir_with_selection(
            &index,
            &canonical_repo,
            &relative.to_string_lossy(),
            "provide a path relative to the repo root",
            fix,
            dry_run,
            selection,
        )
    } else {
        lint::lint_walk_with_selection(&index, &target, &target, Some(&target), fix, dry_run, selection)
    }
}

pub fn lint_terms_selection(
    repo_root: &Path,
    fix: bool,
    dry_run: bool,
    selection: &FixSelection,
) -> Result<TermLintReport> {
    let terms = load_terms(repo_root)?;
    lint::validate_fix_selection(&terms, selection)?;
    if terms.is_empty() {
        return Ok(lint::empty_report(fix, dry_run));
    }
    lint::lint_walk_with_selection(&global_index(terms), repo_root, repo_root, None, fix, dry_run, selection)
}

// ── Global term rename ───────────────────────────────────────────────────────

/// Rename a global term.
pub fn rename_term(
    repo_root: &Path,
    old_name: &str,
    new_name: &str,
    keep_alias: bool,
    force: bool,
    dry_run: bool,
) -> Result<super::rename::TermRenameReport> {
    if old_name.trim().is_empty() {
        return Err(MfError::usage("old term name cannot be empty", None));
    }
    if new_name.trim().is_empty() {
        return Err(MfError::usage("new term name cannot be empty", None));
    }

    let mut terms = load_terms(repo_root)?;

    let pos = terms.iter().position(|t| t.term == old_name).ok_or_else(|| {
        MfError::not_found(
            format!("term '{old_name}' not found"),
            Some("use `mf term list` to see available terms".to_string()),
        )
    })?;

    if old_name != new_name && terms.iter().any(|t| t.term == new_name) && !force {
        return Err(MfError::usage(
            format!("a term named '{new_name}' already exists"),
            Some("use --force to overwrite".to_string()),
        ));
    }

    let scope = ScopeRef { project: None, global: true };
    let before = super::rename::TermRenameIdentity { name: old_name.to_string(), scope: scope.clone() };
    let after = super::rename::TermRenameIdentity { name: new_name.to_string(), scope };

    let planned: Vec<PlannedChange> =
        vec![lifecycle::planned_yaml_update(&path_for(repo_root).to_string_lossy(), Some(old_name), Some(new_name))];

    if dry_run {
        return Ok(super::rename::TermRenameReport {
            verb: "rename".into(),
            kind: "term".into(),
            before,
            after,
            references: vec![],
            side_effects: planned,
            force,
            dry_run: true,
        });
    }

    let term = &mut terms[pos];
    if keep_alias && !term.aliases.iter().any(|a| a == old_name) {
        term.aliases.push(old_name.to_string());
    }
    term.term = new_name.to_string();
    sort_terms_by_name(&mut terms);
    save_terms(repo_root, &terms)?;

    Ok(super::rename::TermRenameReport {
        verb: "rename".into(),
        kind: "term".into(),
        before,
        after,
        references: vec![],
        side_effects: planned,
        force,
        dry_run: false,
    })
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

        let r = new_term(root, "api-gateway", def_input("The API gateway service"), &[]).unwrap();
        let t = r.term;
        assert_eq!(t.term, "api-gateway");
        assert_eq!(t.definition.as_deref(), Some("The API gateway service"));

        let list = list_terms(root, None).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].term, "api-gateway");
    }

    #[test]
    fn duplicate_term_is_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        seed(root);
        new_term(root, "foo", TermInput::default(), &[]).unwrap();
        let result = new_term(root, "foo", TermInput::default(), &[]).unwrap();
        assert!(!result.created, "repeating the same new_term should be idempotent");
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

    #[test]
    fn external_directory_uses_target_as_display_and_write_base() {
        let repo = tempfile::tempdir().unwrap();
        seed(repo.path());
        let corrections = vec!["synthetic".to_string()];
        new_term(repo.path(), "Synthetic", TermInput::default(), &corrections).unwrap();
        let external = tempfile::tempdir().unwrap();
        let visible_dir = external.path().join("visible");
        std::fs::create_dir_all(&visible_dir).unwrap();
        std::fs::write(visible_dir.join("document.md"), "synthetic\n").unwrap();
        let report =
            lint_path_selection(repo.path(), &visible_dir.to_string_lossy(), false, false, &FixSelection::default())
                .unwrap();
        assert_eq!(report.scanned_files, 1);
        assert_eq!(report.findings.len(), 1);
        assert_eq!(report.findings[0].path, "document.md");
    }
}
