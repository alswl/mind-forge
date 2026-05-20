use std::collections::{BTreeMap, HashMap};

use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize};

use crate::defaults;

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

/// Banner presentation level for generated article output.
#[derive(Debug, Clone, JsonSchema, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BannerLevel {
    Note,
    Tip,
    Warning,
    Danger,
}

/// Optional banner injected into generated article output.
#[derive(Debug, Clone, JsonSchema, Serialize, Deserialize)]
pub struct BannerConfig {
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub level: Option<BannerLevel>,
}

/// Per-article build settings.
#[derive(Debug, Clone, Default, JsonSchema, Serialize, Deserialize)]
pub struct ArticleBuildConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_dir: Option<String>,
}

/// Build configuration for the project.
#[derive(Debug, Clone, JsonSchema, Serialize, Deserialize)]
pub struct BuildConfig {
    #[serde(default = "default_output_dir", deserialize_with = "deserialize_output_dir")]
    pub output_dir: String,
    #[serde(default)]
    pub merge_order: Vec<String>,
    #[serde(default = "default_build_format")]
    pub format: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub banner: Option<BannerConfig>,
    #[serde(default)]
    pub articles: HashMap<String, ArticleBuildConfig>,
}

impl Default for BuildConfig {
    fn default() -> Self {
        Self {
            output_dir: default_output_dir(),
            merge_order: Vec::new(),
            format: default_build_format(),
            banner: None,
            articles: HashMap::new(),
        }
    }
}

fn default_output_dir() -> String {
    defaults::BUILD_OUTPUT_DIR.to_string()
}

fn default_build_format() -> String {
    defaults::DEFAULT_BUILD_FORMAT.to_string()
}

fn deserialize_output_dir<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    Ok(Option::<String>::deserialize(deserializer)?.unwrap_or_else(default_output_dir))
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

/// Canonical layout configuration for project directories.
///
/// `articles` accepts `docs` as a compatibility alias. `build_output` accepts
/// `output_dir` as a compatibility alias.
#[derive(Debug, Clone, JsonSchema, Serialize, Deserialize)]
pub struct LayoutConfig {
    /// Project-relative path for articles (compat alias: `docs`).
    #[serde(default, alias = "docs")]
    pub articles: Option<String>,
    /// Project-relative path for source materials.
    #[serde(default)]
    pub sources: Option<String>,
    /// Project-relative path for assets.
    #[serde(default)]
    pub assets: Option<String>,
    /// Project-relative path for article-creation guide templates.
    #[serde(default)]
    pub templates: Option<String>,
    /// Project-relative path for build output (compat alias: `output_dir`).
    #[serde(default, alias = "output_dir")]
    pub build_output: Option<String>,
}

impl Default for LayoutConfig {
    fn default() -> Self {
        Self {
            articles: Some(defaults::LAYOUT_ARTICLES_DEFAULT.to_string()),
            sources: Some(defaults::LAYOUT_SOURCES_DEFAULT.to_string()),
            assets: Some(defaults::LAYOUT_ASSETS_DEFAULT.to_string()),
            templates: Some(defaults::LAYOUT_TEMPLATES_DEFAULT.to_string()),
            build_output: Some(defaults::LAYOUT_BUILD_OUTPUT_DEFAULT.to_string()),
        }
    }
}

/// Resolved effective layout with all values filled in.
#[derive(Debug, Clone, Serialize)]
pub struct EffectiveLayout {
    pub articles: String,
    pub sources: String,
    pub assets: String,
    pub templates: String,
    pub build_output: String,
}

/// Default paths for project directories (compatibility layer).
///
/// Accepted on read for backward compatibility with mind 0.3.0 projects.
/// `archive` is not a Layout category and remains here.
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
    defaults::DOCS_DIR.to_string()
}
fn default_sources() -> String {
    defaults::SOURCES_DIR.to_string()
}
fn default_assets() -> String {
    defaults::ASSETS_DIR.to_string()
}
fn default_archive() -> String {
    defaults::ARCHIVE_DIR.to_string()
}

fn default_schema_version() -> String {
    defaults::SCHEMA_VERSION.to_string()
}

/// Forward-compatible plugins configuration block.
///
/// Known plugin keys are typed; unknown keys are preserved round-trip for
/// future plugins.
#[derive(Debug, Clone, JsonSchema, Serialize, Deserialize)]
pub struct PluginsConfig {
    /// Configuration for the Typora front-matter plugin.
    #[serde(default, rename = "typora-front-matter")]
    pub typora_front_matter: Option<TyporaFrontMatterPluginConfig>,
    /// Unknown plugin keys, preserved for forward compatibility.
    #[serde(flatten)]
    #[schemars(skip)]
    pub extra: serde_yaml::Mapping,
}

impl Default for PluginsConfig {
    fn default() -> Self {
        Self {
            typora_front_matter: Some(TyporaFrontMatterPluginConfig {
                enabled: Some(true),
                extra: serde_yaml::Mapping::new(),
            }),
            extra: serde_yaml::Mapping::new(),
        }
    }
}

/// Configuration for the typora-front-matter plugin.
#[derive(Debug, Clone, JsonSchema, Serialize, Deserialize)]
pub struct TyporaFrontMatterPluginConfig {
    /// Whether Typora front-matter injection is enabled. Defaults to true when absent.
    #[serde(default)]
    pub enabled: Option<bool>,
    /// Unknown fields under this plugin, preserved for forward compatibility.
    #[serde(flatten)]
    #[schemars(skip)]
    pub extra: serde_yaml::Mapping,
}

/// Template mode: how the template pattern is interpreted.
#[derive(Debug, Clone, PartialEq, JsonSchema, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TemplateMode {
    Generated,
    #[serde(untagged)]
    Other(String),
}

/// A single template definition in mind.yaml.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Template {
    pub pattern: String,
    pub mode: TemplateMode,
    /// Extra unknown fields preserved verbatim (e.g. `cadence: daily`).
    #[serde(flatten)]
    pub extra: serde_yaml::Mapping,
}

/// Typed templates collection from mind.yaml.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Templates {
    #[serde(flatten)]
    pub items: BTreeMap<String, Template>,
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
    #[serde(skip_serializing)]
    pub paths: PathsConfig,
    #[serde(default)]
    pub layout: Option<LayoutConfig>,
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
    #[schemars(skip)]
    pub templates: Option<Templates>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plugins: Option<PluginsConfig>,
}

impl Default for MindConfig {
    fn default() -> Self {
        Self {
            schema_version: defaults::SCHEMA_VERSION.to_string(),
            project: None,
            build: BuildConfig::default(),
            publish: PublishConfig::default(),
            source: SourceConfig::default(),
            term: TermConfig::default(),
            paths: PathsConfig::default(),
            layout: Some(LayoutConfig::default()),
            name: None,
            description: None,
            created: None,
            updated: None,
            articles: None,
            templates: None,
            plugins: Some(PluginsConfig::default()),
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
    fn test_null_build_output_dir_defaults_to_outputs() {
        let config: MindConfig = serde_yaml::from_str("build:\n  output_dir:\n").unwrap();
        assert_eq!(config.build.output_dir, "outputs");
    }

    #[test]
    fn test_layout_canonical_populates_layout_field() {
        let config: MindConfig =
            serde_yaml::from_str("layout:\n  articles: notes\n  sources: refs\n  assets: media\n").unwrap();
        let layout = config.layout.as_ref().unwrap();
        assert_eq!(layout.articles.as_deref(), Some("notes"));
        assert_eq!(layout.sources.as_deref(), Some("refs"));
        assert_eq!(layout.assets.as_deref(), Some("media"));
    }

    #[test]
    fn test_layout_docs_compat_alias() {
        let config: MindConfig =
            serde_yaml::from_str("layout:\n  docs: notes\n  sources: refs\n  assets: media\n").unwrap();
        let layout = config.layout.as_ref().unwrap();
        // "docs" is an alias for "articles" in LayoutConfig
        assert_eq!(layout.articles.as_deref(), Some("notes"));
        assert_eq!(layout.sources.as_deref(), Some("refs"));
        assert_eq!(layout.assets.as_deref(), Some("media"));
    }

    #[test]
    fn test_paths_still_readable() {
        let config: MindConfig =
            serde_yaml::from_str("paths:\n  docs: notes\n  sources: refs\n  assets: media\n").unwrap();
        assert_eq!(config.paths.docs, "notes");
        assert_eq!(config.paths.sources, "refs");
        assert_eq!(config.paths.assets, "media");
        assert!(config.layout.is_none());
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

    // ── Typed Templates tests (T002) ──

    #[test]
    fn templates_round_trip_generated_mode() {
        let yaml = r#"
schema_version: '1'
templates:
  daily_report:
    pattern: "outputs/{date:YYYY-MM}/{date:YYYY-MM-DD}.md"
    mode: generated
"#;
        let config: MindConfig = serde_yaml::from_str(yaml).unwrap();
        let templates_ref = config.templates.as_ref().unwrap();
        let tmpl = templates_ref.items.get("daily_report").unwrap();
        assert_eq!(tmpl.pattern, "outputs/{date:YYYY-MM}/{date:YYYY-MM-DD}.md");
        assert!(matches!(tmpl.mode, TemplateMode::Generated));

        // Round-trip: reserialize and parse back
        let serialized = serde_yaml::to_string(&config).unwrap();
        let config2: MindConfig = serde_yaml::from_str(&serialized).unwrap();
        let templates_ref2 = config2.templates.as_ref().unwrap();
        let tmpl2 = templates_ref2.items.get("daily_report").unwrap();
        assert_eq!(tmpl2.pattern, tmpl.pattern);
        assert!(matches!(tmpl2.mode, TemplateMode::Generated));
    }

    #[test]
    fn templates_preserve_unknown_mode_as_other() {
        let yaml = r#"
schema_version: '1'
templates:
  custom:
    pattern: "outputs/{date:YYYY-MM-DD}.md"
    mode: custom_mode
"#;
        let config: MindConfig = serde_yaml::from_str(yaml).unwrap();
        let templates_ref = config.templates.as_ref().unwrap();
        let tmpl = templates_ref.items.get("custom").unwrap();
        assert!(matches!(tmpl.mode, TemplateMode::Other(_)));
        if let TemplateMode::Other(ref mode) = tmpl.mode {
            assert_eq!(mode, "custom_mode");
        }
    }

    #[test]
    fn templates_preserve_extra_fields_like_cadence() {
        let yaml = r#"
schema_version: '1'
templates:
  weekly:
    pattern: "outputs/{date:YYYY-MM-DD}.md"
    mode: generated
    cadence: weekly
"#;
        let config: MindConfig = serde_yaml::from_str(yaml).unwrap();
        let templates_ref = config.templates.as_ref().unwrap();
        let tmpl = templates_ref.items.get("weekly").unwrap();
        assert!(tmpl.extra.contains_key(serde_yaml::Value::String("cadence".to_string())));

        // Round-trip preserves cadence
        let serialized = serde_yaml::to_string(&config).unwrap();
        assert!(serialized.contains("cadence:"));
    }

    #[test]
    fn templates_absent_deserializes_as_none() {
        let yaml = r#"
schema_version: '1'
project:
  name: test
"#;
        let config: MindConfig = serde_yaml::from_str(yaml).unwrap();
        assert!(config.templates.is_none());
    }

    // ── T007: Plugins config tests ──

    #[test]
    fn plugins_missing_yields_none() {
        let config: MindConfig = serde_yaml::from_str("schema_version: '1'\n").unwrap();
        assert!(config.plugins.is_none());
    }

    #[test]
    fn plugins_absent_but_other_fields_present() {
        let yaml = "schema_version: '1'\nproject:\n  name: test\n";
        let config: MindConfig = serde_yaml::from_str(yaml).unwrap();
        assert!(config.plugins.is_none());
    }

    #[test]
    fn typora_front_matter_enabled_true() {
        let yaml = "schema_version: '1'\nplugins:\n  typora-front-matter:\n    enabled: true\n";
        let config: MindConfig = serde_yaml::from_str(yaml).unwrap();
        let plugins = config.plugins.unwrap();
        let tfm = plugins.typora_front_matter.unwrap();
        assert_eq!(tfm.enabled, Some(true));
    }

    #[test]
    fn typora_front_matter_enabled_false() {
        let yaml = "schema_version: '1'\nplugins:\n  typora-front-matter:\n    enabled: false\n";
        let config: MindConfig = serde_yaml::from_str(yaml).unwrap();
        let plugins = config.plugins.unwrap();
        let tfm = plugins.typora_front_matter.unwrap();
        assert_eq!(tfm.enabled, Some(false));
    }

    #[test]
    fn typora_front_matter_missing_enabled() {
        let yaml = "schema_version: '1'\nplugins:\n  typora-front-matter: {}\n";
        let config: MindConfig = serde_yaml::from_str(yaml).unwrap();
        let plugins = config.plugins.unwrap();
        let tfm = plugins.typora_front_matter.unwrap();
        assert_eq!(tfm.enabled, None);
    }

    #[test]
    fn plugins_unknown_plugin_key_preserved() {
        let yaml = "schema_version: '1'\nplugins:\n  some-other-plugin:\n    key: value\n";
        let config: MindConfig = serde_yaml::from_str(yaml).unwrap();
        let plugins = config.plugins.unwrap();
        assert!(plugins.typora_front_matter.is_none());
        // Unknown plugin key preserved in extra
        let serialized = serde_yaml::to_string(&plugins).unwrap();
        assert!(serialized.contains("some-other-plugin"));
    }

    #[test]
    fn typora_front_matter_unknown_field_preserved() {
        let yaml = "schema_version: '1'\nplugins:\n  typora-front-matter:\n    enabled: true\n    custom: keep-me\n";
        let config: MindConfig = serde_yaml::from_str(yaml).unwrap();
        let plugins = config.plugins.as_ref().unwrap();
        let tfm = plugins.typora_front_matter.as_ref().unwrap();
        assert_eq!(tfm.enabled, Some(true));
        let serialized = serde_yaml::to_string(&plugins).unwrap();
        assert!(serialized.contains("custom: keep-me"));
    }

    #[test]
    fn plugins_empty_block_yields_present() {
        let yaml = "schema_version: '1'\nplugins: {}\n";
        let config: MindConfig = serde_yaml::from_str(yaml).unwrap();
        assert!(config.plugins.is_some());
        let plugins = config.plugins.unwrap();
        assert!(plugins.typora_front_matter.is_none());
    }

    // ── T008: Invalid config shapes ──

    #[test]
    fn typora_front_matter_scalar_is_invalid() {
        let yaml = "schema_version: '1'\nplugins:\n  typora-front-matter: false\n";
        let result: Result<MindConfig, _> = serde_yaml::from_str(yaml);
        assert!(result.is_err(), "typora-front-matter as bool should be rejected");
    }

    #[test]
    fn typora_enabled_string_is_invalid() {
        let yaml = "schema_version: '1'\nplugins:\n  typora-front-matter:\n    enabled: \"yes\"\n";
        let result: Result<MindConfig, _> = serde_yaml::from_str(yaml);
        assert!(result.is_err(), "enabled as string should be rejected");
    }

    #[test]
    fn typora_enabled_number_is_invalid() {
        let yaml = "schema_version: '1'\nplugins:\n  typora-front-matter:\n    enabled: 1\n";
        let result: Result<MindConfig, _> = serde_yaml::from_str(yaml);
        assert!(result.is_err(), "enabled as integer should be rejected");
    }

    #[test]
    fn plugin_unknown_round_trips_through_mind_config() {
        let yaml = "schema_version: '1'\nplugins:\n  some-other-plugin:\n    key: value\n";
        let config: MindConfig = serde_yaml::from_str(yaml).unwrap();
        let serialized = serde_yaml::to_string(&config).unwrap();
        assert!(serialized.contains("some-other-plugin"));
    }
}
