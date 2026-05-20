use std::path::PathBuf;

use clap::{Args, Subcommand, ValueEnum};
use serde::Serialize;

use crate::cli::deprecation::DeprecationContext;
use crate::cli::CommandOutcome;
use crate::error::{MfError, Result};
use crate::model::index::PublishStatus;
use crate::model::publish::{LocalRunOutcome, PublishRunOutcome, PublishUpdateOutcome, UpdateAction};
use crate::model::publisher::PublishersOutcome;
use crate::output::Format;
use crate::service::publish as publish_svc;
use crate::service::publisher as publisher_svc;

#[derive(Debug, Clone, Args)]
pub struct PublishCmd {
    #[command(subcommand)]
    pub command: Option<PublishSubcommand>,
}

#[derive(Debug, Clone, Subcommand)]
pub enum PublishSubcommand {
    #[command(about = "Publish an article to a target")]
    Run(PublishRunArgs),
    #[command(about = "Update publish record metadata")]
    Update(PublishUpdateArgs),
    #[command(about = "List and manage publish targets")]
    Target(PublishTargetCmd),
}

#[derive(Debug, Clone, Args)]
pub struct PublishTargetCmd {
    #[command(subcommand)]
    pub command: Option<PublishTargetSubcommand>,
}

#[derive(Debug, Clone, Subcommand)]
pub enum PublishTargetSubcommand {
    #[command(about = "List publishers and diagnostics")]
    List,
}

#[derive(Debug, Clone, Args, Serialize)]
pub struct PublishRunArgs {
    /// Article name (kebab-case, no extension, no path separators)
    pub article: String,
    #[arg(long)]
    pub target: Option<String>,
    #[arg(short = 'p', long)]
    pub project: Option<String>,
    #[arg(long = "dry-run")]
    pub dry_run: bool,
    #[arg(short = 'f', long)]
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
    #[arg(long = "set", value_name = "KEY=VALUE")]
    pub set: Vec<String>,
    #[arg(short = 'p', long)]
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

/// Handle `publish target list` (moved from removed top-level `publisher list`).
fn handle_target_list(repo_root: Option<&PathBuf>, format: Format) -> Result<CommandOutcome> {
    let root = repo_root.ok_or_else(MfError::not_in_mind_repo)?;

    let report = publisher_svc::discover(root)?;
    let outcome = PublishersOutcome::from_report(&report, root);

    match format {
        Format::Json => {
            let data = serde_json::to_value(&outcome)?;
            Ok(CommandOutcome::Success(data, None))
        }
        Format::Text => Ok(CommandOutcome::Raw(render_target_list_text(&outcome), None)),
    }
}

fn render_target_list_text(outcome: &PublishersOutcome) -> String {
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

pub fn dispatch(
    command: PublishCmd,
    repo_root: Option<&PathBuf>,
    format: Format,
    deprecation: &mut DeprecationContext,
) -> Result<CommandOutcome> {
    match command.command {
        None => Ok(CommandOutcome::GroupHelp("publish")),
        Some(PublishSubcommand::Run(args)) => {
            let root = repo_root.ok_or_else(MfError::not_in_mind_repo)?;
            let cwd = std::env::current_dir().map_err(MfError::Io)?;
            let outcome = publish_svc::run(&args, root, &cwd)?;
            render_run_outcome(outcome, format)
        }
        Some(PublishSubcommand::Update(args)) => {
            // Emit deprecation warnings for --status and --target-url
            if args.status.is_some() {
                deprecation.warn_subject("--status", "--set status=<value>");
            }
            if args.target_url.is_some() {
                deprecation.warn_subject("--target-url", "--set url=<value>");
            }

            // Merge --set values into the existing fields
            let merged_args = merge_set_values(args);

            let root = repo_root.ok_or_else(MfError::not_in_mind_repo)?;
            let cwd = std::env::current_dir().map_err(MfError::Io)?;
            let outcome = publish_svc::update(&merged_args, root, &cwd)?;
            render_update_outcome(outcome, format)
        }
        Some(PublishSubcommand::Target(target_cmd)) => match target_cmd.command {
            None => Ok(CommandOutcome::GroupHelp("publish target")),
            Some(PublishTargetSubcommand::List) => handle_target_list(repo_root, format),
        },
    }
}

/// Merge `--set key=val` pairs into status/target_url, with `--set` taking precedence.
fn merge_set_values(args: PublishUpdateArgs) -> PublishUpdateArgs {
    let mut merged = args;

    for kv in &merged.set {
        if let Some((key, val)) = kv.split_once('=') {
            match key {
                "status" => match val {
                    "draft" => merged.status = Some(PublishStatusArg::Draft),
                    "published" => merged.status = Some(PublishStatusArg::Published),
                    "archived" => merged.status = Some(PublishStatusArg::Archived),
                    _ => {
                        // Unknown status value is silently ignored
                    }
                },
                "url" => {
                    merged.target_url = Some(val.to_string());
                }
                _ => {
                    // Unknown keys are silently ignored at the CLI layer;
                    // the service layer can process them if needed.
                }
            }
        }
    }

    merged
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

fn render_local_text(o: &LocalRunOutcome) -> String {
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

fn render_update_outcome(outcome: PublishUpdateOutcome, format: Format) -> Result<CommandOutcome> {
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
                UpdateAction::Created => "created",
                UpdateAction::Updated => "updated",
            };
            lines.push(format!("action       {action}"));
            let status = match outcome.record.status {
                PublishStatus::Draft => "draft",
                PublishStatus::Published => "published",
                PublishStatus::Archived => "archived",
            };
            lines.push(format!("status       {status}"));
            lines.push(format!("target_url   {}", outcome.record.target_url.as_deref().unwrap_or("null")));
            lines.push(format!("published_at {}", outcome.record.published_at.as_deref().unwrap_or("null")));
            lines.push(format!("dry_run      {}", outcome.dry_run));
            Ok(CommandOutcome::Raw(lines.join("\n"), None))
        }
    }
}
