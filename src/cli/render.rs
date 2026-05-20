use clap::{Args, Parser, Subcommand};
use serde_json::json;

use crate::error::{MfError, Result};
use crate::model::render::{HtmlForm, RenderRequest, RenderScope};
use crate::output::Format;
use crate::service::config as config_svc;
use crate::service::render as render_svc;
use crate::service::util as svc_util;

use super::CommandOutcome;

#[derive(Debug, Parser)]
pub struct RenderCmd {
    /// Article name to render
    pub article: Option<String>,
    /// Select project context
    #[arg(short = 'p', long)]
    pub project: Option<String>,
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
}

#[derive(Debug, Clone, Args)]
pub struct TemplateListArgs;

/// Dispatch the `mf render` command.
pub fn dispatch(args: RenderCmd, repo_root: Option<&std::path::PathBuf>, format: Format) -> Result<CommandOutcome> {
    match args.command {
        Some(RenderSubcommand::Template(tpl_cmd)) => match tpl_cmd.command {
            TemplateSubcommand::List(_) => dispatch_list_templates(repo_root, format),
        },
        None => dispatch_render(args, repo_root, format),
    }
}

fn dispatch_render(args: RenderCmd, repo_root: Option<&std::path::PathBuf>, format: Format) -> Result<CommandOutcome> {
    let root = repo_root.ok_or_else(MfError::not_in_mind_repo)?;

    let cwd = std::env::current_dir().map_err(MfError::Io)?;
    let (project_path, config) = render_svc::resolve_project_config(root, args.project.as_deref(), &cwd)?;
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

fn dispatch_list_templates(repo_root: Option<&std::path::PathBuf>, format: Format) -> Result<CommandOutcome> {
    let root = repo_root.ok_or_else(MfError::not_in_mind_repo)?;

    let built_ins = render_svc::built_in_templates();
    let custom = render_svc::discover_custom_templates(root)?;

    match format {
        Format::Json => {
            let mut templates_json: Vec<serde_json::Value> = Vec::new();
            for t in &built_ins {
                templates_json.push(json!({
                    "name": t.name,
                    "label": t.label,
                    "description": t.description,
                    "source": t.source,
                    "path": None::<String>,
                    "default": t.default,
                }));
            }
            for (t, _) in &custom {
                templates_json.push(json!({
                    "name": t.name,
                    "label": t.label,
                    "description": t.description,
                    "source": t.source,
                    "path": t.path,
                    "default": t.default,
                }));
            }
            let data = json!({ "templates": templates_json });
            Ok(CommandOutcome::Success(data, None))
        }
        Format::Text => {
            let mut lines = vec!["Available render templates:".to_string()];
            for t in &built_ins {
                lines.push(format!("  {}  {} (built_in)", t.name, t.description));
            }
            for (t, _) in &custom {
                let source_info = format!("custom: {}", t.path.as_deref().unwrap_or(""));
                lines.push(format!("  {}  {} ({})", t.name, t.description, source_info));
            }
            Ok(CommandOutcome::Raw(lines.join("\n"), None))
        }
    }
}
