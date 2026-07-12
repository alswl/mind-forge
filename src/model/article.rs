use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArticleType {
    Arch,
    Prd,
    Blog,
    #[default]
    Blank,
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
    pub article_path: String,
    #[serde(default)]
    pub status: ArticleStatus,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub updated_at: String,
    /// Optional template origin for generated articles (US2).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub template_origin: Option<TemplateOrigin>,
}

/// Origin information for a generated article (discovered via templates).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TemplateOrigin {
    pub template_name: String,
    pub slot_value: String,
}

/// A file discovered during `docs/` scan, before matching against the index.
#[derive(Debug, Clone, Serialize)]
pub struct ScannedArticle {
    pub title: String,
    pub filename: String,
    /// The project-relative article directory this article was found in (e.g. "docs", "specs").
    /// `None` means the default docs directory.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub article_dir: Option<String>,
    /// Explicit project-relative article path when the scan result represents a
    /// configured article directory rather than a single Markdown file.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub article_path: Option<String>,
}

/// Result of comparing the index against a filesystem scan.
#[derive(Debug, Clone, Serialize)]
pub struct ArticleDiff {
    pub added: Vec<ScannedArticle>,
    pub removed: Vec<Article>,
}

// ── Lifecycle reports ──────────────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub struct ArticleRemoveReport {
    pub verb: String,
    pub kind: String,
    pub before: ArticleIdentity,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub after: Option<ArticleIdentity>,
    #[serde(default)]
    pub references: Vec<crate::model::lifecycle::Reference>,
    #[serde(default)]
    pub side_effects: Vec<crate::model::lifecycle::PlannedChange>,
    pub force: bool,
    pub dry_run: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub struct ArticleIdentity {
    pub title: String,
    pub article_path: String,
    pub scope: crate::model::lifecycle::ScopeRef,
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

// ── Article shape conversion types ─────────────────────────────────────────

/// Target shape for article conversion.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConversionDirection {
    ToSingleFile,
    ToDirectory,
}

impl ConversionDirection {
    /// Target on-disk shape produced by this direction.
    pub fn target_shape(self) -> ArticleShape {
        match self {
            ConversionDirection::ToSingleFile => ArticleShape::SingleFile,
            ConversionDirection::ToDirectory => ArticleShape::Directory,
        }
    }
}

impl std::fmt::Display for ConversionDirection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConversionDirection::ToSingleFile => write!(f, "--to-single-file"),
            ConversionDirection::ToDirectory => write!(f, "--to-directory"),
        }
    }
}

/// How the conversion direction was selected.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DirectionSource {
    Explicit,
    Inferred,
}

/// Outcome status for a single article conversion candidate.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConversionStatus {
    Converted,
    WouldConvert,
    Skipped,
    Failed,
    Declined,
}

/// On-disk shape of an article.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArticleShape {
    SingleFile,
    Directory,
}

/// Stable skip reasons for automation consumption.
pub mod skip_reason {
    pub const NO_SECTION_FILES: &str = "no_section_files";
    pub const MULTIPLE_SECTION_FILES: &str = "multiple_section_files";
    pub const TARGET_EXISTS: &str = "target_exists";
    pub const EXTRA_FILES: &str = "extra_files";
    pub const NOT_DIRECTORY_ARTICLE: &str = "not_directory_article";
    pub const NOT_SINGLE_FILE_ARTICLE: &str = "not_single_file_article";
}

/// Result for a single article conversion candidate.
#[derive(Debug, Clone, Serialize)]
pub struct ConversionResult {
    pub status: ConversionStatus,
    pub direction: ConversionDirection,
    pub source_shape: ArticleShape,
    pub target_shape: ArticleShape,
    pub source_path: String,
    pub source_content_path: String,
    pub target_path: String,
    pub target_content_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    pub index_updated: bool,
    pub source_removed: bool,
    /// Source block files folded into the target, in merge order. Present
    /// only when this conversion merged multiple blocks (spec 064 FR-007).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merged_section_files: Option<Vec<String>>,
}

/// Top-level conversion command result.
#[derive(Debug, Clone, Serialize)]
pub struct ConversionSummary {
    pub kind: String,
    pub direction: ConversionDirection,
    pub direction_source: DirectionSource,
    pub dry_run: bool,
    pub converted_count: usize,
    pub skipped_count: usize,
    pub failed_count: usize,
    pub scanned_count: usize,
    pub converted: Vec<ConversionResult>,
    pub skipped: Vec<ConversionResult>,
    pub failed: Vec<ConversionResult>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn article_without_template_origin_round_trips_byte_identical() {
        let yaml = r#"title: Test Article
project: my-project
type: blog
article_path: docs/test.md
status: draft
created_at: '2026-05-15T00:00:00Z'
updated_at: '2026-05-15T00:00:00Z'
"#;
        let article: Article = serde_yaml::from_str(yaml).unwrap();
        assert!(article.template_origin.is_none());

        // Round-trip: the pre-23 form should serialize back without template_origin
        let serialized = serde_yaml::to_string(&article).unwrap();
        assert!(!serialized.contains("template_origin"));
    }

    #[test]
    fn article_type_blank_round_trips() {
        // Wire form is "blank" (snake_case)
        let yaml = "type: blank\n";
        let article: Article = serde_yaml::from_str(&format!("title: T\n{yaml}")).unwrap();
        assert_eq!(article.article_type, ArticleType::Blank);

        let serialized = serde_yaml::to_string(&article).unwrap();
        assert!(serialized.contains("type: blank"), "expected 'type: blank' in serialized YAML");
    }

    #[test]
    fn article_type_default_is_blank() {
        assert_eq!(ArticleType::default(), ArticleType::Blank);
    }

    #[test]
    fn article_type_blog_still_deserializes() {
        let yaml = "title: Old\nproject: p\ntype: blog\narticle_path: docs/old.md\nstatus: draft\ncreated_at: '2026-01-01T00:00:00Z'\nupdated_at: '2026-01-01T00:00:00Z'\n";
        let article: Article = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(article.article_type, ArticleType::Blog);
    }

    #[test]
    fn article_with_template_origin_serializes() {
        let article = Article {
            title: "Daily Report".to_string(),
            project: "my-project".to_string(),
            article_type: ArticleType::Blog,
            article_path: "outputs/2026-05/2026-05-15.md".to_string(),
            status: ArticleStatus::Draft,
            created_at: "2026-05-15T00:00:00Z".to_string(),
            updated_at: "2026-05-15T00:00:00Z".to_string(),
            template_origin: Some(TemplateOrigin {
                template_name: "daily_report".to_string(),
                slot_value: "2026-05-15".to_string(),
            }),
        };
        let v = serde_json::to_value(&article).unwrap();
        assert_eq!(v["template_origin"]["template_name"], "daily_report");
        assert_eq!(v["template_origin"]["slot_value"], "2026-05-15");
    }
}
