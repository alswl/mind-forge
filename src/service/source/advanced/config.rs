//! Repository-level Source configuration resolution.
//!
//! Reads the `minds.yaml.source` block for the user's declared backend
//! intent, and resolves whether Lance is actually usable on this machine by
//! checking disk facts: does a corpus pointer resolve, and do its tables
//! match the schema this build requires. Nothing here can be "corrupt" the
//! way the old three-field activation marker could — there is no declared
//! value to disagree with reality (spec 075, research D5).

use std::path::Path;

use crate::error::{MfError, Result};
use crate::model::manifest::{RepositorySourceConfig, SearchDefaultMode, SourceBackend};

/// Load the Source configuration selected by a Mind repository and resolve
/// whether Lance is actually usable here.
///
/// Keeping this at the service boundary prevents command handlers from
/// accidentally treating an activated Lance repository as legacy.  An absent
/// `source` block deliberately retains the legacy defaults. This never
/// fails on missing or absent machine-local state (FR-003) — the corpus
/// pointer on disk is the only thing that decides whether Lance is active.
pub fn load_repository_config(repo_root: &Path) -> Result<ResolvedSourceConfig> {
    let manifest_path = repo_root.join("minds.yaml");
    let source_cfg =
        if manifest_path.exists() { crate::service::repo::load_manifest(&manifest_path)?.source } else { None };
    let activated_here = crate::service::repo::load_local_state(repo_root)?.activated;

    let wants_lance = source_cfg.as_ref().is_some_and(|c| c.backend == SourceBackend::Lance);
    let corpus_found = wants_lance
        && crate::service::source::advanced::publication::read_pointer(
            &crate::service::source::advanced::advanced_store_dir(repo_root),
        )?
        .is_some();

    Ok(ResolvedSourceConfig::from_config(source_cfg.as_ref(), corpus_found, activated_here))
}

/// Resolve the configured embedding vector dimension for a repository.
///
/// Reads `minds.yaml`'s `source.advanced.embedding_dimension`, falling back to
/// the default when unset. Table creation, chunk appends, and vector queries
/// must all agree on this value.
pub fn embedding_dimension_for(repo_root: &std::path::Path) -> usize {
    let manifest_path = repo_root.join("minds.yaml");
    if !manifest_path.exists() {
        return super::lance_store::DEFAULT_VECTOR_DIMENSION;
    }
    crate::service::repo::load_manifest(&manifest_path)
        .ok()
        .and_then(|m| m.source)
        .and_then(|s| s.advanced)
        .map(|a| a.embedding_dimension as usize)
        .unwrap_or(super::lance_store::DEFAULT_VECTOR_DIMENSION)
}

/// Schema compatibility of the on-disk registration tables against what this
/// build requires, determined by inspecting the tables' actual structure —
/// never by a recorded self-declared version (spec 075 FR-002).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchemaStatus {
    /// Backend is not Lance, or no corpus is on disk yet: the question does
    /// not apply.
    NotApplicable,
    Current,
    /// The tables predate what this build requires.
    Older,
    /// The tables are structured for a newer build than this one.
    Newer,
}

/// Resolved backend usability and search configuration for a Source operation.
///
/// `backend` is the declared intent from `minds.yaml`. `is_lance_active`
/// reflects whether Lance is actually usable right now: backend is `lance`
/// and a corpus pointer resolves on disk. There is deliberately no "corrupt"
/// state — an unresolved pointer just means the corpus is missing, which
/// `corpus_missing` reports and which `sync` resolves by adopting or
/// activating (spec 075 FR-004, FR-005).
#[derive(Debug, Clone)]
pub struct ResolvedSourceConfig {
    pub backend: SourceBackend,
    pub is_lance_active: bool,
    /// True when the backend is `lance` but no corpus pointer resolves —
    /// the case that used to be reported as "marker corrupt". `sync` fixes
    /// this by adopting an existing corpus or activating fresh.
    pub corpus_missing: bool,
    /// Whether this machine has completed activation, per its own
    /// machine-local status. Informational only — never gates behaviour.
    pub activated_here: bool,
    /// Chunk token count from advanced config (default 384).
    pub chunk_tokens: u32,
    /// Chunk overlap from advanced config (default 48).
    pub chunk_overlap: u32,
    /// HTTP acquisition byte limit from advanced config (default 64 MiB).
    pub fetch_max_bytes: u64,
    /// HTTP acquisition timeout from advanced config (default 30 s).
    pub fetch_timeout_seconds: u32,
    /// HTTP acquisition redirect limit from advanced config (default 5).
    pub fetch_max_redirects: u32,
    /// Search mode used when the CLI does not provide an explicit override.
    pub default_search_mode: SearchDefaultMode,
}

impl ResolvedSourceConfig {
    /// Resolve from an optional `RepositorySourceConfig` plus the disk facts
    /// `load_repository_config` already gathered. Never fails: there is no
    /// declared value here that can disagree with reality.
    fn from_config(config: Option<&RepositorySourceConfig>, corpus_found: bool, activated_here: bool) -> Self {
        let default = RepositorySourceConfig::default();
        let cfg = config.unwrap_or(&default);
        let wants_lance = cfg.backend == SourceBackend::Lance;
        let is_lance_active = wants_lance && corpus_found;

        let adv = cfg.advanced.as_ref();
        Self {
            backend: if is_lance_active { SourceBackend::Lance } else { SourceBackend::Legacy },
            is_lance_active,
            corpus_missing: wants_lance && !corpus_found,
            activated_here,
            chunk_tokens: adv.map(|a| a.chunk_tokens).unwrap_or(384),
            chunk_overlap: adv.map(|a| a.chunk_overlap).unwrap_or(48),
            fetch_max_bytes: adv.map(|a| a.fetch_max_bytes).unwrap_or(64 * 1024 * 1024),
            fetch_timeout_seconds: adv.map(|a| a.fetch_timeout_seconds).unwrap_or(30),
            fetch_max_redirects: adv.map(|a| a.fetch_max_redirects).unwrap_or(5),
            default_search_mode: if is_lance_active {
                cfg.search.as_ref().map(|search| search.default_mode).unwrap_or_default()
            } else {
                SearchDefaultMode::Basic
            },
        }
    }

    /// Returns true when the Lance backend is fully active and healthy.
    pub fn is_lance(&self) -> bool {
        self.backend == SourceBackend::Lance && self.is_lance_active
    }

    /// Determine schema compatibility by inspecting the actual on-disk table
    /// structure (spec 075 FR-002). Legacy or not-yet-active repositories
    /// have nothing to check and are always compatible.
    pub fn schema_status(&self, repo_root: &Path) -> Result<SchemaStatus> {
        if !self.is_lance_active {
            return Ok(SchemaStatus::NotApplicable);
        }
        let store = crate::service::source::advanced::sync::open_active_store(repo_root)?;
        store.registrations_schema_status()
    }

    /// Reject serving reads/incremental sync from an out-of-date snapshot.
    ///
    /// Schema compatibility is read from the tables themselves — there is no
    /// recorded version left to go stale or be hand-edited (spec 075 FR-002,
    /// FR-010). Legacy/inactive backends pass.
    pub fn require_current_schema(&self, repo_root: &Path) -> Result<()> {
        use crate::service::source::advanced::activation::STORAGE_SCHEMA_VERSION;
        match self.schema_status(repo_root)? {
            SchemaStatus::NotApplicable | SchemaStatus::Current => Ok(()),
            SchemaStatus::Older => Err(MfError::advanced_store(
                format!("repository Source index predates schema v{STORAGE_SCHEMA_VERSION} required by this build"),
                Some("run `mf source sync --rebuild` to regenerate the index with the current schema".to_string()),
            )),
            SchemaStatus::Newer => Err(MfError::advanced_store(
                format!(
                    "repository Source index is a newer schema than this build supports (requires v{STORAGE_SCHEMA_VERSION})"
                ),
                Some("upgrade mf to a version that supports this repository's schema".to_string()),
            )),
        }
    }

    /// Returns true when the legacy backend should be used.
    pub fn is_legacy(&self) -> bool {
        !self.is_lance()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::manifest::{AdvancedSourceConfig, RepositorySourceConfig, SourceBackend};

    #[test]
    fn resolved_config_legacy_by_default() {
        let resolved = ResolvedSourceConfig::from_config(None, false, false);
        assert!(resolved.is_legacy());
        assert!(!resolved.is_lance());
        assert_eq!(resolved.chunk_tokens, 384);
        assert_eq!(resolved.chunk_overlap, 48);
    }

    #[test]
    fn resolved_config_lance_active_when_corpus_found() {
        let config = RepositorySourceConfig {
            backend: SourceBackend::Lance,
            search: None,
            advanced: Some(AdvancedSourceConfig { chunk_tokens: 256, chunk_overlap: 32, ..Default::default() }),
        };
        let resolved = ResolvedSourceConfig::from_config(Some(&config), true, true);
        assert!(resolved.is_lance());
        assert!(!resolved.corpus_missing);
        assert!(resolved.activated_here);
        assert_eq!(resolved.chunk_tokens, 256);
        assert_eq!(resolved.chunk_overlap, 32);
        assert_eq!(resolved.default_search_mode, SearchDefaultMode::Both);
    }

    #[test]
    fn declared_lance_without_corpus_is_missing_not_corrupt() {
        // FR-004/FR-005: no corpus on disk is a recoverable state that sync
        // resolves, never an error at config-resolution time.
        let config = RepositorySourceConfig { backend: SourceBackend::Lance, search: None, advanced: None };
        let resolved = ResolvedSourceConfig::from_config(Some(&config), false, false);
        assert!(!resolved.is_lance());
        assert!(resolved.corpus_missing);
        assert!(!resolved.activated_here);
    }

    #[test]
    fn absent_local_state_never_fails_resolution() {
        // FR-003: no machine-local state file at all must resolve cleanly.
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("minds.yaml"), "schema_version: '1'\nprojects: []\nsource:\n  backend: lance\n")
            .unwrap();
        assert!(!dir.path().join(".mind-forge/state.yaml").exists());

        let resolved = load_repository_config(dir.path()).unwrap();
        assert!(!resolved.is_lance(), "no corpus on disk yet");
        assert!(resolved.corpus_missing);
        assert!(!resolved.activated_here);
    }

    #[test]
    fn declared_legacy_ignores_local_activation_status() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("minds.yaml"),
            "schema_version: '1'\nprojects: []\nsource:\n  backend: legacy\n",
        )
        .unwrap();
        crate::service::repo::save_local_state(
            dir.path(),
            &crate::model::manifest::LocalSourceState { activated: true },
        )
        .unwrap();

        let resolved = load_repository_config(dir.path()).unwrap();
        assert!(resolved.is_legacy());
        assert!(!resolved.corpus_missing);
    }
}
