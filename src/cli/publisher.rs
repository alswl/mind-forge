use std::path::PathBuf;

use clap::{Args, Subcommand};
use serde::Serialize;

use crate::cli::deprecation::DeprecationContext;
use crate::cli::CommandOutcome;
use crate::error::Result;
use crate::output::Format;

#[derive(Debug, Clone, Args)]
pub struct PublisherCmd {
    #[command(subcommand)]
    pub command: Option<PublisherSubcommand>,
}

#[derive(Debug, Clone, Subcommand)]
pub enum PublisherSubcommand {
    #[command(about = "List publishers and diagnostics")]
    List(PublisherListArgs),
}

#[derive(Debug, Clone, Args, Serialize)]
pub struct PublisherListArgs {}

pub fn dispatch(
    command: PublisherCmd,
    repo_root: Option<&PathBuf>,
    format: Format,
    _deprecation: &mut DeprecationContext,
) -> Result<CommandOutcome> {
    match command.command {
        None => Ok(CommandOutcome::GroupHelp("publisher")),
        Some(PublisherSubcommand::List(_args)) => handle_list(repo_root, format),
    }
}

fn handle_list(repo_root: Option<&PathBuf>, format: Format) -> Result<CommandOutcome> {
    let root = repo_root.ok_or_else(crate::error::MfError::not_in_mind_repo)?;

    let report = crate::service::publisher::discover(root)?;
    let outcome = crate::model::publisher::PublishersOutcome::from_report(&report, root);

    match format {
        Format::Json => {
            let data = serde_json::to_value(&outcome)?;
            Ok(CommandOutcome::Success(data, None))
        }
        Format::Text => Ok(CommandOutcome::Raw(render_text(&outcome), None)),
    }
}

fn render_text(outcome: &crate::model::publisher::PublishersOutcome) -> String {
    let mut lines = Vec::new();

    if outcome.publishers.is_empty() && outcome.diagnostics.is_empty() {
        return "No publishers found.".to_string();
    }

    if !outcome.publishers.is_empty() {
        lines.push(format!("{:<24} {:<12} STATUS", "NAME", "TYPE"));
        for p in &outcome.publishers {
            let status = p.status.as_str();
            let label = p.label.as_deref().unwrap_or("-");
            lines.push(format!("{:<24} {:<12} {}", p.name, p.target_type, status));
            if label != "-" {
                lines.push(format!("  label:       {label}"));
            }
            if let Some(ref desc) = p.description {
                lines.push(format!("  description: {desc}"));
            }
            lines.push(format!("  source:      {}", p.source_path));
            lines.push(format!("  inputs:      [{}]", p.required_inputs.join(", ")));
        }
    }

    if !outcome.diagnostics.is_empty() {
        if !outcome.publishers.is_empty() {
            lines.push(String::new());
        }
        lines.push("Diagnostics:".to_string());
        for d in &outcome.diagnostics {
            let path_str = d.path.as_deref().unwrap_or("-");
            let name_str = d.publisher_name.as_deref().unwrap_or("-");
            let hint_str = d.hint.as_deref().unwrap_or("");
            lines.push(format!("  [{kind}] {path} ({name})", kind = d.kind, path = path_str, name = name_str));
            lines.push(format!("    {msg}", msg = d.message));
            if !hint_str.is_empty() {
                lines.push(format!("    hint: {hint_str}"));
            }
        }
    }

    lines.join("\n")
}
