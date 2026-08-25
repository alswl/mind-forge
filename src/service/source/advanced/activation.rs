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

/// Current authoritative storage schema version. Bumped 1→2 for spec 071
/// (per-registration `context_json` + source `imported_by`); bumped 2→3 for
/// spec 075 (`added_at`/`updated_at`/`extras_json`, completing the record so
/// the project-index mirror is lossless). An older snapshot must be rebuilt
/// (`mf source sync --rebuild`) before search/sync will serve it — compatibility
/// is determined by inspecting the tables' actual structure, not this constant
/// (see `ResolvedSourceConfig::schema_status`); this value only names what the
/// build requires in diagnostics.
pub const STORAGE_SCHEMA_VERSION: &str = "3";

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
    #[serde(default = "empty_json_object")]
    pub labels_json: String,
    #[serde(default = "empty_json_object")]
    pub annotations_json: String,
    /// The legacy entry's own creation/modification timestamps, when present
    /// (spec 075 FR-011). Activation must not stamp a fresh time over
    /// history a user has already accrued; only a legacy entry with no
    /// timestamp at all falls back to the activation moment.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub added_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
}

fn empty_json_object() -> String {
    "{}".to_string()
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
                // Project identity is stored in older sequence-form indexes; the
                // current mapping-form index omits it, so fall back to the
                // project directory name.
                let project_identity =
                    index.get("project").and_then(|v| v.as_str()).map(str::to_string).unwrap_or_else(|| {
                        project_path.file_name().and_then(|n| n.to_str()).unwrap_or("unknown").to_string()
                    });
                let project_path_rel =
                    project_path.strip_prefix(repo_root).unwrap_or(&project_path).to_string_lossy().to_string();
                let pk = identity::project_key(&project_path_rel);

                // `sources` is a mapping keyed by registered path in current
                // indexes, but a sequence in older ones — accept both.
                let source_entries: Vec<&serde_yaml::Value> = match index.get("sources") {
                    Some(serde_yaml::Value::Mapping(map)) => map.values().collect(),
                    Some(serde_yaml::Value::Sequence(seq)) => seq.iter().collect(),
                    _ => Vec::new(),
                };
                for source in source_entries {
                    let name = source.get("name").and_then(|v| v.as_str()).unwrap_or("unknown");
                    // Registrations record the file kind under `type`; older
                    // indexes used `kind`.
                    let kind =
                        source.get("type").or_else(|| source.get("kind")).and_then(|v| v.as_str()).unwrap_or("file");
                    // Prefer a real local path, then a URL. `and_then(as_str)`
                    // before `or_else` so a present-but-null `path` does not
                    // shadow the `url` fallback (which broke web Sources).
                    let location = source
                        .get("path")
                        .and_then(|v| v.as_str())
                        .or_else(|| source.get("url").and_then(|v| v.as_str()))
                        .unwrap_or("unknown");
                    let source_kind = source.get("source_kind").and_then(|v| v.as_str()).map(str::to_string);
                    let tags = source
                        .get("tags")
                        .and_then(|v| v.as_sequence())
                        .map(|tags| tags.iter().filter_map(|tag| tag.as_str().map(str::to_string)).collect())
                        .unwrap_or_default();

                    let rk = identity::registration_key(&pk, kind, location);
                    let non_empty = |value: Option<&str>| value.filter(|v| !v.is_empty()).map(str::to_string);
                    let added_at = non_empty(source.get("added_at").and_then(|v| v.as_str()));
                    let updated_at = non_empty(source.get("updated_at").and_then(|v| v.as_str()));

                    items.push(ActivationItem {
                        project_identity: project_identity.clone(),
                        project_path: project_path_rel.clone(),
                        source_identity: name.to_string(),
                        source_type: kind.to_string(),
                        source_kind,
                        tags,
                        registered_location: location.to_string(),
                        registration_key: rk,
                        labels_json: "{}".to_string(),
                        annotations_json: "{}".to_string(),
                        added_at,
                        updated_at,
                    });
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
            Some("use `mf source status` to inspect the active index".to_string()),
        ));
    }

    // 1. Preview to count and collect all registrations
    let preview = preview_activation(repo_root, config)?;

    // 2. Create the advanced directory and LanceDB store
    let advanced_dir = super::advanced_store_dir(repo_root);
    publication::ensure_gitignore(&advanced_dir)?;

    let lock_file = publication::try_acquire_writer_lock(&advanced_dir)?;

    let generation_id = format!("gen-{}", chrono::Utc::now().format("%Y%m%dT%H%M%SZ"));
    let gen_dir = publication::generation_path(&advanced_dir, &generation_id);
    let db_path = gen_dir.join("lancedb");

    let store = LanceStore::create(&db_path)?.with_dimension(super::config::embedding_dimension_for(repo_root));
    store.ensure_tables()?;

    // The activation snapshot is only valid once the legacy inventory is in
    // the primary catalog.  Previously this created an empty table while
    // reporting the preview count, which made Lance mode look ready without
    // any registrations to query or sync.
    let activated_at = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
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
            labels_json: item.labels_json.clone(),
            annotations_json: item.annotations_json.clone(),
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
            context_json: None,
            imported_by_json: None,
            added_at: Some(item.added_at.clone().unwrap_or_else(|| activated_at.clone())),
            updated_at: Some(item.updated_at.clone().unwrap_or_else(|| activated_at.clone())),
            extras_json: None,
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

    // 5. Keep backend selection in tracked config; this machine's activation
    // status is machine-local state (spec 075 FR-001).
    patch_backend_marker(repo_root)?;

    publication::release_writer_lock(lock_file);

    Ok(ActivationResult {
        snapshot_id,
        generation_id,
        total_registrations: preview.total_registrations,
        catalog_fingerprint: catalog_fp,
    })
}

/// Atomically patch `minds.yaml` to set `source.backend: lance`, then record
/// this machine's activation status in the gitignored local state file.
fn patch_backend_marker(repo_root: &Path) -> Result<()> {
    let minds_yaml = repo_root.join("minds.yaml");
    let original = if minds_yaml.exists() {
        fs::read_to_string(&minds_yaml)?
    } else {
        "schema_version: '1'\nprojects: []\n".to_string()
    };

    // Use serde_yaml to round-trip, preserving structure
    let mut root: serde_yaml::Value = serde_yaml::from_str(&original)
        .map_err(|e| MfError::advanced_store(format!("cannot parse minds.yaml: {e}"), None))?;

    if let serde_yaml::Value::Mapping(ref mut map) = root {
        let source_key = serde_yaml::Value::String("source".to_string());
        // Preserve any existing `source` sub-blocks (e.g. `advanced`, `search`)
        // and overlay only the backend field. No activation marker is written
        // here at all (spec 075 FR-001) — the three legacy keys are stripped
        // defensively in case a pre-075 binary left them behind.
        if !matches!(map.get(&source_key), Some(serde_yaml::Value::Mapping(_))) {
            map.insert(source_key.clone(), serde_yaml::Value::Mapping(serde_yaml::Mapping::new()));
        }
        let source_block =
            map.get_mut(&source_key).and_then(serde_yaml::Value::as_mapping_mut).expect("source is a mapping");
        source_block.insert("backend".into(), serde_yaml::Value::String("lance".into()));
        source_block.remove("activation_snapshot_id");
        source_block.remove("activation_catalog_fingerprint");
        source_block.remove("storage_schema_version");
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

    // This machine has completed activation. That is the only fact recorded
    // locally — no snapshot id, fingerprint, or schema version (FR-001).
    crate::service::repo::save_local_state(repo_root, &crate::model::manifest::LocalSourceState { activated: true })?;

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
    crate::service::repo::save_local_state(repo_root, &Default::default())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::manifest::{SearchDefaultMode, SourceBackend};

    #[test]
    fn preview_excludes_articles_from_source_catalog() {
        let dir = tempfile::tempdir().unwrap();
        let proj = dir.path().join("projects/alpha");
        fs::create_dir_all(&proj).unwrap();
        fs::write(
            proj.join("mind-index.yaml"),
            "schema: '1'\nsources:\n  s1:\n    name: s1\n    type: file\n    path: sources/a.md\narticles:\n  docs/my-article:\n    title: My Article\n    type: blog\n    article_path: docs/my-article\n",
        )
        .unwrap();
        let config = ResolvedSourceConfig {
            backend: SourceBackend::Legacy,
            is_lance_active: false,
            corpus_missing: false,
            activated_here: false,
            chunk_tokens: 384,
            chunk_overlap: 48,
            fetch_max_bytes: 64 * 1024 * 1024,
            fetch_timeout_seconds: 30,
            fetch_max_redirects: 5,
            default_search_mode: SearchDefaultMode::Basic,
        };
        let preview = preview_activation(dir.path(), &config).unwrap();
        assert_eq!(preview.items.len(), 1);
        assert_eq!(preview.items[0].source_identity, "s1");
        assert!(preview.items.iter().all(|item| item.source_type != "article"));
    }

    #[test]
    fn patch_backend_marker_preserves_existing_advanced_block() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("minds.yaml"),
            "schema: '1'\nprojects: []\nsource:\n  advanced:\n    embedding_endpoint: http://x/v1/embeddings\n    embedding_model: m\n    embedding_dimension: 1024\n",
        )
        .unwrap();
        patch_backend_marker(dir.path()).unwrap();
        let updated = fs::read_to_string(dir.path().join("minds.yaml")).unwrap();
        assert!(updated.contains("backend: lance"), "marker must be written: {updated}");
        // The advanced block (embedding config) must survive the marker patch.
        assert!(updated.contains("embedding_endpoint"), "advanced block dropped: {updated}");
        assert!(updated.contains("embedding_dimension: 1024"), "advanced block dropped: {updated}");
        // No activation marker fields are written at all (FR-001).
        assert!(!updated.contains("activation_snapshot_id"));
        let state = crate::service::repo::load_local_state(dir.path()).unwrap();
        assert!(state.activated, "local state must record this machine as activated");
    }

    #[test]
    fn preview_returns_empty_for_missing_repo() {
        let dir = tempfile::tempdir().unwrap();
        let config = ResolvedSourceConfig {
            backend: SourceBackend::Legacy,
            is_lance_active: false,
            corpus_missing: false,
            activated_here: false,
            chunk_tokens: 384,
            chunk_overlap: 48,
            fetch_max_bytes: 64 * 1024 * 1024,
            fetch_timeout_seconds: 30,
            fetch_max_redirects: 5,
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
            corpus_missing: false,
            activated_here: false,
            chunk_tokens: 384,
            chunk_overlap: 48,
            fetch_max_bytes: 64 * 1024 * 1024,
            fetch_timeout_seconds: 30,
            fetch_max_redirects: 5,
            default_search_mode: SearchDefaultMode::Basic,
        };
        let preview = preview_activation(dir.path(), &config).unwrap();
        assert_eq!(preview.total_registrations, 2);
        assert_eq!(preview.projects, 1);
    }

    #[test]
    fn preview_imports_mapping_form_index_with_type_field() {
        // `source index` writes `sources` as a mapping keyed by path, with the
        // file kind under `type` and no top-level `project` field. Activation
        // must import these, deriving project identity from the directory name.
        let dir = tempfile::tempdir().unwrap();
        let projects_dir = dir.path().join("projects").join("alpha");
        fs::create_dir_all(&projects_dir).unwrap();

        let index_yaml = r#"
schema: '1'
sources:
  sources/file/notes.md:
    name: notes
    type: file
    path: sources/file/notes.md
    tags: []
"#;
        fs::write(projects_dir.join("mind-index.yaml"), index_yaml).unwrap();

        let config = ResolvedSourceConfig {
            backend: SourceBackend::Legacy,
            is_lance_active: false,
            corpus_missing: false,
            activated_here: false,
            chunk_tokens: 384,
            chunk_overlap: 48,
            fetch_max_bytes: 64 * 1024 * 1024,
            fetch_timeout_seconds: 30,
            fetch_max_redirects: 5,
            default_search_mode: SearchDefaultMode::Basic,
        };
        let preview = preview_activation(dir.path(), &config).unwrap();
        assert_eq!(preview.total_registrations, 1);
        assert_eq!(preview.projects, 1);
        let item = &preview.items[0];
        assert_eq!(item.project_identity, "alpha");
        assert_eq!(item.source_type, "file");
        assert_eq!(item.registered_location, "sources/file/notes.md");
    }

    #[test]
    fn activation_runtime_ignore_is_written_under_mind_forge_not_repo_root() {
        let dir = tempfile::tempdir().unwrap();
        let advanced_dir = super::super::advanced_store_dir(dir.path());

        publication::ensure_gitignore(&advanced_dir).unwrap();

        // The ignore scopes only the rebuildable cache; committed config
        // (renders/publisher/enrichments) under `.mind-forge/` stays tracked.
        assert_eq!(fs::read_to_string(dir.path().join(".mind-forge/.gitignore")).unwrap(), "/cache/\n");
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
            corpus_missing: false,
            activated_here: false,
            chunk_tokens: 384,
            chunk_overlap: 48,
            fetch_max_bytes: 64 * 1024 * 1024,
            fetch_timeout_seconds: 30,
            fetch_max_redirects: 5,
            default_search_mode: SearchDefaultMode::Basic,
        };
        let result = activate(dir.path(), &config).unwrap();
        assert_eq!(result.total_registrations, 1);

        let store = LanceStore::open(
            &super::super::advanced_store_dir(dir.path())
                .join("generations")
                .join(&result.generation_id)
                .join("lancedb"),
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
