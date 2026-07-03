use super::*;

fn build_fix_selection(args: &TermLintArgs) -> Result<crate::model::term::FixSelection> {
    let mut selection = crate::model::term::FixSelection {
        excluded_terms: args.exclude_term.iter().cloned().collect(),
        excluded_originals: args.exclude_original.iter().cloned().collect(),
        include_suggested: args.lint.include_suggested,
        min_confidence: args.lint.min_confidence,
        ..Default::default()
    };
    for raw in &args.term {
        if let Some((term, original)) = raw.split_once(':') {
            if term.is_empty() || original.is_empty() {
                return Err(MfError::usage(
                    format!("invalid qualified term selector '{raw}'"),
                    Some("use --term NAME or --term NAME:ORIGINAL".to_string()),
                ));
            }
            selection.selected_pairs.insert((term.to_string(), original.to_string()));
        } else if raw.is_empty() {
            return Err(MfError::usage("term selector cannot be empty", None));
        } else {
            selection.selected_terms.insert(raw.clone());
        }
    }
    selection.validate().map_err(|message| MfError::usage(message, None))?;
    Ok(selection)
}

pub(super) fn handle_lint(args: TermLintArgs, ctx: &CommandCtx) -> Result<CommandOutcome> {
    use std::io::IsTerminal;

    let root = ctx.require_repo_path()?;
    let effective_fix = args.lint.fix;
    let effective_dry_run = args.lint.fix && args.lint.dry_run;
    let selection = build_fix_selection(&args)?;
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
                    term_svc::lint_path_with_global_selection(&pp, root, path, true, true, &selection)?
                } else {
                    term_svc::lint_terms_with_global_selection(&pp, root, true, true, &selection)?
                }
            } else if let Some(ref path) = args.path {
                term_svc::global::lint_path_selection(root, path, true, true, &selection)?
            } else {
                term_svc::global::lint_terms_selection(root, true, true, &selection)?
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
    } else if let Some(path) = args.path.as_deref() {
        let candidate = std::path::Path::new(path);
        if candidate.is_dir() || ctx.cwd().join(candidate).is_dir() || root.join(candidate).is_dir() {
            "directory"
        } else {
            "file"
        }
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
            term_svc::lint_path_with_global_selection(
                &project_path,
                root,
                &rel.to_string_lossy(),
                effective_fix,
                effective_dry_run,
                &selection,
            )?
        } else {
            term_svc::lint_terms_with_global_selection(
                &project_path,
                root,
                effective_fix,
                effective_dry_run,
                &selection,
            )?
        }
    } else if let Some(ref path) = args.path {
        let resolved = svc_util::path::resolve_lint_path(path, None, cwd, root)?;
        term_svc::global::lint_path_selection(
            root,
            &resolved.to_string_lossy(),
            effective_fix,
            effective_dry_run,
            &selection,
        )?
    } else {
        term_svc::global::lint_terms_selection(root, effective_fix, effective_dry_run, &selection)?
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
        let mode = if fix && dry_run { " (dry-run preview)" } else { "" };
        lines.push(format!("target: {tt}{mode}"));
    }

    if fix {
        if report.findings.is_empty() && report.failures.is_empty() {
            return "No term issues found.".to_string();
        }
        if dry_run {
            for f in &report.findings {
                let confidence = f.confidence.map_or_else(|| "none".to_string(), |value| format!("{value:.2}"));
                let selection = serde_json::to_value(f.selection)
                    .ok()
                    .and_then(|value| value.as_str().map(str::to_string))
                    .unwrap_or_else(|| "ineligible".to_string());
                let match_kind = serde_json::to_value(f.match_kind)
                    .ok()
                    .and_then(|value| value.as_str().map(str::to_string))
                    .unwrap_or_else(|| "word".to_string());
                lines.push(format!(
                    "{}:{}:{}: \"{}\" → \"{}\" [{}] match={} confidence={} selection={}",
                    f.path, f.line, f.column, f.original, f.correct, f.term, match_kind, confidence, selection
                ));
                lines.push(format!("  {}", f.context));
            }
        } else {
            // Group by path for modified files
            for path in &report.modified_files {
                let count = report
                    .findings
                    .iter()
                    .filter(|f| f.path.as_str() == path.as_str() && f.selection.is_selected())
                    .count();
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
                return if target_type == Some("directory") {
                    "No eligible files found.".to_string()
                } else {
                    "No terms registered.".to_string()
                };
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
            "{total_findings} findings in {file_count} files (selected {wf}, excluded {}, below confidence {}, ineligible {}, {} failure{})",
            report.excluded_count,
            report.below_confidence_count,
            report.ineligible_count,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::term::{Boundary, FindingSelection, FixKind, MatchKind, TermFinding, TermLintReport};

    #[test]
    fn dry_run_formatter_lists_each_finding_and_selection_reason() {
        let report = TermLintReport {
            findings: vec![TermFinding {
                path: "synthetic.md".into(),
                line: 2,
                column: 4,
                original: "old".into(),
                correct: "new".into(),
                term: "Synthetic".into(),
                description: None,
                confidence: Some(0.9),
                replacement_eligible: true,
                safety_reason: None,
                candidates: vec![],
                match_kind: MatchKind::Word,
                fix_kind: FixKind::Suggested,
                boundary: Boundary::Standalone,
                boundary_mode: "standalone",
                selection: FindingSelection::Selected,
                context: "context with old token".into(),
            }],
            scanned_files: 1,
            skipped_files: vec![],
            fixed_count: 0,
            modified_files: vec![],
            failures: vec![],
            would_fix_count: Some(1),
            would_apply_count: 0,
            selected_count: 1,
            excluded_count: 0,
            below_confidence_count: 0,
            ineligible_count: 0,
        };
        let output = format_lint_text(&report, true, true);
        assert!(output.contains("synthetic.md:2:4"));
        assert!(output.contains("\"old\" → \"new\""));
        assert!(output.contains("match=word"));
        assert!(output.contains("selection=selected"));
        assert!(output.contains("context with old token"));
        assert!(output.contains("selected 1"));
    }
}
