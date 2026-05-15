use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArticleType {
    Arch,
    Prd,
    #[default]
    Blog,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArticleStatus {
    #[default]
    Draft,
    Published,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Article {
    pub title: String,
    #[serde(default)]
    pub project: String,
    #[serde(rename = "type", default)]
    pub article_type: ArticleType,
    #[serde(default)]
    pub source_path: String,
    #[serde(default)]
    pub status: ArticleStatus,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub updated_at: String,
}

/// A file discovered during `docs/` scan, before matching against the index.
#[derive(Debug, Clone, Serialize)]
pub struct ScannedArticle {
    pub title: String,
    pub filename: String,
    /// The project-relative source directory this article was found in (e.g. "docs", "specs").
    /// `None` means the default docs directory.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_dir: Option<String>,
}

/// Result of comparing the index against a filesystem scan.
#[derive(Debug, Clone, Serialize)]
pub struct ArticleDiff {
    pub added: Vec<ScannedArticle>,
    pub removed: Vec<Article>,
}

/// A single lint issue found during `mf article lint`.
#[derive(Debug, Clone, Serialize)]
pub struct LintIssue {
    pub severity: String,
    pub kind: String,
    pub message: String,
    pub path: String,
    pub fixable: bool,
}
