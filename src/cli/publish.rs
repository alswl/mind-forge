use std::path::PathBuf;

use clap::{Args, Subcommand};
use serde::Serialize;

use crate::cli::{placeholder, CommandOutcome};
use crate::error::Result;

#[derive(Debug, Clone, Args)]
pub struct PublishCmd {
    #[command(subcommand)]
    pub command: Option<PublishSubcommand>,
}

#[derive(Debug, Clone, Subcommand)]
pub enum PublishSubcommand {
    #[command(about = "Publish a file to a target")]
    Run(PublishRunArgs),
    #[command(about = "Update publish record")]
    Update(PublishUpdateArgs),
}

#[derive(Debug, Clone, Args, Serialize)]
pub struct PublishRunArgs {
    pub path: PathBuf,
    #[arg(long)]
    pub target: Option<String>,
    #[arg(long = "dry-run")]
    pub dry_run: bool,
    #[arg(long)]
    pub force: bool,
}

#[derive(Debug, Clone, Args, Serialize)]
pub struct PublishUpdateArgs {
    pub path: PathBuf,
    #[arg(long)]
    pub status: Option<String>,
    #[arg(long = "target-url")]
    pub target_url: Option<String>,
}

pub fn dispatch(command: PublishCmd) -> Result<CommandOutcome> {
    match command.command {
        None => Ok(CommandOutcome::GroupHelp(crate::cli::HelpTarget::Publish)),
        Some(PublishSubcommand::Run(args)) => placeholder("mf publish run", args),
        Some(PublishSubcommand::Update(args)) => placeholder("mf publish update", args),
    }
}
