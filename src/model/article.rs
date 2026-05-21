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
