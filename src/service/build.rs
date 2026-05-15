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
    // Load index and find article
    let index = index::load(project_path)?;
    let paths = config_svc::project_paths(project_path)?;
    let article_entry = index
        .articles
        .iter()
        .flat_map(|a| a.iter())
        .find(|a| {
            // Priority 1: exact path match
            let exact = format!("{}/{}", paths.docs, article);
            let exact_md = format!("{}/{}.{}", paths.docs, article, defaults::MARKDOWN_EXTENSION);
            if a.source_path == exact || a.source_path == exact_md {
                return true;
            }
            // Priority 2: slug match — strip docs/ and optional .md
            if let Some(stripped) = a.source_path.strip_prefix(&format!("{}/", paths.docs)) {
                let slug = stripped.strip_suffix(&format!(".{}", defaults::MARKDOWN_EXTENSION)).unwrap_or(stripped);
                if slug == article {
                    return true;
                }
            }
            // Priority 3: title match
            if a.title == article || a.title.replace(' ', "-") == article || util::to_filename(&a.title) == article {
                return true;
            }
            false
        })
        .ok_or_else(|| {
            let project_name = util::dir_name(project_path);
            MfError::not_found(
                format!("article '{article}' not found in project '{project_name}'"),
                Some("use `mf article list` to see available articles".to_string()),
            )
        })?;

    let source_path = project_path.join(&article_entry.source_path);
    if source_path.exists() {
        return Ok(source_path);
    }

    let title_slug = util::to_filename(&article_entry.title);
    let candidates = [
        project_path.join(&paths.docs).join(article),
        project_path.join(&paths.docs).join(format!("{article}.{}", defaults::MARKDOWN_EXTENSION)),
        project_path.join(&paths.docs).join(&title_slug),
        project_path.join(&paths.docs).join(format!("{title_slug}.{}", defaults::MARKDOWN_EXTENSION)),
    ];
    candidates.into_iter().find(|path| path.exists()).ok_or_else(|| {
        MfError::usage(
            format!("source not found: {}", article_entry.source_path),
            Some("the file or directory may have been moved or deleted".to_string()),
        )
    })
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
        None => project_path.join(&build_cfg.output_dir).join(format!("{}.{}", article, build_cfg.format)),
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
