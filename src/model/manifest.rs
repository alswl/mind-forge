use serde::{Deserialize, Serialize};

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
    #[serde(default)]
    pub projects: Vec<ProjectEntry>,
}

impl MindsManifest {
    /// 返回含 `schema_version: "1"` 与空 `projects` 列表的默认 manifest。
    pub fn create_default() -> Self {
        Self { schema_version: "1".to_string(), projects: Vec::new() }
    }
}
