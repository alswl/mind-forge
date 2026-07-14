use super::*;

fn update_input<'a>(args: &'a TermUpdateArgs) -> term_svc::TermUpdate<'a> {
    term_svc::TermUpdate {
        definition: args.definition.as_deref(),
        description: args.description.as_deref(),
        clear_description: args.clear_description,
        confidence: args.confidence,
        clear_confidence: args.clear_confidence,
        aliases: &args.alias,
        tags: &args.tag,
        delete_aliases: &args.delete_alias,
        delete_tags: &args.delete_tag,
        add_corrections: &args.add_correction,
        delete_corrections: &args.delete_correction,
        correction_matches: &args.correction_match,
        correction_fixes: &args.correction_fix,
        correction_pinyins: &args.correction_pinyin,
    }
}

pub(super) fn handle_update(args: TermUpdateArgs, ctx: &CommandCtx) -> Result<CommandOutcome> {
    // US1: --misrecognition is unsupported on term update
    if !args.misrecognition.is_empty() {
        return Err(MfError::usage(
            "--misrecognition is not supported on `term update`; use `mf term correction add` to add a correction to an existing term, or `mf term new --misrecognition` when creating a new term",
            Some("use `mf term correction add <TERM> <ORIGINAL> <CORRECT>` to add a correction".to_string()),
        ));
    }
    if args.description.is_some() && args.clear_description {
        return Err(MfError::usage("--description and --clear-description are mutually exclusive", None));
    }
    if args.confidence.is_some() && args.clear_confidence {
        return Err(MfError::usage("--confidence and --clear-confidence are mutually exclusive", None));
    }

    let root = ctx.require_repo_path()?;

    let update = update_input(&args);
    let mut warnings: Vec<String> = Vec::new();

    if args.dry_run.dry_run {
        return handle_update_dry_run(&args, &update, root, ctx, &mut warnings);
    }
    let (term, global_fallback) = if let Some(project_name) = ctx.project() {
        let project_path = svc_util::resolve_project(root, Some(project_name), ctx.cwd())?;
        match term_svc::fix_term(&project_path, &args.term, update) {
            Ok(term) => (term, false),
            Err(MfError::NotFound { .. }) => {
                // Try global fallback; emit the WARN only when that write actually succeeds.
                let term = term_svc::global::fix_term(root, &args.term, update)?;
                emit_scope_fallback_warning(project_name, &args.term, &mut warnings);
                (term, true)
            }
            Err(e) => return Err(e),
        }
    } else {
        (term_svc::global::fix_term(root, &args.term, update)?, false)
    };

    let mut changes = serde_json::Map::new();
    if args.definition.is_some() {
        changes.insert("definition".to_string(), serde_json::json!({"changed": true}));
    }
    if args.description.is_some() || args.clear_description {
        changes.insert("description".to_string(), serde_json::json!({"changed": true}));
    }
    if args.confidence.is_some() || args.clear_confidence {
        changes.insert("confidence".to_string(), serde_json::json!({"changed": true}));
    }
    if !args.alias.is_empty() {
        changes.insert("aliases".to_string(), serde_json::json!({"added": args.alias}));
    }
    if !args.delete_alias.is_empty() {
        changes.insert("aliases".to_string(), serde_json::json!({"deleted": args.delete_alias}));
    }
    if !args.tag.is_empty() {
        changes.insert("tags".to_string(), serde_json::json!({"added": args.tag}));
    }
    if !args.delete_tag.is_empty() {
        changes.insert("tags".to_string(), serde_json::json!({"deleted": args.delete_tag}));
    }
    // Group add + delete under one `corrections` object so a combined
    // `--add-correction … --delete-correction …` call reports both (a second
    // `insert` on the same key would otherwise overwrite the first).
    if !args.add_correction.is_empty() || !args.delete_correction.is_empty() {
        let mut corrections = serde_json::Map::new();
        if !args.add_correction.is_empty() {
            corrections.insert("added".to_string(), serde_json::json!(args.add_correction));
        }
        if !args.delete_correction.is_empty() {
            corrections.insert("deleted".to_string(), serde_json::json!(args.delete_correction));
        }
        changes.insert("corrections".to_string(), serde_json::Value::Object(corrections));
    }
    if !args.correction_match.is_empty() {
        changes.insert("correction_match".to_string(), serde_json::json!(args.correction_match));
    }
    if !args.correction_fix.is_empty() {
        changes.insert("correction_fix".to_string(), serde_json::json!(args.correction_fix));
    }
    if !args.correction_pinyin.is_empty() {
        changes.insert("correction_pinyin".to_string(), serde_json::json!(args.correction_pinyin));
    }

    let mut result = VerbResult {
        verb: Verb::Update,
        kind: "term",
        identity: term.term.clone(),
        old_identity: None,
        path: None,
        dry_run: false,
        details: serde_json::json!({"changes": changes}),
    };
    if global_fallback && let serde_json::Value::Object(ref mut map) = result.details {
        map.insert("scope".to_string(), serde_json::json!("global"));
    }
    match ctx.format() {
        Format::Json => Ok(CommandOutcome::Success(verb_json(&result), warnings, None)),
        Format::Text => Ok(CommandOutcome::Success(
            serde_json::Value::String(verb_text(&result, &VerbOpts::from_repo_root(Some(root)))),
            warnings,
            None,
        )),
    }
}

// ── Handle: mf term update (dry-run) ─────────────────────────────────────

fn handle_update_dry_run(
    args: &TermUpdateArgs,
    update: &term_svc::TermUpdate<'_>,
    root: &std::path::Path,
    ctx: &CommandCtx,
    warnings: &mut Vec<String>,
) -> Result<CommandOutcome> {
    // Resolve which term would be targeted (project or global)
    let scope = resolve_update_target(root, ctx, &args.term)?;

    // Build planned changes list
    let mut planned = Vec::new();
    if update.definition.is_some() {
        planned.push("definition".to_string());
    }
    if update.description.is_some() || update.clear_description {
        planned.push("description".to_string());
    }
    if update.confidence.is_some() || update.clear_confidence {
        planned.push("confidence".to_string());
    }
    if !update.aliases.is_empty() {
        planned.push(format!("add {} alias(es)", update.aliases.len()));
    }
    if !update.tags.is_empty() {
        planned.push(format!("add {} tag(s)", update.tags.len()));
    }
    if !update.delete_aliases.is_empty() {
        planned.push(format!("remove {} alias(es)", update.delete_aliases.len()));
    }
    if !update.delete_tags.is_empty() {
        planned.push(format!("remove {} tag(s)", update.delete_tags.len()));
    }
    // Correction operations must be previewed too (New A: dry-run parity).
    if !update.add_corrections.is_empty() {
        planned.push(format!("add {} correction(s)", update.add_corrections.len()));
    }
    if !update.delete_corrections.is_empty() {
        planned.push(format!("remove {} correction(s)", update.delete_corrections.len()));
    }
    if !update.correction_matches.is_empty() {
        planned.push(format!("set match kind on {} correction(s)", update.correction_matches.len()));
    }
    if !update.correction_fixes.is_empty() {
        planned.push(format!("set fix kind on {} correction(s)", update.correction_fixes.len()));
    }
    if !update.correction_pinyins.is_empty() {
        planned.push(format!("set pinyin on {} correction(s)", update.correction_pinyins.len()));
    }

    let scope_str = scope.as_str();
    if scope_str == "global"
        && let Some(pn) = ctx.project()
    {
        emit_scope_fallback_warning(pn, &args.term, warnings);
    }

    let result = VerbResult {
        verb: Verb::Update,
        kind: "term",
        identity: args.term.clone(),
        old_identity: None,
        path: None,
        dry_run: true,
        details: serde_json::json!({
            "scope": scope_str,
            "changes": planned,
        }),
    };

    match ctx.format() {
        Format::Json => Ok(CommandOutcome::Success(verb_json(&result), warnings.clone(), None)),
        Format::Text => Ok(CommandOutcome::Success(
            serde_json::Value::String(verb_text(&result, &VerbOpts::from_repo_root(Some(root)))),
            warnings.clone(),
            None,
        )),
    }
}

/// Resolve which scope an update would target (project or global) without
/// writing anything. The `show_term` lookups also validate that the term
/// exists, propagating a not-found error when it does not.
fn resolve_update_target(root: &std::path::Path, ctx: &CommandCtx, term_name: &str) -> Result<term_svc::WriteScope> {
    use term_svc::WriteScope;

    if let Some(project_name) = ctx.project() {
        let project_path = crate::service::util::resolve_project(root, Some(project_name), ctx.cwd())?;
        match term_svc::show_term(&project_path, term_name) {
            Ok(_) => Ok(WriteScope::Project(project_path)),
            Err(MfError::NotFound { .. }) => {
                term_svc::global::show_term(root, term_name)?;
                Ok(WriteScope::Global(root.to_path_buf()))
            }
            Err(e) => Err(e),
        }
    } else {
        term_svc::global::show_term(root, term_name)?;
        Ok(WriteScope::Global(root.to_path_buf()))
    }
}
