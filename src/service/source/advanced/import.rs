//! Import (restore) a corpus bundle into the current repository.
//!
//! `import_bundle` validates a bundle directory produced by `export_bundle`,
//! restores source registrations and content files, rebuilds derived data
//! (chunks + embeddings) through the existing sync pipeline, and publishes
//! atomically.

use std::fs;
use std::path::{Component, Path};

use serde::Serialize;

use crate::error::{MfError, Result};
use crate::model::source_advanced::{RegistrationState, SourceRegistration};

use super::bundle::{self, BundleDocumentRecord, BundleRegistrationRecord, ContentFidelity};
use super::config::load_repository_config;
use super::publication;
use super::sync;

// ── Public API ────────────────────────────────────────────────────────────────

/// Result payload for `import_bundle`.
#[derive(Debug, Clone, Serialize)]
pub struct ImportReport {
    pub restored_counts: RestoredCounts,
    pub rebuilt_counts: Option<RebuiltCounts>,
    pub dry_run: bool,
    pub overwrite: bool,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct RestoredCounts {
    pub registrations: u64,
    pub documents: u64,
    pub byte_exact: u64,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct RebuiltCounts {
    pub registrations_added: u64,
    pub registrations_failed: u64,
}

/// Import a bundle into the current repository.
///
/// All validation (manifest, digest, schema compatibility, blob integrity)
/// happens before any write.  On failure the existing corpus is untouched.
pub fn import_bundle(repo_root: &Path, bundle: &Path, overwrite: bool, dry_run: bool) -> Result<ImportReport> {
    // ── 1. Validate bundle ──────────────────────────────────────────────
    let bundle_dir = bundle::validate_bundle_dir(bundle)?;
    let manifest_path = bundle_dir.join(bundle::MANIFEST_FILE);
    let manifest_json = fs::read_to_string(&manifest_path).map_err(MfError::Io)?;
    let manifest: bundle::BundleManifest = serde_json::from_str(&manifest_json).map_err(MfError::Json)?;

    if !bundle::compatible_bundle_version(&manifest.bundle_format_version) {
        return Err(MfError::advanced_store(
            format!(
                "incompatible bundle format version: {} (expected major version 1)",
                manifest.bundle_format_version
            ),
            None,
        ));
    }

    // Verify bundle integrity digest before any writes.
    let digest_files = bundle::digest_file_list(&bundle_dir);
    if !digest_files.is_empty() {
        let actual_digest = bundle::compute_bundle_digest(&digest_files)?;
        if actual_digest != manifest.content_digest {
            return Err(MfError::advanced_store(
                "bundle integrity check failed: content digest mismatch".to_string(),
                Some("the bundle may be corrupted or tampered".to_string()),
            ));
        }
    }

    // ── 2. Read bundle records ──────────────────────────────────────────
    let registrations: Vec<BundleRegistrationRecord> =
        bundle::read_jsonl(&bundle_dir.join(bundle::REGISTRATIONS_JSONL))?;
    let documents: Vec<BundleDocumentRecord> = bundle::read_jsonl(&bundle_dir.join(bundle::DOCUMENTS_JSONL))?;
    let rel_content: Vec<bundle::BundleRegistrationContentRecord> =
        bundle::read_jsonl(&bundle_dir.join(bundle::REGISTRATION_CONTENT_JSONL))?;

    validate_restore_paths(&registrations, &rel_content)?;

    // Verify every byte_exact document's blob exists and matches digest. Size is
    // not validated here (the digest is the integrity guarantee).
    let content_dir = bundle_dir.join(bundle::CONTENT_DIR);
    let byte_exact_docs: Vec<&BundleDocumentRecord> =
        documents.iter().filter(|d| d.fidelity == ContentFidelity::ByteExact).collect();
    for doc in &byte_exact_docs {
        bundle::read_blob(&content_dir, &doc.raw_fingerprint)?;
    }

    // ── 3. Config / backend check ──────────────────────────────────────
    let config = load_repository_config(repo_root)?;
    if !config.is_lance() {
        // Activate Lance backend first if not already active.
        if !dry_run {
            let legacy = super::config::ResolvedSourceConfig::from_config(
                crate::service::repo::load_manifest(&repo_root.join("minds.yaml"))?.source.as_ref(),
            )?;
            super::activation::activate(repo_root, &legacy)?;
        }
    } else if !overwrite {
        // Target already has a published corpus — explicit --overwrite required.
        let advanced_dir = super::advanced_store_dir(repo_root);
        if publication::read_pointer(&advanced_dir)?.is_some() {
            return Err(MfError::usage(
                "target repository already has a published corpus".to_string(),
                Some("use --overwrite to replace the existing corpus".to_string()),
            ));
        }
    }

    let restored = RestoredCounts {
        registrations: registrations.len() as u64,
        documents: documents.len() as u64,
        byte_exact: byte_exact_docs.len() as u64,
    };

    if dry_run {
        return Ok(ImportReport { restored_counts: restored, rebuilt_counts: None, dry_run: true, overwrite });
    }

    // ── 4. Overwrite: clear existing data ──────────────────────────────
    if overwrite {
        let clear_config = load_repository_config(repo_root)?;
        sync::clear_derived(repo_root, &clear_config, None, None, true, false)?;
    }

    // ── 5. Write source files back to disk ────────────────────────────
    // Build a registration_key → project_path lookup from registrations so we
    // can resolve project-relative paths correctly.
    let reg_project_paths: std::collections::HashMap<&str, &str> =
        registrations.iter().map(|r| (r.registration_key.as_str(), r.project_path.as_str())).collect();

    for doc in &byte_exact_docs {
        let blob_bytes = bundle::read_blob(&content_dir, &doc.raw_fingerprint)?;
        if let Some(binding) = rel_content.iter().find(|rc| rc.document_key.as_deref() == Some(&doc.document_key)) {
            let loc = &binding.acquired_location;
            if !loc.starts_with("http://") && !loc.starts_with("https://") {
                let project_path = reg_project_paths.get(binding.registration_key.as_str()).copied().unwrap_or("");
                let abs = repo_root.join(project_path).join(loc);
                if let Some(parent) = abs.parent() {
                    fs::create_dir_all(parent).map_err(|e| {
                        MfError::advanced_store(format!("cannot create directory for restored file: {e}"), None)
                    })?;
                }
                fs::write(&abs, &blob_bytes).map_err(|e| {
                    MfError::advanced_store(format!("cannot write restored file {}: {e}", abs.display()), None)
                })?;
            }
        }
    }

    // ── 6. Restore registrations to Lance store ────────────────────────
    let store = sync::open_active_store(repo_root)?;
    let source_registrations: Vec<SourceRegistration> = registrations
        .iter()
        .map(|r| SourceRegistration {
            registration_key: r.registration_key.clone(),
            project_key: r.project_key.clone(),
            project_identity: r.project_identity.clone(),
            project_path: r.project_path.clone(),
            source_identity: r.source_identity.clone(),
            source_type: r.source_type.clone(),
            source_kind: r.source_kind.clone(),
            registered_location: r.registered_location.clone(),
            tags_json: r.tags_json.clone(),
            labels_json: r.labels_json.clone(),
            annotations_json: r.annotations_json.clone(),
            fact_fingerprint: r.fact_fingerprint.clone(),
            registration_revision: r.registration_revision,
            state: match r.state {
                RegistrationState::Live => crate::model::source_advanced::RegistrationState::Live,
                RegistrationState::Pending => crate::model::source_advanced::RegistrationState::Pending,
                RegistrationState::Failed => crate::model::source_advanced::RegistrationState::Failed,
                RegistrationState::Orphaned => crate::model::source_advanced::RegistrationState::Orphaned,
            },
            // Regenerated by the sync pass below; provenance re-captured via source add.
            context_json: None,
            imported_by_json: None,
        })
        .collect();
    store.append_registrations(&source_registrations)?;

    // ── 7. Rebuild derived data (chunks + embeddings) ──────────────────
    let sync_config = load_repository_config(repo_root)?;
    let sync_report = sync::sync_repository(repo_root, &sync_config, None, None, false, false)?;
    let rebuilt = RebuiltCounts {
        registrations_added: sync_report.registrations_added,
        registrations_failed: sync_report.registrations_failed,
    };

    Ok(ImportReport { restored_counts: restored, rebuilt_counts: Some(rebuilt), dry_run: false, overwrite })
}

/// Reject bundle-controlled filesystem paths that could escape the destination
/// repository. URLs are not restored to disk and are validated separately.
fn validate_restore_paths(
    registrations: &[BundleRegistrationRecord],
    relations: &[bundle::BundleRegistrationContentRecord],
) -> Result<()> {
    let projects = registrations
        .iter()
        .map(|registration| {
            ensure_safe_relative_path(&registration.project_path, "project_path")?;
            if !acquisition_is_url(&registration.registered_location) {
                ensure_safe_relative_path(&registration.registered_location, "registered_location")?;
            }
            Ok((registration.registration_key.as_str(), registration.project_path.as_str()))
        })
        .collect::<Result<std::collections::HashMap<_, _>>>()?;

    for relation in relations {
        let Some(_project_path) = projects.get(relation.registration_key.as_str()) else {
            return Err(MfError::advanced_store(
                format!("bundle relation references unknown registration: {}", relation.registration_key),
                None,
            ));
        };
        if !acquisition_is_url(&relation.acquired_location) {
            ensure_safe_relative_path(&relation.acquired_location, "acquired_location")?;
        }
    }
    Ok(())
}

fn acquisition_is_url(location: &str) -> bool {
    location.starts_with("http://") || location.starts_with("https://")
}

fn ensure_safe_relative_path(value: &str, field: &str) -> Result<()> {
    let path = Path::new(value);
    if value.is_empty()
        || path.is_absolute()
        || path.components().any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(MfError::advanced_store(
            format!("bundle {field} must be a non-empty relative path without traversal: {value:?}"),
            None,
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::ensure_safe_relative_path;

    #[test]
    fn restore_paths_must_stay_relative() {
        assert!(ensure_safe_relative_path("projects/alpha/sources/note.md", "path").is_ok());
        assert!(ensure_safe_relative_path("../outside", "path").is_err());
        assert!(ensure_safe_relative_path("/tmp/outside", "path").is_err());
        assert!(ensure_safe_relative_path("", "path").is_err());
    }
}
