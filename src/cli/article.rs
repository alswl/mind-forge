use std::path::PathBuf;

use clap::{Args, Subcommand, ValueEnum};
use serde::Serialize;

use crate::cli::{placeholder, CommandOutcome, HelpTarget};
use crate::error::Result;

#[derive(Debug, Clone, Args)]
pub struct ArticleCmd {
    #[command(subcommand)]
    pub command: Option<ArticleSubcommand>,
}

#[derive(Debug, Clone, Subcommand)]
pub enum ArticleSubcommand {
    #[command(about = "Create an article")]
    New(ArticleNewArgs),
    #[command(about = "List articles")]
    List(ArticleListArgs),
    #[command(about = "Build articles")]
    Build(ArticleBuildArgs),
    #[command(about = "Publish an article")]
    Publish(ArticlePublishArgs),
}

#[derive(Debug, Clone, Args)]
pub struct ArticleNewArgs {
    pub title: String,
    #[arg(long)]
    pub project: Option<String>,
    #[arg(long)]
    pub template: Option<String>,
    #[arg(long = "tag")]
    pub tag: Vec<String>,
    #[arg(long, default_value_t = true)]
    pub draft: bool,
}

#[derive(Debug, Clone, Args, Serialize)]
pub struct ArticleListArgs {
    #[arg(long)]
    pub project: Option<String>,
    #[arg(long, value_enum, default_value_t = ArticleStatusFilter::All)]
    pub status: ArticleStatusFilter,
}

#[derive(Debug, Clone, Copy, ValueEnum, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ArticleStatusFilter {
    Draft,
    Published,
    All,
}

#[derive(Debug, Clone, Args, Serialize)]
pub struct ArticleBuildArgs {
    pub id_or_path: Option<String>,
    #[arg(long)]
    pub out: Option<PathBuf>,
    #[arg(long)]
    pub watch: bool,
    #[arg(long)]
    pub clean: bool,
}

#[derive(Debug, Clone, Args, Serialize)]
pub struct ArticlePublishArgs {
    pub id_or_path: String,
    #[arg(long)]
    pub target: Option<String>,
    #[arg(long = "dry-run")]
    pub dry_run: bool,
    #[arg(long)]
    pub force: bool,
}

pub fn dispatch(command: ArticleCmd) -> Result<CommandOutcome> {
    match command.command {
        None => Ok(CommandOutcome::GroupHelp(HelpTarget::Article)),
        Some(ArticleSubcommand::New(args)) => {
            placeholder("mf article new", ArticleNewPayload::from(args))
        }
        Some(ArticleSubcommand::List(args)) => placeholder("mf article list", args),
        Some(ArticleSubcommand::Build(args)) => placeholder("mf article build", args),
        Some(ArticleSubcommand::Publish(args)) => placeholder("mf article publish", args),
    }
}

#[derive(Debug, Serialize)]
struct ArticleNewPayload {
    title: String,
    project: Option<String>,
    template: Option<String>,
    tag: Vec<String>,
    draft: bool,
}

impl From<ArticleNewArgs> for ArticleNewPayload {
    fn from(value: ArticleNewArgs) -> Self {
        Self {
            title: value.title,
            project: value.project,
            template: value.template,
            tag: value.tag,
            draft: value.draft,
        }
    }
}
