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
pub mod shared_flags;
pub mod source;
pub mod term;
pub mod version;

use std::path::PathBuf;

use clap::{ArgAction, Args, Parser, Subcommand};
use serde::Serialize;

use crate::cli::deprecation::DeprecationContext;
use crate::error::{MfError, Result};
use crate::output::Format;
use crate::runtime::AppContext;
use crate::service::repo;

/// Command-scope context threaded by `&mut` into dispatch and every handler.
///
/// Wraps a reference to the owned `AppContext` and the mutable diagnostics sink.
/// Handlers call accessors instead of threading loose params or reading globals.
pub struct CommandCtx<'a> {
    app: &'a AppContext,
    diagnostics: &'a mut DeprecationContext<'a>,
}

impl<'a> CommandCtx<'a> {
    pub fn new(app: &'a AppContext, diagnostics: &'a mut DeprecationContext<'a>) -> Self {
        Self { app, diagnostics }
    }

    // ── Delegating accessors ──

    pub fn format(&self) -> Format {
        self.app.format()
    }

    pub fn repo_root(&self) -> Option<&PathBuf> {
        self.app.repo_root()
    }

    pub fn cwd(&self) -> &PathBuf {
        self.app.cwd()
    }

    pub fn project(&self) -> Option<&str> {
        self.app.project()
    }

    pub fn quiet(&self) -> bool {
        self.app.quiet()
    }

    pub fn require_repo_path(&self) -> Result<&PathBuf> {
        self.app.require_repo_path()
    }

    pub fn warn_subject(&mut self, subject: &str, replacement: &str) {
        self.diagnostics.warn_subject(subject, replacement);
    }
}

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
    #[arg(long = "output", short = 'o', global = true, value_enum, default_value_t = Format::Text, help = "Output format")]
    pub format: Format,
    #[arg(long, global = true, help = "Shorthand for --output json")]
    pub json: bool,
    #[arg(long = "no-color", global = true, help = "Disable colored output")]
    pub no_color: bool,
    #[arg(short = 'p', long, global = true, value_name = "NAME", help = "Project name for project-scoped operations")]
    pub project: Option<String>,
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
#[allow(clippy::large_enum_variant)]
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
    /// Successful command execution. The optional exit code overrides the default 0
    /// (used by commands like `lint` that signal issues via non-zero exit codes).
    /// Warnings collected during execution are injected into `data.warnings` when non-empty.
    Success(serde_json::Value, Vec<String>, Option<u8>),
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

    pub fn dispatch(self, ctx: &mut CommandCtx) -> Result<CommandOutcome> {
        let outcome = match self.command {
            None => return Ok(CommandOutcome::RootHelp),
            Some(TopLevelCommand::Version) => version::handle_version(ctx),
            Some(TopLevelCommand::Source(command)) => source::dispatch(command, ctx),
            Some(TopLevelCommand::Asset(command)) => asset::dispatch(command, ctx),
            Some(TopLevelCommand::Project(command)) => project::dispatch(command, ctx),
            Some(TopLevelCommand::Article(command)) => article::dispatch(command, ctx),
            Some(TopLevelCommand::Term(command)) => term::dispatch(command, ctx),
            Some(TopLevelCommand::Completion(command)) => completion::dispatch(command, ctx),
            Some(TopLevelCommand::Build(args)) => build::dispatch(args, ctx),
            Some(TopLevelCommand::Publish(command)) => publish::dispatch(command, ctx),
            Some(TopLevelCommand::Config(command)) => config::dispatch(command, ctx),
            Some(TopLevelCommand::Render(command)) => render::dispatch(command, ctx),
            Some(TopLevelCommand::Init(args)) => dispatch_init(args),
        }?;
        // FR-080: in quiet mode, suppress success stdout (non-error data).
        if ctx.quiet() {
            match &outcome {
                CommandOutcome::Success(_, _, _) => {
                    return Ok(CommandOutcome::Success(serde_json::Value::String(String::new()), Vec::new(), None));
                }
                CommandOutcome::Raw(_, _) => {
                    return Ok(CommandOutcome::Raw(String::new(), None));
                }
                _ => {}
            }
        }
        Ok(outcome)
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
    Ok(CommandOutcome::Success(data, Vec::new(), None))
}
