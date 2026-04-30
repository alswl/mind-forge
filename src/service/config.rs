//! Config service: load, merge, schema generation, init template, and atomic write.
//!
//! This module implements the business logic for `mf config {schema,show,init}`.
//! It strictly separates from `src/cli/` (dispatch + formatting) and `src/model/` (pure data).

use std::fs;
use std::path::{Path, PathBuf};

use chrono::Utc;
use schemars::schema_for;
use serde::Serialize;

use crate::error::{MfError, Result};
use crate::model::config::MindConfig;
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
pub fn load_project(cwd: &Path, repo_root: Option<&Path>) -> Result<Option<MindConfig>> {
    let mind_path = find_mind_yaml(cwd, repo_root)?;
    match mind_path {
        None => Ok(None),
        Some(path) => {
            let content = fs::read_to_string(&path).map_err(MfError::Io)?;
            if content.trim().is_empty() {
                return Ok(None);
            }
            let mut config: MindConfig =
                serde_yaml::from_str(&content).map_err(|e| MfError::ParseError {
                    kind: "yaml".to_string(),
                    path: path.clone(),
                    detail: e.to_string(),
                })?;
            // Schema version fallback: missing → default "1"
            if config.schema_version.is_empty() {
                config.schema_version = "1".to_string();
            }
            util::validate_schema_version(&config.schema_version, &path)?;
            Ok(Some(config))
        }
    }
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
// Init template
// ---------------------------------------------------------------------------

const INIT_PROJECT_TEMPLATE: &str = r#"schema_version: "1"
project:
  name: "{name}"
  created_at: "{created_at}"

# build:
#   output_dir: "_build"       # 默认构建输出目录
#   merge_order: []            # 合并优先级（按文件名模式）

# publish:
#   default_target: null       # 默认发布目标名称
#   targets: null              # 发布目标列表

# source:
#   scan_paths: []             # 源文件扫描路径
#   types: []                  # 源文件类型筛选

# term:
#   enabled: true              # 术语检查启用
#   case_sensitive: false      # 术语大小写敏感

# paths:
#   docs: "docs"               # 文档目录
#   sources: "sources"         # 源文件目录
#   assets: "assets"           # 资源目录
#   archive: "_archived"       # 归档目录
"#;

/// Generate the default `mind.yaml` content for `mf config init`.
pub fn init_template(name: &str, created_at: &str) -> String {
    INIT_PROJECT_TEMPLATE.replace("{name}", name).replace("{created_at}", created_at)
}

/// Sanitize a directory name to kebab-case for use as project name.
///
/// Rules: lowercase; replace whitespace, `_`, `.` with `-`; strip non `[a-z0-9-]`;
/// collapse consecutive `-`; trim leading/trailing `-`; fallback to "untitled" if empty.
pub fn sanitize_project_name(raw: &str) -> String {
    let sanitized: String = raw
        .to_lowercase()
        .chars()
        .map(|c| match c {
            'a'..='z' | '0'..='9' => c,
            ' ' | '_' | '.' => '-',
            _ => '-',
        })
        .collect();

    // Collapse consecutive dashes, strip leading/trailing dashes
    let collapsed: String = sanitized
        .chars()
        .fold(String::new(), |mut acc, c| {
            let last_is_dash = acc.ends_with('-');
            if c == '-' && last_is_dash {
                // skip duplicate dash
            } else {
                acc.push(c);
            }
            acc
        })
        .trim_matches('-')
        .to_string();

    if collapsed.is_empty() {
        "untitled".to_string()
    } else {
        collapsed
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

/// Serialize a value to pretty-printed JSON string.
pub fn to_json(value: &impl Serialize) -> Result<String> {
    serde_json::to_string_pretty(value).map_err(MfError::Json)
}

// ---------------------------------------------------------------------------
// Orchestration functions (called by CLI dispatch)
// ---------------------------------------------------------------------------

/// Run `mf config show`: load and merge config, serialize to `output_format`.
pub fn show_effective(cwd: &Path, repo_root: Option<&Path>, output_format: &str) -> Result<String> {
    let project_layer = load_project(cwd, repo_root)?;
    let effective = match project_layer {
        Some(overlay) => merge(MindConfig::default(), overlay),
        None => MindConfig::default(),
    };
    match output_format {
        "json" => to_json(&effective),
        _ => to_yaml(&effective),
    }
}

/// Run `mf config init`: generate mind.yaml and atomic-write it.
/// Returns the output file path.
pub fn init_config(
    cwd: &Path,
    output: Option<&Path>,
    target: &str,
    force: bool,
) -> Result<PathBuf> {
    if target != "project" {
        return Err(MfError::not_implemented("--target user"));
    }

    let output_path = match output {
        Some(p) => {
            if p.is_absolute() {
                p.to_path_buf()
            } else {
                cwd.join(p)
            }
        }
        None => cwd.join("mind.yaml"),
    };

    if output_path.exists() && !force {
        return Err(MfError::file_exists(output_path));
    }

    // Derive project name from cwd directory name
    let dir_name = cwd.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let project_name = sanitize_project_name(&dir_name);
    let created_at = Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
    let content = init_template(&project_name, &created_at);

    util::atomic_write(&output_path, &content)?;
    Ok(output_path)
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
                created_at: Some("2026-01-01T00:00:00Z".to_string()),
            }),
            ..Default::default()
        };
        let result = merge(base, overlay);
        assert_eq!(result.project.unwrap().name, "my-project");
    }

    // --- sanitize tests ---

    #[test]
    fn test_sanitize_normal() {
        assert_eq!(sanitize_project_name("my-project"), "my-project");
    }

    #[test]
    fn test_sanitize_whitespace() {
        assert_eq!(sanitize_project_name("My Project"), "my-project");
    }

    #[test]
    fn test_sanitize_underscores() {
        assert_eq!(sanitize_project_name("hello_world"), "hello-world");
    }

    #[test]
    fn test_sanitize_dots() {
        assert_eq!(sanitize_project_name("my.project"), "my-project");
    }

    #[test]
    fn test_sanitize_special_chars() {
        assert_eq!(sanitize_project_name("Hello! @World#"), "hello-world");
    }

    #[test]
    fn test_sanitize_chinese_fallback() {
        assert_eq!(sanitize_project_name("中文"), "untitled");
    }

    #[test]
    fn test_sanitize_empty_fallback() {
        assert_eq!(sanitize_project_name(""), "untitled");
    }

    // --- template tests ---

    #[test]
    fn test_init_template_renders() {
        let result = init_template("test-proj", "2026-04-29T12:00:00Z");
        assert!(result.contains("test-proj"));
        assert!(result.contains("2026-04-29T12:00:00Z"));
        assert!(result.contains("schema_version: \"1\""));
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
}
