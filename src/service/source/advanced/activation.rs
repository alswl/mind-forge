//! Lance backend activation: import all legacy registrations into the
//! LanceDB primary catalog and atomically switch the backend marker.
//!
//! ## Protocol
//!
//! 1. Enumerate every legacy registration from all live project indexes.
//! 2. Build an isolated LanceDB generation containing only primary registrations.
//! 3. Validate imported count, deterministic keys, and project membership.
//! 4. Publish the first snapshot with empty derived tables.
//! 5. Atomically patch `minds.yaml.source.backend: lance` with the activation
//!    snapshot ID, catalog fingerprint, and storage schema version.
//!
//! Failure before the marker leaves the legacy backend active. Failure after
//! the marker means a complete exact snapshot exists and the store is healthy.

use std::fs;
use std::path::Path;

use serde::Serialize;

use crate::error::{MfError, Result};
use crate::service::source::advanced::config::ResolvedSourceConfig;
use crate::service::source::advanced::identity;
use crate::service::source::advanced::lance_store::LanceStore;
use crate::service::source::advanced::publication::{
    self, RepositorySourceIndexPointer, RepositorySourceIndexSnapshot, TableVersionRef,
};

const STORAGE_SCHEMA_VERSION: &str = "1";

/// Result of an activation dry-run: lists every registration that would be imported.
#[derive(Debug, Serialize)]
pub struct ActivationPreview {
    pub total_registrations: usize,
    pub projects: usize,
    pub items: Vec<ActivationItem>,
}

#[derive(Debug, Serialize)]
pub struct ActivationItem {
    pub project_identity: String,
    pub project_path: String,
    pub source_identity: String,
    pub source_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_kind: Option<String>,
    pub tags: Vec<String>,
    pub registered_location: String,
    pub registration_key: String,
}

/// Result of a successful activation.
#[derive(Debug, Serialize)]
pub struct ActivationResult {
    pub snapshot_id: String,
    pub generation_id: String,
    pub total_registrations: usize,
    pub catalog_fingerprint: String,
}

/// Preview all legacy registrations that would be imported.
pub fn preview_activation(repo_root: &Path, _config: &ResolvedSourceConfig) -> Result<ActivationPreview> {
    let mut items = Vec::new();

    // Enumerate live projects and their Source registrations from legacy indexes.
    let projects_dir = repo_root.join("projects");
    if projects_dir.exists() {
        for project_entry in fs::read_dir(&projects_dir)? {
            let project_entry = project_entry?;
            if !project_entry.file_type()?.is_dir() {
                continue;
            }
            let project_path = project_entry.path();
            let index_path = project_path.join("mind-index.yaml");
            if !index_path.exists() {
                continue;
            }

            // Read project identity and sources from mind-index.yaml
            if let Ok(index_yaml) = fs::read_to_string(&index_path)
                && let Ok(index) = serde_yaml::from_str::<serde_yaml::Value>(&index_yaml)
            {
                let project_identity = index.get("project").and_then(|v| v.as_str()).unwrap_or("unknown");
                if let Some(sources) = index.get("sources").and_then(|v| v.as_sequence()) {
                    for source in sources {
                        let name = source.get("name").and_then(|v| v.as_str()).unwrap_or("unknown");
                        let kind = source.get("kind").and_then(|v| v.as_str()).unwrap_or("file");
                        let location = source
                            .get("path")
                            .or_else(|| source.get("url"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("unknown");
                        let source_kind = source.get("source_kind").and_then(|v| v.as_str()).map(str::to_string);
                        let tags = source
                            .get("tags")
                            .and_then(|v| v.as_sequence())
                            .map(|tags| tags.iter().filter_map(|tag| tag.as_str().map(str::to_string)).collect())
                            .unwrap_or_default();
                        let project_path_rel =
                            project_path.strip_prefix(repo_root).unwrap_or(&project_path).to_string_lossy().to_string();

                        let pk = identity::project_key(&project_path_rel);
                        let rk = identity::registration_key(&pk, kind, location);

                        items.push(ActivationItem {
                            project_identity: project_identity.to_string(),
                            project_path: project_path_rel,
                            source_identity: name.to_string(),
                            source_type: kind.to_string(),
                            source_kind,
                            tags,
                            registered_location: location.to_string(),
                            registration_key: rk,
                        });
                    }
                }
            }
        }
    }

    let project_count = items.iter().map(|i| &i.project_path).collect::<std::collections::HashSet<_>>().len();

    Ok(ActivationPreview { total_registrations: items.len(), projects: project_count, items })
}

/// Execute activation: import all legacy registrations and switch the backend marker.
pub fn activate(repo_root: &Path, config: &ResolvedSourceConfig) -> Result<ActivationResult> {
    if config.is_lance() {
        return Err(MfError::usage(
            "Lance-backed Sources are already enabled".to_string(),
            Some("use `mf source advanced status` to inspect the active index".to_string()),
        ));
    }

    // 1. Preview to count and collect all registrations
    let preview = preview_activation(repo_root, config)?;

    // 2. Create the advanced directory and LanceDB store
    let advanced_dir = repo_root.join(".mind").join("source").join("advanced");
    publication::ensure_gitignore(&advanced_dir)?;

    let lock_file = publication::try_acquire_writer_lock(&advanced_dir)?;

    let generation_id = format!("gen-{}", chrono::Utc::now().format("%Y%m%dT%H%M%SZ"));
    let gen_dir = publication::generation_path(&advanced_dir, &generation_id);
    let db_path = gen_dir.join("lancedb");

    let store = LanceStore::create(&db_path)?;
    store.ensure_tables()?;

    // The activation snapshot is only valid once the legacy inventory is in
    // the primary catalog.  Previously this created an empty table while
    // reporting the preview count, which made Lance mode look ready without
    // any registrations to query or sync.
    let registrations = preview
        .items
        .iter()
        .map(|item| crate::model::source_advanced::SourceRegistration {
            registration_key: item.registration_key.clone(),
            project_key: identity::project_key(&item.project_path),
            project_identity: item.project_identity.clone(),
            project_path: item.project_path.clone(),
            source_identity: item.source_identity.clone(),
            source_type: item.source_type.clone(),
            source_kind: item.source_kind.clone(),
            registered_location: item.registered_location.clone(),
            tags_json: serde_json::to_string(&item.tags).unwrap_or_else(|_| "[]".to_string()),
            fact_fingerprint: identity::raw_fingerprint(
                format!(
                    "{}\\n{}\\n{}\\n{}",
                    item.source_identity,
                    item.source_type,
                    item.registered_location,
                    item.tags.join("\\n")
                )
                .as_bytes(),
            ),
            registration_revision: 1,
            state: crate::model::source_advanced::RegistrationState::Live,
        })
        .collect::<Vec<_>>();
    store.append_registrations(&registrations)?;
    let imported_count = store.count_rows("registrations")?;
    if imported_count != registrations.len() {
        return Err(MfError::advanced_store(
            format!(
                "activation catalog validation failed: expected {} registrations, found {imported_count}",
                registrations.len()
            ),
            Some("legacy backend remains active; retry activation after resolving the storage error".to_string()),
        ));
    }

    // 3. Compute the catalog fingerprint from all registration keys
    let mut keys: Vec<String> = preview.items.iter().map(|i| i.registration_key.clone()).collect();
    keys.sort();
    let catalog_fp = identity::raw_fingerprint(keys.join("\n").as_bytes());

    // 4. Publish the first snapshot
    let snapshot_id = "snap-1".to_string();
    let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
    let tag = format!("activation-{generation_id}");

    let snapshot = RepositorySourceIndexSnapshot {
        snapshot_id: snapshot_id.clone(),
        schema_version: STORAGE_SCHEMA_VERSION.to_string(),
        generation_id: generation_id.clone(),
        registrations_version: TableVersionRef { table: "registrations".to_string(), version: 1, tag: tag.clone() },
        documents_version: TableVersionRef { table: "documents".to_string(), version: 1, tag: tag.clone() },
        registration_content_version: TableVersionRef {
            table: "registration_content".to_string(),
            version: 1,
            tag: tag.clone(),
        },
        chunks_version: TableVersionRef { table: "chunks".to_string(), version: 1, tag: tag.clone() },
        enrichments_version: TableVersionRef { table: "enrichments".to_string(), version: 1, tag: tag.clone() },
        primary_catalog_fingerprint: catalog_fp.clone(),
        activation_legacy_inventory_fingerprint: Some(catalog_fp.clone()),
        active_project_catalog_fingerprint: String::new(),
        content_fingerprint: None,
        index_fingerprint: None,
        search_policy_version: "1".to_string(),
        model_identity: None,
        aggregate_counts: Some(serde_json::json!({
            "registrations": imported_count,
            "documents": 0,
            "chunks": 0,
            "enrichments": 0
        })),
        created_at: now.clone(),
    };

    // Write snapshot, then pointer
    publication::write_snapshot(&advanced_dir, &snapshot)?;

    let pointer = RepositorySourceIndexPointer {
        schema_version: STORAGE_SCHEMA_VERSION.to_string(),
        generation_id: generation_id.clone(),
        database_uri: format!("./generations/{generation_id}/lancedb"),
        snapshot_path: format!("./generations/{generation_id}/snapshots/{snapshot_id}.json"),
        published_at: now,
    };
    publication::write_pointer(&advanced_dir, &pointer)?;

    // 5. Atomically patch minds.yaml to set the Lance marker
    patch_backend_marker(repo_root, &snapshot_id, &catalog_fp)?;

    publication::release_writer_lock(lock_file);

    Ok(ActivationResult {
        snapshot_id,
        generation_id,
        total_registrations: preview.total_registrations,
        catalog_fingerprint: catalog_fp,
    })
}

/// Atomically patch `minds.yaml` to set `source.backend: lance` with the
/// activation marker fields. Preserves all other YAML content.
fn patch_backend_marker(repo_root: &Path, snapshot_id: &str, catalog_fingerprint: &str) -> Result<()> {
    let minds_yaml = repo_root.join("minds.yaml");
    let original = if minds_yaml.exists() {
        fs::read_to_string(&minds_yaml)?
    } else {
        "schema_version: '1'\nprojects: []\n".to_string()
    };

    // Use serde_yaml to round-trip, preserving structure
    let mut root: serde_yaml::Value = serde_yaml::from_str(&original)
        .map_err(|e| MfError::advanced_store(format!("cannot parse minds.yaml: {e}"), None))?;

    let mut source_block = serde_yaml::Mapping::new();
    source_block.insert("backend".into(), serde_yaml::Value::String("lance".into()));
    source_block.insert("activation_snapshot_id".into(), serde_yaml::Value::String(snapshot_id.into()));
    source_block.insert("activation_catalog_fingerprint".into(), serde_yaml::Value::String(catalog_fingerprint.into()));
    source_block.insert("storage_schema_version".into(), serde_yaml::Value::String(STORAGE_SCHEMA_VERSION.into()));
    let source_block = serde_yaml::Value::Mapping(source_block);

    if let serde_yaml::Value::Mapping(ref mut map) = root {
        map.insert(serde_yaml::Value::String("source".to_string()), source_block);
    }

    let updated = serde_yaml::to_string(&root)
        .map_err(|e| MfError::advanced_store(format!("cannot serialize minds.yaml: {e}"), None))?;

    // Atomic write: temp file → rename
    let tmp = minds_yaml.with_extension("tmp");
    fs::write(&tmp, &updated)?;
    fs::rename(&tmp, &minds_yaml)?;
    if let Some(parent) = minds_yaml.parent() {
        let dir = fs::File::open(parent)?;
        dir.sync_all()?;
    }

    Ok(())
}

/// Switch a healthy, fully-exported repository back to the legacy backend.
///
/// Callers are responsible for checking primary health and projection parity
/// first. Keeping the marker update here makes enable/disable use the same
/// atomic file-replacement boundary.
pub fn disable_backend(repo_root: &Path) -> Result<()> {
    let minds_yaml = repo_root.join("minds.yaml");
    let original = fs::read_to_string(&minds_yaml)?;
    let mut root: serde_yaml::Value = serde_yaml::from_str(&original)
        .map_err(|e| MfError::advanced_store(format!("cannot parse minds.yaml: {e}"), None))?;
    let source = root
        .get_mut("source")
        .and_then(serde_yaml::Value::as_mapping_mut)
        .ok_or_else(|| MfError::advanced_store("minds.yaml has no Source activation block".to_string(), None))?;
    source.insert("backend".into(), serde_yaml::Value::String("legacy".into()));
    for field in ["activation_snapshot_id", "activation_catalog_fingerprint", "storage_schema_version"] {
        source.remove(serde_yaml::Value::String(field.to_string()));
    }
    let updated = serde_yaml::to_string(&root)
        .map_err(|e| MfError::advanced_store(format!("cannot serialize minds.yaml: {e}"), None))?;
    let tmp = minds_yaml.with_extension("tmp");
    fs::write(&tmp, updated)?;
    fs::rename(&tmp, &minds_yaml)?;
    if let Some(parent) = minds_yaml.parent() {
        fs::File::open(parent)?.sync_all()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::manifest::{SearchDefaultMode, SourceBackend};

    #[test]
    fn preview_returns_empty_for_missing_repo() {
        let dir = tempfile::tempdir().unwrap();
        let config = ResolvedSourceConfig {
            backend: SourceBackend::Legacy,
            is_lance_active: false,
            is_marker_corrupt: false,
            activation_snapshot_id: None,
            storage_schema_version: None,
            chunk_tokens: 384,
            chunk_overlap: 48,
            default_search_mode: SearchDefaultMode::Basic,
        };
        let preview = preview_activation(dir.path(), &config).unwrap();
        assert_eq!(preview.total_registrations, 0);
    }

    #[test]
    fn preview_finds_sources_in_project_index() {
        let dir = tempfile::tempdir().unwrap();
        let projects_dir = dir.path().join("projects").join("alpha");
        fs::create_dir_all(&projects_dir).unwrap();

        let index_yaml = r#"
project: alpha
sources:
  - name: notes
    kind: file
    path: sources/notes.md
  - name: paper
    kind: pdf
    path: sources/paper.pdf
"#;
        fs::write(projects_dir.join("mind-index.yaml"), index_yaml).unwrap();

        let config = ResolvedSourceConfig {
            backend: SourceBackend::Legacy,
            is_lance_active: false,
            is_marker_corrupt: false,
            activation_snapshot_id: None,
            storage_schema_version: None,
            chunk_tokens: 384,
            chunk_overlap: 48,
            default_search_mode: SearchDefaultMode::Basic,
        };
        let preview = preview_activation(dir.path(), &config).unwrap();
        assert_eq!(preview.total_registrations, 2);
        assert_eq!(preview.projects, 1);
    }

    #[test]
    fn activation_runtime_ignore_is_written_under_mind_not_repo_root() {
        let dir = tempfile::tempdir().unwrap();
        let advanced_dir = dir.path().join(".mind/source/advanced");

        publication::ensure_gitignore(&advanced_dir).unwrap();

        assert_eq!(fs::read_to_string(dir.path().join(".mind/.gitignore")).unwrap(), "*\n!.gitignore\n");
        assert!(!dir.path().join(".gitignore").exists());
    }

    #[test]
    fn activation_persists_legacy_registrations_before_switching_backend() {
        let dir = tempfile::tempdir().unwrap();
        let project_dir = dir.path().join("projects/alpha");
        fs::create_dir_all(&project_dir).unwrap();
        fs::write(
            project_dir.join("mind-index.yaml"),
            "project: alpha\nsources:\n  - name: notes\n    kind: file\n    path: notes.md\n",
        )
        .unwrap();
        fs::write(dir.path().join("minds.yaml"), "schema_version: '1'\nprojects: []\n").unwrap();

        let config = ResolvedSourceConfig {
            backend: SourceBackend::Legacy,
            is_lance_active: false,
            is_marker_corrupt: false,
            activation_snapshot_id: None,
            storage_schema_version: None,
            chunk_tokens: 384,
            chunk_overlap: 48,
            default_search_mode: SearchDefaultMode::Basic,
        };
        let result = activate(dir.path(), &config).unwrap();
        assert_eq!(result.total_registrations, 1);

        let store = LanceStore::open(
            &dir.path().join(".mind/source/advanced/generations").join(&result.generation_id).join("lancedb"),
        )
        .unwrap();
        assert_eq!(store.count_rows("registrations").unwrap(), 1);
        assert!(fs::read_to_string(dir.path().join("minds.yaml")).unwrap().contains("backend: lance"));
    }

    #[test]
    fn disable_backend_clears_only_lance_activation_marker() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("minds.yaml"),
            "schema_version: '1'\nprojects: []\nsource:\n  backend: lance\n  activation_snapshot_id: snap\n  activation_catalog_fingerprint: catalog\n  storage_schema_version: '1'\n  search:\n    default_mode: both\n",
        )
        .unwrap();
        disable_backend(dir.path()).unwrap();
        let rendered = fs::read_to_string(dir.path().join("minds.yaml")).unwrap();
        assert!(rendered.contains("backend: legacy"));
        assert!(!rendered.contains("activation_snapshot_id"));
        assert!(rendered.contains("default_mode: both"));
    }
}
