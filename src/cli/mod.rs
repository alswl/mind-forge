pub mod article;
pub mod asset;
pub mod build;
pub mod completion;
pub mod config;
pub mod deprecation;
pub mod project;
pub mod prompt;
pub mod publish;
pub mod render;
pub mod source;
pub mod term;

use std::path::PathBuf;

use clap::{ArgAction, Args, Parser, Subcommand};
use serde::Serialize;

use crate::cli::completion::ShellKind;
use crate::cli::deprecation::DeprecationContext;
use crate::error::{MfError, Result};
use crate::output::Format;
use crate::service::repo;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepoRequirement {
    Required,
    NotRequired,
}

#[derive(Debug, Args)]
pub struct InitArgs {
    /// Target directory path. Defaults to the current directory when omitted.
    #[arg(value_name = "PATH", help = "Target directory path (defaults to current directory)")]
    pub path: Option<PathBuf>,
}

#[derive(Debug, Parser)]
#[command(
    name = "mf",
    version,
    about = "mind-forge: a local-first knowledge management CLI",
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
    #[arg(long, global = true, value_name = "PATH", help = "Mind Repo root directory")]
    pub root: Option<PathBuf>,
    #[arg(long, global = true, value_name = "PATH", help = "Config file path")]
    pub config: Option<PathBuf>,
    #[arg(short = 'v', long = "verbose", global = true, action = ArgAction::Count, help = "Verbose output")]
    pub verbose: u8,
    #[arg(short = 'q', long = "quiet", global = true, help = "Silence non-error output")]
    pub quiet: bool,
    #[arg(long, global = true, value_enum, default_value_t = Format::Text, help = "Output format")]
    pub format: Format,
    #[arg(long, global = true, help = "Shorthand for --format json")]
    pub json: bool,
    #[arg(long = "no-color", global = true, help = "Disable colored output")]
    pub no_color: bool,
    #[arg(long = "install-completion", global = true, value_enum, help = "Install shell completion script")]
    pub install_completion: Option<ShellKind>,
    #[arg(long = "show-completion", global = true, value_enum, help = "Show shell completion script")]
    pub show_completion: Option<ShellKind>,
}

impl GlobalOpts {
    pub fn effective_format(&self) -> Format {
        if self.json {
            Format::Json
        } else {
            self.format
        }
    }
}

#[derive(Debug, Subcommand)]
pub enum TopLevelCommand {
    // ── Repo lifecycle ──
    #[command(about = "Initialize a directory as a Mind Repo")]
    Init(InitArgs),

    // ── Managed resources ──
    #[command(about = "Manage projects")]
    Project(project::ProjectCmd),
    #[command(about = "Manage articles")]
    Article(article::ArticleCmd),
    #[command(about = "Manage content sources")]
    Source(source::SourceCmd),
    #[command(about = "Manage project assets")]
    Asset(asset::AssetCmd),
    #[command(about = "Manage terminology", visible_alias = "terms")]
    Term(term::TermCmd),

    // ── Workflows ──
    #[command(about = "Build articles")]
    Build(build::BuildArgs),
    #[command(about = "Publish articles to configured targets")]
    Publish(publish::PublishCmd),
    #[command(about = "Generate render prompts (emits prompts only, no output files)")]
    Render(render::RenderCmd),

    // ── Utilities ──
    #[command(about = "Manage configuration")]
    Config(config::ConfigCmd),
    #[command(about = "Generate shell completion scripts")]
    Completion(completion::CompletionArgs),
    #[command(about = "Show version information")]
    Version,
}

#[derive(Debug)]
pub enum CommandOutcome {
    RootHelp,
    GroupHelp(&'static str),
    Completion(clap_complete::Shell),
    /// Show version information
    Version,
    /// Successful command execution. The optional exit code overrides the default 0
    /// (used by commands like `lint` that signal issues via non-zero exit codes).
    Success(serde_json::Value, Option<u8>),
    /// Pre-serialized string content for raw output (e.g. YAML/JSON config).
    ///
    /// In text mode the string is printed as-is; in JSON mode it is embedded
    /// directly into the `{ status, data }` envelope, avoiding double-encoding.
    /// Optional exit code overrides the default 0.
    Raw(String, Option<u8>),
}

impl RootCli {
    pub fn requires_repo(&self) -> RepoRequirement {
        self.command.as_ref().map(|c| c.requires_repo()).unwrap_or(RepoRequirement::NotRequired)
    }

    pub fn dispatch(
        self,
        repo_root: Option<&std::path::PathBuf>,
        format: Format,
        deprecation: &mut DeprecationContext,
    ) -> Result<CommandOutcome> {
        match self.command {
            None => Ok(CommandOutcome::RootHelp),
            Some(TopLevelCommand::Version) => Ok(CommandOutcome::Version),
            Some(TopLevelCommand::Source(command)) => source::dispatch(command, repo_root, format, deprecation),
            Some(TopLevelCommand::Asset(command)) => asset::dispatch(command, repo_root, format, deprecation),
            Some(TopLevelCommand::Project(command)) => project::dispatch(command, repo_root, format, deprecation),
            Some(TopLevelCommand::Article(command)) => article::dispatch(command, repo_root, format, deprecation),
            Some(TopLevelCommand::Term(command)) => term::dispatch(command, repo_root, format, deprecation),
            Some(TopLevelCommand::Completion(command)) => completion::dispatch(command),
            Some(TopLevelCommand::Build(args)) => build::dispatch(args, repo_root, format, deprecation),
            Some(TopLevelCommand::Publish(command)) => publish::dispatch(command, repo_root, format, deprecation),
            Some(TopLevelCommand::Config(command)) => config::dispatch(command, repo_root, format, deprecation),
            Some(TopLevelCommand::Render(command)) => render::dispatch(command, repo_root, format),
            Some(TopLevelCommand::Init(args)) => dispatch_init(args),
        }
    }
}

impl TopLevelCommand {
    pub fn requires_repo(&self) -> RepoRequirement {
        match self {
            Self::Init(_) | Self::Completion(_) | Self::Config(_) | Self::Version => RepoRequirement::NotRequired,
            Self::Project(cmd) => cmd.requires_repo(),
            _ => RepoRequirement::Required,
        }
    }
}

fn dispatch_init(args: InitArgs) -> Result<CommandOutcome> {
    let target = args.path.unwrap_or_else(|| PathBuf::from("."));
    let kind = repo::classify_repo_target(&target)?;

    // Only guard against nesting when we'd actually create a new repo.
    // ExistingRepo is idempotent and MalformedManifest will surface its own
    // error from init_repo with better context.
    if matches!(kind, repo::RepoTargetKind::NewDirectory | repo::RepoTargetKind::ExistingEmptyDirectory) {
        repo::validate_not_nested(&target)?;
    }

    let report = repo::init_repo(&target, &kind)?;
    let data = serde_json::to_value(&report).map_err(|e| MfError::Internal(e.into()))?;
    Ok(CommandOutcome::Success(data, None))
}
