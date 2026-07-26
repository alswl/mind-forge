//! Legacy compatibility projections: export Lance primary registrations to
//! project `mind-index.yaml.sources`, compute projection fingerprints,
//! and detect drift.
//!
//! In Lance mode, every successful registration mutation publishes to the
//! primary Lance table first, then best-effort updates the owning project's
//! legacy YAML. Projection failure reports degraded state without changing
//! the primary operation's exit code.

use std::fs;
use std::path::Path;

use serde::Serialize;

use crate::error::{MfError, Result};
use crate::model::source_advanced::ProjectionStatus;

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
pub fn export_project(repo_root: &Path, project_name: &str, dry_run: bool) -> Result<ProjectionComparison> {
    if project_name == "all" {
        let projects_dir = repo_root.join("projects");
        let mut comparisons = Vec::new();
        if projects_dir.exists() {
            for entry in fs::read_dir(&projects_dir)? {
                let entry = entry?;
                if entry.file_type()?.is_dir() {
                    comparisons.push(export_project(repo_root, &entry.file_name().to_string_lossy(), dry_run)?);
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
    let mut projected = Vec::with_capacity(primary_count);
    for registration in registrations {
        let mut source = serde_yaml::Mapping::new();
        source.insert("name".into(), serde_yaml::Value::String(registration.source_identity));
        source.insert("kind".into(), serde_yaml::Value::String(registration.source_type));
        if registration.registered_location.starts_with("http://")
            || registration.registered_location.starts_with("https://")
        {
            source.insert("url".into(), serde_yaml::Value::String(registration.registered_location));
        } else {
            source.insert("path".into(), serde_yaml::Value::String(registration.registered_location));
        }
        if let Some(source_kind) = registration.source_kind {
            source.insert("source_kind".into(), serde_yaml::Value::String(source_kind));
        }
        let tags: Vec<String> = serde_json::from_str(&registration.tags_json).unwrap_or_default();
        if !tags.is_empty() {
            source.insert(
                "tags".into(),
                serde_yaml::Value::Sequence(tags.into_iter().map(serde_yaml::Value::String).collect()),
            );
        }
        projected.push(serde_yaml::Value::Mapping(source));
    }
    if let serde_yaml::Value::Mapping(ref mut root) = legacy {
        root.insert("sources".into(), serde_yaml::Value::Sequence(projected));
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
