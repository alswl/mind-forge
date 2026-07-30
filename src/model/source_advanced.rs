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
    /// k8s-style identifying key/value labels (JSON object). Selectable for
    /// retrieval prefiltering (`--label k=v`); keep short and low-cardinality.
    #[serde(default = "empty_json_object")]
    pub labels_json: String,
    /// k8s-style non-identifying annotations (JSON object). Free-form metadata
    /// returned with results but never used for selection.
    #[serde(default = "empty_json_object")]
    pub annotations_json: String,
    pub fact_fingerprint: String,
    pub registration_revision: i64,
    pub state: RegistrationState,
    /// Serialized [`DocumentContext`] derived during sync (schema v2). `None`
    /// until the registration has been enriched.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_json: Option<String>,
    /// Serialized [`ImportProvenance`] for `source` bindings, captured at
    /// `source add`/`source new` (schema v2). `None` for non-source or legacy rows.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub imported_by_json: Option<String>,
}

fn empty_json_object() -> String {
    "{}".to_string()
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

// ── Document context (schema v2) ────────────────────────────────────────────

/// The kind of content a registration indexes. Determines whether structured
/// context participates in vector embedding (`single_owner`) or is returned as
/// per-binding provenance only (`source`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContentKind {
    Source,
    Article,
    ArticlePrompt,
    ArticleThinking,
    Project,
    Term,
}

impl ContentKind {
    /// Parse from the `source_type`/`source_kind` string used in registrations.
    pub fn from_registration_kind(kind: &str) -> Self {
        match kind {
            "article" => Self::Article,
            "article_prompt" => Self::ArticlePrompt,
            "article_thinking" => Self::ArticleThinking,
            "project" => Self::Project,
            "term" => Self::Term,
            _ => Self::Source,
        }
    }

    /// A single-owner kind is registered 1:1 with its content and may carry its
    /// context into the embedded chunk text. `source` content is shared across
    /// projects, so its context is provenance-only.
    pub fn is_single_owner(self) -> bool {
        !matches!(self, Self::Source)
    }
}

/// A directed relationship discovered from a document (internal links,
/// prompt/thinking siblings). `resolved` reflects current repository facts only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Relation {
    pub relation_type: RelationType,
    pub target: String,
    pub resolved: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[allow(clippy::enum_variant_names)]
pub enum RelationType {
    ArticleToArticle,
    ArticleToFile,
    ArticleToPrompt,
    ArticleToThinking,
    ArticleToTerm,
}

/// Import provenance for a `source` binding: which project (and, when captured
/// at creation, which originating article) introduced this source. Authoritative
/// fact persisted per binding; never inferred from article prose.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportProvenance {
    pub project: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub article: Option<String>,
}

/// Structured context derived per registration and persisted to
/// `registrations.context_json` (authoritative, schema v2). Drives the context
/// preamble for single-owner kinds and search provenance for all kinds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocumentContext {
    pub repository: String,
    pub project_identity: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_goal: Option<String>,
    pub content_kind: ContentKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lifecycle_status: Option<String>,
    pub relations: Vec<Relation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub imported_by: Option<ImportProvenance>,
    pub single_owner: bool,
}

impl DocumentContext {
    /// Render the deterministic, compact context preamble prepended to
    /// single-owner content before chunking (spec 071 data-model). Returns
    /// `None` for shared `source` content, whose vectors must stay context-free.
    pub fn preamble(&self) -> Option<String> {
        if !self.single_owner {
            return None;
        }
        let kind = serde_json::to_value(self.content_kind).ok().and_then(|v| v.as_str().map(str::to_string));
        let mut header = format!("[project: {}", self.project_identity);
        if let Some(goal) = &self.project_goal {
            header.push_str(&format!(" — {goal}"));
        }
        header.push(']');
        if let Some(kind) = kind {
            header.push_str(&format!(" [kind: {kind}]"));
        }
        if let Some(status) = &self.lifecycle_status {
            header.push_str(&format!(" [status: {status}]"));
        }
        if !self.relations.is_empty() {
            let targets = self.relations.iter().map(|r| r.target.as_str()).collect::<Vec<_>>().join(", ");
            header.push_str(&format!("\n[links: {targets}]"));
        }
        Some(header)
    }

    /// Normalize relations into deterministic order and drop duplicates so the
    /// persisted context and derived preamble are stable (SC-006).
    pub fn normalize(&mut self) {
        self.relations.sort_by(|a, b| a.relation_type.cmp(&b.relation_type).then_with(|| a.target.cmp(&b.target)));
        self.relations.dedup_by(|a, b| a.relation_type == b.relation_type && a.target == b.target);
        self.single_owner = self.content_kind.is_single_owner();
    }
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
    /// Registration type (e.g. `file`, `web`, `article`) so callers can tell
    /// what kind of Source a hit came from.
    pub source_type: String,
    pub registered_location: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_kind: Option<String>,
    pub tags: Vec<String>,
    /// Identifying labels carried from the registration (k8s-style).
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub labels: std::collections::BTreeMap<String, String>,
    /// Non-identifying annotations carried from the registration.
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub annotations: std::collections::BTreeMap<String, String>,
    /// Structured context for this binding (spec 071): repository/project
    /// attribution, content kind, lifecycle, relations, and — for source
    /// bindings — import provenance. Every hit carries one.
    pub context: DocumentContext,
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

/// Per-kind coverage counts in a sync report (spec 071). `indexed + skipped`
/// equals the number of discovered items of that kind.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoverageByKind {
    pub kind: String,
    pub indexed: u64,
    pub skipped: u64,
}

/// One discovered item that was not indexed, with a machine reason so nothing is
/// silently dropped (spec 071, FR-003). `reason` ∈ `empty` / `binary` /
/// `excluded` / `encoding_error` / `over_budget`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkippedItem {
    pub location: String,
    pub kind: String,
    pub reason: String,
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
    /// Per-kind coverage counts (spec 071). `indexed + skipped` equals the
    /// discovered total for each kind, so coverage is auditable.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub coverage: Vec<CoverageByKind>,
    /// Every discovered item that was not indexed, with a machine reason
    /// (spec 071) — no silent drops.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub skipped_items: Vec<SkippedItem>,
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
    /// Chunks with a non-zero embedding vector. `<= chunks_count`. Distinguishes
    /// "vectors present" from "text indexed" so `index_status: ready` cannot
    /// mask missing vectors after an offline/keyword-only sync (#27).
    pub chunks_embedded_count: u64,
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

#[cfg(test)]
mod context_tests {
    use super::*;

    fn rel(kind: RelationType, target: &str) -> Relation {
        Relation { relation_type: kind, target: target.to_string(), resolved: true }
    }

    #[test]
    fn normalize_sorts_and_dedups_relations() {
        let mut ctx = DocumentContext {
            repository: "r".into(),
            project_identity: "p".into(),
            project_goal: None,
            content_kind: ContentKind::Article,
            lifecycle_status: None,
            relations: vec![
                rel(RelationType::ArticleToFile, "b.png"),
                rel(RelationType::ArticleToArticle, "z.md"),
                rel(RelationType::ArticleToArticle, "a.md"),
                rel(RelationType::ArticleToArticle, "a.md"),
            ],
            imported_by: None,
            single_owner: false,
        };
        ctx.normalize();
        assert_eq!(ctx.relations.iter().map(|r| r.target.as_str()).collect::<Vec<_>>(), vec!["a.md", "z.md", "b.png"]);
    }

    #[test]
    fn normalize_derives_single_owner_from_kind() {
        let mut article = DocumentContext {
            repository: "r".into(),
            project_identity: "p".into(),
            project_goal: None,
            content_kind: ContentKind::Article,
            lifecycle_status: None,
            relations: vec![],
            imported_by: None,
            single_owner: false,
        };
        article.normalize();
        assert!(article.single_owner);

        let mut source = DocumentContext { content_kind: ContentKind::Source, ..article.clone() };
        source.single_owner = true;
        source.normalize();
        assert!(!source.single_owner);
    }

    #[test]
    fn content_kind_parses_registration_kinds() {
        assert_eq!(ContentKind::from_registration_kind("article"), ContentKind::Article);
        assert_eq!(ContentKind::from_registration_kind("article_prompt"), ContentKind::ArticlePrompt);
        assert_eq!(ContentKind::from_registration_kind("project"), ContentKind::Project);
        assert_eq!(ContentKind::from_registration_kind("term"), ContentKind::Term);
        assert_eq!(ContentKind::from_registration_kind("file"), ContentKind::Source);
        assert_eq!(ContentKind::from_registration_kind("web"), ContentKind::Source);
    }

    #[test]
    fn imported_by_omits_null_article() {
        let prov = ImportProvenance { project: "beta".into(), article: None };
        let json = serde_json::to_string(&prov).unwrap();
        assert_eq!(json, r#"{"project":"beta"}"#);
    }
}
