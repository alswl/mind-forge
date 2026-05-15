use std::path::{Path, PathBuf};

use clap::Args;
use serde::Serialize;

use crate::cli::deprecation::DeprecationContext;
use crate::cli::CommandOutcome;
use crate::error::{MfError, Result};
use crate::service::{build as build_svc, util as svc_util};

#[derive(Debug, Clone, Args, Serialize)]
pub struct BuildArgs {
    pub article: String,
    #[arg(short = 'p', long)]
    pub project: Option<String>,
    #[arg(short = 'o', long)]
    pub output: Option<PathBuf>,
    #[arg(long = "dry-run")]
    pub dry_run: bool,
}

pub fn dispatch(
    args: BuildArgs,
    repo_root: Option<&PathBuf>,
    format: crate::output::Format,
    _deprecation: &mut DeprecationContext,
) -> Result<CommandOutcome> {
    let root = repo_root.ok_or_else(MfError::not_in_mind_repo)?;

    if args.article.is_empty() {
        return Err(MfError::usage(
            format!("invalid article name: '{}'", args.article),
            Some("use `mf article list` to see available articles".to_string()),
        ));
    }

    let cwd = std::env::current_dir().map_err(MfError::Io)?;
    let (project_path, source_path, article_name) = if let Some(target) = args.article.strip_prefix('@') {
        let target_path = if Path::new(target).is_absolute() { PathBuf::from(target) } else { root.join(target) };
        let source_path = svc_util::canonicalize_within(root, &target_path)?;
        let project_path = project_root_for_source(root, &source_path)?;
        let article_name = source_path
            .file_stem()
            .or_else(|| source_path.file_name())
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| "article".to_string());
        (project_path, Some(source_path), article_name)
    } else {
        if args.article.contains('/') || args.article.contains('\\') {
            return Err(MfError::usage(
                format!("invalid article name: '{}'", args.article),
                Some(
                    "prefix repo-relative paths with '@', for example `mf build @projects/my-project/docs/post/`"
                        .to_string(),
                ),
            ));
        }
        (svc_util::resolve_project(root, args.project.as_deref(), &cwd)?, None, args.article.clone())
    };

    let output = match source_path {
        Some(source_path) => {
            build_svc::build_article_path(&project_path, root, &source_path, args.dry_run, args.output.as_deref())?
        }
        None => build_svc::build_article(&project_path, root, &article_name, args.dry_run, args.output.as_deref())?,
    };

    match output {
        build_svc::BuildOutput::Plan(plan) => {
            let data = serde_json::json!({
                "article": plan.article,
                "project": plan.project,
                "source_path": plan.source_path,
                "input_sources": plan.input_sources,
                "merge_order": plan.merge_order,
                "output_path": plan.output_path,
                "size_bytes": plan.size_bytes,
                "estimated_size": plan.estimated_size,
                "dry_run": true,
                "banner": plan.banner,
            });

            match format {
                crate::output::Format::Json => Ok(CommandOutcome::Success(data, None)),
                crate::output::Format::Text => {
                    let mut lines = vec![format!("Build Plan: {}", plan.article)];
                    lines.push("  Input sources:".to_string());
                    for (i, src) in plan.input_sources.iter().enumerate() {
                        let size_kb = format!("{:.1}", src.size as f64 / 1024.0);
                        lines.push(format!("    {}. {} ({} KB)", i + 1, src.path, size_kb));
                    }
                    lines.push(format!("  Merge order: {:?}", plan.merge_order));
                    lines.push(format!("  Output path: {}", plan.output_path));
                    let size_kb = format!("{:.1}", plan.estimated_size as f64 / 1024.0);
                    lines.push(format!("  Estimated size: {} KB", size_kb));
                    // Add banner info to text output if configured
                    if let Some(ref banner) = plan.banner {
                        let level_str = banner.level.as_deref().unwrap_or("raw");
                        lines.push(format!("  Banner: enabled, level={}", level_str));
                    }
                    Ok(CommandOutcome::Raw(lines.join("\n"), None))
                }
            }
        }
        build_svc::BuildOutput::Rendered(result) => {
            let data = serde_json::json!({
                "article": article_name,
                "output": result.output_path,
                "size_bytes": result.size_bytes,
            });

            match format {
                crate::output::Format::Json => Ok(CommandOutcome::Success(data, None)),
                crate::output::Format::Text => {
                    let size_kb = format!("{:.1}", result.size_bytes as f64 / 1024.0);
                    let msg = format!(
                        "Article built: {}\n  Output: {}\n  Size: {} KB",
                        article_name, result.output_path, size_kb
                    );
                    Ok(CommandOutcome::Raw(msg, None))
                }
            }
        }
    }
}

fn project_root_for_source(repo_root: &Path, source_path: &Path) -> Result<PathBuf> {
    let mut current = if source_path.is_dir() {
        source_path.to_path_buf()
    } else {
        source_path.parent().unwrap_or(source_path).to_path_buf()
    };

    loop {
        if current.join("mind.yaml").exists() {
            return Ok(current);
        }
        if current == repo_root {
            return Err(MfError::usage(
                format!("path '{}' is not under a Mind Project", source_path.display()),
                Some("choose a path below a directory containing mind.yaml".to_string()),
            ));
        }
        current = current
            .parent()
            .ok_or_else(|| {
                MfError::usage(
                    format!("path '{}' is not under a Mind Project", source_path.display()),
                    Some("choose a path below a directory containing mind.yaml".to_string()),
                )
            })?
            .to_path_buf();
    }
}
