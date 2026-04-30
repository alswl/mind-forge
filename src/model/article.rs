use std::fmt;

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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Warning,
    Error,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Warning => write!(f, "warning"),
            Self::Error => write!(f, "error"),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LintKind {
    FilenameConvention,
    EmptyFile,
}

impl fmt::Display for LintKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FilenameConvention => write!(f, "filename_convention"),
            Self::EmptyFile => write!(f, "empty_file"),
        }
    }
}

/// A single lint issue found during `mf article lint`.
#[derive(Debug, Clone, Serialize)]
pub struct LintIssue {
    pub severity: Severity,
    pub kind: LintKind,
    pub message: String,
    pub path: String,
    pub fixable: bool,
}
