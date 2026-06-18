use std::collections::HashMap;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use chrono::Utc;
use serde::Serialize;

use crate::defaults;
use crate::error::{MfError, Result};
use crate::model::article::{
    skip_reason, Article, ArticleDiff, ArticleShape, ArticleStatus, ArticleType, ConversionDirection, ConversionResult,
    ConversionStatus, LintIssue, ScannedArticle, TemplateOrigin,
};
use crate::model::config::{MindConfig, TemplateMode};
use crate::model::index::IndexFile;
use crate::service::config as config_svc;
use crate::service::index;
use crate::service::util;
use crate::service::util::path_template::PathTemplate;

#[allow(dead_code)]
const ARTICLE_TEMPLATE: &str = r#"# {title}

> Created: {created_at}

## Summary

## Content
"#;

const TEMPLATE_BLANK: &str = "# {title}\n\n> Created: {created_at}\n";
const TEMPLATE_ARCH: &str = "# {title}\n\n> Created: {created_at}\n\n## Context\n\n## Decision\n\n## Consequence\n\n## Alternatives Considered\n";
const TEMPLATE_PRD: &str =
    "# {title}\n\n> Created: {created_at}\n\n## Background\n\n## Goals\n\n## Non-Goals\n\n## Requirements\n";
const TEMPLATE_BLOG: &str = "# {title}\n\n> Created: {created_at}\n\n## Summary\n\n## Content\n";

fn builtin_template(name: &str) -> Option<(&'static str, ArticleType)> {
    match name {
        "blank" => Some((TEMPLATE_BLANK, ArticleType::Blank)),
        "arch" => Some((TEMPLATE_ARCH, ArticleType::Arch)),
        "prd" => Some((TEMPLATE_PRD, ArticleType::Prd)),
        "blog" => Some((TEMPLATE_BLOG, ArticleType::Blog)),
        _ => None,
    }
}

fn resolve_custom_template_path(project_path: &Path, template_arg: &str) -> Result<PathBuf> {
    let relative = Path::new(template_arg);
    if relative.components().any(|c| matches!(c, Component::ParentDir | Component::RootDir | Component::Prefix(_))) {
        return Err(MfError::usage(
            format!("template path '{template_arg}' is outside the project root"),
            Some("use a path relative to the project root".to_string()),
        ));
    }

    let tmpl_path = project_path.join(relative);
    if tmpl_path.exists() {
        return util::canonicalize_within(project_path, &tmpl_path);
    }

    if let Some(parent) = tmpl_path.parent() {
        if parent.exists() {
            let _ = util::canonicalize_within(project_path, parent)?;
        }
    }
    Ok(tmpl_path)
}

/// Split a resolved template body into block files for a directory article.
///
/// LF-normalises input, scans for `^## ` headings, and returns a vector of
/// `(filename, body)` pairs. Returns [`MfError::DuplicateBlockSlug`] when
/// two headings produce the same slug.
fn split_template_into_blocks(resolved: &str) -> Result<Vec<(String, String)>> {
    let normalized = resolved.replace("\r\n", "\n");
    let lines: Vec<&str> = normalized.lines().collect();

    struct Block {
        h2_text: String,
        slug: String,
        body: String,
    }

    let mut raw: Vec<Block> = Vec::new();
    let mut head_lines: Vec<&str> = Vec::new();
    let mut current_h2: Option<&str> = None;
    let mut current_body: Vec<&str> = Vec::new();

    for line in &lines {
        if let Some(h2_text) = line.strip_prefix("## ") {
            if let Some(h2_text) = current_h2.take() {
                let slug = util::to_filename(h2_text.trim());
                let body = current_body.join("\n");
                raw.push(Block { h2_text: h2_text.to_string(), slug, body });
            } else {
                let body = head_lines.join("\n");
                raw.push(Block { h2_text: String::new(), slug: String::new(), body });
                head_lines.clear();
            }
            current_h2 = Some(h2_text);
            current_body = vec![*line];
        } else if current_h2.is_some() {
            current_body.push(line);
        } else {
            head_lines.push(line);
        }
    }

    if let Some(h2_text) = current_h2.take() {
        let slug = util::to_filename(h2_text.trim());
        let body = current_body.join("\n");
        raw.push(Block { h2_text: h2_text.to_string(), slug, body });
    } else {
        let body = head_lines.join("\n");
        raw.push(Block { h2_text: String::new(), slug: String::new(), body });
    }

    // Check for duplicate slugs among H2 blocks (skip head block at index 0)
    let mut slug_map: std::collections::HashMap<&str, &str> = std::collections::HashMap::new();
    for block in raw.iter().skip(1) {
        if block.slug.is_empty() {
            continue;
        }
        if let Some(prev_h2) = slug_map.insert(&block.slug, &block.h2_text) {
            return Err(MfError::DuplicateBlockSlug {
                slug: block.slug.clone(),
                h1: prev_h2.to_string(),
                h2: block.h2_text.clone(),
            });
        }
    }

    let mut result: Vec<(String, String)> = Vec::new();
    for (i, block) in raw.into_iter().enumerate() {
        if i == 0 {
            result.push(("01-opening.md".to_string(), block.body));
        } else {
            result.push((format!("{:02}-{}.md", i + 1, block.slug), block.body));
        }
    }

    Ok(result)
}

/// Create a new article in the given project directory.
///
/// Handles both directory mode (default) and file mode (`--file`). The
/// template arg is resolved via [`builtin_template`] first and falls back
/// to a project-root-relative path lookup.
pub fn new_article(
    project_path: &Path,
    title: &str,
    template_arg: &str,
    file_mode: bool,
    tags: &[String],
    draft: bool,
    force: bool,
) -> Result<NewArticleResult> {
    let filename = util::to_filename(title);
    let layout = config_svc::effective_layout(project_path)?;
    let docs_dir = project_path.join(&layout.articles);
    fs::create_dir_all(&docs_dir).map_err(MfError::Io)?;

    // Resolve template
    let (resolved_body, article_type, template_label) = if let Some((body, at)) = builtin_template(template_arg) {
        (body.to_string(), at, template_arg.to_string())
    } else {
        let tmpl_path = resolve_custom_template_path(project_path, template_arg)?;
        match fs::read_to_string(&tmpl_path) {
            Ok(body) => (body, ArticleType::Blank, template_arg.to_string()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Err(MfError::UnknownTemplate { name: template_arg.to_string() });
            }
            Err(e) => return Err(MfError::Io(e)),
        }
    };

    // Load full config for Typora plugin settings
    let config = config_svc::load_project(project_path, Some(project_path))?.unwrap_or_default();
    let plugins = config.plugins.as_ref();
    let typora_enabled = effective_typora_enabled(plugins);
    let typora_path: Option<String> = if typora_enabled {
        let file_dir = if file_mode {
            project_path.join(&layout.articles)
        } else {
            project_path.join(format!("{}/{}", layout.articles, filename))
        };
        Some(compute_typora_assets_path(project_path, &layout.assets, &file_dir))
    } else {
        None
    };

    let now = Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
    let resolved =
        resolved_body.replace("{title}", title).replace("{created_at}", &now).replace("{tags}", &tags.join(", "));

    let files = if file_mode {
        write_article_file(
            project_path,
            &layout.articles,
            &filename,
            &resolved,
            typora_path.as_deref(),
            &now,
            title,
            article_type,
            draft,
            force,
        )
    } else {
        write_article_directory(
            project_path,
            &layout.articles,
            &filename,
            &resolved,
            typora_path.as_deref(),
            &now,
            title,
            article_type,
            draft,
            force,
        )
    }?;

    Ok(NewArticleResult {
        filename: filename.clone(),
        template: template_label,
        shape: if file_mode { "file".to_string() } else { "directory".to_string() },
        docs_dir: layout.articles,
        files,
        typora_front_matter_injected: typora_enabled,
        typora_copy_images_to: typora_path,
    })
}

/// Result of creating a new article, carrying metadata for the JSON envelope.
pub struct NewArticleResult {
    pub filename: String,
    pub template: String,
    pub shape: String,
    pub docs_dir: String,
    pub files: Vec<String>,
    pub typora_front_matter_injected: bool,
    pub typora_copy_images_to: Option<String>,
}

fn sibling_backup_path(target: &Path) -> PathBuf {
    let parent = target.parent().unwrap_or_else(|| Path::new("."));
    let name = target.file_name().unwrap_or_default().to_string_lossy();
    let pid = std::process::id();
    let rand = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos();
    parent.join(format!(".{name}.bak.{pid}.{rand}"))
}

#[allow(clippy::too_many_arguments)]
fn write_article_file(
    project_path: &Path,
    docs: &str,
    slug: &str,
    content: &str,
    typora_assets_path: Option<&str>,
    now: &str,
    title: &str,
    article_type: ArticleType,
    draft: bool,
    force: bool,
) -> Result<Vec<String>> {
    let file_path = project_path.join(format!("{docs}/{slug}.{}", defaults::MARKDOWN_EXTENSION));
    let dir_path = project_path.join(format!("{docs}/{slug}"));
    let file_name = format!("{slug}.{}", defaults::MARKDOWN_EXTENSION);

    // Cross-shape conflict check
    if dir_path.exists() {
        return Err(MfError::ShapeConflict {
            wanted_shape: "file".to_string(),
            existing_shape: "directory".to_string(),
            path: dir_path,
        });
    }

    let backup_path = if file_path.exists() {
        if force {
            let backup_path = sibling_backup_path(&file_path);
            fs::rename(&file_path, &backup_path).map_err(MfError::Io)?;
            Some(backup_path)
        } else {
            return Err(MfError::file_exists(file_path));
        }
    } else {
        None
    };

    let content = if let Some(assets_path) = typora_assets_path {
        inject_typora_front_matter(content, assets_path)
    } else {
        content.to_string()
    };

    if let Err(e) = fs::write(&file_path, content).map_err(MfError::Io) {
        if let Some(backup_path) = &backup_path {
            let _ = fs::rename(backup_path, &file_path);
        }
        return Err(e);
    }

    let article_path = format!("{docs}/{file_name}");
    match write_index_entry(project_path, title, article_type, &article_path, now, draft, force) {
        Ok(()) => {
            if let Some(backup_path) = backup_path {
                let _ = fs::remove_file(backup_path);
            }
            Ok(vec![file_name])
        }
        Err(e) => {
            let _ = fs::remove_file(&file_path);
            if let Some(backup_path) = &backup_path {
                let _ = fs::rename(backup_path, &file_path);
            }
            Err(e)
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn write_article_directory(
    project_path: &Path,
    docs: &str,
    slug: &str,
    content: &str,
    typora_assets_path: Option<&str>,
    now: &str,
    title: &str,
    article_type: ArticleType,
    draft: bool,
    force: bool,
) -> Result<Vec<String>> {
    let dir_path = project_path.join(format!("{docs}/{slug}"));
    let file_path = project_path.join(format!("{docs}/{slug}.{}", defaults::MARKDOWN_EXTENSION));

    // Cross-shape conflict check
    if file_path.exists() {
        return Err(MfError::ShapeConflict {
            wanted_shape: "directory".to_string(),
            existing_shape: "file".to_string(),
            path: file_path,
        });
    }

    let backup_path = if dir_path.exists() {
        if force {
            let backup_path = sibling_backup_path(&dir_path);
            fs::rename(&dir_path, &backup_path).map_err(MfError::Io)?;
            Some(backup_path)
        } else {
            return Err(MfError::file_exists(dir_path));
        }
    } else {
        None
    };

    let mut blocks = split_template_into_blocks(content)?;
    if let Some(assets_path) = typora_assets_path {
        for (_, body) in &mut blocks {
            *body = inject_typora_front_matter(body, assets_path);
        }
    }
    let files: Vec<String> = blocks.iter().map(|(filename, _)| filename.clone()).collect();
    let block_refs: Vec<(&str, &str)> = blocks.iter().map(|(f, b)| (f.as_str(), b.as_str())).collect();
    if let Err(e) = util::atomic_write_directory(&dir_path, &block_refs) {
        if let Some(backup_path) = &backup_path {
            let _ = fs::rename(backup_path, &dir_path);
        }
        return Err(e);
    }

    let article_path = format!("{docs}/{slug}");
    match write_index_entry(project_path, title, article_type, &article_path, now, draft, force) {
        Ok(()) => {
            if let Some(backup_path) = backup_path {
                let _ = fs::remove_dir_all(backup_path);
            }
            Ok(files)
        }
        Err(e) => {
            let _ = fs::remove_dir_all(&dir_path);
            if let Some(backup_path) = &backup_path {
                let _ = fs::rename(backup_path, &dir_path);
            }
            Err(e)
        }
    }
}

fn write_index_entry(
    project_path: &Path,
    title: &str,
    article_type: ArticleType,
    article_path: &str,
    now: &str,
    draft: bool,
    force: bool,
) -> Result<()> {
    let project_name = util::dir_name(project_path);
    let mut index = index::load(project_path)?;
    let articles = index.articles.get_or_insert_with(Vec::new);
    let status = if draft { ArticleStatus::Draft } else { ArticleStatus::Published };

    if force {
        articles.retain(|a| a.article_path != article_path);
    }

    articles.push(Article {
        title: title.to_string(),
        project: project_name,
        article_type,
        article_path: article_path.to_string(),
        status,
        created_at: now.to_string(),
        updated_at: now.to_string(),
        template_origin: None,
    });
    index::save(project_path, &index)?;
    Ok(())
}

/// Build an in-memory index from all discovery sources.
///
/// Priority order (higher wins): declared > docs > templates.
/// Loads the existing index (tolerating missing), scans declared articles,
/// docs-declared articles and template-generated articles, merges results
/// (preserving metadata for articles that still exist). Unlike
/// [`refresh_index`], this does **not** persist the result.
pub fn build_index(project_root: &Path, config: &MindConfig) -> Result<IndexFile> {
    let existing = index::load(project_root)?;
    let existing_map: HashMap<&str, &Article> = existing
        .articles
        .as_ref()
        .map(|a| a.iter().map(|a| (a.article_path.as_str(), a)).collect())
        .unwrap_or_default();

    let mut articles: Vec<Article> = Vec::new();
    let mut covered: std::collections::HashSet<String> = std::collections::HashSet::new();
    let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
    let project_name = util::dir_name(project_root);
    let layout = config_svc::effective_layout(project_root)?;

    // Phase 1: Declared articles (from build.articles + compat articles)
    let declared = scan_declared(project_root, config)?;
    let mut declared_prefixes: Vec<String> = Vec::new();
    for da in &declared {
        covered.insert(da.article_path.clone());
        // Track directory-like article_paths for template dedup (FR-006)
        if !da.article_path.ends_with(&format!(".{}", defaults::MARKDOWN_EXTENSION)) {
            let prefix = da.article_path.trim_end_matches('/').to_string() + "/";
            declared_prefixes.push(prefix);
        }
        articles.push(fresh_or_existing(da, &existing_map, &now, &project_name));
    }

    // Phase 2: Docs scan
    let scanned = scan_docs(project_root)?;
    for s in scanned {
        let sp = article_path_for_scanned(&s, &layout.articles);
        if covered.contains(&sp) {
            continue;
        }
        covered.insert(sp.clone());
        if let Some(existing_article) = existing_map.get(sp.as_str()) {
            let mut article = (*existing_article).clone();
            article.article_path = sp;
            articles.push(article);
        } else {
            articles.push(Article {
                title: s.title,
                project: project_name.clone(),
                article_type: ArticleType::Blog,
                article_path: sp,
                status: ArticleStatus::Draft,
                created_at: now.clone(),
                updated_at: now.clone(),
                template_origin: None,
            });
        }
    }

    // Phase 3: Template-generated files
    let template_articles = scan_templates(project_root, config)?;
    for ta in template_articles {
        if covered.contains(&ta.article_path) {
            continue;
        }
        // FR-006: Skip if template file falls under a declared article's article_dir
        if declared_prefixes.iter().any(|p| ta.article_path.starts_with(p)) {
            continue;
        }
        covered.insert(ta.article_path.clone());
        if let Some(existing_article) = existing_map.get(ta.article_path.as_str()) {
            let mut article = (*existing_article).clone();
            article.template_origin = ta.template_origin;
            articles.push(article);
        } else {
            articles.push(ta);
        }
    }

    // Preserve existing articles not found by scanning (defined only in index)
    for (sp, article) in &existing_map {
        if !covered.contains(*sp) {
            articles.push((*article).clone());
        }
    }

    // Sort for deterministic output
    articles.sort_by(|a, b| a.article_path.cmp(&b.article_path));

    let index = IndexFile {
        schema_version: defaults::SCHEMA_VERSION.to_string(),
        articles: Some(articles),
        publish_records: existing.publish_records,
        sources: existing.sources,
        assets: existing.assets,
        terms: existing.terms,
        extra: existing.extra,
    };

    Ok(index)
}

/// Helper: reuse existing article metadata if available, otherwise create fresh.
fn fresh_or_existing(src: &Article, existing_map: &HashMap<&str, &Article>, now: &str, project_name: &str) -> Article {
    if let Some(existing) = existing_map.get(src.article_path.as_str()) {
        let mut article = (*existing).clone();
        article.article_path = src.article_path.clone();
        article.template_origin = src.template_origin.clone();
        article
    } else {
        Article {
            title: src.title.clone(),
            project: project_name.to_string(),
            article_type: ArticleType::Blog,
            article_path: src.article_path.clone(),
            status: ArticleStatus::Draft,
            created_at: now.to_string(),
            updated_at: now.to_string(),
            template_origin: None,
        }
    }
}

/// Rebuild the index by scanning docs/ and template patterns, then persist.
///
/// See [`build_index`] for the computation logic. This function additionally
/// writes the result to `mind-index.yaml`.
pub fn refresh_index(project_root: &Path, config: &MindConfig) -> Result<IndexFile> {
    let index = build_index(project_root, config)?;
    index::save(project_root, &index)?;
    Ok(index)
}

/// List articles in a project, rebuilding the index first.
pub fn list_articles(project_path: &Path) -> Result<Vec<Article>> {
    let config = config_svc::load_project(project_path, None)?
        .ok_or_else(|| MfError::not_found("mind.yaml not found".to_string(), None))?;
    let index = build_index(project_path, &config)?;
    Ok(index.articles.unwrap_or_default())
}

/// List articles across all projects in a repo, sorted by file modification time
/// (newest first). Returns tuples of (article, project_path, mtime_seconds).
pub fn list_articles_all_projects(repo_root: &Path) -> Result<Vec<(Article, PathBuf, u64)>> {
    let projects_dir = crate::service::repo::projects_dir_for(repo_root)?;
    let manifest_path = repo_root.join("minds.yaml");
    let manifest = if manifest_path.exists() { crate::service::repo::load_manifest(&manifest_path).ok() } else { None };

    let mut project_paths: Vec<PathBuf> = Vec::new();
    if let Some(ref m) = manifest {
        for entry in &m.projects {
            project_paths.push(repo_root.join(entry.path.trim_start_matches("./")));
        }
    }
    let scanned = crate::service::repo::scan_project_dirs(repo_root, &projects_dir);
    for sp in &scanned {
        let path = repo_root.join(sp.path.trim_start_matches("./"));
        if !project_paths.contains(&path) {
            project_paths.push(path);
        }
    }

    let mut results: Vec<(Article, PathBuf, u64)> = Vec::new();
    for project_path in &project_paths {
        if let Ok(articles) = list_articles(project_path) {
            for article in articles {
                let mtime = article_file_mtime(project_path, &article.article_path);
                results.push((article, project_path.clone(), mtime));
            }
        }
    }

    results.sort_by(|a, b| b.2.cmp(&a.2).then_with(|| a.0.article_path.cmp(&b.0.article_path)));
    Ok(results)
}

/// Get the newest file modification time (Unix epoch seconds) for an article.
fn article_file_mtime(project_path: &Path, article_path: &str) -> u64 {
    let full_path = project_path.join(article_path);
    match fs::metadata(&full_path) {
        Ok(meta) => {
            if meta.is_dir() {
                newest_file_mtime_in_dir(&full_path).unwrap_or(0)
            } else {
                meta_unixtime(&meta).unwrap_or(0)
            }
        }
        Err(_) => 0,
    }
}

fn newest_file_mtime_in_dir(dir: &Path) -> Option<u64> {
    let mut newest: Option<u64> = None;
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Ok(meta) = fs::metadata(&path) {
                let t = if meta.is_dir() { newest_file_mtime_in_dir(&path) } else { meta_unixtime(&meta) };
                if let Some(t) = t {
                    newest = Some(newest.map_or(t, |n| n.max(t)));
                }
            }
        }
    }
    newest
}

fn meta_unixtime(meta: &fs::Metadata) -> Option<u64> {
    meta.modified().ok().and_then(|t| t.duration_since(UNIX_EPOCH).ok()).map(|d| d.as_secs())
}

/// Derive the article key from its article_path.
///
/// Returns the full project-relative path without `.md` extension.
/// For `"docs/my-article.md"` → `"docs/my-article"`.
/// For `"outputs/2026-05/2026-05-15.md"` → `"outputs/2026-05/2026-05-15"`.
fn article_key_from_article_path(article_path: &str) -> String {
    article_path.strip_suffix(&format!(".{}", defaults::MARKDOWN_EXTENSION)).unwrap_or(article_path).to_string()
}

/// Resolve the docs-relative article path for a declared article key.
///
/// Checks existing paths in order:
/// 1. `docs/<key>/` directory
/// 2. `docs/<key>.md` file
/// 3. Falls back to `docs/<key>.md` (may not exist — caller diagnoses)
fn resolve_docs_article_path(project_root: &Path, key: &str, articles_dir: &str) -> String {
    let dir_path = project_root.join(articles_dir).join(key);
    if dir_path.is_dir() {
        return format!("{}/{}", articles_dir, key);
    }
    format!("{}/{}.{}", articles_dir, key, defaults::MARKDOWN_EXTENSION)
}

/// FR-003: warn on stderr when a declared article's resolved article path does
/// not exist on disk. The entry is still emitted (so `mf article list` shows
/// it), but the user is told to fix it.
fn warn_if_source_missing(project_root: &Path, id: &str, article_path: &str) {
    if !project_root.join(article_path).exists() {
        eprintln!("warning: declared article identity '{article_path}' has no content on disk (configured as '{id}')");
    }
}

/// Scan config for declared articles from `build.articles` (typed) and
/// compat top-level `articles` (Python mind 0.3.0).
///
/// Returns entries sorted by `<id>` lexicographically. Typed wins over compat
/// on `<id>` collision (FR-004). Entries whose `article_path` does not exist on
/// disk are still emitted using the conventional `docs/<key>.md` fallback so
/// they remain visible to `mf article list` (FR-003). `template_origin` is
/// always `None`.
///
/// Source-path inference order:
/// 1. Configured article_dir (if present): `<article_dir>/<key>.md` file, or
///    `<article_dir>/` directory, else fall back to docs convention.
/// 2. No article_dir configured: `docs/<key>/` directory before `docs/<key>.md`.
pub fn scan_declared(project_root: &Path, config: &MindConfig) -> Result<Vec<Article>> {
    let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
    let project_name = util::dir_name(project_root);
    let layout = config_svc::effective_layout(project_root)?;
    let mut seen: std::collections::BTreeMap<String, Article> = std::collections::BTreeMap::new();

    // 1. Typed build.articles (authoritative)
    for (id, cfg) in &config.build.articles {
        let article_path = if let Some(ref article_dir) = cfg.article_dir {
            let dir_path = project_root.join(article_dir);
            let file_name = format!("{}.{}", id, defaults::MARKDOWN_EXTENSION);
            if dir_path.join(&file_name).is_file() {
                format!("{article_dir}/{file_name}")
            } else if dir_path.is_dir() {
                article_dir.to_string()
            } else {
                resolve_docs_article_path(project_root, id, &layout.articles)
            }
        } else {
            resolve_docs_article_path(project_root, id, &layout.articles)
        };
        warn_if_source_missing(project_root, id, &article_path);

        seen.insert(
            id.clone(),
            Article {
                title: id.replace('-', " "),
                project: project_name.clone(),
                article_type: ArticleType::Blog,
                article_path,
                status: ArticleStatus::Draft,
                created_at: now.clone(),
                updated_at: now.clone(),
                template_origin: None,
            },
        );
    }

    // 2. Compat top-level articles (only for IDs not already seen)
    if let Some(serde_json::Value::Object(map)) = config.articles.as_ref() {
        for (id, value) in map {
            if seen.contains_key(id) {
                continue;
            }
            let article_path = match value {
                serde_json::Value::Object(obj) => {
                    if let Some(article_dir) = obj.get("article_dir").and_then(|v| v.as_str()) {
                        let dir_path = project_root.join(article_dir);
                        let file_name = format!("{}.{}", id, defaults::MARKDOWN_EXTENSION);
                        if dir_path.join(&file_name).is_file() {
                            format!("{article_dir}/{file_name}")
                        } else if dir_path.is_dir() {
                            article_dir.to_string()
                        } else {
                            resolve_docs_article_path(project_root, id, &layout.articles)
                        }
                    } else {
                        resolve_docs_article_path(project_root, id, &layout.articles)
                    }
                }
                _ => resolve_docs_article_path(project_root, id, &layout.articles),
            };
            warn_if_source_missing(project_root, id, &article_path);
            seen.insert(
                id.clone(),
                Article {
                    title: id.replace('-', " "),
                    project: project_name.clone(),
                    article_type: ArticleType::Blog,
                    article_path,
                    status: ArticleStatus::Draft,
                    created_at: now.clone(),
                    updated_at: now.clone(),
                    template_origin: None,
                },
            );
        }
    }

    Ok(seen.into_values().collect())
}

/// Compute the effective article directory for an article based on project config.
///
/// Returns the project-root-relative directory path:
/// - Article's configured `article_dir` in `build.articles[article_key]` if present
/// - Otherwise `docs/<article-name>` as the default
pub fn effective_article_dir(project_path: &Path, config: &MindConfig, article: &Article) -> String {
    let article_key = article_key_from_article_path(&article.article_path);
    let layout = config_svc::effective_layout(project_path).ok();
    let articles_dir = layout.as_ref().map(|l| l.articles.as_str()).unwrap_or(defaults::DOCS_DIR);
    let docs_prefix = format!("{articles_dir}/");
    let short_key = article_key.strip_prefix(&docs_prefix).unwrap_or(&article_key);
    if let Some(article_dir) = config.build.articles.get(short_key).and_then(|a| a.article_dir.clone()) {
        return article_dir;
    }
    if let Some(article_dir) = config
        .build
        .articles
        .values()
        .filter_map(|a| a.article_dir.as_ref())
        .find(|article_dir| article_dir.as_str() == article.article_path)
    {
        return article_dir.clone();
    }
    format!("{articles_dir}/{short_key}")
}

/// Scan the docs directory for markdown files and return discovered articles.
///
/// Scans the default docs directory and any configured `article_dir` directories
/// from `mind.yaml`'s `build.articles.*.article_dir`.
pub fn scan_docs(project_path: &Path) -> Result<Vec<ScannedArticle>> {
    let layout = config_svc::effective_layout(project_path)?;
    let mut scanned = Vec::new();

    // Scan default docs directory
    let docs_dir = project_path.join(&layout.articles);
    if docs_dir.exists() {
        scan_md_dir(&docs_dir, &layout.articles, &mut scanned)?;
    }

    // Scan configured article_dir directories from mind.yaml. Each
    // build.articles.<name>.article_dir entry represents one configured article
    // source, even when that directory contains several Markdown parts.
    if let Ok(Some(config)) = config_svc::load_project(project_path, None) {
        for (article_name, article_cfg) in &config.build.articles {
            if let Some(ref article_dir) = article_cfg.article_dir {
                let dir_path = project_path.join(article_dir);
                if dir_path.exists() && dir_path.is_dir() {
                    let article_path = configured_article_path(article_name, &dir_path, article_dir);
                    scanned.push(ScannedArticle {
                        title: article_name.replace('-', " "),
                        filename: article_name.clone(),
                        article_dir: Some(article_dir.clone()),
                        article_path: Some(article_path),
                    });
                }
            }
        }
    }

    // Deduplicate by article path (keep first occurrence)
    let mut seen = std::collections::HashSet::new();
    scanned.retain(|a| {
        let key = article_path_for_scanned(a, &layout.articles);
        seen.insert(key)
    });

    Ok(scanned)
}

/// Scan filesystem for files matching template patterns (US2).
///
/// Iterates `config.templates`, builds a `PathTemplate` + `PatternMatcher` for
/// each `Generated` mode template, walks the project root, and returns an
/// `Article` for every matched file.
pub fn scan_templates(project_root: &Path, config: &MindConfig) -> Result<Vec<Article>> {
    let templates = match config.templates.as_ref() {
        Some(t) => &t.items,
        None => return Ok(Vec::new()),
    };

    let mut articles = Vec::new();

    for (name, template) in templates {
        if !matches!(template.mode, TemplateMode::Generated) {
            continue;
        }

        let path_tmpl = PathTemplate::parse(&template.pattern)?;
        path_tmpl.validate_slot_redundancy().map_err(|e| {
            if let MfError::MultiSlotTemplate { .. } = &e {
                MfError::MultiSlotTemplate { template_name: name.clone() }
            } else {
                e
            }
        })?;
        let matcher = path_tmpl.compile_matcher();
        let now = Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
        let project_name = util::dir_name(project_root);

        for entry in walkdir::WalkDir::new(project_root)
            .into_iter()
            .filter_entry(|e| {
                let name = e.file_name().to_string_lossy();
                !name.starts_with('.')
            })
            .filter_map(|e| e.ok())
        {
            if !entry.file_type().is_file() {
                continue;
            }
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some(defaults::MARKDOWN_EXTENSION) {
                continue;
            }
            let rel_path = path.strip_prefix(project_root).unwrap_or(path);
            if let Some(pm) = matcher.try_match(rel_path) {
                let slot_value = pm.most_specific_slot_value;
                let article_id = format!("{}/{}", name, slot_value);
                articles.push(Article {
                    title: article_id.clone(),
                    project: project_name.clone(),
                    article_type: ArticleType::Blog,
                    article_path: rel_path.to_string_lossy().to_string(),
                    status: ArticleStatus::Draft,
                    created_at: now.clone(),
                    updated_at: now.clone(),
                    template_origin: Some(TemplateOrigin { template_name: name.clone(), slot_value }),
                });
            }
        }
    }

    // Sort for deterministic order
    articles.sort_by(|a, b| a.article_path.cmp(&b.article_path));

    Ok(articles)
}

fn configured_article_path(article_name: &str, dir_path: &Path, article_dir: &str) -> String {
    let file_name = format!("{article_name}.{}", defaults::MARKDOWN_EXTENSION);
    if dir_path.join(&file_name).is_file() {
        format!("{article_dir}/{file_name}")
    } else {
        article_dir.to_string()
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
                    article_dir: Some(rel_dir.to_string()),
                    article_path: None,
                });
            }
        }
    }
    Ok(())
}

/// Build the project-relative article path for a scanned article.
fn article_path_for_scanned(a: &ScannedArticle, articles_dir: &str) -> String {
    if let Some(ref article_path) = a.article_path {
        return article_path.clone();
    }
    match a.article_dir {
        Some(ref dir) => format!("{}/{}.{}", dir, a.filename, defaults::MARKDOWN_EXTENSION),
        None => format!("{}/{}.{}", articles_dir, a.filename, defaults::MARKDOWN_EXTENSION),
    }
}

/// Compare the index against a filesystem scan to find added/removed articles.
///
/// `docs_dir` is the configured docs directory name (e.g. "docs").
pub fn compute_article_diff(index: &IndexFile, scanned: &[ScannedArticle], docs_dir: &str) -> ArticleDiff {
    let mut added = Vec::new();
    let mut removed = Vec::new();

    // Build set of scanned article_paths (project-relative)
    let scanned_paths: std::collections::HashSet<String> =
        scanned.iter().map(|s| article_path_for_scanned(s, docs_dir)).collect();

    // Build set of scanned filenames for the legacy fallback check
    let scanned_filenames: std::collections::HashSet<&str> = scanned.iter().map(|s| s.filename.as_str()).collect();

    // Removed: articles in index whose article_path no longer has a matching file
    for a in index.articles.iter().flat_map(|a| a.iter()) {
        if !scanned_paths.contains(&a.article_path) {
            // For articles in the default docs dir, also check via the old
            // filename-based method (strip docs/ prefix + .md extension)
            let docs_prefix = format!("{docs_dir}/");
            let in_docs = a.article_path.starts_with(&docs_prefix);
            let matched = if in_docs {
                a.article_path
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
        let sp = article_path_for_scanned(s, docs_dir);
        let exists = index.articles.as_ref().is_some_and(|articles| articles.iter().any(|a| a.article_path == sp));
        if !exists {
            added.push(s.clone());
        }
    }

    ArticleDiff { added, removed }
}

/// Apply a diff to the index: add new articles, remove deleted ones.
pub fn reconcile_articles(project_path: &Path, mut index: IndexFile, diff: ArticleDiff) -> Result<IndexFile> {
    let project_name = util::dir_name(project_path);
    let layout = config_svc::effective_layout(project_path)?;

    // Remove deleted articles
    if let Some(ref mut articles) = index.articles {
        let remove_paths: std::collections::HashSet<String> =
            diff.removed.iter().map(|a| a.article_path.clone()).collect();
        articles.retain(|a| !remove_paths.contains(&a.article_path));
    }

    // Add new articles
    let articles = index.articles.get_or_insert_with(Vec::new);
    for a in &diff.added {
        let now = Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
        let article_path = if a.article_dir.is_some() {
            article_path_for_scanned(a, &layout.articles)
        } else {
            // Defensive: article_path_for_scanned falls back to the configured articles dir,
            // but we use layout.articles from config which may differ. This branch is
            // currently unreachable (scan_md_dir always sets article_dir), kept
            // for correctness if a future caller produces ScannedArticle without
            // a article_dir.
            format!("{}/{}.{}", layout.articles, a.filename, defaults::MARKDOWN_EXTENSION)
        };
        articles.push(Article {
            title: a.title.clone(),
            project: project_name.clone(),
            article_type: ArticleType::Blog,
            article_path,
            status: ArticleStatus::Draft,
            created_at: now.clone(),
            updated_at: now,
            template_origin: None,
        });
    }

    Ok(index)
}

/// Lint articles in the project: check filenames and content quality.
/// When `fix` is true, auto-fix fixable issues.
pub fn lint_articles(project_path: &Path, fix: bool) -> Result<Vec<LintIssue>> {
    let layout = config_svc::effective_layout(project_path)?;
    let docs_dir = project_path.join(&layout.articles);
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
            let rel_path = format!("{}/{}.{}", layout.articles, stem, defaults::MARKDOWN_EXTENSION);

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
    pub old_article_path: String,
    pub new_article_path: String,
    #[serde(default)]
    pub references: Vec<crate::model::lifecycle::Reference>,
    #[serde(default)]
    pub side_effects: Vec<crate::model::lifecycle::PlannedChange>,
    #[serde(default)]
    pub force: bool,
    #[serde(default)]
    pub dry_run: bool,
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
    let layout = config_svc::effective_layout(project_path)?;

    // Load the index and find the article by title
    let mut index = index::load(project_path)?;
    let articles = index.articles.as_mut().ok_or_else(|| {
        MfError::not_found(
            format!("article '{old_title}' not found"),
            Some("use `mf article list --project <project>` to see available articles".to_string()),
        )
    })?;

    let article =
        articles.iter_mut().find(|a| a.title == old_title || a.article_path == old_title).ok_or_else(|| {
            MfError::not_found(
                format!("article '{old_title}' not found"),
                Some("use `mf article list --project <project>` to see available articles".to_string()),
            )
        })?;

    let old_article_path = article.article_path.clone();
    let new_article_path = format!("{}/{}.{}", layout.articles, new_filename, defaults::MARKDOWN_EXTENSION);

    // Rename the file on disk (only if the path actually differs)
    if old_article_path != new_article_path {
        let old_full = project_path.join(&old_article_path);
        let new_full = project_path.join(&new_article_path);

        if !old_full.exists() {
            return Err(MfError::not_found(
                format!("article file not found at {}", old_full.display()),
                Some("the index may be out of date; try `mf article index`".to_string()),
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
    article.article_path = new_article_path.clone();
    article.updated_at = now;
    index::save(project_path, &index)?;

    Ok(ArticleRenameReport {
        old_title: old_title.to_string(),
        new_title: new_title.to_string(),
        old_article_path,
        new_article_path,
        references: vec![],
        side_effects: vec![],
        force,
        dry_run: false,
    })
}

#[derive(Debug, Clone)]
pub struct ArticleUpdate<'a> {
    pub selector: &'a str,
    pub status: Option<ArticleStatus>,
    pub dry_run: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct ArticleUpdateReport {
    pub article: Article,
    pub dry_run: bool,
    pub changes: serde_json::Value,
}

pub fn update_article(project_path: &Path, update: ArticleUpdate<'_>) -> Result<ArticleUpdateReport> {
    if update.status.is_none() {
        return Err(MfError::usage(
            "nothing to update: use --status",
            Some("pass --status draft or --status published".to_string()),
        ));
    }

    let mut index_file = index::load(project_path)?;
    let articles = index_file.articles.as_mut().ok_or_else(|| article_not_found(update.selector))?;
    let article = articles
        .iter_mut()
        .find(|a| a.title == update.selector || a.article_path == update.selector)
        .ok_or_else(|| article_not_found(update.selector))?;

    let mut changes = serde_json::Map::new();
    if let Some(status) = update.status.clone() {
        changes.insert("status".to_string(), serde_json::json!({"from": article.status, "to": status}));
        if !update.dry_run {
            article.status = status;
            article.updated_at = Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
        }
    }

    let updated = article.clone();
    if !update.dry_run {
        index::save(project_path, &index_file)?;
    }

    Ok(ArticleUpdateReport { article: updated, dry_run: update.dry_run, changes: serde_json::Value::Object(changes) })
}

fn article_not_found(selector: &str) -> MfError {
    MfError::not_found(
        format!("article '{selector}' not found"),
        Some("use `mf article list --project <project>` to see available articles".to_string()),
    )
}

// ── Article remove ────────────────────────────────────────────────────────

use crate::model::article::{ArticleIdentity, ArticleRemoveReport};

/// Hard-remove an article: delete the file/directory and update the index.
pub fn remove_article(project_path: &Path, title: &str, force: bool, dry_run: bool) -> Result<ArticleRemoveReport> {
    crate::service::util::require_nonempty(title, "article title")?;

    let mut index = index::load(project_path)?;
    let articles = index.articles.as_ref().ok_or_else(|| {
        MfError::not_found(
            format!("article '{title}' not found"),
            Some("use `mf article list --project <project>` to see available articles".to_string()),
        )
    })?;

    let article = articles.iter().find(|a| a.title == title || a.article_path == title).ok_or_else(|| {
        MfError::not_found(
            format!("article '{title}' not found"),
            Some("use `mf article list --project <project>` to see available articles".to_string()),
        )
    })?;

    let scope = crate::model::lifecycle::ScopeRef { project: Some(article.project.clone()), global: false };
    let before = ArticleIdentity { title: article.title.clone(), article_path: article.article_path.clone(), scope };

    // Reference scan (articles reference other objects, not typically referenced themselves)
    let refs: Vec<crate::model::lifecycle::Reference> = Vec::new();

    let mut planned: Vec<crate::model::lifecycle::PlannedChange> = Vec::new();
    let abs_path = project_path.join(&article.article_path);
    if abs_path.is_dir() {
        planned.push(crate::model::lifecycle::PlannedChange {
            op: crate::model::lifecycle::PlannedOp::RemoveDir,
            path: abs_path.to_string_lossy().to_string(),
            old: Some(article.title.clone()),
            new: None,
        });
    } else if abs_path.exists() {
        planned.push(crate::model::lifecycle::PlannedChange {
            op: crate::model::lifecycle::PlannedOp::RemoveFile,
            path: abs_path.to_string_lossy().to_string(),
            old: Some(article.title.clone()),
            new: None,
        });
    }
    planned.push(crate::model::lifecycle::PlannedChange {
        op: crate::model::lifecycle::PlannedOp::UpdateYaml,
        path: project_path.join("mind-index.yaml").to_string_lossy().to_string(),
        old: Some(article.title.clone()),
        new: None,
    });
    planned.push(crate::model::lifecycle::PlannedChange {
        op: crate::model::lifecycle::PlannedOp::RefreshIndex,
        path: project_path.join("mind-index.yaml").to_string_lossy().to_string(),
        old: None,
        new: None,
    });

    if dry_run {
        return Ok(ArticleRemoveReport {
            verb: "remove".into(),
            kind: "article".into(),
            before,
            after: None,
            references: refs,
            side_effects: planned,
            force,
            dry_run: true,
        });
    }

    // Remove file/directory from disk
    if abs_path.is_dir() {
        fs::remove_dir_all(&abs_path).map_err(MfError::Io)?;
    } else if abs_path.exists() {
        fs::remove_file(&abs_path).map_err(MfError::Io)?;
    }

    // Remove from index
    {
        let articles = index.articles.as_mut().expect("already checked");
        articles.retain(|a| a.title != title);
    }
    index::save(project_path, &index)?;

    Ok(ArticleRemoveReport {
        verb: "remove".into(),
        kind: "article".into(),
        before,
        after: None,
        references: refs,
        side_effects: planned,
        force,
        dry_run: false,
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

// ── Typora front-matter helpers ──

/// Returns true unless the Typora plugin is explicitly disabled.
pub fn effective_typora_enabled(plugins: Option<&crate::model::config::PluginsConfig>) -> bool {
    plugins.and_then(|p| p.typora_front_matter.as_ref()).and_then(|t| t.enabled).unwrap_or(true)
}

/// Compute the value for `typora-copy-images-to`.
///
/// - Absolute `assets` → emitted unchanged.
/// - Relative `assets` → POSIX relative path from `file_dir` to
///   `project_path.join(assets)`.
pub fn compute_typora_assets_path(project_path: &Path, assets: &str, file_dir: &Path) -> String {
    if assets.starts_with('/') {
        return assets.to_string();
    }
    let assets_abs = project_path.join(assets);
    relative_path_from(file_dir, &assets_abs)
}

/// POSIX-style relative path from `from_dir` to `to_dir`.
fn relative_path_from(from_dir: &Path, to_dir: &Path) -> String {
    // Canonicalize both if possible; fall back to the input paths.
    let from = from_dir.canonicalize().unwrap_or_else(|_| from_dir.to_path_buf());
    let to = to_dir.canonicalize().unwrap_or_else(|_| to_dir.to_path_buf());

    let mut from_components: Vec<&std::ffi::OsStr> = from
        .components()
        .filter_map(|c| match c {
            std::path::Component::Normal(p) => Some(p),
            _ => None,
        })
        .collect();
    let mut to_components: Vec<&std::ffi::OsStr> = to
        .components()
        .filter_map(|c| match c {
            std::path::Component::Normal(p) => Some(p),
            _ => None,
        })
        .collect();

    // Strip common prefix
    let common = from_components.iter().zip(to_components.iter()).take_while(|(f, t)| f == t).count();
    from_components.drain(..common);
    to_components.drain(..common);

    let mut result = Vec::new();
    for _ in &from_components {
        result.push("..".to_string());
    }
    for c in &to_components {
        result.push(c.to_string_lossy().to_string());
    }

    if result.is_empty() {
        ".".to_string()
    } else {
        result.join("/")
    }
}

/// Inject or merge `typora-copy-images-to` into an initial YAML front-matter block.
///
/// - No front matter → prepends a new `---` block.
/// - Existing front matter with `typora-copy-images-to` → returns unchanged.
/// - Existing front matter without the key → inserts the key before the closing `---`.
pub fn inject_typora_front_matter(content: &str, assets_path: &str) -> String {
    let key_line = format!("typora-copy-images-to: {}", assets_path);

    if let Some((front, body, eol)) = split_initial_yaml_front_matter(content) {
        if front.lines().any(is_typora_copy_images_to_line) {
            return content.to_string();
        }

        let mut merged = String::new();
        merged.push_str("---");
        merged.push_str(eol);
        merged.push_str(front);
        if !front.is_empty() && !front.ends_with('\n') {
            merged.push_str(eol);
        }
        merged.push_str(&key_line);
        merged.push_str(eol);
        merged.push_str("---");
        merged.push_str(eol);
        merged.push_str(body);
        return merged;
    }

    // No front matter block
    format!("---\n{}\n---\n\n{}", key_line, content)
}

fn split_initial_yaml_front_matter(content: &str) -> Option<(&str, &str, &'static str)> {
    let (opening_len, eol) = if content.starts_with("---\r\n") {
        (5, "\r\n")
    } else if content.starts_with("---\n") {
        (4, "\n")
    } else {
        return None;
    };

    let remaining = &content[opening_len..];
    let mut offset = 0;
    for line in remaining.split_inclusive('\n') {
        let line_body = line.trim_end_matches(['\r', '\n']);
        let next_offset = offset + line.len();
        if line_body == "---" {
            let front = &remaining[..offset];
            let body = &remaining[next_offset..];
            return Some((front, body, eol));
        }
        offset = next_offset;
    }

    let trailing = &remaining[offset..];
    if trailing == "---" {
        let front = &remaining[..offset];
        return Some((front, "", eol));
    }

    None
}

fn is_typora_copy_images_to_line(line: &str) -> bool {
    let line = line.trim_start();
    line.starts_with("typora-copy-images-to:")
}

// ── Article shape conversion ───────────────────────────────────────────────

/// Derive the target single-file path from a directory article path.
///
/// `docs/daily` → `docs/daily.md`
pub fn derive_target_single_file_path(source_dir: &str) -> String {
    format!("{}.{}", source_dir, defaults::MARKDOWN_EXTENSION)
}

/// Derive the target directory path from a single-file article path.
///
/// `docs/daily.md` → `docs/daily`
pub fn derive_target_directory_path(source_file: &str) -> String {
    source_file.strip_suffix(&format!(".{}", defaults::MARKDOWN_EXTENSION)).unwrap_or(source_file).to_string()
}

/// Derive the target opening section path for a directory article.
///
/// `docs/daily` → `docs/daily/01-opening.md`
pub fn derive_opening_section_path(target_dir: &str) -> String {
    format!("{}/{}", target_dir, defaults::OPENING_SECTION_FILENAME)
}

/// Classify the on-disk shape of an article.
pub fn classify_article_shape(project_root: &Path, article_path: &str) -> ArticleShape {
    let full = project_root.join(article_path);
    if full.is_dir() {
        ArticleShape::Directory
    } else {
        ArticleShape::SingleFile
    }
}

/// List markdown section files in a directory article, sorted by name.
/// Returns the project-relative paths (e.g. `docs/daily/01-opening.md`).
pub fn list_section_files(project_root: &Path, article_path: &str) -> Result<Vec<String>> {
    let dir = project_root.join(article_path);
    let mut files: Vec<String> = Vec::new();
    for entry in std::fs::read_dir(&dir).map_err(MfError::Io)? {
        let entry = entry.map_err(MfError::Io)?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some(defaults::MARKDOWN_EXTENSION) {
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                files.push(format!("{}/{}", article_path, name));
            }
        }
    }
    files.sort();
    Ok(files)
}

/// Check if a directory article has non-section files that would prevent safe deletion.
pub fn has_extra_files(project_root: &Path, article_path: &str) -> Result<bool> {
    let dir = project_root.join(article_path);
    for entry in std::fs::read_dir(&dir).map_err(MfError::Io)? {
        let entry = entry.map_err(MfError::Io)?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some(defaults::MARKDOWN_EXTENSION) {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Candidate inspection result for a single direction.
pub struct CandidateInspection {
    pub eligible: bool,
    pub skip_reason: Option<String>,
    pub source_shape: ArticleShape,
    pub source_path: String,
    pub source_content_path: String,
    pub target_path: String,
    pub target_content_path: String,
}

impl CandidateInspection {
    /// Build a `ConversionResult` carrying this inspection's paths and shapes.
    pub fn to_result(
        &self,
        status: ConversionStatus,
        direction: ConversionDirection,
        reason: Option<String>,
        index_updated: bool,
        source_removed: bool,
    ) -> ConversionResult {
        ConversionResult {
            status,
            direction,
            source_shape: self.source_shape,
            target_shape: direction.target_shape(),
            source_path: self.source_path.clone(),
            source_content_path: self.source_content_path.clone(),
            target_path: self.target_path.clone(),
            target_content_path: self.target_content_path.clone(),
            reason,
            index_updated,
            source_removed,
        }
    }
}

/// Build an ineligible inspection sharing the common path/shape boilerplate.
fn ineligible(
    reason: &'static str,
    source_shape: ArticleShape,
    source_path: &str,
    source_content_path: String,
    target_path: String,
    target_content_path: String,
) -> CandidateInspection {
    CandidateInspection {
        eligible: false,
        skip_reason: Some(reason.to_string()),
        source_shape,
        source_path: source_path.to_string(),
        source_content_path,
        target_path,
        target_content_path,
    }
}

/// Inspect a single article for a given conversion direction and return eligibility.
pub fn inspect_candidate(
    project_root: &Path,
    article_path: &str,
    direction: ConversionDirection,
) -> Result<CandidateInspection> {
    let source_shape = classify_article_shape(project_root, article_path);

    match direction {
        ConversionDirection::ToSingleFile => inspect_to_single_file(project_root, article_path, source_shape),
        ConversionDirection::ToDirectory => inspect_to_directory(project_root, article_path, source_shape),
    }
}

fn inspect_to_single_file(
    project_root: &Path,
    article_path: &str,
    source_shape: ArticleShape,
) -> Result<CandidateInspection> {
    let target_path = derive_target_single_file_path(article_path);
    let target_content_path = target_path.clone();
    let bail = |reason, content_path: String| {
        ineligible(reason, source_shape, article_path, content_path, target_path.clone(), target_content_path.clone())
    };

    if source_shape != ArticleShape::Directory {
        return Ok(bail(skip_reason::NOT_DIRECTORY_ARTICLE, article_path.to_string()));
    }

    let section_files = list_section_files(project_root, article_path)?;
    if section_files.is_empty() {
        return Ok(bail(skip_reason::NO_SECTION_FILES, article_path.to_string()));
    }
    if section_files.len() > 1 {
        return Ok(bail(skip_reason::MULTIPLE_SECTION_FILES, article_path.to_string()));
    }
    let section = section_files.into_iter().next().expect("section_files has exactly one entry");

    if project_root.join(&target_path).exists() {
        return Ok(bail(skip_reason::TARGET_EXISTS, section));
    }
    if has_extra_files(project_root, article_path)? {
        return Ok(bail(skip_reason::EXTRA_FILES, section));
    }

    Ok(CandidateInspection {
        eligible: true,
        skip_reason: None,
        source_shape,
        source_path: article_path.to_string(),
        source_content_path: section,
        target_path,
        target_content_path,
    })
}

fn inspect_to_directory(
    project_root: &Path,
    article_path: &str,
    source_shape: ArticleShape,
) -> Result<CandidateInspection> {
    let target_dir = derive_target_directory_path(article_path);
    let target_content_path = derive_opening_section_path(&target_dir);
    let bail = |reason| {
        ineligible(
            reason,
            source_shape,
            article_path,
            article_path.to_string(),
            target_dir.clone(),
            target_content_path.clone(),
        )
    };

    if source_shape != ArticleShape::SingleFile {
        return Ok(bail(skip_reason::NOT_SINGLE_FILE_ARTICLE));
    }
    if project_root.join(&target_dir).exists() {
        return Ok(bail(skip_reason::TARGET_EXISTS));
    }

    Ok(CandidateInspection {
        eligible: true,
        skip_reason: None,
        source_shape,
        source_path: article_path.to_string(),
        source_content_path: article_path.to_string(),
        target_path: target_dir,
        target_content_path,
    })
}

/// Determine plausible directions and per-direction eligible counts for a set
/// of articles, in stable (ToSingleFile, ToDirectory) order. Directions with
/// zero eligible candidates are omitted.
pub fn plausible_directions(
    project_root: &Path,
    article_paths: &[String],
) -> Result<Vec<(ConversionDirection, usize)>> {
    let mut to_single = 0usize;
    let mut to_directory = 0usize;
    for ap in article_paths {
        let shape = classify_article_shape(project_root, ap);
        match shape {
            ArticleShape::Directory => {
                if inspect_to_single_file(project_root, ap, shape)?.eligible {
                    to_single += 1;
                }
            }
            ArticleShape::SingleFile => {
                if inspect_to_directory(project_root, ap, shape)?.eligible {
                    to_directory += 1;
                }
            }
        }
    }
    let mut directions = Vec::new();
    if to_single > 0 {
        directions.push((ConversionDirection::ToSingleFile, to_single));
    }
    if to_directory > 0 {
        directions.push((ConversionDirection::ToDirectory, to_directory));
    }
    Ok(directions)
}

/// Plan conversion: inspect all article paths for a given direction, return
/// results sorted by source_path.
pub fn plan_conversion(
    project_root: &Path,
    article_paths: &[String],
    direction: ConversionDirection,
) -> Result<Vec<CandidateInspection>> {
    let mut results = Vec::new();
    for ap in article_paths {
        results.push(inspect_candidate(project_root, ap, direction)?);
    }
    results.sort_by(|a, b| a.source_path.cmp(&b.source_path));
    Ok(results)
}

/// Execute to_single_file conversion: write target file, remove source directory.
/// Index update is the caller's responsibility.
pub fn execute_to_single_file(project_root: &Path, inspection: &CandidateInspection) -> Result<ConversionResult> {
    let source_content =
        std::fs::read_to_string(project_root.join(&inspection.source_content_path)).map_err(MfError::Io)?;

    let target_abs = project_root.join(&inspection.target_content_path);
    if let Some(parent) = target_abs.parent() {
        std::fs::create_dir_all(parent).map_err(MfError::Io)?;
    }
    std::fs::write(&target_abs, &source_content).map_err(MfError::Io)?;

    std::fs::remove_dir_all(project_root.join(&inspection.source_path)).map_err(MfError::Io)?;

    Ok(inspection.to_result(ConversionStatus::Converted, ConversionDirection::ToSingleFile, None, false, true))
}

/// Execute to_directory conversion: write target directory with the opening
/// section, remove the source file. Index update is the caller's responsibility.
pub fn execute_to_directory(project_root: &Path, inspection: &CandidateInspection) -> Result<ConversionResult> {
    let source_content =
        std::fs::read_to_string(project_root.join(&inspection.source_content_path)).map_err(MfError::Io)?;

    let target_dir = project_root.join(&inspection.target_path);
    util::atomic_write_directory(&target_dir, &[(defaults::OPENING_SECTION_FILENAME, &source_content)])?;

    std::fs::remove_file(project_root.join(&inspection.source_path)).map_err(MfError::Io)?;

    Ok(inspection.to_result(ConversionStatus::Converted, ConversionDirection::ToDirectory, None, false, true))
}

/// Update the index for a converted article. No-op (still `Ok`) if the old
/// path is not present — the caller already trusts the planning pass.
pub fn update_index_for_conversion(project_root: &Path, old_article_path: &str, new_article_path: &str) -> Result<()> {
    let mut index = index::load(project_root)?;
    if let Some(ref mut articles) = index.articles {
        for article in articles.iter_mut() {
            if article.article_path == old_article_path {
                article.article_path = new_article_path.to_string();
                article.updated_at = Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
                break;
            }
        }
    }
    index::save(project_root, &index)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::config::{ArticleBuildConfig, BuildConfig};
    use std::collections::HashMap;

    // ── T011: Typora front-matter helper unit tests ──

    #[test]
    fn inject_no_front_matter_prepend() {
        let content = "# Title\n\nBody\n";
        let result = inject_typora_front_matter(content, "../assets");
        assert!(result.starts_with("---\n"));
        assert!(result.contains("typora-copy-images-to: ../assets\n"));
        assert!(result.contains("# Title"));
    }

    #[test]
    fn inject_existing_front_matter_merge() {
        let content = "---\ntitle: Test\n---\n# Body\n";
        let result = inject_typora_front_matter(content, "../assets");
        // Should have exactly one --- block
        assert_eq!(result.matches("---").count(), 2, "should have exactly one opening and one closing ---");
        assert!(result.contains("typora-copy-images-to: ../assets\n"));
        assert!(result.contains("title: Test\n"));
        assert!(result.contains("# Body"));
    }

    #[test]
    fn inject_existing_typora_value_preserved() {
        let content = "---\ntitle: Test\ntypora-copy-images-to: ../../media\n---\n# Body\n";
        let result = inject_typora_front_matter(content, "../assets");
        // The existing value should be preserved (not our injected arg)
        assert!(result.contains("typora-copy-images-to: ../../media\n"));
        assert!(!result.contains("typora-copy-images-to: ../assets\n"));
    }

    #[test]
    fn inject_existing_front_matter_with_crlf_merge() {
        let content = "---\r\ntitle: Test\r\n---\r\n# Body\r\n";
        let result = inject_typora_front_matter(content, "../assets");
        assert!(result.starts_with("---\r\n"));
        assert!(result.contains("title: Test\r\n"));
        assert!(result.contains("typora-copy-images-to: ../assets\r\n"));
        assert!(result.contains("---\r\n# Body\r\n"));
    }

    #[test]
    fn inject_comment_mentioning_typora_key_does_not_count_as_existing_value() {
        let content = "---\n# typora-copy-images-to: old\ntitle: Test\n---\n# Body\n";
        let result = inject_typora_front_matter(content, "../assets");
        assert!(result.contains("# typora-copy-images-to: old\n"));
        assert!(result.contains("typora-copy-images-to: ../assets\n"));
    }

    #[test]
    fn compute_path_absolute() {
        let result = compute_typora_assets_path(
            Path::new("/Users/me/proj"),
            "/static/images",
            Path::new("/Users/me/proj/docs/post"),
        );
        assert_eq!(result, "/static/images");
    }

    #[test]
    fn compute_path_root_relative() {
        // For a file in docs/, assets as "assets" relative to project root = ../assets from docs/
        let tmp = tempfile::tempdir().unwrap();
        let assets_dir = tmp.path().join("assets");
        std::fs::create_dir_all(&assets_dir).unwrap();
        let docs_dir = tmp.path().join("docs");
        std::fs::create_dir_all(&docs_dir).unwrap();
        let result = compute_typora_assets_path(tmp.path(), "assets", &docs_dir);
        assert_eq!(result, "../assets");
    }

    #[test]
    fn compute_path_directory_article_relative() {
        let tmp = tempfile::tempdir().unwrap();
        let assets_dir = tmp.path().join("assets");
        std::fs::create_dir_all(&assets_dir).unwrap();
        let post_dir = tmp.path().join("docs/my-post");
        std::fs::create_dir_all(&post_dir).unwrap();
        let result = compute_typora_assets_path(tmp.path(), "assets", &post_dir);
        assert_eq!(result, "../../assets");
    }

    #[test]
    fn compute_path_single_file_relative() {
        let tmp = tempfile::tempdir().unwrap();
        let assets_dir = tmp.path().join("assets");
        std::fs::create_dir_all(&assets_dir).unwrap();
        let docs_dir = tmp.path().join("docs");
        std::fs::create_dir_all(&docs_dir).unwrap();
        let result = compute_typora_assets_path(tmp.path(), "assets", &docs_dir);
        assert_eq!(result, "../assets");
    }

    #[test]
    fn effective_enabled_default() {
        assert!(effective_typora_enabled(None));
    }

    #[test]
    fn effective_enabled_missing_plugin() {
        let plugins = crate::model::config::PluginsConfig::default();
        assert!(effective_typora_enabled(Some(&plugins)));
    }

    #[test]
    fn effective_enabled_explicit_true() {
        let plugins = crate::model::config::PluginsConfig {
            typora_front_matter: Some(crate::model::config::TyporaFrontMatterPluginConfig {
                enabled: Some(true),
                extra: serde_yaml::Mapping::new(),
            }),
            ..Default::default()
        };
        assert!(effective_typora_enabled(Some(&plugins)));
    }

    #[test]
    fn effective_enabled_explicit_false() {
        let plugins = crate::model::config::PluginsConfig {
            typora_front_matter: Some(crate::model::config::TyporaFrontMatterPluginConfig {
                enabled: Some(false),
                extra: serde_yaml::Mapping::new(),
            }),
            ..Default::default()
        };
        assert!(!effective_typora_enabled(Some(&plugins)));
    }

    /// Helper: build a MindConfig with typed build.articles entries.
    fn config_with_typed(entries: Vec<(&str, Option<&str>)>) -> MindConfig {
        let mut articles: HashMap<String, ArticleBuildConfig> = HashMap::new();
        for (id, article_dir) in entries {
            articles.insert(id.to_string(), ArticleBuildConfig { article_dir: article_dir.map(|s| s.to_string()) });
        }
        MindConfig { build: BuildConfig { articles, ..Default::default() }, ..Default::default() }
    }

    /// Helper: build a MindConfig with compat top-level articles entries.
    fn config_with_compat(entries: Vec<(&str, Option<&str>)>) -> MindConfig {
        let mut map = serde_json::Map::new();
        for (id, article_dir) in entries {
            let entry = match article_dir {
                Some(dir) => serde_json::json!({"article_dir": dir}),
                None => serde_json::json!({"type": "blog"}),
            };
            map.insert(id.to_string(), entry);
        }
        MindConfig { articles: Some(serde_json::Value::Object(map)), ..Default::default() }
    }

    #[test]
    fn scan_declared_typed_only_returns_typed_id_with_article_dir_resolved_path() {
        let config = config_with_typed(vec![("reports", Some("reports"))]);
        let dir = tempfile::tempdir().unwrap();
        let articles = scan_declared(dir.path(), &config).unwrap();
        assert_eq!(articles.len(), 1, "should return one declared article");
        assert_eq!(articles[0].title, "reports");
        assert_eq!(
            articles[0].article_path, "docs/reports.md",
            "falls back to docs/{{id}}.md when no file exists in article_dir"
        );
        assert!(articles[0].template_origin.is_none());
    }

    #[test]
    fn scan_declared_compat_only_returns_compat_id_with_docs_fallback() {
        let config = config_with_compat(vec![("legacy-blog", None)]);
        let dir = tempfile::tempdir().unwrap();
        let articles = scan_declared(dir.path(), &config).unwrap();
        assert_eq!(articles.len(), 1, "should return one compat article");
        assert_eq!(articles[0].title, "legacy blog");
        assert_eq!(articles[0].article_path, "docs/legacy-blog.md");
    }

    #[test]
    fn scan_declared_typed_wins_over_compat_on_id_collision() {
        let typed_cfg = ArticleBuildConfig { article_dir: Some("typed-path".to_string()) };
        let mut typed: HashMap<String, ArticleBuildConfig> = HashMap::new();
        typed.insert("shared".to_string(), typed_cfg);
        let mut compat_map = serde_json::Map::new();
        compat_map.insert("shared".to_string(), serde_json::json!({"article_dir": "compat-path"}));
        let config = MindConfig {
            build: BuildConfig { articles: typed, ..Default::default() },
            articles: Some(serde_json::Value::Object(compat_map)),
            ..Default::default()
        };
        let dir = tempfile::tempdir().unwrap();
        let articles = scan_declared(dir.path(), &config).unwrap();
        assert_eq!(articles.len(), 1, "typed wins should produce exactly one article");
        assert_eq!(
            articles[0].article_path, "docs/shared.md",
            "typed article without article file falls back to docs/{{id}}.md"
        );
    }

    #[test]
    fn scan_declared_sorts_alphabetically_by_id() {
        let config = config_with_typed(vec![
            ("zulu", Some("zulu/src")),
            ("alpha", Some("alpha/src")),
            ("beta", Some("beta/src")),
        ]);
        let dir = tempfile::tempdir().unwrap();
        let articles = scan_declared(dir.path(), &config).unwrap();
        assert_eq!(articles.len(), 3, "should return 3 declared articles");
        assert_eq!(articles[0].title, "alpha", "first should be alpha");
        assert_eq!(articles[1].title, "beta", "second should be beta");
        assert_eq!(articles[2].title, "zulu", "third should be zulu");
    }

    #[test]
    fn scan_declared_missing_source_still_returns_article() {
        let config = config_with_typed(vec![("ghost", Some("nonexistent-dir"))]);
        let dir = tempfile::tempdir().unwrap();
        let articles = scan_declared(dir.path(), &config).unwrap();
        assert_eq!(articles.len(), 1, "should return article even with missing source");
        assert_eq!(articles[0].title, "ghost");
    }

    #[test]
    fn scan_declared_template_origin_always_none() {
        let config = config_with_typed(vec![("reports", Some("reports"))]);
        let dir = tempfile::tempdir().unwrap();
        let articles = scan_declared(dir.path(), &config).unwrap();
        for a in &articles {
            assert!(a.template_origin.is_none(), "declared articles must never have template_origin");
        }
    }

    // ── T004: article-path inference order tests ──

    #[test]
    fn scan_declared_prefers_docs_dir_over_md_file() {
        let dir = tempfile::tempdir().unwrap();
        // Create docs/2026-05-monthly/ as an existing directory
        std::fs::create_dir_all(dir.path().join("docs/2026-05-monthly")).unwrap();
        // Do NOT create docs/2026-05-monthly.md

        let config = config_with_typed(vec![("2026-05-monthly", None)]);
        let articles = scan_declared(dir.path(), &config).unwrap();
        assert_eq!(articles.len(), 1);
        assert_eq!(
            articles[0].article_path, "docs/2026-05-monthly",
            "should use directory path when docs/<key>/ exists"
        );
    }

    #[test]
    fn scan_declared_uses_md_file_when_dir_absent() {
        let dir = tempfile::tempdir().unwrap();
        // Create docs/my-article.md but NOT docs/my-article/
        std::fs::create_dir_all(dir.path().join("docs")).unwrap();
        std::fs::write(dir.path().join("docs/my-article.md"), "# content\n").unwrap();

        let config = config_with_typed(vec![("my-article", None)]);
        let articles = scan_declared(dir.path(), &config).unwrap();
        assert_eq!(articles.len(), 1);
        assert_eq!(articles[0].article_path, "docs/my-article.md");
    }

    #[test]
    fn scan_declared_directory_precedence_over_file() {
        let dir = tempfile::tempdir().unwrap();
        // Create both docs/my-article/ (dir) AND docs/my-article.md (file)
        std::fs::create_dir_all(dir.path().join("docs/my-article")).unwrap();
        std::fs::write(dir.path().join("docs/my-article.md"), "# content\n").unwrap();

        let config = config_with_typed(vec![("my-article", None)]);
        let articles = scan_declared(dir.path(), &config).unwrap();
        assert_eq!(articles.len(), 1);
        assert_eq!(articles[0].article_path, "docs/my-article", "directory should take precedence over .md file");
    }

    #[test]
    fn scan_declared_resolve_docs_fallback_for_config_no_article_dir() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("docs/team-updates")).unwrap();

        let config = config_with_typed(vec![("team-updates", None)]);
        let articles = scan_declared(dir.path(), &config).unwrap();
        assert_eq!(articles.len(), 1);
        assert_eq!(articles[0].article_path, "docs/team-updates");
    }

    #[test]
    fn scan_declared_configured_article_dir_dir_article() {
        let dir = tempfile::tempdir().unwrap();
        // Configured article_dir is a directory article (no <id>.md inside)
        std::fs::create_dir_all(dir.path().join("specs/quarterly")).unwrap();

        let config = config_with_typed(vec![("quarterly-review", Some("specs/quarterly"))]);
        let articles = scan_declared(dir.path(), &config).unwrap();
        assert_eq!(articles.len(), 1);
        assert_eq!(
            articles[0].article_path, "specs/quarterly",
            "configured article_dir directory should be used directly"
        );
    }

    #[test]
    fn scan_declared_configured_article_dir_with_id_file() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("specs")).unwrap();
        std::fs::write(dir.path().join("specs/my-article.md"), "# spec\n").unwrap();

        let config = config_with_typed(vec![("my-article", Some("specs"))]);
        let articles = scan_declared(dir.path(), &config).unwrap();
        assert_eq!(articles.len(), 1);
        assert_eq!(articles[0].article_path, "specs/my-article.md", "when <article_dir>/<id>.md exists, use file path");
    }

    // ── T007: builtin_template tests ──

    #[test]
    fn builtin_blank() {
        let (body, at) = builtin_template("blank").unwrap();
        assert_eq!(at, ArticleType::Blank);
        assert!(body.contains("{title}"));
    }

    #[test]
    fn builtin_arch() {
        let (body, at) = builtin_template("arch").unwrap();
        assert_eq!(at, ArticleType::Arch);
        assert!(body.contains("## Context"));
        assert!(body.contains("## Decision"));
        assert!(body.contains("## Consequence"));
        assert!(body.contains("## Alternatives Considered"));
    }

    #[test]
    fn builtin_prd() {
        let (body, at) = builtin_template("prd").unwrap();
        assert_eq!(at, ArticleType::Prd);
        assert!(body.contains("## Background"));
        assert!(body.contains("## Goals"));
    }

    #[test]
    fn builtin_blog_matches_legacy_article_template() {
        let (body, at) = builtin_template("blog").unwrap();
        assert_eq!(at, ArticleType::Blog);
        assert_eq!(body, ARTICLE_TEMPLATE, "TEMPLATE_BLOG must be byte-identical to ARTICLE_TEMPLATE");
    }

    #[test]
    fn builtin_blog_body_matches_legacy_byte_for_byte() {
        let (body, _) = builtin_template("blog").unwrap();
        assert_eq!(body, super::ARTICLE_TEMPLATE);
    }

    #[test]
    fn builtin_unknown_returns_none() {
        assert!(builtin_template("nope").is_none());
        assert!(builtin_template("ARCH").is_none());
        assert!(builtin_template("").is_none());
    }

    #[test]
    fn builtin_h2_slugs_are_distinct() {
        for name in &["arch", "prd", "blog"] {
            let (body, _) = builtin_template(name).unwrap();
            let mut slugs: std::collections::HashSet<String> = std::collections::HashSet::new();
            for line in body.lines() {
                if let Some(h2) = line.strip_prefix("## ") {
                    let slug = util::to_filename(h2.trim());
                    assert!(slugs.insert(slug.clone()), "duplicate slug '{slug}' in template '{name}'");
                }
            }
        }
    }

    // ── T008: split_template_into_blocks tests ──

    #[test]
    fn split_zero_h2_produces_head_only() {
        let input = "# Title\n\nBody text\n";
        let blocks = split_template_into_blocks(input).unwrap();
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].0, "01-opening.md");
        assert_eq!(blocks[0].1, "# Title\n\nBody text");
    }

    #[test]
    fn split_four_h2_arch_style() {
        let input = "# Title\n\n> Created: now\n\n## Context\n\nctx body\n\n## Decision\n\ndec\n\n## Consequence\n\ncons\n\n## Alternatives Considered\n\nalt\n";
        let blocks = split_template_into_blocks(input).unwrap();
        assert_eq!(blocks.len(), 5);
        assert_eq!(blocks[0].0, "01-opening.md");
        assert!(blocks[0].1.starts_with("# Title"));
        assert_eq!(blocks[1].0, "02-context.md");
        assert!(blocks[1].1.starts_with("## Context"));
        assert_eq!(blocks[2].0, "03-decision.md");
        assert_eq!(blocks[3].0, "04-consequence.md");
        assert_eq!(blocks[4].0, "05-alternatives-considered.md");
    }

    #[test]
    fn split_leading_h2_empty_head() {
        let input = "## Intro\n\nintro body\n";
        let blocks = split_template_into_blocks(input).unwrap();
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].0, "01-opening.md");
        assert_eq!(blocks[0].1, "");
        assert_eq!(blocks[1].0, "02-intro.md");
        assert_eq!(blocks[1].1, "## Intro\n\nintro body");
    }

    #[test]
    fn split_crlf_normalized() {
        let input = "# Title\r\n\r\n## Summary\r\n\r\nbody\r\n";
        let blocks = split_template_into_blocks(input).unwrap();
        assert_eq!(blocks.len(), 2);
        assert!(!blocks[0].1.contains('\r'));
        assert!(!blocks[1].1.contains('\r'));
    }

    #[test]
    fn split_duplicate_slug_rejected() {
        let input = "# T\n\n## Notes\n\nbody1\n\n## NOTES\n\nbody2\n";
        let err = split_template_into_blocks(input).unwrap_err();
        assert!(matches!(err, MfError::DuplicateBlockSlug { .. }));
        assert_eq!(err.kind(), "duplicate_block_slug");
    }

    #[test]
    fn split_single_h2() {
        let input = "# Title\n\nintro\n\n## Summary\n\nsummary body\n";
        let blocks = split_template_into_blocks(input).unwrap();
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].0, "01-opening.md");
        assert_eq!(blocks[0].1, "# Title\n\nintro\n");
        assert_eq!(blocks[1].0, "02-summary.md");
        assert!(blocks[1].1.starts_with("## Summary"));
    }
}
