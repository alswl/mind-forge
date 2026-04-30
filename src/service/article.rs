use std::fs;
use std::path::Path;

use chrono::Utc;

use crate::error::{MfError, Result};
use crate::model::article::{
    Article, ArticleDiff, ArticleStatus, ArticleType, LintIssue, LintKind, ScannedArticle, Severity,
};
use crate::model::index::IndexFile;
use crate::service::util;

const ARTICLE_TEMPLATE: &str = r#"# {title}

> Created: {created_at}

## Summary

## Content
"#;

pub fn load_index(project_path: &Path) -> Result<IndexFile> {
    let path = project_path.join("mind-index.yaml");
    if !path.exists() {
        return Ok(IndexFile::create_default());
    }
    let content = fs::read_to_string(&path).map_err(MfError::Io)?;
    if content.trim().is_empty() {
        return Ok(IndexFile::create_default());
    }
    let index: IndexFile = serde_yaml::from_str(&content).map_err(|e| MfError::ParseError {
        kind: "yaml".to_string(),
        path: path.clone(),
        detail: e.to_string(),
    })?;
    util::validate_schema_version(&index.schema_version, &path)?;
    Ok(index)
}

pub fn save_index(index: &IndexFile, project_path: &Path) -> Result<()> {
    let path = project_path.join("mind-index.yaml");
    let content = serde_yaml::to_string(index).map_err(|e| MfError::Internal(e.into()))?;
    util::atomic_write(&path, &content)
}

/// Create a new article in the given project directory.
pub fn new_article(
    project_path: &Path,
    title: &str,
    template_text: Option<&str>,
    tags: &[String],
    draft: bool,
    force: bool,
) -> Result<String> {
    let filename = util::to_filename(title);
    let docs_dir = project_path.join("docs");
    fs::create_dir_all(&docs_dir).map_err(MfError::Io)?;

    let article_path = docs_dir.join(format!("{filename}.md"));
    if article_path.exists() {
        if force {
            fs::remove_file(&article_path).map_err(MfError::Io)?;
        } else {
            return Err(MfError::file_exists(article_path));
        }
    }

    let now = Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
    let content = template_text.unwrap_or(ARTICLE_TEMPLATE);
    let content = content
        .replace("{title}", title)
        .replace("{created_at}", &now)
        .replace("{tags}", &tags.join(", "));

    fs::write(&article_path, content).map_err(MfError::Io)?;

    let project_name = util::dir_name(project_path);

    let mut index = load_index(project_path)?;
    let articles = index.articles.get_or_insert_with(Vec::new);
    let status = if draft { ArticleStatus::Draft } else { ArticleStatus::Published };

    // When force, replace existing index entry instead of duplicating
    let source_path = format!("docs/{filename}.md");
    if force {
        articles.retain(|a| a.source_path != source_path);
    }

    articles.push(Article {
        title: title.to_string(),
        project: project_name,
        article_type: ArticleType::Blog,
        source_path,
        status,
        created_at: now.clone(),
        updated_at: now,
    });
    save_index(&index, project_path)?;

    Ok(filename)
}

/// List articles in a project.
pub fn list_articles(project_path: &Path) -> Result<Vec<Article>> {
    let index = load_index(project_path)?;
    Ok(index.articles.unwrap_or_default())
}

/// Scan `docs/` directory for markdown files and return discovered articles.
pub fn scan_docs(project_path: &Path) -> Result<Vec<ScannedArticle>> {
    let docs_dir = project_path.join("docs");
    if !docs_dir.exists() {
        return Ok(Vec::new());
    }

    let mut scanned = Vec::new();
    let entries = fs::read_dir(&docs_dir).map_err(MfError::Io)?;
    for entry in entries {
        let entry = entry.map_err(|e| {
            tracing::warn!("failed to read directory entry in {}: {e}", docs_dir.display());
            MfError::Io(e)
        })?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("md") {
            if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                let title = stem.replace('-', " ");
                scanned.push(ScannedArticle { title, filename: stem.to_string() });
            }
        }
    }
    Ok(scanned)
}

/// Compare the index against a filesystem scan to find added/removed articles.
pub fn compute_article_diff(index: &IndexFile, scanned: &[ScannedArticle]) -> ArticleDiff {
    let scanned_names: std::collections::HashSet<&str> =
        scanned.iter().map(|s| s.filename.as_str()).collect();

    let mut added = Vec::new();
    let mut removed = Vec::new();

    if let Some(ref articles) = index.articles {
        let mut index_names: std::collections::HashSet<&str> =
            std::collections::HashSet::with_capacity(articles.len());

        for a in articles {
            if let Some(name) =
                a.source_path.strip_prefix("docs/").and_then(|s| s.strip_suffix(".md"))
            {
                index_names.insert(name);
                if !scanned_names.contains(name) {
                    removed.push(a.clone());
                }
            }
        }

        for s in scanned {
            if !index_names.contains(s.filename.as_str()) {
                added.push(s.clone());
            }
        }
    } else {
        // No existing index — all scanned files are additions
        added.extend(scanned.iter().cloned());
    }

    ArticleDiff { added, removed }
}

/// Apply a diff to the index: add new articles, remove deleted ones.
pub fn reconcile_articles(
    project_path: &Path,
    mut index: IndexFile,
    diff: ArticleDiff,
) -> Result<IndexFile> {
    let project_name = util::dir_name(project_path);

    // Remove deleted articles
    if let Some(ref mut articles) = index.articles {
        let remove_paths: std::collections::HashSet<String> =
            diff.removed.iter().map(|a| a.source_path.clone()).collect();
        articles.retain(|a| !remove_paths.contains(&a.source_path));
    }

    // Add new articles
    let articles = index.articles.get_or_insert_with(Vec::new);
    for a in &diff.added {
        let now = Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
        articles.push(Article {
            title: a.title.clone(),
            project: project_name.clone(),
            article_type: ArticleType::Blog,
            source_path: format!("docs/{}.md", a.filename),
            status: ArticleStatus::Draft,
            created_at: now.clone(),
            updated_at: now,
        });
    }

    Ok(index)
}

/// Lint articles in the project: check filenames and content quality.
/// When `fix` is true, auto-fix fixable issues.
pub fn lint_articles(project_path: &Path, fix: bool) -> Result<Vec<LintIssue>> {
    let docs_dir = project_path.join("docs");
    if !docs_dir.exists() {
        return Ok(Vec::new());
    }

    let mut issues = Vec::new();

    let entries = fs::read_dir(&docs_dir).map_err(MfError::Io)?;
    for entry in entries {
        let entry = entry.map_err(|e| {
            tracing::warn!("failed to read directory entry in {}: {e}", docs_dir.display());
            MfError::Io(e)
        })?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
            // Check content before filename (filename may rename the file)
            check_content(&mut issues, &path, stem)?;
            // Check filename convention: lowercase with hyphens only
            check_filename(&mut issues, stem, fix, &path)?;
        }
    }

    Ok(issues)
}

fn check_filename(
    issues: &mut Vec<LintIssue>,
    stem: &str,
    fix: bool,
    full_path: &Path,
) -> Result<()> {
    if !stem.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-') {
        let expected = stem
            .to_lowercase()
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '-' })
            .collect::<String>();

        let expected_name = format!("{expected}.md");
        issues.push(LintIssue {
            severity: Severity::Warning,
            kind: LintKind::FilenameConvention,
            message: format!(
                "filename '{stem}' should be lowercase with hyphens (suggest: '{expected_name}')"
            ),
            path: format!("docs/{stem}.md"),
            fixable: true,
        });

        if fix {
            let new_path = full_path.with_file_name(&expected_name);
            if !new_path.exists() {
                fs::rename(full_path, &new_path).map_err(MfError::Io)?;
            }
        }
    }
    Ok(())
}

fn check_content(issues: &mut Vec<LintIssue>, full_path: &Path, stem: &str) -> Result<()> {
    let content = fs::read_to_string(full_path).map_err(MfError::Io)?;

    if content.trim().is_empty() {
        issues.push(LintIssue {
            severity: Severity::Error,
            kind: LintKind::EmptyFile,
            message: "article file is empty".to_string(),
            path: format!("docs/{stem}.md"),
            fixable: false,
        });
    }

    Ok(())
}
