use super::*;

pub(super) fn handle_article_show(
    args: ArticleShowArgs,
    project_path: &Path,
    format: Format,
) -> Result<CommandOutcome> {
    identity::validate_entity_path(project_path, &args.path)?;
    let config = config_svc::load_project(project_path, None)?;
    let articles = article_svc::list_articles(project_path)?;

    // Prefer exact article_path match (path selector), then fall back to
    // title/stem/contains for legacy title compatibility.
    let resolved = articles.iter().find(|a| a.article_path == args.path).or_else(|| {
        articles.iter().find(|a| {
            let stem = crate::service::index::article_output_stem(&a.article_path);
            a.title.eq_ignore_ascii_case(&args.path)
                || stem.eq_ignore_ascii_case(&args.path)
                || a.article_path.contains(&args.path)
        })
    });

    match resolved {
        None => Err(MfError::usage(
            format!("article '{}' not found", args.path),
            Some("use `mf article list` to see available articles".to_string()),
        )),
        Some(article) => {
            let article_dir = config.as_ref().map(|cfg| article_svc::effective_article_dir(project_path, cfg, article));
            let content_kind = article_content_kind(project_path, &article.article_path);
            let status_str = match article.status {
                ArticleStatus::Draft => "draft",
                ArticleStatus::Published => "published",
            };

            let mut fields = vec![
                ShowField { label: "Path", value: ShowValue::Path(article.article_path.clone()) },
                ShowField { label: "Title", value: ShowValue::Text(article.title.clone()) },
                ShowField { label: "Status", value: ShowValue::Text(status_str.to_string()) },
                ShowField { label: "Content", value: ShowValue::Text(content_kind.to_string()) },
            ];
            if let Some(ref dir) = article_dir {
                fields.push(ShowField { label: "Article dir", value: ShowValue::Path(dir.clone()) });
            }
            if let Some(ref origin) = article.template_origin {
                fields.push(ShowField {
                    label: "Template",
                    value: ShowValue::Text(format!("{} ({})", origin.template_name, origin.slot_value)),
                });
            }
            fields.push(ShowField { label: "Created", value: ShowValue::Text(article.created_at.clone()) });
            fields.push(ShowField { label: "Updated", value: ShowValue::Text(article.updated_at.clone()) });

            let idx = crate::service::index::load(project_path)?;
            let prompt_view = article_svc::prompt_view_for_article(&idx, &article.article_path);
            let thinking_view = article_svc::thinking_view_for_article(&idx, &article.article_path);

            match &prompt_view {
                Some(p) => {
                    fields.push(ShowField { label: "Prompt", value: ShowValue::Path(p.path.clone()) });
                    fields.push(ShowField {
                        label: "Prompt mode",
                        value: ShowValue::Optional(p.mode.map(|m| m.to_string())),
                    });
                    fields.push(ShowField {
                        label: "Prompt status",
                        value: ShowValue::Text(p.binding_status.as_str().to_string()),
                    });
                    if !p.conflicts.is_empty() {
                        fields.push(ShowField {
                            label: "Prompt conflicts",
                            value: ShowValue::Text(p.conflicts.join(", ")),
                        });
                    }
                }
                None => fields.push(ShowField { label: "Prompt", value: ShowValue::Optional(None) }),
            }
            match &thinking_view {
                Some(t) => fields.push(ShowField { label: "Thinking", value: ShowValue::Path(t.path.clone()) }),
                None => fields.push(ShowField { label: "Thinking", value: ShowValue::Optional(None) }),
            }

            let block = ShowBlock { kind: "article", identity: article.article_path.clone(), fields, sections: vec![] };

            match format {
                Format::Json => {
                    let article_json = serde_json::to_value(article).map_err(MfError::Json)?;
                    let mut extra = article_json.as_object().cloned().unwrap_or_default();
                    extra.insert("prompt".to_string(), serde_json::to_value(&prompt_view).map_err(MfError::Json)?);
                    extra.insert("thinking".to_string(), serde_json::to_value(&thinking_view).map_err(MfError::Json)?);
                    Ok(CommandOutcome::Success(json_envelope(&block, extra), Vec::new(), None))
                }
                Format::Text => Ok(CommandOutcome::Raw(
                    render_show_text(&block, &ShowOpts::from_repo_root(Some(project_path))),
                    None,
                )),
            }
        }
    }
}
