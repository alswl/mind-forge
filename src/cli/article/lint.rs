use super::*;

pub(super) fn handle_lint(args: ArticleLintArgs, project_path: &Path, format: Format) -> Result<CommandOutcome> {
    let fix = args.lint.fix;
    let dry_run = args.lint.dry_run;

    let issues = article_svc::lint_articles(project_path, fix && !dry_run)?;

    // Apply --rule filter
    let filtered: Vec<_> = if args.lint.rule.is_empty() {
        issues
    } else {
        issues.into_iter().filter(|i| args.lint.rule.iter().any(|r| r == &i.kind)).collect()
    };

    // Apply --severity filter
    let severity_level = severity_rank(args.lint.severity.as_deref());
    let filtered: Vec<_> =
        filtered.into_iter().filter(|i| severity_rank(Some(&i.severity)) <= severity_level).collect();

    // Compute summary
    let errors = filtered.iter().filter(|i| i.severity == "error").count() as u64;
    let warnings = filtered.iter().filter(|i| i.severity == "warning").count() as u64;
    let info = filtered.iter().filter(|i| i.severity == "info").count() as u64;
    let fixed_count = 0u64;

    let json_issues: Vec<serde_json::Value> =
        filtered.iter().map(|i| serde_json::to_value(i).unwrap_or_default()).collect();

    let details = serde_json::json!({
        "issues": json_issues,
        "summary": { "errors": errors, "warnings": warnings, "info": info, "fixed": fixed_count },
    });

    let exit_code =
        if args.lint.max_warnings.is_some_and(|max| warnings as i32 > max) || errors > 0 { Some(1) } else { None };

    match format {
        Format::Json => {
            let data = serde_json::json!({ "kind": "article", "issues": json_issues, "summary": { "errors": errors, "warnings": warnings, "info": info, "fixed": fixed_count }, "dry_run": dry_run });
            Ok(CommandOutcome::Success(data, Vec::new(), exit_code))
        }
        Format::Text => {
            let result = VerbResult {
                verb: Verb::Lint,
                kind: "article",
                identity: String::new(),
                old_identity: None,
                path: None,
                dry_run,
                details,
            };
            Ok(CommandOutcome::Raw(verb_text(&result, &VerbOpts::from_repo_root(Some(project_path))), exit_code))
        }
    }
}

fn severity_rank(severity: Option<&str>) -> u8 {
    match severity {
        Some("error") => 0,
        Some("warning") => 1,
        Some("info") => 2,
        _ => 2,
    }
}

// ── Handle: mf article convert ────────────────────────────────────────────
