use std::path::PathBuf;

use clap::{Args, Subcommand};
use serde::Serialize;

use crate::cli::{placeholder, CommandOutcome};
use crate::error::Result;

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

pub fn dispatch(command: ConfigCmd) -> Result<CommandOutcome> {
    match command.command {
        None => Ok(CommandOutcome::GroupHelp(crate::cli::HelpTarget::Config)),
        Some(ConfigSubcommand::Schema(args)) => placeholder("mf config schema", args),
        Some(ConfigSubcommand::Show(args)) => placeholder("mf config show", args),
        Some(ConfigSubcommand::Init(args)) => placeholder("mf config init", args),
    }
}
