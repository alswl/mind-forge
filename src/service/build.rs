use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::error::{MfError, Result};
use crate::service::{config as config_svc, index, util};

/// A single source file entry in a build plan.
#[derive(Debug, Serialize)]
pub struct SourceEntry {
    pub path: String,
    pub size: u64,
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
    let source_path = resolve_indexed_article_source(project_path, article)?;
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
    let article_entry = index
        .articles
        .iter()
        .flat_map(|a| a.iter())
        .find(|a| {
            // Priority 1: exact path match
            let exact = format!("docs/{}", article);
            let exact_md = format!("docs/{}.md", article);
            if a.source_path == exact || a.source_path == exact_md {
                return true;
            }
            // Priority 2: slug match — strip docs/ and optional .md
            if let Some(stripped) = a.source_path.strip_prefix("docs/") {
                let slug = stripped.strip_suffix(".md").unwrap_or(stripped);
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
        project_path.join("docs").join(article),
        project_path.join("docs").join(format!("{article}.md")),
        project_path.join("docs").join(&title_slug),
        project_path.join("docs").join(format!("{title_slug}.md")),
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
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "md"))
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
            estimated_size: total_size,
            dry_run: true,
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
