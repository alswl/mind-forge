//! Export the whole-repository advanced Sources corpus to a self-contained bundle.
//!
//! `export_bundle` reads every source-kind registration, its content documents,
//! relations, and enrichments from the Lance store; captures original bytes for
//! each document (verified against the stored `raw_fingerprint`); and writes a
//! portable directory of JSONL records + content-addressed blobs.

use std::collections::HashSet;
use std::fs;
use std::path::Path;

use arrow_array::{Array, Float32Array, Int64Array, RecordBatch, StringArray, UInt32Array, UInt64Array};
use serde::Serialize;

use crate::error::{MfError, Result};
use crate::model::source_advanced::{DocumentState, RegistrationState, RelationState};

use super::bundle::{
    self, BundleDocumentRecord, BundleEnrichmentRecord, BundleManifest, BundleRegistrationContentRecord,
    BundleRegistrationRecord, ContentFidelity, FidelitySummary, ModelIdentityRecord, ObjectCounts,
};
use super::config::load_repository_config;
use super::publication;
use super::sync;
use super::trace::article_kind;

// ── Public API ────────────────────────────────────────────────────────────────

/// Result payload for `export_bundle`.
#[derive(Debug, Clone, Serialize)]
pub struct ExportReport {
    pub bundle_path: String,
    pub source_snapshot_id: String,
    pub counts: ObjectCounts,
    pub fidelity_summary: FidelitySummary,
    pub content_digest: String,
    pub dry_run: bool,
}

/// Export the whole-repository source corpus to a self-contained bundle.
///
/// `output` is the destination directory (will be created).  `dry_run` reports
/// planned counts without writing or touching the network.  `force` allows
/// writing into an existing/non-empty directory.
pub fn export_bundle(repo_root: &Path, output: &Path, force: bool, dry_run: bool) -> Result<ExportReport> {
    let config = load_repository_config(repo_root)?;
    if !config.is_lance() {
        return Err(MfError::usage(
            "export requires an active Lance backend",
            Some("run `mf source sync` first".to_string()),
        ));
    }
    let store = sync::open_active_store(repo_root)?;
    let advanced_dir = super::advanced_store_dir(repo_root);
    let pointer = publication::read_pointer(&advanced_dir)?.ok_or_else(|| {
        MfError::usage("no published corpus found", Some("run `mf source sync` to build the corpus first".to_string()))
    })?;

    // Read the snapshot for metadata (snapshot_id, model_identity, schema version).
    // The snapshot_path on the pointer is of the form "./generations/<gen>/snapshots/<id>.json".
    let snapshot_id = pointer
        .snapshot_path
        .rsplit('/')
        .next()
        .and_then(|name| name.strip_suffix(".json"))
        .unwrap_or(&pointer.generation_id);
    let snapshot = publication::read_snapshot(&advanced_dir, &pointer.generation_id, snapshot_id)?;

    // ── 1. Scan source registrations ───────────────────────────────────────
    let mut registrations: Vec<BundleRegistrationRecord> = Vec::new();
    for batch in store.scan_rows("registrations")? {
        for row in 0..batch.num_rows() {
            let kind = nullable_str(string_col(&batch, "source_kind")?, row);
            // Export only raw source kinds (not derived article/project/term).
            if kind.as_deref().is_some_and(article_kind::is_derived_kind) {
                continue;
            }
            let revision = int64_col(&batch, "registration_revision")?.value(row);
            registrations.push(BundleRegistrationRecord {
                registration_key: string_col(&batch, "registration_key")?.value(row).to_string(),
                project_key: string_col(&batch, "project_key")?.value(row).to_string(),
                project_identity: string_col(&batch, "project_identity")?.value(row).to_string(),
                project_path: string_col(&batch, "project_path")?.value(row).to_string(),
                source_identity: string_col(&batch, "source_identity")?.value(row).to_string(),
                source_type: string_col(&batch, "source_type")?.value(row).to_string(),
                source_kind: kind,
                registered_location: string_col(&batch, "registered_location")?.value(row).to_string(),
                tags_json: string_col(&batch, "tags_json")?.value(row).to_string(),
                labels_json: string_col(&batch, "labels_json")?.value(row).to_string(),
                annotations_json: string_col(&batch, "annotations_json")?.value(row).to_string(),
                fact_fingerprint: string_col(&batch, "fact_fingerprint")?.value(row).to_string(),
                registration_revision: revision,
                state: parse_registration_state(string_col(&batch, "state")?.value(row)),
            });
        }
    }
    registrations.sort_by(|a, b| a.registration_key.cmp(&b.registration_key));
    let registration_keys: HashSet<&str> =
        registrations.iter().map(|registration| registration.registration_key.as_str()).collect();

    if registrations.is_empty() {
        return Err(MfError::usage(
            "no source registrations found in the corpus",
            Some("add sources and run `mf source sync` first".to_string()),
        ));
    }

    // ── 2. Scan documents ──────────────────────────────────────────────────
    let mut documents: Vec<BundleDocumentRecord> = Vec::new();
    for batch in store.scan_rows("documents")? {
        for row in 0..batch.num_rows() {
            let raw_fp = string_col(&batch, "raw_fingerprint")?.value(row).to_string();
            documents.push(BundleDocumentRecord {
                document_key: string_col(&batch, "document_key")?.value(row).to_string(),
                acquisition_kind: string_col(&batch, "acquisition_kind")?.value(row).to_string(),
                raw_fingerprint: raw_fp.clone(),
                extracted_fingerprint: string_col(&batch, "extracted_fingerprint")?.value(row).to_string(),
                content_fingerprint: string_col(&batch, "content_fingerprint")?.value(row).to_string(),
                content_revision: int64_col(&batch, "content_revision")?.value(row),
                state: parse_doc_state(string_col(&batch, "state")?.value(row)),
                last_error_kind: nullable_str(string_col(&batch, "last_error_kind")?, row),
                last_error: nullable_str(string_col(&batch, "last_error")?, row),
                chunk_count: uint64_col(&batch, "chunk_count")?.value(row),
                fidelity: ContentFidelity::ByteExact,
                extracted_text_ref: None,
            });
        }
    }
    documents.sort_by(|a, b| a.document_key.cmp(&b.document_key));

    // ── 3. Scan registration_content ───────────────────────────────────────
    let mut reg_content: Vec<BundleRegistrationContentRecord> = Vec::new();
    for batch in store.scan_rows("registration_content")? {
        for row in 0..batch.num_rows() {
            reg_content.push(BundleRegistrationContentRecord {
                registration_key: string_col(&batch, "registration_key")?.value(row).to_string(),
                document_key: nullable_str(string_col(&batch, "document_key")?, row),
                content_revision: if int64_col(&batch, "content_revision")?.is_null(row) {
                    None
                } else {
                    Some(int64_col(&batch, "content_revision")?.value(row))
                },
                acquisition_key: string_col(&batch, "acquisition_key")?.value(row).to_string(),
                acquired_location: string_col(&batch, "acquired_location")?.value(row).to_string(),
                registered_revision: string_col(&batch, "registered_revision")?.value(row).to_string(),
                state: parse_rel_state(string_col(&batch, "state")?.value(row)),
                last_error_kind: nullable_str(string_col(&batch, "last_error_kind")?, row),
                last_error: nullable_str(string_col(&batch, "last_error")?, row),
                attempted_at: nullable_str(string_col(&batch, "attempted_at")?, row),
                synced_at: nullable_str(string_col(&batch, "synced_at")?, row),
            });
        }
    }
    reg_content.retain(|relation| registration_keys.contains(relation.registration_key.as_str()));
    reg_content.sort_by(|a, b| a.registration_key.cmp(&b.registration_key));
    let document_keys: HashSet<&str> =
        reg_content.iter().filter_map(|relation| relation.document_key.as_deref()).collect();
    documents.retain(|document| document_keys.contains(document.document_key.as_str()));

    // ── 4. Scan enrichments ────────────────────────────────────────────────
    let mut enrichments: Vec<BundleEnrichmentRecord> = Vec::new();
    for batch in store.scan_rows("enrichments")? {
        for row in 0..batch.num_rows() {
            enrichments.push(BundleEnrichmentRecord {
                enrichment_key: string_col(&batch, "enrichment_key")?.value(row).to_string(),
                document_key: string_col(&batch, "document_key")?.value(row).to_string(),
                content_revision: int64_col(&batch, "content_revision")?.value(row),
                schema_version: string_col(&batch, "schema_version")?.value(row).to_string(),
                prompt_version: string_col(&batch, "prompt_version")?.value(row).to_string(),
                summary: string_col(&batch, "summary")?.value(row).to_string(),
                language: string_col(&batch, "language")?.value(row).to_string(),
                document_type: string_col(&batch, "document_type")?.value(row).to_string(),
                topics_json: string_col(&batch, "topics_json")?.value(row).to_string(),
                keywords_json: string_col(&batch, "keywords_json")?.value(row).to_string(),
                entities_json: string_col(&batch, "entities_json")?.value(row).to_string(),
                confidence: float32_col(&batch, "confidence")?.value(row),
                warnings_json: string_col(&batch, "warnings_json")?.value(row).to_string(),
                processed_chunks: uint32_col(&batch, "processed_chunks")?.value(row),
                total_chunks: uint32_col(&batch, "total_chunks")?.value(row),
                coverage: string_col(&batch, "coverage")?.value(row).to_string(),
                state: string_col(&batch, "state")?.value(row).to_string(),
                generated_at: string_col(&batch, "generated_at")?.value(row).to_string(),
            });
        }
    }
    enrichments.retain(|enrichment| document_keys.contains(enrichment.document_key.as_str()));
    enrichments.sort_by(|a, b| a.enrichment_key.cmp(&b.enrichment_key));

    // ── 5. Build fidelity summary ──────────────────────────────────────────
    let byte_exact = documents.iter().filter(|d| d.fidelity == ContentFidelity::ByteExact).count() as u64;
    let extracted_only = documents.len() as u64 - byte_exact;
    let fidelity_summary = FidelitySummary { byte_exact, extracted_only };

    let counts = ObjectCounts {
        registrations: registrations.len() as u64,
        documents: documents.len() as u64,
        registration_content: reg_content.len() as u64,
        enrichments: enrichments.len() as u64,
        blobs: byte_exact,
    };

    // ── 6. Model identity ──────────────────────────────────────────────────
    let model_identity = snapshot.model_identity.as_ref().and_then(|v| {
        Some(ModelIdentityRecord {
            embedding_model: v.get("embedding_model")?.as_str().map(str::to_string),
            embedding_dimension: v.get("embedding_dimension")?.as_u64().map(|d| d as u32),
            embedding_provider_endpoint: v.get("embedding_provider_endpoint")?.as_str().map(str::to_string),
        })
    });

    if dry_run {
        return Ok(ExportReport {
            bundle_path: output.to_string_lossy().to_string(),
            source_snapshot_id: snapshot.snapshot_id.clone(),
            counts,
            fidelity_summary,
            content_digest: String::new(),
            dry_run: true,
        });
    }

    // ── 7. Write bundle directory ──────────────────────────────────────────
    if output.exists() && !force {
        let non_empty = fs::read_dir(output).ok().is_some_and(|mut rd| rd.next().is_some());
        if non_empty {
            return Err(MfError::usage(
                format!("output directory '{}' is not empty", output.display()),
                Some("use --force to overwrite".to_string()),
            ));
        }
    }
    fs::create_dir_all(output).map_err(|e| MfError::advanced_store(format!("cannot create output dir: {e}"), None))?;

    // Write JSONL files
    bundle::write_jsonl(&output.join(bundle::REGISTRATIONS_JSONL), &registrations)?;
    bundle::write_jsonl(&output.join(bundle::DOCUMENTS_JSONL), &documents)?;
    bundle::write_jsonl(&output.join(bundle::REGISTRATION_CONTENT_JSONL), &reg_content)?;
    bundle::write_jsonl(&output.join(bundle::ENRICHMENTS_JSONL), &enrichments)?;

    // Build a registration_key → (project_path, registered_location) lookup.
    let reg_lookup: std::collections::HashMap<&str, (&str, &str)> = registrations
        .iter()
        .map(|r| (r.registration_key.as_str(), (r.project_path.as_str(), r.registered_location.as_str())))
        .collect();

    // Write content blobs (byte_exact documents only).
    let content_dir = output.join(bundle::CONTENT_DIR);
    fs::create_dir_all(&content_dir)
        .map_err(|e| MfError::advanced_store(format!("cannot create content dir: {e}"), None))?;
    for doc in &documents {
        if doc.fidelity != ContentFidelity::ByteExact {
            continue;
        }
        // Find the registration that binds this document to get the source file path.
        let binding = reg_content.iter().find(|rc| rc.document_key.as_deref() == Some(&doc.document_key));
        let reg_key = binding.map(|rc| rc.registration_key.as_str()).unwrap_or("");
        let (project_path, registered_location) = reg_lookup.get(reg_key).copied().unwrap_or(("", ""));
        if registered_location.is_empty()
            || registered_location.starts_with("http://")
            || registered_location.starts_with("https://")
        {
            continue;
        }
        // registered_location is project-relative (e.g. "sources/file/notes.md").
        // Resolve against repo_root + project_path.
        let abs = repo_root.join(project_path).join(registered_location);
        if let Ok(bytes) = fs::read(&abs) {
            if bundle::sha256_hex(&bytes) != doc.raw_fingerprint {
                return Err(MfError::advanced_store(
                    format!("source content changed during export: {}", abs.display()),
                    Some("run `mf source sync` and retry the export".to_string()),
                ));
            }
            bundle::capture_blob(&content_dir, &bytes)?;
        }
    }

    // Write config (secrets stripped — only model identity & schema version)
    let config_json = serde_json::json!({
        "storage_schema_version": snapshot.schema_version,
        "model_identity": model_identity,
    });
    fs::write(output.join(bundle::CONFIG_FILE), serde_json::to_string_pretty(&config_json).map_err(MfError::Json)?)
        .map_err(MfError::Io)?;

    // Write manifest
    let digest_files = bundle::digest_file_list(output);
    let content_digest = bundle::compute_bundle_digest(&digest_files)?;
    let manifest = BundleManifest {
        bundle_format_version: bundle::BUNDLE_FORMAT_VERSION.to_string(),
        created_at: chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string(),
        source_snapshot_id: snapshot.snapshot_id.clone(),
        storage_schema_version: snapshot.schema_version.clone(),
        model_identity,
        counts: counts.clone(),
        fidelity_summary: fidelity_summary.clone(),
        content_digest: content_digest.clone(),
    };
    let manifest_json = serde_json::to_string_pretty(&manifest).map_err(MfError::Json)?;
    fs::write(output.join(bundle::MANIFEST_FILE), manifest_json).map_err(MfError::Io)?;

    Ok(ExportReport {
        bundle_path: output.to_string_lossy().to_string(),
        source_snapshot_id: snapshot.snapshot_id,
        counts,
        fidelity_summary,
        content_digest,
        dry_run: false,
    })
}

// ── Arrow column helpers ──────────────────────────────────────────────────────

fn string_col<'a>(batch: &'a RecordBatch, name: &str) -> Result<&'a StringArray> {
    batch
        .column_by_name(name)
        .and_then(|c| c.as_any().downcast_ref::<StringArray>())
        .ok_or_else(|| MfError::advanced_store(format!("table missing column '{name}'"), None))
}

fn int64_col<'a>(batch: &'a RecordBatch, name: &str) -> Result<&'a Int64Array> {
    batch
        .column_by_name(name)
        .and_then(|c| c.as_any().downcast_ref::<Int64Array>())
        .ok_or_else(|| MfError::advanced_store(format!("table missing column '{name}'"), None))
}

fn uint64_col<'a>(batch: &'a RecordBatch, name: &str) -> Result<&'a UInt64Array> {
    batch
        .column_by_name(name)
        .and_then(|c| c.as_any().downcast_ref::<UInt64Array>())
        .ok_or_else(|| MfError::advanced_store(format!("table missing column '{name}'"), None))
}

fn uint32_col<'a>(batch: &'a RecordBatch, name: &str) -> Result<&'a UInt32Array> {
    batch
        .column_by_name(name)
        .and_then(|c| c.as_any().downcast_ref::<UInt32Array>())
        .ok_or_else(|| MfError::advanced_store(format!("table missing column '{name}'"), None))
}

fn float32_col<'a>(batch: &'a RecordBatch, name: &str) -> Result<&'a Float32Array> {
    batch
        .column_by_name(name)
        .and_then(|c| c.as_any().downcast_ref::<Float32Array>())
        .ok_or_else(|| MfError::advanced_store(format!("table missing column '{name}'"), None))
}

fn nullable_str(col: &StringArray, row: usize) -> Option<String> {
    if col.is_null(row) { None } else { Some(col.value(row).to_string()) }
}

// ── State parsers ─────────────────────────────────────────────────────────────

fn parse_registration_state(s: &str) -> RegistrationState {
    match s {
        "live" => RegistrationState::Live,
        "pending" => RegistrationState::Pending,
        "failed" => RegistrationState::Failed,
        "orphaned" => RegistrationState::Orphaned,
        _ => RegistrationState::Pending,
    }
}

fn parse_doc_state(s: &str) -> DocumentState {
    match s {
        "ready" => DocumentState::Ready,
        "stale" => DocumentState::Stale,
        "failed" => DocumentState::Failed,
        "skipped" => DocumentState::Skipped,
        "unbound" => DocumentState::Unbound,
        _ => DocumentState::Failed,
    }
}

fn parse_rel_state(s: &str) -> RelationState {
    match s {
        "missing" => RelationState::Missing,
        "pending" => RelationState::Pending,
        "ready" => RelationState::Ready,
        "stale" => RelationState::Stale,
        "failed" => RelationState::Failed,
        "orphaned" => RelationState::Orphaned,
        "skipped" => RelationState::Skipped,
        _ => RelationState::Pending,
    }
}
