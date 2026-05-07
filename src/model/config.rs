use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Project metadata for a mind-forge project.
#[derive(Debug, Clone, Default, JsonSchema, Serialize, Deserialize)]
pub struct ProjectMeta {
    pub name: String,
    pub created_at: Option<String>,
}

/// Publish target type: where to publish content.
#[derive(Debug, Clone, JsonSchema, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PublishTargetType {
    Local,
    #[serde(rename = "yuque-prompt")]
    YuquePrompt,
    Yuque,
    GithubPages,
    Custom,
}

/// A single publish target definition.
#[derive(Debug, Clone, JsonSchema, Serialize, Deserialize)]
pub struct PublishTarget {
    pub name: String,
    #[serde(rename = "type")]
    pub target_type: PublishTargetType,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config: Option<serde_json::Value>,
}

fn default_enabled() -> bool {
    true
}

/// Publish configuration for the project.
#[derive(Debug, Clone, Default, JsonSchema, Serialize, Deserialize)]
pub struct PublishConfig {
    pub default_target: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub targets: Option<Vec<PublishTarget>>,
}

/// Build configuration for the project.
#[derive(Debug, Clone, JsonSchema, Serialize, Deserialize)]
pub struct BuildConfig {
    #[serde(default = "default_output_dir")]
    pub output_dir: String,
    #[serde(default)]
    pub merge_order: Vec<String>,
    #[serde(default = "default_build_format")]
    pub format: String,
}

impl Default for BuildConfig {
    fn default() -> Self {
        Self {
            output_dir: default_output_dir(),
            merge_order: Vec::new(),
            format: default_build_format(),
        }
    }
}

fn default_output_dir() -> String {
    "_build".to_string()
}

fn default_build_format() -> String {
    "md".to_string()
}

/// Source scanning configuration.
#[derive(Debug, Clone, Default, JsonSchema, Serialize, Deserialize)]
pub struct SourceConfig {
    #[serde(default)]
    pub scan_paths: Vec<String>,
    #[serde(default)]
    pub types: Vec<String>,
}

/// Terminology checking configuration.
#[derive(Debug, Clone, JsonSchema, Serialize, Deserialize)]
pub struct TermConfig {
    #[serde(default = "default_term_enabled")]
    pub enabled: bool,
    #[serde(default)]
    pub case_sensitive: bool,
}

impl Default for TermConfig {
    fn default() -> Self {
        Self { enabled: true, case_sensitive: false }
    }
}

fn default_term_enabled() -> bool {
    true
}

/// Default paths for project directories.
#[derive(Debug, Clone, JsonSchema, Serialize, Deserialize)]
pub struct PathsConfig {
    #[serde(default = "default_docs")]
    pub docs: String,
    #[serde(default = "default_sources")]
    pub sources: String,
    #[serde(default = "default_assets")]
    pub assets: String,
    #[serde(default = "default_archive")]
    pub archive: String,
}

impl Default for PathsConfig {
    fn default() -> Self {
        Self {
            docs: default_docs(),
            sources: default_sources(),
            assets: default_assets(),
            archive: default_archive(),
        }
    }
}

fn default_docs() -> String {
    "docs".to_string()
}
fn default_sources() -> String {
    "sources".to_string()
}
fn default_assets() -> String {
    "assets".to_string()
}
fn default_archive() -> String {
    "_archived".to_string()
}

/// Top-level configuration for a mind-forge project (mind.yaml schema).
#[derive(Debug, Clone, JsonSchema, Serialize, Deserialize)]
pub struct MindConfig {
    pub schema_version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project: Option<ProjectMeta>,
    #[serde(default)]
    pub build: BuildConfig,
    #[serde(default)]
    pub publish: PublishConfig,
    #[serde(default)]
    pub source: SourceConfig,
    #[serde(default)]
    pub term: TermConfig,
    #[serde(default)]
    pub paths: PathsConfig,
}

impl Default for MindConfig {
    fn default() -> Self {
        Self {
            schema_version: "1".to_string(),
            project: None,
            build: BuildConfig::default(),
            publish: PublishConfig::default(),
            source: SourceConfig::default(),
            term: TermConfig::default(),
            paths: PathsConfig::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn yaml_for(t: PublishTargetType) -> String {
        let target =
            PublishTarget { name: "x".to_string(), target_type: t, enabled: true, config: None };
        serde_yaml::to_string(&target).unwrap()
    }

    #[test]
    fn target_type_local_serializes_snake_case() {
        assert!(yaml_for(PublishTargetType::Local).contains("type: local"));
    }

    #[test]
    fn target_type_yuque_prompt_uses_kebab_case() {
        assert!(yaml_for(PublishTargetType::YuquePrompt).contains("type: yuque-prompt"));
    }

    #[test]
    fn target_type_yuque_serializes_snake_case() {
        assert!(yaml_for(PublishTargetType::Yuque).contains("type: yuque"));
    }

    #[test]
    fn target_type_github_pages_keeps_snake_case() {
        assert!(yaml_for(PublishTargetType::GithubPages).contains("type: github_pages"));
    }

    #[test]
    fn target_type_custom_serializes_snake_case() {
        assert!(yaml_for(PublishTargetType::Custom).contains("type: custom"));
    }

    #[test]
    fn target_type_yuque_prompt_round_trips() {
        let yaml = "name: x\ntype: yuque-prompt\nenabled: true\n";
        let target: PublishTarget = serde_yaml::from_str(yaml).unwrap();
        assert!(matches!(target.target_type, PublishTargetType::YuquePrompt));
    }

    #[test]
    fn target_type_github_pages_round_trips_snake_case() {
        let yaml = "name: x\ntype: github_pages\nenabled: true\n";
        let target: PublishTarget = serde_yaml::from_str(yaml).unwrap();
        assert!(matches!(target.target_type, PublishTargetType::GithubPages));
    }
}
