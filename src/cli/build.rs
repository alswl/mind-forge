use clap::Args;
use serde::Serialize;

use crate::cli::{placeholder, CommandOutcome};
use crate::error::{MfError, Result};

#[derive(Debug, Clone, Args, Serialize)]
pub struct BuildArgs {
    pub article: String,
    #[arg(long)]
    pub project: Option<String>,
    #[arg(long)]
    pub output: Option<std::path::PathBuf>,
    #[arg(long = "dry-run")]
    pub dry_run: bool,
}

pub fn dispatch(args: BuildArgs) -> Result<CommandOutcome> {
    if args.article.is_empty() || args.article.contains('/') || args.article.contains('\\') {
        return Err(MfError::usage(
            format!("invalid article name: '{}'", args.article),
            Some("use `mf article list` to see available articles".to_string()),
        ));
    }
    placeholder("mf build", args)
}
