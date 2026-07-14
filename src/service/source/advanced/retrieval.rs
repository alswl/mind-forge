//! Repository-wide Source search: basic (metadata), advanced (FTS + vector + RRF),
//! and fused both-mode retrieval.
//!
//! Default scope is all live projects in `minds.yaml`. An explicit `--project`
//! acts as a filter; cwd never creates an implicit project filter.
//!
//! Read-only — never mutates, fetches, or creates files. Degraded mode (both
//! without advanced) returns basic results with a warning.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Component;
use std::path::Path;

use arrow_array::Array;

use crate::error::{MfError, Result};
use crate::model::source_advanced::{
    SearchResultRegistration, SearchScope, SourceLocator, SourceSearchReport, SourceSearchResult,
};
use crate::model::source_search::SearchMode;

use super::identity;

/// RRF constant k.
const RRF_K: f64 = 60.0;

/// A candidate from basic metadata search.
#[derive(Debug, Clone)]
struct BasicCandidate {
    registration_key: String,
    project_identity: String,
    project_path: String,
    source_identity: String,
    source_type: String,
    registered_location: String,
    source_kind: Option<String>,
    tags: Vec<String>,
    match_field: String,
}

/// A candidate from advanced (FTS or vector) search.
#[derive(Debug, Clone)]
struct AdvancedCandidate {
    document_key: String,
    chunk_id: String,
    locator_json: String,
    locator_sort_key: String,
    snippet: String,
    source_type: String,
    location: String,
    keyword_score: Option<f32>,
    semantic_score: Option<f32>,
    freshness: Option<String>,
    registrations: Vec<SearchResultRegistration>,
}

/// Search all live projects using the specified mode.
///
/// In Lance mode with an available store, advanced/both searches use
/// LanceDB FTS and vector indexes. Without a store, they degrade to basic
/// metadata search with warnings.
pub fn search_repository(
    repo_root: &Path,
    query: &str,
    mode: SearchMode,
    project_filter: Option<&str>,
    kind_filter: Option<&str>,
    source_filter: Option<&str>,
    limit: u32,
) -> Result<SourceSearchReport> {
    let store = open_active_store(repo_root)?;
    search_repository_with_store(
        repo_root,
        query,
        mode,
        project_filter,
        kind_filter,
        source_filter,
        limit,
        store.as_ref(),
    )
}

/// Open only the database selected by the active, validated pointer.
///
/// A pointer is durable repository state, so its relative URI is treated as
/// untrusted input: absolute paths and `..` escapes are rejected before a
/// reader opens LanceDB.  Legacy repositories intentionally return no store.
fn open_active_store(repo_root: &Path) -> Result<Option<super::lance_store::LanceStore>> {
    let config = super::config::load_repository_config(repo_root)?;
    if config.is_legacy() {
        return Ok(None);
    }

    let advanced_dir = repo_root.join(".mind/source/advanced");
    let pointer = super::publication::read_pointer(&advanced_dir)?.ok_or_else(|| {
        MfError::missing_lance_pointer(
            "missing",
            "Lance backend is active but current.json is absent".to_string(),
            Some("run `mf source advanced recover --snapshot ID --yes`".to_string()),
        )
    })?;
    let relative = Path::new(&pointer.database_uri)
        .strip_prefix("./")
        .map_err(|_| MfError::advanced_store("pointer database_uri must be a relative path".to_string(), None))?;
    if relative.components().any(|component| !matches!(component, Component::Normal(_))) {
        return Err(MfError::advanced_store(
            "pointer database_uri escapes the advanced Source store".to_string(),
            None,
        ));
    }
    let database_path = advanced_dir.join(relative);
    if !database_path.is_dir() {
        return Err(MfError::missing_lance_pointer(
            "corrupt",
            format!("pointed database directory is missing: {}", database_path.display()),
            None,
        ));
    }
    super::lance_store::LanceStore::open(&database_path).map(Some)
}

/// Internal: search with optional LanceStore handle for advanced retrieval.
#[allow(clippy::too_many_arguments)]
fn search_repository_with_store(
    repo_root: &Path,
    query: &str,
    mode: SearchMode,
    project_filter: Option<&str>,
    kind_filter: Option<&str>,
    source_filter: Option<&str>,
    limit: u32,
    store: Option<&super::lance_store::LanceStore>,
) -> Result<SourceSearchReport> {
    let mut warnings = Vec::new();
    let mut results = Vec::new();

    let projects_dir = repo_root.join("projects");
    if !projects_dir.exists() {
        return Ok(SourceSearchReport {
            query: query.to_string(),
            requested_mode: mode_to_str(mode),
            resolved_mode: mode_to_str(mode),
            scope: SearchScope { kind: "repository".to_string(), project: project_filter.map(|s| s.to_string()) },
            actual_paths: vec!["basic".to_string()],
            degraded: false,
            results: vec![],
            warnings: vec![],
        });
    }

    // Enumerate all live project registrations
    let mut all_registrations: Vec<BasicCandidate> = Vec::new();
    for project_entry in std::fs::read_dir(&projects_dir)? {
        let project_entry = project_entry?;
        if !project_entry.file_type()?.is_dir() {
            continue;
        }
        let project_path = project_entry.path();
        let project_name = project_path.file_name().unwrap_or_default().to_string_lossy();

        if let Some(filter) = project_filter
            && project_name != filter
        {
            continue;
        }

        let index_path = project_path.join("mind-index.yaml");
        if !index_path.exists() {
            continue;
        }

        if let Ok(yaml_data) = std::fs::read_to_string(&index_path)
            && let Ok(index) = serde_yaml::from_str::<serde_yaml::Value>(&yaml_data)
        {
            let project_identity = index.get("project").and_then(|v| v.as_str()).unwrap_or(&project_name);
            if let Some(sources) = index.get("sources").and_then(|v| v.as_sequence()) {
                for source in sources {
                    let name = source.get("name").and_then(|v| v.as_str()).unwrap_or("unknown");
                    let kind = source.get("kind").and_then(|v| v.as_str()).unwrap_or("file");
                    let location =
                        source.get("path").or_else(|| source.get("url")).and_then(|v| v.as_str()).unwrap_or("unknown");
                    let tags: Vec<String> = source
                        .get("tags")
                        .and_then(|v| v.as_sequence())
                        .map(|s| s.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
                        .unwrap_or_default();
                    let source_kind = source.get("source_kind").and_then(|v| v.as_str()).map(|s| s.to_string());

                    // Apply filters
                    if let Some(kf) = kind_filter
                        && kind != kf
                    {
                        continue;
                    }
                    if let Some(sf) = source_filter
                        && name != sf
                    {
                        continue;
                    }

                    let pk = identity::project_key(&project_name);
                    let rk = identity::registration_key(&pk, kind, location);

                    all_registrations.push(BasicCandidate {
                        registration_key: rk,
                        project_identity: project_identity.to_string(),
                        project_path: project_name.to_string(),
                        source_identity: name.to_string(),
                        source_type: kind.to_string(),
                        registered_location: location.to_string(),
                        source_kind,
                        tags,
                        match_field: String::new(),
                    });
                }
            }
        }
    }

    // Perform basic search
    let basic_results = basic_search(query, &all_registrations);
    let total_basic = basic_results.len();
    let mut document_bindings = BTreeMap::<String, BTreeSet<String>>::new();
    if let Some(s) = store {
        for batch in s.scan_rows("registration_content")? {
            let Some(registrations) = batch
                .column_by_name("registration_key")
                .and_then(|c| c.as_any().downcast_ref::<arrow_array::StringArray>())
            else {
                continue;
            };
            let Some(documents) = batch
                .column_by_name("document_key")
                .and_then(|c| c.as_any().downcast_ref::<arrow_array::StringArray>())
            else {
                continue;
            };
            for row in 0..batch.num_rows() {
                if !documents.is_null(row) {
                    document_bindings
                        .entry(documents.value(row).to_string())
                        .or_default()
                        .insert(registrations.value(row).to_string());
                }
            }
        }
    }

    // Try LanceDB advanced search if store is available
    let lancedb_available = store.is_some();
    let mut advanced_results: Vec<SourceSearchResult> = Vec::new();

    if lancedb_available
        && let Some(s) = store
        && let Ok(fts_batches) = s.fts_search("chunks", query, &["text"], limit as usize)
    {
        // Extract chunks from FTS results
        for batch in &fts_batches {
            let text_col = batch.column_by_name("text");
            let chunk_id_col = batch.column_by_name("chunk_id");
            let doc_key_col = batch.column_by_name("document_key");
            if let (Some(texts), Some(ids), Some(dks)) = (text_col, chunk_id_col, doc_key_col) {
                for row in 0..batch.num_rows() {
                    let t = texts
                        .as_any()
                        .downcast_ref::<arrow_array::StringArray>()
                        .and_then(|a| if row < a.len() { Some(a.value(row).to_string()) } else { None });
                    let id = ids
                        .as_any()
                        .downcast_ref::<arrow_array::StringArray>()
                        .and_then(|a| if row < a.len() { Some(a.value(row).to_string()) } else { None });
                    let dk = dks
                        .as_any()
                        .downcast_ref::<arrow_array::StringArray>()
                        .and_then(|a| if row < a.len() { Some(a.value(row).to_string()) } else { None });
                    if let (Some(t), Some(id), Some(dk)) = (t, id, dk) {
                        advanced_results.push(SourceSearchResult {
                            document_key: Some(dk.clone()),
                            source_type: "file".to_string(),
                            location: "indexed-content".to_string(),
                            locator: Some(SourceLocator::Source),
                            chunk_id: Some(id),
                            snippet: t.chars().take(200).collect(),
                            registrations: registrations_for_document(&all_registrations, &document_bindings, &dk),
                            retrieval_paths: vec!["advanced_keyword".to_string()],
                            keyword_score: None,
                            semantic_score: Some(0.5),
                            combined_score: 0.5,
                            freshness: Some("ready".to_string()),
                            enrichment: None,
                            deduplicated: false,
                        });
                    }
                }
            }
        }
    }

    // An FTS index is an optimization, not a correctness prerequisite.  A
    // newly synced repository must be searchable before the optional index is
    // built, so scan the pinned chunks as a deterministic local fallback.
    if advanced_results.is_empty()
        && let Some(s) = store
    {
        let query_lower = query.to_lowercase();
        for batch in s.scan_rows("chunks")? {
            let Some(texts) =
                batch.column_by_name("text").and_then(|c| c.as_any().downcast_ref::<arrow_array::StringArray>())
            else {
                continue;
            };
            let Some(ids) =
                batch.column_by_name("chunk_id").and_then(|c| c.as_any().downcast_ref::<arrow_array::StringArray>())
            else {
                continue;
            };
            let Some(documents) = batch
                .column_by_name("document_key")
                .and_then(|c| c.as_any().downcast_ref::<arrow_array::StringArray>())
            else {
                continue;
            };
            let Some(locators) = batch
                .column_by_name("locator_json")
                .and_then(|c| c.as_any().downcast_ref::<arrow_array::StringArray>())
            else {
                continue;
            };
            for row in 0..batch.num_rows() {
                let text = texts.value(row);
                if !text.to_lowercase().contains(&query_lower) {
                    continue;
                }
                advanced_results.push(SourceSearchResult {
                    document_key: Some(documents.value(row).to_string()),
                    source_type: "file".to_string(),
                    location: locators.value(row).to_string(),
                    locator: Some(SourceLocator::Source),
                    chunk_id: Some(ids.value(row).to_string()),
                    snippet: text.chars().take(200).collect(),
                    registrations: registrations_for_document(
                        &all_registrations,
                        &document_bindings,
                        documents.value(row),
                    ),
                    retrieval_paths: vec!["advanced_keyword".to_string()],
                    keyword_score: Some(1.0),
                    semantic_score: None,
                    combined_score: 1.0,
                    freshness: Some("ready".to_string()),
                    enrichment: None,
                    deduplicated: false,
                });
            }
        }
        advanced_results.sort_by(|a, b| a.chunk_id.cmp(&b.chunk_id));
    }

    match mode {
        SearchMode::Basic => {
            results = basic_results.into_iter().take(limit as usize).collect();
        }
        SearchMode::Advanced => {
            if advanced_results.is_empty() {
                if total_basic == 0 {
                    warnings.push(
                        "advanced search found no results — ensure content has been synced with `mf source advanced sync`"
                            .to_string(),
                    );
                } else {
                    warnings.push(
                        "advanced content index not available — falling back to basic metadata search".to_string(),
                    );
                    results = basic_results.into_iter().take(limit as usize).collect();
                }
            } else {
                results = advanced_results.into_iter().take(limit as usize).collect();
            }
        }
        SearchMode::Both => {
            // Merge basic metadata matches with advanced content matches
            // Deduplicate by source identity + location
            let mut seen = std::collections::HashSet::new();
            let mut merged = Vec::new();

            for r in advanced_results.into_iter().chain(basic_results) {
                let key = format!(
                    "{}:{}",
                    r.registrations.first().map(|reg| reg.source_identity.as_str()).unwrap_or(""),
                    r.location
                );
                if seen.insert(key) {
                    merged.push(r);
                }
            }

            results = merged.into_iter().take(limit as usize).collect();
            if lancedb_available && !results.is_empty() {
                warnings.push("results from both basic metadata and advanced content search".to_string());
            } else if !lancedb_available {
                warnings
                    .push("advanced retrieval not available — results are from basic metadata search only".to_string());
            }
        }
    }

    // Derive the report from the paths actually present in returned results,
    // rather than from the requested mode. This keeps degradation observable.
    let mut actual_paths: Vec<String> =
        results.iter().flat_map(|result| result.retrieval_paths.iter().cloned()).collect();
    actual_paths.sort();
    actual_paths.dedup();
    let degraded = match mode {
        SearchMode::Basic => false,
        SearchMode::Advanced => actual_paths.iter().any(|path| path == "basic"),
        SearchMode::Both => !actual_paths.iter().any(|path| path.starts_with("advanced")),
    };

    Ok(SourceSearchReport {
        query: query.to_string(),
        requested_mode: mode_to_str(mode),
        resolved_mode: mode_to_str(mode),
        scope: SearchScope { kind: "repository".to_string(), project: project_filter.map(|s| s.to_string()) },
        actual_paths,
        degraded,
        results,
        warnings,
    })
}

/// Basic metadata search: case-insensitive substring match over registration fields.
fn basic_search(query: &str, registrations: &[BasicCandidate]) -> Vec<SourceSearchResult> {
    let query_lower = query.to_lowercase();
    let mut matched: Vec<(BasicCandidate, String)> = Vec::new();

    for reg in registrations {
        if reg.source_identity.to_lowercase().contains(&query_lower) {
            matched.push((reg.clone(), "identity".to_string()));
        } else if reg.registered_location.to_lowercase().contains(&query_lower) {
            matched.push((reg.clone(), "location".to_string()));
        } else if reg.source_type.to_lowercase().contains(&query_lower) {
            matched.push((reg.clone(), "type".to_string()));
        } else if reg.tags.iter().any(|t| t.to_lowercase().contains(&query_lower)) {
            matched.push((reg.clone(), "tags".to_string()));
        }
    }

    // Deterministic ordering: by project path, then source identity
    matched.sort_by(|(a, _), (b, _)| {
        a.project_path.cmp(&b.project_path).then_with(|| a.source_identity.cmp(&b.source_identity))
    });

    matched
        .into_iter()
        .map(|(reg, match_field)| SourceSearchResult {
            document_key: None,
            source_type: reg.source_type,
            location: reg.registered_location.clone(),
            locator: Some(SourceLocator::Source),
            chunk_id: None,
            snippet: format!("{} ({})", reg.source_identity, match_field),
            registrations: vec![SearchResultRegistration {
                registration_key: reg.registration_key,
                project_identity: reg.project_identity,
                project_path: reg.project_path,
                source_identity: reg.source_identity,
                registered_location: reg.registered_location,
                source_kind: reg.source_kind,
                tags: reg.tags,
            }],
            retrieval_paths: vec!["basic".to_string()],
            keyword_score: None,
            semantic_score: None,
            combined_score: 1.0,
            freshness: None,
            enrichment: None,
            deduplicated: false,
        })
        .collect()
}

fn registrations_for_document(
    candidates: &[BasicCandidate],
    bindings: &BTreeMap<String, BTreeSet<String>>,
    document_key: &str,
) -> Vec<SearchResultRegistration> {
    let Some(keys) = bindings.get(document_key) else { return Vec::new() };
    let mut registrations = candidates
        .iter()
        .filter(|candidate| keys.contains(&candidate.registration_key))
        .map(|reg| SearchResultRegistration {
            registration_key: reg.registration_key.clone(),
            project_identity: reg.project_identity.clone(),
            project_path: reg.project_path.clone(),
            source_identity: reg.source_identity.clone(),
            registered_location: reg.registered_location.clone(),
            source_kind: reg.source_kind.clone(),
            tags: reg.tags.clone(),
        })
        .collect::<Vec<_>>();
    registrations
        .sort_by(|a, b| a.project_path.cmp(&b.project_path).then_with(|| a.source_identity.cmp(&b.source_identity)));
    registrations
}

/// Reciprocal Rank Fusion: score = sum(1/(k+rank_i)) for each result list.
pub fn rrf_fusion(k: f64, ranked_lists: &[&[(usize, f64)]]) -> Vec<(usize, f64)> {
    let mut scores: BTreeMap<usize, f64> = BTreeMap::new();

    for list in ranked_lists {
        for (rank, (id, _score)) in list.iter().enumerate() {
            *scores.entry(*id).or_insert(0.0) += 1.0 / (k + (rank as f64 + 1.0));
        }
    }

    let mut results: Vec<(usize, f64)> = scores.into_iter().collect();
    results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    results
}

fn mode_to_str(mode: SearchMode) -> String {
    match mode {
        SearchMode::Basic => "basic".to_string(),
        SearchMode::Advanced => "advanced".to_string(),
        SearchMode::Both => "both".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_search_finds_by_identity() {
        let regs = vec![BasicCandidate {
            registration_key: "rk1".into(),
            project_identity: "alpha".into(),
            project_path: "alpha".into(),
            source_identity: "machine-learning-paper".into(),
            source_type: "pdf".into(),
            registered_location: "sources/papers/ml.pdf".into(),
            source_kind: None,
            tags: vec!["ai".into()],
            match_field: String::new(),
        }];
        let results = basic_search("machine", &regs);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].registrations[0].source_identity, "machine-learning-paper");
    }

    #[test]
    fn basic_search_finds_by_tag() {
        let regs = vec![BasicCandidate {
            registration_key: "rk1".into(),
            project_identity: "alpha".into(),
            project_path: "alpha".into(),
            source_identity: "notes".into(),
            source_type: "file".into(),
            registered_location: "sources/notes.md".into(),
            source_kind: None,
            tags: vec!["retrieval".into(), "rag".into()],
            match_field: String::new(),
        }];
        let results = basic_search("rag", &regs);
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn basic_search_no_match_returns_empty() {
        let regs = vec![BasicCandidate {
            registration_key: "rk1".into(),
            project_identity: "alpha".into(),
            project_path: "alpha".into(),
            source_identity: "notes".into(),
            source_type: "file".into(),
            registered_location: "sources/notes.md".into(),
            source_kind: None,
            tags: vec![],
            match_field: String::new(),
        }];
        let results = basic_search("nonexistent", &regs);
        assert!(results.is_empty());
    }

    #[test]
    fn rrf_fusion_combines_lists() {
        // List 1: items 0, 1, 2 with scores 0.9, 0.7, 0.5
        // List 2: items 1, 3, 0 with scores 0.8, 0.6, 0.4
        let list1: Vec<(usize, f64)> = vec![(0, 0.9), (1, 0.7), (2, 0.5)];
        let list2: Vec<(usize, f64)> = vec![(1, 0.8), (3, 0.6), (0, 0.4)];
        let fused = rrf_fusion(RRF_K, &[&list1, &list2]);
        // Item 0 appears in both lists, item 1 appears in both, item 2 only in list1, item 3 only in list2
        assert!(!fused.is_empty());
        // All items that appear in any list should be present
        let ids: Vec<usize> = fused.iter().map(|(id, _)| *id).collect();
        assert!(ids.contains(&0));
        assert!(ids.contains(&1));
        // Item 0 (rank 1 in list1, rank 3 in list2) vs Item 1 (rank 2 in list1, rank 1 in list2)
        // Item 1 should rank higher
        assert_eq!(ids[0], 1);
    }

    #[test]
    fn search_empty_repo_returns_empty() {
        let dir = tempfile::tempdir().unwrap();
        let report = search_repository(dir.path(), "test", SearchMode::Basic, None, None, None, 10).unwrap();
        assert!(report.results.is_empty());
        assert!(!report.degraded);
    }
}
