use std::path::PathBuf;

use clap::{Args, Subcommand};
use serde::Serialize;

use crate::cli::{CommandOutcome, HelpTarget};
use crate::error::Result;
use crate::output::Format;
use crate::service;

#[derive(Debug, Clone, Args)]
pub struct ConfigCmd {
    #[command(subcommand)]
    pub command: Option<ConfigSubcommand>,
}

#[derive(Debug, Clone, Subcommand)]
pub enum ConfigSubcommand {
    #[command(about = "Show config schema")]
    Schema(ConfigSchemaArgs),
    #[command(about = "Show effective config")]
    Show(ConfigShowArgs),
    #[command(about = "Initialize config file")]
    Init(ConfigInitArgs),
}

#[derive(Debug, Clone, Args, Serialize)]
pub struct ConfigSchemaArgs {
    #[arg(long = "output-format", default_value = "json")]
    pub output_format: String,
}

#[derive(Debug, Clone, Args, Serialize)]
pub struct ConfigShowArgs {
    #[arg(long = "output-format", default_value = "yaml")]
    pub output_format: String,
}

#[derive(Debug, Clone, Args, Serialize)]
pub struct ConfigInitArgs {
    #[arg(long)]
    pub output: Option<PathBuf>,
    #[arg(long, default_value = "project")]
    pub target: String,
    #[arg(long)]
    pub force: bool,
}

pub fn dispatch(
    command: ConfigCmd,
    repo_root: Option<&PathBuf>,
    _format: Format,
) -> Result<CommandOutcome> {
    match command.command {
        None => Ok(CommandOutcome::GroupHelp(HelpTarget::Config)),
        Some(ConfigSubcommand::Schema(args)) => handle_schema(args),
        Some(ConfigSubcommand::Show(args)) => handle_show(args, repo_root),
        Some(ConfigSubcommand::Init(args)) => handle_init(args),
    }
}

fn handle_schema(args: ConfigSchemaArgs) -> Result<CommandOutcome> {
    let output = service::config::schema_output(&args.output_format)?;
    Ok(CommandOutcome::Raw(output))
}

fn handle_show(args: ConfigShowArgs, repo_root: Option<&PathBuf>) -> Result<CommandOutcome> {
    let cwd = std::env::current_dir().map_err(|e| {
        crate::error::MfError::usage(format!("failed to get current directory: {e}"), None)
    })?;
    let output =
        service::config::show_effective(&cwd, repo_root.map(|p| p.as_path()), &args.output_format)?;
    Ok(CommandOutcome::Raw(output))
}

fn handle_init(args: ConfigInitArgs) -> Result<CommandOutcome> {
    let cwd = std::env::current_dir().map_err(|e| {
        crate::error::MfError::usage(format!("failed to get current directory: {e}"), None)
    })?;
    let output_path = args.output.clone();
    let path =
        service::config::init_config(&cwd, output_path.as_deref(), &args.target, args.force)?;
    Ok(CommandOutcome::Success(serde_json::json!({
        "path": path.to_string_lossy().to_string(),
    })))
}
