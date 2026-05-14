use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Project metadata for a mind-forge project.
#[derive(Debug, Clone, Default, JsonSchema, Serialize, Deserialize)]
pub struct ProjectMeta {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
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
    /// Recognized for loading/lint compatibility; full publish is out of scope.
    #[serde(rename = "yuque_cc")]
    YuqueCc,
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
    // -- Compatibility fields (Python mind 0.3.0 shapes) --
    // Accepted on read but not re-serialized; normalized to config in service layer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prefix: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub book_slug: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
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
        Self { output_dir: default_output_dir(), merge_order: Vec::new(), format: default_build_format() }
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
        Self { docs: default_docs(), sources: default_sources(), assets: default_assets(), archive: default_archive() }
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

fn default_schema_version() -> String {
    "1".to_string()
}

/// Top-level configuration for a mind-forge project (mind.yaml schema).
#[derive(Debug, Clone, JsonSchema, Serialize, Deserialize)]
pub struct MindConfig {
    #[serde(alias = "schema", default = "default_schema_version")]
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
    // -- Compatibility top-level fields (Python mind 0.3.0) --
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub articles: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub templates: Option<serde_json::Value>,
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
            name: None,
            description: None,
            created: None,
            updated: None,
            articles: None,
            templates: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn yaml_for(t: PublishTargetType) -> String {
        let target = PublishTarget {
            name: "x".to_string(),
            target_type: t,
            enabled: true,
            config: None,
            path: None,
            prefix: None,
            book_slug: None,
            namespace: None,
        };
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

    // -- Compatibility tests (T027/T028) --

    #[test]
    fn test_empty_mind_yaml_defaults() {
        let config: MindConfig = serde_yaml::from_str("schema_version: '1'\n").unwrap();
        assert_eq!(config.schema_version, "1");
        assert!(config.project.is_none());
        assert!(config.name.is_none());
    }

    #[test]
    fn test_schema_alias() {
        let config: MindConfig = serde_yaml::from_str("schema: '1'\n").unwrap();
        assert_eq!(config.schema_version, "1");
    }

    #[test]
    fn test_missing_schema_version_defaults_to_1() {
        let config: MindConfig = serde_yaml::from_str("name: test\n").unwrap();
        assert_eq!(config.schema_version, "1");
    }

    #[test]
    fn test_top_level_name_and_description() {
        let yaml = r#"
schema_version: '1'
name: My Project
description: A test project
created: 2026-01-01
updated: 2026-05-10
"#;
        let config: MindConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.name.as_deref(), Some("My Project"));
        assert_eq!(config.description.as_deref(), Some("A test project"));
        assert_eq!(config.created.as_deref(), Some("2026-01-01"));
        assert_eq!(config.updated.as_deref(), Some("2026-05-10"));
    }

    #[test]
    fn test_top_level_articles_with_null_value() {
        let yaml = r#"
schema_version: '1'
articles:
  weekly-summary:
    type: blog
  null-entry:
"#;
        let config: MindConfig = serde_yaml::from_str(yaml).unwrap();
        assert!(config.articles.is_some());
    }

    #[test]
    fn test_local_publish_target_with_direct_path() {
        let yaml = r#"
schema_version: '1'
publish:
  targets:
    - name: local-pdf
      type: local
      path: ./output
      prefix: reports-
"#;
        let target: MindConfig = serde_yaml::from_str(yaml).unwrap();
        let targets = target.publish.targets.unwrap();
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].name, "local-pdf");
        assert!(matches!(targets[0].target_type, PublishTargetType::Local));
        assert_eq!(targets[0].path.as_deref(), Some("./output"));
        assert_eq!(targets[0].prefix.as_deref(), Some("reports-"));
    }

    #[test]
    fn test_yuque_cc_publish_target() {
        let yaml = r#"
schema_version: '1'
publish:
  targets:
    - name: yuque-tech
      type: yuque_cc
      book_slug: tech-blog
      namespace: engineering
"#;
        let config: MindConfig = serde_yaml::from_str(yaml).unwrap();
        let targets = config.publish.targets.unwrap();
        assert_eq!(targets.len(), 1);
        assert!(matches!(targets[0].target_type, PublishTargetType::YuqueCc));
        assert_eq!(targets[0].book_slug.as_deref(), Some("tech-blog"));
        assert_eq!(targets[0].namespace.as_deref(), Some("engineering"));
    }

    #[test]
    fn test_unknown_top_level_fields_ignored() {
        let yaml = r#"
schema_version: '1'
unknown_field: value
another_unknown: 42
"#;
        let config: MindConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.schema_version, "1");
        // These fields are just silently accepted
    }

    #[test]
    fn target_type_github_pages_round_trips_snake_case() {
        let yaml = "name: x\ntype: github_pages\nenabled: true\n";
        let target: PublishTarget = serde_yaml::from_str(yaml).unwrap();
        assert!(matches!(target.target_type, PublishTargetType::GithubPages));
    }
}
