//! Legacy compatibility projections: mirror Lance primary registrations into
//! project `mind-index.yaml.sources`, compute projection fingerprints, and
//! detect drift.
//!
//! In Lance mode, every successful registration mutation publishes to the
//! primary Lance table first, then best-effort mirrors the owning project's
//! `sources:` key. The mirror is a merge, never a replacement (spec 075
//! I-1/I-2): a YAML entry the store does not know about is preserved and
//! surfaced for later import, never deleted; only the `sources:` key is
//! touched, so `terms:`/`articles:`/etc. are byte-identical afterward
//! (FR-013). Projection failure reports degraded state without changing the
//! primary operation's exit code.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

use serde::Serialize;
use serde_yaml::Value;

use crate::error::{MfError, Result};
use crate::model::source_advanced::ProjectionStatus;
use crate::service::util::yaml_splice::splice_top_level_key;
#[cfg(test)]
use crate::service::util::yaml_splice::top_level_key_span;

use super::catalog::CatalogRegistration;

fn name_key() -> Value {
    Value::String("name".to_string())
}

/// Fields whose representation is owned by the source registration schema.
/// Everything else in a legacy entry is opaque user/future-schema data and
/// must travel with the registration when it is adopted into Lance.
const OWNED_SOURCE_FIELDS: &[&str] =
    &["name", "kind", "type", "path", "url", "source_kind", "tags", "added_at", "updated_at"];

/// Extract source-entry fields which the catalog does not model.  Keeping
/// them as JSON in `extras_json` makes an activate/adopt → project mirror
/// round trip lossless without making the catalog schema chase every future
/// YAML field.
pub(crate) fn source_entry_extras(entry: &Value) -> Option<String> {
    let Value::Mapping(entry) = entry else {
        return None;
    };
    let extras = entry
        .iter()
        .filter(|(key, _)| !key.as_str().is_some_and(|key| OWNED_SOURCE_FIELDS.contains(&key)))
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect::<serde_yaml::Mapping>();
    (!extras.is_empty()).then(|| serde_json::to_string(&Value::Mapping(extras)).ok()).flatten()
}

/// Return opaque source-entry fields keyed by their registered path or URL.
/// Both list and mapping forms are accepted because older project indexes use
/// lists while the current serializer uses a path-keyed mapping.
pub(crate) fn source_extras_by_location(index: &Value) -> HashMap<String, String> {
    let Some(sources) = index.get("sources") else {
        return HashMap::new();
    };
    let entries: Vec<(&Value, Option<&Value>)> = match sources {
        Value::Sequence(entries) => entries.iter().map(|entry| (entry, None)).collect(),
        Value::Mapping(entries) => entries.iter().map(|(key, entry)| (entry, Some(key))).collect(),
        _ => return HashMap::new(),
    };

    entries
        .into_iter()
        .filter_map(|(entry, fallback_location)| {
            let location =
                entry.get("path").or_else(|| entry.get("url")).or(fallback_location).and_then(Value::as_str)?;
            source_entry_extras(entry).map(|extras| (location.to_string(), extras))
        })
        .collect()
}

/// Read the current `sources:` value into an ordered `(name -> mapping)`
/// view, accepting both the sequence-of-mappings shape this module writes
/// and the mapping-keyed-by-name shape some legacy/hand-edited files use.
fn read_existing_sources(sources_value: Option<&Value>) -> (Vec<String>, HashMap<String, serde_yaml::Mapping>) {
    let mut order = Vec::new();
    let mut by_name = HashMap::new();
    match sources_value {
        Some(Value::Sequence(seq)) => {
            for item in seq {
                if let Value::Mapping(m) = item
                    && let Some(name) = m.get(name_key()).and_then(|v| v.as_str())
                {
                    order.push(name.to_string());
                    by_name.insert(name.to_string(), m.clone());
                }
            }
        }
        Some(Value::Mapping(map)) => {
            for (k, v) in map {
                let Some(fallback_key) = k.as_str() else { continue };
                let mut entry = match v {
                    Value::Mapping(m) => m.clone(),
                    _ => serde_yaml::Mapping::new(),
                };
                // `service::index::serialize_mind_index` keys this mapping by
                // the source's *path* (`source_key`), not its name — reading
                // the map key as the name here corrupted every entry's `name`
                // to its path (spec 075 regression caught by the US2 adoption
                // tests). The entry's own `name:` field is authoritative;
                // the map key is only a fallback for a hand-written entry
                // that omits it.
                let name = entry.get(name_key()).and_then(|v| v.as_str()).map(str::to_string).unwrap_or_else(|| {
                    entry.insert(name_key(), Value::String(fallback_key.to_string()));
                    fallback_key.to_string()
                });
                order.push(name.clone());
                by_name.insert(name, entry);
            }
        }
        _ => {}
    }
    (order, by_name)
}

/// Build (or update) one projected entry from a store registration. When an
/// existing YAML mapping for the same name is supplied, start from it —
/// preserving any field the store does not own — and overwrite only the
/// fields the store is authoritative for.
fn project_entry(registration: &CatalogRegistration, existing: Option<&serde_yaml::Mapping>) -> Value {
    let mut source = existing.cloned().unwrap_or_default();
    source.insert(name_key(), Value::String(registration.source_identity.clone()));
    source.insert("kind".into(), Value::String(registration.source_type.clone()));
    let is_url = registration.registered_location.starts_with("http://")
        || registration.registered_location.starts_with("https://");
    if is_url {
        source.insert("url".into(), Value::String(registration.registered_location.clone()));
        source.remove("path");
    } else {
        source.insert("path".into(), Value::String(registration.registered_location.clone()));
        source.remove("url");
    }
    match &registration.source_kind {
        Some(source_kind) => {
            source.insert("source_kind".into(), Value::String(source_kind.clone()));
        }
        None => {
            source.remove("source_kind");
        }
    }
    let tags: Vec<String> = serde_json::from_str(&registration.tags_json).unwrap_or_default();
    if tags.is_empty() {
        source.remove("tags");
    } else {
        source.insert("tags".into(), Value::Sequence(tags.into_iter().map(Value::String).collect()));
    }
    // The store is authoritative for these once populated (spec 075 FR-011);
    // an entry not yet carrying them (pre-migration) leaves the existing
    // YAML value, if any, untouched rather than erasing it.
    if let Some(added_at) = &registration.added_at {
        source.insert("added_at".into(), Value::String(added_at.clone()));
    }
    if let Some(updated_at) = &registration.updated_at {
        source.insert("updated_at".into(), Value::String(updated_at.clone()));
    }
    // Re-inject fields the store carries but does not itself interpret
    // (spec 075 FR-012) — this is what makes the mirror lossless for
    // hand-added or future per-entry fields.
    if let Some(extras) = &registration.extras_json
        && let Ok(Value::Mapping(extra_map)) = serde_yaml::from_str::<Value>(extras)
    {
        for (k, v) in extra_map {
            source.insert(k, v);
        }
    }
    Value::Mapping(source)
}

/// Merge store registrations into the existing `sources:` value. An entry the
/// store does not (yet) know about is kept — divergence that resolves by
/// import, never by deletion (spec 075 I-1) — except for names the caller
/// explicitly lists in `permitted_removals` because it just performed a real
/// removal.
fn merge_sources(
    sources_value: Option<&Value>,
    registrations: &[CatalogRegistration],
    permitted_removals: &[String],
) -> Value {
    let (order, by_name) = read_existing_sources(sources_value);
    let mut result = Vec::with_capacity(order.len().max(registrations.len()));
    let mut emitted: HashSet<String> = HashSet::new();

    for name in &order {
        if !emitted.insert(name.clone()) {
            continue;
        }
        if let Some(registration) = registrations.iter().find(|r| &r.source_identity == name) {
            result.push(project_entry(registration, by_name.get(name)));
        } else if permitted_removals.iter().any(|removed| removed == name) {
            // An explicit removal operation just dropped this name from the
            // store — the mirror follows, unlike unexplained divergence.
        } else {
            // The store does not know this entry: keep it exactly as-is.
            result.push(Value::Mapping(by_name[name].clone()));
        }
    }

    let mut new_registrations: Vec<&CatalogRegistration> =
        registrations.iter().filter(|r| !emitted.contains(&r.source_identity)).collect();
    new_registrations.sort_by(|a, b| a.source_identity.cmp(&b.source_identity));
    for registration in new_registrations {
        result.push(project_entry(registration, None));
        emitted.insert(registration.source_identity.clone());
    }

    if matches!(sources_value, Some(Value::Mapping(_))) {
        let mut mapped = serde_yaml::Mapping::new();
        for entry in result {
            let Value::Mapping(fields) = &entry else {
                continue;
            };
            let key = fields
                .get("path")
                .or_else(|| fields.get("url"))
                .or_else(|| fields.get(name_key()))
                .cloned()
                .unwrap_or(Value::Null);
            mapped.insert(key, entry);
        }
        Value::Mapping(mapped)
    } else {
        Value::Sequence(result)
    }
}

/// Compare Lance primary registrations with a project's legacy YAML.
#[derive(Debug, Serialize)]
pub struct ProjectionComparison {
    pub project_key: String,
    pub project_identity: String,
    pub primary_count: usize,
    pub legacy_count: usize,
    pub state: ProjectionStatus,
    pub expected_fingerprint: Option<String>,
    pub observed_fingerprint: Option<String>,
    pub drift_details: Vec<String>,
}

/// Export Lance primary registrations for a project to its legacy YAML.
///
/// Equivalent to [`export_project_with_removals`] with no permitted removals
/// — the common case, where divergence resolves by import, never deletion
/// (spec 075 I-1).
pub fn export_project(repo_root: &Path, project_name: &str, dry_run: bool) -> Result<ProjectionComparison> {
    export_project_with_removals(repo_root, project_name, dry_run, &[])
}

/// Export Lance primary registrations for a project to its legacy YAML,
/// permitting `permitted_removals` (source identities) to be dropped from the
/// existing YAML even though the store no longer has them.
///
/// Every other divergence resolves by import: an existing YAML entry with no
/// store row is kept, not deleted (I-1) — only an entry the caller names here,
/// because it just performed an explicit removal (`mf source remove` or the
/// stale-file cleanup in `clean_registrations`), may be dropped.
pub fn export_project_with_removals(
    repo_root: &Path,
    project_name: &str,
    dry_run: bool,
    permitted_removals: &[String],
) -> Result<ProjectionComparison> {
    if project_name == "all" {
        let projects_dir = repo_root.join("projects");
        let mut comparisons = Vec::new();
        if projects_dir.exists() {
            for entry in fs::read_dir(&projects_dir)? {
                let entry = entry?;
                if entry.file_type()?.is_dir() {
                    comparisons.push(export_project_with_removals(
                        repo_root,
                        &entry.file_name().to_string_lossy(),
                        dry_run,
                        permitted_removals,
                    )?);
                }
            }
        }
        let primary_count = comparisons.iter().map(|comparison| comparison.primary_count).sum();
        let legacy_count = comparisons.iter().map(|comparison| comparison.legacy_count).sum();
        let drift_details = comparisons.into_iter().flat_map(|comparison| comparison.drift_details).collect::<Vec<_>>();
        return Ok(ProjectionComparison {
            project_key: "all".to_string(),
            project_identity: "all".to_string(),
            primary_count,
            legacy_count,
            state: if drift_details.is_empty() { ProjectionStatus::Current } else { ProjectionStatus::Drifted },
            expected_fingerprint: None,
            observed_fingerprint: None,
            drift_details,
        });
    }
    let project_path = repo_root.join("projects").join(project_name);
    let index_path = project_path.join("mind-index.yaml");

    if !index_path.exists() {
        return Ok(ProjectionComparison {
            project_key: project_name.to_string(),
            project_identity: project_name.to_string(),
            primary_count: 0,
            legacy_count: 0,
            state: ProjectionStatus::Missing,
            expected_fingerprint: None,
            observed_fingerprint: None,
            drift_details: vec!["no legacy index found".to_string()],
        });
    }

    let yaml_data = fs::read_to_string(&index_path)?;
    let legacy: serde_yaml::Value = serde_yaml::from_str(&yaml_data)
        .map_err(|e| MfError::advanced_store(format!("cannot parse legacy index: {e}"), None))?;

    // Project indexes may use either the historical sequence or the current
    // path-keyed mapping form. Count through the same reader used by the
    // merge so status/reporting does not claim a mapping-form index is empty.
    let existing_sources = legacy.get("sources").cloned();
    let legacy_count = read_existing_sources(existing_sources.as_ref()).0.len();
    let config = super::config::load_repository_config(repo_root)?;
    if !config.is_lance() {
        return Err(MfError::usage("legacy export requires an active Lance backend".to_string(), None));
    }
    let store = super::sync::open_active_store(repo_root)?;
    let catalog = super::catalog::SourceCatalog::discover(&config, repo_root)?;
    let expected_path = format!("projects/{project_name}");
    let registrations = catalog
        .registrations(Some(&store))?
        .into_iter()
        .filter(|registration| registration.project_path == expected_path)
        .collect::<Vec<_>>();
    let primary_count = registrations.len();
    // FR-014/I-1: entries with no store row yet are kept, not dropped — `mf
    // source index` is what imports them into the store.
    let merged = merge_sources(existing_sources.as_ref(), &registrations, permitted_removals);
    // FR-013/I-2: splice only the `sources:` block into the raw text so every
    // other key (`terms:`, `articles:`, `prompts:`, `thinking:`, ...) stays
    // byte-identical — see `splice_top_level_key`.
    let mut sources_block_map = serde_yaml::Mapping::new();
    sources_block_map.insert(Value::String("sources".to_string()), merged);
    let sources_block = serde_yaml::to_string(&Value::Mapping(sources_block_map))
        .map_err(|e| MfError::advanced_store(format!("cannot serialize sources projection: {e}"), None))?;
    let rendered = splice_top_level_key(&yaml_data, "sources", &sources_block);
    let expected_fp = crate::service::source::advanced::identity::raw_fingerprint(rendered.as_bytes());
    let observed_fp = crate::service::source::advanced::identity::raw_fingerprint(yaml_data.as_bytes());

    if dry_run {
        return Ok(ProjectionComparison {
            project_key: project_name.to_string(),
            project_identity: legacy.get("project").and_then(|v| v.as_str()).unwrap_or(project_name).to_string(),
            primary_count,
            legacy_count,
            state: if expected_fp == observed_fp { ProjectionStatus::Current } else { ProjectionStatus::Drifted },
            expected_fingerprint: Some(expected_fp.clone()),
            observed_fingerprint: Some(observed_fp),
            drift_details: if expected_fp
                == crate::service::source::advanced::identity::raw_fingerprint(yaml_data.as_bytes())
            {
                vec![]
            } else {
                vec!["legacy YAML differs from Lance primary projection".to_string()]
            },
        });
    }
    // Nothing to write when the mirror already matches: rewriting an
    // identical file only churns its mtime on every registration mutation.
    if expected_fp != observed_fp {
        let tmp = index_path.with_extension("yaml.tmp");
        fs::write(&tmp, &rendered)?;
        fs::rename(&tmp, &index_path)?;
    }
    Ok(ProjectionComparison {
        project_key: project_name.to_string(),
        project_identity: legacy.get("project").and_then(|v| v.as_str()).unwrap_or(project_name).to_string(),
        primary_count,
        legacy_count,
        state: ProjectionStatus::Current,
        expected_fingerprint: Some(expected_fp.clone()),
        observed_fingerprint: Some(expected_fp),
        drift_details: vec![],
    })
}

/// Check projection state for all projects (read-only).
pub fn status_all(repo_root: &Path) -> Result<Vec<ProjectionComparison>> {
    let projects_dir = repo_root.join("projects");
    if !projects_dir.exists() {
        return Ok(Vec::new());
    }

    let mut results = Vec::new();
    for entry in fs::read_dir(&projects_dir)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            let name = entry.file_name().to_string_lossy().to_string();
            if let Ok(c) = export_project(repo_root, &name, true) {
                results.push(c)
            }
        }
    }
    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn export_nonexistent_project_reports_missing() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("projects")).unwrap();
        let result = export_project(dir.path(), "nonexistent", false).unwrap();
        assert_eq!(result.state, ProjectionStatus::Missing);
    }

    fn sample_registration(extras_json: Option<&str>) -> CatalogRegistration {
        CatalogRegistration {
            registration_key: "key-1".to_string(),
            project_key: "alpha".to_string(),
            project_identity: "alpha".to_string(),
            project_path: "alpha".to_string(),
            source_identity: "notes".to_string(),
            source_type: "file".to_string(),
            source_kind: None,
            registered_location: "sources/notes.md".to_string(),
            tags_json: "[]".to_string(),
            labels_json: "{}".to_string(),
            annotations_json: "{}".to_string(),
            state: "live".to_string(),
            context_json: None,
            imported_by_json: None,
            added_at: Some("2026-01-01T00:00:00Z".to_string()),
            updated_at: Some("2026-01-01T00:00:00Z".to_string()),
            extras_json: extras_json.map(str::to_string),
        }
    }

    #[test]
    fn project_entry_passes_through_an_unrecognised_field() {
        let registration = sample_registration(Some(r#"{"custom_field": "keep me"}"#));
        let entry = project_entry(&registration, None);
        let Value::Mapping(m) = entry else { panic!("expected a mapping") };
        assert_eq!(
            m.get(Value::String("custom_field".to_string())),
            Some(&Value::String("keep me".to_string())),
            "an extras_json field the store does not itself interpret must survive into the projected entry: {m:?}"
        );
    }

    #[test]
    fn source_extras_are_keyed_by_location_for_both_index_shapes() {
        let sequence: Value =
            serde_yaml::from_str("sources:\n  - name: notes\n    path: sources/notes.md\n    review_state: approved\n")
                .unwrap();
        let mapping: Value =
            serde_yaml::from_str("sources:\n  sources/notes.md:\n    name: notes\n    review_state: approved\n")
                .unwrap();

        for index in [&sequence, &mapping] {
            let extras = source_extras_by_location(index);
            assert_eq!(
                extras.get("sources/notes.md"),
                Some(&r#"{"review_state":"approved"}"#.to_string()),
                "opaque fields must be retained regardless of legacy index shape"
            );
        }
    }

    #[test]
    fn project_entry_survives_a_full_merge_and_splice_round_trip() {
        let registration = sample_registration(Some(r#"{"custom_field": "keep me"}"#));
        let merged = merge_sources(None, std::slice::from_ref(&registration), &[]);
        let rendered = serde_yaml::to_string(&merged).unwrap();
        let block = format!("sources:\n{rendered}");
        let original = "project: alpha\nsources:\n  - name: placeholder\nterms:\n  - term: API\n";
        let spliced = splice_top_level_key(original, "sources", &block);
        assert!(
            spliced.contains("custom_field: keep me"),
            "the unrecognised field must survive a merge + splice write: {spliced:?}"
        );
        assert!(spliced.starts_with("project: alpha\n"), "untouched keys stay in place: {spliced:?}");
        assert!(spliced.ends_with("terms:\n  - term: API\n"), "untouched keys stay in place: {spliced:?}");
    }

    /// T030/FR-014/I-1: an existing YAML entry the store does not know about
    /// (divergence) is kept, not dropped — `merge_sources` has no removal
    /// path for it, only the caller-supplied `permitted_removals` allowlist.
    #[test]
    fn merge_sources_keeps_entries_the_store_does_not_know_about() {
        let existing: Value =
            serde_yaml::from_str("sources:\n  - name: orphan\n    path: sources/orphan.md\n").unwrap();
        let sources_value = existing.get("sources").cloned();

        let merged = merge_sources(sources_value.as_ref(), &[], &[]);
        let Value::Sequence(seq) = &merged else { panic!("expected a sequence: {merged:?}") };
        assert_eq!(seq.len(), 1, "an entry the store does not know about must be kept, not dropped: {merged:?}");
        assert_eq!(
            seq[0].get(name_key()),
            Some(&Value::String("orphan".to_string())),
            "the kept entry must be the untouched original: {merged:?}"
        );
    }

    /// T030/FR-014: the only way an unexplained-divergence entry disappears
    /// is the caller explicitly naming it in `permitted_removals` — because
    /// it just performed a real removal, not because the store failed to
    /// mention it.
    #[test]
    fn merge_sources_drops_only_names_the_caller_explicitly_permits() {
        let existing: Value = serde_yaml::from_str(
            "sources:\n  - name: orphan\n    path: sources/orphan.md\n  - name: removed\n    path: sources/removed.md\n",
        )
        .unwrap();
        let sources_value = existing.get("sources").cloned();

        let merged = merge_sources(sources_value.as_ref(), &[], &["removed".to_string()]);
        let Value::Sequence(seq) = &merged else { panic!("expected a sequence: {merged:?}") };
        let names: Vec<&str> = seq.iter().filter_map(|e| e.get(name_key()).and_then(|v| v.as_str())).collect();
        assert_eq!(names, vec!["orphan"], "only the explicitly permitted name may be dropped: {names:?}");
    }

    /// T030/FR-011: an entry present on both sides is reconciled by
    /// overwriting only the fields the store owns, while unrecognised
    /// fields survive from the existing entry — the store never
    /// wholesale-replaces a kept entry.
    #[test]
    fn merge_sources_reconciles_a_kept_entry_store_wins_owned_fields_extras_survive() {
        let existing: Value = serde_yaml::from_str(
            "sources:\n  - name: notes\n    path: sources/notes.md\n    kind: file\n    review_state: approved\n",
        )
        .unwrap();
        let sources_value = existing.get("sources").cloned();
        let mut registration = sample_registration(None);
        registration.source_identity = "notes".to_string();
        registration.registered_location = "sources/notes.md".to_string();

        let merged = merge_sources(sources_value.as_ref(), std::slice::from_ref(&registration), &[]);
        let Value::Sequence(seq) = &merged else { panic!("expected a sequence: {merged:?}") };
        assert_eq!(seq.len(), 1, "one name present on both sides must reconcile to one entry: {merged:?}");
        let entry = seq[0].as_mapping().unwrap();
        assert_eq!(
            entry.get(Value::String("kind".to_string())),
            Some(&Value::String(registration.source_type.clone())),
            "the store is authoritative for kind: {entry:?}"
        );
        assert_eq!(
            entry.get(Value::String("review_state".to_string())),
            Some(&Value::String("approved".to_string())),
            "a field the store does not own must survive reconciliation: {entry:?}"
        );
    }

    /// T030/FR-015: a registration the existing YAML has never seen is
    /// imported as a new entry, appended after the kept ones.
    #[test]
    fn merge_sources_imports_a_registration_absent_from_the_existing_yaml() {
        let registration = sample_registration(None);
        let merged = merge_sources(None, std::slice::from_ref(&registration), &[]);
        let Value::Sequence(seq) = &merged else { panic!("expected a sequence: {merged:?}") };
        assert_eq!(seq.len(), 1, "the new registration must be imported: {merged:?}");
        assert_eq!(seq[0].get(name_key()), Some(&Value::String("notes".to_string())));
    }

    #[test]
    fn export_requires_lance_primary() {
        let dir = tempfile::tempdir().unwrap();
        let proj = dir.path().join("projects").join("alpha");
        fs::create_dir_all(&proj).unwrap();
        fs::write(
            proj.join("mind-index.yaml"),
            "project: alpha\nsources:\n  - name: notes\n    kind: file\n    path: sources/notes.md\n",
        )
        .unwrap();

        assert!(export_project(dir.path(), "alpha", true).is_err());
    }

    // Spec 075 FR-013/I-2 regressions: the projection writer is confined to the
    // `sources:` block and must leave every byte outside it untouched.

    const INDEX_WITH_INNER_COMMENT: &str = concat!(
        "project: alpha\n",
        "sources:\n",
        "  - name: notes\n",
        "    path: sources/notes.md\n",
        "# hand-added note about the entries above\n",
        "  - name: other\n",
        "    path: sources/other.md\n",
        "\n",
        "terms:\n",
        "  - term: API\n",
    );

    #[test]
    fn key_span_spans_past_a_column_zero_comment_inside_the_block() {
        let (start, end) = top_level_key_span(INDEX_WITH_INNER_COMMENT, "sources").unwrap();
        let span = &INDEX_WITH_INNER_COMMENT[start..end];
        // A comment at column 0 does not end the block: everything down to the
        // next real top-level key belongs to `sources:`.
        assert!(span.starts_with("sources:\n"), "span was {span:?}");
        assert!(span.contains("name: other"), "span was {span:?}");
        assert!(!span.contains("terms:"), "span was {span:?}");
    }

    #[test]
    fn splice_over_a_block_with_an_inner_comment_leaves_no_leftover_tail() {
        let spliced = splice_top_level_key(
            INDEX_WITH_INNER_COMMENT,
            "sources",
            "sources:\n  - name: notes\n    path: sources/notes.md\n",
        );
        // The old block, comment and all, is gone rather than re-appended
        // after the freshly rendered one.
        assert_eq!(spliced.matches("sources:").count(), 1, "got {spliced:?}");
        assert!(!spliced.contains("name: other"), "got {spliced:?}");
        assert!(!spliced.contains("hand-added note"), "got {spliced:?}");
        // Untouched keys survive byte-identically, and the result still parses.
        assert!(spliced.starts_with("project: alpha\n"), "got {spliced:?}");
        assert!(spliced.ends_with("\nterms:\n  - term: API\n"), "got {spliced:?}");
        serde_yaml::from_str::<Value>(&spliced).expect("spliced index must stay parseable");
    }

    #[test]
    fn key_span_leaves_the_blank_line_before_the_next_key_outside() {
        let text = "sources:\n  - name: notes\n\nterms:\n  - term: API\n";
        let (_, end) = top_level_key_span(text, "sources").unwrap();
        assert_eq!(&text[end..], "\nterms:\n  - term: API\n");

        let spliced = splice_top_level_key(text, "sources", "sources:\n  - name: renamed\n");
        assert_eq!(spliced, "sources:\n  - name: renamed\n\nterms:\n  - term: API\n");
    }
}
