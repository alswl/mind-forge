use std::fs;
use std::path::Path;

use chrono::Utc;
use serde::Serialize;

use crate::defaults;
use crate::error::{MfError, Result};
use crate::model::article::{Article, ArticleDiff, ArticleStatus, ArticleType, LintIssue, ScannedArticle};
use crate::model::config::MindConfig;
use crate::model::index::IndexFile;
use crate::service::config as config_svc;
use crate::service::index;
use crate::service::util;

const ARTICLE_TEMPLATE: &str = r#"# {title}

> Created: {created_at}

## Summary

## Content
"#;

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
    let paths = config_svc::project_paths(project_path)?;
    let docs_dir = project_path.join(&paths.docs);
    fs::create_dir_all(&docs_dir).map_err(MfError::Io)?;

    let article_path = docs_dir.join(format!("{filename}.{}", defaults::MARKDOWN_EXTENSION));
    if article_path.exists() {
        if force {
            fs::remove_file(&article_path).map_err(MfError::Io)?;
        } else {
            return Err(MfError::file_exists(article_path));
        }
    }

    let now = Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
    let content = template_text.unwrap_or(ARTICLE_TEMPLATE);
    let content = content.replace("{title}", title).replace("{created_at}", &now).replace("{tags}", &tags.join(", "));

    fs::write(&article_path, content).map_err(MfError::Io)?;

    let project_name = util::dir_name(project_path);

    let mut index = index::load(project_path)?;
    let articles = index.articles.get_or_insert_with(Vec::new);
    let status = if draft { ArticleStatus::Draft } else { ArticleStatus::Published };

    // When force, replace existing index entry instead of duplicating
    let source_path = format!("{}/{filename}.{}", paths.docs, defaults::MARKDOWN_EXTENSION);
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
    index::save(project_path, &index)?;

    Ok(filename)
}

/// List articles in a project.
pub fn list_articles(project_path: &Path) -> Result<Vec<Article>> {
    let index = index::load(project_path)?;
    Ok(index.articles.unwrap_or_default())
}

/// Derive the article key (slug) from its source_path.
///
/// For a source_path like `"docs/my-article.md"`, returns `"my-article"`.
fn article_key_from_source_path(source_path: &str) -> String {
    let without_ext = source_path.strip_suffix(&format!(".{}", defaults::MARKDOWN_EXTENSION)).unwrap_or(source_path);
    let (_docs_prefix, key) = without_ext.rsplit_once('/').unwrap_or(("", without_ext));
    key.to_string()
}

/// Compute the effective source directory for an article based on project config.
///
/// Returns the project-root-relative directory path:
/// - Article's configured `source_dir` in `build.articles[article_key]` if present
/// - Otherwise `docs/<article-name>` as the default
pub fn effective_source_dir(config: &MindConfig, article: &Article) -> String {
    let article_key = article_key_from_source_path(&article.source_path);
    config.build.articles.get(&article_key).and_then(|a| a.source_dir.clone()).unwrap_or_else(|| {
        let paths = crate::defaults::DOCS_DIR;
        format!("{}/{}", paths, article_key)
    })
}

/// Scan the docs directory for markdown files and return discovered articles.
pub fn scan_docs(project_path: &Path) -> Result<Vec<ScannedArticle>> {
    let paths = config_svc::project_paths(project_path)?;
    let docs_dir = project_path.join(&paths.docs);
    if !docs_dir.exists() {
        return Ok(Vec::new());
    }

    let mut scanned = Vec::new();
    let entries = fs::read_dir(&docs_dir).map_err(MfError::Io)?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some(defaults::MARKDOWN_EXTENSION) {
            if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                let title = stem.replace('-', " ");
                scanned.push(ScannedArticle { title, filename: stem.to_string() });
            }
        }
    }
    Ok(scanned)
}

/// Compare the index against a filesystem scan to find added/removed articles.
pub fn compute_article_diff(index: &IndexFile, scanned: &[ScannedArticle], docs_dir: &str) -> ArticleDiff {
    let mut added = Vec::new();
    let mut removed = Vec::new();

    let index_names: std::collections::HashSet<&str> = index
        .articles
        .as_ref()
        .map(|a| {
            a.iter()
                .filter_map(|a| {
                    a.source_path
                        .strip_prefix(&format!("{docs_dir}/"))
                        .and_then(|s| s.strip_suffix(&format!(".{}", defaults::MARKDOWN_EXTENSION)))
                })
                .collect()
        })
        .unwrap_or_default();

    let scanned_names: std::collections::HashSet<&str> = scanned.iter().map(|s| s.filename.as_str()).collect();

    for a in index.articles.iter().flat_map(|a| a.iter()) {
        if let Some(name) = a
            .source_path
            .strip_prefix(&format!("{docs_dir}/"))
            .and_then(|s| s.strip_suffix(&format!(".{}", defaults::MARKDOWN_EXTENSION)))
        {
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

    ArticleDiff { added, removed }
}

/// Apply a diff to the index: add new articles, remove deleted ones.
pub fn reconcile_articles(project_path: &Path, mut index: IndexFile, diff: ArticleDiff) -> Result<IndexFile> {
    let project_name = util::dir_name(project_path);
    let paths = config_svc::project_paths(project_path)?;

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
            source_path: format!("{}/{}.{}", paths.docs, a.filename, defaults::MARKDOWN_EXTENSION),
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
    let paths = config_svc::project_paths(project_path)?;
    let docs_dir = project_path.join(&paths.docs);
    if !docs_dir.exists() {
        return Ok(Vec::new());
    }

    let mut issues = Vec::new();

    let entries = fs::read_dir(&docs_dir).map_err(MfError::Io)?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some(defaults::MARKDOWN_EXTENSION) {
            continue;
        }
        if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
            let rel_path = format!("{}/{}.{}", paths.docs, stem, defaults::MARKDOWN_EXTENSION);

            // Check content before filename (filename may rename the file)
            check_content(&mut issues, &path, &rel_path)?;
            // Check filename convention: lowercase with hyphens only
            check_filename(&mut issues, stem, &rel_path, fix, &path)?;
        }
    }

    Ok(issues)
}

/// Report from a successful article rename.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct ArticleRenameReport {
    pub old_title: String,
    pub new_title: String,
    pub old_source_path: String,
    pub new_source_path: String,
}

/// Rename an article: renames the file on disk and updates the index.
///
/// `old_title` is matched against article titles in the index. `new_title`
/// is the desired display title; the filename is derived from it via
/// [`util::to_filename`].
pub fn rename_article(
    project_path: &Path,
    old_title: &str,
    new_title: &str,
    force: bool,
) -> Result<ArticleRenameReport> {
    let new_filename = util::to_filename(new_title);
    let paths = config_svc::project_paths(project_path)?;

    // Load the index and find the article by title
    let mut index = index::load(project_path)?;
    let articles = index.articles.as_mut().ok_or_else(|| {
        MfError::not_found(
            format!("article '{old_title}' not found"),
            Some("use 'mf article list --project <project>' to see available articles".to_string()),
        )
    })?;

    let article = articles.iter_mut().find(|a| a.title == old_title).ok_or_else(|| {
        MfError::not_found(
            format!("article '{old_title}' not found"),
            Some("use 'mf article list --project <project>' to see available articles".to_string()),
        )
    })?;

    let old_source_path = article.source_path.clone();
    let new_source_path = format!("{}/{}.{}", paths.docs, new_filename, defaults::MARKDOWN_EXTENSION);

    // Rename the file on disk (only if the path actually differs)
    if old_source_path != new_source_path {
        let old_full = project_path.join(&old_source_path);
        let new_full = project_path.join(&new_source_path);

        if !old_full.exists() {
            return Err(MfError::not_found(
                format!("article file not found at {}", old_full.display()),
                Some("the index may be out of date; try 'mf article index'".to_string()),
            ));
        }

        if new_full.exists() {
            if force {
                fs::remove_file(&new_full).map_err(MfError::Io)?;
            } else {
                return Err(MfError::file_exists(new_full));
            }
        }

        fs::rename(&old_full, &new_full).map_err(MfError::Io)?;
    }

    // Update the index entry
    let now = Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
    article.title = new_title.to_string();
    article.source_path = new_source_path.clone();
    article.updated_at = now;
    index::save(project_path, &index)?;

    Ok(ArticleRenameReport {
        old_title: old_title.to_string(),
        new_title: new_title.to_string(),
        old_source_path,
        new_source_path,
    })
}

fn check_filename(issues: &mut Vec<LintIssue>, stem: &str, rel_path: &str, fix: bool, full_path: &Path) -> Result<()> {
    if !stem.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-') {
        let expected = stem
            .to_lowercase()
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '-' })
            .collect::<String>();

        issues.push(LintIssue {
            severity: "warning".to_string(),
            kind: "filename_convention".to_string(),
            message: format!("filename '{}' should be lowercase with hyphens (suggest: '{}.md')", stem, expected),
            path: rel_path.to_string(),
            fixable: true,
        });

        if fix {
            let new_path = full_path.with_file_name(format!("{}.md", expected));
            if !new_path.exists() {
                fs::rename(full_path, &new_path).map_err(MfError::Io)?;
            }
        }
    }
    Ok(())
}

fn check_content(issues: &mut Vec<LintIssue>, full_path: &Path, rel_path: &str) -> Result<()> {
    let content = fs::read_to_string(full_path).map_err(MfError::Io)?;

    if content.trim().is_empty() {
        issues.push(LintIssue {
            severity: "error".to_string(),
            kind: "empty_file".to_string(),
            message: "article file is empty".to_string(),
            path: rel_path.to_string(),
            fixable: false,
        });
    }

    Ok(())
}
