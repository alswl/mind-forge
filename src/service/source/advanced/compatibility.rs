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

use super::catalog::CatalogRegistration;

fn name_key() -> Value {
    Value::String("name".to_string())
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

/// Merge store registrations into the existing `sources:` value. Returns the
/// merged sequence and the number of entries kept only because the store
/// does not (yet) know about them — divergence that resolves by import, never
/// by deletion (spec 075 I-1), except for names the caller explicitly names
/// in `permitted_removals` because it just performed a real removal.
fn merge_sources(
    sources_value: Option<&Value>,
    registrations: &[CatalogRegistration],
    permitted_removals: &[String],
) -> (Vec<Value>, usize) {
    let (order, by_name) = read_existing_sources(sources_value);
    let store_names: HashSet<&str> = registrations.iter().map(|r| r.source_identity.as_str()).collect();

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

    let kept_yaml_only = order.iter().filter(|name| !store_names.contains(name.as_str())).count();
    (result, kept_yaml_only)
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
    let mut legacy: serde_yaml::Value = serde_yaml::from_str(&yaml_data)
        .map_err(|e| MfError::advanced_store(format!("cannot parse legacy index: {e}"), None))?;

    let legacy_count = legacy.get("sources").and_then(|s| s.as_sequence()).map(|s| s.len()).unwrap_or(0);
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
    let existing_sources = legacy.get("sources").cloned();
    // FR-014/I-1: entries with no store row yet are kept, not dropped — `mf
    // source index` is what imports them into the store.
    let (merged, _kept_yaml_only) = merge_sources(existing_sources.as_ref(), &registrations, permitted_removals);
    if let serde_yaml::Value::Mapping(ref mut root) = legacy {
        root.insert("sources".into(), serde_yaml::Value::Sequence(merged));
    }
    let rendered = serde_yaml::to_string(&legacy)
        .map_err(|e| MfError::advanced_store(format!("cannot serialize legacy projection: {e}"), None))?;
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
    let tmp = index_path.with_extension("yaml.tmp");
    fs::write(&tmp, &rendered)?;
    fs::rename(&tmp, &index_path)?;
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
}
