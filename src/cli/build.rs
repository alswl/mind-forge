use std::path::PathBuf;

use clap::Args;
use serde::Serialize;

use crate::cli::CommandOutcome;
use crate::error::{MfError, Result};
use crate::service::{build as build_svc, util as svc_util};

#[derive(Debug, Clone, Args, Serialize)]
pub struct BuildArgs {
    pub article: String,
    #[arg(long)]
    pub project: Option<String>,
    #[arg(long)]
    pub output: Option<PathBuf>,
    #[arg(long = "dry-run")]
    pub dry_run: bool,
}

pub fn dispatch(
    args: BuildArgs,
    repo_root: Option<&PathBuf>,
    format: crate::output::Format,
) -> Result<CommandOutcome> {
    let root = repo_root.ok_or_else(MfError::not_in_mind_repo)?;

    if args.article.is_empty() || args.article.contains('/') || args.article.contains('\\') {
        return Err(MfError::usage(
            format!("invalid article name: '{}'", args.article),
            Some("use `mf article list` to see available articles".to_string()),
        ));
    }

    let cwd = std::env::current_dir().map_err(MfError::Io)?;
    let project_path = svc_util::resolve_project(root, args.project.as_deref(), &cwd)?;

    match build_svc::build_article(
        &project_path,
        root,
        &args.article,
        args.dry_run,
        args.output.as_deref(),
    )? {
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
                    Ok(CommandOutcome::Raw(lines.join("\n"), None))
                }
            }
        }
        build_svc::BuildOutput::Rendered(result) => {
            let data = serde_json::json!({
                "article": args.article,
                "output": result.output_path,
                "size_bytes": result.size_bytes,
            });

            match format {
                crate::output::Format::Json => Ok(CommandOutcome::Success(data, None)),
                crate::output::Format::Text => {
                    let size_kb = format!("{:.1}", result.size_bytes as f64 / 1024.0);
                    let msg = format!(
                        "Article built: {}\n  Output: {}\n  Size: {} KB",
                        args.article, result.output_path, size_kb
                    );
                    Ok(CommandOutcome::Raw(msg, None))
                }
            }
        }
    }
}
