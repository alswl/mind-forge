//! Repository-wide Source search: basic (metadata), advanced (FTS + vector + RRF),
//! and fused both-mode retrieval.
//!
//! Default scope is all live projects in `minds.yaml`. An explicit `--project`
//! acts as a filter; cwd never creates an implicit project filter.
//!
//! Read-only — never mutates, fetches, or creates files. Degraded mode (both
//! without advanced) returns basic results with a warning.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Component;
use std::path::Path;

use arrow_array::Array;
use chrono::NaiveDate;

use crate::error::{MfError, Result};
use crate::model::source_advanced::{
    SearchResultRegistration, SearchScope, SourceLocator, SourceSearchReport, SourceSearchResult,
};
use crate::model::source_search::SearchMode;

use super::identity;

/// RRF constant k.
const RRF_K: f64 = 60.0;

/// Parse a user-supplied revision string into a concrete filter.
///
/// Returns `Ok(RevisionFilter::Exact(n))` for an integer revision number, or
/// `Ok(RevisionFilter::Date(date))` when the input looks like a date (ISO 8601
/// calendar, relative expressions like `yesterday` or `N days ago`).
fn parse_revision(raw: &str) -> Result<RevisionFilter> {
    // Try integer first.
    if let Ok(n) = raw.trim().parse::<i64>() {
        return Ok(RevisionFilter::Exact(n));
    }

    // Direct calendar formats.
    let formats = ["%Y-%m-%d", "%Y/%m/%d", "%Y%m%d"];
    for fmt in &formats {
        if let Ok(d) = NaiveDate::parse_from_str(raw.trim(), fmt) {
            return Ok(RevisionFilter::Date(d));
        }
    }

    let today = chrono::Utc::now().date_naive();
    match raw.trim().to_lowercase().as_str() {
        "today" => return Ok(RevisionFilter::Date(today)),
        "yesterday" => return Ok(RevisionFilter::Date(today - chrono::Duration::days(1))),
        _ => {}
    }

    if let Some(stripped) = raw.trim().to_lowercase().strip_suffix(" days ago")
        && let Ok(n) = stripped.trim().parse::<i64>()
    {
        return Ok(RevisionFilter::Date(today - chrono::Duration::days(n)));
    }

    // Last resort: try chrono's own lenient parsing.
    if let Ok(d) = NaiveDate::parse_from_str(raw.trim(), "%Y-%m-%d") {
        return Ok(RevisionFilter::Date(d));
    }

    Err(MfError::usage(
        format!(
            "invalid revision '{}': expected an integer, a date like 2026-07-25, or a relative date like yesterday or '7 days ago'",
            raw
        ),
        None,
    ))
}

/// Resolved revision filter: either an exact integer revision or a calendar
/// date that must be mapped to per-registration revisions at query time.
#[derive(Debug, Clone)]
enum RevisionFilter {
    Exact(i64),
    Date(NaiveDate),
}

/// Map a target date to the latest `content_revision` per registration whose
/// `synced_at` is on or before that date.
///
/// Returns `HashMap<registration_key, content_revision>`. Registrations with
/// no `synced_at` or whose earliest sync is already after the target date are
/// omitted from the map (they will produce zero results).
fn resolve_revision_date(store: &super::lance_store::LanceStore, target: NaiveDate) -> Result<HashMap<String, i64>> {
    let mut map: HashMap<String, i64> = HashMap::new();
    let target_str = target.format("%Y-%m-%d").to_string();

    for batch in store.scan_rows("registration_content")? {
        let Some(registrations) = batch
            .column_by_name("registration_key")
            .and_then(|c| c.as_any().downcast_ref::<arrow_array::StringArray>())
        else {
            continue;
        };
        let Some(revisions) =
            batch.column_by_name("content_revision").and_then(|c| c.as_any().downcast_ref::<arrow_array::Int64Array>())
        else {
            continue;
        };
        let synced =
            batch.column_by_name("synced_at").and_then(|c| c.as_any().downcast_ref::<arrow_array::StringArray>());

        for row in 0..batch.num_rows() {
            let synced_str = synced.and_then(|s| if s.is_null(row) { None } else { Some(s.value(row)) });
            // A row without a synced_at timestamp cannot be resolved by date.
            let Some(synced_str) = synced_str else { continue };

            // synced_at is ISO 8601, e.g. "2026-07-25T12:34:56Z". Compare
            // only the date prefix so that records on the target date itself
            // are included.
            let synced_date = if synced_str.len() >= 10 { &synced_str[..10] } else { synced_str };
            if synced_date > target_str.as_str() {
                continue; // this revision is newer than the target date
            }

            let reg = registrations.value(row).to_string();
            let rev = revisions.value(row);
            map.entry(reg)
                .and_modify(|current| {
                    if rev > *current {
                        *current = rev;
                    }
                })
                .or_insert(rev);
        }
    }

    Ok(map)
}

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
    labels: std::collections::BTreeMap<String, String>,
    annotations: std::collections::BTreeMap<String, String>,
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
#[allow(clippy::too_many_arguments)]
pub fn search_repository(
    repo_root: &Path,
    query: &str,
    mode: SearchMode,
    project_filter: Option<&str>,
    kind_filter: Option<&str>,
    source_filter: Option<&str>,
    label_filter: &[(String, String)],
    revision_filter: Option<&str>,
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
        label_filter,
        revision_filter,
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

    let advanced_dir = super::advanced_store_dir(repo_root);
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
    super::lance_store::LanceStore::open(&database_path)
        .map(|s| Some(s.with_dimension(super::config::embedding_dimension_for(repo_root))))
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
    label_filter: &[(String, String)],
    revision_filter: Option<&str>,
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
    if let Some(s) = store {
        let config = super::config::load_repository_config(repo_root)?;
        let catalog = super::catalog::SourceCatalog::discover(&config, repo_root)?;
        for registration in catalog.registrations(Some(s))? {
            if project_filter.is_some_and(|filter| {
                filter != registration.project_identity
                    && filter
                        != Path::new(&registration.project_path)
                            .file_name()
                            .and_then(|name| name.to_str())
                            .unwrap_or("")
            }) {
                continue;
            }
            if kind_filter.is_some_and(|kind| kind != registration.source_type)
                || source_filter.is_some_and(|source| source != registration.source_identity)
            {
                continue;
            }
            let labels: std::collections::BTreeMap<String, String> =
                serde_json::from_str(&registration.labels_json).unwrap_or_default();
            // Label selectors are ANDed equality (k8s-style); a registration
            // must carry every requested label to be a candidate.
            if !label_filter.iter().all(|(k, v)| labels.get(k).map(String::as_str) == Some(v.as_str())) {
                continue;
            }
            all_registrations.push(BasicCandidate {
                registration_key: registration.registration_key,
                project_identity: registration.project_identity,
                project_path: registration.project_path,
                source_identity: registration.source_identity,
                source_type: registration.source_type,
                registered_location: registration.registered_location,
                source_kind: registration.source_kind,
                tags: serde_json::from_str(&registration.tags_json).unwrap_or_default(),
                labels,
                annotations: serde_json::from_str(&registration.annotations_json).unwrap_or_default(),
                match_field: String::new(),
            });
        }
    } else {
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
                        let location = source
                            .get("path")
                            .or_else(|| source.get("url"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("unknown");
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

                        // Legacy filesystem registrations carry no labels; a
                        // label selector therefore excludes them all.
                        if !label_filter.is_empty() {
                            continue;
                        }
                        all_registrations.push(BasicCandidate {
                            registration_key: rk,
                            project_identity: project_identity.to_string(),
                            project_path: project_name.to_string(),
                            source_identity: name.to_string(),
                            source_type: kind.to_string(),
                            registered_location: location.to_string(),
                            source_kind,
                            tags,
                            labels: std::collections::BTreeMap::new(),
                            annotations: std::collections::BTreeMap::new(),
                            match_field: String::new(),
                        });
                    }
                }
            }
        }
    }

    // Perform basic search
    let basic_results = basic_search(query, &all_registrations);
    let total_basic = basic_results.len();
    // Resolve the revision filter: integer → exact match; date → per-registration
    // revision map resolved from synced_at timestamps.
    let resolved_rev: Option<RevisionFilter> = match revision_filter {
        Some(raw) => Some(parse_revision(raw)?),
        None => None,
    };
    let date_rev_map: Option<HashMap<String, i64>> = match (&resolved_rev, store) {
        (Some(RevisionFilter::Date(date)), Some(s)) => Some(resolve_revision_date(s, *date)?),
        _ => None,
    };

    let mut document_bindings = BTreeMap::<String, BTreeSet<String>>::new();
    if let Some(s) = store {
        // Select one relation per registration: the current (highest) revision
        // by default, the exact `revision_filter` integer, or the revision
        // resolved from a target date. Chunks belonging to any other revision
        // then have no binding and are excluded from results, so search
        // defaults to the current version and never mixes revisions.
        let mut selected = BTreeMap::<String, (i64, String)>::new();
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
            let revisions = batch
                .column_by_name("content_revision")
                .and_then(|c| c.as_any().downcast_ref::<arrow_array::Int64Array>());
            for row in 0..batch.num_rows() {
                if documents.is_null(row) {
                    continue;
                }
                let rev = revisions.filter(|r| !r.is_null(row)).map(|r| r.value(row)).unwrap_or(1);
                let reg = registrations.value(row).to_string();
                match &resolved_rev {
                    Some(RevisionFilter::Exact(target)) if rev == *target => {
                        selected.insert(reg, (rev, documents.value(row).to_string()));
                    }
                    Some(RevisionFilter::Exact(_)) => {}
                    Some(RevisionFilter::Date(_)) => {
                        // Only select rows whose revision matches the date-resolved
                        // target for this registration (if any).
                        if let Some(date_map) = &date_rev_map
                            && date_map.get(&reg).is_some_and(|target| rev == *target)
                        {
                            selected.insert(reg, (rev, documents.value(row).to_string()));
                        }
                    }
                    None => {
                        if selected.get(&reg).is_none_or(|(current, _)| rev > *current) {
                            selected.insert(reg, (rev, documents.value(row).to_string()));
                        }
                    }
                }
            }
        }
        for (reg, (_, document_key)) in selected {
            document_bindings.entry(document_key).or_default().insert(reg);
        }
    }

    // LanceDB advanced retrieval. When an embedding model is installed, fuse
    // vector similarity with BM25 using native reciprocal-rank fusion;
    // otherwise use BM25-only full-text search. Loading is gated on an explicit
    // install so a read-only search never triggers a download.
    let lancedb_available = store.is_some();
    let mut advanced_results: Vec<SourceSearchResult> = Vec::new();

    if let Some(s) = store {
        let query_embedding = match super::embedding::provider_for_repo(repo_root) {
            Ok(Some(provider)) => match provider.embed_query(query) {
                Ok(vector) => Some(vector),
                Err(error) => {
                    warnings.push(format!("embedding provider unavailable; semantic retrieval degraded: {error}"));
                    None
                }
            },
            Ok(None) => {
                warnings.push("embedding provider is not configured; semantic retrieval degraded".to_string());
                None
            }
            Err(error) => {
                warnings
                    .push(format!("embedding provider configuration is invalid; semantic retrieval degraded: {error}"));
                None
            }
        };
        let (batches, path_label, hybrid) = match &query_embedding {
            Some(vector) => {
                (s.hybrid_search("chunks", query, vector, "vector", limit as usize), "advanced_hybrid", true)
            }
            None => (s.fts_search("chunks", query, &["text"], limit as usize), "advanced_keyword", false),
        };
        if let Ok(batches) = batches {
            for batch in &batches {
                let str_col = |name: &str| {
                    batch.column_by_name(name).and_then(|c| c.as_any().downcast_ref::<arrow_array::StringArray>())
                };
                let (Some(texts), Some(ids), Some(dks)) =
                    (str_col("text"), str_col("chunk_id"), str_col("document_key"))
                else {
                    continue;
                };
                let locators = str_col("locator_json");
                // Hybrid results carry `_relevance_score` (RRF); FTS carry `_score` (BM25).
                let scores = batch
                    .column_by_name("_relevance_score")
                    .or_else(|| batch.column_by_name("_score"))
                    .and_then(|c| c.as_any().downcast_ref::<arrow_array::Float32Array>());
                for row in 0..batch.num_rows() {
                    let dk = dks.value(row).to_string();
                    let score = scores.map(|s| s.value(row)).unwrap_or(0.0);
                    // Attach only registrations that passed the prefilters; a
                    // chunk with no matching registration is filtered out. The
                    // result's type is the matched registration's real type.
                    let regs = registrations_for_document(&all_registrations, &document_bindings, &dk);
                    if regs.is_empty() {
                        continue;
                    }
                    let source_type =
                        regs.first().map(|r| r.source_type.clone()).unwrap_or_else(|| "unknown".to_string());
                    advanced_results.push(SourceSearchResult {
                        document_key: Some(dk.clone()),
                        source_type,
                        location: locators
                            .map(|l| l.value(row).to_string())
                            .unwrap_or_else(|| "indexed-content".to_string()),
                        locator: Some(SourceLocator::Source),
                        chunk_id: Some(ids.value(row).to_string()),
                        snippet: texts.value(row).chars().take(200).collect(),
                        registrations: regs,
                        retrieval_paths: vec![path_label.to_string()],
                        keyword_score: (!hybrid).then_some(score),
                        semantic_score: None,
                        combined_score: score as f64,
                        freshness: Some("ready".to_string()),
                        enrichment: None,
                        deduplicated: false,
                    });
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
                let regs = registrations_for_document(&all_registrations, &document_bindings, documents.value(row));
                if regs.is_empty() {
                    continue;
                }
                let source_type = regs.first().map(|r| r.source_type.clone()).unwrap_or_else(|| "unknown".to_string());
                advanced_results.push(SourceSearchResult {
                    document_key: Some(documents.value(row).to_string()),
                    source_type,
                    location: locators.value(row).to_string(),
                    locator: Some(SourceLocator::Source),
                    chunk_id: Some(ids.value(row).to_string()),
                    snippet: text.chars().take(200).collect(),
                    registrations: regs,
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

    // Locators must never expose URL credentials, however the source was
    // registered.
    for result in &mut results {
        result.location = super::acquisition::redact_locator(&result.location);
        for registration in &mut result.registrations {
            registration.registered_location = super::acquisition::redact_locator(&registration.registered_location);
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
            source_type: reg.source_type.clone(),
            location: reg.registered_location.clone(),
            locator: Some(SourceLocator::Source),
            chunk_id: None,
            snippet: format!("{} ({})", reg.source_identity, match_field),
            registrations: vec![SearchResultRegistration {
                registration_key: reg.registration_key,
                project_identity: reg.project_identity,
                project_path: reg.project_path,
                source_identity: reg.source_identity,
                source_type: reg.source_type,
                registered_location: reg.registered_location,
                source_kind: reg.source_kind,
                tags: reg.tags,
                labels: reg.labels,
                annotations: reg.annotations,
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
            source_type: reg.source_type.clone(),
            registered_location: reg.registered_location.clone(),
            source_kind: reg.source_kind.clone(),
            tags: reg.tags.clone(),
            labels: reg.labels.clone(),
            annotations: reg.annotations.clone(),
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
            labels: Default::default(),
            annotations: Default::default(),
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
            labels: Default::default(),
            annotations: Default::default(),
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
            labels: Default::default(),
            annotations: Default::default(),
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
        let report = search_repository(dir.path(), "test", SearchMode::Basic, None, None, None, &[], None, 10).unwrap();
        assert!(report.results.is_empty());
        assert!(!report.degraded);
    }
}
