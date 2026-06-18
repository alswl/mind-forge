use std::path::{Path, PathBuf};

use clap::{Args, Subcommand, ValueEnum};
use serde::Serialize;

use crate::cli::deprecation::DeprecationContext;
use crate::cli::shared_flags::NoHeadersFlag;
use crate::cli::shared_flags::NoTruncFlag;
use crate::cli::{merge_warnings, CommandOutcome};
use crate::defaults;
use crate::error::{MfError, Result};
use crate::model::source::{FileKind, SourceKind};
use crate::model::Resource;
use crate::output::confirm::{require_confirmation, ConfirmArgs};
use crate::output::list::{json_collection, render_text, ListCell, ListOpts, ListRow, ListView};
use crate::output::show::{json_envelope, render_text as render_show_text, ShowBlock, ShowField, ShowOpts, ShowValue};
use crate::output::verb::{json_envelope as verb_json, render_text as verb_text, Verb, VerbOpts, VerbResult};
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
    #[command(about = "Create a source")]
    New(SourceAddArgs),
    #[command(about = "Add a source", hide = true)]
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
    #[command(about = "Show source details")]
    Show(SourceShowArgs),
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
    #[arg(long = "dry-run")]
    pub dry_run: bool,
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
    #[command(flatten)]
    pub no_headers: NoHeadersFlag,
    #[command(flatten)]
    pub no_trunc: NoTruncFlag,
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
    #[arg(long = "dry-run")]
    pub dry_run: bool,
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
    #[arg(short = 'y', long)]
    pub yes: bool,
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

#[derive(Debug, Clone, Args, Serialize)]
pub struct SourceShowArgs {
    /// Source path (e.g. sources/meeting/notes.md) or name
    pub path: String,
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
        Some(SourceSubcommand::New(args)) => handle_add(args, root, &cwd, format, project),
        Some(SourceSubcommand::Add(args)) => {
            // Deprecation warning for --type
            if args.kind.is_some() {
                deprecation.warn_subject("--type", "--file-kind or --source-kind");
            }
            let mut warnings: Vec<String> = Vec::new();
            crate::output::warning::emit_warning(
                &format!("`source add` is deprecated; use `mf source new {}` instead.", args.input),
                &mut warnings,
            );
            let outcome = handle_add(args, root, &cwd, format, project)?;
            Ok(merge_warnings(outcome, warnings))
        }
        Some(SourceSubcommand::List(args)) => handle_list(args, root, &cwd, format, project),
        Some(SourceSubcommand::Update(args)) => handle_update(args, root, &cwd, format, project),
        Some(SourceSubcommand::Index(args)) => handle_index(args, root, &cwd, format, project),
        Some(SourceSubcommand::Remove(args)) => handle_remove(args, root, &cwd, format, project, deprecation),
        Some(SourceSubcommand::Rename(args)) => handle_rename(args, root, &cwd, format, project),
        Some(SourceSubcommand::Clean(args)) => handle_clean(args, root, &cwd, format, project),
        Some(SourceSubcommand::Show(args)) => handle_source_show(args, root, &cwd, format, project),
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

    let opts = ListOpts::from_flags(args.no_headers.no_headers, args.no_trunc.no_trunc)
        .with_repo_root(Some(project_path.to_path_buf()));

    match format {
        Format::Json => {
            let items: Vec<serde_json::Value> = sources
                .iter()
                .map(|s| {
                    let mut v = serde_json::to_value(s).map_err(MfError::Json)?;
                    if let Some(obj) = v.as_object_mut() {
                        obj.insert("identity".to_string(), serde_json::Value::String(s.identity()));
                    }
                    Ok(v)
                })
                .collect::<Result<Vec<_>>>()?;
            Ok(CommandOutcome::Success(json_collection("sources", items), Vec::new(), None))
        }
        Format::Text => {
            let mut rows = Vec::with_capacity(sources.len());
            for s in &sources {
                let location = s.path.as_deref().or(s.url.as_deref()).unwrap_or("-").to_string();
                rows.push(ListRow {
                    cells: vec![
                        ListCell::Text(s.name.clone()),
                        ListCell::Text(s.kind.as_str().to_string()),
                        ListCell::Path(location),
                    ],
                });
            }
            let view = ListView { headers: &["NAME", "TYPE", "LOCATION"], rows, plural_noun: "sources" };
            Ok(CommandOutcome::Raw(render_text(&view, &opts), None))
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

    if args.dry_run {
        let mut changes = serde_json::Map::new();
        if let Some(ref rename) = args.rename {
            changes.insert("rename".to_string(), serde_json::json!({"from": args.path, "to": rename}));
        }
        if let Some(ref url) = args.url {
            changes.insert("url".to_string(), serde_json::json!({"to": url}));
        }
        let identity = args.rename.as_ref().unwrap_or(&args.path).clone();
        let old_identity = args.rename.as_ref().map(|_| args.path.clone());
        let result = VerbResult {
            verb: Verb::Update,
            kind: "source",
            identity,
            old_identity,
            path: None,
            dry_run: true,
            details: serde_json::json!({"changes": changes}),
        };
        return match format {
            Format::Json => Ok(CommandOutcome::Success(verb_json(&result), Vec::new(), None)),
            Format::Text => Ok(CommandOutcome::Success(
                serde_json::Value::String(verb_text(&result, &VerbOpts::from_repo_root(Some(project_path.as_path())))),
                Vec::new(),
                None,
            )),
        };
    }

    let update_args =
        svc_source::UpdateArgs { name: &args.path, rename: args.rename.as_deref(), url: args.url.as_deref() };

    let source = svc_source::update(&project_path, &update_args)?;

    let mut changes = serde_json::Map::new();
    if let Some(ref rename) = args.rename {
        changes.insert("rename".to_string(), serde_json::json!({"from": args.path, "to": rename}));
    }
    if let Some(ref url) = args.url {
        changes.insert("url".to_string(), serde_json::json!({"to": url}));
    }

    let identity = args.rename.as_ref().unwrap_or(&args.path).clone();
    let old_identity = args.rename.as_ref().map(|_| args.path.clone());
    let result = VerbResult {
        verb: Verb::Update,
        kind: "source",
        identity,
        old_identity,
        path: None,
        dry_run: false,
        details: serde_json::json!({"changes": changes, "source": source}),
    };
    match format {
        Format::Json => Ok(CommandOutcome::Success(verb_json(&result), Vec::new(), None)),
        Format::Text => Ok(CommandOutcome::Success(
            serde_json::Value::String(verb_text(&result, &VerbOpts::from_repo_root(Some(project_path.as_path())))),
            Vec::new(),
            None,
        )),
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

    let scanned_count = report.added.len() + report.removed.len() + report.kept_count as usize;

    match format {
        Format::Json => {
            let data = serde_json::json!({
                "kind": "source",
                "added": report.added,
                "removed": report.removed,
                "kept_count": report.kept_count,
                "scanned_count": scanned_count,
                "dry_run": args.dry_run,
            });
            Ok(CommandOutcome::Success(data, Vec::new(), None))
        }
        Format::Text => {
            let details = serde_json::json!({
                "added": report.added,
                "removed": report.removed,
                "kept_count": report.kept_count,
                "scanned_count": scanned_count,
            });
            let result = VerbResult {
                verb: Verb::Index,
                kind: "source",
                identity: String::new(),
                old_identity: None,
                path: None,
                dry_run: args.dry_run,
                details,
            };
            Ok(CommandOutcome::Success(
                serde_json::Value::String(verb_text(&result, &VerbOpts::from_repo_root(Some(project_path.as_path())))),
                Vec::new(),
                None,
            ))
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

    require_confirmation(&ConfirmArgs {
        verb_label: "removal",
        kind: "source",
        identity: &args.name_or_path,
        yes: args.yes,
        force: args.force,
    })?;

    let report =
        svc_source::remove_source(&project_path, &args.name_or_path, args.keep_file, args.force, args.dry_run)?;

    let result = VerbResult {
        verb: Verb::Remove,
        kind: "source",
        identity: report.source.name.clone(),
        old_identity: None,
        path: report.source.path.clone(),
        dry_run: args.dry_run,
        details: serde_json::json!({"removed": true}),
    };
    match format {
        Format::Json => Ok(CommandOutcome::Success(verb_json(&result), Vec::new(), None)),
        Format::Text => Ok(CommandOutcome::Success(
            serde_json::Value::String(verb_text(&result, &VerbOpts::from_repo_root(Some(project_path.as_path())))),
            Vec::new(),
            None,
        )),
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
            Ok(CommandOutcome::Success(data, Vec::new(), None))
        }
        Format::Text => {
            if report.removed.is_empty() {
                return Ok(CommandOutcome::Success(
                    serde_json::Value::String("No dirty sources.".to_string()),
                    Vec::new(),
                    None,
                ));
            }
            let mut lines = Vec::new();
            let prefix = if args.dry_run { "[dry-run] " } else { "" };

            for entry in &report.removed {
                let kind_str = entry.kind.as_str();
                lines.push(format!("{}- removed: {} ({})", prefix, entry.name, kind_str));
            }
            lines.push(format!("{}kept: {} entries", prefix, report.kept_count));

            let output = lines.join("\n");
            Ok(CommandOutcome::Success(serde_json::Value::String(output), Vec::new(), None))
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

    if args.dry_run {
        let result = VerbResult {
            verb: Verb::Rename,
            kind: "source",
            identity: args.new_path.clone(),
            old_identity: Some(args.old_path.clone()),
            path: None,
            dry_run: true,
            details: serde_json::json!({}),
        };
        return match format {
            Format::Json => Ok(CommandOutcome::Success(verb_json(&result), Vec::new(), None)),
            Format::Text => Ok(CommandOutcome::Success(
                serde_json::Value::String(verb_text(&result, &VerbOpts::from_repo_root(Some(project_path.as_path())))),
                Vec::new(),
                None,
            )),
        };
    }

    let report = svc_source::rename_source(&project_path, &args.old_path, &args.new_path, args.force, false)?;

    let result = VerbResult {
        verb: Verb::Rename,
        kind: "source",
        identity: report.after.name.clone(),
        old_identity: Some(report.before.name.clone()),
        path: report.after.path.clone(),
        dry_run: false,
        details: serde_json::json!({}),
    };
    match format {
        Format::Json => Ok(CommandOutcome::Success(verb_json(&result), Vec::new(), None)),
        Format::Text => Ok(CommandOutcome::Success(
            serde_json::Value::String(verb_text(&result, &VerbOpts::from_repo_root(Some(project_path.as_path())))),
            Vec::new(),
            None,
        )),
    }
}

// ---------------------------------------------------------------------------
// Handle: mf source show
// ---------------------------------------------------------------------------

fn handle_source_show(
    args: SourceShowArgs,
    root: &Path,
    cwd: &Path,
    format: Format,
    project: Option<&str>,
) -> Result<CommandOutcome> {
    let project_path = svc_util::resolve_project(root, project, cwd)?;
    identity::validate_entity_path(&project_path, &args.path)?;
    let sources = svc_source::list(&project_path, None, None)?;

    let resolved = sources
        .iter()
        .find(|s| s.path.as_deref() == Some(&args.path))
        .or_else(|| sources.iter().find(|s| s.name.eq_ignore_ascii_case(&args.path)));

    match resolved {
        None => Err(MfError::usage(
            format!("source '{}' not found", args.path),
            Some("use `mf source list` to see available sources".to_string()),
        )),
        Some(source) => {
            let file_kind = source.kind.as_str().to_string();
            let type_str =
                if let Some(ref sk) = source.source_kind { format!("{} ({})", file_kind, sk) } else { file_kind };
            let location = source.path.as_deref().or(source.url.as_deref()).unwrap_or("-").to_string();

            let block = ShowBlock {
                kind: "source",
                identity: source.name.clone(),
                fields: vec![
                    ShowField { label: "Name", value: ShowValue::Text(source.name.clone()) },
                    ShowField { label: "Type", value: ShowValue::Text(type_str) },
                    ShowField { label: "Location", value: ShowValue::Path(location) },
                    ShowField { label: "Added", value: ShowValue::Text(source.added_at.clone()) },
                ],
                sections: vec![],
            };

            match format {
                Format::Json => {
                    let source_json = serde_json::to_value(source).map_err(MfError::Json)?;
                    let extra = source_json.as_object().cloned().unwrap_or_default();
                    Ok(CommandOutcome::Success(json_envelope(&block, extra), Vec::new(), None))
                }
                Format::Text => Ok(CommandOutcome::Raw(
                    render_show_text(&block, &ShowOpts::from_repo_root(Some(project_path.as_path()))),
                    None,
                )),
            }
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
        let model_kind = match &input_form {
            svc_source::InputForm::Url => FileKind::Web,
            svc_source::InputForm::Path => FileKind::File,
        };
        Some(model_kind)
    } else if let Some(k) = args.kind {
        let model_kind = k.resolve(&input_form)?;
        Some(model_kind)
    } else {
        None
    };

    // Resolve source_kind
    let source_kind = args.source_kind.map(SourceKind::from);

    if args.dry_run {
        let name = args.name.as_deref().unwrap_or_else(|| {
            let p = std::path::Path::new(&args.input);
            p.file_stem().and_then(|s| s.to_str()).unwrap_or("unknown")
        });
        let result = VerbResult {
            verb: Verb::Add,
            kind: "source",
            identity: name.to_string(),
            old_identity: None,
            path: Some(name.to_string()),
            dry_run: true,
            details: serde_json::json!({"input": args.input}),
        };
        return match format {
            Format::Json => Ok(CommandOutcome::Success(verb_json(&result), Vec::new(), None)),
            Format::Text => Ok(CommandOutcome::Success(
                serde_json::Value::String(verb_text(&result, &VerbOpts::from_repo_root(Some(project_path.as_path())))),
                Vec::new(),
                None,
            )),
        };
    }

    let add_args = svc_source::AddArgs {
        input: &args.input,
        name: args.name.as_deref(),
        kind,
        source_kind,
        link: args.link,
        force: args.force,
    };

    let outcome = svc_source::add(&project_path, cwd, &add_args)?;

    let result = VerbResult {
        verb: Verb::Add,
        kind: "source",
        identity: outcome.source.name.clone(),
        old_identity: None,
        path: outcome.source.path.clone(),
        dry_run: false,
        details: serde_json::json!({
            "name": outcome.source.name,
            "type": outcome.source.kind.as_str(),
            "url": outcome.source.url,
            "path": outcome.source.path,
            "added_at": outcome.source.added_at,
            "updated_at": outcome.source.updated_at,
            "mode": match outcome.mode {
                svc_source::AddMode::Copy => "copy",
                svc_source::AddMode::Link => "link",
                svc_source::AddMode::Url => "url",
            },
            "replaced": outcome.replaced,
        }),
    };
    match format {
        Format::Json => Ok(CommandOutcome::Success(verb_json(&result), Vec::new(), None)),
        Format::Text => Ok(CommandOutcome::Success(
            serde_json::Value::String(verb_text(&result, &VerbOpts::from_repo_root(Some(project_path.as_path())))),
            Vec::new(),
            None,
        )),
    }
}
