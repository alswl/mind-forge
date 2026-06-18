use std::path::PathBuf;

use clap::{Args, Subcommand};
use serde::Serialize;

use crate::cli::deprecation::DeprecationContext;
use crate::cli::CommandOutcome;
use crate::error::Result;
use crate::model::terminal::{CapabilityDiagnosticReport, DiagnosticCheck, OutputFormat};
use crate::output::capability::{build_environment_summary, build_policy, build_profile};
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
    #[command(about = "Show terminal capability diagnostics")]
    Terminal,
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
    format: Format,
    deprecation: &mut DeprecationContext,
) -> Result<CommandOutcome> {
    match command.command {
        None => Ok(CommandOutcome::GroupHelp("config")),
        Some(ConfigSubcommand::Schema(args)) => handle_schema(args),
        Some(ConfigSubcommand::Show(args)) => handle_show(args, repo_root, format),
        Some(ConfigSubcommand::Compile(args)) => {
            deprecation.warn_subject("config compile", "config show");
            handle_show(ConfigShowArgs { output_format: args.output_format }, repo_root, format)
        }
        Some(ConfigSubcommand::Generate(args)) => handle_generate(args, repo_root),
        Some(ConfigSubcommand::Default(args)) => handle_default(args),
        Some(ConfigSubcommand::Init(args)) => {
            deprecation.warn_subject("config init", "mf init");
            handle_init(args)
        }
        Some(ConfigSubcommand::Terminal) => handle_terminal(format),
    }
}

fn handle_schema(args: ConfigSchemaArgs) -> Result<CommandOutcome> {
    let output = service::config::schema_output(&args.output_format)?;
    Ok(CommandOutcome::Raw(output, None))
}

fn handle_show(args: ConfigShowArgs, repo_root: Option<&PathBuf>, global_format: Format) -> Result<CommandOutcome> {
    let cwd = std::env::current_dir()
        .map_err(|e| crate::error::MfError::usage(format!("failed to get current directory: {e}"), None))?;
    // When global format is JSON, force internal JSON to return structured object
    // so data is a JSON object, not an embedded YAML/JSON string (FR-066 / FR-070).
    let internal_format = if global_format == Format::Json { "json" } else { args.output_format.as_str() };
    let output = service::config::show_effective(&cwd, repo_root.map(|p| p.as_path()), internal_format)?;
    if global_format == Format::Json {
        let data: serde_json::Value = serde_json::from_str(&output).map_err(crate::error::MfError::Json)?;
        Ok(CommandOutcome::Success(data, Vec::new(), None))
    } else {
        Ok(CommandOutcome::Raw(output, None))
    }
}

// ── B2 thin alias: config generate (show + write to file) ───────────────────

fn handle_generate(args: ConfigGenerateArgs, repo_root: Option<&PathBuf>) -> Result<CommandOutcome> {
    use std::io::Write;

    let cwd = std::env::current_dir()
        .map_err(|e| crate::error::MfError::usage(format!("failed to get current directory: {e}"), None))?;
    let output = service::config::show_effective(&cwd, repo_root.map(|p| p.as_path()), &args.output_format)?;
    let mut file = std::fs::File::create(&args.output).map_err(crate::error::MfError::Io)?;
    file.write_all(output.as_bytes()).map_err(crate::error::MfError::Io)?;
    Ok(CommandOutcome::Success(
        serde_json::json!({ "path": args.output.to_string_lossy().to_string() }),
        Vec::new(),
        None,
    ))
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
        Vec::new(),
        None,
    ))
}

fn handle_terminal(format: Format) -> Result<CommandOutcome> {
    let profile = build_profile();
    let output_format = match format {
        Format::Text => OutputFormat::Text,
        Format::Json => OutputFormat::Json,
    };
    let policy = build_policy(&profile, output_format);
    let environment = build_environment_summary();

    let checks = build_diagnostic_checks(&profile);
    let recommendations = build_recommendations(&profile);

    let report = CapabilityDiagnosticReport { profile, policy, environment, checks, recommendations };

    match format {
        Format::Text => {
            let text = render_terminal_text(&report);
            Ok(CommandOutcome::Raw(text, None))
        }
        Format::Json => {
            let data = serde_json::to_value(&report).map_err(crate::error::MfError::Json)?;
            Ok(CommandOutcome::Success(data, Vec::new(), None))
        }
    }
}

fn build_diagnostic_checks(profile: &crate::model::terminal::TerminalCapabilityProfile) -> Vec<DiagnosticCheck> {
    let mut checks = Vec::new();

    checks.push(DiagnosticCheck {
        name: "stdout_tty".into(),
        status: if profile.stdout_is_tty { "pass".into() } else { "fail".into() },
        detail: Some(if profile.stdout_is_tty {
            "stdout is interactive".into()
        } else {
            "stdout is not a terminal".into()
        }),
    });

    checks.push(DiagnosticCheck {
        name: "truecolor".into(),
        status: if profile.truecolor { "pass".into() } else { "info".into() },
        detail: if profile.truecolor {
            Some("truecolor support detected".into())
        } else {
            Some("no truecolor evidence found".into())
        },
    });

    checks.push(DiagnosticCheck {
        name: "hyperlinks".into(),
        status: if profile.hyperlinks { "pass".into() } else { "info".into() },
        detail: if profile.hyperlinks {
            Some("OSC 8 hyperlinks supported".into())
        } else {
            Some("OSC 8 hyperlinks not detected".into())
        },
    });

    checks
}

fn build_recommendations(profile: &crate::model::terminal::TerminalCapabilityProfile) -> Vec<String> {
    let mut recs = Vec::new();
    if let Some(ref reason) = profile.fallback_reason {
        recs.push(format!("Fallback active: {reason}"));
    }
    if !profile.stdout_is_tty {
        recs.push("Run with MF_FORCE_TTY=1 to test terminal behavior in pipes".into());
    }
    if !profile.truecolor && profile.stdout_is_tty {
        recs.push("Set COLORTERM=truecolor for richer color output".into());
    }
    recs
}

fn render_terminal_text(report: &CapabilityDiagnosticReport) -> String {
    let p = &report.profile;
    let mut out = String::new();

    out.push_str(&format!("Terminal: {}\n", p.term.as_deref().unwrap_or("unknown")));
    out.push_str(&format!("TTY: {}\n", if p.stdout_is_tty { "yes" } else { "no" }));
    out.push_str(&format!(
        "Color: {}\n",
        match p.color_mode {
            crate::model::terminal::ColorMode::None => "none",
            crate::model::terminal::ColorMode::Ansi16 => "16-color",
            crate::model::terminal::ColorMode::Ansi256 => "256-color",
            crate::model::terminal::ColorMode::Truecolor => "truecolor",
        }
    ));
    out.push_str(&format!("Hyperlinks: {}\n", if p.hyperlinks { "yes" } else { "no" }));
    out.push_str(&format!("Fallback: {}\n", p.fallback_reason.as_deref().unwrap_or("-")));

    out
}
