use std::path::PathBuf;

use clap::{Args, Subcommand, ValueEnum};
use serde::Serialize;

use crate::cli::CommandOutcome;
use crate::error::{MfError, Result};
use crate::model::index::PublishStatus;
use crate::model::publish::PublishRunOutcome;
use crate::output::Format;
use crate::service::publish as publish_svc;

#[derive(Debug, Clone, Args)]
pub struct PublishCmd {
    #[command(subcommand)]
    pub command: Option<PublishSubcommand>,
}

#[derive(Debug, Clone, Subcommand)]
pub enum PublishSubcommand {
    #[command(about = "Publish article to a target (supported: local, yuque-prompt)")]
    Run(PublishRunArgs),
    #[command(about = "Update a publish_records entry in mind-index.yaml")]
    Update(PublishUpdateArgs),
}

#[derive(Debug, Clone, Args, Serialize)]
pub struct PublishRunArgs {
    /// Article name (kebab-case, no extension, no path separators)
    pub article: String,
    #[arg(long)]
    pub target: Option<String>,
    #[arg(long)]
    pub project: Option<String>,
    #[arg(long = "dry-run")]
    pub dry_run: bool,
    #[arg(long)]
    pub force: bool,
}

#[derive(Debug, Clone, Args, Serialize)]
pub struct PublishUpdateArgs {
    /// Article name (kebab-case, no extension, no path separators)
    pub article: String,
    #[arg(long, required = true)]
    pub target: String,
    #[arg(long, value_enum)]
    pub status: Option<PublishStatusArg>,
    #[arg(long = "target-url")]
    pub target_url: Option<String>,
    #[arg(long)]
    pub project: Option<String>,
    #[arg(long = "dry-run")]
    pub dry_run: bool,
}

/// CLI-facing mirror of `model::index::PublishStatus` exposing `clap::ValueEnum`.
#[derive(Debug, Clone, Copy, ValueEnum, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PublishStatusArg {
    Draft,
    Published,
    Archived,
}

impl From<PublishStatusArg> for PublishStatus {
    fn from(s: PublishStatusArg) -> Self {
        match s {
            PublishStatusArg::Draft => PublishStatus::Draft,
            PublishStatusArg::Published => PublishStatus::Published,
            PublishStatusArg::Archived => PublishStatus::Archived,
        }
    }
}

pub fn dispatch(
    command: PublishCmd,
    repo_root: Option<&PathBuf>,
    format: Format,
) -> Result<CommandOutcome> {
    match command.command {
        None => Ok(CommandOutcome::GroupHelp(crate::cli::HelpTarget::Publish)),
        Some(PublishSubcommand::Run(args)) => {
            let root = repo_root.ok_or_else(MfError::not_in_mind_repo)?;
            let cwd = std::env::current_dir().map_err(MfError::Io)?;
            let outcome = publish_svc::run(&args, root, &cwd)?;
            render_run_outcome(outcome, format)
        }
        Some(PublishSubcommand::Update(args)) => {
            let root = repo_root.ok_or_else(MfError::not_in_mind_repo)?;
            let cwd = std::env::current_dir().map_err(MfError::Io)?;
            let outcome = publish_svc::update(&args, root, &cwd)?;
            render_update_outcome(outcome, format)
        }
    }
}

fn render_run_outcome(outcome: PublishRunOutcome, format: Format) -> Result<CommandOutcome> {
    match format {
        Format::Json => {
            let data = serde_json::to_value(&outcome)?;
            Ok(CommandOutcome::Success(data, None))
        }
        Format::Text => {
            let text = match &outcome {
                PublishRunOutcome::Local(o) => render_local_text(o),
                PublishRunOutcome::YuquePrompt(o) => publish_svc::render_prompt_text(o),
            };
            Ok(CommandOutcome::Raw(text, None))
        }
    }
}

fn render_local_text(o: &crate::model::publish::LocalRunOutcome) -> String {
    let mut lines = Vec::new();
    lines.push(format!("target      {}", o.target_name));
    lines.push("type        local".to_string());
    lines.push(format!("article     {}", o.article));
    lines.push(format!("source      {}", o.source));
    lines.push(format!("destination {}", o.destination));
    lines.push(format!("size        {} bytes", o.size_bytes));
    lines.push(format!("dry_run     {}", o.dry_run));
    lines.join("\n")
}

fn render_update_outcome(
    outcome: crate::model::publish::PublishUpdateOutcome,
    format: Format,
) -> Result<CommandOutcome> {
    match format {
        Format::Json => {
            let data = serde_json::to_value(&outcome)?;
            Ok(CommandOutcome::Success(data, None))
        }
        Format::Text => {
            let mut lines = Vec::new();
            lines.push(format!("article      {}", outcome.article));
            lines.push(format!("target       {}", outcome.target_name));
            let action = match outcome.action {
                crate::model::publish::UpdateAction::Created => "created",
                crate::model::publish::UpdateAction::Updated => "updated",
            };
            lines.push(format!("action       {action}"));
            let status = match outcome.record.status {
                PublishStatus::Draft => "draft",
                PublishStatus::Published => "published",
                PublishStatus::Archived => "archived",
            };
            lines.push(format!("status       {status}"));
            lines.push(format!(
                "target_url   {}",
                outcome.record.target_url.as_deref().unwrap_or("null")
            ));
            lines.push(format!(
                "published_at {}",
                outcome.record.published_at.as_deref().unwrap_or("null")
            ));
            lines.push(format!("dry_run      {}", outcome.dry_run));
            Ok(CommandOutcome::Raw(lines.join("\n"), None))
        }
    }
}
