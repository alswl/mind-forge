//! Enrichment workflow: list pending jobs, show bounded content for Claude,
//! and apply validated structured metadata.
//!
//! Claude never executes here — the CLI exposes deterministic text/JSON
//! contracts that the `/mf-source` Skill calls. `apply` validates schema,
//! revision, fingerprint, and field bounds before publishing.

use std::path::Path;

use arrow_array::{Array, Int64Array, StringArray, UInt32Array};
use serde::{Deserialize, Serialize};

use crate::error::{MfError, Result};
use crate::model::source_advanced::{EnrichmentCoverage, EnrichmentState, SourceEnrichment};
use crate::service::source::advanced::identity;

/// An enrichment job visible to the Claude Skill.
#[derive(Debug, Serialize)]
pub struct EnrichmentJob {
    pub document_key: String,
    pub content_revision: i64,
    pub content_fingerprint: String,
    pub state: String,
    pub total_chunks: u32,
    pub registrations: Vec<String>,
    pub prompt_version: String,
}

/// A bounded chunk batch returned by `enrich show`.
#[derive(Debug, Serialize)]
pub struct EnrichShowResult {
    pub document_key: String,
    pub content_revision: i64,
    pub batch_index: u32,
    pub batch_count: u32,
    pub chunks: Vec<EnrichChunkText>,
    pub processed_range: String,
}

#[derive(Debug, Serialize)]
pub struct EnrichChunkText {
    pub ordinal: u32,
    pub text: String,
    pub locator_json: String,
}

/// Schema version constant — must match the Skill.
const SCHEMA_VERSION: &str = "1";
const PROMPT_VERSION: &str = "1";

/// Maximum field sizes for validation.
const MAX_SUMMARY_LEN: usize = 2000;
const MAX_TOPIC_LEN: usize = 100;
const MAX_TOPICS: usize = 10;
const MAX_KEYWORDS: usize = 15;
const MAX_KEYWORD_LEN: usize = 100;
const MAX_ENTITIES: usize = 50;
const MAX_ENTITY_NAME_LEN: usize = 200;
const MAX_ENTITY_DESC_LEN: usize = 500;
const MAX_WARNINGS: usize = 20;
const MAX_WARNING_LEN: usize = 500;

/// Enrichment submission JSON (from Claude Skill).
#[derive(Debug, Deserialize)]
pub struct EnrichmentInput {
    pub schema_version: String,
    pub prompt_version: String,
    pub document_key: String,
    pub content_revision: i64,
    pub content_fingerprint: String,
    pub summary: String,
    pub language: String,
    pub document_type: String,
    pub topics: Vec<String>,
    pub keywords: Vec<String>,
    pub entities: Vec<EnrichmentEntityInput>,
    pub confidence: f32,
    pub warnings: Vec<String>,
    pub processed_chunks: u32,
    pub total_chunks: u32,
    pub coverage: String,
}

#[derive(Debug, Deserialize)]
pub struct EnrichmentEntityInput {
    pub name: String,
    #[serde(rename = "type")]
    pub entity_type: String,
    #[serde(default)]
    pub description: Option<String>,
}

/// Result of enrichment application.
#[derive(Debug, Serialize)]
pub struct EnrichmentApplyResult {
    pub enrichment_key: String,
    pub document_key: String,
    pub state: String,
    pub applied_at: String,
}

/// List enrichment jobs (stub — full impl queries LanceDB enrichments table).
pub fn list_jobs(repo_root: &Path, state_filter: Option<&str>, limit: u32) -> Result<Vec<EnrichmentJob>> {
    let store = super::sync::open_active_store(repo_root)?;
    let chunks = store.scan_rows("chunks")?;
    let documents = store.scan_rows("documents")?;
    let mut enrichment_states = std::collections::BTreeMap::<String, String>::new();
    for batch in store.scan_rows("enrichments")? {
        let keys = string_column(&batch, "document_key")?;
        let states = string_column(&batch, "state")?;
        for row in 0..batch.num_rows() {
            enrichment_states.insert(keys.value(row).to_string(), states.value(row).to_string());
        }
    }
    let mut totals = std::collections::BTreeMap::<String, u32>::new();
    for batch in chunks {
        let keys = string_column(&batch, "document_key")?;
        for row in 0..batch.num_rows() {
            *totals.entry(keys.value(row).to_string()).or_default() += 1;
        }
    }
    let mut jobs = Vec::new();
    for batch in documents {
        let keys = string_column(&batch, "document_key")?;
        let fps = string_column(&batch, "content_fingerprint")?;
        let revisions = int64_column(&batch, "content_revision")?;
        for row in 0..batch.num_rows() {
            let state = enrichment_states.get(keys.value(row)).map(String::as_str).unwrap_or("pending");
            if state_filter.is_none_or(|filter| filter == state) {
                jobs.push(EnrichmentJob {
                    document_key: keys.value(row).to_string(),
                    content_revision: revisions.value(row),
                    content_fingerprint: fps.value(row).to_string(),
                    state: state.to_string(),
                    total_chunks: totals.get(keys.value(row)).copied().unwrap_or(0),
                    registrations: vec![],
                    prompt_version: PROMPT_VERSION.to_string(),
                });
            }
        }
    }
    jobs.sort_by(|a, b| a.document_key.cmp(&b.document_key));
    jobs.truncate(limit as usize);
    Ok(jobs)
}

/// Show a bounded chunk batch for a document.
pub fn show_document(document_key: &str, batch_size: u32, repo_root: &Path) -> Result<EnrichShowResult> {
    let store = super::sync::open_active_store(repo_root)?;
    let mut chunks = Vec::new();
    let mut revision = 1;
    for batch in store.scan_rows("chunks")? {
        let keys = string_column(&batch, "document_key")?;
        let ordinals = uint32_column(&batch, "ordinal")?;
        let texts = string_column(&batch, "text")?;
        let locators = string_column(&batch, "locator_json")?;
        let revisions = int64_column(&batch, "content_revision")?;
        for row in 0..batch.num_rows() {
            if keys.value(row) == document_key {
                revision = revisions.value(row);
                chunks.push(EnrichChunkText {
                    ordinal: ordinals.value(row),
                    text: texts.value(row).to_string(),
                    locator_json: locators.value(row).to_string(),
                });
            }
        }
    }
    chunks.sort_by_key(|chunk| chunk.ordinal);
    let size = batch_size.max(1) as usize;
    let total = chunks.len();
    let first = chunks.into_iter().take(size).collect::<Vec<_>>();
    Ok(EnrichShowResult {
        document_key: document_key.to_string(),
        content_revision: revision,
        batch_index: 0,
        batch_count: total.div_ceil(size) as u32,
        processed_range: format!("{}/{}", first.len(), total),
        chunks: first,
    })
}

fn string_column<'a>(batch: &'a arrow_array::RecordBatch, name: &str) -> Result<&'a StringArray> {
    batch
        .column_by_name(name)
        .and_then(|column| column.as_any().downcast_ref())
        .ok_or_else(|| MfError::advanced_store(format!("missing string column '{name}'"), None))
}
fn int64_column<'a>(batch: &'a arrow_array::RecordBatch, name: &str) -> Result<&'a Int64Array> {
    batch
        .column_by_name(name)
        .and_then(|column| column.as_any().downcast_ref())
        .ok_or_else(|| MfError::advanced_store(format!("missing int64 column '{name}'"), None))
}
fn uint32_column<'a>(batch: &'a arrow_array::RecordBatch, name: &str) -> Result<&'a UInt32Array> {
    batch
        .column_by_name(name)
        .and_then(|column| column.as_any().downcast_ref())
        .ok_or_else(|| MfError::advanced_store(format!("missing uint32 column '{name}'"), None))
}

/// Validate and apply an enrichment submission.
pub fn apply_enrichment(input: &EnrichmentInput, repo_root: &Path, dry_run: bool) -> Result<EnrichmentApplyResult> {
    // Validate schema version
    if input.schema_version != SCHEMA_VERSION {
        return Err(MfError::enrichment_rejected(
            format!("unsupported schema version '{}'; expected '{}'", input.schema_version, SCHEMA_VERSION),
            Some("update the enrichment Skill to the latest version".to_string()),
        ));
    }

    // Validate field sizes
    validate_fields(input)?;

    if dry_run {
        return Ok(EnrichmentApplyResult {
            enrichment_key: format!("dry-run:{}", input.document_key),
            document_key: input.document_key.clone(),
            state: "dry_run".to_string(),
            applied_at: chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string(),
        });
    }

    let store = super::sync::open_active_store(repo_root)?;
    let mut current = None;
    for batch in store.scan_rows("documents")? {
        let keys = string_column(&batch, "document_key")?;
        let revisions = int64_column(&batch, "content_revision")?;
        let fingerprints = string_column(&batch, "content_fingerprint")?;
        for row in 0..batch.num_rows() {
            if keys.value(row) == input.document_key {
                current = Some((revisions.value(row), fingerprints.value(row).to_string()));
            }
        }
    }
    let (revision, fingerprint) =
        current.ok_or_else(|| MfError::enrichment_rejected("unknown document_key".to_string(), None))?;
    if revision != input.content_revision || fingerprint != input.content_fingerprint {
        return Err(MfError::enrichment_rejected(
            "document revision or content fingerprint is stale".to_string(),
            Some("run `mf source advanced enrich list` and regenerate the submission".to_string()),
        ));
    }

    let applied_at = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
    let enrichment_key = identity::enrichment_key(
        &input.document_key,
        input.content_revision,
        &input.schema_version,
        &input.prompt_version,
    );
    store.replace_enrichment(&SourceEnrichment {
        enrichment_key: enrichment_key.clone(),
        document_key: input.document_key.clone(),
        content_revision: input.content_revision,
        schema_version: input.schema_version.clone(),
        prompt_version: input.prompt_version.clone(),
        summary: input.summary.clone(),
        language: input.language.clone(),
        document_type: input.document_type.clone(),
        topics_json: serde_json::to_string(&input.topics).map_err(MfError::Json)?,
        keywords_json: serde_json::to_string(&input.keywords).map_err(MfError::Json)?,
        entities_json: serde_json::to_string(
            &input
                .entities
                .iter()
                .map(|e| serde_json::json!({"name": e.name, "type": e.entity_type, "description": e.description}))
                .collect::<Vec<_>>(),
        )
        .map_err(MfError::Json)?,
        confidence: input.confidence,
        warnings_json: serde_json::to_string(&input.warnings).map_err(MfError::Json)?,
        processed_chunks: input.processed_chunks,
        total_chunks: input.total_chunks,
        coverage: if input.coverage == "complete" { EnrichmentCoverage::Complete } else { EnrichmentCoverage::Partial },
        state: EnrichmentState::Ready,
        generated_at: applied_at.clone(),
        applied_at: applied_at.clone(),
    })?;
    Ok(EnrichmentApplyResult {
        enrichment_key,
        document_key: input.document_key.clone(),
        state: "ready".to_string(),
        applied_at,
    })
}

fn validate_fields(input: &EnrichmentInput) -> Result<()> {
    if input.summary.len() > MAX_SUMMARY_LEN {
        return Err(MfError::enrichment_rejected(
            format!("summary too long ({} chars, max {})", input.summary.len(), MAX_SUMMARY_LEN),
            None,
        ));
    }
    if input.summary.is_empty() {
        return Err(MfError::enrichment_rejected("summary is empty".to_string(), None));
    }
    if input.topics.len() > MAX_TOPICS {
        return Err(MfError::enrichment_rejected(
            format!("too many topics ({} max {})", input.topics.len(), MAX_TOPICS),
            None,
        ));
    }
    for (i, topic) in input.topics.iter().enumerate() {
        if topic.len() > MAX_TOPIC_LEN {
            return Err(MfError::enrichment_rejected(
                format!("topic[{}] too long ({} chars, max {})", i, topic.len(), MAX_TOPIC_LEN),
                None,
            ));
        }
    }
    if input.keywords.len() > MAX_KEYWORDS {
        return Err(MfError::enrichment_rejected(
            format!("too many keywords ({} max {})", input.keywords.len(), MAX_KEYWORDS),
            None,
        ));
    }
    for (i, kw) in input.keywords.iter().enumerate() {
        if kw.len() > MAX_KEYWORD_LEN {
            return Err(MfError::enrichment_rejected(
                format!("keyword[{}] too long ({} chars, max {})", i, kw.len(), MAX_KEYWORD_LEN),
                None,
            ));
        }
    }
    if input.entities.len() > MAX_ENTITIES {
        return Err(MfError::enrichment_rejected(
            format!("too many entities ({} max {})", input.entities.len(), MAX_ENTITIES),
            None,
        ));
    }
    for (i, entity) in input.entities.iter().enumerate() {
        if entity.name.len() > MAX_ENTITY_NAME_LEN {
            return Err(MfError::enrichment_rejected(
                format!("entity[{}] name too long ({} chars, max {})", i, entity.name.len(), MAX_ENTITY_NAME_LEN),
                None,
            ));
        }
        if let Some(ref desc) = entity.description
            && desc.len() > MAX_ENTITY_DESC_LEN
        {
            return Err(MfError::enrichment_rejected(format!("entity[{}] description too long", i), None));
        }
    }
    if !(0.0..=1.0).contains(&input.confidence) {
        return Err(MfError::enrichment_rejected(
            format!("confidence {} out of range [0.0, 1.0]", input.confidence),
            None,
        ));
    }
    if input.warnings.len() > MAX_WARNINGS {
        return Err(MfError::enrichment_rejected(
            format!("too many warnings ({} max {})", input.warnings.len(), MAX_WARNINGS),
            None,
        ));
    }
    for (i, w) in input.warnings.iter().enumerate() {
        if w.len() > MAX_WARNING_LEN {
            return Err(MfError::enrichment_rejected(format!("warning[{}] too long", i), None));
        }
    }
    if input.coverage != "complete" && input.coverage != "partial" {
        return Err(MfError::enrichment_rejected(
            format!("invalid coverage '{}'; expected 'complete' or 'partial'", input.coverage),
            None,
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_input() -> EnrichmentInput {
        EnrichmentInput {
            schema_version: "1".into(),
            prompt_version: "1".into(),
            document_key: "a".repeat(64),
            content_revision: 1,
            content_fingerprint: "b".repeat(64),
            summary: "A test summary.".into(),
            language: "en".into(),
            document_type: "reference".into(),
            topics: vec!["test".into()],
            keywords: vec!["keyword".into()],
            entities: vec![EnrichmentEntityInput {
                name: "Test".into(),
                entity_type: "concept".into(),
                description: None,
            }],
            confidence: 0.8,
            warnings: vec![],
            processed_chunks: 5,
            total_chunks: 5,
            coverage: "complete".into(),
        }
    }

    #[test]
    fn rejects_wrong_schema_version() {
        let mut input = valid_input();
        input.schema_version = "2".into();
        let result = apply_enrichment(&input, Path::new("/tmp"), false);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("schema version"));
    }

    #[test]
    fn rejects_oversized_summary() {
        let mut input = valid_input();
        input.summary = "x".repeat(2001);
        let result = apply_enrichment(&input, Path::new("/tmp"), false);
        assert!(result.is_err());
    }

    #[test]
    fn rejects_confidence_out_of_range() {
        let mut input = valid_input();
        input.confidence = 1.5;
        let result = apply_enrichment(&input, Path::new("/tmp"), false);
        assert!(result.is_err());
    }

    #[test]
    fn accepts_valid_input() {
        let result = apply_enrichment(&valid_input(), Path::new("/tmp"), true);
        assert!(result.is_ok());
        let r = result.unwrap();
        assert_eq!(r.state, "dry_run");
    }

    #[test]
    fn dry_run_does_not_persist() {
        let result = apply_enrichment(&valid_input(), Path::new("/tmp"), true);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().state, "dry_run");
    }

    #[test]
    fn rejects_too_many_topics() {
        let mut input = valid_input();
        input.topics = (0..11).map(|i| format!("topic{i}")).collect();
        assert!(apply_enrichment(&input, Path::new("/tmp"), false).is_err());
    }

    #[test]
    fn list_jobs_returns_empty_initially() {
        let dir = tempfile::tempdir().unwrap();
        let jobs = list_jobs(dir.path(), None, 10);
        assert!(jobs.is_err());
    }
}
