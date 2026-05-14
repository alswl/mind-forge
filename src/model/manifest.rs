use serde::{Deserialize, Deserializer, Serialize};

/// Default value for `MindsManifest.projects_dir` when missing.
pub fn default_projects_dir() -> String {
    "projects".to_string()
}

fn default_schema_version() -> String {
    "1".to_string()
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
}

impl MindsManifest {
    /// Returns a default manifest: `schema_version: "1"`, `projects_dir: "projects"`, empty projects.
    pub fn create_default() -> Self {
        Self { schema_version: "1".to_string(), projects_dir: default_projects_dir(), projects: Vec::new() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
