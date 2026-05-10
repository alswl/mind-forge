use std::path::{Path, PathBuf};

use clap::{Args, Subcommand, ValueEnum};
use serde::Serialize;

use crate::cli::CommandOutcome;
use crate::error::{MfError, Result};
use crate::model::source::SourceKind as ModelKind;
use crate::output::Format;
use crate::service::source::InputForm;
use crate::service::{source as svc_source, util as svc_util};

#[derive(Debug, Clone, Args)]
pub struct SourceCmd {
    #[command(subcommand)]
    pub command: Option<SourceSubcommand>,
}

#[derive(Debug, Clone, Subcommand)]
pub enum SourceSubcommand {
    #[command(about = "List sources")]
    List(SourceListArgs),
    #[command(about = "Add a source")]
    Add(SourceAddArgs),
    #[command(about = "Update a source")]
    Update(SourceUpdateArgs),
    #[command(about = "Index sources")]
    Index(SourceIndexArgs),
    #[command(about = "Remove a source")]
    Remove(SourceRemoveArgs),
    #[command(about = "Clean source index")]
    Clean(SourceCleanArgs),
}

// ---------------------------------------------------------------------------
// T011: CliSourceKind — CLI-only enum (adds `Auto` + `Pdf`)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, ValueEnum, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CliSourceKind {
    Auto,
    Pdf,
    File,
    Rss,
    Web,
}

impl CliSourceKind {
    /// Resolve `Auto` to a concrete `ModelKind` based on input form.
    /// When not `Auto`, validates that the explicit kind is compatible with the input form.
    pub fn resolve(self, form: &InputForm) -> Result<ModelKind> {
        use CliSourceKind::*;
        use InputForm::*;
        match (self, form) {
            (Auto, Path) | (File, Path) => Ok(ModelKind::File),
            (Auto, Url) | (Web, Url) => Ok(ModelKind::Web),
            (Pdf, Path) => Ok(ModelKind::Pdf),
            (Rss, Url) => Ok(ModelKind::Rss),
            (Pdf, Url) | (File, Url) => Err(MfError::usage(
                "cannot use --type pdf or --type file with a URL input",
                Some("download the file first, then add the local path".into()),
            )),
            (Rss, Path) | (Web, Path) => Err(MfError::usage(
                "cannot use --type rss or --type web with a local file input",
                Some("pass an http(s):// URL".into()),
            )),
        }
    }
}

// ---------------------------------------------------------------------------
// T012: SourceAddArgs
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Args, Serialize)]
pub struct SourceAddArgs {
    pub input: String,
    #[arg(long)]
    pub name: Option<String>,
    #[arg(long = "type", value_enum, default_value_t = CliSourceKind::Auto)]
    pub kind: CliSourceKind,
    #[arg(long)]
    pub link: bool,
    #[arg(long)]
    pub force: bool,
    #[arg(long)]
    pub project: Option<String>,
}

// ---------------------------------------------------------------------------
// T013: SourceListArgs
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Args, Serialize)]
pub struct SourceListArgs {
    #[arg(long)]
    pub filter: Option<String>,
    #[arg(long = "type", value_enum)]
    pub kind: Option<CliSourceKind>,
    #[arg(long)]
    pub project: Option<String>,
}

// ---------------------------------------------------------------------------
// T014: SourceUpdateArgs
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Args, Serialize)]
pub struct SourceUpdateArgs {
    pub name: String,
    #[arg(long)]
    pub rename: Option<String>,
    #[arg(long)]
    pub url: Option<String>,
    #[arg(long)]
    pub project: Option<String>,
}

// ---------------------------------------------------------------------------
// T015: SourceRemoveArgs
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Args, Serialize)]
pub struct SourceRemoveArgs {
    pub name: String,
    #[arg(long = "keep-file")]
    pub keep_file: bool,
    #[arg(long)]
    pub project: Option<String>,
}

// ---------------------------------------------------------------------------
// T016: SourceIndexArgs / SourceCleanArgs
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Args, Serialize)]
pub struct SourceIndexArgs {
    #[arg(long = "dry-run")]
    pub dry_run: bool,
    #[arg(long)]
    pub project: Option<String>,
}

#[derive(Debug, Clone, Args, Serialize)]
pub struct SourceCleanArgs {
    #[arg(long = "dry-run")]
    pub dry_run: bool,
    #[arg(long)]
    pub project: Option<String>,
}

// ---------------------------------------------------------------------------
// T017: Dispatch — replaced by user story tasks
// ---------------------------------------------------------------------------

pub fn dispatch(command: SourceCmd, repo_root: Option<&PathBuf>, format: Format) -> Result<CommandOutcome> {
    let root = repo_root.ok_or_else(MfError::not_in_mind_repo)?;
    let cwd = std::env::current_dir().map_err(MfError::Io)?;

    match command.command {
        None => Ok(CommandOutcome::GroupHelp("source")),
        Some(SourceSubcommand::Add(args)) => handle_add(args, root, &cwd, format),
        Some(SourceSubcommand::List(args)) => handle_list(args, root, &cwd, format),
        Some(SourceSubcommand::Update(args)) => handle_update(args, root, &cwd, format),
        Some(SourceSubcommand::Index(args)) => handle_index(args, root, &cwd, format),
        Some(SourceSubcommand::Remove(args)) => handle_remove(args, root, &cwd, format),
        Some(SourceSubcommand::Clean(args)) => handle_clean(args, root, &cwd, format),
    }
}

fn handle_list(args: SourceListArgs, root: &Path, cwd: &Path, format: Format) -> Result<CommandOutcome> {
    let project_path = svc_util::resolve_project(root, args.project.as_deref(), cwd)?;

    // Resolve type filter (CliSourceKind → model SourceKind; Auto is rejected)
    let type_filter = match args.kind {
        Some(CliSourceKind::Auto) => {
            return Err(MfError::usage(
                "--type auto is not valid for listing; specify a concrete type",
                Some("use --type pdf, file, rss, or web".to_string()),
            ));
        }
        Some(CliSourceKind::Pdf) => Some(ModelKind::Pdf),
        Some(CliSourceKind::File) => Some(ModelKind::File),
        Some(CliSourceKind::Rss) => Some(ModelKind::Rss),
        Some(CliSourceKind::Web) => Some(ModelKind::Web),
        None => None,
    };

    let sources = svc_source::list(&project_path, args.filter.as_deref(), type_filter)?;

    match format {
        Format::Json => {
            let data = serde_json::to_value(&sources).map_err(MfError::Json)?;
            Ok(CommandOutcome::Success(data, None))
        }
        Format::Text => {
            if sources.is_empty() {
                return Ok(CommandOutcome::Success(serde_json::Value::String("No sources found.".to_string()), None));
            }
            let mut lines = Vec::new();
            lines.push(format!("{:<24} {:<8} LOCATION", "NAME", "TYPE"));
            for s in &sources {
                let kind_str = s.kind.as_str();
                let location = s.path.as_deref().or(s.url.as_deref()).unwrap_or("-");
                lines.push(format!("{:<24} {:<8} {}", s.name, kind_str, location));
            }
            Ok(CommandOutcome::Raw(lines.join("\n"), None))
        }
    }
}

fn handle_update(args: SourceUpdateArgs, root: &Path, cwd: &Path, format: Format) -> Result<CommandOutcome> {
    let project_path = svc_util::resolve_project(root, args.project.as_deref(), cwd)?;

    let update_args =
        svc_source::UpdateArgs { name: &args.name, rename: args.rename.as_deref(), url: args.url.as_deref() };

    let source = svc_source::update(&project_path, &update_args)?;

    match format {
        Format::Json => {
            let data = serde_json::to_value(&source).map_err(MfError::Json)?;
            Ok(CommandOutcome::Success(data, None))
        }
        Format::Text => {
            let kind_str = source.kind.as_str();
            let msg = format!("✓ updated source: {} ({kind_str})", source.name);
            Ok(CommandOutcome::Success(serde_json::Value::String(msg), None))
        }
    }
}

fn handle_index(args: SourceIndexArgs, root: &Path, cwd: &Path, format: Format) -> Result<CommandOutcome> {
    let project_path = svc_util::resolve_project(root, args.project.as_deref(), cwd)?;

    let report = svc_source::reconcile(&project_path, args.dry_run)?;

    match format {
        Format::Json => {
            let data = serde_json::to_value(&report).map_err(MfError::Json)?;
            Ok(CommandOutcome::Success(data, None))
        }
        Format::Text => {
            let mut lines = Vec::new();
            let prefix = if args.dry_run { "[dry-run] " } else { "" };

            for entry in &report.added {
                let kind_str = entry.kind.as_str();
                lines.push(format!("{}+ added: {} ({})", prefix, entry.name, kind_str));
            }
            for entry in &report.removed {
                let kind_str = entry.kind.as_str();
                lines.push(format!("{}- removed: {} ({})", prefix, entry.name, kind_str));
            }
            lines.push(format!("{}kept: {} entries", prefix, report.kept_count));

            let output = lines.join("\n");
            Ok(CommandOutcome::Success(serde_json::Value::String(output), None))
        }
    }
}

fn handle_remove(args: SourceRemoveArgs, root: &Path, cwd: &Path, format: Format) -> Result<CommandOutcome> {
    let project_path = svc_util::resolve_project(root, args.project.as_deref(), cwd)?;

    let report = svc_source::remove(&project_path, &args.name, args.keep_file)?;

    match format {
        Format::Json => {
            let data = serde_json::to_value(&report).map_err(MfError::Json)?;
            Ok(CommandOutcome::Success(data, None))
        }
        Format::Text => {
            let kind_str = report.source.kind.as_str();

            let mut lines = vec![format!("✓ removed source: {} ({kind_str})", report.source.name)];
            if report.file_deleted {
                if let Some(ref p) = report.source.path {
                    lines.push(format!("  deleted file: {p}"));
                }
            } else if matches!(report.source.kind, ModelKind::Pdf | ModelKind::File) {
                lines.push("  kept file (already missing or --keep-file)".to_string());
            } else {
                lines.push("  (URL source, no file to delete)".to_string());
            }

            Ok(CommandOutcome::Success(serde_json::Value::String(lines.join("\n")), None))
        }
    }
}

fn handle_clean(args: SourceCleanArgs, root: &Path, cwd: &Path, format: Format) -> Result<CommandOutcome> {
    let project_path = svc_util::resolve_project(root, args.project.as_deref(), cwd)?;

    let report = svc_source::clean(&project_path, args.dry_run)?;

    match format {
        Format::Json => {
            let data = serde_json::to_value(&report).map_err(MfError::Json)?;
            Ok(CommandOutcome::Success(data, None))
        }
        Format::Text => {
            if report.removed.is_empty() {
                return Ok(CommandOutcome::Success(serde_json::Value::String("No dirty sources.".to_string()), None));
            }
            let mut lines = Vec::new();
            let prefix = if args.dry_run { "[dry-run] " } else { "" };

            for entry in &report.removed {
                let kind_str = entry.kind.as_str();
                lines.push(format!("{}- removed: {} ({})", prefix, entry.name, kind_str));
            }
            lines.push(format!("{}kept: {} entries", prefix, report.kept_count));

            let output = lines.join("\n");
            Ok(CommandOutcome::Success(serde_json::Value::String(output), None))
        }
    }
}

// ---------------------------------------------------------------------------
// Handle: mf source add
// ---------------------------------------------------------------------------

fn handle_add(args: SourceAddArgs, root: &Path, cwd: &Path, format: Format) -> Result<CommandOutcome> {
    let project_path = svc_util::resolve_project(root, args.project.as_deref(), cwd)?;

    let input_form = svc_source::classify_input(&args.input);

    // Resolve kind
    let kind = match args.kind {
        CliSourceKind::Auto => None,
        explicit => {
            let model_kind = explicit.resolve(&input_form)?;
            Some(model_kind)
        }
    };

    let add_args = svc_source::AddArgs {
        input: &args.input,
        name: args.name.as_deref(),
        kind,
        link: args.link,
        force: args.force,
    };

    let outcome = svc_source::add(&project_path, cwd, &add_args)?;

    match format {
        Format::Json => {
            let mode_str = match outcome.mode {
                svc_source::AddMode::Copy => "copy",
                svc_source::AddMode::Link => "link",
                svc_source::AddMode::Url => "url",
            };
            let kind_str = outcome.source.kind.as_str();
            let data = serde_json::json!({
                "name": outcome.source.name,
                "type": kind_str,
                "url": outcome.source.url,
                "path": outcome.source.path,
                "added_at": outcome.source.added_at,
                "updated_at": outcome.source.updated_at,
                "mode": mode_str,
                "replaced": outcome.replaced,
            });
            Ok(CommandOutcome::Success(data, None))
        }
        Format::Text => {
            let location = outcome.source.path.as_deref().or(outcome.source.url.as_deref()).unwrap_or("unknown");
            let kind_str = outcome.source.kind.as_str();
            let prefix = if outcome.replaced { "replaced source" } else { "added source" };
            let msg = format!("✓ {prefix}: {} ({kind_str}, {location})", outcome.source.name);
            Ok(CommandOutcome::Success(serde_json::Value::String(msg), None))
        }
    }
}
