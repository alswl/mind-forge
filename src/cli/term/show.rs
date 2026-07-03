use super::*;

fn render_term_show(
    term: &crate::model::term::Term,
    repo_root: Option<&std::path::Path>,
    format: Format,
) -> Result<CommandOutcome> {
    render_term_show_inner(term, repo_root, format, None)
}

fn render_term_show_with_scope(
    term: &crate::model::term::Term,
    repo_root: Option<&std::path::Path>,
    scope: &str,
    format: Format,
) -> Result<CommandOutcome> {
    render_term_show_inner(term, repo_root, format, Some(scope))
}

fn render_term_show_inner(
    term: &crate::model::term::Term,
    repo_root: Option<&std::path::Path>,
    format: Format,
    scope: Option<&str>,
) -> Result<CommandOutcome> {
    let corr_count = term.corrections.len();
    let mut fields = vec![
        ShowField { label: "Term", value: ShowValue::Text(term.term.clone()) },
        ShowField { label: "Definition", value: ShowValue::Optional(term.definition.clone()) },
        ShowField { label: "Description", value: ShowValue::Optional(term.description.clone()) },
        ShowField {
            label: "Aliases",
            value: if term.aliases.is_empty() {
                ShowValue::Text("-".to_string())
            } else {
                ShowValue::Text(term.aliases.join(", "))
            },
        },
        ShowField {
            label: "Tags",
            value: if term.tags.is_empty() {
                ShowValue::Text("-".to_string())
            } else {
                ShowValue::Text(term.tags.join(", "))
            },
        },
        ShowField { label: "Confidence", value: ShowValue::Optional(term.confidence.map(|c| format!("{c:.2}"))) },
        ShowField {
            label: "Corrections",
            value: if corr_count == 0 {
                ShowValue::Text("-".to_string())
            } else {
                ShowValue::Text(corr_count.to_string())
            },
        },
    ];
    if let Some(s) = scope {
        fields.push(ShowField { label: "Scope", value: ShowValue::Text(s.to_string()) });
    }

    let mut sections = Vec::new();
    if !term.corrections.is_empty() {
        let corr_fields: Vec<ShowField> = term
            .corrections
            .iter()
            .map(|c| ShowField {
                label: "Correction",
                value: ShowValue::Text(format!("\"{}\" → \"{}\"", c.original, c.correct)),
            })
            .collect();
        sections.push(ShowSection { heading: "Corrections", fields: corr_fields });
    }

    let block = ShowBlock { kind: "term", identity: term.term.clone(), fields, sections };

    match format {
        Format::Json => {
            let mut extra = serde_json::to_value(term).map_err(MfError::Json)?.as_object().cloned().unwrap_or_default();
            if let Some(s) = scope {
                extra.insert("scope".to_string(), serde_json::json!(s));
            }
            Ok(CommandOutcome::Success(json_envelope(&block, extra), Vec::new(), None))
        }
        Format::Text => Ok(CommandOutcome::Raw(render_show_text(&block, &ShowOpts::from_repo_root(repo_root)), None)),
    }
}

pub(super) fn handle_show(args: TermShowArgs, ctx: &CommandCtx) -> Result<CommandOutcome> {
    let root = ctx.require_repo_path()?;
    let format = ctx.format();
    if let Some(project_name) = ctx.project() {
        let project_path = svc_util::resolve_project(root, Some(project_name), ctx.cwd())?;
        match term_svc::show_term(&project_path, &args.term) {
            Ok(term) => render_term_show_with_scope(&term, Some(root), "project", format),
            Err(MfError::NotFound { .. }) => {
                // Fall through to global
                let term = term_svc::global::show_term(root, &args.term)?;
                render_term_show_with_scope(&term, Some(root), "global", format)
            }
            Err(e) => Err(e),
        }
    } else {
        let term = term_svc::global::show_term(root, &args.term)?;
        render_term_show(&term, Some(root), format)
    }
}
