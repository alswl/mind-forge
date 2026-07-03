use super::*;

fn new_input(args: &TermNewArgs) -> term_svc::TermInput<'_> {
    term_svc::TermInput {
        definition: args.definition.as_deref(),
        description: args.description.as_deref(),
        confidence: args.confidence,
        aliases: &args.alias,
        tags: &args.tag,
    }
}

pub(super) fn handle_new(args: TermNewArgs, ctx: &CommandCtx) -> Result<CommandOutcome> {
    let warnings: Vec<String> = Vec::new();
    let term_name = args.term.clone().ok_or_else(|| {
        MfError::usage(
            "term name required",
            Some("pass the canonical term as the positional argument: `mf term new <NAME>`".to_string()),
        )
    })?;

    if args.dry_run.dry_run {
        let has_aliases = !args.alias.is_empty();
        let mut actions = vec!["create canonical term".to_string()];
        if has_aliases {
            actions.push("attach alias".to_string());
            for alias in &args.alias {
                actions.push(format!("  alias: {alias}"));
            }
        }
        let details = serde_json::json!({
            "definition": args.definition,
            "aliases": args.alias,
            "tags": args.tag,
            "planned_actions": actions,
        });
        let result = VerbResult {
            verb: Verb::Create,
            kind: "term",
            identity: term_name.clone(),
            old_identity: None,
            path: None,
            dry_run: true,
            details,
        };
        return match ctx.format() {
            Format::Json => Ok(CommandOutcome::Success(verb_json(&result), warnings, None)),
            Format::Text => {
                let mut lines: Vec<String> = Vec::new();
                lines.push(format!("[dry-run] would create term: {}", term_name));
                for alias in &args.alias {
                    lines.push(format!("[dry-run] would attach alias: {} → {}", alias, term_name));
                }
                Ok(CommandOutcome::Success(serde_json::Value::String(lines.join("\n")), warnings, None))
            }
        };
    }

    let root = ctx.require_repo_path()?;
    let result = if let Some(project_name) = ctx.project() {
        let project_path = svc_util::resolve_project(root, Some(project_name), ctx.cwd())?;
        term_svc::new_term(&project_path, &term_name, new_input(&args), &args.misrecognition)?
    } else {
        term_svc::global::new_term(root, &term_name, new_input(&args), &args.misrecognition)?
    };

    let data = serde_json::json!({
        "term": result.term.term,
        "created": result.created,
        "added_aliases": result.added_aliases,
        "added_tags": result.added_tags,
        "added_misrecognitions": result.added_misrecognitions,
    });

    let text_output = if result.created {
        if result.added_aliases.is_empty() {
            format!("created term \"{}\"", result.term.term)
        } else {
            format!("created term \"{}\" with alias {}", result.term.term, result.added_aliases.join(", "))
        }
    } else if !result.added_aliases.is_empty() {
        format!("added alias {} to existing term \"{}\"", result.added_aliases.join(", "), result.term.term)
    } else {
        format!("term \"{}\" already up to date", result.term.term)
    };

    match ctx.format() {
        Format::Json => Ok(CommandOutcome::Success(data, warnings, None)),
        Format::Text => Ok(CommandOutcome::Success(serde_json::Value::String(text_output), warnings, None)),
    }
}

// ── Handle: mf term list (US2 / T021) ────────────────────────────────────────
