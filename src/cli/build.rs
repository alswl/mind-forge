use std::path::{Path, PathBuf};

use clap::Args;
use serde::Serialize;

use crate::cli::shared_flags::DryRunFlag;
use crate::cli::CommandCtx;
use crate::cli::CommandOutcome;
use crate::error::{MfError, Result};
use crate::service::{build as build_svc, util as svc_util};

#[derive(Debug, Clone, Args, Serialize)]
pub struct BuildArgs {
    pub article: String,
    #[arg(long = "out")]
    pub output: Option<PathBuf>,
    #[command(flatten)]
    pub dry_run: DryRunFlag,
}

pub fn dispatch(args: BuildArgs, ctx: &mut CommandCtx) -> Result<CommandOutcome> {
    let root = ctx.require_repo_path()?;

    if args.article.is_empty() {
        return Err(MfError::usage(
            format!("invalid article name: '{}'", args.article),
            Some("use `mf article list` to see available articles".to_string()),
        ));
    }

    let format = ctx.format();
    let project = ctx.project();
    let cwd = ctx.cwd();
    let (project_path, article_path, article_name) = if let Some(target) = args.article.strip_prefix('@') {
        let target_path = if Path::new(target).is_absolute() { PathBuf::from(target) } else { root.join(target) };
        let article_path = svc_util::canonicalize_within(root, &target_path)?;
        let project_path = project_root_for_source(root, &article_path)?;
        let article_name = article_path
            .file_stem()
            .or_else(|| article_path.file_name())
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| "article".to_string());
        (project_path, Some(article_path), article_name)
    } else {
        (svc_util::resolve_project(root, project, cwd)?, None, args.article.clone())
    };

    let output = match article_path {
        Some(article_path) => build_svc::build_article_path(
            &project_path,
            root,
            &article_path,
            args.dry_run.dry_run,
            args.output.as_deref(),
        )?,
        None => {
            build_svc::build_article(&project_path, root, &article_name, args.dry_run.dry_run, args.output.as_deref())?
        }
    };

    match output {
        build_svc::BuildOutput::Plan(plan) => {
            let data = serde_json::json!({
                "article": plan.article,
                "project": plan.project,
                "article_path": plan.article_path,
                "input_files": plan.input_files,
                "merge_order": plan.merge_order,
                "output_path": plan.output_path,
                "size_bytes": plan.size_bytes,
                "estimated_size": plan.estimated_size,
                "dry_run": true,
                "banner": plan.banner,
            });

            match format {
                crate::output::Format::Json => Ok(CommandOutcome::Success(data, Vec::new(), None)),
                crate::output::Format::Text => {
                    let mut lines = vec![format!("Build Plan: {}", plan.article)];
                    lines.push("  Input sources:".to_string());
                    for (i, src) in plan.input_files.iter().enumerate() {
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
                crate::output::Format::Json => Ok(CommandOutcome::Success(data, Vec::new(), None)),
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

fn project_root_for_source(repo_root: &Path, article_path: &Path) -> Result<PathBuf> {
    let mut current = if article_path.is_dir() {
        article_path.to_path_buf()
    } else {
        article_path.parent().unwrap_or(article_path).to_path_buf()
    };

    loop {
        if current.join("mind.yaml").exists() {
            return Ok(current);
        }
        if current == repo_root {
            return Err(MfError::usage(
                format!("path '{}' is not under a Mind Project", article_path.display()),
                Some("choose a path below a directory containing mind.yaml".to_string()),
            ));
        }
        current = current
            .parent()
            .ok_or_else(|| {
                MfError::usage(
                    format!("path '{}' is not under a Mind Project", article_path.display()),
                    Some("choose a path below a directory containing mind.yaml".to_string()),
                )
            })?
            .to_path_buf();
    }
}
