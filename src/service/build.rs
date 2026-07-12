use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::defaults;
use crate::error::{MfError, Result};
use crate::model::config::{BannerConfig, BannerLevel};
use crate::service::util::markdown;
use crate::service::{config as config_svc, index, util};

/// A single article file entry in a build plan.
#[derive(Debug, Serialize)]
pub struct ArticleInputEntry {
    pub path: String,
    pub size: u64,
}

/// Information about the configured build banner, included in dry-run output.
#[derive(Debug, Serialize)]
pub struct BannerInfo {
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub level: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
}

/// A build plan describing what would happen during a build.
#[derive(Debug, Serialize)]
pub struct BuildPlan {
    pub article: String,
    pub project: String,
    pub article_path: String,
    pub input_files: Vec<ArticleInputEntry>,
    pub merge_order: Vec<String>,
    pub output_path: String,
    pub size_bytes: u64,
    pub estimated_size: u64,
    pub dry_run: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub banner: Option<BannerInfo>,
}

/// Render banner config into the Markdown text to insert into generated output.
pub fn render_banner(banner: &BannerConfig) -> String {
    match &banner.level {
        Some(level) => {
            let level_str = match level {
                BannerLevel::Note => "note",
                BannerLevel::Tip => "tip",
                BannerLevel::Warning => "warning",
                BannerLevel::Danger => "danger",
            };
            let text = banner.text.trim();
            format!(":::{level_str}\n{text}\n:::\n\n")
        }
        None => {
            let text = banner.text.trim();
            format!("{text}\n\n")
        }
    }
}

/// Insert banner text into generated content after a frontmatter header if present,
/// otherwise at the very beginning.
fn insert_banner_into_content(content: &str, banner_text: &str) -> String {
    // Check for YAML frontmatter delimited by `---`
    if let Some(rest) = content.strip_prefix("---") {
        // Find the closing `---` marker
        if let Some(end) = rest.find("\n---") {
            let split = 3 + end + 4; // position after closing `---`
                                     // Check if there's a newline after the closing `---`
            let after_split =
                if split < content.len() && content[split..].starts_with('\n') { split + 1 } else { split };
            let mut result = content[..after_split].to_string();
            result.push('\n');
            result.push_str(banner_text);
            result.push_str(&content[after_split..]);
            return result;
        }
    }
    // No frontmatter: prepend banner
    let mut result = banner_text.to_string();
    result.push_str(content);
    result
}

/// Build an article: load config, resolve sources, render to output.
///
/// `article` is the article name (filename stem within `docs/`).
/// `project_path` is the resolved project directory.
/// `repo_root` is the mind-forge repository root for config resolution.
/// `dry_run` controls whether to produce a plan or actual output.
/// `output_override` optionally overrides the default output path.
pub fn build_article(
    project_path: &Path,
    repo_root: &Path,
    article: &str,
    dry_run: bool,
    output_override: Option<&Path>,
) -> Result<BuildOutput> {
    // Check for configured article_dir in build.articles.<article>.article_dir
    let article_path = match config_svc::load_project(project_path, Some(repo_root)) {
        Ok(Some(config)) => config
            .build
            .articles
            .get(article)
            .and_then(|a| a.article_dir.as_ref())
            .map(|dir| {
                let p = project_path.join(dir);
                if !p.exists() || !p.is_dir() {
                    return Err(MfError::usage(
                        format!("configured article_dir '{dir}' for article '{article}' does not exist or is not a directory"),
                        Some("check the path or create the directory".to_string()),
                    ));
                }
                Ok(p)
            })
            .transpose()?,
        _ => None,
    };

    let article_path = match article_path {
        Some(path) => path,
        None => resolve_indexed_article_path(project_path, article)?,
    };
    build_article_content(project_path, repo_root, article, &article_path, dry_run, output_override)
}

pub fn build_article_path(
    project_path: &Path,
    repo_root: &Path,
    article_path: &Path,
    dry_run: bool,
    output_override: Option<&Path>,
) -> Result<BuildOutput> {
    let article = article_path
        .file_stem()
        .or_else(|| article_path.file_name())
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| "article".to_string());
    build_article_content(project_path, repo_root, &article, article_path, dry_run, output_override)
}

fn resolve_indexed_article_path(project_path: &Path, article: &str) -> Result<PathBuf> {
    let index = index::load(project_path)?;
    let resolved = index::resolve_article(&index, article)?;
    let article_path = project_path.join(&resolved.article.article_path);
    if article_path.exists() {
        return Ok(article_path);
    }
    Err(MfError::usage(
        format!("article path not found: {}", resolved.article.article_path),
        Some("the file or directory may have been moved or deleted".to_string()),
    ))
}

fn build_article_content(
    project_path: &Path,
    repo_root: &Path,
    article: &str,
    article_path: &Path,
    dry_run: bool,
    output_override: Option<&Path>,
) -> Result<BuildOutput> {
    // 1. Load project config (mind.yaml)
    let config = config_svc::load_project(project_path, Some(repo_root))?.ok_or_else(|| {
        MfError::usage("project missing mind.yaml".to_string(), Some("run `mf config init` to create one".to_string()))
    })?;
    let build_cfg = &config.build;

    // 1b. Validate new config fields (e.g. empty banner text)
    config_svc::validate_new_fields(&config)?;

    // 2. Render banner if configured
    let banner_text: Option<String> = build_cfg.banner.as_ref().map(render_banner);
    let banner_size: u64 = banner_text.as_ref().map(|t| t.len() as u64).unwrap_or(0);

    // 3. Build banner info for dry-run output
    let banner_info: Option<BannerInfo> = build_cfg.banner.as_ref().map(|b| {
        let level_str = b.level.as_ref().map(|l| {
            match l {
                BannerLevel::Note => "note",
                BannerLevel::Tip => "tip",
                BannerLevel::Warning => "warning",
                BannerLevel::Danger => "danger",
            }
            .to_string()
        });
        BannerInfo { enabled: true, level: level_str, text: Some(b.text.clone()) }
    });

    if !article_path.exists() {
        return Err(MfError::usage(
            format!("article path not found: {}", article_path.display()),
            Some("the file or directory may have been moved or deleted".to_string()),
        ));
    }

    // 4. Determine output path
    let layout = config_svc::effective_layout(project_path)?;
    let output_path = match output_override {
        Some(path) => path.to_path_buf(),
        None => {
            let stem = crate::service::index::article_output_stem(article);
            project_path.join(&layout.build_output).join(format!("{stem}.{}", build_cfg.format))
        }
    };

    // 5. Gather article files — single file or directory contents
    let article_files: Vec<std::path::PathBuf> = if article_path.is_dir() {
        let mut files: Vec<_> = fs::read_dir(article_path)
            .map_err(MfError::Io)?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().is_some_and(|ext| ext == defaults::MARKDOWN_EXTENSION))
            .map(|e| e.path())
            .collect();
        files.sort();
        files
    } else {
        vec![article_path.to_path_buf()]
    };

    if article_files.is_empty() {
        return Err(MfError::usage(
            format!("no markdown files found in article directory: {}", article_path.display()),
            None,
        ));
    }

    let total_size: u64 = article_files.iter().filter_map(|p| fs::metadata(p).ok()).map(|m| m.len()).sum();

    // 6. Determine merge order (from config or fallback)
    let merge_order =
        if build_cfg.merge_order.is_empty() { vec![article.to_string()] } else { build_cfg.merge_order.clone() };

    // 7. Build plan output (for dry-run)
    let input_files: Vec<ArticleInputEntry> = article_files
        .iter()
        .map(|p| ArticleInputEntry {
            path: p.strip_prefix(project_path).unwrap_or(p).to_string_lossy().to_string(),
            size: fs::metadata(p).ok().map(|m| m.len()).unwrap_or(0),
        })
        .collect();

    if dry_run {
        return Ok(BuildOutput::Plan(BuildPlan {
            article: article.to_string(),
            project: util::dir_name(project_path),
            article_path: article_path.to_string_lossy().to_string(),
            input_files,
            merge_order,
            output_path: output_path.to_string_lossy().to_string(),
            size_bytes: total_size,
            estimated_size: total_size + banner_size,
            dry_run: true,
            banner: banner_info,
        }));
    }

    // 8. Read and concatenate files
    let mut content = String::new();
    for file in &article_files {
        let file_content = fs::read_to_string(file).map_err(MfError::Io)?;
        let file_content = markdown::strip_typora_front_matter(&file_content);
        content.push_str(&file_content);
        if !content.ends_with('\n') {
            content.push('\n');
        }
    }

    // 8b. Rewrite relative paths to resolve from the output directory (Bug #1 fix).
    let source_dir = if article_path.is_dir() {
        article_path.to_path_buf()
    } else {
        article_path.parent().unwrap_or(Path::new(".")).to_path_buf()
    };
    let output_dir = output_path.parent().unwrap_or(Path::new("."));
    let mut warnings = Vec::new();
    content = rewrite_relative_paths(&content, &source_dir, output_dir, &mut warnings);

    // 9. Inject banner into content if configured
    if let Some(ref banner) = banner_text {
        content = insert_banner_into_content(&content, banner);
    }

    // Ensure output directory exists
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(MfError::Io)?;
    }

    util::atomic_write(&output_path, &content)?;
    let result = BuildResult {
        output_path: output_path.to_string_lossy().to_string(),
        size_bytes: content.len() as u64,
        warnings,
    };

    // Auto-index: ensure the built article exists in mind-index.yaml
    if let Err(e) = auto_index_article(project_path, article, article_path) {
        tracing::warn!("failed to auto-index article '{}': {}", article, e);
    }

    Ok(BuildOutput::Rendered(result))
}

/// Result of a successful build.
#[derive(Debug)]
pub struct BuildResult {
    pub output_path: String,
    pub size_bytes: u64,
    /// References that could not be safely rewritten and were left as-is
    /// (Bug #22 defense in depth) — surfaced by the CLI layer as warnings.
    pub warnings: Vec<String>,
}

/// Build output: either the rendered content or a dry-run plan.
#[derive(Debug)]
pub enum BuildOutput {
    Rendered(BuildResult),
    Plan(BuildPlan),
}

/// Rewrite relative image/link/reference paths in `content` so they resolve
/// from `output_dir` to the same target they resolved to from `source_dir`.
/// Skips paths inside fenced code blocks, absolute paths, URLs, data URIs,
/// and anchors. References that cannot be safely rewritten (Bug #22: e.g. a
/// mismatched absolute/relative base) are left untouched and reported via
/// `warnings` instead of being written as a malformed path.
fn rewrite_relative_paths(content: &str, source_dir: &Path, output_dir: &Path, warnings: &mut Vec<String>) -> String {
    // `output_dir` is constant for the whole document — normalise it once rather
    // than per rewritten target.
    let base = markdown::normalize_lexical(output_dir);
    markdown::rewrite_references(content, |target| rewrite_target(target, source_dir, &base, warnings))
}

/// Compute the new relative path from `base_dir` (an already lexically
/// normalised output directory) to the same physical target that `target`
/// resolved to from `source_dir`.
fn rewrite_target(target: &str, source_dir: &Path, base_dir: &Path, warnings: &mut Vec<String>) -> Option<String> {
    if !markdown::should_rewrite_target(target) {
        return None;
    }
    // Rebase onto the output directory (lexically normalised so the result has
    // no interior `foo/../` segments — Bug 1B). A `None` here can only be the
    // mixed absolute/relative base case (Bug #22), since we already know the
    // target is rewritable; surface it as a warning rather than a malformed path.
    let rewritten = markdown::rebase_relative_target(target, source_dir, base_dir);
    if rewritten.is_none() {
        warnings.push(format!(
            "cannot compute a relative path for reference '{target}' (source: {}) to output directory '{}'; keeping original reference",
            source_dir.display(),
            base_dir.display()
        ));
    }
    rewritten
}

/// After a successful build, ensure the article is present in mind-index.yaml.
fn auto_index_article(project_path: &Path, article: &str, article_path: &Path) -> Result<()> {
    let mut index =
        crate::service::index::load(project_path).unwrap_or_else(|_| crate::model::index::IndexFile::create_default());
    let layout = config_svc::effective_layout(project_path)?;

    // Determine the relative article_path from the project root
    let rel_source = if article_path.is_dir() {
        let file_name = format!("{}.{}", article, defaults::MARKDOWN_EXTENSION);
        if article_path.join(&file_name).is_file() {
            article_path
                .join(file_name)
                .strip_prefix(project_path)
                .ok()
                .and_then(|p| p.to_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| format!("{}/{}.{}", layout.articles, article, defaults::MARKDOWN_EXTENSION))
        } else {
            article_path
                .strip_prefix(project_path)
                .ok()
                .and_then(|p| p.to_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| format!("{}/{}", layout.articles, article))
        }
    } else {
        // article_path is a file — strip project prefix to get the relative path
        article_path
            .strip_prefix(project_path)
            .ok()
            .and_then(|p| p.to_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("{}/{}.{}", layout.articles, article, defaults::MARKDOWN_EXTENSION))
    };

    // Check if article is already in the index
    let already_indexed =
        index.articles.as_ref().is_some_and(|articles| articles.iter().any(|a| a.article_path == rel_source));

    if !already_indexed {
        let project_name = util::dir_name(project_path);
        let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
        let articles = index.articles.get_or_insert_with(Vec::new);
        articles.push(crate::model::article::Article {
            title: article.replace('-', " "),
            project: project_name,
            article_type: crate::model::article::ArticleType::Blog,
            article_path: rel_source,
            status: crate::model::article::ArticleStatus::Published,
            created_at: now.clone(),
            updated_at: now,
            template_origin: None,
        });
        crate::service::index::save(project_path, &index)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::article::{Article, ArticleStatus, ArticleType};
    use crate::model::index::IndexFile;

    fn make_article(article_path: &str, title: &str) -> Article {
        Article {
            title: title.to_string(),
            project: "test".to_string(),
            article_type: ArticleType::Blog,
            article_path: article_path.to_string(),
            status: ArticleStatus::Draft,
            created_at: "2026-05-15T00:00:00Z".to_string(),
            updated_at: "2026-05-15T00:00:00Z".to_string(),
            template_origin: None,
        }
    }

    #[test]
    fn resolve_indexed_article_path_by_exact_key() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("docs/2026-05-monthly")).unwrap();
        std::fs::write(dir.path().join("docs/2026-05-monthly/01-team-okr.md"), "# okr\n").unwrap();
        let index = IndexFile {
            schema_version: "1".to_string(),
            articles: Some(vec![make_article("docs/2026-05-monthly", "2026 05 monthly")]),
            ..IndexFile::create_default()
        };
        index::save(dir.path(), &index).unwrap();

        let result = resolve_indexed_article_path(dir.path(), "2026-05-monthly").unwrap();
        assert!(result.exists());
        assert!(result.is_dir());
    }

    #[test]
    fn resolve_indexed_article_path_title_not_used_as_path_derivation() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("docs/team-updates")).unwrap();
        std::fs::write(dir.path().join("docs/team-updates/01-note.md"), "# note\n").unwrap();
        let index = IndexFile {
            schema_version: "1".to_string(),
            articles: Some(vec![make_article("docs/team-updates", "Team Updates")]),
            ..IndexFile::create_default()
        };
        index::save(dir.path(), &index).unwrap();

        // Lookup by key (not title) should work
        let result = resolve_indexed_article_path(dir.path(), "team-updates").unwrap();
        assert!(result.exists());

        // Lookup by display title should fail — title is not a search key
        let err = resolve_indexed_article_path(dir.path(), "Team Updates").unwrap_err();
        assert!(matches!(err, MfError::NotFound { .. }));
    }

    // ── Path-rewrite helpers (spec 059 Bug #1 / 1A / 1B / spec 064 Bug #22) ──
    //
    // The generic scanning/normalisation primitives (normalize_lexical,
    // relative_path_from, should_rewrite_target, the line/reference scanner)
    // moved to `service::util::markdown` (spec 064 Foundational phase) and are
    // tested there. These tests cover only the build-specific composition
    // (`rewrite_target`, `rewrite_relative_paths`) that remains in this module.

    #[test]
    fn rewrite_target_skips_absolute_and_urls() {
        let src = Path::new("docs/art");
        let base = Path::new("outputs");
        let mut warnings = Vec::new();
        for t in
            ["/abs/x.png", "http://x/y.png", "https://x/y.png", "mailto:a@b.com", "data:image/png,AA", "#anchor", ""]
        {
            assert_eq!(rewrite_target(t, src, base, &mut warnings), None, "must not rewrite {t:?}");
        }
        assert!(warnings.is_empty(), "intentionally-skipped targets must not warn: {warnings:?}");
    }

    #[test]
    fn rewrite_target_rebases_and_normalizes() {
        let src = Path::new("docs/art");
        let base = markdown::normalize_lexical(Path::new("outputs"));
        let mut warnings = Vec::new();
        // Plain relative target.
        assert_eq!(rewrite_target("assets/p.png", src, &base, &mut warnings).unwrap(), "../docs/art/assets/p.png");
        // Bug 1B: interior `../` is normalised away, not left interleaved.
        let out = rewrite_target("../shared/p.png", src, &base, &mut warnings).unwrap();
        assert_eq!(out, "../docs/shared/p.png");
        assert!(!out.contains("/../"), "no interior /../: {out}");
        assert!(warnings.is_empty(), "successful rewrites must not warn: {warnings:?}");
    }

    #[test]
    fn rewrite_target_warns_on_mixed_base_instead_of_malformed_path() {
        // Bug #22: an absolute source_dir with a relative base_dir must not
        // produce a malformed concatenated path; it must warn and keep the
        // original reference untouched (returns None to the caller).
        let src = Path::new("/abs/docs/art");
        let base = Path::new("outputs");
        let mut warnings = Vec::new();
        assert_eq!(rewrite_target("assets/p.png", src, base, &mut warnings), None);
        assert_eq!(warnings.len(), 1, "mismatched base must produce exactly one warning: {warnings:?}");
        assert!(warnings[0].contains("assets/p.png"), "warning must name the reference: {warnings:?}");
    }

    #[test]
    fn rewrite_relative_paths_end_to_end() {
        let src = Path::new("docs/art");
        let out_dir = Path::new("outputs");
        let content = "![工作流全景](assets/p.png)\n\n```\n![untouched](x.png)\n```\n[img]: assets/q.png\n";
        let mut warnings = Vec::new();
        let out = rewrite_relative_paths(content, src, out_dir, &mut warnings);
        assert_eq!(
            out,
            "![工作流全景](../docs/art/assets/p.png)\n\n```\n![untouched](x.png)\n```\n[img]: ../docs/art/assets/q.png\n"
        );
        assert!(warnings.is_empty());
    }

    #[test]
    fn rewrite_relative_paths_keeps_unresolvable_reference_and_warns() {
        // Mixed absolute/relative bases: the reference must be kept verbatim
        // (not malformed) and a warning must be collected.
        let src = Path::new("/abs/docs/art");
        let out_dir = Path::new("outputs");
        let content = "![hero](assets/p.png)\n";
        let mut warnings = Vec::new();
        let out = rewrite_relative_paths(content, src, out_dir, &mut warnings);
        assert_eq!(out, content, "unresolvable reference must be kept verbatim, not malformed");
        assert!(!out.contains("////"), "must never contain a malformed path fragment");
        assert_eq!(warnings.len(), 1);
    }
}
