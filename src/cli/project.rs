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
    #[command(about = "Lint a project")]
    Lint(ProjectLintArgs),
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
pub struct ProjectLintArgs {
    pub path: Option<PathBuf>,
    #[arg(long)]
    pub fix: bool,
    #[arg(long = "rule")]
    pub rule: Vec<String>,
}

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
        Some(ProjectSubcommand::Lint(args)) => placeholder("mf project lint", args),
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
