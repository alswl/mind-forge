use clap::{Args, Parser, Subcommand};
use serde_json::json;

use crate::cli::shared_flags::NoHeadersFlag;
use crate::cli::shared_flags::NoTruncFlag;
use crate::error::{MfError, Result};
use crate::model::render::{HtmlForm, RenderRequest, RenderScope};
use crate::output::list::{json_collection, render_text, ListCell, ListOpts, ListRow, ListView};
use crate::output::show::{
    json_envelope, render_text as render_show_text, ShowBlock, ShowField, ShowOpts, ShowSection, ShowValue,
};
use crate::output::Format;
use crate::service::config as config_svc;
use crate::service::render as render_svc;
use crate::service::util as svc_util;

use super::CommandOutcome;

#[derive(Debug, Parser)]
#[command(about = "Generate render prompts (emits prompts only, does not write output files)")]
pub struct RenderCmd {
    /// Article name to render
    pub article: Option<String>,
    /// Render template name
    #[arg(long)]
    pub template: Option<String>,
    /// HTML output form (document or fragment)
    #[arg(long, value_enum)]
    pub html_form: Option<HtmlForm>,
    #[command(subcommand)]
    pub command: Option<RenderSubcommand>,
}

#[derive(Debug, Subcommand)]
pub enum RenderSubcommand {
    /// Manage render templates
    Template(TemplateArgs),
}

#[derive(Debug, Args)]
pub struct TemplateArgs {
    #[command(subcommand)]
    pub command: TemplateSubcommand,
}

#[derive(Debug, Subcommand)]
pub enum TemplateSubcommand {
    /// List available render templates
    List(TemplateListArgs),
    /// Show template details
    Show(TemplateShowArgs),
}

#[derive(Debug, Clone, Args)]
pub struct TemplateListArgs {
    #[command(flatten)]
    pub no_headers: NoHeadersFlag,
    #[command(flatten)]
    pub no_trunc: NoTruncFlag,
}

#[derive(Debug, Clone, Args)]
pub struct TemplateShowArgs {
    pub name: String,
}

/// Dispatch the `mf render` command.
pub fn dispatch(
    args: RenderCmd,
    repo_root: Option<&std::path::PathBuf>,
    format: Format,
    project: Option<&str>,
) -> Result<CommandOutcome> {
    match args.command {
        Some(RenderSubcommand::Template(tpl_cmd)) => match tpl_cmd.command {
            TemplateSubcommand::List(tpl_args) => dispatch_list_templates(repo_root, format, tpl_args),
            TemplateSubcommand::Show(args) => handle_template_show(repo_root, format, args),
        },
        None => dispatch_render(args, repo_root, format, project),
    }
}

fn dispatch_render(
    args: RenderCmd,
    repo_root: Option<&std::path::PathBuf>,
    format: Format,
    project: Option<&str>,
) -> Result<CommandOutcome> {
    let root = repo_root.ok_or_else(MfError::not_in_mind_repo)?;

    let cwd = std::env::current_dir().map_err(MfError::Io)?;
    let (project_path, config) = render_svc::resolve_project_config(root, project, &cwd)?;
    let project_name = svc_util::dir_name(&project_path);

    let layout = config_svc::effective_layout(&project_path)?;
    let output_dir = &layout.build_output;
    let build_format = &config.build.format;
    let template_name = args.template.as_deref().unwrap_or(render_svc::default_template_name());

    let built_in_templates_list = render_svc::built_in_templates();
    let built_ins: Vec<_> = built_in_templates_list
        .iter()
        .map(|t| {
            let body =
                if t.name == "paper" { render_svc::built_in_paper_body() } else { render_svc::built_in_report_body() };
            (t, body)
        })
        .collect();

    let custom_templates = render_svc::discover_custom_templates(root)?;
    let custom_refs: Vec<_> = custom_templates.iter().map(|(t, b)| (t, b.as_str())).collect();

    // Determine scope: article scope when ARTICLE is provided, project scope otherwise.
    let (scope, article) = match args.article {
        Some(ref name) if !name.is_empty() => (RenderScope::Article, Some(name.clone())),
        Some(_) => {
            return Err(MfError::usage(
                "article name cannot be empty",
                Some("provide a non-empty article name, or omit it for project-scope render".to_string()),
            ));
        }
        None => (RenderScope::Project, None),
    };

    let request = RenderRequest {
        scope: scope.clone(),
        project: project_name.clone(),
        article: article.clone(),
        template: template_name.to_string(),
    };

    let generated = render_svc::generate_prompt(
        &request,
        &project_path,
        output_dir,
        build_format,
        &built_ins,
        &custom_refs,
        args.html_form,
    )?;

    let data = json!({
        "prompt": generated.prompt,
        "template": generated.template,
        "template_source": generated.template_source,
        "scope": generated.scope,
        "project": generated.project,
        "article": generated.article,
        "outputs": generated.outputs,
    });

    match format {
        Format::Json => Ok(CommandOutcome::Success(data, None)),
        Format::Text => Ok(CommandOutcome::Raw(generated.prompt, None)),
    }
}

fn handle_template_show(
    repo_root: Option<&std::path::PathBuf>,
    format: Format,
    args: TemplateShowArgs,
) -> Result<CommandOutcome> {
    let root = repo_root.ok_or_else(MfError::not_in_mind_repo)?;
    let built_ins = render_svc::built_in_templates();
    let custom = render_svc::discover_custom_templates(root)?;

    // Find template and its body
    let (template, body): (crate::model::render::RenderTemplate, Option<String>) =
        if let Some(t) = built_ins.iter().find(|t| t.name.eq_ignore_ascii_case(&args.name)) {
            let body = match t.name.as_str() {
                "report" => Some(render_svc::built_in_report_body().to_string()),
                "paper" => Some(render_svc::built_in_paper_body().to_string()),
                _ => None,
            };
            (t.clone(), body)
        } else if let Some((t, b)) = custom.iter().find(|(t, _)| t.name.eq_ignore_ascii_case(&args.name)) {
            (t.clone(), Some(b.clone()))
        } else {
            return Err(MfError::usage(
                format!("template '{}' not found", args.name),
                Some("use `mf render template list` to see available templates".to_string()),
            ));
        };

    let source_str = match template.source {
        crate::model::render::TemplateSource::BuiltIn => "built_in",
        crate::model::render::TemplateSource::Custom => "custom",
    };

    let fields = vec![
        ShowField { label: "Name", value: ShowValue::Text(template.name.clone()) },
        ShowField { label: "Label", value: ShowValue::Text(template.label.clone()) },
        ShowField { label: "Description", value: ShowValue::Text(template.description.clone()) },
        ShowField { label: "Source", value: ShowValue::Text(source_str.to_string()) },
        ShowField { label: "Path", value: ShowValue::Optional(template.path.clone()) },
        ShowField { label: "Default", value: ShowValue::Text(template.default.to_string()) },
    ];

    let mut sections = Vec::new();
    if let Some(body_text) = body {
        let preview: String = body_text.lines().take(10).collect::<Vec<_>>().join("\n");
        sections.push(ShowSection {
            heading: "Preview",
            fields: vec![ShowField { label: "Body", value: ShowValue::Multiline(preview) }],
        });
    }

    let block = ShowBlock { kind: "render_template", identity: template.name.clone(), fields, sections };

    match format {
        Format::Json => {
            let tpl_json = serde_json::to_value(&template).map_err(MfError::Json)?;
            let extra = tpl_json.as_object().cloned().unwrap_or_default();
            Ok(CommandOutcome::Success(json_envelope(&block, extra), None))
        }
        Format::Text => Ok(CommandOutcome::Raw(
            render_show_text(&block, &ShowOpts::from_repo_root(repo_root.map(|r| r.as_path()))),
            None,
        )),
    }
}

fn dispatch_list_templates(
    repo_root: Option<&std::path::PathBuf>,
    format: Format,
    args: TemplateListArgs,
) -> Result<CommandOutcome> {
    let root = repo_root.ok_or_else(MfError::not_in_mind_repo)?;

    let built_ins = render_svc::built_in_templates();
    let custom = render_svc::discover_custom_templates(root)?;

    let opts =
        ListOpts::from_flags(args.no_headers.no_headers, args.no_trunc.no_trunc).with_repo_root(repo_root.cloned());

    match format {
        Format::Json => {
            let mut items: Vec<serde_json::Value> = Vec::new();
            for t in &built_ins {
                items.push(json!({
                    "identity": t.name,
                    "name": t.name,
                    "label": t.label,
                    "description": t.description,
                    "source": "built_in",
                    "path": serde_json::Value::Null,
                    "default": t.default,
                }));
            }
            for (t, _) in &custom {
                items.push(json!({
                    "identity": t.name,
                    "name": t.name,
                    "label": t.label,
                    "description": t.description,
                    "source": "custom",
                    "path": t.path,
                    "default": t.default,
                }));
            }
            Ok(CommandOutcome::Success(json_collection("templates", items), None))
        }
        Format::Text => {
            let mut rows = Vec::new();
            for t in &built_ins {
                rows.push(ListRow {
                    cells: vec![
                        ListCell::Text(t.name.clone()),
                        ListCell::Text(t.label.clone()),
                        ListCell::Text("built_in".to_string()),
                        ListCell::Text(t.default.to_string()),
                    ],
                });
            }
            for (t, _) in &custom {
                rows.push(ListRow {
                    cells: vec![
                        ListCell::Text(t.name.clone()),
                        ListCell::Text(t.label.clone()),
                        ListCell::Text("custom".to_string()),
                        ListCell::Text(t.default.to_string()),
                    ],
                });
            }
            let view = ListView { headers: &["NAME", "LABEL", "SOURCE", "DEFAULT"], rows, plural_noun: "templates" };
            Ok(CommandOutcome::Raw(render_text(&view, &opts), None))
        }
    }
}
