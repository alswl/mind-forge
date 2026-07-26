use std::collections::HashMap;
use std::fs;
use std::path::Path;

use chrono::Utc;

use crate::defaults;
use crate::error::{MfError, Result};
use crate::model::article::{Article, ArticleDiff, ArticleStatus, ArticleType, ScannedArticle, TemplateOrigin};
use crate::model::config::{MindConfig, TemplateMode};
use crate::model::index::IndexFile;
use crate::service::config as config_svc;
use crate::service::index;
use crate::service::util;
use crate::service::util::path_template::PathTemplate;

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

    // Every discovery path above is allowed to refresh derived metadata (for
    // example a template origin), but indexing the same filesystem must not
    // turn a no-op into a timestamp-only mutation.  Some entries can be seen
    // by more than one discovery phase, so enforce the invariant once after
    // merge rather than relying on each phase's precedence branch.
    for article in &mut articles {
        if let Some(existing) = existing_map.get(article.article_path.as_str()) {
            article.created_at = existing.created_at.clone();
            article.updated_at = existing.updated_at.clone();
        }
    }

    let index = IndexFile {
        schema_version: defaults::SCHEMA_VERSION.to_string(),
        articles: Some(articles),
        publish_records: existing.publish_records,
        sources: existing.sources,
        assets: existing.assets,
        prompts: existing.prompts,
        thinking: existing.thinking,
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
    if dir_path.join(&file_name).is_file() { format!("{article_dir}/{file_name}") } else { article_dir.to_string() }
}

/// Scan a single directory for markdown files, appending to `scanned`.
fn scan_md_dir(dir_path: &Path, rel_dir: &str, scanned: &mut Vec<ScannedArticle>) -> Result<()> {
    let entries = fs::read_dir(dir_path).map_err(MfError::Io)?;
    for entry in entries.flatten() {
        let path = entry.path();
        let ft = entry.file_type().map_err(MfError::Io)?;

        // Single-file article: a .md file directly in the docs dir.
        if ft.is_file()
            && path.extension().and_then(|e| e.to_str()) == Some(defaults::MARKDOWN_EXTENSION)
            && let Some(stem) = path.file_stem().and_then(|s| s.to_str())
        {
            let title = stem.replace('-', " ");
            scanned.push(ScannedArticle {
                title,
                filename: stem.to_string(),
                article_dir: Some(rel_dir.to_string()),
                article_path: None,
            });
        }

        // Directory-type article: a sub-directory containing at least one
        // NN-*.md file (Bug #3 fix). Created by `article new --force`.
        if ft.is_dir() {
            let name = path.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
            if name.starts_with('.') {
                continue;
            }
            if let Ok(dir_entries) = fs::read_dir(&path) {
                let has_md_part = dir_entries.flatten().any(|e| {
                    e.file_type().is_ok_and(|t| t.is_file())
                        && e.path().extension().and_then(|s| s.to_str()) == Some(defaults::MARKDOWN_EXTENSION)
                });
                if has_md_part {
                    let title = name.replace('-', " ");
                    let dir_rel = format!("{}/{}", rel_dir, name);
                    scanned.push(ScannedArticle {
                        title,
                        filename: name.clone(),
                        article_dir: Some(dir_rel.clone()),
                        // Directory-type article: article_path is the directory
                        // itself (matching `article new`), NOT dir/<name>.md,
                        // which does not exist (blocks Bug #3 build/publish).
                        article_path: Some(dir_rel),
                    });
                }
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

/// Reconcile a single project's doc-article index against the filesystem:
/// prune stale entries and register newly-scanned docs. Returns the added and
/// removed `article_path` lists. Persists `mind-index.yaml` only on a real run
/// with changes; `dry_run` computes the lists without writing.
///
/// Reused by `mf project index` so it prunes stale per-project article entries
/// consistently with `mf article index` (FR-009).
///
/// Removal is restricted to entries whose target file/dir is truly absent on
/// disk. `compute_article_diff` marks anything missing from the docs scan as
/// removed, which over-approximates here: declared and template-origin articles
/// live outside the docs scan (they are merged back by `mf article index`
/// phases 2–3, which this helper deliberately does not run) and must never be
/// pruned while their files exist (FR-009 safety guard).
pub fn reconcile_project_docs(project_path: &Path, dry_run: bool) -> Result<(Vec<String>, Vec<String>)> {
    let scanned = scan_docs(project_path)?;
    let index = crate::service::index::load(project_path)?;
    let layout = config_svc::effective_layout(project_path)?;
    let mut diff = compute_article_diff(&index, &scanned, &layout.articles);
    diff.removed.retain(|a| !project_path.join(&a.article_path).exists());
    let added: Vec<String> = diff.added.iter().map(|a| article_path_for_scanned(a, &layout.articles)).collect();
    let removed: Vec<String> = diff.removed.iter().map(|a| a.article_path.clone()).collect();
    if !dry_run && (!added.is_empty() || !removed.is_empty()) {
        let updated = reconcile_articles(project_path, index, diff)?;
        crate::service::index::save(project_path, &updated)?;
    }
    Ok((added, removed))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::config::{ArticleBuildConfig, BuildConfig};

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
}
