use super::*;

pub(super) fn handle_convert(args: ArticleConvertArgs, ctx: &mut CommandCtx) -> Result<CommandOutcome> {
    if args.merge && !args.to_single_file {
        return Err(MfError::usage(
            "--merge requires --to-single-file",
            Some("pass `--to-single-file --merge` to merge multi-block directory articles".to_string()),
        ));
    }

    let root = ctx.require_repo_path()?;
    let format = ctx.format();
    let project_path = svc_util::resolve_project(root, ctx.project(), ctx.cwd())?;

    let index = crate::service::index::load(&project_path)?;
    let article_paths: Vec<String> = index
        .articles
        .as_ref()
        .map(|articles| articles.iter().map(|a| a.article_path.clone()).collect())
        .unwrap_or_default();

    let (direction, direction_source) = match resolve_direction(&args, &project_path, &article_paths)? {
        DirectionDecision::Use { direction, source } => (direction, source),
        DirectionDecision::Declined => {
            return Ok(CommandOutcome::Success(
                serde_json::Value::String("conversion declined".to_string()),
                Vec::new(),
                None,
            ));
        }
    };

    let inspections = article_svc::plan_conversion(&project_path, &article_paths, direction, args.merge)?;

    let mut converted: Vec<ConversionResult> = Vec::new();
    let mut skipped: Vec<ConversionResult> = Vec::new();
    let mut failed: Vec<ConversionResult> = Vec::new();

    for inspection in &inspections {
        if !inspection.eligible {
            skipped.push(inspection.to_result(
                ConversionStatus::Skipped,
                direction,
                inspection.skip_reason.clone(),
                false,
                false,
            ));
            continue;
        }

        if args.dry_run.dry_run {
            converted.push(inspection.to_result(ConversionStatus::WouldConvert, direction, None, false, false));
            continue;
        }

        let exec = match direction {
            ConversionDirection::ToSingleFile => article_svc::execute_to_single_file(&project_path, inspection),
            ConversionDirection::ToDirectory => article_svc::execute_to_directory(&project_path, inspection),
        };
        match exec {
            Ok(mut conv) => {
                match article_svc::update_index_for_conversion(&project_path, &conv.source_path, &conv.target_path) {
                    Ok(()) => {
                        conv.index_updated = true;
                        // Spec 064 FR-010: keep any prompt bound to this article
                        // valid across the path change. Best-effort — a failure
                        // here does not roll back the conversion.
                        if let Err(e) = article_svc::update_prompt_binding_for_conversion(
                            &project_path,
                            &conv.source_path,
                            &conv.target_path,
                        ) {
                            tracing::warn!(
                                "failed to rebind prompt for converted article '{}': {}",
                                conv.source_path,
                                e
                            );
                        }
                        converted.push(conv);
                    }
                    Err(e) => {
                        conv.status = ConversionStatus::Failed;
                        conv.reason = Some(format!("index update failed: {}", e));
                        failed.push(conv);
                    }
                }
            }
            Err(e) => {
                failed.push(inspection.to_result(
                    ConversionStatus::Failed,
                    direction,
                    Some(format!("{}", e)),
                    false,
                    false,
                ));
            }
        }
    }

    let summary = ConversionSummary {
        kind: "article".to_string(),
        direction,
        direction_source,
        dry_run: args.dry_run.dry_run,
        converted_count: converted.len(),
        skipped_count: skipped.len(),
        failed_count: failed.len(),
        scanned_count: inspections.len(),
        converted,
        skipped,
        failed,
    };

    match format {
        Format::Json => {
            let data = serde_json::to_value(&summary).map_err(MfError::Json)?;
            Ok(CommandOutcome::Success(data, Vec::new(), None))
        }
        Format::Text => {
            Ok(CommandOutcome::Success(serde_json::Value::String(render_convert_text(&summary)), Vec::new(), None))
        }
    }
}

enum DirectionDecision {
    Use { direction: ConversionDirection, source: DirectionSource },
    Declined,
}

fn resolve_direction(
    args: &ArticleConvertArgs,
    project_path: &Path,
    article_paths: &[String],
) -> Result<DirectionDecision> {
    if args.to_single_file {
        return Ok(DirectionDecision::Use {
            direction: ConversionDirection::ToSingleFile,
            source: DirectionSource::Explicit,
        });
    }
    if args.to_directory {
        return Ok(DirectionDecision::Use {
            direction: ConversionDirection::ToDirectory,
            source: DirectionSource::Explicit,
        });
    }

    let plausible = article_svc::plausible_directions(project_path, article_paths)?;
    match plausible.as_slice() {
        [] => Err(MfError::usage(
            "no eligible articles found for conversion",
            Some("verify that the project has articles that can be converted".to_string()),
        )),
        [(direction, count)] => confirm_inferred_direction(*direction, *count),
        _ => Err(MfError::usage(
            "ambiguous conversion direction; both --to-single-file and --to-directory are possible",
            Some("pass --to-single-file or --to-directory to specify the desired direction".to_string()),
        )),
    }
}

fn confirm_inferred_direction(direction: ConversionDirection, count: usize) -> Result<DirectionDecision> {
    let prompt = format!(
        "No conversion direction specified.\nSuggested direction: {} ({} article{} can be converted)\nProceed? [y/N]: ",
        direction,
        count,
        if count == 1 { "" } else { "s" },
    );
    match prompt_confirmation(&prompt) {
        ConfirmOutcome::Confirmed => Ok(DirectionDecision::Use { direction, source: DirectionSource::Inferred }),
        ConfirmOutcome::Aborted => Ok(DirectionDecision::Declined),
        ConfirmOutcome::NotTty => Err(MfError::usage(
            format!("no conversion direction specified; eligible direction is {}", direction),
            Some(format!("pass {} or run in a terminal for interactive confirmation", direction)),
        )),
    }
}

fn render_convert_text(summary: &ConversionSummary) -> String {
    let prefix = if summary.dry_run { "[dry-run] " } else { "" };
    let convert_verb = if summary.dry_run { "would convert" } else { "converted" };
    let mut lines: Vec<String> = Vec::new();

    for r in &summary.converted {
        lines.push(format!("{prefix}{convert_verb} article: {} -> {}", r.source_path, r.target_path));
    }
    for r in &summary.skipped {
        lines.push(format!("skipped article: {} ({})", r.source_path, r.reason.as_deref().unwrap_or("unknown")));
    }
    for r in &summary.failed {
        lines.push(format!("failed article: {} ({})", r.source_path, r.reason.as_deref().unwrap_or("unknown error")));
    }

    lines.push(format!(
        "{prefix}article convert {}: {} {}, {} skipped, {} failed",
        summary.direction, summary.converted_count, convert_verb, summary.skipped_count, summary.failed_count
    ));

    lines.join("\n")
}
