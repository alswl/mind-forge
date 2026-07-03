use super::*;

/// ── Handle: mf article block rename ────────────────────────────────────
pub(super) fn handle_block_rename(args: ArticleBlockRenameArgs, ctx: &mut CommandCtx) -> Result<CommandOutcome> {
    let root = ctx.require_repo_path()?;
    let format = ctx.format();
    let project_path = svc_util::resolve_project(root, ctx.project(), ctx.cwd())?;

    // Resolve article path (supports title lookup via index)
    let article_path = {
        let index = crate::service::index::load(&project_path)?;
        let articles = index.articles.as_ref().ok_or_else(|| {
            MfError::not_found(
                "no articles in index".to_string(),
                Some("use `mf article list` to see available articles".to_string()),
            )
        })?;
        let article =
            articles.iter().find(|a| a.title == args.article || a.article_path == args.article).ok_or_else(|| {
                MfError::not_found(
                    format!("article '{}' not found", args.article),
                    Some("use `mf article list --project <project>` to see available articles".to_string()),
                )
            })?;
        article.article_path.clone()
    };

    identity::validate_entity_path(&project_path, &article_path)?;

    if args.dry_run.dry_run {
        let result = VerbResult {
            verb: Verb::Rename,
            kind: "block",
            identity: format!("{}/{}", article_path, args.new_slug),
            old_identity: Some(format!("{}/{}", article_path, args.old_block)),
            path: None,
            dry_run: true,
            details: serde_json::json!({}),
        };
        return match format {
            Format::Json => Ok(CommandOutcome::Success(verb_json(&result), Vec::new(), None)),
            Format::Text => Ok(CommandOutcome::Success(
                serde_json::Value::String(verb_text(&result, &VerbOpts::from_repo_root(Some(project_path.as_path())))),
                Vec::new(),
                None,
            )),
        };
    }

    let report =
        article_svc::rename_block(&project_path, &article_path, &args.old_block, &args.new_slug, args.force.force)?;

    let new_full_path = format!("{}/{}", article_path, report.new_filename);
    let old_full_path = format!("{}/{}", article_path, report.old_filename);
    let result = VerbResult {
        verb: Verb::Rename,
        kind: "block",
        identity: new_full_path.clone(),
        old_identity: Some(old_full_path.clone()),
        path: Some(new_full_path),
        dry_run: false,
        details: serde_json::json!({
            "old_filename": report.old_filename,
            "new_filename": report.new_filename,
            "article_path": report.article_path,
        }),
    };
    match format {
        Format::Json => Ok(CommandOutcome::Success(verb_json(&result), Vec::new(), None)),
        Format::Text => Ok(CommandOutcome::Success(
            serde_json::Value::String(verb_text(&result, &VerbOpts::from_repo_root(Some(project_path.as_path())))),
            Vec::new(),
            None,
        )),
    }
}
