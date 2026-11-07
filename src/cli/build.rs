use std::path::{Path, PathBuf};

use clap::Args;
use serde::Serialize;

use crate::cli::CommandCtx;
use crate::cli::CommandOutcome;
use crate::cli::shared_flags::DryRunFlag;
use crate::error::{MfError, Result};
use crate::output::warning::emit_warning;
use crate::service::{build as build_svc, util as svc_util};

#[derive(Debug, Clone, Args, Serialize)]
pub struct BuildArgs {
    /// Mind/output identity, unique output name, or legacy article path
    pub article: String,
    /// Artifact format (independent of the global --output envelope format)
    #[arg(long = "format", value_name = "FORMAT", value_enum, default_value_t = build_svc::BuildFormat::Markdown)]
    pub artifact_format: build_svc::BuildFormat,
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
        let project_path = svc_util::project_root_for_source(root, &article_path)?;
        let article_name = article_path
            .file_stem()
            .or_else(|| article_path.file_name())
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| "article".to_string());
        (project_path, Some(article_path), article_name)
    } else {
        (crate::service::mind::project_for(root, cwd, project, Some(&args.article))?, None, args.article.clone())
    };

    // Bug #22: `--out` must be normalized against the same canonical path
    // space as the source article. Otherwise the build service can compare
    // paths like `/private/var/...` and `/var/...` as if they were unrelated,
    // then emit a relative path that climbs to the filesystem root before
    // descending into the asset tree.
    let output_override: Option<PathBuf> = args.output.as_deref().map(|p| normalize_output_override(cwd, p));

    let location = if crate::service::mind::config_for(&project_path)?.content_layout.is_some() {
        let selector = article_path
            .as_ref()
            .map(|p| svc_util::rel_posix_path(&project_path, p))
            .transpose()?
            .unwrap_or_else(|| article_name.clone());
        Some(crate::service::mind::resolve_output(root, &project_path, &selector)?)
    } else {
        None
    };

    let output = match article_path {
        Some(article_path) => build_svc::build_article_path_format(
            &project_path,
            root,
            &article_path,
            args.dry_run.dry_run,
            output_override.as_deref(),
            args.artifact_format,
        )?,
        None => build_svc::build_article_format(
            &project_path,
            root,
            &article_name,
            args.dry_run.dry_run,
            output_override.as_deref(),
            args.artifact_format,
        )?,
    };

    match output {
        build_svc::BuildOutput::Plan(plan) => {
            let data = serde_json::json!({
                "article": plan.article,
                "location": location,
                "format": args.artifact_format,
                "project": plan.project,
                "article_path": plan.article_path,
                "input_files": plan.input_files,
                "merge_order": plan.merge_order,
                "output_path": plan.output_path,
                "size_bytes": plan.size_bytes,
                "estimated_size": plan.estimated_size,
                "dry_run": true,
                "banner": plan.banner,
                "strip_first_h1": plan.strip_first_h1,
                "pipeline": plan.pipeline,
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
                    for stage in &plan.pipeline {
                        lines.push(format!(
                            "  Pipeline: {} {} -> {} [{}]",
                            stage.rule, stage.input, stage.output, stage.action
                        ));
                    }
                    Ok(CommandOutcome::Raw(lines.join("\n"), None))
                }
            }
        }
        build_svc::BuildOutput::Rendered(result) => {
            let data = serde_json::json!({
                "article": article_name,
                "location": location,
                "format": args.artifact_format,
                "output": result.output_path,
                "size_bytes": result.size_bytes,
            });

            // Bug #22 defense in depth: any reference the service could not
            // safely rewrite is reported here (stderr + JSON envelope) rather
            // than silently written as a malformed path. This is the only
            // emit site: the `Text` arm below must not loop over
            // `result.warnings` again, or every warning hits stderr twice.
            let mut warnings = Vec::new();
            for w in &result.warnings {
                emit_warning(w, &mut warnings);
            }

            match format {
                crate::output::Format::Json => Ok(CommandOutcome::Success(data, warnings, None)),
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

fn normalize_output_override(cwd: &Path, output: &Path) -> PathBuf {
    let path = if output.is_absolute() {
        output.to_path_buf()
    } else {
        let cwd_canonical = cwd.canonicalize().unwrap_or_else(|_| cwd.to_path_buf());
        cwd_canonical.join(output)
    };
    crate::service::mind::canonicalize_existing_prefix(&path)
}
