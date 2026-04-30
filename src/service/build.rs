use std::fs;
use std::path::Path;

use serde::Serialize;

use crate::error::{MfError, Result};
use crate::service::{article as article_svc, config as config_svc, util};

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
    // 1. Load project config (mind.yaml)
    let config = config_svc::load_project(project_path, Some(repo_root))?.ok_or_else(|| {
        MfError::usage(
            "project missing mind.yaml".to_string(),
            Some("run `mf config init` to create one".to_string()),
        )
    })?;
    let build_cfg = &config.build;

    // 2. Load index and find article
    let index = article_svc::load_index(project_path)?;
    let article_entry = index
        .articles
        .iter()
        .flat_map(|a| a.iter())
        .find(|a| {
            let expected = format!("docs/{}.md", article);
            a.source_path == expected || a.source_path.ends_with(&expected)
        })
        .ok_or_else(|| {
            let project_name = util::dir_name(project_path);
            MfError::not_found(
                format!("article '{article}' not found in project '{project_name}'"),
                Some("use `mf article list` to see available articles".to_string()),
            )
        })?;

    // 3. Resolve source file path
    let source_path = project_path.join(&article_entry.source_path);
    if !source_path.exists() {
        return Err(MfError::usage(
            format!("source file not found: {}", article_entry.source_path),
            Some("the file may have been moved or deleted".to_string()),
        ));
    }

    // 4. Determine output path
    let output_path = match output_override {
        Some(path) => path.to_path_buf(),
        None => project_path
            .join(&build_cfg.output_dir)
            .join(format!("{}.{}", article, build_cfg.format)),
    };

    // 5. Gather file metadata
    let metadata = fs::metadata(&source_path).map_err(MfError::Io)?;
    let size_bytes = metadata.len();

    // 6. Determine merge order (from config or fallback)
    let merge_order = if build_cfg.merge_order.is_empty() {
        vec![article.to_string()]
    } else {
        build_cfg.merge_order.clone()
    };

    // 7. Build plan output (for dry-run)
    let input_sources =
        vec![SourceEntry { path: article_entry.source_path.clone(), size: size_bytes }];
    let estimated_size = size_bytes; // single source, no separator overhead

    if dry_run {
        return Ok(BuildOutput::Plan(BuildPlan {
            article: article.to_string(),
            project: util::dir_name(project_path),
            source_path: source_path.to_string_lossy().to_string(),
            input_sources,
            merge_order,
            output_path: output_path.to_string_lossy().to_string(),
            size_bytes,
            estimated_size,
            dry_run: true,
        }));
    }

    // 8. Read and write output
    let content = fs::read_to_string(&source_path).map_err(MfError::Io)?;

    // Ensure output directory exists
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(MfError::Io)?;
    }

    util::atomic_write(&output_path, &content)?;
    let result = BuildResult {
        output_path: output_path.to_string_lossy().to_string(),
        size_bytes: content.len() as u64,
    };
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
