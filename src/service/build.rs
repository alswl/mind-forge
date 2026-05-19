use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::defaults;
use crate::error::{MfError, Result};
use crate::model::config::{BannerConfig, BannerLevel};
use crate::service::{config as config_svc, index, util};

/// A single source file entry in a build plan.
#[derive(Debug, Serialize)]
pub struct SourceEntry {
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
    pub source_path: String,
    pub input_sources: Vec<SourceEntry>,
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
    // Check for configured source_dir in build.articles.<article>.source_dir
    let source_path = match config_svc::load_project(project_path, Some(repo_root)) {
        Ok(Some(config)) => config
            .build
            .articles
            .get(article)
            .and_then(|a| a.source_dir.as_ref())
            .map(|dir| {
                let p = project_path.join(dir);
                if !p.exists() || !p.is_dir() {
                    return Err(MfError::usage(
                        format!("configured source_dir '{dir}' for article '{article}' does not exist or is not a directory"),
                        Some("check the path or create the directory".to_string()),
                    ));
                }
                Ok(p)
            })
            .transpose()?,
        _ => None,
    };

    let source_path = match source_path {
        Some(path) => path,
        None => resolve_indexed_article_source(project_path, article)?,
    };
    build_source(project_path, repo_root, article, &source_path, dry_run, output_override)
}

pub fn build_article_path(
    project_path: &Path,
    repo_root: &Path,
    source_path: &Path,
    dry_run: bool,
    output_override: Option<&Path>,
) -> Result<BuildOutput> {
    let article = source_path
        .file_stem()
        .or_else(|| source_path.file_name())
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| "article".to_string());
    build_source(project_path, repo_root, &article, source_path, dry_run, output_override)
}

fn resolve_indexed_article_source(project_path: &Path, article: &str) -> Result<PathBuf> {
    let index = index::load(project_path)?;
    let resolved = index::resolve_article(&index, article)?;
    let source_path = project_path.join(&resolved.article.source_path);
    if source_path.exists() {
        return Ok(source_path);
    }
    Err(MfError::usage(
        format!("source not found: {}", resolved.article.source_path),
        Some("the file or directory may have been moved or deleted".to_string()),
    ))
}

fn build_source(
    project_path: &Path,
    repo_root: &Path,
    article: &str,
    source_path: &Path,
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

    if !source_path.exists() {
        return Err(MfError::usage(
            format!("source not found: {}", source_path.display()),
            Some("the file or directory may have been moved or deleted".to_string()),
        ));
    }

    // 4. Determine output path
    let output_path = match output_override {
        Some(path) => path.to_path_buf(),
        None => {
            let stem = crate::service::index::article_output_stem(article);
            project_path.join(&build_cfg.output_dir).join(format!("{stem}.{}", build_cfg.format))
        }
    };

    // 5. Gather source files — single file or directory contents
    let source_files: Vec<std::path::PathBuf> = if source_path.is_dir() {
        let mut files: Vec<_> = fs::read_dir(source_path)
            .map_err(MfError::Io)?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().is_some_and(|ext| ext == defaults::MARKDOWN_EXTENSION))
            .map(|e| e.path())
            .collect();
        files.sort();
        files
    } else {
        vec![source_path.to_path_buf()]
    };

    if source_files.is_empty() {
        return Err(MfError::usage(
            format!("no markdown files found in source directory: {}", source_path.display()),
            None,
        ));
    }

    let total_size: u64 = source_files.iter().filter_map(|p| fs::metadata(p).ok()).map(|m| m.len()).sum();

    // 6. Determine merge order (from config or fallback)
    let merge_order =
        if build_cfg.merge_order.is_empty() { vec![article.to_string()] } else { build_cfg.merge_order.clone() };

    // 7. Build plan output (for dry-run)
    let input_sources: Vec<SourceEntry> = source_files
        .iter()
        .map(|p| SourceEntry {
            path: p.strip_prefix(project_path).unwrap_or(p).to_string_lossy().to_string(),
            size: fs::metadata(p).ok().map(|m| m.len()).unwrap_or(0),
        })
        .collect();

    if dry_run {
        return Ok(BuildOutput::Plan(BuildPlan {
            article: article.to_string(),
            project: util::dir_name(project_path),
            source_path: source_path.to_string_lossy().to_string(),
            input_sources,
            merge_order,
            output_path: output_path.to_string_lossy().to_string(),
            size_bytes: total_size,
            estimated_size: total_size + banner_size,
            dry_run: true,
            banner: banner_info,
        }));
    }

    // 8. Read and concatenate files, write output
    let mut content = String::new();
    for file in &source_files {
        let file_content = fs::read_to_string(file).map_err(MfError::Io)?;
        content.push_str(&file_content);
        if !content.ends_with('\n') {
            content.push('\n');
        }
    }

    // 9. Inject banner into content if configured
    if let Some(ref banner) = banner_text {
        content = insert_banner_into_content(&content, banner);
    }

    // Ensure output directory exists
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(MfError::Io)?;
    }

    util::atomic_write(&output_path, &content)?;
    let result =
        BuildResult { output_path: output_path.to_string_lossy().to_string(), size_bytes: content.len() as u64 };

    // Auto-index: ensure the built article exists in mind-index.yaml
    if let Err(e) = auto_index_article(project_path, article, source_path) {
        tracing::warn!("failed to auto-index article '{}': {}", article, e);
    }

    Ok(BuildOutput::Rendered(result))
}

/// Result of a successful build.
#[derive(Debug)]
pub struct BuildResult {
    pub output_path: String,
    pub size_bytes: u64,
}

/// Build output: either the rendered content or a dry-run plan.
#[derive(Debug)]
pub enum BuildOutput {
    Rendered(BuildResult),
    Plan(BuildPlan),
}

/// After a successful build, ensure the article is present in mind-index.yaml.
fn auto_index_article(project_path: &Path, article: &str, source_path: &Path) -> Result<()> {
    let mut index =
        crate::service::index::load(project_path).unwrap_or_else(|_| crate::model::index::IndexFile::create_default());
    let paths = config_svc::project_paths(project_path)?;

    // Determine the relative source_path from the project root
    let rel_source = if source_path.is_dir() {
        let file_name = format!("{}.{}", article, defaults::MARKDOWN_EXTENSION);
        if source_path.join(&file_name).is_file() {
            source_path
                .join(file_name)
                .strip_prefix(project_path)
                .ok()
                .and_then(|p| p.to_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| format!("{}/{}.{}", paths.docs, article, defaults::MARKDOWN_EXTENSION))
        } else {
            source_path
                .strip_prefix(project_path)
                .ok()
                .and_then(|p| p.to_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| format!("{}/{}", paths.docs, article))
        }
    } else {
        // source_path is a file — strip project prefix to get the relative path
        source_path
            .strip_prefix(project_path)
            .ok()
            .and_then(|p| p.to_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("{}/{}.{}", paths.docs, article, defaults::MARKDOWN_EXTENSION))
    };

    // Check if article is already in the index
    let already_indexed =
        index.articles.as_ref().is_some_and(|articles| articles.iter().any(|a| a.source_path == rel_source));

    if !already_indexed {
        let project_name = util::dir_name(project_path);
        let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
        let articles = index.articles.get_or_insert_with(Vec::new);
        articles.push(crate::model::article::Article {
            title: article.replace('-', " "),
            project: project_name,
            article_type: crate::model::article::ArticleType::Blog,
            source_path: rel_source,
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

    fn make_article(source_path: &str, title: &str) -> Article {
        Article {
            title: title.to_string(),
            project: "test".to_string(),
            article_type: ArticleType::Blog,
            source_path: source_path.to_string(),
            status: ArticleStatus::Draft,
            created_at: "2026-05-15T00:00:00Z".to_string(),
            updated_at: "2026-05-15T00:00:00Z".to_string(),
            template_origin: None,
        }
    }

    #[test]
    fn resolve_indexed_article_source_by_exact_key() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("docs/2026-05-monthly")).unwrap();
        std::fs::write(dir.path().join("docs/2026-05-monthly/01-team-okr.md"), "# okr\n").unwrap();
        let index = IndexFile {
            schema_version: "1".to_string(),
            articles: Some(vec![make_article("docs/2026-05-monthly", "2026 05 monthly")]),
            ..IndexFile::create_default()
        };
        index::save(dir.path(), &index).unwrap();

        let result = resolve_indexed_article_source(dir.path(), "2026-05-monthly").unwrap();
        assert!(result.exists());
        assert!(result.is_dir());
    }

    #[test]
    fn resolve_indexed_article_source_title_not_used_as_path_derivation() {
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
        let result = resolve_indexed_article_source(dir.path(), "team-updates").unwrap();
        assert!(result.exists());

        // Lookup by display title should fail — title is not a search key
        let err = resolve_indexed_article_source(dir.path(), "Team Updates").unwrap_err();
        assert!(matches!(err, MfError::NotFound { .. }));
    }
}
