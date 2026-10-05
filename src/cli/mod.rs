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
pub mod source_rag;
pub mod term;
pub mod thinking;
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
    /// Project derived from a path-form positional argument; consulted only
    /// when `--project` was not given.
    project_override: Option<String>,
}

impl<'a> CommandCtx<'a> {
    pub fn new(app: &'a AppContext, diagnostics: &'a mut DeprecationContext<'a>) -> Self {
        Self { app, diagnostics, project_override: None }
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
        self.app.project().or(self.project_override.as_deref())
    }

    /// Let a path-form positional argument (e.g. `projects/blog/docs/post`)
    /// select its own project. The argument is rewritten to the
    /// project-relative form the commands already understand. Does nothing
    /// when `--project` is given or the argument is not an existing path
    /// inside a project.
    fn adopt_path_selector(&mut self, selector: &mut String) {
        use crate::service::util;

        if self.app.project().is_some() {
            return;
        }
        let Some(root) = self.app.repo_root() else { return };
        let Some(path) = util::existing_path_selector(root, self.app.cwd(), selector) else { return };
        let Ok(project_path) = util::project_root_for_source(root, &path) else { return };
        let Ok(relative) = util::rel_posix_path(&project_path, &path) else { return };
        if relative.is_empty() {
            return;
        }
        let cwd_project = util::detect_current_project(root, self.app.cwd())
            .and_then(|detected| util::resolve_project(root, Some(&detected), self.app.cwd()).ok());
        if cwd_project.map(|p| util::try_canonicalize(&p)) != Some(util::try_canonicalize(&project_path)) {
            self.project_override = Some(project_path.to_string_lossy().into_owned());
        }
        *selector = relative;
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
    version = version::FULL_VERSION,
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
    #[arg(
        short = 'p',
        long,
        global = true,
        value_name = "NAME_OR_PATH",
        help = "Project name or path (cwd-relative, repo-relative, or absolute)"
    )]
    pub project: Option<String>,
    #[arg(long = "generate-manual", global = true, hide = true)]
    pub generate_manual: bool,
}

impl GlobalOpts {
    pub fn effective_format(&self) -> Format {
        if self.json { Format::Json } else { self.format }
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
    #[command(about = "View prompt projections")]
    Prompt(prompt::PromptCmd),
    #[command(about = "View thinking projections")]
    Thinking(thinking::ThinkingCmd),
    #[command(about = "Manage content sources")]
    Source(source::SourceCmd),
    #[command(about = "Search the repository-wide RAG corpus")]
    Search(source::GlobalSearchArgs),
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

    pub fn dispatch(mut self, ctx: &mut CommandCtx) -> Result<CommandOutcome> {
        if let Some(selector) = self.command.as_mut().and_then(TopLevelCommand::path_selector_mut) {
            ctx.adopt_path_selector(selector);
        }
        let outcome = match self.command {
            None => return Ok(CommandOutcome::RootHelp),
            Some(TopLevelCommand::Version) => version::handle_version(ctx),
            Some(TopLevelCommand::Source(command)) => source::dispatch(command, ctx),
            Some(TopLevelCommand::Search(args)) => source::dispatch_global_search(args, ctx),
            Some(TopLevelCommand::Asset(command)) => asset::dispatch(command, ctx),
            Some(TopLevelCommand::Project(command)) => project::dispatch(command, ctx),
            Some(TopLevelCommand::Article(command)) => article::dispatch(command, ctx),
            Some(TopLevelCommand::Prompt(command)) => prompt::dispatch(command, ctx),
            Some(TopLevelCommand::Thinking(command)) => thinking::dispatch(command, ctx),
            Some(TopLevelCommand::Term(command)) => term::dispatch(command, ctx),
            Some(TopLevelCommand::Completion(command)) => completion::dispatch(command, ctx),
            Some(TopLevelCommand::Build(args)) => build::dispatch(args, ctx),
            Some(TopLevelCommand::Publish(command)) => publish::dispatch(command, ctx),
            Some(TopLevelCommand::Config(command)) => config::dispatch(command, ctx),
            Some(TopLevelCommand::Render(command)) => render::dispatch(command, ctx),
            Some(TopLevelCommand::Init(args)) => dispatch_init(args),
        }?;
        Ok(outcome)
    }
}

impl TopLevelCommand {
    /// The positional argument that names an existing article, prompt,
    /// thinking, asset or source, if this command takes one.
    fn path_selector_mut(&mut self) -> Option<&mut String> {
        use article::{ArticleBlockSubcommand as Block, ArticleSubcommand as A};
        use asset::AssetSubcommand as As;
        use publish::PublishSubcommand as P;
        use source::SourceSubcommand as S;

        match self {
            Self::Build(args) => Some(&mut args.article),
            Self::Render(cmd) => cmd.article.as_mut(),
            Self::Prompt(cmd) => match cmd.command.as_mut()? {
                prompt::PromptSubcommand::Show { path } => Some(path),
                _ => None,
            },
            Self::Thinking(cmd) => match cmd.command.as_mut()? {
                thinking::ThinkingSubcommand::Show { path } => Some(path),
                _ => None,
            },
            Self::Publish(cmd) => match cmd.command.as_mut()? {
                P::Run(args) => Some(&mut args.article),
                P::Update(args) => Some(&mut args.article),
                P::Target(_) => None,
            },
            Self::Article(cmd) => match cmd.command.as_mut()? {
                A::Show(args) => Some(&mut args.path),
                A::Update(args) => Some(&mut args.path),
                A::Remove(args) => Some(&mut args.path),
                A::Move(args) => Some(&mut args.path),
                A::Rename(args) => Some(&mut args.old_path),
                A::Block(Block::New(args)) => Some(&mut args.article),
                A::Block(Block::Move(args)) => Some(&mut args.article),
                A::Block(Block::Renumber(args)) => Some(&mut args.article),
                A::Block(Block::Rename(args)) => Some(&mut args.article),
                A::Block(Block::Rm(args)) => Some(&mut args.article),
                _ => None,
            },
            Self::Asset(cmd) => match cmd.command.as_mut()? {
                As::Show(args) => Some(&mut args.path),
                As::Remove(args) => Some(&mut args.path),
                As::Rename(args) => Some(&mut args.old_path),
                As::Move(args) => Some(&mut args.path),
                _ => None,
            },
            Self::Source(cmd) => match cmd.command.as_mut()? {
                S::Show(args) => Some(&mut args.path),
                S::Update(args) => Some(&mut args.path),
                S::Remove(args) => Some(&mut args.name_or_path),
                S::Rename(args) => Some(&mut args.old_path),
                S::Move(args) => Some(&mut args.path),
                _ => None,
            },
            _ => None,
        }
    }

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
