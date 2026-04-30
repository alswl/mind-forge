use std::path::PathBuf;

use clap::Args;
use serde::Serialize;

use crate::cli::CommandOutcome;
use crate::error::{MfError, Result};
use crate::output::Format;
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
    format: Format,
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

    match build_svc::build_article(&project_path, &args.article, args.dry_run)? {
        build_svc::BuildOutput::Plan(plan) => {
            let data = serde_json::json!({
                "article": plan.article,
                "project": plan.project,
                "source_path": plan.source_path,
                "size_bytes": plan.size_bytes,
                "dry_run": true,
            });
            Ok(CommandOutcome::Success(data, None))
        }
        build_svc::BuildOutput::Content(content) => {
            if let Some(output_path) = args.output {
                std::fs::write(&output_path, &content).map_err(MfError::Io)?;
                let data = serde_json::json!({
                    "output": output_path.to_string_lossy().to_string(),
                    "size_bytes": content.len(),
                });
                Ok(CommandOutcome::Success(data, None))
            } else {
                // Print content directly in text mode
                match format {
                    Format::Json => {
                        let data = serde_json::json!({
                            "article": args.article,
                            "content": content,
                            "size_bytes": content.len(),
                        });
                        Ok(CommandOutcome::Success(data, None))
                    }
                    Format::Text => Ok(CommandOutcome::Raw(content, None)),
                }
            }
        }
    }
}
