use super::*;

pub(super) fn handle_rename(args: TermRenameArgs, ctx: &CommandCtx) -> Result<CommandOutcome> {
    let root = ctx.require_repo_path()?;
    if args.dry_run.dry_run {
        let result = VerbResult {
            verb: Verb::Rename,
            kind: "term",
            identity: args.new_term.clone(),
            old_identity: Some(args.old_term.clone()),
            path: None,
            dry_run: true,
            details: serde_json::json!({"keep_alias": args.keep_alias}),
        };
        return match ctx.format() {
            Format::Json => Ok(CommandOutcome::Success(verb_json(&result), Vec::new(), None)),
            Format::Text => Ok(CommandOutcome::Success(
                serde_json::Value::String(verb_text(&result, &VerbOpts::from_repo_root(Some(root)))),
                Vec::new(),
                None,
            )),
        };
    }

    let mut rename_warnings: Vec<String> = Vec::new();
    let report = if let Some(project_name) = ctx.project() {
        let project_path = svc_util::resolve_project(root, Some(project_name), ctx.cwd())?;
        match term_svc::rename_term(
            &project_path,
            &args.old_term,
            &args.new_term,
            args.keep_alias,
            args.force.force,
            false,
        ) {
            Ok(report) => report,
            Err(MfError::NotFound { .. }) => {
                let report = term_svc::global::rename_term(
                    root,
                    &args.old_term,
                    &args.new_term,
                    args.keep_alias,
                    args.force.force,
                    false,
                )?;
                emit_scope_fallback_warning(project_name, &args.old_term, &mut rename_warnings);
                report
            }
            Err(e) => return Err(e),
        }
    } else {
        term_svc::global::rename_term(root, &args.old_term, &args.new_term, args.keep_alias, args.force.force, false)?
    };

    let result = VerbResult {
        verb: Verb::Rename,
        kind: "term",
        identity: report.after.name.clone(),
        old_identity: Some(report.before.name.clone()),
        path: None,
        dry_run: false,
        details: serde_json::json!({"keep_alias": args.keep_alias}),
    };
    match ctx.format() {
        Format::Json => Ok(CommandOutcome::Success(verb_json(&result), rename_warnings, None)),
        Format::Text => Ok(CommandOutcome::Success(
            serde_json::Value::String(verb_text(&result, &VerbOpts::from_repo_root(Some(root)))),
            rename_warnings,
            None,
        )),
    }
}

// ── Correction subcommand dispatch ──────────────────────────────────────────
