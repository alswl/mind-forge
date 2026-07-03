use super::*;

pub(super) fn handle_lint(args: TermLintArgs, ctx: &CommandCtx) -> Result<CommandOutcome> {
    use std::io::IsTerminal;

    let root = ctx.require_repo_path()?;
    let effective_fix = args.lint.fix;
    let effective_dry_run = args.lint.fix && args.lint.dry_run;
    let include_suggested = args.lint.include_suggested;
    let warnings: Vec<String> = Vec::new();

    // US1: confirmation gate for --fix (non-dry-run)
    if effective_fix && !effective_dry_run && !args.yes.yes {
        if !std::io::stdout().is_terminal() {
            return Err(MfError::usage(
                "--fix in non-interactive context requires -y / --yes",
                Some("pass --yes to confirm".to_string()),
            ));
        }
        // Show dry-run preview in text mode before prompting
        if matches!(ctx.format(), Format::Text) {
            let preview = if let Some(pn) = ctx.project() {
                let pp = svc_util::resolve_project(root, Some(pn), ctx.cwd())?;
                if let Some(ref path) = args.path {
                    term_svc::lint_path_with_global(&pp, root, path, true, true, include_suggested, &args.term)?
                } else {
                    term_svc::lint_terms_with_global(&pp, root, true, true, include_suggested, &args.term)?
                }
            } else if let Some(ref path) = args.path {
                term_svc::global::lint_path(root, path, true, true, include_suggested, &args.term)?
            } else {
                term_svc::global::lint_terms(root, true, true, include_suggested, &args.term)?
            };
            if !preview.findings.is_empty() {
                println!("{}", format_lint_text(&preview, true, true));
            }
        }
        match crate::output::confirm::prompt_confirmation("Apply changes? [y/N] ") {
            crate::output::confirm::ConfirmOutcome::Confirmed => {}
            crate::output::confirm::ConfirmOutcome::Aborted => {
                return Ok(CommandOutcome::Raw("Aborted by user.".to_string(), Some(0)));
            }
            crate::output::confirm::ConfirmOutcome::NotTty => {
                return Err(MfError::usage(
                    "--fix in non-interactive context requires -y / --yes",
                    Some("pass --yes to confirm".to_string()),
                ));
            }
        }
    }

    // Determine effective target type for output
    let target_type: &str = if args.article.is_some() {
        "article"
    } else if args.path.is_some() {
        "file"
    } else {
        "project"
    };

    let cwd = ctx.cwd();
    let report = if let Some(project_name) = ctx.project() {
        let project_path = svc_util::resolve_project(root, Some(project_name), cwd)?;
        if let Some(ref path) = args.path {
            let resolved = svc_util::path::resolve_lint_path(path, Some(&project_path), cwd, root)?;
            // Canonicalize before the containment check; otherwise `..` components
            // in `resolved` slip past `strip_prefix` and let the path escape the
            // project root.
            let canon_resolved = resolved.canonicalize().map_err(MfError::Io)?;
            let canon_project = project_path.canonicalize().map_err(MfError::Io)?;
            let rel = canon_resolved.strip_prefix(&canon_project).map_err(|_| {
                MfError::usage(
                    format!("path '{}' is not under project root '{}'", resolved.display(), project_path.display()),
                    None,
                )
            })?;
            term_svc::lint_path_with_global(
                &project_path,
                root,
                &rel.to_string_lossy(),
                effective_fix,
                effective_dry_run,
                include_suggested,
                &args.term,
            )?
        } else {
            term_svc::lint_terms_with_global(
                &project_path,
                root,
                effective_fix,
                effective_dry_run,
                include_suggested,
                &args.term,
            )?
        }
    } else if let Some(ref path) = args.path {
        let resolved = svc_util::path::resolve_lint_path(path, None, cwd, root)?;
        term_svc::global::lint_path(
            root,
            &resolved.to_string_lossy(),
            effective_fix,
            effective_dry_run,
            include_suggested,
            &args.term,
        )?
    } else {
        term_svc::global::lint_terms(root, effective_fix, effective_dry_run, include_suggested, &args.term)?
    };

    // Determine exit code
    let base_exit = compute_lint_exit_code(&report, effective_fix, effective_dry_run);
    let warnings_count = report.findings.len() as i32;
    let exit_code =
        if args.lint.max_warnings.is_some_and(|max| warnings_count > max) { Some(1) } else { Some(base_exit) };

    match ctx.format() {
        Format::Json => {
            let report_value = serde_json::to_value(&report).map_err(MfError::Json)?;
            let mut data = serde_json::Map::new();
            data.insert("kind".to_string(), serde_json::Value::String("term".to_string()));
            data.insert("dry_run".to_string(), serde_json::Value::Bool(effective_dry_run));
            data.insert("target_type".to_string(), serde_json::Value::String(target_type.to_string()));
            data.insert(
                "term_filter".to_string(),
                serde_json::Value::Array(args.term.iter().map(|n| serde_json::Value::String(n.clone())).collect()),
            );
            // Flatten report fields into data
            if let serde_json::Value::Object(obj) = report_value {
                for (k, v) in obj {
                    if k != "kind" && k != "dry_run" {
                        data.insert(k, v);
                    }
                }
            }
            Ok(CommandOutcome::Success(serde_json::Value::Object(data), warnings, exit_code))
        }
        Format::Text => {
            let output = format_lint_text_with_target(&report, effective_fix, effective_dry_run, Some(target_type));
            if args.term.is_empty() {
                Ok(CommandOutcome::Raw(output, exit_code))
            } else {
                let scoped = format!("scoped to term(s): {}\n{output}", args.term.join(", "));
                Ok(CommandOutcome::Raw(scoped, exit_code))
            }
        }
    }
}

fn compute_lint_exit_code(report: &crate::model::term::TermLintReport, fix: bool, dry_run: bool) -> u8 {
    // --fix (without --dry-run) exits non-zero only on write failures.
    // All other modes (read-only and --fix --dry-run) exit non-zero when findings remain.
    let has_issue = if fix && !dry_run { !report.failures.is_empty() } else { !report.findings.is_empty() };
    u8::from(has_issue)
}

fn format_lint_text(report: &crate::model::term::TermLintReport, fix: bool, dry_run: bool) -> String {
    format_lint_text_with_target(report, fix, dry_run, None)
}

fn format_lint_text_with_target(
    report: &crate::model::term::TermLintReport,
    fix: bool,
    dry_run: bool,
    target_type: Option<&str>,
) -> String {
    let mut lines = Vec::new();

    if let Some(tt) = target_type {
        lines.push(format!("target: {tt}"));
    }

    if fix {
        if report.findings.is_empty() && report.failures.is_empty() {
            return "No term issues found.".to_string();
        }
        if dry_run {
            // Group by path
            let mut by_path: std::collections::BTreeMap<&str, u64> = std::collections::BTreeMap::new();
            for f in &report.findings {
                *by_path.entry(f.path.as_str()).or_default() += 1;
            }
            for (path, count) in &by_path {
                let s = if *count == 1 { "" } else { "s" };
                lines.push(format!("[dry-run] would fix: {path} ({count} replacement{s})"));
            }
        } else {
            // Group by path for modified files
            for path in &report.modified_files {
                let count = report.findings.iter().filter(|f| f.path.as_str() == path.as_str()).count();
                let s = if count == 1 { "" } else { "s" };
                lines.push(format!("✓ fixed: {path} ({count} replacement{s})"));
            }
            for f in &report.failures {
                lines.push(format!("✗ failed: {} — {}", f.path, f.reason));
            }
        }
    } else {
        if report.findings.is_empty() {
            if report.scanned_files == 0 && report.skipped_files.is_empty() && report.failures.is_empty() {
                return "No terms registered.".to_string();
            }
            if report.failures.is_empty() {
                return "No term issues found.".to_string();
            }
        }
        for f in &report.findings {
            if f.safety_reason.as_deref() == Some("ambiguous") {
                let mut seen = std::collections::BTreeSet::new();
                let unique: Vec<&str> =
                    f.candidates.iter().map(|c| c.term.as_str()).filter(|t| seen.insert(*t)).collect();
                lines.push(format!(
                    "{}:{}:{}: \"{}\" ambiguous: {}",
                    f.path,
                    f.line,
                    f.column,
                    f.original,
                    unique.join(", ")
                ));
            } else {
                let confidence_part = match f.confidence {
                    Some(c) => format!(" [confidence={c:.2}]"),
                    None => String::new(),
                };
                let suggested_mark = if f.fix_kind == crate::model::term::FixKind::Suggested { "?" } else { "" };
                let boundary_mark =
                    if f.boundary == crate::model::term::Boundary::Standalone { ", standalone" } else { "" };
                lines.push(format!(
                    "{}:{}:{}: \"{}\" → \"{}\" [{}]{}{}{}",
                    f.path,
                    f.line,
                    f.column,
                    f.original,
                    f.correct,
                    f.term,
                    confidence_part,
                    suggested_mark,
                    boundary_mark
                ));
            }
        }
    }

    // Summary line
    let total_findings = report.findings.len();
    let unique_files: std::collections::BTreeSet<&str> = report.findings.iter().map(|f| f.path.as_str()).collect();
    let file_count = unique_files.len();

    if fix && dry_run {
        let wf = report.would_fix_count.unwrap_or(0);
        lines.push(format!(
            "{total_findings} findings in {file_count} files (would fix {wf}, {} failure{})",
            report.failures.len(),
            if report.failures.len() == 1 { "" } else { "s" },
        ));
    } else if fix {
        lines.push(format!(
            "{total_findings} findings in {file_count} files ({} fixed, {} failure{})",
            report.fixed_count,
            report.failures.len(),
            if report.failures.len() == 1 { "" } else { "s" },
        ));
    } else {
        lines.push(format!("{total_findings} findings in {file_count} files"));
    }

    lines.join("\n")
}
