//! Aggregate status: backend health, primary catalog, retained snapshots,
//! pending intents, projections, registrations, documents, and enrichments.
//!
//! Read-only — never creates `.mind`, mutates LanceDB, or repairs state.

use std::fs;
use std::path::Path;

use arrow_array::Array;

use crate::error::Result;
use crate::model::source_advanced::{AdvancedSourceStatusReport, IndexStatus, ProjectAdvancedStatus, ProjectionStatus};

use super::config::ResolvedSourceConfig;
use super::lifecycle;
use super::publication;

/// Build an aggregate status report for the repository.
pub fn build_status(repo_root: &Path, config: &ResolvedSourceConfig) -> Result<AdvancedSourceStatusReport> {
    let advanced_dir = repo_root.join(".mind").join("source").join("advanced");

    let (index_status, activation_snapshot_id, primary_fp, retained_snapshots, pending_intents) = if config.is_lance() {
        let pointer = publication::read_pointer(&advanced_dir).unwrap_or(None);
        let snapshots = publication::list_snapshots(&advanced_dir).unwrap_or_default();
        let intents = lifecycle::list_pending_intents(&advanced_dir).unwrap_or_default();

        match pointer {
            Some(p) => (
                IndexStatus::Ready,
                Some(p.snapshot_path),
                Some(p.generation_id),
                snapshots.len() as u32,
                intents.len() as u32,
            ),
            None => (
                IndexStatus::Missing,
                config.activation_snapshot_id.clone(),
                None,
                snapshots.len() as u32,
                intents.len() as u32,
            ),
        }
    } else {
        (IndexStatus::Inactive, None, None, 0, 0)
    };

    // Enumerate project-level stats
    let mut projects = Vec::new();
    let mut total_regs = 0u64;
    let mut total_docs = 0u64;
    let mut total_rels = 0u64;
    let mut total_chunks = 0u64;
    let mut enrichments_ready = 0u64;
    let mut enrichments_pending = 0u64;
    let mut enrichments_failed = 0u64;

    // Lance-mode status reports the primary/derived store, not compatibility
    // YAML. All reads are explicitly side-effect-free.
    if config.is_lance()
        && let Ok(store) = super::sync::open_active_store(repo_root)
    {
        total_regs = store.count_rows("registrations")? as u64;
        total_docs = store.count_rows("documents")? as u64;
        total_rels = store.count_rows("registration_content")? as u64;
        total_chunks = store.count_rows("chunks")? as u64;
        for batch in store.scan_rows("enrichments")? {
            if let Some(states) = batch
                .column_by_name("state")
                .and_then(|column| column.as_any().downcast_ref::<arrow_array::StringArray>())
            {
                for row in 0..batch.num_rows() {
                    match states.value(row) {
                        "ready" => enrichments_ready += 1,
                        "failed" => enrichments_failed += 1,
                        _ => enrichments_pending += 1,
                    }
                }
            }
        }
    }

    let projects_dir = repo_root.join("projects");
    if projects_dir.exists() {
        for entry in fs::read_dir(&projects_dir)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let proj_path = entry.path();
            let proj_name = entry.file_name().to_string_lossy().to_string();
            let index_path = proj_path.join("mind-index.yaml");

            let mut proj_regs = 0u64;
            let mut proj_ready = 0u64;
            let proj_pending = 0u64;
            let proj_failed = 0u64;

            if index_path.exists()
                && let Ok(data) = fs::read_to_string(&index_path)
                && let Ok(index) = serde_yaml::from_str::<serde_yaml::Value>(&data)
                && let Some(sources) = index.get("sources").and_then(|v| v.as_sequence())
            {
                proj_regs = sources.len() as u64;
                proj_ready = sources.len() as u64; // all legacy sources are "ready"
            }

            if !config.is_lance() {
                total_regs += proj_regs;
                total_rels += proj_regs;
            }

            projects.push(ProjectAdvancedStatus {
                project_key: proj_name.clone(),
                project_identity: proj_name,
                registrations: proj_regs,
                relations_ready: proj_ready,
                relations_pending: proj_pending,
                relations_failed: proj_failed,
                projection_state: ProjectionStatus::Current,
            });
        }
    }

    // Sort projects by identity
    projects.sort_by(|a, b| a.project_identity.cmp(&b.project_identity));

    Ok(AdvancedSourceStatusReport {
        backend: if config.is_lance() { "lance" } else { "legacy" }.to_string(),
        index_status,
        activation_snapshot_id,
        activation_catalog_fingerprint: None,
        primary_catalog_fingerprint: primary_fp,
        retained_snapshots,
        pending_intents,
        registrations_count: total_regs,
        documents_count: total_docs,
        relations_count: total_rels,
        chunks_count: total_chunks,
        enrichments_ready,
        enrichments_pending,
        enrichments_failed,
        projects,
        warnings: if config.is_lance() && index_status == IndexStatus::Missing {
            vec![
                "Lance backend is active but no index pointer found — run `mf source advanced enable` first"
                    .to_string(),
            ]
        } else {
            vec![]
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::manifest::{SearchDefaultMode, SourceBackend};

    #[test]
    fn legacy_status_reports_inactive() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("projects").join("alpha")).unwrap();
        fs::write(
            dir.path().join("projects").join("alpha").join("mind-index.yaml"),
            "project: alpha\nsources:\n  - name: n\n    kind: file\n    path: s/n.md\n",
        )
        .unwrap();

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
        let report = build_status(dir.path(), &config).unwrap();
        assert_eq!(report.backend, "legacy");
        assert_eq!(report.index_status, IndexStatus::Inactive);
        assert_eq!(report.registrations_count, 1);
        assert_eq!(report.projects.len(), 1);
    }

    #[test]
    fn lance_missing_pointer_reports_missing() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("projects")).unwrap();
        let config = ResolvedSourceConfig {
            backend: SourceBackend::Lance,
            is_lance_active: true,
            is_marker_corrupt: false,
            activation_snapshot_id: Some("s".into()),
            storage_schema_version: Some("1".into()),
            chunk_tokens: 384,
            chunk_overlap: 48,
            default_search_mode: SearchDefaultMode::Both,
        };
        let report = build_status(dir.path(), &config).unwrap();
        assert_eq!(report.index_status, IndexStatus::Missing);
        assert!(!report.warnings.is_empty());
    }
}
