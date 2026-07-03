use super::*;

pub(super) fn handle_correction(cmd: TermCorrectionCmd, ctx: &CommandCtx) -> Result<CommandOutcome> {
    use TermCorrectionSubcommand::*;
    match cmd.command {
        Add(args) => handle_correction_add(args, ctx),
        List(args) => handle_correction_list(args, ctx),
        Show(args) => handle_correction_show(args, ctx),
        Update(args) => handle_correction_update(args, ctx),
        Remove(args) => handle_correction_remove(args, ctx),
    }
}

fn correction_scope(root: &std::path::Path, ctx: &CommandCtx) -> Result<(String, std::path::PathBuf)> {
    if let Some(pn) = ctx.project() {
        let pp = crate::service::util::resolve_project(root, Some(pn), ctx.cwd())?;
        Ok(("project".to_string(), pp))
    } else {
        Ok(("global".to_string(), root.to_path_buf()))
    }
}

fn handle_correction_add(args: TermCorrectionAddArgs, ctx: &CommandCtx) -> Result<CommandOutcome> {
    let root = ctx.require_repo_path()?;
    let (scope, scope_path) = correction_scope(root, ctx)?;
    let warnings: Vec<String> = Vec::new();

    let (corr, created) = if scope == "project" {
        term_svc::correction::add_correction(
            &scope_path,
            &args.term,
            &args.original,
            &args.correct,
            parse_opt_match(args.r#match.as_deref())?,
            parse_opt_fix(args.fix.as_deref())?,
            parse_opt_boundary(args.boundary.as_deref())?,
            args.pinyin.map(|s| if s.is_empty() { None } else { Some(s) }),
        )?
    } else {
        term_svc::correction::add_correction_global(
            &scope_path,
            &args.term,
            &args.original,
            &args.correct,
            parse_opt_match(args.r#match.as_deref())?,
            parse_opt_fix(args.fix.as_deref())?,
            parse_opt_boundary(args.boundary.as_deref())?,
            args.pinyin.map(|s| if s.is_empty() { None } else { Some(s) }),
        )?
    };

    let data = serde_json::json!({
        "term": args.term,
        "scope": scope,
        "created": created,
        "correction": serde_json::to_value(&corr).unwrap_or_default(),
    });

    match ctx.format() {
        Format::Json => Ok(CommandOutcome::Success(data, warnings, None)),
        Format::Text => Ok(CommandOutcome::Success(
            serde_json::Value::String(if created {
                format!("added correction \"{}\" → \"{}\" to term \"{}\"", corr.original, corr.correct, args.term)
            } else {
                format!(
                    "correction \"{}\" → \"{}\" already exists on term \"{}\", skipped",
                    corr.original, corr.correct, args.term
                )
            }),
            warnings,
            None,
        )),
    }
}

fn handle_correction_list(args: TermCorrectionListArgs, ctx: &CommandCtx) -> Result<CommandOutcome> {
    let root = ctx.require_repo_path()?;
    let (scope, scope_path) = correction_scope(root, ctx)?;

    let corrections = if scope == "project" {
        term_svc::correction::list_corrections(&scope_path, &args.term)?
    } else {
        term_svc::correction::list_corrections_global(&scope_path, &args.term)?
    };

    match ctx.format() {
        Format::Json => {
            let items: Vec<serde_json::Value> =
                corrections.iter().map(|c| serde_json::to_value(c).unwrap_or_default()).collect();
            Ok(CommandOutcome::Success(
                serde_json::json!({"term": args.term, "scope": scope, "corrections": items}),
                Vec::new(),
                None,
            ))
        }
        Format::Text => {
            if corrections.is_empty() {
                return Ok(CommandOutcome::Raw(format!("no corrections for term \"{}\"", args.term), None));
            }
            let mut lines = vec![format!("corrections for term \"{}\":", args.term)];
            for c in &corrections {
                lines.push(format!(
                    "  \"{}\" → \"{}\" [match={}, fix={}]",
                    c.original,
                    c.correct,
                    match_to_str(&c.r#match),
                    fix_to_str(&c.fix)
                ));
            }
            Ok(CommandOutcome::Raw(lines.join("\n"), None))
        }
    }
}

fn handle_correction_show(args: TermCorrectionShowArgs, ctx: &CommandCtx) -> Result<CommandOutcome> {
    let root = ctx.require_repo_path()?;
    let (scope, scope_path) = correction_scope(root, ctx)?;

    let corr = if scope == "project" {
        term_svc::correction::show_correction(&scope_path, &args.term, &args.original)?
    } else {
        term_svc::correction::show_correction_global(&scope_path, &args.term, &args.original)?
    };

    match ctx.format() {
        Format::Json => Ok(CommandOutcome::Success(
            serde_json::json!({"term": args.term, "scope": scope, "correction": serde_json::to_value(&corr).unwrap_or_default()}),
            Vec::new(),
            None,
        )),
        Format::Text => Ok(CommandOutcome::Raw(
            format!(
                "correction \"{}\" → \"{}\"\n  match: {}\n  fix: {}\n  boundary: {}{}",
                corr.original,
                corr.correct,
                match_to_str(&corr.r#match),
                fix_to_str(&corr.fix),
                boundary_to_str(&corr.boundary),
                corr.pinyin.map_or(String::new(), |p| format!("\n  pinyin: {p}")),
            ),
            None,
        )),
    }
}

fn handle_correction_update(args: TermCorrectionUpdateArgs, ctx: &CommandCtx) -> Result<CommandOutcome> {
    let root = ctx.require_repo_path()?;
    let (scope, scope_path) = correction_scope(root, ctx)?;

    let pinyin_val = args.pinyin.map(|s| if s.is_empty() { None } else { Some(s) });

    let corr = if scope == "project" {
        term_svc::correction::update_correction(
            &scope_path,
            &args.term,
            &args.original,
            args.correct,
            parse_opt_match(args.r#match.as_deref())?,
            parse_opt_fix(args.fix.as_deref())?,
            parse_opt_boundary(args.boundary.as_deref())?,
            pinyin_val,
        )?
    } else {
        term_svc::correction::update_correction_global(
            &scope_path,
            &args.term,
            &args.original,
            args.correct,
            parse_opt_match(args.r#match.as_deref())?,
            parse_opt_fix(args.fix.as_deref())?,
            parse_opt_boundary(args.boundary.as_deref())?,
            pinyin_val,
        )?
    };

    match ctx.format() {
        Format::Json => Ok(CommandOutcome::Success(
            serde_json::json!({"term": args.term, "scope": scope, "correction": serde_json::to_value(&corr).unwrap_or_default()}),
            Vec::new(),
            None,
        )),
        Format::Text => Ok(CommandOutcome::Success(
            serde_json::Value::String(format!("updated correction \"{}\" on term \"{}\"", args.original, args.term)),
            Vec::new(),
            None,
        )),
    }
}

fn handle_correction_remove(args: TermCorrectionRemoveArgs, ctx: &CommandCtx) -> Result<CommandOutcome> {
    let root = ctx.require_repo_path()?;
    let (scope, scope_path) = correction_scope(root, ctx)?;
    let warnings: Vec<String> = Vec::new();

    if args.dry_run.dry_run {
        let result = VerbResult {
            verb: Verb::Remove,
            kind: "correction",
            identity: format!("{}/{}", args.term, args.original),
            old_identity: None,
            path: None,
            dry_run: true,
            details: serde_json::json!({"scope": scope}),
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

    let corr = if scope == "project" {
        term_svc::correction::remove_correction(&scope_path, &args.term, &args.original)?
    } else {
        term_svc::correction::remove_correction_global(&scope_path, &args.term, &args.original)?
    };

    match ctx.format() {
        Format::Json => Ok(CommandOutcome::Success(
            serde_json::json!({"term": args.term, "scope": scope, "removed": serde_json::to_value(&corr).unwrap_or_default()}),
            warnings,
            None,
        )),
        Format::Text => Ok(CommandOutcome::Success(
            serde_json::Value::String(format!(
                "removed correction \"{}\" → \"{}\" from term \"{}\"",
                corr.original, corr.correct, args.term
            )),
            warnings,
            None,
        )),
    }
}
