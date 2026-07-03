use super::*;

pub(super) fn handle_move(args: TermMoveArgs, ctx: &CommandCtx) -> Result<CommandOutcome> {
    let root = ctx.require_repo_path()?;
    let warnings: Vec<String> = Vec::new();

    let dst_project = args.to_project.as_deref();

    if !args.to_global && dst_project.is_none() {
        return Err(MfError::usage("must specify --to-global or --to-project <PROJECT> for the destination", None));
    }
    // Reject early so dry-run and real runs reject identical inputs.
    if args.from_global && args.to_global {
        return Err(MfError::usage("source and destination are both global; nothing to do", None));
    }

    // Resolve source path
    let src_path = if args.from_global {
        root.to_path_buf()
    } else if let Some(pn) = ctx.project() {
        crate::service::util::resolve_project(root, Some(pn), ctx.cwd())?
    } else {
        root.to_path_buf()
    };

    // Resolve destination path
    let dst_path = if args.to_global {
        root.to_path_buf()
    } else if let Some(pn) = dst_project {
        crate::service::util::resolve_project(root, Some(pn), ctx.cwd())?
    } else {
        root.to_path_buf()
    };

    if args.dry_run.dry_run {
        let result = VerbResult {
            verb: Verb::Move,
            kind: "term",
            identity: args.term.clone(),
            old_identity: None,
            path: None,
            dry_run: true,
            details: serde_json::json!({
                "from_scope": if args.from_global { "global" } else { "project" },
                "to_scope": if args.to_global { "global" } else { "project" },
                "force": args.force.force,
            }),
        };
        return match ctx.format() {
            Format::Json => Ok(CommandOutcome::Success(verb_json(&result), warnings, None)),
            Format::Text => Ok(CommandOutcome::Success(
                serde_json::Value::String(verb_text(&result, &VerbOpts::from_repo_root(Some(root)))),
                warnings,
                None,
            )),
        };
    }

    let outcome = if args.to_global {
        term_svc::move_::move_project_to_global(&src_path, &dst_path, &args.term, args.force.force)?
    } else if args.from_global {
        term_svc::move_::move_global_to_project(&src_path, &dst_path, &args.term, args.force.force)?
    } else {
        term_svc::move_::move_project_to_project(&src_path, &dst_path, &args.term, args.force.force)?
    };

    let result = VerbResult {
        verb: Verb::Move,
        kind: "term",
        identity: outcome.term.term.clone(),
        old_identity: None,
        path: None,
        dry_run: false,
        details: serde_json::json!({
            "from_scope": outcome.from_scope,
            "to_scope": outcome.to_scope,
            "side_effects": outcome.side_effects,
        }),
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
