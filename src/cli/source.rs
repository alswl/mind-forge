use std::path::{Path, PathBuf};

use clap::{Args, Subcommand, ValueEnum};
use serde::Serialize;

use crate::cli::deprecation::DeprecationContext;
use crate::cli::CommandOutcome;
use crate::defaults;
use crate::error::{MfError, Result};
use crate::model::source::{FileKind, SourceKind};
use crate::output::Format;
use crate::service::source::InputForm;
use crate::service::{identity, source as svc_source, util as svc_util};

#[derive(Debug, Clone, Args)]
pub struct SourceCmd {
    #[command(subcommand)]
    pub command: Option<SourceSubcommand>,
}

#[derive(Debug, Clone, Subcommand)]
pub enum SourceSubcommand {
    #[command(about = "List sources", visible_alias = "ls")]
    List(SourceListArgs),
    #[command(about = "Add a source")]
    Add(SourceAddArgs),
    #[command(about = "Update a source")]
    Update(SourceUpdateArgs),
    #[command(about = "Index sources")]
    Index(SourceIndexArgs),
    #[command(about = "Remove a source", visible_alias = "rm")]
    Remove(SourceRemoveArgs),
    #[command(about = "Rename a source")]
    Rename(SourceRenameArgs),
    #[command(about = "Clean source index")]
    Clean(SourceCleanArgs),
}

// ---------------------------------------------------------------------------
// CliSourceKind — CLI enum mapping to FileKind (mf primary)
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
    pub fn resolve(self, form: &InputForm) -> Result<FileKind> {
        use CliSourceKind::*;
        use InputForm::*;
        match (self, form) {
            (Auto, Path) | (File, Path) => Ok(FileKind::File),
            (Auto, Url) | (Web, Url) => Ok(FileKind::Web),
            (Pdf, Path) => Ok(FileKind::Pdf),
            (Rss, Url) => Ok(FileKind::Rss),
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
// CliSourceKindType — CLI enum mapping to SourceKind (mind primary)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, ValueEnum, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CliSourceKindType {
    Yuque,
    Meeting,
    Misc,
}

impl From<CliSourceKindType> for SourceKind {
    fn from(k: CliSourceKindType) -> Self {
        match k {
            CliSourceKindType::Yuque => SourceKind::Yuque,
            CliSourceKindType::Meeting => SourceKind::Meeting,
            CliSourceKindType::Misc => SourceKind::Misc,
        }
    }
}

// ---------------------------------------------------------------------------
// T012: SourceAddArgs
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Args, Serialize)]
pub struct SourceAddArgs {
    pub input: String,
    #[arg(short = 'n', long)]
    pub name: Option<String>,
    /// File kind (mf primary). Use --source-kind for mind channel type.
    #[arg(long = "file-kind", value_enum)]
    pub file_kind: Option<CliSourceKind>,
    /// Source channel type (mind primary).
    #[arg(long = "source-kind", value_enum)]
    pub source_kind: Option<CliSourceKindType>,
    /// Deprecated: use --file-kind or --source-kind instead.
    #[arg(short = 't', long = "type", value_enum)]
    pub kind: Option<CliSourceKind>,
    #[arg(long)]
    pub link: bool,
    #[arg(short = 'f', long)]
    pub force: bool,
}

// ---------------------------------------------------------------------------
// T013: SourceListArgs
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Args, Serialize)]
pub struct SourceListArgs {
    #[arg(long)]
    pub filter: Option<String>,
    #[arg(short = 't', long = "type", value_enum)]
    pub kind: Option<CliSourceKind>,
}

// ---------------------------------------------------------------------------
// T014: SourceUpdateArgs
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Args, Serialize)]
pub struct SourceUpdateArgs {
    /// Source path (e.g. sources/meeting/notes.md) or name
    pub path: String,
    #[arg(long)]
    pub rename: Option<String>,
    #[arg(long)]
    pub url: Option<String>,
}

// ---------------------------------------------------------------------------
// T015: SourceRemoveArgs
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Args, Serialize)]
pub struct SourceRemoveArgs {
    /// Source path (e.g. sources/yuque/foo.md) or source name (deprecated)
    pub name_or_path: String,
    #[arg(long = "keep-file")]
    pub keep_file: bool,
    #[arg(short = 'f', long)]
    pub force: bool,
    #[arg(long = "dry-run")]
    pub dry_run: bool,
}

// ---------------------------------------------------------------------------
// T016: SourceIndexArgs / SourceCleanArgs
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Args, Serialize)]
pub struct SourceIndexArgs {
    #[arg(long = "dry-run")]
    pub dry_run: bool,
}

#[derive(Debug, Clone, Args, Serialize)]
pub struct SourceRenameArgs {
    /// Current source path or name
    pub old_path: String,
    /// New source path or name
    pub new_path: String,
    #[arg(short = 'f', long)]
    pub force: bool,
    #[arg(long = "dry-run")]
    pub dry_run: bool,
}

#[derive(Debug, Clone, Args, Serialize)]
pub struct SourceCleanArgs {
    #[arg(long = "dry-run")]
    pub dry_run: bool,
}

// ---------------------------------------------------------------------------
// T017: Dispatch — replaced by user story tasks
// ---------------------------------------------------------------------------

pub fn dispatch(
    command: SourceCmd,
    repo_root: Option<&PathBuf>,
    format: Format,
    project: Option<&str>,
    deprecation: &mut DeprecationContext,
) -> Result<CommandOutcome> {
    let root = repo_root.ok_or_else(MfError::not_in_mind_repo)?;
    let cwd = std::env::current_dir().map_err(MfError::Io)?;

    match command.command {
        None => Ok(CommandOutcome::GroupHelp("source")),
        Some(SourceSubcommand::Add(args)) => {
            // Deprecation warning for --type
            if args.kind.is_some() {
                deprecation.warn_subject("--type", "--file-kind or --source-kind");
            }
            handle_add(args, root, &cwd, format, project)
        }
        Some(SourceSubcommand::List(args)) => handle_list(args, root, &cwd, format, project),
        Some(SourceSubcommand::Update(args)) => handle_update(args, root, &cwd, format, project),
        Some(SourceSubcommand::Index(args)) => handle_index(args, root, &cwd, format, project),
        Some(SourceSubcommand::Remove(args)) => handle_remove(args, root, &cwd, format, project, deprecation),
        Some(SourceSubcommand::Rename(args)) => handle_rename(args, root, &cwd, format, project),
        Some(SourceSubcommand::Clean(args)) => handle_clean(args, root, &cwd, format, project),
    }
}

fn handle_list(
    args: SourceListArgs,
    root: &Path,
    cwd: &Path,
    format: Format,
    project: Option<&str>,
) -> Result<CommandOutcome> {
    let project_path = svc_util::resolve_project(root, project, cwd)?;

    // Resolve type filter (CliSourceKind → model FileKind; Auto is rejected)
    let type_filter = match args.kind {
        Some(CliSourceKind::Auto) => {
            return Err(MfError::usage(
                "--type auto is not valid for listing; specify a concrete type",
                Some("use --type pdf, file, rss, or web".to_string()),
            ));
        }
        Some(CliSourceKind::Pdf) => Some(FileKind::Pdf),
        Some(CliSourceKind::File) => Some(FileKind::File),
        Some(CliSourceKind::Rss) => Some(FileKind::Rss),
        Some(CliSourceKind::Web) => Some(FileKind::Web),
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

fn handle_update(
    args: SourceUpdateArgs,
    root: &Path,
    cwd: &Path,
    format: Format,
    project: Option<&str>,
) -> Result<CommandOutcome> {
    let project_path = svc_util::resolve_project(root, project, cwd)?;
    identity::validate_entity_path(&project_path, &args.path)?;

    let update_args =
        svc_source::UpdateArgs { name: &args.path, rename: args.rename.as_deref(), url: args.url.as_deref() };

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

fn handle_index(
    args: SourceIndexArgs,
    root: &Path,
    cwd: &Path,
    format: Format,
    project: Option<&str>,
) -> Result<CommandOutcome> {
    let project_path = svc_util::resolve_project(root, project, cwd)?;

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

fn handle_remove(
    args: SourceRemoveArgs,
    root: &Path,
    cwd: &Path,
    format: Format,
    project: Option<&str>,
    deprecation: &mut DeprecationContext,
) -> Result<CommandOutcome> {
    let is_path = args.name_or_path.contains('/') || args.name_or_path.starts_with(defaults::SOURCES_DIR);
    if !is_path {
        deprecation.warn_subject("positional NAME", "full PATH (e.g., sources/yuque/foo.md)");
    }
    let project_path = svc_util::resolve_project(root, project, cwd)?;
    identity::validate_entity_path(&project_path, &args.name_or_path)?;
    let report =
        svc_source::remove_source(&project_path, &args.name_or_path, args.keep_file, args.force, args.dry_run)?;

    match format {
        Format::Json => {
            let data = serde_json::to_value(&report).map_err(MfError::Json)?;
            Ok(CommandOutcome::Success(data, None))
        }
        Format::Text => {
            let prefix = if report.dry_run { "[dry-run] would remove" } else { "✓ removed" };
            let kind_str = report.source.kind.as_str();
            let mut lines = vec![format!("{prefix} source: {} ({kind_str})", report.source.name)];
            if report.file_deleted {
                if let Some(ref p) = report.source.path {
                    lines.push(format!("  deleted file: {p}"));
                }
            }
            Ok(CommandOutcome::Success(serde_json::Value::String(lines.join("\n")), None))
        }
    }
}

fn handle_clean(
    args: SourceCleanArgs,
    root: &Path,
    cwd: &Path,
    format: Format,
    project: Option<&str>,
) -> Result<CommandOutcome> {
    let project_path = svc_util::resolve_project(root, project, cwd)?;

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

// ── Handle: mf source rename ────────────────────────────────────────────────

fn handle_rename(
    args: SourceRenameArgs,
    root: &Path,
    cwd: &Path,
    format: Format,
    project: Option<&str>,
) -> Result<CommandOutcome> {
    let project_path = svc_util::resolve_project(root, project, cwd)?;
    identity::validate_entity_path(&project_path, &args.old_path)?;
    identity::validate_entity_path(&project_path, &args.new_path)?;
    let report = svc_source::rename_source(&project_path, &args.old_path, &args.new_path, args.force, args.dry_run)?;

    match format {
        Format::Json => {
            let data = serde_json::to_value(&report).map_err(MfError::Json)?;
            Ok(CommandOutcome::Success(data, None))
        }
        Format::Text => {
            let prefix = if report.dry_run { "[dry-run] would rename" } else { "✓ renamed" };
            let kind_str = report.before.file_kind.as_str();
            let msg = format!("{} source: {} → {} ({})", prefix, report.before.name, report.after.name, kind_str);
            Ok(CommandOutcome::Success(serde_json::Value::String(msg), None))
        }
    }
}

// ---------------------------------------------------------------------------
// Handle: mf source add
// ---------------------------------------------------------------------------

fn handle_add(
    args: SourceAddArgs,
    root: &Path,
    cwd: &Path,
    format: Format,
    project: Option<&str>,
) -> Result<CommandOutcome> {
    let project_path = svc_util::resolve_project(root, project, cwd)?;

    let input_form = svc_source::classify_input(&args.input);

    // Resolve kind: prefer --file-kind or --source-kind, fall back to deprecated --type
    let kind = if let Some(fk) = args.file_kind {
        let model_kind = fk.resolve(&input_form)?;
        Some(model_kind)
    } else if args.source_kind.is_some() {
        // --source-kind resolves to an auto-inferred FileKind based on input form
        // For URL inputs: Web; for Path inputs: File (store, not infer)
        let model_kind = match &input_form {
            svc_source::InputForm::Url => FileKind::Web,
            svc_source::InputForm::Path => FileKind::File,
        };
        Some(model_kind)
    } else if let Some(k) = args.kind {
        // Deprecated --type fallback
        let model_kind = k.resolve(&input_form)?;
        Some(model_kind)
    } else {
        None
    };

    // Resolve source_kind
    let source_kind = args.source_kind.map(SourceKind::from);

    let add_args = svc_source::AddArgs {
        input: &args.input,
        name: args.name.as_deref(),
        kind,
        source_kind,
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
