use std::path::PathBuf;

use clap::{Args, Subcommand};
use serde::Serialize;

use crate::cli::{placeholder, CommandOutcome, HelpTarget};
use crate::error::{MfError, Result};

#[derive(Debug, Clone, Args)]
pub struct ProjectCmd {
    #[command(subcommand)]
    pub command: Option<ProjectSubcommand>,
}

#[derive(Debug, Clone, Subcommand)]
pub enum ProjectSubcommand {
    #[command(about = "Create a project")]
    New(ProjectNewArgs),
    #[command(about = "List projects")]
    List(ProjectListArgs),
    #[command(about = "Archive a project")]
    Archive(ProjectArchiveArgs),
    #[command(about = "Show project status")]
    Status(ProjectStatusArgs),
    #[command(about = "Lint a project")]
    Lint(ProjectLintArgs),
    #[command(about = "Index projects")]
    Index(ProjectIndexArgs),
}

#[derive(Debug, Clone, Args, Serialize)]
pub struct ProjectNewArgs {
    pub name: String,
    #[arg(long)]
    pub path: Option<PathBuf>,
    #[arg(long)]
    pub template: Option<String>,
    #[arg(long)]
    pub force: bool,
}

#[derive(Debug, Clone, Args, Serialize)]
pub struct ProjectListArgs {}

#[derive(Debug, Clone, Args, Serialize)]
pub struct ProjectArchiveArgs {
    pub name_or_path: String,
}

#[derive(Debug, Clone, Args, Serialize)]
pub struct ProjectStatusArgs {
    pub name_or_path: Option<String>,
    #[arg(long = "output-format")]
    pub output_format: Option<String>,
}

#[derive(Debug, Clone, Args, Serialize)]
pub struct ProjectLintArgs {
    pub path: Option<PathBuf>,
    #[arg(long)]
    pub fix: bool,
    #[arg(long = "rule")]
    pub rule: Vec<String>,
}

#[derive(Debug, Clone, Args, Serialize)]
pub struct ProjectIndexArgs {}

pub fn dispatch(command: ProjectCmd) -> Result<CommandOutcome> {
    match command.command {
        None => Ok(CommandOutcome::GroupHelp(HelpTarget::Project)),
        Some(ProjectSubcommand::New(args)) => {
            let resolved_path =
                args.path.clone().unwrap_or_else(|| PathBuf::from(format!("./{}", args.name)));
            if resolved_path.exists() && !args.force {
                return Err(MfError::usage(
                    format!(
                        "directory '{}' already exists; pass '--force' to overwrite",
                        resolved_path.display()
                    ),
                    None,
                ));
            }
            placeholder("mf project new", ProjectNewPayload::from(args))
        }
        Some(ProjectSubcommand::List(args)) => placeholder("mf project list", args),
        Some(ProjectSubcommand::Archive(args)) => placeholder("mf project archive", args),
        Some(ProjectSubcommand::Status(args)) => placeholder("mf project status", args),
        Some(ProjectSubcommand::Lint(args)) => placeholder("mf project lint", args),
        Some(ProjectSubcommand::Index(args)) => placeholder("mf project index", args),
    }
}

#[derive(Debug, Serialize)]
struct ProjectNewPayload {
    name: String,
    path: PathBuf,
    template: Option<String>,
    force: bool,
}

impl From<ProjectNewArgs> for ProjectNewPayload {
    fn from(value: ProjectNewArgs) -> Self {
        let path = value.path.unwrap_or_else(|| PathBuf::from(format!("./{}", value.name)));
        Self { name: value.name, path, template: value.template, force: value.force }
    }
}
