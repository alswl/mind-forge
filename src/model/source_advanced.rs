//! Advanced Source types for LanceDB-backed repository Sources.
//!
//! These types represent the LanceDB primary catalog and derived state
//! when the repository backend is `lance`. In `legacy` mode, project
//! `mind-index.yaml.sources` remains the authoritative store.
//!
//! Types in this module are scaffolding for Phase 3+ and will be used
//! when service and CLI layers are built on top.

#![allow(dead_code)]

use serde::{Deserialize, Serialize};

// ── State enums ────────────────────────────────────────────────────────────

/// Lifecycle state of a Source registration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RegistrationState {
    Live,
    Pending,
    Failed,
    Orphaned,
}

/// State of a shared content document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DocumentState {
    Ready,
    Stale,
    Failed,
    Skipped,
    Unbound,
}

/// State of a registration-to-content relation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RelationState {
    Missing,
    Pending,
    Ready,
    Stale,
    Failed,
    Orphaned,
    Skipped,
}

/// State of an enrichment record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EnrichmentState {
    Pending,
    Ready,
    Stale,
    Failed,
    Skipped,
}

/// Coverage of a Claude enrichment job over the source document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EnrichmentCoverage {
    Complete,
    Partial,
}

/// Global index health status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum IndexStatus {
    Inactive,
    Missing,
    Ready,
    Stale,
    Degraded,
    Corrupt,
    Incompatible,
}

// ── Source registration ────────────────────────────────────────────────────

/// One project Source registration in the Lance primary catalog.
/// Never deduplicated across projects.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceRegistration {
    pub registration_key: String,
    pub project_key: String,
    pub project_identity: String,
    pub project_path: String,
    pub source_identity: String,
    pub source_type: String, // file, pdf, web, rss
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_kind: Option<String>,
    pub registered_location: String,
    pub tags_json: String,
    pub fact_fingerprint: String,
    pub registration_revision: i64,
    pub state: RegistrationState,
}

// ── Legacy compatibility ───────────────────────────────────────────────────

/// Projection state for one project's compatibility YAML.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LegacySourceProjectionState {
    pub project_key: String,
    pub primary_snapshot_id: String,
    pub expected_fingerprint: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub observed_fingerprint: Option<String>,
    pub state: ProjectionStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProjectionStatus {
    Current,
    Missing,
    Drifted,
    Failed,
}

// ── Shared content ─────────────────────────────────────────────────────────

/// One verified shared content document, independent of project/article.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedContentDocument {
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fetched_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub synced_at: Option<String>,
    pub chunk_count: u64,
}

// ── Registration-content relation ──────────────────────────────────────────

/// Versioned relation between a registration and its last-good shared document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistrationContentRelation {
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

// ── Content chunk ──────────────────────────────────────────────────────────

/// One searchable content fragment with vector embedding.
/// Contains no project/article metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentChunk {
    pub chunk_id: String,
    pub document_key: String,
    pub content_revision: i64,
    pub ordinal: u32,
    pub locator_json: String,
    pub locator_sort_key: String,
    pub text: String,
    pub text_fingerprint: String,
    pub token_count: u32,
}

// ── Source enrichment ──────────────────────────────────────────────────────

/// Claude-produced, mf-validated semantic metadata for a shared document revision.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceEnrichment {
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
    pub coverage: EnrichmentCoverage,
    pub state: EnrichmentState,
    pub generated_at: String,
    pub applied_at: String,
}

// ── Enrichment job ─────────────────────────────────────────────────────────

/// A pending/stale enrichment job exposed to the Claude Skill.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnrichmentJob {
    pub document_key: String,
    pub content_revision: i64,
    pub content_fingerprint: String,
    pub state: EnrichmentState,
    pub total_chunks: u32,
    pub registrations: Vec<String>,
    pub prompt_version: String,
}

// ── Search ─────────────────────────────────────────────────────────────────

/// Content location within a document.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum SourceLocator {
    #[serde(rename = "text")]
    Text { start_line: u64, end_line: u64, start_byte: u64, end_byte: u64 },
    #[serde(rename = "pdf")]
    Pdf { page: u32, start_char: u64, end_char: u64 },
    #[serde(rename = "html")]
    Html { block: String, heading_path: Vec<String>, selector: Option<String> },
    #[serde(rename = "feed")]
    Feed { entry_id: Option<String>, entry_url: Option<String>, entry_ordinal: u32, start_char: u64, end_char: u64 },
    #[serde(rename = "source")]
    Source,
}

/// A single search result with full provenance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceSearchResult {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub document_key: Option<String>,
    pub source_type: String,
    pub location: String,
    pub locator: Option<SourceLocator>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chunk_id: Option<String>,
    pub snippet: String,
    pub registrations: Vec<SearchResultRegistration>,
    pub retrieval_paths: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keyword_score: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub semantic_score: Option<f32>,
    pub combined_score: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub freshness: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enrichment: Option<SearchResultEnrichment>,
    pub deduplicated: bool,
}

/// Registration summary in a search result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResultRegistration {
    pub registration_key: String,
    pub project_identity: String,
    pub project_path: String,
    pub source_identity: String,
    pub registered_location: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_kind: Option<String>,
    pub tags: Vec<String>,
}

/// Enrichment summary in a search result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResultEnrichment {
    pub state: EnrichmentState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub topics: Option<Vec<String>>,
    pub coverage: EnrichmentCoverage,
}

// ── Reports ────────────────────────────────────────────────────────────────

/// Aggregate search report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceSearchReport {
    pub query: String,
    pub requested_mode: String,
    pub resolved_mode: String,
    pub scope: SearchScope,
    pub actual_paths: Vec<String>,
    pub degraded: bool,
    pub results: Vec<SourceSearchResult>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchScope {
    pub kind: String, // "repository" | "project"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
}

/// Sync report for a single item.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncItem {
    pub project_identity: String,
    pub registration_key: String,
    pub source_identity: String,
    pub action: String, // added, updated, skipped, failed
    pub before_state: Option<RelationState>,
    pub after_state: RelationState,
    pub detected_format: Option<String>,
    pub affected_chunks: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Aggregate sync report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncReport {
    pub scope: String, // repository | project | registration
    pub dry_run: bool,
    pub registrations_total: u64,
    pub registrations_added: u64,
    pub registrations_updated: u64,
    pub registrations_skipped: u64,
    pub registrations_failed: u64,
    pub projects_processed: u64,
    pub projects_ready: u64,
    pub projects_failed: u64,
    pub items: Vec<SyncItem>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub index_revision: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub warnings: Vec<String>,
}

/// Aggregate advanced Source status report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdvancedSourceStatusReport {
    pub backend: String,
    pub index_status: IndexStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub activation_snapshot_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub activation_catalog_fingerprint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub primary_catalog_fingerprint: Option<String>,
    pub retained_snapshots: u32,
    pub pending_intents: u32,
    pub registrations_count: u64,
    pub documents_count: u64,
    pub relations_count: u64,
    pub chunks_count: u64,
    pub enrichments_ready: u64,
    pub enrichments_pending: u64,
    pub enrichments_failed: u64,
    pub projects: Vec<ProjectAdvancedStatus>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectAdvancedStatus {
    pub project_key: String,
    pub project_identity: String,
    pub registrations: u64,
    pub relations_ready: u64,
    pub relations_pending: u64,
    pub relations_failed: u64,
    pub projection_state: ProjectionStatus,
}

// ── Durable mutation intent ────────────────────────────────────────────────

/// Durable transaction record for cross-store Project lifecycle operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdvancedMutationIntent {
    pub transaction_id: String,
    pub operation: MutationOperation,
    pub phase: MutationPhase,
    pub baseline_snapshot_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub staged_snapshot_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub before_project_fingerprint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub after_project_fingerprint: Option<String>,
    pub affected_registration_keys: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MutationOperation {
    #[serde(rename = "project_new")]
    New,
    #[serde(rename = "project_import")]
    Import,
    #[serde(rename = "project_rename")]
    Rename,
    #[serde(rename = "project_archive")]
    Archive,
    #[serde(rename = "project_remove")]
    Remove,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MutationPhase {
    Prepared,
    FactsCommitted,
    PrimaryPublished,
    Projected,
    Completed,
    Failed,
}

// ── Model identity ─────────────────────────────────────────────────────────

/// Pinned model/runtime identity for content fingerprints.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelIdentity {
    pub model_id: String,
    pub revision: String,
    pub artifact_sha256: String,
    pub dimension: u32,
    pub runtime: String,
    pub runtime_artifact_sha256: String,
    pub pooling: String,
    pub normalization: String,
    pub query_prefix: String,
    pub passage_prefix: String,
}
