//! Global `minds-terms.yaml` persistence.
//!
//! Uses the schema-version layout: a `schema_version` key followed by a
//! `terms: [...]` list. Load and save go through serde_yaml so the file is
//! always a clean, machine-managed serialisation.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::defaults;
use crate::error::{MfError, Result};
use crate::model::term::Term;
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
    Ok(file.terms)
}

/// Save terms to `<repo_root>/minds-terms.yaml`.
pub fn save(repo_root: &Path, terms: &[Term]) -> Result<()> {
    let file = GlobalTermsFile { schema_version: defaults::SCHEMA_VERSION.to_string(), terms: terms.to_vec() };
    let yaml = serde_yaml::to_string(&file).map_err(|e| MfError::Internal(e.into()))?;
    atomic_write(&path_for(repo_root), &yaml)
}
