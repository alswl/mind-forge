//! Config service: load, merge, schema generation, and atomic write.
//!
//! This module implements the business logic for `mf config {schema,show}`.
//! It strictly separates from `src/cli/` (dispatch + formatting) and `src/model/` (pure data).

use std::fs;
use std::path::{Component, Path, PathBuf};

use schemars::schema_for;
use serde::Serialize;

use crate::defaults;
use crate::error::{MfError, Result};
use crate::model::config::{EffectiveLayout, LayoutConfig, MindConfig, PathsConfig, ProjectMeta};
use crate::service::util;

// ---------------------------------------------------------------------------
// Merge
// ---------------------------------------------------------------------------

/// Merge two `MindConfig` values: base (layer 0) is overwritten by overlay (layer 1).
///
/// Rules:
/// - Scalar fields: overlay value wins (base as fallback)
/// - Object fields: shallow merge (field-by-field within each sub-struct)
/// - Array fields: full replacement by overlay
pub fn merge(base: MindConfig, overlay: MindConfig) -> MindConfig {
    MindConfig {
        schema_version: overlay.schema_version,
        project: overlay.project.or(base.project),
        // BuildConfig, SourceConfig, TermConfig, PathsConfig have no optional
        // fields so "merge" = full replacement by overlay.
        build: overlay.build,
        publish: merge_publish(base.publish, overlay.publish),
        source: overlay.source,
        term: overlay.term,
        paths: overlay.paths,
        layout: overlay.layout,
        // Compatibility fields: overlay wins, fall back to base
        name: overlay.name.or(base.name),
        description: overlay.description.or(base.description),
        created: overlay.created.or(base.created),
        updated: overlay.updated.or(base.updated),
        articles: overlay.articles.or(base.articles),
        templates: overlay.templates.or(base.templates),
        plugins: overlay.plugins.or(base.plugins),
    }
}

fn merge_publish(
    base: crate::model::config::PublishConfig,
    overlay: crate::model::config::PublishConfig,
) -> crate::model::config::PublishConfig {
    crate::model::config::PublishConfig {
        default_target: overlay.default_target.or(base.default_target),
        targets: overlay.targets.or(base.targets),
    }
}

// ---------------------------------------------------------------------------
// Load project-level config (mind.yaml)
// ---------------------------------------------------------------------------

/// Load `mind.yaml` by walking up from `cwd`, stopping at `repo_root` boundary.
///
/// Returns `Ok(None)` when no `mind.yaml` is found (layer absent).
/// Returns parse errors for malformed YAML or incompatible schema_version.
///
/// **Compatibility**: Empty files return default config. Top-level `name`,
/// `description`, `articles`, `created`, `updated` fields are accepted and
/// normalized into the canonical nested structure. Project name is inferred
/// from the containing directory when absent.
pub fn load_project(cwd: &Path, repo_root: Option<&Path>) -> Result<Option<MindConfig>> {
    let mind_path = find_mind_yaml(cwd, repo_root)?;
    match mind_path {
        None => Ok(None),
        Some(path) => {
            let content = fs::read_to_string(&path).map_err(MfError::Io)?;
            if content.trim().is_empty() {
                // Empty file returns default config
                return Ok(Some(MindConfig {
                    schema_version: defaults::SCHEMA_VERSION.to_string(),
                    ..MindConfig::default()
                }));
            }
            let mut config: MindConfig = serde_yaml::from_str(&content).map_err(|e| MfError::ParseError {
                kind: "yaml".to_string(),
                path: path.clone(),
                detail: e.to_string(),
            })?;
            // Schema version fallback: missing → default "1"
            if config.schema_version.is_empty() {
                config.schema_version = defaults::SCHEMA_VERSION.to_string();
            }
            util::validate_schema_version(&config.schema_version, &path)?;

            // Normalize publish targets: move compatible top-level `path`/`prefix`
            // into `config.path`/`config.prefix` when config is absent.
            if let Some(ref mut targets) = config.publish.targets {
                for target in targets.iter_mut() {
                    if target.config.is_none() {
                        let mut cfg = serde_json::Map::new();
                        if let Some(ref p) = target.path {
                            cfg.insert("path".to_string(), serde_json::Value::String(p.clone()));
                        }
                        if let Some(ref p) = target.prefix {
                            cfg.insert("prefix".to_string(), serde_json::Value::String(p.clone()));
                        }
                        if let Some(ref bs) = target.book_slug {
                            cfg.insert("book_slug".to_string(), serde_json::Value::String(bs.clone()));
                        }
                        if let Some(ref ns) = target.namespace {
                            cfg.insert("namespace".to_string(), serde_json::Value::String(ns.clone()));
                        }
                        if !cfg.is_empty() {
                            target.config = Some(serde_json::Value::Object(cfg));
                        }
                    }
                }
            }

            // Validate template names (US2, T025)
            validate_template_names(&config)?;

            // Normalize top-level compatibility fields into canonical project metadata
            if config.project.is_none() {
                let inferred_name = path
                    .parent()
                    .and_then(|p| p.file_name())
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_default();
                let meta = ProjectMeta {
                    name: config.name.clone().unwrap_or(inferred_name),
                    description: config.description.clone(),
                    created_at: config.created.clone(),
                };
                config.project = Some(meta);
            } else if let Some(ref mut meta) = config.project {
                if meta.name.is_empty() {
                    let inferred_name = path
                        .parent()
                        .and_then(|p| p.file_name())
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_default();
                    meta.name = config.name.clone().unwrap_or(inferred_name);
                }
                if meta.description.is_none() {
                    meta.description = config.description.clone();
                }
            }

            Ok(Some(config))
        }
    }
}

#[allow(dead_code)]
pub fn project_paths(project_path: &Path) -> Result<PathsConfig> {
    Ok(load_project(project_path, Some(project_path))?.map(|config| config.paths).unwrap_or_default())
}

/// Load project config, resolve effective layout with defaults and compat
/// mapping, and validate the result. Returns the resolved layout.
pub fn effective_layout(project_path: &Path) -> Result<EffectiveLayout> {
    let mut config = load_project(project_path, Some(project_path))?.unwrap_or_else(MindConfig::default);
    resolve_and_validate(project_path, &mut config)
}

// ---------------------------------------------------------------------------
// Effective layout resolution
// ---------------------------------------------------------------------------

/// Resolve the effective layout from both canonical `layout` and
/// compatibility `paths`/`build.output_dir` fields.
///
/// Precedence (highest wins):
/// 1. Canonical `layout.<category>`
/// 2. Historical `paths.<category>` or `build.output_dir`
/// 3. Default values
///
/// Returns the resolved layout and an optional conflict warning message.
pub fn resolve_effective_layout(config: &MindConfig) -> (EffectiveLayout, Option<String>) {
    let defaults = LayoutConfig::default();
    let mut conflicts: Vec<String> = Vec::new();

    let layout = config.layout.as_ref();
    let articles = resolve_layout_value(
        &mut conflicts,
        "layout.articles",
        layout.and_then(|l| l.articles.as_deref()),
        "paths.docs",
        &config.paths.docs,
        defaults.articles.as_deref().unwrap_or(defaults::LAYOUT_ARTICLES_DEFAULT),
    );
    let sources = resolve_layout_value(
        &mut conflicts,
        "layout.sources",
        layout.and_then(|l| l.sources.as_deref()),
        "paths.sources",
        &config.paths.sources,
        defaults.sources.as_deref().unwrap_or(defaults::LAYOUT_SOURCES_DEFAULT),
    );
    let assets = resolve_layout_value(
        &mut conflicts,
        "layout.assets",
        layout.and_then(|l| l.assets.as_deref()),
        "paths.assets",
        &config.paths.assets,
        defaults.assets.as_deref().unwrap_or(defaults::LAYOUT_ASSETS_DEFAULT),
    );
    let templates = layout
        .and_then(|l| l.templates.clone())
        .unwrap_or_else(|| defaults.templates.unwrap_or_else(|| defaults::LAYOUT_TEMPLATES_DEFAULT.to_string()));
    let build_output = resolve_layout_value(
        &mut conflicts,
        "layout.build_output",
        layout.and_then(|l| l.build_output.as_deref()),
        "build.output_dir",
        &config.build.output_dir,
        defaults::LAYOUT_BUILD_OUTPUT_DEFAULT,
    );

    let conflict_msg = if conflicts.is_empty() { None } else { Some(conflicts.join("; ")) };

    (EffectiveLayout { articles, sources, assets, templates, build_output }, conflict_msg)
}

fn resolve_layout_value(
    conflicts: &mut Vec<String>,
    canonical_name: &str,
    canonical_value: Option<&str>,
    compat_name: &str,
    compat_value: &str,
    default_value: &str,
) -> String {
    match canonical_value {
        Some(value) => {
            if value != compat_value && compat_value != default_value {
                conflicts.push(format!("{canonical_name}=\"{value}\" overrides {compat_name}=\"{compat_value}\""));
            }
            value.to_string()
        }
        None => compat_value.to_string(),
    }
}

/// Resolve effective layout and update the config's `layout` field in-place
/// so serialization emits the correct canonical values.
pub fn apply_effective_layout(config: &mut MindConfig) -> EffectiveLayout {
    let (effective, _conflict) = resolve_effective_layout(config);
    config.layout = Some(LayoutConfig {
        articles: Some(effective.articles.clone()),
        sources: Some(effective.sources.clone()),
        assets: Some(effective.assets.clone()),
        templates: Some(effective.templates.clone()),
        build_output: Some(effective.build_output.clone()),
    });
    effective
}

// ---------------------------------------------------------------------------
// Layout validation
// ---------------------------------------------------------------------------

/// Validate a layout value against all safety rules.
///
/// Returns `Ok(())` when the value is safe to use as a project-relative
/// directory path. Returns a usage error naming the category, value, and
/// the reason when validation fails.
pub fn validate_layout(project_root: &Path, effective: &EffectiveLayout) -> Result<()> {
    let categories = layout_entries(effective);
    let canonical_root = project_root.canonicalize().unwrap_or_else(|_| project_root.to_path_buf());

    // 1. Empty or whitespace-only values
    for (name, value) in &categories {
        if value.trim().is_empty() {
            return Err(MfError::usage(
                format!("{name} is empty or whitespace-only"),
                Some(format!("provide a project-relative directory name, e.g. {name}: my_dir")),
            ));
        }
    }

    // 2. Absolute paths
    for (name, value) in &categories {
        if Path::new(value).is_absolute() {
            return Err(MfError::usage(
                format!("{name} is an absolute path: '{value}'"),
                Some("use a project-relative directory name without a leading /".to_string()),
            ));
        }
    }

    // 3. Project-boundary escapes and file-backed paths
    for (name, value) in &categories {
        let normalized = normalize_relative_layout_path(value).ok_or_else(|| {
            MfError::usage(
                format!("{name} escapes the project root: '{value}'"),
                Some("use a directory name that stays inside the project".to_string()),
            )
        })?;
        let full_path = project_root.join(&normalized);

        if let Ok(canonical_full) = full_path.canonicalize() {
            if canonical_full.strip_prefix(&canonical_root).is_err() {
                return Err(MfError::usage(
                    format!("{name} escapes the project root: '{value}'"),
                    Some("use a directory name that stays inside the project".to_string()),
                ));
            }
        }

        // 4. Existing regular file
        if full_path.exists() && full_path.is_file() {
            return Err(MfError::usage(
                format!("{name} points to an existing file: '{value}'"),
                Some("remove the file or choose a different directory name".to_string()),
            ));
        }
    }

    // 5. Duplicate normalized paths
    let mut seen: std::collections::HashMap<PathBuf, &str> = std::collections::HashMap::new();
    for (name, value) in &categories {
        let normalized = normalize_relative_layout_path(value)
            .expect("layout paths were already normalized during boundary validation");
        if let Some(existing) = seen.get(&normalized) {
            return Err(MfError::usage(
                format!("{name} and {existing} resolve to the same path: '{value}'"),
                Some("each layout category must use a unique directory name".to_string()),
            ));
        }
        seen.insert(normalized, name);
    }

    Ok(())
}

fn layout_entries(effective: &EffectiveLayout) -> Vec<(&'static str, &str)> {
    vec![
        ("layout.articles", effective.articles.as_str()),
        ("layout.sources", effective.sources.as_str()),
        ("layout.assets", effective.assets.as_str()),
        ("layout.templates", effective.templates.as_str()),
        ("layout.build_output", effective.build_output.as_str()),
    ]
}

fn normalize_relative_layout_path(value: &str) -> Option<PathBuf> {
    let mut normalized = PathBuf::new();
    for component in Path::new(value.trim()).components() {
        match component {
            Component::CurDir => {}
            Component::Normal(segment) => normalized.push(segment),
            Component::ParentDir => {
                if !normalized.pop() {
                    return None;
                }
            }
            Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    (!normalized.as_os_str().is_empty()).then_some(normalized)
}

/// Validate and apply effective layout, returning the validated layout
/// or a usage error.
pub fn resolve_and_validate(project_root: &Path, config: &mut MindConfig) -> Result<EffectiveLayout> {
    let effective = apply_effective_layout(config);
    validate_layout(project_root, &effective)?;
    Ok(effective)
}

/// Validate new config fields after loading.
///
/// Returns an error for invalid configurations such as:
/// - Banner present but text is empty
pub fn validate_new_fields(config: &MindConfig) -> Result<()> {
    if let Some(banner) = &config.build.banner {
        if banner.text.trim().is_empty() {
            return Err(crate::error::MfError::usage(
                "build.banner.text must be non-empty when banner is configured".to_string(),
                Some("provide banner text or remove the banner section".to_string()),
            ));
        }
    }
    Ok(())
}

/// Validate that all template names match `^[a-z][a-z0-9_]*$` (T025).
fn validate_template_names(config: &MindConfig) -> Result<()> {
    let templates = match config.templates.as_ref() {
        Some(t) => &t.items,
        None => return Ok(()),
    };
    for name in templates.keys() {
        if name.is_empty()
            || !name.starts_with(|c: char| c.is_ascii_lowercase())
            || !name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        {
            return Err(MfError::InvalidTemplateName { name: name.clone() });
        }
    }
    Ok(())
}

fn find_mind_yaml(start: &Path, repo_root: Option<&Path>) -> Result<Option<PathBuf>> {
    let mut current = util::try_canonicalize(start);

    // Outside a Mind Repo: no recursion, just check cwd
    let boundary = match repo_root {
        Some(r) => Some(util::try_canonicalize(r)),
        None => {
            // Check current directory only
            let candidate = current.join("mind.yaml");
            if candidate.exists() {
                return Ok(Some(candidate));
            }
            return Ok(None);
        }
    };

    loop {
        let candidate = current.join("mind.yaml");
        if candidate.exists() {
            return Ok(Some(candidate));
        }
        // Stop before crossing the repo root boundary
        if let Some(ref boundary) = boundary {
            if current == *boundary {
                return Ok(None);
            }
        }
        match current.parent() {
            Some(parent) => {
                current = parent.to_path_buf();
            }
            None => return Ok(None),
        }
    }
}

// ---------------------------------------------------------------------------
// JSON Schema generation
// ---------------------------------------------------------------------------

/// Generate a JSON Schema (Draft-07) for `MindConfig`.
pub fn to_json_schema() -> serde_json::Value {
    let schema = schema_for!(MindConfig);
    serde_json::to_value(schema).unwrap_or(serde_json::Value::Null)
}

// ---------------------------------------------------------------------------
// Serialization helpers
// ---------------------------------------------------------------------------

/// Serialize a value to YAML string.
pub fn to_yaml(value: &impl Serialize) -> Result<String> {
    serde_yaml::to_string(value).map_err(|e| MfError::Internal(e.into()))
}

/// Serialize a value to compact JSON string.
pub fn to_json(value: &impl Serialize) -> Result<String> {
    serde_json::to_string(value).map_err(MfError::Json)
}

// ---------------------------------------------------------------------------
// Orchestration functions (called by CLI dispatch)
// ---------------------------------------------------------------------------

/// Run `mf config show`: load and merge config, resolve layout, serialize.
pub fn show_effective(cwd: &Path, repo_root: Option<&Path>, output_format: &str) -> Result<String> {
    let project_layer = load_project(cwd, repo_root)?;
    let mut effective = match project_layer {
        Some(overlay) => merge(MindConfig::default(), overlay),
        None => MindConfig::default(),
    };
    let project_path = cwd.canonicalize().unwrap_or_else(|_| cwd.to_path_buf());
    resolve_and_validate(&project_path, &mut effective)?;
    match output_format {
        "json" => to_json(&effective),
        _ => to_yaml(&effective),
    }
}

/// Run `mf config schema`: generate JSON Schema and serialize to `output_format`.
pub fn schema_output(output_format: &str) -> Result<String> {
    let schema = to_json_schema();
    match output_format {
        "yaml" => to_yaml(&schema),
        _ => to_json(&schema),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::config::*;

    // --- merge tests ---

    #[test]
    fn test_merge_base_only() {
        let base = MindConfig::default();
        let overlay = MindConfig::default();
        let result = merge(base, overlay);
        assert_eq!(result.schema_version, "1");
    }

    #[test]
    fn test_merge_overlay_wins_scalar() {
        let base = MindConfig::default();
        let mut overlay = MindConfig::default();
        overlay.build.output_dir = "_out".to_string();
        let result = merge(base, overlay);
        assert_eq!(result.build.output_dir, "_out");
    }

    #[test]
    fn test_merge_array_replacement() {
        let mut base = MindConfig::default();
        base.source.scan_paths = vec!["a".to_string()];
        let mut overlay = MindConfig::default();
        overlay.source.scan_paths = vec!["b".to_string()];
        let result = merge(base, overlay);
        assert_eq!(result.source.scan_paths, vec!["b"]);
    }

    #[test]
    fn test_merge_project_none() {
        let base = MindConfig::default();
        let overlay = MindConfig::default();
        let result = merge(base, overlay);
        assert!(result.project.is_none());
    }

    #[test]
    fn test_merge_project_overlay_some() {
        let base = MindConfig::default();
        let overlay = MindConfig {
            project: Some(ProjectMeta {
                name: "my-project".to_string(),
                description: None,
                created_at: Some("2026-01-01T00:00:00Z".to_string()),
            }),
            ..Default::default()
        };
        let result = merge(base, overlay);
        assert_eq!(result.project.unwrap().name, "my-project");
    }

    // --- serde helpers ---

    #[test]
    fn test_to_yaml_roundtrip() {
        let cfg = MindConfig::default();
        let yaml = to_yaml(&cfg).unwrap();
        assert!(yaml.contains("schema_version"));
    }

    #[test]
    fn test_to_json_roundtrip() {
        let cfg = MindConfig::default();
        let json = to_json(&cfg).unwrap();
        assert!(json.contains("schema_version"));
    }

    // --- json schema tests ---

    #[test]
    fn test_to_json_schema_valid() {
        let schema = to_json_schema();
        assert!(schema.get("properties").is_some());
        assert!(schema.get("definitions").is_some());
        // Should contain PublishTargetType enum
        let schema_str = serde_json::to_string(&schema).unwrap();
        assert!(schema_str.contains("github_pages"));
    }

    // --- T011: default layout resolution ---

    #[test]
    fn test_resolve_default_layout() {
        let config = MindConfig::default();
        let (effective, _) = resolve_effective_layout(&config);
        assert_eq!(effective.articles, "docs");
        assert_eq!(effective.sources, "sources");
        assert_eq!(effective.assets, "assets");
        assert_eq!(effective.templates, "templates");
        assert_eq!(effective.build_output, "outputs");
    }

    #[test]
    fn test_effective_layout_fields_order_is_deterministic() {
        let config = MindConfig::default();
        let (effective, _) = resolve_effective_layout(&config);
        let yaml = serde_yaml::to_string(&effective).unwrap();
        let articles_pos = yaml.find("articles:").unwrap();
        let sources_pos = yaml.find("sources:").unwrap();
        let assets_pos = yaml.find("assets:").unwrap();
        let templates_pos = yaml.find("templates:").unwrap();
        let build_output_pos = yaml.find("build_output:").unwrap();
        assert!(articles_pos < sources_pos);
        assert!(sources_pos < assets_pos);
        assert!(assets_pos < templates_pos);
        assert!(templates_pos < build_output_pos);
    }

    // --- T012: compatibility mapping and canonical precedence ---

    #[test]
    fn test_paths_compat_maps_to_layout() {
        let config = MindConfig {
            layout: None, // Simulate project with only paths
            paths: PathsConfig {
                docs: "notes".to_string(),
                sources: "refs".to_string(),
                assets: "media".to_string(),
                ..Default::default()
            },
            ..Default::default()
        };
        let (effective, _) = resolve_effective_layout(&config);
        assert_eq!(effective.articles, "notes");
        assert_eq!(effective.sources, "refs");
        assert_eq!(effective.assets, "media");
    }

    #[test]
    fn test_build_output_dir_maps_to_layout() {
        let config = MindConfig {
            layout: None,
            build: BuildConfig { output_dir: "dist".to_string(), ..Default::default() },
            ..Default::default()
        };
        let (effective, _) = resolve_effective_layout(&config);
        assert_eq!(effective.build_output, "dist");
    }

    #[test]
    fn test_canonical_layout_wins_over_paths() {
        let config: MindConfig =
            serde_yaml::from_str("schema_version: '1'\nlayout:\n  articles: canon\npaths:\n  docs: compat\n").unwrap();
        let (effective, _) = resolve_effective_layout(&config);
        assert_eq!(effective.articles, "canon");
    }

    #[test]
    fn test_layout_docs_alias_maps_to_articles() {
        let config: MindConfig = serde_yaml::from_str("schema_version: '1'\nlayout:\n  docs: legacy_name\n").unwrap();
        let layout = config.layout.as_ref().unwrap();
        assert_eq!(layout.articles.as_deref(), Some("legacy_name"));
    }

    #[test]
    fn test_layout_output_dir_alias_maps_to_build_output() {
        let config: MindConfig =
            serde_yaml::from_str("schema_version: '1'\nlayout:\n  output_dir: _myoutput\n").unwrap();
        let layout = config.layout.as_ref().unwrap();
        assert_eq!(layout.build_output.as_deref(), Some("_myoutput"));
    }

    // --- T013: invalid layout values ---

    #[test]
    fn test_validate_empty_layout_value() {
        let tmp = tempfile::tempdir().unwrap();
        let effective = EffectiveLayout {
            articles: "docs".to_string(),
            sources: "".to_string(),
            assets: "assets".to_string(),
            templates: "templates".to_string(),
            build_output: "outputs".to_string(),
        };
        let err = validate_layout(tmp.path(), &effective).unwrap_err();
        assert!(err.to_string().contains("layout.sources"));
        assert!(err.to_string().contains("empty"));
    }

    #[test]
    fn test_validate_absolute_path() {
        let tmp = tempfile::tempdir().unwrap();
        let effective = EffectiveLayout {
            articles: "/etc/passwd".to_string(),
            sources: "sources".to_string(),
            assets: "assets".to_string(),
            templates: "templates".to_string(),
            build_output: "outputs".to_string(),
        };
        let err = validate_layout(tmp.path(), &effective).unwrap_err();
        assert!(err.to_string().contains("layout.articles"));
        assert!(err.to_string().contains("absolute"));
    }

    #[test]
    fn test_validate_escape_path() {
        let tmp = tempfile::tempdir().unwrap();
        let effective = EffectiveLayout {
            articles: "../outside".to_string(),
            sources: "sources".to_string(),
            assets: "assets".to_string(),
            templates: "templates".to_string(),
            build_output: "outputs".to_string(),
        };
        let err = validate_layout(tmp.path(), &effective).unwrap_err();
        assert!(err.to_string().contains("layout.articles"));
        assert!(err.to_string().contains("escapes"));
    }

    #[test]
    fn test_validate_allows_parent_dir_text_in_segment_name() {
        let tmp = tempfile::tempdir().unwrap();
        let effective = EffectiveLayout {
            articles: "notes..drafts".to_string(),
            sources: "sources".to_string(),
            assets: "assets".to_string(),
            templates: "templates".to_string(),
            build_output: "outputs".to_string(),
        };
        validate_layout(tmp.path(), &effective).unwrap();
    }

    #[test]
    fn test_validate_duplicate_paths() {
        let tmp = tempfile::tempdir().unwrap();
        let effective = EffectiveLayout {
            articles: "content".to_string(),
            sources: "content".to_string(),
            assets: "assets".to_string(),
            templates: "templates".to_string(),
            build_output: "outputs".to_string(),
        };
        let err = validate_layout(tmp.path(), &effective).unwrap_err();
        assert!(err.to_string().contains("layout.articles"));
        assert!(err.to_string().contains("layout.sources"));
        assert!(err.to_string().contains("same path"));
    }

    #[test]
    fn test_validate_duplicate_paths_after_normalization() {
        let tmp = tempfile::tempdir().unwrap();
        let effective = EffectiveLayout {
            articles: "docs".to_string(),
            sources: "./docs".to_string(),
            assets: "assets".to_string(),
            templates: "templates".to_string(),
            build_output: "outputs".to_string(),
        };
        let err = validate_layout(tmp.path(), &effective).unwrap_err();
        assert!(err.to_string().contains("layout.articles"));
        assert!(err.to_string().contains("layout.sources"));
        assert!(err.to_string().contains("same path"));
    }

    #[test]
    fn test_validate_file_path() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("existing_file"), b"hello").unwrap();
        let effective = EffectiveLayout {
            articles: "existing_file".to_string(),
            sources: "sources".to_string(),
            assets: "assets".to_string(),
            templates: "templates".to_string(),
            build_output: "outputs".to_string(),
        };
        let err = validate_layout(tmp.path(), &effective).unwrap_err();
        assert!(err.to_string().contains("layout.articles"));
        assert!(err.to_string().contains("file"));
    }

    #[test]
    fn test_resolve_and_validate_passes_for_valid_layout() {
        let tmp = tempfile::tempdir().unwrap();
        let mut config = MindConfig::default();
        resolve_and_validate(tmp.path(), &mut config).unwrap();
        let layout = config.layout.as_ref().unwrap();
        assert_eq!(layout.articles.as_deref(), Some("docs"));
    }
}
