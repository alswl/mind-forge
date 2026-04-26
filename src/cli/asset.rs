use std::path::PathBuf;

use clap::{Args, Subcommand, ValueEnum};
use serde::Serialize;

use crate::cli::{placeholder, CommandOutcome, HelpTarget};
use crate::error::{MfError, Result};

#[derive(Debug, Clone, Args)]
pub struct AssetCmd {
    #[command(subcommand)]
    pub command: Option<AssetSubcommand>,
}

#[derive(Debug, Clone, Subcommand)]
pub enum AssetSubcommand {
    #[command(about = "List assets")]
    List(AssetListArgs),
    #[command(about = "Add an asset")]
    Add(AssetAddArgs),
    #[command(about = "Update assets")]
    Update(AssetUpdateArgs),
}

#[derive(Debug, Clone, Args, Serialize)]
pub struct AssetListArgs {
    #[arg(long)]
    pub filter: Option<String>,
    #[arg(long = "type", value_enum)]
    pub asset_type: Option<AssetKind>,
}

#[derive(Debug, Clone, Copy, ValueEnum, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum AssetKind {
    Image,
    Video,
    Audio,
    Other,
}

#[derive(Debug, Clone, Args)]
pub struct AssetAddArgs {
    pub path: PathBuf,
    #[arg(long = "tag")]
    pub tag: Vec<String>,
    #[arg(long, conflicts_with = "link")]
    pub copy: bool,
    #[arg(long, conflicts_with = "copy")]
    pub link: bool,
}

#[derive(Debug, Clone, Args, Serialize)]
#[group(id = "target", required = true, multiple = false)]
pub struct AssetUpdateArgs {
    #[arg(group = "target")]
    pub id: Option<String>,
    #[arg(long, group = "target")]
    pub all: bool,
}

pub fn dispatch(command: AssetCmd) -> Result<CommandOutcome> {
    match command.command {
        None => Ok(CommandOutcome::GroupHelp(HelpTarget::Asset)),
        Some(AssetSubcommand::List(args)) => {
            placeholder("mf asset list", AssetListPayload::from(args))
        }
        Some(AssetSubcommand::Add(args)) => {
            if !args.path.exists() {
                return Err(MfError::usage(
                    format!("path '{}' does not exist", args.path.display()),
                    None,
                ));
            }
            placeholder("mf asset add", AssetAddPayload::from(args))
        }
        Some(AssetSubcommand::Update(args)) => placeholder("mf asset update", args),
    }
}

#[derive(Debug, Serialize)]
struct AssetListPayload {
    filter: Option<String>,
    #[serde(rename = "type")]
    asset_type: Option<AssetKind>,
}

impl From<AssetListArgs> for AssetListPayload {
    fn from(value: AssetListArgs) -> Self {
        Self { filter: value.filter, asset_type: value.asset_type }
    }
}

#[derive(Debug, Serialize)]
struct AssetAddPayload {
    path: PathBuf,
    tag: Vec<String>,
    mode: &'static str,
}

impl From<AssetAddArgs> for AssetAddPayload {
    fn from(value: AssetAddArgs) -> Self {
        let mode = if value.link { "link" } else { "copy" };
        Self { path: value.path, tag: value.tag, mode }
    }
}
