//! Export/import bundle model, serde, integrity, and blob capture.
//!
//! An export bundle is a self-contained directory (`<name>.mfbundle/`) holding a
//! `manifest.json`, newline-delimited JSON record files for each Lance table's
//! authoritative rows, and a content-addressed `content/` blob tree.  No
//! compression / tar / zip dependency — the bundle is human-inspectable.

use std::collections::BTreeMap;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::{MfError, Result};
use crate::model::source_advanced::{DocumentState, RegistrationState, RelationState};

// ── Format constants ─────────────────────────────────────────────────────────

/// Current export bundle format version (semver-like).  Import rejects unknown
/// major versions.
pub const BUNDLE_FORMAT_VERSION: &str = "1.0.0";

/// File name of the bundle manifest.
pub const MANIFEST_FILE: &str = "manifest.json";

/// JSONL record file names (one per Lance table).
pub const REGISTRATIONS_JSONL: &str = "registrations.jsonl";
pub const DOCUMENTS_JSONL: &str = "documents.jsonl";
pub const REGISTRATION_CONTENT_JSONL: &str = "registration_content.jsonl";
pub const ENRICHMENTS_JSONL: &str = "enrichments.jsonl";

/// Config export file.
pub const CONFIG_FILE: &str = "config.json";

/// Content-addressed blob directory.
pub const CONTENT_DIR: &str = "content";

// ── Manifest ──────────────────────────────────────────────────────────────────

/// Bundle header: format version, timestamps, model identity, counts, and
/// integrity digest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleManifest {
    /// Export representation version; import fails closed on unknown major.
    pub bundle_format_version: String,
    /// Export time (RFC 3339).
    pub created_at: String,
    /// The published snapshot ID the export was taken from.
    pub source_snapshot_id: String,
    /// Advanced schema version at export time.
    pub storage_schema_version: String,
    /// Exported ModelIdentity (may be absent if activation-only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_identity: Option<ModelIdentityRecord>,
    /// Object counts: registrations, documents, registration_content, enrichments, blobs.
    pub counts: ObjectCounts,
    /// Per-fidelity document counts.
    pub fidelity_summary: FidelitySummary,
    /// Digest over the ordered record files + blob digests, for integrity /
    /// tamper detection.
    pub content_digest: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelIdentityRecord {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub embedding_model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub embedding_dimension: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub embedding_provider_endpoint: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ObjectCounts {
    pub registrations: u64,
    pub documents: u64,
    pub registration_content: u64,
    pub enrichments: u64,
    pub blobs: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FidelitySummary {
    pub byte_exact: u64,
    pub extracted_only: u64,
}

// ── Bundle records (one per Lance table row) ──────────────────────────────────

/// Serialised form of a `registrations` Lance row.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleRegistrationRecord {
    pub registration_key: String,
    pub project_key: String,
    pub project_identity: String,
    pub project_path: String,
    pub source_identity: String,
    pub source_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_kind: Option<String>,
    pub registered_location: String,
    pub tags_json: String,
    pub labels_json: String,
    pub annotations_json: String,
    pub fact_fingerprint: String,
    pub registration_revision: i64,
    pub state: RegistrationState,
}

/// Serialised form of a `documents` Lance row plus export-only fields.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleDocumentRecord {
    pub document_key: String,
    pub acquisition_kind: String,
    pub raw_fingerprint: String,
    pub extracted_fingerprint: String,
    pub content_fingerprint: String,
    pub content_revision: i64,
    pub state: DocumentState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_error_kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
    pub chunk_count: u64,
    /// Export-time fidelity level.
    pub fidelity: ContentFidelity,
    /// Reference to extracted text (used for `extracted_only` docs or rebuild
    /// verification).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extracted_text_ref: Option<String>,
}

/// Whether the original bytes were captured at export time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContentFidelity {
    /// Original bytes captured and verified against `raw_fingerprint`; import
    /// restores byte-identical content.
    ByteExact,
    /// Original bytes were unavailable at export time; only extracted text is
    /// preserved (rare fallback).
    ExtractedOnly,
}

/// Serialised form of a `registration_content` Lance row.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleRegistrationContentRecord {
    pub registration_key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub document_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_revision: Option<i64>,
    pub acquisition_key: String,
    pub acquired_location: String,
    pub registered_revision: String,
    pub state: RelationState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_error_kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attempted_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub synced_at: Option<String>,
}

/// Serialised form of an `enrichments` Lance row.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundleEnrichmentRecord {
    pub enrichment_key: String,
    pub document_key: String,
    pub content_revision: i64,
    pub schema_version: String,
    pub prompt_version: String,
    pub summary: String,
    pub language: String,
    pub document_type: String,
    pub topics_json: String,
    pub keywords_json: String,
    pub entities_json: String,
    pub confidence: f32,
    pub warnings_json: String,
    pub processed_chunks: u32,
    pub total_chunks: u32,
    pub coverage: String,
    pub state: String,
    pub generated_at: String,
}

// ── Bundle directory layout helpers ───────────────────────────────────────────

/// Validate that `path` is a readable bundle directory (contains `manifest.json`).
pub fn validate_bundle_dir(path: &Path) -> Result<PathBuf> {
    let canonical = path
        .canonicalize()
        .map_err(|_| MfError::usage(format!("bundle path does not exist: {}", path.display()), None))?;
    if !canonical.is_dir() {
        return Err(MfError::usage(format!("bundle path is not a directory: {}", canonical.display()), None));
    }
    let manifest = canonical.join(MANIFEST_FILE);
    if !manifest.is_file() {
        return Err(MfError::usage(
            format!("not a valid bundle (missing manifest.json): {}", canonical.display()),
            None,
        ));
    }
    Ok(canonical)
}

// ── Digest helpers ────────────────────────────────────────────────────────────

/// Compute a SHA-256 hex digest of `bytes`.
pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

/// Compute the bundle-level integrity digest.
///
/// Walks the file list in order, hashing each file's SHA-256, then returns
/// the hex-encoded SHA-256 of the concatenated per-file digests.
pub fn compute_bundle_digest(files: &[PathBuf]) -> Result<String> {
    let mut compound = Sha256::new();
    for file in files {
        let bytes = fs::read(file)
            .map_err(|e| MfError::advanced_store(format!("cannot read bundle file {}: {e}", file.display()), None))?;
        let file_hash = sha256_hex(&bytes);
        compound.update(file_hash.as_bytes());
    }
    Ok(format!("{:x}", compound.finalize()))
}

/// Verify every blob referenced in blob_hashes exists under the content dir and
/// matches its digest.
pub fn verify_blob_integrity(content_dir: &Path, blob_hashes: &BTreeMap<String, u64>) -> Result<()> {
    for (expected_hash, expected_size) in blob_hashes {
        let blob_path = content_dir.join(expected_hash);
        if !blob_path.is_file() {
            return Err(MfError::advanced_store(
                format!("bundle blob missing: {expected_hash}"),
                Some("the bundle is incomplete or corrupted".to_string()),
            ));
        }
        let actual =
            fs::read(&blob_path).map_err(|e| MfError::advanced_store(format!("cannot read blob: {e}"), None))?;
        if actual.len() as u64 != *expected_size {
            return Err(MfError::advanced_store(
                format!("blob size mismatch: {expected_hash} (expected {expected_size}, got {})", actual.len()),
                Some("the bundle may be corrupted".to_string()),
            ));
        }
        let actual_hash = sha256_hex(&actual);
        if actual_hash != *expected_hash {
            return Err(MfError::advanced_store(
                format!("blob content digest mismatch: {expected_hash}"),
                Some("the bundle may be corrupted or tampered".to_string()),
            ));
        }
    }
    Ok(())
}

/// Verify that `bundle_format_version` shares the same major version.
pub fn compatible_bundle_version(version: &str) -> bool {
    version.split('.').next().map(|major| major == "1").unwrap_or(false)
}

// ── JSONL helpers ─────────────────────────────────────────────────────────────

/// Write an iterator of `Serialize`-able records to a newline-delimited JSON
/// file (one JSON object per line).
pub fn write_jsonl<T: Serialize>(path: &Path, records: &[T]) -> Result<()> {
    let mut file =
        fs::File::create(path).map_err(|e| MfError::advanced_store(format!("cannot create {path:?}: {e}"), None))?;
    for record in records {
        let line = serde_json::to_string(record).map_err(MfError::Json)?;
        writeln!(file, "{line}")
            .map_err(|e| MfError::advanced_store(format!("cannot write to {path:?}: {e}"), None))?;
    }
    Ok(())
}

/// Read newline-delimited JSON records from a file.
pub fn read_jsonl<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<Vec<T>> {
    let file = fs::File::open(path).map_err(|e| MfError::advanced_store(format!("cannot open {path:?}: {e}"), None))?;
    let reader = BufReader::new(file);
    let mut records = Vec::new();
    for (i, line) in reader.lines().enumerate() {
        let line = line.map_err(|e| MfError::advanced_store(format!("cannot read {path:?}:{i}: {e}"), None))?;
        if line.trim().is_empty() {
            continue;
        }
        let record: T = serde_json::from_str(&line)
            .map_err(|e| MfError::advanced_store(format!("invalid JSON at {path:?}:{i}: {e}"), None))?;
        records.push(record);
    }
    Ok(records)
}

/// Return the expected file list (in deterministic order) for the bundle
/// digest computation.  `manifest.json` is **excluded** — it contains the
/// digest itself and must not be part of its own input.
pub fn digest_file_list(bundle_dir: &Path) -> Vec<PathBuf> {
    [CONFIG_FILE, REGISTRATIONS_JSONL, DOCUMENTS_JSONL, REGISTRATION_CONTENT_JSONL, ENRICHMENTS_JSONL]
        .iter()
        .map(|name| bundle_dir.join(name))
        .filter(|p| p.is_file())
        .collect()
}

// ── Blob capture ─────────────────────────────────────────────────────────────

/// Write a content blob into the bundle's `content/` directory, keyed by its
/// SHA-256 digest.  Returns the hex digest.  Idempotent: if the blob already
/// exists with matching content it is not rewritten.
pub fn capture_blob(content_dir: &Path, bytes: &[u8]) -> Result<String> {
    let digest = sha256_hex(bytes);
    let blob_path = content_dir.join(&digest);
    if blob_path.is_file() {
        let existing =
            fs::read(&blob_path).map_err(|e| MfError::advanced_store(format!("cannot read blob: {e}"), None))?;
        if existing == bytes {
            return Ok(digest);
        }
        return Err(MfError::advanced_store(
            format!("blob content hash collision: {digest}"),
            Some("two different contents produced the same SHA-256 hash".to_string()),
        ));
    }
    fs::write(&blob_path, bytes)
        .map_err(|e| MfError::advanced_store(format!("cannot write blob {digest}: {e}"), None))?;
    Ok(digest)
}

/// Read a blob from the bundle's `content/` directory and verify its digest.
pub fn read_blob(content_dir: &Path, digest: &str) -> Result<Vec<u8>> {
    let blob_path = content_dir.join(digest);
    let bytes = fs::read(&blob_path).map_err(|_| MfError::advanced_store(format!("blob not found: {digest}"), None))?;
    let actual = sha256_hex(&bytes);
    if actual != digest {
        return Err(MfError::advanced_store(
            format!("blob content digest mismatch: expected {digest}, got {actual}"),
            Some("the bundle may be corrupted or tampered".to_string()),
        ));
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_is_stable() {
        assert_eq!(sha256_hex(b"hello world"), "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9");
    }

    #[test]
    fn compatible_bundle_version_accepts_same_major() {
        assert!(compatible_bundle_version("1.0.0"));
        assert!(compatible_bundle_version("1.2.3"));
        assert!(!compatible_bundle_version("2.0.0"));
        assert!(!compatible_bundle_version("0.9.0"));
    }

    #[test]
    fn jsonl_roundtrip_preserves_records() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.jsonl");
        let records = vec![BundleRegistrationRecord {
            registration_key: "k".into(),
            project_key: "pk".into(),
            project_identity: "alpha".into(),
            project_path: "projects/alpha".into(),
            source_identity: "src".into(),
            source_type: "web".into(),
            source_kind: None,
            registered_location: "sources/web/src.html".into(),
            tags_json: "[]".into(),
            labels_json: "{}".into(),
            annotations_json: "{}".into(),
            fact_fingerprint: "ff".into(),
            registration_revision: 1,
            state: RegistrationState::Live,
        }];
        write_jsonl(&path, &records).unwrap();
        let back: Vec<BundleRegistrationRecord> = read_jsonl(&path).unwrap();
        assert_eq!(back.len(), 1);
        assert_eq!(back[0].registration_key, "k");
    }

    #[test]
    fn blob_roundtrip_is_byte_identical() {
        let dir = tempfile::tempdir().unwrap();
        let content_dir = dir.path().join(CONTENT_DIR);
        fs::create_dir_all(&content_dir).unwrap();
        let original = b"blob payload for export";
        let digest = capture_blob(&content_dir, original).unwrap();
        let result = read_blob(&content_dir, &digest).unwrap();
        assert_eq!(result, original);
    }

    #[test]
    fn blob_capture_is_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let content_dir = dir.path().join(CONTENT_DIR);
        fs::create_dir_all(&content_dir).unwrap();
        let digest1 = capture_blob(&content_dir, b"same bytes").unwrap();
        let digest2 = capture_blob(&content_dir, b"same bytes").unwrap();
        assert_eq!(digest1, digest2);
    }

    #[test]
    fn blob_verify_detects_corruption() {
        let dir = tempfile::tempdir().unwrap();
        let content_dir = dir.path().join(CONTENT_DIR);
        fs::create_dir_all(&content_dir).unwrap();
        let digest = capture_blob(&content_dir, b"original").unwrap();
        // Corrupt the blob file on disk.
        fs::write(content_dir.join(&digest), b"tampered").unwrap();
        let err = read_blob(&content_dir, &digest).unwrap_err();
        assert!(err.to_string().contains("mismatch"));
    }

    #[test]
    fn verify_blob_integrity_rejects_missing_blob() {
        let dir = tempfile::tempdir().unwrap();
        let content_dir = dir.path().join(CONTENT_DIR);
        fs::create_dir_all(&content_dir).unwrap();
        capture_blob(&content_dir, b"present").unwrap();
        let mut hashes = BTreeMap::new();
        hashes.insert(sha256_hex(b"present"), 7);
        hashes.insert(sha256_hex(b"missing"), 0);
        let err = verify_blob_integrity(&content_dir, &hashes).unwrap_err();
        assert!(err.to_string().contains("missing"));
    }
}
