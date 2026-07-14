use std::path::Path;

use crate::defaults;
use crate::error::Result;
use crate::model::index::IndexFile;
use crate::model::project::{MindYamlSummary, ProjectDetails};
use crate::service::config as config_svc;

/// Show project details: metadata, counts, and mind.yaml summary.
pub fn show(project_path: &Path, project_name: &str) -> Result<ProjectDetails> {
    let rel_path = format!("./{}", project_name);

    // Read mind-index.yaml for counts
    let index = match crate::service::index::load(project_path) {
        Ok(idx) => idx,
        Err(e) => {
            let detail = match &e {
                crate::error::MfError::ParseError { detail, .. } => format!(": {detail}"),
                _ => String::new(),
            };
            tracing::warn!("failed to load index for {}{detail}", project_path.display());
            IndexFile::create_default()
        }
    };

    let article_count = index.articles.as_ref().map(|v| v.len() as u64).unwrap_or(0);
    let source_count = index.sources.as_ref().map(|v| v.len() as u64).unwrap_or(0);
    let asset_count = index.assets.as_ref().map(|v| v.len() as u64).unwrap_or(0);

    // Compute last_active from max timestamp
    let mut last_active: Option<String> = None;
    for entry in index.articles.iter().flatten() {
        if last_active.as_ref().is_none_or(|m| &entry.updated_at > m) {
            last_active = Some(entry.updated_at.clone());
        }
    }
    for entry in index.assets.iter().flatten() {
        if last_active.as_ref().is_none_or(|m| &entry.added_at > m) {
            last_active = Some(entry.added_at.clone());
        }
    }
    for entry in index.sources.iter().flatten() {
        if last_active.as_ref().is_none_or(|m| &entry.updated_at > m) {
            last_active = Some(entry.updated_at.clone());
        }
    }

    // Read mind.yaml for summary
    let mind_yaml_path = project_path.join("mind.yaml");
    let mind_yaml_summary = if mind_yaml_path.exists() {
        match std::fs::read_to_string(&mind_yaml_path) {
            Ok(content) => {
                let parsed: serde_json::Value = serde_yaml::from_str(&content).unwrap_or_default();
                let schema_version = parsed
                    .get("schema_version")
                    .and_then(|v| v.as_str())
                    .unwrap_or(defaults::SCHEMA_VERSION)
                    .to_string();
                let types: Vec<String> = parsed
                    .get("types")
                    .and_then(|v| v.as_array())
                    .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                    .unwrap_or_default();
                let source_dirs: Vec<String> = parsed
                    .get("source_dirs")
                    .and_then(|v| v.as_array())
                    .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                    .unwrap_or_default();
                let assets_dir =
                    parsed.get("assets_dir").and_then(|v| v.as_str()).unwrap_or(defaults::ASSETS_DIR).to_string();
                Some(MindYamlSummary { schema_version, types, source_dirs, assets_dir })
            }
            Err(_) => None,
        }
    } else {
        None
    };

    let layout = Some(config_svc::effective_layout(project_path)?);

    Ok(ProjectDetails {
        name: project_name.to_string(),
        path: rel_path,
        article_count,
        source_count,
        asset_count,
        last_active,
        mind_yaml_summary,
        layout,
    })
}
