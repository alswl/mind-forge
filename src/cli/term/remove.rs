use super::*;

pub(super) fn handle_remove(args: TermRemoveArgs, ctx: &CommandCtx) -> Result<CommandOutcome> {
    require_confirmation(&ConfirmArgs {
        verb_label: "removal",
        kind: "term",
        identity: &args.term,
        yes: args.yes.yes,
        force: args.force.force,
    })?;

    let root = ctx.require_repo_path()?;
    let mut warnings: Vec<String> = Vec::new();
    let report = if let Some(project_name) = ctx.project() {
        let project_path = svc_util::resolve_project(root, Some(project_name), ctx.cwd())?;
        match term_svc::remove_term(&project_path, &args.term, args.force.force, args.dry_run.dry_run) {
            Ok(report) => report,
            Err(MfError::NotFound { .. }) => {
                let report = term_svc::remove_term_global(root, &args.term, args.force.force, args.dry_run.dry_run)?;
                emit_scope_fallback_warning(project_name, &args.term, &mut warnings);
                report
            }
            Err(e) => return Err(e),
        }
    } else {
        term_svc::remove_term_global(root, &args.term, args.force.force, args.dry_run.dry_run)?
    };

    let result = VerbResult {
        verb: Verb::Remove,
        kind: "term",
        identity: report.before.name.clone(),
        old_identity: None,
        path: None,
        dry_run: args.dry_run.dry_run,
        details: serde_json::json!({"removed": true}),
    };
    match ctx.format() {
        Format::Json => Ok(CommandOutcome::Success(verb_json(&result), warnings, None)),
        Format::Text => Ok(CommandOutcome::Success(
            serde_json::Value::String(verb_text(&result, &VerbOpts::from_repo_root(Some(root)))),
            warnings,
            None,
        )),
    }
}
