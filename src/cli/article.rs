use clap::{Args, Subcommand};
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
    #[command(about = "Lint articles (see also `mf build --help`)")]
    Lint(ArticleLintArgs),
    #[command(about = "Index articles (see also `mf build --help`)")]
    Index(ArticleIndexArgs),
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
}

#[derive(Debug, Clone, Args, Serialize)]
pub struct ArticleLintArgs {
    #[arg(long)]
    pub fix: bool,
}

#[derive(Debug, Clone, Args, Serialize)]
pub struct ArticleIndexArgs {}

pub fn dispatch(command: ArticleCmd) -> Result<CommandOutcome> {
    match command.command {
        None => Ok(CommandOutcome::GroupHelp(HelpTarget::Article)),
        Some(ArticleSubcommand::New(args)) => {
            placeholder("mf article new", ArticleNewPayload::from(args))
        }
        Some(ArticleSubcommand::List(args)) => placeholder("mf article list", args),
        Some(ArticleSubcommand::Lint(args)) => placeholder("mf article lint", args),
        Some(ArticleSubcommand::Index(args)) => placeholder("mf article index", args),
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
