use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArticleType {
    Arch,
    Prd,
    Blog,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArticleStatus {
    Draft,
    Published,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Article {
    pub title: String,
    pub project: String,
    #[serde(rename = "type")]
    pub article_type: ArticleType,
    pub source_path: String,
    pub status: ArticleStatus,
    pub created_at: String,
    pub updated_at: String,
}

/// A file discovered during `docs/` scan, before matching against the index.
#[derive(Debug, Clone, Serialize)]
pub struct ScannedArticle {
    pub title: String,
    pub filename: String,
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
