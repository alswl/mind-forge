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
    let mut index = index::load(project_path)?;
    let scanned = scan_docs(project_path)?;
    let project_name = util::dir_name(project_path);
    let articles = index.articles.get_or_insert_with(Vec::new);
    let existing_paths: std::collections::HashSet<String> = articles.iter().map(|a| a.source_path.clone()).collect();

    for scanned_article in scanned {
        let source_path = source_path_for_scanned(&scanned_article);
        if existing_paths.contains(&source_path) {
            continue;
        }
        let now = Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
        articles.push(Article {
            title: scanned_article.title,
            project: project_name.clone(),
            article_type: ArticleType::Blog,
            source_path,
            status: ArticleStatus::Draft,
            created_at: now.clone(),
            updated_at: now,
        });
    }

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
    if let Some(source_dir) = config.build.articles.get(&article_key).and_then(|a| a.source_dir.clone()) {
        return source_dir;
    }
    if let Some(source_dir) = config
        .build
        .articles
        .values()
        .filter_map(|a| a.source_dir.as_ref())
        .find(|source_dir| source_dir.as_str() == article.source_path)
    {
        return source_dir.clone();
    }
    let paths = crate::defaults::DOCS_DIR;
    format!("{}/{}", paths, article_key)
}

/// Scan the docs directory for markdown files and return discovered articles.
///
/// Scans the default docs directory and any configured `source_dir` directories
/// from `mind.yaml`'s `build.articles.*.source_dir`.
pub fn scan_docs(project_path: &Path) -> Result<Vec<ScannedArticle>> {
    let paths = config_svc::project_paths(project_path)?;
    let mut scanned = Vec::new();

    // Scan default docs directory
    let docs_dir = project_path.join(&paths.docs);
    if docs_dir.exists() {
        scan_md_dir(&docs_dir, &paths.docs, &mut scanned)?;
    }

    // Scan configured source_dir directories from mind.yaml. Each
    // build.articles.<name>.source_dir entry represents one configured article
    // source, even when that directory contains several Markdown parts.
    if let Ok(Some(config)) = config_svc::load_project(project_path, None) {
        for (article_name, article_cfg) in &config.build.articles {
            if let Some(ref source_dir) = article_cfg.source_dir {
                let dir_path = project_path.join(source_dir);
                if dir_path.exists() && dir_path.is_dir() {
                    let source_path = configured_article_source_path(article_name, &dir_path, source_dir);
                    scanned.push(ScannedArticle {
                        title: article_name.replace('-', " "),
                        filename: article_name.clone(),
                        source_dir: Some(source_dir.clone()),
                        source_path: Some(source_path),
                    });
                }
            }
        }
    }

    // Deduplicate by source path (keep first occurrence)
    let mut seen = std::collections::HashSet::new();
    scanned.retain(|a| {
        let key = source_path_for_scanned(a);
        seen.insert(key)
    });

    Ok(scanned)
}

fn configured_article_source_path(article_name: &str, dir_path: &Path, source_dir: &str) -> String {
    let file_name = format!("{article_name}.{}", defaults::MARKDOWN_EXTENSION);
    if dir_path.join(&file_name).is_file() {
        format!("{source_dir}/{file_name}")
    } else {
        source_dir.to_string()
    }
}

/// Scan a single directory for markdown files, appending to `scanned`.
fn scan_md_dir(dir_path: &Path, rel_dir: &str, scanned: &mut Vec<ScannedArticle>) -> Result<()> {
    let entries = fs::read_dir(dir_path).map_err(MfError::Io)?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some(defaults::MARKDOWN_EXTENSION) {
            if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                let title = stem.replace('-', " ");
                scanned.push(ScannedArticle {
                    title,
                    filename: stem.to_string(),
                    source_dir: Some(rel_dir.to_string()),
                    source_path: None,
                });
            }
        }
    }
    Ok(())
}

/// Build the project-relative source path for a scanned article.
fn source_path_for_scanned(a: &ScannedArticle) -> String {
    if let Some(ref source_path) = a.source_path {
        return source_path.clone();
    }
    match a.source_dir {
        Some(ref dir) => format!("{}/{}.{}", dir, a.filename, defaults::MARKDOWN_EXTENSION),
        None => format!("{}/{}.{}", crate::defaults::DOCS_DIR, a.filename, defaults::MARKDOWN_EXTENSION),
    }
}

/// Compare the index against a filesystem scan to find added/removed articles.
///
/// `docs_dir` is the configured docs directory name (e.g. "docs").
pub fn compute_article_diff(index: &IndexFile, scanned: &[ScannedArticle], docs_dir: &str) -> ArticleDiff {
    let mut added = Vec::new();
    let mut removed = Vec::new();

    // Build set of scanned source_paths (project-relative)
    let scanned_paths: std::collections::HashSet<String> = scanned.iter().map(source_path_for_scanned).collect();

    // Build set of scanned filenames for the legacy fallback check
    let scanned_filenames: std::collections::HashSet<&str> = scanned.iter().map(|s| s.filename.as_str()).collect();

    // Removed: articles in index whose source_path no longer has a matching file
    for a in index.articles.iter().flat_map(|a| a.iter()) {
        if !scanned_paths.contains(&a.source_path) {
            // For articles in the default docs dir, also check via the old
            // filename-based method (strip docs/ prefix + .md extension)
            let docs_prefix = format!("{docs_dir}/");
            let in_docs = a.source_path.starts_with(&docs_prefix);
            let matched = if in_docs {
                a.source_path
                    .strip_prefix(&docs_prefix)
                    .and_then(|s| s.strip_suffix(&format!(".{}", defaults::MARKDOWN_EXTENSION)))
                    .is_some_and(|name| scanned_filenames.contains(name))
            } else {
                false
            };
            if !matched {
                removed.push(a.clone());
            }
        }
    }

    // Added: scanned articles not yet in index
    for s in scanned {
        let sp = source_path_for_scanned(s);
        let exists = index.articles.as_ref().is_some_and(|articles| articles.iter().any(|a| a.source_path == sp));
        if !exists {
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
        let source_path = if a.source_dir.is_some() {
            source_path_for_scanned(a)
        } else {
            // Defensive: source_path_for_scanned falls back to defaults::DOCS_DIR,
            // but we use paths.docs from config which may differ. This branch is
            // currently unreachable (scan_md_dir always sets source_dir), kept
            // for correctness if a future caller produces ScannedArticle without
            // a source_dir.
            format!("{}/{}.{}", paths.docs, a.filename, defaults::MARKDOWN_EXTENSION)
        };
        articles.push(Article {
            title: a.title.clone(),
            project: project_name.clone(),
            article_type: ArticleType::Blog,
            source_path,
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
