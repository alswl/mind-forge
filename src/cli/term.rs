use clap::{Args, Subcommand};
use serde::Serialize;

use crate::cli::{placeholder, CommandOutcome, HelpTarget};
use crate::error::Result;

#[derive(Debug, Clone, Args)]
pub struct TermCmd {
    #[command(subcommand)]
    pub command: Option<TermSubcommand>,
}

#[derive(Debug, Clone, Subcommand)]
pub enum TermSubcommand {
    #[command(about = "List terms")]
    List(TermListArgs),
    #[command(about = "Create a term")]
    New(TermNewArgs),
    #[command(about = "Lint terms")]
    Lint(TermLintArgs),
    #[command(about = "Learn term correction")]
    Learn(TermLearnArgs),
    #[command(about = "Fix a term")]
    Fix(TermFixArgs),
}

#[derive(Debug, Clone, Args, Serialize)]
pub struct TermListArgs {
    #[arg(long)]
    pub filter: Option<String>,
}

#[derive(Debug, Clone, Args, Serialize)]
pub struct TermNewArgs {
    pub term: String,
    #[arg(long)]
    pub definition: Option<String>,
    #[arg(long = "alias")]
    pub alias: Vec<String>,
    #[arg(long = "tag")]
    pub tag: Vec<String>,
}

#[derive(Debug, Clone, Args, Serialize)]
pub struct TermLintArgs {}

#[derive(Debug, Clone, Args, Serialize)]
pub struct TermLearnArgs {
    #[arg(long)]
    pub original: String,
    #[arg(long)]
    pub correct: String,
}

#[derive(Debug, Clone, Args, Serialize)]
pub struct TermFixArgs {
    pub term: String,
    #[arg(long)]
    pub definition: Option<String>,
    #[arg(long = "alias")]
    pub alias: Vec<String>,
    #[arg(long = "tag")]
    pub tag: Vec<String>,
}

pub fn dispatch(command: TermCmd) -> Result<CommandOutcome> {
    match command.command {
        None => Ok(CommandOutcome::GroupHelp(HelpTarget::Term)),
        Some(TermSubcommand::List(args)) => placeholder("mf term list", args),
        Some(TermSubcommand::New(args)) => placeholder("mf term new", args),
        Some(TermSubcommand::Lint(args)) => placeholder("mf term lint", args),
        Some(TermSubcommand::Learn(args)) => placeholder("mf term learn", args),
        Some(TermSubcommand::Fix(args)) => placeholder("mf term fix", args),
    }
}
