pub mod article;
pub mod asset;
pub mod build;
pub mod completion;
pub mod config;
pub mod project;
pub mod publish;
pub mod source;
pub mod term;

use std::path::PathBuf;

use clap::{ArgAction, Args, Parser, Subcommand};
use serde::Serialize;

use crate::error::Result;
use crate::output::{Format, PlaceholderInvocation};

#[derive(Debug, Parser)]
#[command(
    name = "mf",
    version,
    about = "mind-forge command line interface",
    disable_help_subcommand = true,
    propagate_version = true
)]
pub struct RootCli {
    #[command(flatten)]
    pub global: GlobalOpts,
    #[command(subcommand)]
    pub command: Option<TopLevelCommand>,
}

#[derive(Debug, Clone, Args, Serialize)]
pub struct GlobalOpts {
    #[arg(long, global = true, value_name = "PATH")]
    pub config: Option<PathBuf>,
    #[arg(short = 'v', long = "verbose", global = true, action = ArgAction::Count)]
    pub verbose: u8,
    #[arg(short = 'q', long = "quiet", global = true)]
    pub quiet: bool,
    #[arg(long, global = true, value_enum, default_value_t = Format::Text)]
    pub format: Format,
    #[arg(long = "no-color", global = true)]
    pub no_color: bool,
}

#[derive(Debug, Subcommand)]
pub enum TopLevelCommand {
    #[command(about = "Manage content sources")]
    Source(source::SourceCmd),
    #[command(about = "Manage project assets")]
    Asset(asset::AssetCmd),
    #[command(about = "Manage projects")]
    Project(project::ProjectCmd),
    #[command(about = "Manage articles")]
    Article(article::ArticleCmd),
    #[command(about = "Manage terminology")]
    Term(term::TermCmd),
    #[command(about = "Generate shell completion scripts")]
    Completion(completion::CompletionArgs),
    #[command(about = "Build articles")]
    Build(build::BuildArgs),
    #[command(about = "Publish articles")]
    Publish(publish::PublishCmd),
    #[command(about = "Manage configuration")]
    Config(config::ConfigCmd),
}

#[derive(Debug, Clone, Copy)]
pub enum HelpTarget {
    Source,
    Asset,
    Project,
    Article,
    Term,
    Config,
    Publish,
}

#[derive(Debug)]
pub enum CommandOutcome {
    RootHelp,
    GroupHelp(HelpTarget),
    Completion(clap_complete::Shell),
    Placeholder(PlaceholderInvocation),
    Success(serde_json::Value),
    /// Pre-serialized string content for raw output (e.g. YAML/JSON config).
    ///
    /// In text mode the string is printed as-is; in JSON mode it is embedded
    /// directly into the `{ status, data }` envelope, avoiding double-encoding.
    Raw(String),
}

impl RootCli {
    /// 判断选中的命令是否需要 Mind Repo 上下文
    pub fn command_needs_repo(&self) -> bool {
        match &self.command {
            None | Some(TopLevelCommand::Completion(_)) | Some(TopLevelCommand::Config(_)) => false,
            Some(TopLevelCommand::Project(cmd)) if is_project_index(cmd) => false,
            _ => true,
        }
    }

    pub fn dispatch(
        self,
        repo_root: Option<&std::path::PathBuf>,
        format: Format,
    ) -> Result<CommandOutcome> {
        match self.command {
            None => Ok(CommandOutcome::RootHelp),
            Some(TopLevelCommand::Source(command)) => source::dispatch(command),
            Some(TopLevelCommand::Asset(command)) => asset::dispatch(command),
            Some(TopLevelCommand::Project(command)) => {
                project::dispatch(command, repo_root, format)
            }
            Some(TopLevelCommand::Article(command)) => article::dispatch(command),
            Some(TopLevelCommand::Term(command)) => term::dispatch(command),
            Some(TopLevelCommand::Completion(command)) => completion::dispatch(command),
            Some(TopLevelCommand::Build(args)) => build::dispatch(args),
            Some(TopLevelCommand::Publish(command)) => publish::dispatch(command),
            Some(TopLevelCommand::Config(command)) => config::dispatch(command, repo_root, format),
        }
    }
}

/// `mf project index` 可以在 Mind Repo 外运行（用于创建 minds.yaml）
fn is_project_index(cmd: &project::ProjectCmd) -> bool {
    matches!(cmd.command, Some(project::ProjectSubcommand::Index(_)))
}

pub fn placeholder(command: &str, args: impl Serialize) -> Result<CommandOutcome> {
    Ok(CommandOutcome::Placeholder(PlaceholderInvocation::new(
        command,
        serde_json::to_value(args)?,
    )))
}
