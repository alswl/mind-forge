//! Global `minds-terms.yaml` persistence.
//!
//! Uses the schema-version layout: a `schema_version` key followed by a
//! `terms: [...]` list. Load and save go through serde_yaml so the file is
//! always a clean, machine-managed serialisation.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::defaults;
use crate::error::{MfError, Result};
use crate::model::term::{validate_corrections, Term};
use crate::service::util::{atomic_write, validate_schema_version};

// ── File model ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GlobalTermsFile {
    #[serde(alias = "schema")]
    schema_version: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    terms: Vec<Term>,
}

pub(super) const GLOBAL_TERMS_FILENAME: &str = "minds-terms.yaml";

pub(super) fn path_for(repo_root: &Path) -> std::path::PathBuf {
    repo_root.join(GLOBAL_TERMS_FILENAME)
}

// ── Load / Save ─────────────────────────────────────────────────────────────

/// Load terms from `<repo_root>/minds-terms.yaml`.
///
/// Returns an empty vec when the file is missing.
pub fn load(repo_root: &Path) -> Result<Vec<Term>> {
    load_inner(repo_root, true)
}

/// Load global terms without enforcing correction cross-field invariants.
///
/// Mirrors [`crate::service::index::load_lenient`]: used only by term
/// management/self-repair paths so an already-invalid `minds-terms.yaml` can
/// still be read and repaired via the CLI.
pub fn load_lenient(repo_root: &Path) -> Result<Vec<Term>> {
    load_inner(repo_root, false)
}

fn load_inner(repo_root: &Path, validate: bool) -> Result<Vec<Term>> {
    let path = path_for(repo_root);
    let content = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
        Err(e) => return Err(MfError::from(e)),
    };

    let file: GlobalTermsFile = serde_yaml::from_str(&content).map_err(|e| MfError::ParseError {
        kind: "yaml".to_string(),
        path: path.clone(),
        detail: e.to_string(),
    })?;
    validate_schema_version(&file.schema_version, &path)?;
    if validate {
        validate_corrections(&file.terms).map_err(|m| MfError::usage(m, None::<String>))?;
    }
    Ok(file.terms)
}

/// Save terms to `<repo_root>/minds-terms.yaml`.
pub fn save(repo_root: &Path, terms: &[Term]) -> Result<()> {
    let file = GlobalTermsFile { schema_version: defaults::SCHEMA_VERSION.to_string(), terms: terms.to_vec() };
    let yaml = serde_yaml::to_string(&file).map_err(|e| MfError::Internal(e.into()))?;
    atomic_write(&path_for(repo_root), &yaml)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn global_load_rejects_standalone_with_substring() {
        let dir = tempfile::tempdir().unwrap();
        let content = r#"schema_version: '1'
terms:
- term: AIDC
  corrections:
  - original: aidc
    correct: AIDC
    match: substring
    boundary: standalone
"#;
        std::fs::write(path_for(dir.path()), content).unwrap();
        let err = load(dir.path()).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("standalone is only valid with match: word"), "got: {msg}");
        assert_eq!(err.exit_code(), crate::exit::ExitCode::UsageError);
    }

    #[test]
    fn global_load_allows_pinyin_with_standalone() {
        // Pinyin matches ignore the boundary field — standalone+pinyin passes validation.
        let dir = tempfile::tempdir().unwrap();
        let content = r#"schema_version: '1'
terms:
- term: KAIFEIDI
  corrections:
  - original: kaifeidi
    correct: kaifeidi
    match: pinyin
    boundary: standalone
"#;
        std::fs::write(path_for(dir.path()), content).unwrap();
        load(dir.path()).expect("pinyin+standalone should pass validation");
    }

    #[test]
    fn global_load_rejects_standalone_edge_hyphen() {
        let dir = tempfile::tempdir().unwrap();
        let content = r#"schema_version: '1'
terms:
- term: AIDC
  corrections:
  - original: aidc-
    correct: AIDC
    boundary: standalone
"#;
        std::fs::write(path_for(dir.path()), content).unwrap();
        let err = load(dir.path()).unwrap_err();
        assert!(err.to_string().contains("identifier-character edges"));
    }

    #[test]
    fn global_load_accepts_valid_corrections() {
        let dir = tempfile::tempdir().unwrap();
        let content = r#"schema_version: '1'
terms:
- term: AIDC
  corrections:
  - original: aidc
    correct: AIDC
    boundary: standalone
"#;
        std::fs::write(path_for(dir.path()), content).unwrap();
        let terms = load(dir.path()).unwrap();
        assert_eq!(terms.len(), 1);
        assert_eq!(terms[0].corrections.len(), 1);
    }
}
