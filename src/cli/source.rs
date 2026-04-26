use clap::{Args, Subcommand, ValueEnum};
use serde::Serialize;

use crate::cli::{placeholder, CommandOutcome, HelpTarget};
use crate::error::Result;

#[derive(Debug, Clone, Args)]
pub struct SourceCmd {
    #[command(subcommand)]
    pub command: Option<SourceSubcommand>,
}

#[derive(Debug, Clone, Subcommand)]
pub enum SourceSubcommand {
    #[command(about = "List sources")]
    List(SourceListArgs),
    #[command(about = "Add a source")]
    Add(SourceAddArgs),
    #[command(about = "Update sources")]
    Update(SourceUpdateArgs),
}

#[derive(Debug, Clone, Args, Serialize)]
pub struct SourceListArgs {
    #[arg(long)]
    pub filter: Option<String>,
}

#[derive(Debug, Clone, Args, Serialize)]
pub struct SourceAddArgs {
    pub url: String,
    #[arg(long)]
    pub name: Option<String>,
    #[arg(long = "type", value_enum, default_value_t = SourceKind::Auto)]
    pub source_type: SourceKind,
    #[arg(long = "tag")]
    pub tag: Vec<String>,
}

#[derive(Debug, Clone, Copy, ValueEnum, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SourceKind {
    Auto,
    Rss,
    Web,
    File,
}

#[derive(Debug, Clone, Args, Serialize)]
#[group(id = "target", required = true, multiple = false)]
pub struct SourceUpdateArgs {
    #[arg(group = "target")]
    pub id: Option<String>,
    #[arg(long, group = "target")]
    pub all: bool,
}

pub fn dispatch(command: SourceCmd) -> Result<CommandOutcome> {
    match command.command {
        None => Ok(CommandOutcome::GroupHelp(HelpTarget::Source)),
        Some(SourceSubcommand::List(args)) => placeholder("mf source list", args),
        Some(SourceSubcommand::Add(args)) => {
            placeholder("mf source add", SourceAddPayload::from(args))
        }
        Some(SourceSubcommand::Update(args)) => placeholder("mf source update", args),
    }
}

#[derive(Debug, Serialize)]
struct SourceAddPayload {
    url: String,
    name: Option<String>,
    #[serde(rename = "type")]
    source_type: SourceKind,
    tag: Vec<String>,
}

impl From<SourceAddArgs> for SourceAddPayload {
    fn from(value: SourceAddArgs) -> Self {
        Self { url: value.url, name: value.name, source_type: value.source_type, tag: value.tag }
    }
}
