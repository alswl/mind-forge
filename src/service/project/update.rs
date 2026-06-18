use std::path::Path;

use serde::Serialize;

use crate::error::{MfError, Result};
use crate::service::{repo, util};

#[derive(Debug, Clone)]
pub struct ProjectUpdate<'a> {
    pub name: &'a str,
    pub description: Option<&'a str>,
    pub clear_description: bool,
    pub dry_run: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct ProjectUpdateReport {
    pub name: String,
    pub path: String,
    pub dry_run: bool,
    pub changes: serde_json::Value,
}

pub fn update_project(repo_root: &Path, update: ProjectUpdate<'_>) -> Result<ProjectUpdateReport> {
    if update.description.is_none() && !update.clear_description {
        return Err(MfError::usage(
            "nothing to update: use --description or --clear-description",
            Some("pass --description <TEXT> or --clear-description".to_string()),
        ));
    }
    if update.description.is_some() && update.clear_description {
        return Err(MfError::usage("--description and --clear-description are mutually exclusive", None));
    }

    let projects_dir = repo::projects_dir_for(repo_root)?;
    let project_path = util::project_dir_for(repo_root, &projects_dir, update.name);
    if !project_path.join("mind.yaml").exists() {
        return Err(MfError::usage(
            format!("project '{}' not found in Mind Repo", update.name),
            Some("use `mf project list` to see available projects".to_string()),
        ));
    }

    let mind_path = project_path.join("mind.yaml");
    let current = std::fs::read_to_string(&mind_path).map_err(MfError::Io)?;
    let mut doc = parse_mind_yaml(&current, &mind_path)?;
    let old_description = project_description(&doc);

    let mut changes = serde_json::Map::new();
    if let Some(description) = update.description {
        changes.insert("description".to_string(), serde_json::json!({"from": old_description, "to": description}));
    } else if update.clear_description {
        changes.insert("description".to_string(), serde_json::json!({"from": old_description, "to": null}));
    }

    if !update.dry_run {
        apply_description(&mut doc, update.name, update.description, update.clear_description);
        let yaml = serde_yaml::to_string(&doc)
            .map_err(|e| MfError::Internal(anyhow::anyhow!("serialize {}: {e}", mind_path.display())))?;
        util::atomic_write(&mind_path, &yaml)?;
    }

    Ok(ProjectUpdateReport {
        name: update.name.to_string(),
        path: project_path.strip_prefix(repo_root).unwrap_or(&project_path).to_string_lossy().replace('\\', "/"),
        dry_run: update.dry_run,
        changes: serde_json::Value::Object(changes),
    })
}

fn parse_mind_yaml(content: &str, path: &Path) -> Result<serde_yaml::Value> {
    if content.trim().is_empty() {
        return Ok(serde_yaml::Value::Mapping(serde_yaml::Mapping::new()));
    }
    serde_yaml::from_str(content).map_err(|e| MfError::ParseError {
        kind: "yaml".to_string(),
        path: path.to_path_buf(),
        detail: e.to_string(),
    })
}

fn project_description(doc: &serde_yaml::Value) -> Option<String> {
    doc.get("project")
        .and_then(|project| project.get("description"))
        .and_then(|value| value.as_str())
        .map(ToString::to_string)
        .or_else(|| doc.get("description").and_then(|value| value.as_str()).map(ToString::to_string))
}

fn apply_description(
    doc: &mut serde_yaml::Value,
    project_name: &str,
    description: Option<&str>,
    clear_description: bool,
) {
    if !matches!(doc, serde_yaml::Value::Mapping(_)) {
        *doc = serde_yaml::Value::Mapping(serde_yaml::Mapping::new());
    }
    let serde_yaml::Value::Mapping(root) = doc else {
        return;
    };

    root.entry(serde_yaml::Value::String("schema_version".to_string()))
        .or_insert_with(|| serde_yaml::Value::String(crate::defaults::SCHEMA_VERSION.to_string()));

    let project_key = serde_yaml::Value::String("project".to_string());
    let project = root.entry(project_key).or_insert_with(|| serde_yaml::Value::Mapping(serde_yaml::Mapping::new()));
    if !matches!(project, serde_yaml::Value::Mapping(_)) {
        *project = serde_yaml::Value::Mapping(serde_yaml::Mapping::new());
    }
    let serde_yaml::Value::Mapping(project_map) = project else {
        return;
    };

    project_map
        .entry(serde_yaml::Value::String("name".to_string()))
        .or_insert_with(|| serde_yaml::Value::String(project_name.to_string()));

    let description_key = serde_yaml::Value::String("description".to_string());
    if clear_description {
        project_map.remove(&description_key);
    } else if let Some(description) = description {
        project_map.insert(description_key, serde_yaml::Value::String(description.to_string()));
    }
}
