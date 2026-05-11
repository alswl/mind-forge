use serde::{Deserialize, Serialize};

/// Default value for `MindsManifest.projects_dir` when missing.
pub fn default_projects_dir() -> String {
    "projects".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectEntry {
    pub name: String,
    pub path: String,
    pub created_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub archived_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MindsManifest {
    pub schema_version: String,
    /// Subdirectory under repo root that holds project directories.
    /// Defaults to `"projects"`. Use `"."` for a flat layout.
    #[serde(default = "default_projects_dir")]
    pub projects_dir: String,
    #[serde(default)]
    pub projects: Vec<ProjectEntry>,
}

impl MindsManifest {
    /// Returns a default manifest: `schema_version: "1"`, `projects_dir: "projects"`, empty projects.
    pub fn create_default() -> Self {
        Self { schema_version: "1".to_string(), projects_dir: default_projects_dir(), projects: Vec::new() }
    }
}
