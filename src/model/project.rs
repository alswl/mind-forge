use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// View models (output-only, Serialize only)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct ProjectListEntry {
    pub name: String,
    pub path: String,
    pub created_at: String,
    pub archived_at: Option<String>,
    pub document_count: u64,
    pub last_activity_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct ProjectStatusSnapshot {
    pub name: String,
    pub path: String,
    pub articles: u64,
    pub assets: u64,
    pub sources: u64,
    pub terms: u64,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct ProjectLintIssue {
    pub severity: LintSeverity,
    pub kind: LintKind,
    pub message: String,
    pub path: String,
    pub fixable: bool,
    pub fixed: bool,
}

// ---------------------------------------------------------------------------
// Lint enums
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LintSeverity {
    Warning,
    Error,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
#[clap(rename_all = "snake_case")]
pub enum LintKind {
    MissingDirectory,
    StaleIndexEntry,
    NameConvention,
    MissingManifest,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_project_list_entry_serialization() {
        let entry = ProjectListEntry {
            name: "alpha".to_string(),
            path: "./alpha".to_string(),
            created_at: "2026-04-30T12:00:00Z".to_string(),
            archived_at: None,
            document_count: 5,
            last_activity_at: Some("2026-04-30T12:15:00Z".to_string()),
        };
        let json = serde_json::to_value(&entry).unwrap();
        assert_eq!(json["name"], "alpha");
        assert_eq!(json["path"], "./alpha");
        assert_eq!(json["document_count"], 5);
        assert!(json.get("archived_at").unwrap().is_null(), "archived_at should be null");
        assert_eq!(json["last_activity_at"], "2026-04-30T12:15:00Z");
    }

    #[test]
    fn test_project_list_entry_null_last_activity() {
        let entry = ProjectListEntry {
            name: "empty".to_string(),
            path: "./empty".to_string(),
            created_at: "2026-04-30T12:00:00Z".to_string(),
            archived_at: None,
            document_count: 0,
            last_activity_at: None,
        };
        let json = serde_json::to_value(&entry).unwrap();
        assert!(json.get("last_activity_at").unwrap().is_null());
    }

    #[test]
    fn test_project_status_snapshot_serialization() {
        let snap = ProjectStatusSnapshot {
            name: "alpha".to_string(),
            path: "./alpha".to_string(),
            articles: 2,
            assets: 1,
            sources: 1,
            terms: 0,
            updated_at: Some("2026-04-30T12:15:00Z".to_string()),
        };
        let json = serde_json::to_value(&snap).unwrap();
        assert_eq!(json["articles"], 2);
        assert_eq!(json["assets"], 1);
        assert_eq!(json["sources"], 1);
        assert_eq!(json["terms"], 0);
        assert_eq!(json["updated_at"], "2026-04-30T12:15:00Z");
    }

    #[test]
    fn test_project_status_snapshot_null_updated_at() {
        let snap = ProjectStatusSnapshot {
            name: "empty".to_string(),
            path: "./empty".to_string(),
            articles: 0,
            assets: 0,
            sources: 0,
            terms: 0,
            updated_at: None,
        };
        let json = serde_json::to_value(&snap).unwrap();
        assert!(json.get("updated_at").unwrap().is_null());
    }

    #[test]
    fn test_lint_issue_serialization() {
        let issue = ProjectLintIssue {
            severity: LintSeverity::Error,
            kind: LintKind::MissingDirectory,
            message: "missing sources/ directory".to_string(),
            path: "sources/".to_string(),
            fixable: true,
            fixed: false,
        };
        let json = serde_json::to_value(&issue).unwrap();
        assert_eq!(json["severity"], "error");
        assert_eq!(json["kind"], "missing_directory");
        assert_eq!(json["message"], "missing sources/ directory");
        assert_eq!(json["fixable"], true);
        assert_eq!(json["fixed"], false);
    }

    #[test]
    fn test_lint_kind_serde() {
        assert_eq!(serde_json::to_value(LintKind::StaleIndexEntry).unwrap(), "stale_index_entry");
        assert_eq!(serde_json::to_value(LintKind::NameConvention).unwrap(), "name_convention");
        assert_eq!(serde_json::to_value(LintKind::MissingManifest).unwrap(), "missing_manifest");
    }
}
