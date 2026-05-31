use std::path::PathBuf;

use clap::{Args, Subcommand, ValueEnum};
use serde::Serialize;

use crate::cli::deprecation::DeprecationContext;
use crate::cli::shared_flags::NoHeadersFlag;
use crate::cli::shared_flags::NoTruncFlag;
use crate::cli::CommandOutcome;
use crate::error::{MfError, Result};
use crate::model::index::PublishStatus;
use crate::model::publish::{LocalRunOutcome, PublishRunOutcome, PublishUpdateOutcome, UpdateAction};
use crate::model::publisher::PublishersOutcome;
use crate::model::Resource;
use crate::output::list::{render_text, ListCell, ListOpts, ListRow, ListView};
use crate::output::show::{json_envelope, render_text as render_show_text, ShowBlock, ShowField, ShowOpts, ShowValue};
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
    List(PublishTargetListArgs),
    #[command(about = "Show publish target details")]
    Show(PublishTargetShowArgs),
}

#[derive(Debug, Clone, Args, Serialize)]
pub struct PublishTargetListArgs {
    #[command(flatten)]
    pub no_headers: NoHeadersFlag,
    #[command(flatten)]
    pub no_trunc: NoTruncFlag,
}

#[derive(Debug, Clone, Args, Serialize)]
pub struct PublishTargetShowArgs {
    pub name: String,
}

#[derive(Debug, Clone, Args, Serialize)]
pub struct PublishRunArgs {
    /// Article name (kebab-case, no extension, no path separators)
    pub article: String,
    #[arg(long)]
    pub target: Option<String>,
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
fn handle_target_list(
    repo_root: Option<&PathBuf>,
    format: Format,
    args: &PublishTargetListArgs,
) -> Result<CommandOutcome> {
    let root = repo_root.ok_or_else(MfError::not_in_mind_repo)?;

    let report = publisher_svc::discover(root)?;
    let outcome = PublishersOutcome::from_report(&report, root);

    let opts =
        ListOpts::from_flags(args.no_headers.no_headers, args.no_trunc.no_trunc).with_repo_root(repo_root.cloned());

    match format {
        Format::Json => {
            let items: Vec<serde_json::Value> = outcome
                .publishers
                .iter()
                .map(|p| {
                    let mut v = serde_json::to_value(p).map_err(MfError::Json)?;
                    if let Some(obj) = v.as_object_mut() {
                        obj.insert("identity".to_string(), serde_json::Value::String(p.identity()));
                    }
                    Ok(v)
                })
                .collect::<Result<Vec<_>>>()?;
            let diagnostics: Vec<serde_json::Value> = outcome
                .diagnostics
                .iter()
                .map(|d| serde_json::to_value(d).map_err(MfError::Json))
                .collect::<Result<Vec<_>>>()?;
            let data = serde_json::json!({
                "publish_targets": items,
                "diagnostics": diagnostics,
            });
            Ok(CommandOutcome::Success(data, None))
        }
        Format::Text => {
            // Emit diagnostics to stderr before the table
            let mut diag_text = String::new();
            if !outcome.diagnostics.is_empty() {
                for d in &outcome.diagnostics {
                    let path_str = d.path.as_deref().unwrap_or("-");
                    let name_str = d.publisher_name.as_deref().unwrap_or("-");
                    diag_text.push_str(&format!(
                        "[{kind}] {path} ({name}): {msg}\n",
                        kind = d.kind,
                        path = path_str,
                        name = name_str,
                        msg = d.message
                    ));
                }
            }

            let mut rows = Vec::with_capacity(outcome.publishers.len());
            for p in &outcome.publishers {
                rows.push(ListRow {
                    cells: vec![
                        ListCell::Text(p.name.clone()),
                        ListCell::Text(p.target_type.clone()),
                        ListCell::Text(p.status.clone()),
                    ],
                });
            }
            let view = ListView { headers: &["NAME", "TYPE", "STATUS"], rows, plural_noun: "publish targets" };
            let table = render_text(&view, &opts);
            // Prepend diagnostics text (stderr) to the combined output
            // The caller will print this to stdout; diagnostics should ideally go to stderr
            // but for now we embed them since CommandOutcome only has a single text output.
            Ok(CommandOutcome::Raw(if diag_text.is_empty() { table } else { format!("{diag_text}{table}") }, None))
        }
    }
}

pub fn dispatch(
    command: PublishCmd,
    repo_root: Option<&PathBuf>,
    format: Format,
    project: Option<&str>,
    deprecation: &mut DeprecationContext,
) -> Result<CommandOutcome> {
    match command.command {
        None => Ok(CommandOutcome::GroupHelp("publish")),
        Some(PublishSubcommand::Run(args)) => {
            let root = repo_root.ok_or_else(MfError::not_in_mind_repo)?;
            let cwd = std::env::current_dir().map_err(MfError::Io)?;
            let outcome = publish_svc::run(&args, root, &cwd, project)?;
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
            let outcome = publish_svc::update(&merged_args, root, &cwd, project)?;
            render_update_outcome(outcome, format)
        }
        Some(PublishSubcommand::Target(target_cmd)) => match target_cmd.command {
            None => Ok(CommandOutcome::GroupHelp("publish target")),
            Some(PublishTargetSubcommand::List(args)) => handle_target_list(repo_root, format, &args),
            Some(PublishTargetSubcommand::Show(args)) => handle_target_show(repo_root, format, &args),
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

fn handle_target_show(
    repo_root: Option<&PathBuf>,
    format: Format,
    args: &PublishTargetShowArgs,
) -> Result<CommandOutcome> {
    let root = repo_root.ok_or_else(MfError::not_in_mind_repo)?;
    let report = publisher_svc::discover(root)?;
    let outcome = PublishersOutcome::from_report(&report, root);

    let publisher = outcome.publishers.iter().find(|p| p.name.eq_ignore_ascii_case(&args.name)).ok_or_else(|| {
        MfError::usage(
            format!("publish target '{}' not found", args.name),
            Some("use `mf publish target list` to see available targets".to_string()),
        )
    })?;

    let required =
        if publisher.required_inputs.is_empty() { "-".to_string() } else { publisher.required_inputs.join(", ") };

    let block = ShowBlock {
        kind: "publish_target",
        identity: publisher.name.clone(),
        fields: vec![
            ShowField { label: "Name", value: ShowValue::Text(publisher.name.clone()) },
            ShowField { label: "Type", value: ShowValue::Text(publisher.target_type.clone()) },
            ShowField { label: "Status", value: ShowValue::Text(publisher.status.clone()) },
            ShowField { label: "Label", value: ShowValue::Optional(publisher.label.clone()) },
            ShowField { label: "Description", value: ShowValue::Optional(publisher.description.clone()) },
            ShowField { label: "Source", value: ShowValue::Path(publisher.source_path.clone()) },
            ShowField { label: "Required inputs", value: ShowValue::Text(required) },
        ],
        sections: vec![],
    };

    match format {
        Format::Json => {
            let pub_json = serde_json::to_value(publisher).map_err(MfError::Json)?;
            let extra = pub_json.as_object().cloned().unwrap_or_default();
            Ok(CommandOutcome::Success(json_envelope(&block, extra), None))
        }
        Format::Text => Ok(CommandOutcome::Raw(
            render_show_text(&block, &ShowOpts::from_repo_root(repo_root.map(|r| r.as_path()))),
            None,
        )),
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
