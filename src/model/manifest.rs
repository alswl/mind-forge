use serde::{Deserialize, Deserializer, Serialize};

use crate::defaults;

/// Default value for `MindsManifest.projects_dir` when missing.
pub fn default_projects_dir() -> String {
    defaults::PROJECTS_DIR.to_string()
}

fn default_schema_version() -> String {
    defaults::SCHEMA_VERSION.to_string()
}

/// A project entry in `minds.yaml`.
///
/// When deserializing, accepts both:
/// - an object `{name, path, created_at, archived_at}`
/// - a plain string (path), which is converted with default values.
#[derive(Debug, Clone, Serialize)]
pub struct ProjectEntry {
    pub name: String,
    pub path: String,
    pub created_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub archived_at: Option<String>,
}

impl<'de> Deserialize<'de> for ProjectEntry {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum RawEntry {
            Obj { name: String, path: String, created_at: String, archived_at: Option<String> },
            Str(String),
        }

        match RawEntry::deserialize(deserializer)? {
            RawEntry::Obj { name, path, created_at, archived_at } => {
                Ok(ProjectEntry { name, path, created_at, archived_at })
            }
            RawEntry::Str(s) => {
                // Store the raw path string; service layer resolves the full path.
                Ok(ProjectEntry { name: s.clone(), path: s, created_at: String::new(), archived_at: None })
            }
        }
    }
}

// ── Repository Source Configuration ────────────────────────────────────────

/// Repository-level Source backend mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum SourceBackend {
    #[default]
    Legacy,
    Lance,
}

/// Default search mode for Source queries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum SearchDefaultMode {
    Basic,
    Advanced,
    #[default]
    Both,
}

/// Search configuration within the repository Source block.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceSearchConfig {
    #[serde(default = "SearchDefaultMode::default")]
    pub default_mode: SearchDefaultMode,
}

/// Advanced Source configuration (chunk/model/network).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdvancedSourceConfig {
    /// Explicit OpenAI-compatible embedding endpoint.  When absent, semantic
    /// indexing is unavailable; no implicit provider is selected.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub embedding_endpoint: Option<String>,
    /// Model name sent to the OpenAI-compatible `/v1/embeddings` endpoint.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub embedding_model: Option<String>,
    /// Name of the environment variable holding the provider API key.  The
    /// value itself is deliberately never serialized into `minds.yaml`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub embedding_api_key_env: Option<String>,
    /// Expected vector dimension returned by the provider.
    #[serde(default = "default_embedding_dimension")]
    pub embedding_dimension: u32,
    #[serde(default = "default_model_id")]
    pub model: String,
    #[serde(default = "default_model_revision")]
    pub model_revision: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_path: Option<String>,
    #[serde(default = "default_chunk_tokens")]
    pub chunk_tokens: u32,
    #[serde(default = "default_chunk_overlap")]
    pub chunk_overlap: u32,
    #[serde(default = "default_fetch_max_bytes")]
    pub fetch_max_bytes: u64,
    #[serde(default = "default_fetch_timeout_seconds")]
    pub fetch_timeout_seconds: u32,
    #[serde(default = "default_fetch_max_redirects")]
    pub fetch_max_redirects: u32,
}

fn default_model_id() -> String {
    "intfloat/multilingual-e5-small".to_string()
}
fn default_embedding_dimension() -> u32 {
    384
}
fn default_model_revision() -> String {
    "main".to_string()
}
fn default_chunk_tokens() -> u32 {
    384
}
fn default_chunk_overlap() -> u32 {
    48
}
fn default_fetch_max_bytes() -> u64 {
    64 * 1024 * 1024 // 64 MiB
}
fn default_fetch_timeout_seconds() -> u32 {
    30
}
fn default_fetch_max_redirects() -> u32 {
    5
}

impl Default for AdvancedSourceConfig {
    fn default() -> Self {
        Self {
            embedding_endpoint: None,
            embedding_model: None,
            embedding_api_key_env: None,
            embedding_dimension: default_embedding_dimension(),
            model: default_model_id(),
            model_revision: default_model_revision(),
            model_path: None,
            chunk_tokens: default_chunk_tokens(),
            chunk_overlap: default_chunk_overlap(),
            fetch_max_bytes: default_fetch_max_bytes(),
            fetch_timeout_seconds: default_fetch_timeout_seconds(),
            fetch_max_redirects: default_fetch_max_redirects(),
        }
    }
}

/// Repository-level Source configuration (`minds.yaml.source`).
///
/// Controls backend selection, activation marker, and advanced search/content
/// policy. The three activation fields (`activation_snapshot_id`,
/// `activation_catalog_fingerprint`, `storage_schema_version`) form the Lance
/// marker; a partial marker is corrupt and the system must fail closed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepositorySourceConfig {
    #[serde(default)]
    pub backend: SourceBackend,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub activation_snapshot_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub activation_catalog_fingerprint: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub storage_schema_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub search: Option<SourceSearchConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub advanced: Option<AdvancedSourceConfig>,
}

impl Default for RepositorySourceConfig {
    fn default() -> Self {
        Self {
            backend: SourceBackend::Legacy,
            activation_snapshot_id: None,
            activation_catalog_fingerprint: None,
            storage_schema_version: None,
            search: None,
            advanced: None,
        }
    }
}

impl RepositorySourceConfig {
    /// Returns true when the backend is `lance` and all three marker fields are present.
    pub fn is_lance_active(&self) -> bool {
        self.backend == SourceBackend::Lance
            && self.activation_snapshot_id.is_some()
            && self.activation_catalog_fingerprint.is_some()
            && self.storage_schema_version.is_some()
    }

    /// Returns true when `backend` is `lance` but the marker is incomplete (corrupt).
    pub fn is_lance_marker_corrupt(&self) -> bool {
        self.backend == SourceBackend::Lance
            && (self.activation_snapshot_id.is_none()
                || self.activation_catalog_fingerprint.is_none()
                || self.storage_schema_version.is_none())
    }

    /// Effective backend: `legacy` unless a complete Lance marker is present.
    pub fn effective_backend(&self) -> SourceBackend {
        if self.is_lance_active() { SourceBackend::Lance } else { SourceBackend::Legacy }
    }

    /// Resolved search mode given an optional CLI override.
    #[allow(dead_code)]
    pub fn resolved_search_mode(&self, cli_mode: Option<SearchDefaultMode>) -> SearchDefaultMode {
        if let Some(m) = cli_mode {
            return m;
        }
        if self.effective_backend() == SourceBackend::Lance {
            self.search.as_ref().map(|s| s.default_mode).unwrap_or_default()
        } else {
            SearchDefaultMode::Basic
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MindsManifest {
    #[serde(alias = "schema", default = "default_schema_version")]
    pub schema_version: String,
    /// Subdirectory under repo root that holds project directories.
    /// Defaults to `"projects"`. Use `"."` for a flat layout.
    #[serde(default = "default_projects_dir")]
    pub projects_dir: String,
    #[serde(default)]
    pub projects: Vec<ProjectEntry>,
    /// Repository-level Source backend and advanced configuration.
    /// Absent means legacy backend with defaults.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<RepositorySourceConfig>,
}

impl MindsManifest {
    /// Returns a default manifest: `schema_version: "1"`, `projects_dir: "projects"`, empty projects.
    pub fn create_default() -> Self {
        Self {
            schema_version: defaults::SCHEMA_VERSION.to_string(),
            projects_dir: default_projects_dir(),
            projects: Vec::new(),
            source: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Source config marker validation ───────────────────────────────────

    #[test]
    fn legacy_backend_by_default() {
        let config = RepositorySourceConfig {
            backend: SourceBackend::Legacy,
            activation_snapshot_id: None,
            activation_catalog_fingerprint: None,
            storage_schema_version: None,
            search: None,
            advanced: None,
        };
        assert_eq!(config.effective_backend(), SourceBackend::Legacy);
        assert!(!config.is_lance_active());
        assert!(!config.is_lance_marker_corrupt());
    }

    #[test]
    fn lance_backend_with_complete_marker_is_active() {
        let config = RepositorySourceConfig {
            backend: SourceBackend::Lance,
            activation_snapshot_id: Some("snap-1".into()),
            activation_catalog_fingerprint: Some("fp-1".into()),
            storage_schema_version: Some("1".into()),
            search: None,
            advanced: None,
        };
        assert!(config.is_lance_active());
        assert!(!config.is_lance_marker_corrupt());
        assert_eq!(config.effective_backend(), SourceBackend::Lance);
    }

    #[test]
    fn lance_backend_with_missing_snapshot_is_corrupt_not_active() {
        let config = RepositorySourceConfig {
            backend: SourceBackend::Lance,
            activation_snapshot_id: None,
            activation_catalog_fingerprint: Some("fp-1".into()),
            storage_schema_version: Some("1".into()),
            search: None,
            advanced: None,
        };
        assert!(!config.is_lance_active());
        assert!(config.is_lance_marker_corrupt());
        assert_eq!(config.effective_backend(), SourceBackend::Legacy);
    }

    #[test]
    fn lance_backend_with_missing_fingerprint_is_corrupt_not_active() {
        let config = RepositorySourceConfig {
            backend: SourceBackend::Lance,
            activation_snapshot_id: Some("snap-1".into()),
            activation_catalog_fingerprint: None,
            storage_schema_version: Some("1".into()),
            search: None,
            advanced: None,
        };
        assert!(!config.is_lance_active());
        assert!(config.is_lance_marker_corrupt());
    }

    #[test]
    fn lance_backend_with_missing_schema_version_is_corrupt_not_active() {
        let config = RepositorySourceConfig {
            backend: SourceBackend::Lance,
            activation_snapshot_id: Some("snap-1".into()),
            activation_catalog_fingerprint: Some("fp-1".into()),
            storage_schema_version: None,
            search: None,
            advanced: None,
        };
        assert!(!config.is_lance_active());
        assert!(config.is_lance_marker_corrupt());
    }

    #[test]
    fn resolved_mode_prefers_cli_override() {
        let config = RepositorySourceConfig {
            backend: SourceBackend::Lance,
            activation_snapshot_id: Some("s".into()),
            activation_catalog_fingerprint: Some("f".into()),
            storage_schema_version: Some("1".into()),
            search: Some(SourceSearchConfig { default_mode: SearchDefaultMode::Both }),
            advanced: None,
        };
        assert_eq!(config.resolved_search_mode(Some(SearchDefaultMode::Basic)), SearchDefaultMode::Basic);
        assert_eq!(config.resolved_search_mode(None), SearchDefaultMode::Both);
    }

    #[test]
    fn legacy_mode_resolves_to_basic() {
        let config = RepositorySourceConfig { backend: SourceBackend::Legacy, ..Default::default() };
        assert_eq!(config.resolved_search_mode(None), SearchDefaultMode::Basic);
    }

    #[test]
    fn minds_manifest_deserializes_without_source_block() {
        let yaml = r#"
schema_version: '1'
projects:
  - alpha
"#;
        let manifest: MindsManifest = serde_yaml::from_str(yaml).unwrap();
        assert!(manifest.source.is_none());
    }

    #[test]
    fn minds_manifest_deserializes_with_source_legacy() {
        let yaml = r#"
schema_version: '1'
projects: []
source:
  backend: legacy
"#;
        let manifest: MindsManifest = serde_yaml::from_str(yaml).unwrap();
        let source = manifest.source.unwrap();
        assert_eq!(source.backend, SourceBackend::Legacy);
        assert!(!source.is_lance_active());
    }

    #[test]
    fn minds_manifest_deserializes_with_full_lance_marker() {
        let yaml = r#"
schema_version: '1'
projects: []
source:
  backend: lance
  activation_snapshot_id: "snap-abc"
  activation_catalog_fingerprint: "fp-def"
  storage_schema_version: "1"
  search:
    default_mode: both
  advanced:
    model: intfloat/multilingual-e5-small
    model_revision: main
    chunk_tokens: 384
    chunk_overlap: 48
"#;
        let manifest: MindsManifest = serde_yaml::from_str(yaml).unwrap();
        let source = manifest.source.unwrap();
        assert!(source.is_lance_active());
        assert!(!source.is_lance_marker_corrupt());
        assert_eq!(source.activation_snapshot_id.as_deref(), Some("snap-abc"));
        assert_eq!(source.storage_schema_version.as_deref(), Some("1"));
        let adv = source.advanced.unwrap();
        assert_eq!(adv.chunk_tokens, 384);
        assert_eq!(adv.chunk_overlap, 48);
    }

    #[test]
    fn minds_manifest_preserves_unknown_source_fields() {
        // Unknown YAML fields inside `source` should not cause deserialization errors.
        let yaml = r#"
schema_version: '1'
projects: []
source:
  backend: legacy
  future_feature: experimental
"#;
        let manifest: MindsManifest = serde_yaml::from_str(yaml).unwrap();
        let source = manifest.source.unwrap();
        assert_eq!(source.backend, SourceBackend::Legacy);
    }

    // ── Original project entry tests ─────────────────────────────────────

    #[test]
    fn test_deserialize_string_project_entry() {
        let yaml = r#"
schema_version: '1'
projects:
  - alpha
  - beta
"#;
        let manifest: MindsManifest = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(manifest.projects.len(), 2);
        assert_eq!(manifest.projects[0].name, "alpha");
        assert_eq!(manifest.projects[0].path, "alpha");
        assert_eq!(manifest.projects[1].name, "beta");
    }

    #[test]
    fn test_deserialize_object_project_entry() {
        let yaml = r#"
schema_version: '1'
projects:
  - name: alpha
    path: ./projects/alpha
    created_at: "2026-01-01T00:00:00Z"
"#;
        let manifest: MindsManifest = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(manifest.projects.len(), 1);
        assert_eq!(manifest.projects[0].name, "alpha");
        assert_eq!(manifest.projects[0].path, "./projects/alpha");
    }

    #[test]
    fn test_deserialize_mixed_project_entries() {
        let yaml = r#"
schema_version: '1'
projects:
  - alpha
  - name: beta
    path: ./projects/beta
    created_at: "2026-01-01T00:00:00Z"
  - gamma
"#;
        let manifest: MindsManifest = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(manifest.projects.len(), 3);
    }

    #[test]
    fn test_deserialize_schema_alias() {
        let yaml = r#"
schema: '1'
projects: []
"#;
        let manifest: MindsManifest = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(manifest.schema_version, "1");
    }

    #[test]
    fn test_deserialize_missing_schema_version_defaults_to_1() {
        let yaml = r#"
projects:
  - alpha
"#;
        let manifest: MindsManifest = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(manifest.schema_version, "1");
        assert_eq!(manifest.projects.len(), 1);
    }

    #[test]
    fn test_serialize_project_entry_keeps_object_form() {
        let entry = ProjectEntry {
            name: "alpha".to_string(),
            path: "./projects/alpha".to_string(),
            created_at: "2026-01-01T00:00:00Z".to_string(),
            archived_at: None,
        };
        let yaml = serde_yaml::to_string(&entry).unwrap();
        assert!(yaml.contains("name: alpha"));
        assert!(yaml.contains("path:"));
        // Should serialize as object, not string
        assert!(!yaml.starts_with("alpha\n"));
    }
}
