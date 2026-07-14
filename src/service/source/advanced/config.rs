//! Repository-level Source configuration resolution.
//!
//! Reads and validates the `minds.yaml.source` block, resolves
//! effective backend mode, and validates the Lance activation marker.

use crate::error::{MfError, Result};
use crate::model::manifest::{RepositorySourceConfig, SearchDefaultMode, SourceBackend};

/// Load and validate the Source configuration selected by a Mind repository.
///
/// Keeping this at the service boundary prevents command handlers from
/// accidentally treating an activated Lance repository as legacy.  An absent
/// `source` block deliberately retains the legacy defaults.
pub fn load_repository_config(repo_root: &std::path::Path) -> Result<ResolvedSourceConfig> {
    let manifest = crate::service::repo::load_manifest(&repo_root.join("minds.yaml"))?;
    ResolvedSourceConfig::from_config(manifest.source.as_ref())
}

/// Validated backend mode and search configuration for a Source operation.
#[derive(Debug, Clone)]
pub struct ResolvedSourceConfig {
    pub backend: SourceBackend,
    pub is_lance_active: bool,
    pub is_marker_corrupt: bool,
    /// The activation snapshot ID when Lance is active.
    pub activation_snapshot_id: Option<String>,
    /// Storage schema version when Lance is active.
    pub storage_schema_version: Option<String>,
    /// Chunk token count from advanced config (default 384).
    pub chunk_tokens: u32,
    /// Chunk overlap from advanced config (default 48).
    pub chunk_overlap: u32,
    /// Search mode used when the CLI does not provide an explicit override.
    pub default_search_mode: SearchDefaultMode,
}

impl ResolvedSourceConfig {
    /// Resolve from an optional `RepositorySourceConfig`.
    ///
    /// Returns an error when the Lance marker is corrupt (backend is `lance`
    /// but one or more activation fields are missing).
    pub fn from_config(config: Option<&RepositorySourceConfig>) -> Result<Self> {
        let default = RepositorySourceConfig::default();
        let cfg = config.unwrap_or(&default);

        if cfg.is_lance_marker_corrupt() {
            return Err(MfError::usage(
                "Lance backend is selected but the activation marker is incomplete. \
                 The marker requires activation_snapshot_id, activation_catalog_fingerprint, \
                 and storage_schema_version."
                    .to_string(),
                Some(
                    "run `mf source advanced enable` to activate, or set backend to `legacy` \
                     in minds.yaml if Lance is not yet activated"
                        .to_string(),
                ),
            ));
        }

        let adv = cfg.advanced.as_ref();
        Ok(Self {
            backend: cfg.effective_backend(),
            is_lance_active: cfg.is_lance_active(),
            is_marker_corrupt: false,
            activation_snapshot_id: cfg.activation_snapshot_id.clone(),
            storage_schema_version: cfg.storage_schema_version.clone(),
            chunk_tokens: adv.map(|a| a.chunk_tokens).unwrap_or(384),
            chunk_overlap: adv.map(|a| a.chunk_overlap).unwrap_or(48),
            default_search_mode: if cfg.is_lance_active() {
                cfg.search.as_ref().map(|search| search.default_mode).unwrap_or_default()
            } else {
                SearchDefaultMode::Basic
            },
        })
    }

    /// Returns true when the Lance backend is fully active and healthy.
    pub fn is_lance(&self) -> bool {
        self.backend == SourceBackend::Lance && self.is_lance_active
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
        let resolved = ResolvedSourceConfig::from_config(None).unwrap();
        assert!(resolved.is_legacy());
        assert!(!resolved.is_lance());
        assert_eq!(resolved.chunk_tokens, 384);
        assert_eq!(resolved.chunk_overlap, 48);
    }

    #[test]
    fn resolved_config_lance_active() {
        let config = RepositorySourceConfig {
            backend: SourceBackend::Lance,
            activation_snapshot_id: Some("snap-1".into()),
            activation_catalog_fingerprint: Some("fp-1".into()),
            storage_schema_version: Some("1".into()),
            search: None,
            advanced: Some(AdvancedSourceConfig { chunk_tokens: 256, chunk_overlap: 32, ..Default::default() }),
        };
        let resolved = ResolvedSourceConfig::from_config(Some(&config)).unwrap();
        assert!(resolved.is_lance());
        assert_eq!(resolved.chunk_tokens, 256);
        assert_eq!(resolved.chunk_overlap, 32);
        assert_eq!(resolved.activation_snapshot_id.as_deref(), Some("snap-1"));
        assert_eq!(resolved.default_search_mode, SearchDefaultMode::Both);
    }

    #[test]
    fn resolved_config_corrupt_marker_is_error() {
        let config = RepositorySourceConfig {
            backend: SourceBackend::Lance,
            activation_snapshot_id: None, // missing
            activation_catalog_fingerprint: Some("fp-1".into()),
            storage_schema_version: Some("1".into()),
            search: None,
            advanced: None,
        };
        let result = ResolvedSourceConfig::from_config(Some(&config));
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("incomplete"));
    }

    #[test]
    fn load_repository_config_uses_manifest_source_block() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("minds.yaml"),
            "schema_version: '1'\nprojects: []\nsource:\n  backend: lance\n  activation_snapshot_id: snap-1\n  activation_catalog_fingerprint: fp-1\n  storage_schema_version: '1'\n",
        )
        .unwrap();

        let resolved = load_repository_config(dir.path()).unwrap();
        assert!(resolved.is_lance());
        assert_eq!(resolved.activation_snapshot_id.as_deref(), Some("snap-1"));
    }
}
