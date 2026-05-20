use std::path::PathBuf;

use clap::{Args, Subcommand};
use serde::Serialize;

use crate::cli::deprecation::DeprecationContext;
use crate::cli::CommandOutcome;
use crate::error::Result;
use crate::output::Format;
use crate::service;

#[derive(Debug, Clone, Args)]
pub struct ConfigCmd {
    #[command(subcommand)]
    pub command: Option<ConfigSubcommand>,
}

#[derive(Debug, Clone, Subcommand)]
pub enum ConfigSubcommand {
    #[command(about = "Show config schema")]
    Schema(ConfigSchemaArgs),
    #[command(about = "Show effective config")]
    Show(ConfigShowArgs),
    #[command(about = "Compile config (deprecated: use `show`)", hide = true)]
    Compile(ConfigCompileArgs),
    #[command(about = "Generate effective config file")]
    Generate(ConfigGenerateArgs),
    #[command(about = "Show default config values")]
    Default(ConfigDefaultArgs),
    #[command(about = "Initialize config file (deprecated: use `mf init`)")]
    Init(ConfigInitArgs),
}

#[derive(Debug, Clone, Args, Serialize)]
pub struct ConfigSchemaArgs {
    #[arg(long = "output-format", default_value = "json")]
    pub output_format: String,
}

#[derive(Debug, Clone, Args, Serialize)]
pub struct ConfigShowArgs {
    #[arg(long = "output-format", default_value = "yaml")]
    pub output_format: String,
}

#[derive(Debug, Clone, Args, Serialize)]
pub struct ConfigInitArgs {
    #[arg(long)]
    pub output: Option<PathBuf>,
    #[arg(long, default_value = "project")]
    pub target: String,
    #[arg(long)]
    pub force: bool,
}

// ---------------------------------------------------------------------------
// B2 thin alias args
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Args, Serialize)]
pub struct ConfigCompileArgs {
    #[arg(long = "output-format", default_value = "yaml")]
    pub output_format: String,
}

#[derive(Debug, Clone, Args, Serialize)]
pub struct ConfigGenerateArgs {
    #[arg(long = "output-format", default_value = "yaml")]
    pub output_format: String,
    #[arg(short = 'o', long)]
    pub output: PathBuf,
}

#[derive(Debug, Clone, Args, Serialize)]
pub struct ConfigDefaultArgs {
    #[arg(long = "output-format", default_value = "yaml")]
    pub output_format: String,
}

pub fn dispatch(
    command: ConfigCmd,
    repo_root: Option<&PathBuf>,
    _format: Format,
    deprecation: &mut DeprecationContext,
) -> Result<CommandOutcome> {
    match command.command {
        None => Ok(CommandOutcome::GroupHelp("config")),
        Some(ConfigSubcommand::Schema(args)) => handle_schema(args),
        Some(ConfigSubcommand::Show(args)) => handle_show(args, repo_root),
        Some(ConfigSubcommand::Compile(args)) => {
            deprecation.warn_subject("config compile", "config show");
            handle_show(ConfigShowArgs { output_format: args.output_format }, repo_root)
        }
        Some(ConfigSubcommand::Generate(args)) => handle_generate(args, repo_root),
        Some(ConfigSubcommand::Default(args)) => handle_default(args),
        Some(ConfigSubcommand::Init(args)) => {
            deprecation.warn_subject("config init", "mf init");
            handle_init(args)
        }
    }
}

fn handle_schema(args: ConfigSchemaArgs) -> Result<CommandOutcome> {
    let output = service::config::schema_output(&args.output_format)?;
    Ok(CommandOutcome::Raw(output, None))
}

fn handle_show(args: ConfigShowArgs, repo_root: Option<&PathBuf>) -> Result<CommandOutcome> {
    let cwd = std::env::current_dir()
        .map_err(|e| crate::error::MfError::usage(format!("failed to get current directory: {e}"), None))?;
    let output = service::config::show_effective(&cwd, repo_root.map(|p| p.as_path()), &args.output_format)?;
    Ok(CommandOutcome::Raw(output, None))
}

// ── B2 thin alias: config generate (show + write to file) ───────────────────

fn handle_generate(args: ConfigGenerateArgs, repo_root: Option<&PathBuf>) -> Result<CommandOutcome> {
    use std::io::Write;

    let cwd = std::env::current_dir()
        .map_err(|e| crate::error::MfError::usage(format!("failed to get current directory: {e}"), None))?;
    let output = service::config::show_effective(&cwd, repo_root.map(|p| p.as_path()), &args.output_format)?;
    let mut file = std::fs::File::create(&args.output).map_err(crate::error::MfError::Io)?;
    file.write_all(output.as_bytes()).map_err(crate::error::MfError::Io)?;
    Ok(CommandOutcome::Success(serde_json::json!({ "path": args.output.to_string_lossy().to_string() }), None))
}

// ── B2 thin alias: config default (show default config) ─────────────────────

fn handle_default(args: ConfigDefaultArgs) -> Result<CommandOutcome> {
    let default_config = crate::model::config::MindConfig::default();
    let output = match args.output_format.as_str() {
        "json" => service::config::to_json(&default_config)?,
        _ => service::config::to_yaml(&default_config)?,
    };
    Ok(CommandOutcome::Raw(output, None))
}

fn handle_init(args: ConfigInitArgs) -> Result<CommandOutcome> {
    let cwd = std::env::current_dir()
        .map_err(|e| crate::error::MfError::usage(format!("failed to get current directory: {e}"), None))?;
    let output_path = args.output.clone();
    let path = service::config::init_config(&cwd, output_path.as_deref(), &args.target, args.force)?;
    Ok(CommandOutcome::Success(
        serde_json::json!({
            "path": path.to_string_lossy().to_string(),
        }),
        None,
    ))
}
