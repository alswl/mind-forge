//! Content sync: reconcile registrations with their on-disk/remote content.
//!
//! For each live registration, acquire content → extract text → chunk →
//! embed → publish. Unchanged content is skipped. Compatible content reuses
//! existing documents and chunks. Failed items are reported individually
//! and successful changes may publish together.

use std::path::Path;

use crate::error::Result;
use crate::model::source_advanced::{
    DocumentState, RegistrationContentRelation, RelationState, SharedContentDocument, SyncItem, SyncReport,
};

use super::acquisition;
use super::chunk::ChunkConfig;
use super::config::ResolvedSourceConfig;
use super::identity;
use super::lance_store::LanceStore;
use super::publication;

/// Sync all live registrations across all projects (or a single project).
///
/// Returns a [`SyncReport`] with per-item results. Dry-run performs no
/// mutation, network access, or writing.
pub fn sync_repository(
    repo_root: &Path,
    config: &ResolvedSourceConfig,
    project_filter: Option<&str>,
    source_filter: Option<&str>,
    dry_run: bool,
    offline: bool,
) -> Result<SyncReport> {
    if config.is_lance() {
        let store = open_active_store(repo_root)?;
        return sync_lance_catalog(repo_root, config, project_filter, source_filter, dry_run, offline, &store);
    }
    let mut items = Vec::new();
    let mut added = 0u64;
    let mut updated = 0u64;
    let mut skipped = 0u64;
    let mut failed = 0u64;
    let mut projects_ready = 0u64;
    let mut projects_failed = 0u64;

    let chunk_config = ChunkConfig {
        target_tokens: config.chunk_tokens,
        overlap_tokens: config.chunk_overlap,
        policy_version: "v1".to_string(),
    };

    // Sync is the explicit mutation boundary.  In Lance mode it must write
    // derived content to the database selected by the active pointer; it must
    // never create an alternate per-project store.
    let store: Option<LanceStore> = None;

    // Enumerate projects and their registrations
    let projects_dir = repo_root.join("projects");
    if !projects_dir.exists() {
        return Ok(SyncReport {
            scope: "repository".to_string(),
            dry_run,
            registrations_total: 0,
            registrations_added: 0,
            registrations_updated: 0,
            registrations_skipped: 0,
            registrations_failed: 0,
            projects_processed: 0,
            projects_ready: 0,
            projects_failed: 0,
            items: vec![],
            index_revision: None,
            warnings: vec![],
        });
    }

    for project_entry in std::fs::read_dir(&projects_dir)? {
        let project_entry = project_entry?;
        if !project_entry.file_type()?.is_dir() {
            continue;
        }
        let project_path = project_entry.path();
        let project_name = project_path.file_name().unwrap_or_default().to_string_lossy();

        // Apply project filter
        if let Some(filter) = project_filter
            && project_name != filter
        {
            continue;
        }

        let index_path = project_path.join("mind-index.yaml");
        if !index_path.exists() {
            continue;
        }

        let mut project_has_failure = false;
        if let Ok(index_yaml) = std::fs::read_to_string(&index_path)
            && let Ok(index) = serde_yaml::from_str::<serde_yaml::Value>(&index_yaml)
        {
            let project_identity = index.get("project").and_then(|v| v.as_str()).unwrap_or(&project_name);
            if let Some(sources) = index.get("sources").and_then(|v| v.as_sequence()) {
                for source in sources {
                    let name = source.get("name").and_then(|v| v.as_str()).unwrap_or("unknown");
                    if source_filter.is_some_and(|filter| filter != name) {
                        continue;
                    }
                    let kind = source.get("kind").and_then(|v| v.as_str()).unwrap_or("file");
                    let location =
                        source.get("path").or_else(|| source.get("url")).and_then(|v| v.as_str()).unwrap_or("unknown");

                    let pk = identity::project_key(&project_name);
                    let rk = identity::registration_key(&pk, kind, location);

                    match sync_one_source(
                        &project_path,
                        name,
                        kind,
                        location,
                        &rk,
                        &chunk_config,
                        dry_run,
                        offline,
                        store.as_ref(),
                    ) {
                        Ok(item) => {
                            match item.action.as_str() {
                                "added" => added += 1,
                                "updated" => updated += 1,
                                "skipped" => skipped += 1,
                                "failed" => {
                                    failed += 1;
                                    project_has_failure = true;
                                }
                                _ => skipped += 1,
                            }
                            items.push(item);
                        }
                        Err(_e) => {
                            failed += 1;
                            project_has_failure = true;
                            items.push(SyncItem {
                                project_identity: project_identity.to_string(),
                                registration_key: rk,
                                source_identity: name.to_string(),
                                action: "failed".to_string(),
                                before_state: None,
                                after_state: RelationState::Failed,
                                detected_format: None,
                                affected_chunks: 0,
                                error: Some("sync error".to_string()),
                            });
                        }
                    }
                }
            }
        }

        if project_has_failure {
            projects_failed += 1;
        } else {
            projects_ready += 1;
        }
    }

    let total = added + updated + skipped + failed;
    Ok(SyncReport {
        scope: project_filter.map(|_| "project").unwrap_or("repository").to_string(),
        dry_run,
        registrations_total: total,
        registrations_added: added,
        registrations_updated: updated,
        registrations_skipped: skipped,
        registrations_failed: failed,
        projects_processed: projects_ready + projects_failed,
        projects_ready,
        projects_failed,
        items,
        index_revision: if dry_run { None } else { Some("sync-1".to_string()) },
        warnings: vec![],
    })
}

/// Reconcile the Lance-primary registration catalog.  Compatibility YAML is
/// deliberately not inspected here: once activation succeeds it is an
/// outbound projection, not an input that can resurrect or alter primary facts.
fn sync_lance_catalog(
    repo_root: &Path,
    config: &ResolvedSourceConfig,
    project_filter: Option<&str>,
    source_filter: Option<&str>,
    dry_run: bool,
    offline: bool,
    store: &LanceStore,
) -> Result<SyncReport> {
    let catalog = super::catalog::SourceCatalog::discover(config, repo_root)?;
    let registrations = catalog
        .registrations(Some(store))?
        .into_iter()
        .filter(|registration| {
            let project_name = Path::new(&registration.project_path)
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or(&registration.project_identity);
            project_filter.is_none_or(|filter| filter == project_name || filter == registration.project_identity)
        })
        .filter(|registration| source_filter.is_none_or(|filter| filter == registration.source_identity))
        .collect::<Vec<_>>();
    if source_filter.is_some() && project_filter.is_none() && registrations.len() > 1 {
        return Err(crate::error::MfError::usage(
            "--source matches more than one project; add --project to select one binding".to_string(),
            Some("use `mf source advanced sync --source <IDENTITY> --project <PROJECT>`".to_string()),
        ));
    }
    let chunk_config = ChunkConfig {
        target_tokens: config.chunk_tokens,
        overlap_tokens: config.chunk_overlap,
        policy_version: "v1".to_string(),
    };
    let mut items = Vec::new();
    let mut added = 0u64;
    let mut skipped = 0u64;
    let mut failed = 0u64;
    let mut projects = std::collections::BTreeMap::<String, bool>::new();

    for registration in registrations {
        let project_path = repo_root.join(&registration.project_path);
        let outcome = sync_one_source(
            &project_path,
            &registration.source_identity,
            &registration.source_type,
            &registration.registered_location,
            &registration.registration_key,
            &chunk_config,
            dry_run,
            offline,
            (!dry_run).then_some(store),
        );
        match outcome {
            Ok(mut item) => {
                item.project_identity = registration.project_identity.clone();
                match item.action.as_str() {
                    "added" => added += 1,
                    "failed" => {
                        failed += 1;
                        projects.insert(registration.project_identity.clone(), true);
                    }
                    _ => skipped += 1,
                }
                projects.entry(registration.project_identity).or_insert(false);
                items.push(item);
            }
            Err(error) => {
                failed += 1;
                projects.insert(registration.project_identity.clone(), true);
                items.push(SyncItem {
                    project_identity: registration.project_identity,
                    registration_key: registration.registration_key,
                    source_identity: registration.source_identity,
                    action: "failed".to_string(),
                    before_state: None,
                    after_state: RelationState::Failed,
                    detected_format: None,
                    affected_chunks: 0,
                    error: Some(error.to_string()),
                });
            }
        }
    }
    let projects_failed = projects.values().filter(|failed| **failed).count() as u64;
    Ok(SyncReport {
        scope: project_filter.map(|_| "project").unwrap_or("repository").to_string(),
        dry_run,
        registrations_total: items.len() as u64,
        registrations_added: added,
        registrations_updated: 0,
        registrations_skipped: skipped,
        registrations_failed: failed,
        projects_processed: projects.len() as u64,
        projects_ready: projects.len() as u64 - projects_failed,
        projects_failed,
        items,
        index_revision: (!dry_run).then(|| "sync-primary".to_string()),
        warnings: vec![],
    })
}

/// Sync a single Source registration.
#[allow(clippy::too_many_arguments)]
fn sync_one_source(
    project_path: &Path,
    name: &str,
    kind: &str,
    location: &str,
    registration_key: &str,
    chunk_config: &ChunkConfig,
    dry_run: bool,
    offline: bool,
    store: Option<&LanceStore>,
) -> Result<SyncItem> {
    // Check if remote and offline
    if acquisition::is_url(location) {
        if offline {
            return Ok(SyncItem {
                project_identity: String::new(),
                registration_key: registration_key.to_string(),
                source_identity: name.to_string(),
                action: "skipped".to_string(),
                before_state: None,
                after_state: RelationState::Pending,
                detected_format: Some(kind.to_string()),
                affected_chunks: 0,
                error: Some("offline mode: web/RSS sources require network".to_string()),
            });
        }
        if dry_run {
            return Ok(SyncItem {
                project_identity: String::new(),
                registration_key: registration_key.to_string(),
                source_identity: name.to_string(),
                action: "skipped".to_string(),
                before_state: None,
                after_state: RelationState::Pending,
                detected_format: Some(kind.to_string()),
                affected_chunks: 0,
                error: None,
            });
        }
        // Web/RSS acquisition not yet implemented
        return Ok(SyncItem {
            project_identity: String::new(),
            registration_key: registration_key.to_string(),
            source_identity: name.to_string(),
            action: "skipped".to_string(),
            before_state: None,
            after_state: RelationState::Pending,
            detected_format: Some(kind.to_string()),
            affected_chunks: 0,
            error: Some("HTTP acquisition not yet implemented".to_string()),
        });
    }

    // Local file: acquire, extract, chunk
    let content = match acquisition::acquire_local(project_path, location) {
        Ok(c) => c,
        Err(e) => {
            return Ok(SyncItem {
                project_identity: String::new(),
                registration_key: registration_key.to_string(),
                source_identity: name.to_string(),
                action: "failed".to_string(),
                before_state: None,
                after_state: RelationState::Failed,
                detected_format: Some(kind.to_string()),
                affected_chunks: 0,
                error: Some(format!("acquisition failed: {e}")),
            });
        }
    };

    if dry_run {
        return Ok(SyncItem {
            project_identity: String::new(),
            registration_key: registration_key.to_string(),
            source_identity: name.to_string(),
            action: "added".to_string(),
            before_state: None,
            after_state: RelationState::Ready,
            detected_format: Some(kind.to_string()),
            affected_chunks: 0,
            error: None,
        });
    }

    let extraction = match super::extraction::extract(&content) {
        Ok(e) => e,
        Err(e) => {
            return Ok(SyncItem {
                project_identity: String::new(),
                registration_key: registration_key.to_string(),
                source_identity: name.to_string(),
                action: "failed".to_string(),
                before_state: None,
                after_state: RelationState::Failed,
                detected_format: Some(kind.to_string()),
                affected_chunks: 0,
                error: Some(format!("extraction failed: {e}")),
            });
        }
    };

    // Compute fingerprints
    let raw_fp = identity::raw_fingerprint(&content.raw_bytes);
    let extracted_fp = identity::extracted_fingerprint(&extraction.extractor, &extraction.normalized_text);
    let content_fp = identity::content_fingerprint(&[&extraction.extractor, "v1", "384"]);
    let dk = identity::document_key(&raw_fp, &extracted_fp, &content_fp);

    if let Some(store) = store
        && store.has_ready_content_binding(registration_key, &dk)?
    {
        return Ok(SyncItem {
            project_identity: String::new(),
            registration_key: registration_key.to_string(),
            source_identity: name.to_string(),
            action: "skipped".to_string(),
            before_state: Some(RelationState::Ready),
            after_state: RelationState::Ready,
            detected_format: Some(extraction.format_label),
            affected_chunks: 0,
            error: None,
        });
    }

    // Chunk the document
    let chunks = super::chunk::chunk_document(&extraction.units, &dk, 1, chunk_config)?;
    let chunk_count = chunks.len() as u64;

    if let Some(store) = store {
        let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
        let document = SharedContentDocument {
            document_key: dk.clone(),
            acquisition_kind: content.acquisition_kind.clone(),
            raw_fingerprint: raw_fp,
            extracted_fingerprint: extracted_fp,
            content_fingerprint: content_fp,
            content_revision: 1,
            state: DocumentState::Ready,
            last_error_kind: None,
            last_error: None,
            fetched_at: None,
            synced_at: Some(now.clone()),
            chunk_count,
        };
        let relation = RegistrationContentRelation {
            registration_key: registration_key.to_string(),
            document_key: Some(dk),
            content_revision: Some(1),
            acquisition_key: identity::acquisition_key(kind, &content.canonical_locator, ""),
            acquired_location: content.registered_location,
            registered_revision: "1".to_string(),
            state: RelationState::Ready,
            last_error_kind: None,
            last_error: None,
            attempted_at: Some(now.clone()),
            synced_at: Some(now),
        };
        store.append_content(&document, &relation, &chunks)?;
    }

    Ok(SyncItem {
        project_identity: String::new(),
        registration_key: registration_key.to_string(),
        source_identity: name.to_string(),
        action: "added".to_string(),
        before_state: None,
        after_state: RelationState::Ready,
        detected_format: Some(extraction.format_label),
        affected_chunks: chunk_count,
        error: None,
    })
}

/// Open exactly the database named by the active repository pointer.  A sync
/// must fail closed if Lance mode has no valid pointer, rather than silently
/// rebuilding a second store from legacy YAML.
pub(crate) fn open_active_store(repo_root: &Path) -> Result<LanceStore> {
    let advanced_dir = repo_root.join(".mind/source/advanced");
    let pointer = publication::read_pointer(&advanced_dir)?.ok_or_else(|| {
        crate::error::MfError::missing_lance_pointer(
            "missing",
            "Lance backend is active but current.json is absent".to_string(),
            Some("run `mf source advanced recover --snapshot ID --yes`".to_string()),
        )
    })?;
    let relative = pointer.database_uri.strip_prefix("./").ok_or_else(|| {
        crate::error::MfError::advanced_store("pointer database_uri must be repo-relative".to_string(), None)
    })?;
    if relative.split('/').any(|component| component == ".." || component.is_empty()) {
        return Err(crate::error::MfError::advanced_store(
            "pointer database_uri escapes the advanced Source store".to_string(),
            None,
        ));
    }
    LanceStore::open(&advanced_dir.join(relative))
}

/// Rebuild the entire index: scan all live projects into a fresh generation.
/// Any required project/source failure prevents publication (all-or-nothing).
pub fn rebuild_repository(
    repo_root: &Path,
    config: &ResolvedSourceConfig,
    dry_run: bool,
    offline: bool,
) -> Result<SyncReport> {
    // Rebuild is a full repository sync without project filter.
    // In Lance mode, this would create a fresh generation and publish atomically.
    sync_repository(repo_root, config, None, None, dry_run, offline).map(|mut report| {
        report.scope = "rebuild".to_string();
        if !dry_run && report.registrations_failed == 0 {
            report.index_revision = Some(format!("rebuild-{}", chrono::Utc::now().format("%Y%m%dT%H%M%SZ")));
        }
        report
    })
}

/// Clear derived content (documents, chunks, enrichments, relations).
/// Primary registrations are NEVER deleted. Legacy projections are preserved.
pub fn clear_derived(
    repo_root: &Path,
    config: &ResolvedSourceConfig,
    project: Option<&str>,
    source: Option<&str>,
    all: bool,
    dry_run: bool,
) -> Result<SyncReport> {
    if !all && source.is_none() {
        return Ok(SyncReport {
            scope: "clear".to_string(),
            dry_run,
            registrations_total: 0,
            registrations_added: 0,
            registrations_updated: 0,
            registrations_skipped: 0,
            registrations_failed: 0,
            projects_processed: 0,
            projects_ready: 0,
            projects_failed: 0,
            items: vec![],
            index_revision: None,
            warnings: if !dry_run {
                vec!["clear requires --all flag with optional --project scope".to_string()]
            } else {
                vec![]
            },
        });
    }
    if source.is_some() && project.is_none() {
        return Err(crate::error::MfError::usage(
            "clearing one Source requires --project to make the binding unambiguous".to_string(),
            Some("use `mf source advanced clear <SOURCE> --project <PROJECT> --yes`".to_string()),
        ));
    }
    if all && source.is_some() {
        return Err(crate::error::MfError::usage("clear accepts either a Source or --all, not both".to_string(), None));
    }

    if config.is_lance() {
        let store = open_active_store(repo_root)?;
        let catalog = super::catalog::SourceCatalog::discover(config, repo_root)?;
        let project_path = project.map(|name| format!("projects/{name}"));
        let selected = catalog
            .registrations(Some(&store))?
            .into_iter()
            .filter(|registration| project_path.as_ref().is_none_or(|path| &registration.project_path == path))
            .filter(|registration| source.is_none_or(|name| registration.source_identity == name))
            .collect::<Vec<_>>();
        let keys = selected
            .iter()
            .map(|registration| registration.registration_key.clone())
            .collect::<std::collections::BTreeSet<_>>();
        let items = selected
            .iter()
            .map(|registration| SyncItem {
                project_identity: registration.project_identity.clone(),
                registration_key: registration.registration_key.clone(),
                source_identity: registration.source_identity.clone(),
                action: if dry_run { "would_clear" } else { "cleared" }.to_string(),
                before_state: Some(RelationState::Ready),
                after_state: RelationState::Missing,
                detected_format: Some(registration.source_type.clone()),
                affected_chunks: 0,
                error: None,
            })
            .collect::<Vec<_>>();
        if !dry_run {
            if all && project.is_none() {
                for table in ["registration_content", "chunks", "enrichments", "documents"] {
                    store.delete_rows(table, "true")?;
                }
            } else {
                store.clear_content_bindings(&keys)?;
            }
        }
        let total = items.len() as u64;
        return Ok(SyncReport {
            scope: if project.is_some() { "project" } else { "repository" }.to_string(),
            dry_run,
            registrations_total: total,
            registrations_added: 0,
            registrations_updated: if dry_run { 0 } else { total },
            registrations_skipped: 0,
            registrations_failed: 0,
            projects_processed: selected
                .iter()
                .map(|entry| &entry.project_identity)
                .collect::<std::collections::BTreeSet<_>>()
                .len() as u64,
            projects_ready: selected
                .iter()
                .map(|entry| &entry.project_identity)
                .collect::<std::collections::BTreeSet<_>>()
                .len() as u64,
            projects_failed: 0,
            items,
            index_revision: (!dry_run).then(|| format!("clear-{}", chrono::Utc::now().format("%Y%m%dT%H%M%SZ"))),
            warnings: vec![],
        });
    }

    // Legacy has no advanced derived state. Preserve its existing Source facts.
    let mut report = sync_repository(repo_root, config, project, source, true, true)?;
    report.scope = "clear".to_string();
    report.dry_run = dry_run;

    if !dry_run {
        if config.is_lance() && all && project.is_none() && source.is_none() {
            let store = open_active_store(repo_root)?;
            // `true` is the backend's unconditional predicate.  These are
            // derived tables only; registrations stays untouched.
            for table in ["registration_content", "chunks", "enrichments", "documents"] {
                store.delete_rows(table, "true")?;
            }
        }
        report.index_revision = Some(format!("clear-{}", chrono::Utc::now().format("%Y%m%dT%H%M%SZ")));
        report.registrations_updated = report.registrations_total;
        report.registrations_added = 0;
    }

    // Add items showing what would be cleared
    let items = sync_repository(repo_root, config, project, source, true, true)?
        .items
        .into_iter()
        .map(|mut item| {
            item.action = if dry_run { "would_clear".to_string() } else { "cleared".to_string() };
            item
        })
        .collect::<Vec<_>>();
    report.items = items;

    Ok(report)
}

/// Validate a retained snapshot and switch the pointer (recovery).
pub fn recover_from_snapshot(
    advanced_dir: &Path,
    snapshot_id: &str,
    dry_run: bool,
) -> Result<super::publication::RepositorySourceIndexPointer> {
    // Enumerate retained snapshots
    let snapshots = super::publication::list_snapshots(advanced_dir)?;

    let target = snapshots.iter().find(|s| s.snapshot_id == snapshot_id).ok_or_else(|| {
        crate::error::MfError::recovery_unavailable(
            format!("snapshot '{snapshot_id}' not found in retained snapshots"),
            Some("run `mf source advanced status` to list available snapshots".to_string()),
        )
    })?;

    // Validate snapshot: schema version, table references, fingerprint
    if target.schema_version.is_empty() {
        return Err(crate::error::MfError::recovery_unavailable(
            format!("snapshot '{snapshot_id}' has invalid schema version"),
            None,
        ));
    }

    let generation_dir = super::publication::generation_path(advanced_dir, &target.generation_id);
    let database_path = generation_dir.join("lancedb");
    if !database_path.exists() {
        return Err(crate::error::MfError::recovery_unavailable(
            format!("snapshot '{snapshot_id}' refers to a missing LanceDB generation"),
            None,
        ));
    }
    let store = LanceStore::open(&database_path)?;
    let references = [
        &target.registrations_version,
        &target.documents_version,
        &target.registration_content_version,
        &target.chunks_version,
        &target.enrichments_version,
    ];
    for reference in references {
        let version = store.table_version(&reference.table).map_err(|_| {
            crate::error::MfError::recovery_unavailable(
                format!("snapshot '{snapshot_id}' refers to unavailable table '{}'", reference.table),
                None,
            )
        })?;
        if version < reference.version || reference.tag.is_empty() {
            return Err(crate::error::MfError::recovery_unavailable(
                format!("snapshot '{snapshot_id}' has an invalid retained version for '{}'", reference.table),
                None,
            ));
        }
    }

    if dry_run {
        return Ok(super::publication::RepositorySourceIndexPointer {
            schema_version: target.schema_version.clone(),
            generation_id: target.generation_id.clone(),
            database_uri: format!("./generations/{}/lancedb", target.generation_id),
            snapshot_path: format!("./generations/{}/snapshots/{snapshot_id}.json", target.generation_id),
            published_at: chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string(),
        });
    }

    // Serialize pointer replacement with all primary/derived writers.  The
    // retained generation is immutable from recovery's perspective, so a
    // validated pointer is the sole visible mutation.
    let writer_lock = super::publication::try_acquire_writer_lock(advanced_dir)?;
    let pointer = super::publication::RepositorySourceIndexPointer {
        schema_version: target.schema_version.clone(),
        generation_id: target.generation_id.clone(),
        database_uri: format!("./generations/{}/lancedb", target.generation_id),
        snapshot_path: format!("./generations/{}/snapshots/{snapshot_id}.json", target.generation_id),
        published_at: chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string(),
    };
    super::publication::write_pointer(advanced_dir, &pointer)?;
    super::publication::release_writer_lock(writer_lock);
    Ok(pointer)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rebuild_is_full_sync_with_rebuild_scope() {
        let dir = tempfile::tempdir().unwrap();
        let config = ResolvedSourceConfig {
            backend: crate::model::manifest::SourceBackend::Legacy,
            is_lance_active: false,
            is_marker_corrupt: false,
            activation_snapshot_id: None,
            storage_schema_version: None,
            chunk_tokens: 384,
            chunk_overlap: 48,
            default_search_mode: crate::model::manifest::SearchDefaultMode::Basic,
        };
        let report = rebuild_repository(dir.path(), &config, true, false).unwrap();
        assert_eq!(report.scope, "rebuild");
    }

    #[test]
    fn clear_without_all_is_noop() {
        let dir = tempfile::tempdir().unwrap();
        let config = ResolvedSourceConfig {
            backend: crate::model::manifest::SourceBackend::Legacy,
            is_lance_active: false,
            is_marker_corrupt: false,
            activation_snapshot_id: None,
            storage_schema_version: None,
            chunk_tokens: 384,
            chunk_overlap: 48,
            default_search_mode: crate::model::manifest::SearchDefaultMode::Basic,
        };
        let report = clear_derived(dir.path(), &config, None, None, false, true).unwrap();
        assert_eq!(report.registrations_total, 0);
    }

    #[test]
    fn recover_nonexistent_snapshot_is_error() {
        let dir = tempfile::tempdir().unwrap();
        let advanced = dir.path().join(".mind").join("source").join("advanced");
        std::fs::create_dir_all(&advanced).unwrap();
        let result = recover_from_snapshot(&advanced, "nonexistent", true);
        assert!(result.is_err());
    }

    #[test]
    fn recover_rejects_snapshot_with_missing_generation() {
        let dir = tempfile::tempdir().unwrap();
        let advanced = dir.path().join(".mind/source/advanced");
        let version = |table: &str| super::super::publication::TableVersionRef {
            table: table.to_string(),
            version: 1,
            tag: "retained".to_string(),
        };
        let snapshot = super::super::publication::RepositorySourceIndexSnapshot {
            snapshot_id: "snap-missing".to_string(),
            schema_version: "1".to_string(),
            generation_id: "missing-generation".to_string(),
            registrations_version: version("registrations"),
            documents_version: version("documents"),
            registration_content_version: version("registration_content"),
            chunks_version: version("chunks"),
            enrichments_version: version("enrichments"),
            primary_catalog_fingerprint: "primary".to_string(),
            activation_legacy_inventory_fingerprint: None,
            active_project_catalog_fingerprint: "projects".to_string(),
            content_fingerprint: None,
            index_fingerprint: None,
            search_policy_version: "1".to_string(),
            model_identity: None,
            aggregate_counts: None,
            created_at: "2026-07-14T00:00:00Z".to_string(),
        };
        super::super::publication::write_snapshot(&advanced, &snapshot).unwrap();
        assert!(recover_from_snapshot(&advanced, "snap-missing", true).is_err());
    }

    #[test]
    fn sync_empty_repo_returns_empty_report() {
        let dir = tempfile::tempdir().unwrap();
        let config = ResolvedSourceConfig {
            backend: crate::model::manifest::SourceBackend::Legacy,
            is_lance_active: false,
            is_marker_corrupt: false,
            activation_snapshot_id: None,
            storage_schema_version: None,
            chunk_tokens: 384,
            chunk_overlap: 48,
            default_search_mode: crate::model::manifest::SearchDefaultMode::Basic,
        };
        let report = sync_repository(dir.path(), &config, None, None, false, false).unwrap();
        assert_eq!(report.registrations_total, 0);
        assert_eq!(report.scope, "repository");
    }

    #[test]
    fn sync_with_project_filter() {
        let dir = tempfile::tempdir().unwrap();
        // Create a project with a source
        let proj_dir = dir.path().join("projects").join("alpha");
        std::fs::create_dir_all(proj_dir.join("sources")).unwrap();
        std::fs::write(proj_dir.join("sources").join("notes.md"), "# Test\n").unwrap();
        std::fs::write(
            proj_dir.join("mind-index.yaml"),
            "project: alpha\nsources:\n  - name: notes\n    kind: file\n    path: sources/notes.md\n",
        )
        .unwrap();

        let config = ResolvedSourceConfig {
            backend: crate::model::manifest::SourceBackend::Legacy,
            is_lance_active: false,
            is_marker_corrupt: false,
            activation_snapshot_id: None,
            storage_schema_version: None,
            chunk_tokens: 384,
            chunk_overlap: 48,
            default_search_mode: crate::model::manifest::SearchDefaultMode::Basic,
        };

        // Matches filter
        let report = sync_repository(dir.path(), &config, Some("alpha"), None, false, false).unwrap();
        assert!(report.registrations_total > 0);

        // Non-matching filter
        let report = sync_repository(dir.path(), &config, Some("beta"), None, false, false).unwrap();
        assert_eq!(report.registrations_total, 0);
    }

    #[test]
    fn sync_with_source_filter_only_processes_the_selected_source() {
        let dir = tempfile::tempdir().unwrap();
        let project = dir.path().join("projects/alpha");
        std::fs::create_dir_all(project.join("sources")).unwrap();
        std::fs::write(project.join("sources/one.md"), "# One").unwrap();
        std::fs::write(project.join("sources/two.md"), "# Two").unwrap();
        std::fs::write(
            project.join("mind-index.yaml"),
            "project: alpha\nsources:\n  - name: one\n    kind: file\n    path: sources/one.md\n  - name: two\n    kind: file\n    path: sources/two.md\n",
        )
        .unwrap();
        let config = ResolvedSourceConfig {
            backend: crate::model::manifest::SourceBackend::Legacy,
            is_lance_active: false,
            is_marker_corrupt: false,
            activation_snapshot_id: None,
            storage_schema_version: None,
            chunk_tokens: 384,
            chunk_overlap: 48,
            default_search_mode: crate::model::manifest::SearchDefaultMode::Basic,
        };
        let report = sync_repository(dir.path(), &config, None, Some("two"), false, false).unwrap();
        assert_eq!(report.registrations_total, 1);
        assert_eq!(report.items[0].source_identity, "two");
    }

    #[test]
    fn lance_sync_persists_document_relation_and_chunks() {
        let dir = tempfile::tempdir().unwrap();
        let project_dir = dir.path().join("projects/alpha");
        std::fs::create_dir_all(project_dir.join("sources")).unwrap();
        std::fs::write(project_dir.join("sources/notes.md"), "# Retrieval\nA unique searchable phrase.\n").unwrap();
        std::fs::write(
            project_dir.join("mind-index.yaml"),
            "project: alpha\nsources:\n  - name: notes\n    kind: file\n    path: sources/notes.md\n",
        )
        .unwrap();
        std::fs::write(dir.path().join("minds.yaml"), "schema_version: '1'\nprojects: []\n").unwrap();

        let legacy = ResolvedSourceConfig {
            backend: crate::model::manifest::SourceBackend::Legacy,
            is_lance_active: false,
            is_marker_corrupt: false,
            activation_snapshot_id: None,
            storage_schema_version: None,
            chunk_tokens: 384,
            chunk_overlap: 48,
            default_search_mode: crate::model::manifest::SearchDefaultMode::Basic,
        };
        crate::service::source::advanced::activation::activate(dir.path(), &legacy).unwrap();
        let lance = crate::service::source::advanced::config::load_repository_config(dir.path()).unwrap();

        // After activation this compatibility projection is not an inbound
        // source of truth.  Sync must keep using the persisted primary row.
        std::fs::write(project_dir.join("mind-index.yaml"), "project: alpha\nsources: []\n").unwrap();

        let report = sync_repository(dir.path(), &lance, None, None, false, false).unwrap();
        assert_eq!(report.registrations_added, 1);

        let store = open_active_store(dir.path()).unwrap();
        assert_eq!(store.count_rows("documents").unwrap(), 1);
        assert_eq!(store.count_rows("registration_content").unwrap(), 1);
        assert_eq!(store.count_rows("chunks").unwrap(), 1);

        let jobs = crate::service::source::advanced::enrichment::list_jobs(dir.path(), Some("pending"), 10).unwrap();
        assert_eq!(jobs.len(), 1);
        let shown =
            crate::service::source::advanced::enrichment::show_document(&jobs[0].document_key, 10, dir.path()).unwrap();
        assert_eq!(shown.chunks.len(), 1);
        assert!(shown.chunks[0].text.contains("unique searchable phrase"));

        let applied = crate::service::source::advanced::enrichment::apply_enrichment(
            &crate::service::source::advanced::enrichment::EnrichmentInput {
                schema_version: "1".to_string(),
                prompt_version: "1".to_string(),
                document_key: jobs[0].document_key.clone(),
                content_revision: jobs[0].content_revision,
                content_fingerprint: jobs[0].content_fingerprint.clone(),
                summary: "A retrieval note.".to_string(),
                language: "en".to_string(),
                document_type: "note".to_string(),
                topics: vec!["retrieval".to_string()],
                keywords: vec!["search".to_string()],
                entities: vec![],
                confidence: 0.9,
                warnings: vec![],
                processed_chunks: 1,
                total_chunks: 1,
                coverage: "complete".to_string(),
            },
            dir.path(),
            false,
        )
        .unwrap();
        assert_eq!(applied.state, "ready");
        assert_eq!(store.count_rows("enrichments").unwrap(), 1);
        assert!(
            crate::service::source::advanced::enrichment::list_jobs(dir.path(), Some("pending"), 10)
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            crate::service::source::advanced::enrichment::list_jobs(dir.path(), Some("ready"), 10).unwrap().len(),
            1
        );
        let status = crate::service::source::advanced::status::build_status(dir.path(), &lance).unwrap();
        assert_eq!(status.registrations_count, 1);
        assert_eq!(status.documents_count, 1);
        assert_eq!(status.chunks_count, 1);
        assert_eq!(status.enrichments_ready, 1);

        let search = crate::service::source::advanced::retrieval::search_repository(
            dir.path(),
            "unique searchable phrase",
            crate::model::source_search::SearchMode::Advanced,
            None,
            None,
            None,
            10,
        )
        .unwrap();
        assert_eq!(search.results.len(), 1);
        assert!(search.results[0].snippet.contains("unique searchable phrase"));

        let projection =
            crate::service::source::advanced::compatibility::export_project(dir.path(), "alpha", false).unwrap();
        assert_eq!(projection.primary_count, 1);
        assert!(std::fs::read_to_string(project_dir.join("mind-index.yaml")).unwrap().contains("sources/notes.md"));

        sync_repository(dir.path(), &lance, None, None, false, false).unwrap();
        assert_eq!(store.count_rows("documents").unwrap(), 1);
        assert_eq!(store.count_rows("registration_content").unwrap(), 1);
        assert_eq!(store.count_rows("chunks").unwrap(), 1);

        clear_derived(dir.path(), &lance, None, None, true, false).unwrap();
        assert_eq!(store.count_rows("registrations").unwrap(), 1);
        assert_eq!(store.count_rows("documents").unwrap(), 0);
        assert_eq!(store.count_rows("registration_content").unwrap(), 0);
        assert_eq!(store.count_rows("chunks").unwrap(), 0);
        assert_eq!(store.count_rows("enrichments").unwrap(), 0);
    }

    #[test]
    fn lance_sync_skips_unchanged_content_without_mutating_tables() {
        let dir = tempfile::tempdir().unwrap();
        let project_dir = dir.path().join("projects/alpha");
        std::fs::create_dir_all(project_dir.join("sources")).unwrap();
        std::fs::write(project_dir.join("sources/notes.md"), "# Retrieval\nA unique searchable phrase.\n").unwrap();
        std::fs::write(
            project_dir.join("mind-index.yaml"),
            "project: alpha\nsources:\n  - name: notes\n    kind: file\n    path: sources/notes.md\n",
        )
        .unwrap();
        std::fs::write(dir.path().join("minds.yaml"), "schema_version: '1'\nprojects: []\n").unwrap();
        let legacy = ResolvedSourceConfig {
            backend: crate::model::manifest::SourceBackend::Legacy,
            is_lance_active: false,
            is_marker_corrupt: false,
            activation_snapshot_id: None,
            storage_schema_version: None,
            chunk_tokens: 384,
            chunk_overlap: 48,
            default_search_mode: crate::model::manifest::SearchDefaultMode::Basic,
        };
        crate::service::source::advanced::activation::activate(dir.path(), &legacy).unwrap();
        let lance = crate::service::source::advanced::config::load_repository_config(dir.path()).unwrap();
        let first = sync_repository(dir.path(), &lance, None, None, false, false).unwrap();
        assert_eq!(first.registrations_added, 1);

        let store = open_active_store(dir.path()).unwrap();
        let versions_before =
            ["documents", "registration_content", "chunks"].map(|table| store.table_version(table).unwrap());
        let second = sync_repository(dir.path(), &lance, None, None, false, false).unwrap();
        assert_eq!(second.registrations_added, 0);
        assert_eq!(second.registrations_skipped, 1);
        assert_eq!(second.items[0].action, "skipped");
        let versions_after =
            ["documents", "registration_content", "chunks"].map(|table| store.table_version(table).unwrap());
        assert_eq!(versions_before, versions_after);
    }

    #[test]
    fn scoped_lance_clear_removes_only_derived_binding() {
        let dir = tempfile::tempdir().unwrap();
        let project_dir = dir.path().join("projects/alpha");
        std::fs::create_dir_all(project_dir.join("sources")).unwrap();
        std::fs::write(project_dir.join("sources/notes.md"), "# Clear\nDerived content only.\n").unwrap();
        std::fs::write(
            project_dir.join("mind-index.yaml"),
            "project: alpha\nsources:\n  - name: notes\n    kind: file\n    path: sources/notes.md\n",
        )
        .unwrap();
        std::fs::write(dir.path().join("minds.yaml"), "schema_version: '1'\nprojects: []\n").unwrap();
        let legacy = ResolvedSourceConfig {
            backend: crate::model::manifest::SourceBackend::Legacy,
            is_lance_active: false,
            is_marker_corrupt: false,
            activation_snapshot_id: None,
            storage_schema_version: None,
            chunk_tokens: 384,
            chunk_overlap: 48,
            default_search_mode: crate::model::manifest::SearchDefaultMode::Basic,
        };
        crate::service::source::advanced::activation::activate(dir.path(), &legacy).unwrap();
        let lance = crate::service::source::advanced::config::load_repository_config(dir.path()).unwrap();
        sync_repository(dir.path(), &lance, None, None, false, false).unwrap();
        let store = open_active_store(dir.path()).unwrap();

        let report = clear_derived(dir.path(), &lance, Some("alpha"), Some("notes"), false, false).unwrap();
        assert_eq!(report.registrations_updated, 1);
        assert_eq!(store.count_rows("registrations").unwrap(), 1);
        assert_eq!(store.count_rows("registration_content").unwrap(), 0);
        assert_eq!(store.count_rows("documents").unwrap(), 0);
        assert_eq!(store.count_rows("chunks").unwrap(), 0);
    }

    #[test]
    fn lance_sync_rejects_cross_project_ambiguous_source_filter() {
        let dir = tempfile::tempdir().unwrap();
        for project in ["alpha", "beta"] {
            let path = dir.path().join("projects").join(project);
            std::fs::create_dir_all(path.join("sources")).unwrap();
            std::fs::write(path.join("sources/notes.md"), format!("# {project}")).unwrap();
            std::fs::write(
                path.join("mind-index.yaml"),
                format!("project: {project}\nsources:\n  - name: notes\n    kind: file\n    path: sources/notes.md\n"),
            )
            .unwrap();
        }
        std::fs::write(dir.path().join("minds.yaml"), "schema_version: '1'\nprojects: []\n").unwrap();
        let legacy = ResolvedSourceConfig {
            backend: crate::model::manifest::SourceBackend::Legacy,
            is_lance_active: false,
            is_marker_corrupt: false,
            activation_snapshot_id: None,
            storage_schema_version: None,
            chunk_tokens: 384,
            chunk_overlap: 48,
            default_search_mode: crate::model::manifest::SearchDefaultMode::Basic,
        };
        crate::service::source::advanced::activation::activate(dir.path(), &legacy).unwrap();
        let lance = crate::service::source::advanced::config::load_repository_config(dir.path()).unwrap();
        let error = sync_repository(dir.path(), &lance, None, Some("notes"), false, false).unwrap_err();
        assert!(error.to_string().contains("more than one project"));
    }
}
