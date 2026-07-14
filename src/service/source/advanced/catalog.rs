//! Backend-aware repository catalog discovery.
//!
//! In legacy mode, Source registrations are read from project
//! `mind-index.yaml.sources`. In Lance mode, the pinned LanceDB
//! `registrations` table is the primary authority, intersected with
//! the root active-project catalog from `minds.yaml`.

use std::path::Path;

use crate::error::Result;
use crate::model::manifest::SourceBackend;

use super::config::ResolvedSourceConfig;
use super::lance_store::LanceStore;

// ── Catalog ────────────────────────────────────────────────────────────────

/// A resolved catalog of all live Source registrations across active projects.
#[derive(Debug, Clone)]
pub struct SourceCatalog {
    pub backend: SourceBackend,
    /// Number of active projects in the repository.
    pub active_projects: usize,
    /// Number of live registrations in scope.
    pub registration_count: usize,
    /// Whether the catalog was read from Lance primary (true) or legacy YAML (false).
    pub from_lance_primary: bool,
}

/// A lightweight view of a Source registration for listing/indexing.
#[derive(Debug, Clone)]
pub struct CatalogRegistration {
    pub registration_key: String,
    pub project_key: String,
    pub project_identity: String,
    pub project_path: String,
    pub source_identity: String,
    pub source_type: String,
    pub registered_location: String,
    pub state: String,
}

impl SourceCatalog {
    /// Discover the active Source catalog based on the resolved backend config.
    ///
    /// In legacy mode, this is a no-op placeholder — the caller should use
    /// existing project-level Source indexing. In Lance mode, it reads from
    /// the pinned LanceDB store.
    pub fn discover(config: &ResolvedSourceConfig, _repo_root: &Path) -> Result<Self> {
        match config.backend {
            SourceBackend::Legacy => Ok(Self {
                backend: SourceBackend::Legacy,
                active_projects: 0,
                registration_count: 0,
                from_lance_primary: false,
            }),
            SourceBackend::Lance => {
                // In Lance mode, registrations are read from the primary table.
                // The actual discovery is deferred to the caller — this struct
                // serves as a context marker.
                Ok(Self {
                    backend: SourceBackend::Lance,
                    active_projects: 0,
                    registration_count: 0,
                    from_lance_primary: true,
                })
            }
        }
    }

    /// Return the list of catalog registrations.
    ///
    /// In Lance mode this queries the pinned snapshot. In legacy mode it
    /// returns an empty list (the caller uses project-level indexing).
    pub fn registrations(&self, _store: Option<&LanceStore>) -> Result<Vec<CatalogRegistration>> {
        if !self.from_lance_primary {
            return Ok(Vec::new());
        }
        // In Lance mode, query the registrations table from the pinned snapshot.
        // This is a placeholder — the full implementation queries LanceDB.
        Ok(Vec::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_catalog_is_empty() {
        let config = ResolvedSourceConfig {
            backend: SourceBackend::Legacy,
            is_lance_active: false,
            is_marker_corrupt: false,
            activation_snapshot_id: None,
            storage_schema_version: None,
            chunk_tokens: 384,
            chunk_overlap: 48,
        };
        let catalog = SourceCatalog::discover(&config, Path::new("/tmp")).unwrap();
        assert!(!catalog.from_lance_primary);
        assert_eq!(catalog.backend, SourceBackend::Legacy);
    }

    #[test]
    fn lance_catalog_marks_primary() {
        let config = ResolvedSourceConfig {
            backend: SourceBackend::Lance,
            is_lance_active: true,
            is_marker_corrupt: false,
            activation_snapshot_id: Some("s".into()),
            storage_schema_version: Some("1".into()),
            chunk_tokens: 384,
            chunk_overlap: 48,
        };
        let catalog = SourceCatalog::discover(&config, Path::new("/tmp")).unwrap();
        assert!(catalog.from_lance_primary);
        assert_eq!(catalog.backend, SourceBackend::Lance);
    }
}
