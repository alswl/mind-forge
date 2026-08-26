use clap::{Args, Subcommand, ValueEnum};
use serde::Serialize;

use crate::cli::CommandCtx;
use crate::cli::CommandOutcome;
use crate::cli::shared_flags::DryRunFlag;
use crate::cli::shared_flags::ForceFlag;
use crate::cli::shared_flags::NoHeadersFlag;
use crate::cli::shared_flags::NoTruncFlag;
use crate::cli::shared_flags::YesFlag;
use crate::defaults;
use crate::error::{MfError, Result};
use crate::model::Resource;
use crate::model::source::{FileKind, SourceKind};
use crate::output::Format;
use crate::output::confirm::{ConfirmArgs, require_confirmation};
use crate::output::list::{ListCell, ListOpts, ListRow, ListView, json_collection, render_text};
use crate::output::show::{ShowBlock, ShowField, ShowOpts, ShowValue, json_envelope, render_text as render_show_text};
use crate::output::verb::{Verb, VerbOpts, VerbResult, json_envelope as verb_json, render_text as verb_text};
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
    #[command(about = "Update a source")]
    Update(SourceUpdateArgs),
    #[command(about = "Index sources")]
    Index(SourceIndexArgs),
    #[command(about = "Remove a source", visible_alias = "rm")]
    Remove(SourceRemoveArgs),
    #[command(about = "Rename a source")]
    Rename(SourceRenameArgs),
    #[command(about = "Move a source to another project")]
    Move(SourceMoveArgs),
    #[command(about = "Clean source index")]
    Clean(SourceCleanArgs),
    #[command(about = "Show source details")]
    Show(SourceShowArgs),
    #[command(about = "Search sources across the repository")]
    Search(SourceSearchArgs),
    #[command(about = "Synchronize sources and article content")]
    Sync(crate::cli::source_rag::AdvancedSyncArgs),
    #[command(about = "Report source corpus status")]
    Status(crate::cli::source_rag::AdvancedStatusArgs),
    #[command(about = "Export the source corpus")]
    Export(crate::cli::source_rag::AdvancedExportArgs),
    #[command(about = "Import a source corpus bundle")]
    Import(crate::cli::source_rag::AdvancedImportArgs),
    #[command(about = "Trace source locations as Markdown links")]
    Trace(crate::cli::source_rag::AdvancedTraceArgs),
    #[command(about = "Maintain the source corpus")]
    #[command(subcommand)]
    Admin(SourceAdminCmd),
}

#[derive(Debug, Clone, Subcommand)]
pub enum SourceAdminCmd {
    Rebuild(crate::cli::source_rag::AdvancedRebuildArgs),
    Clear(crate::cli::source_rag::AdvancedClearArgs),
    Recover(crate::cli::source_rag::AdvancedRecoverArgs),
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
    /// Override the source name derived from the input
    #[arg(short = 'n', long)]
    pub name: Option<String>,
    /// File kind (mf primary). Use --source-kind for mind channel type.
    #[arg(long = "file-kind", value_enum)]
    pub file_kind: Option<CliSourceKind>,
    /// Source channel type (mind primary).
    #[arg(long = "source-kind", value_enum)]
    pub source_kind: Option<CliSourceKindType>,
    /// Create a symlink instead of copying a local file
    #[arg(long)]
    pub link: bool,
    /// Register a file already inside the project's sources directory without copying it
    #[arg(long = "register-only")]
    pub register_only: bool,
    /// Register only, without indexing the source into RAG (Lance backend). By
    /// default a new source is chunked and embedded so it is searchable at once.
    #[arg(long = "no-index")]
    pub no_index: bool,
    /// Originating article that introduced this source (project-relative path).
    /// Captured as authoritative import provenance (spec 071). Must stay within
    /// the project; an escaping path is a usage error.
    #[arg(long = "article")]
    pub article: Option<String>,
    #[command(flatten)]
    pub force: ForceFlag,
    #[command(flatten)]
    pub dry_run: DryRunFlag,
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
    #[command(flatten)]
    pub dry_run: DryRunFlag,
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
    #[command(flatten)]
    pub force: ForceFlag,
    #[command(flatten)]
    pub yes: YesFlag,
    #[command(flatten)]
    pub dry_run: DryRunFlag,
}

// ---------------------------------------------------------------------------
// T016: SourceIndexArgs / SourceCleanArgs
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Args, Serialize)]
pub struct SourceIndexArgs {
    #[command(flatten)]
    pub dry_run: DryRunFlag,
}

#[derive(Debug, Clone, Args, Serialize)]
pub struct SourceRenameArgs {
    /// Current source path or name
    pub old_path: String,
    /// New source path or name
    pub new_path: String,
    #[command(flatten)]
    pub force: ForceFlag,
    #[command(flatten)]
    pub dry_run: DryRunFlag,
}

#[derive(Debug, Clone, Args, Serialize)]
pub struct SourceMoveArgs {
    pub path: String,
    #[arg(long = "to-project")]
    pub to_project: String,
    #[command(flatten)]
    pub dry_run: DryRunFlag,
}

#[derive(Debug, Clone, Args, Serialize)]
pub struct SourceCleanArgs {
    #[command(flatten)]
    pub dry_run: DryRunFlag,
}

#[derive(Debug, Clone, Args, Serialize)]
pub struct SourceShowArgs {
    /// Source path (e.g. sources/meeting/notes.md) or name
    pub path: String,
}

#[derive(Debug, Clone, Args)]
pub struct SourceSearchArgs {
    /// Search query
    pub query: String,
    /// Search mode: basic (metadata), advanced (content), or both (fused)
    #[arg(long, value_enum)]
    pub mode: Option<SearchModeArg>,
    /// Limit search to a specific project
    #[arg(short = 'p', long)]
    pub project: Option<String>,
    /// Filter by file kind
    #[arg(long)]
    pub file_kind: Option<String>,
    /// Filter by source identity
    #[arg(long)]
    pub source: Option<String>,
    /// Filter by label, `key=value` (repeatable; all must match)
    #[arg(long = "label", value_name = "KEY=VALUE")]
    pub labels: Vec<String>,
    /// Search a specific content revision instead of the current version.
    /// Accepts an integer revision number, a date (2026-07-25), or a relative
    /// date (yesterday, "7 days ago").
    #[arg(long, value_name = "REV")]
    pub revision: Option<String>,
    /// Maximum results to return
    #[arg(long, default_value = "20")]
    pub limit: u32,
}

/// Canonical global RAG search surface. Retrieval mode is intentionally not a
/// user choice here; the active corpus determines the global search path.
#[derive(Debug, Clone, Args)]
pub struct GlobalSearchArgs {
    pub query: String,
    #[arg(short = 'p', long)]
    pub project: Option<String>,
    #[arg(long)]
    pub file_kind: Option<String>,
    #[arg(long)]
    pub source: Option<String>,
    #[arg(long = "label", value_name = "KEY=VALUE")]
    pub labels: Vec<String>,
    #[arg(long, value_name = "REV")]
    pub revision: Option<String>,
    #[arg(long, default_value = "20")]
    pub limit: u32,
}

impl From<GlobalSearchArgs> for SourceSearchArgs {
    fn from(args: GlobalSearchArgs) -> Self {
        Self {
            query: args.query,
            mode: None,
            project: args.project,
            file_kind: args.file_kind,
            source: args.source,
            labels: args.labels,
            revision: args.revision,
            limit: args.limit,
        }
    }
}

#[derive(Debug, Clone, clap::ValueEnum)]
pub enum SearchModeArg {
    Basic,
    Advanced,
    Both,
}

// ---------------------------------------------------------------------------
// T017: Dispatch — replaced by user story tasks
// ---------------------------------------------------------------------------

pub fn dispatch(command: SourceCmd, ctx: &mut CommandCtx) -> Result<CommandOutcome> {
    match command.command {
        None => Ok(CommandOutcome::GroupHelp("source")),
        Some(SourceSubcommand::New(args)) => handle_add(args, ctx),
        Some(SourceSubcommand::List(args)) => handle_list(args, ctx),
        Some(SourceSubcommand::Update(args)) => handle_update(args, ctx),
        Some(SourceSubcommand::Index(args)) => handle_index(args, ctx),
        Some(SourceSubcommand::Remove(args)) => handle_remove(args, ctx),
        Some(SourceSubcommand::Rename(args)) => handle_rename(args, ctx),
        Some(SourceSubcommand::Move(args)) => handle_move(args, ctx),
        Some(SourceSubcommand::Clean(args)) => handle_clean(args, ctx),
        Some(SourceSubcommand::Show(args)) => handle_source_show(args, ctx),
        Some(SourceSubcommand::Search(args)) => handle_search(args, ctx, false),
        Some(SourceSubcommand::Sync(args)) => crate::cli::source_rag::handle_sync(args, ctx),
        Some(SourceSubcommand::Status(args)) => crate::cli::source_rag::handle_status(args, ctx),
        Some(SourceSubcommand::Export(args)) => crate::cli::source_rag::handle_export(args, ctx),
        Some(SourceSubcommand::Import(args)) => crate::cli::source_rag::handle_import(args, ctx),
        Some(SourceSubcommand::Trace(args)) => crate::cli::source_rag::handle_trace(args, ctx),
        Some(SourceSubcommand::Admin(SourceAdminCmd::Rebuild(args))) => {
            crate::cli::source_rag::handle_rebuild(args, ctx)
        }
        Some(SourceSubcommand::Admin(SourceAdminCmd::Clear(args))) => crate::cli::source_rag::handle_clear(args, ctx),
        Some(SourceSubcommand::Admin(SourceAdminCmd::Recover(args))) => {
            crate::cli::source_rag::handle_recover(args, ctx)
        }
    }
}

fn handle_list(args: SourceListArgs, ctx: &CommandCtx) -> Result<CommandOutcome> {
    let repo_root = ctx.require_repo_path()?;
    let project_path = svc_util::resolve_project(repo_root, ctx.project(), ctx.cwd())?;

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

    let config = svc_source::advanced::config::load_repository_config(repo_root)?;
    let sources = if config.is_lance() {
        let store = svc_source::advanced::sync::open_active_store(repo_root)?;
        let catalog = svc_source::advanced::catalog::SourceCatalog::discover(&config, repo_root)?;
        let project_path_rel =
            project_path.strip_prefix(repo_root).unwrap_or(&project_path).to_string_lossy().replace('\\', "/");
        catalog
            .registrations(Some(&store))?
            .into_iter()
            .filter(|registration| registration.project_path == project_path_rel)
            .filter_map(|registration| {
                let kind = match registration.source_type.as_str() {
                    "pdf" => FileKind::Pdf,
                    "rss" => FileKind::Rss,
                    "web" => FileKind::Web,
                    "file" => FileKind::File,
                    _ => return None,
                };
                let is_url = registration.registered_location.starts_with("http://")
                    || registration.registered_location.starts_with("https://");
                let source_kind = match registration.source_kind.as_deref() {
                    Some("yuque") => Some(SourceKind::Yuque),
                    Some("meeting") => Some(SourceKind::Meeting),
                    Some("misc") => Some(SourceKind::Misc),
                    Some(other) => Some(SourceKind::Other(other.to_string())),
                    None => None,
                };
                Some(crate::model::source::Source {
                    name: registration.source_identity,
                    kind,
                    source_kind,
                    url: is_url.then(|| registration.registered_location.clone()),
                    path: (!is_url).then_some(registration.registered_location),
                    tags: serde_json::from_str(&registration.tags_json).unwrap_or_default(),
                    added_at: String::new(),
                    updated_at: String::new(),
                })
            })
            .filter(|source| {
                args.filter.as_ref().is_none_or(|filter| source.name.to_lowercase().contains(&filter.to_lowercase()))
            })
            .filter(|source| type_filter.as_ref().is_none_or(|kind| source.kind == *kind))
            .collect()
    } else {
        svc_source::list(&project_path, args.filter.as_deref(), type_filter)?
    };

    let opts = ListOpts::from_flags(args.no_headers.no_headers, args.no_trunc.no_trunc)
        .with_repo_root(Some(project_path.to_path_buf()));

    match ctx.format() {
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

fn handle_update(args: SourceUpdateArgs, ctx: &CommandCtx) -> Result<CommandOutcome> {
    let repo_root = ctx.require_repo_path()?;
    let project_path = svc_util::resolve_project(repo_root, ctx.project(), ctx.cwd())?;
    identity::validate_entity_path(&project_path, &args.path)?;

    if args.dry_run.dry_run {
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
        return match ctx.format() {
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

    let config = svc_source::advanced::config::load_repository_config(repo_root)?;
    let source = if config.is_lance() {
        svc_source::advanced::primary::update_registration(
            repo_root,
            &project_path,
            &args.path,
            args.rename.as_deref(),
            args.url.as_deref(),
        )?
    } else {
        svc_source::update(&project_path, &update_args)?
    };

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
    match ctx.format() {
        Format::Json => Ok(CommandOutcome::Success(verb_json(&result), Vec::new(), None)),
        Format::Text => Ok(CommandOutcome::Success(
            serde_json::Value::String(verb_text(&result, &VerbOpts::from_repo_root(Some(project_path.as_path())))),
            Vec::new(),
            None,
        )),
    }
}

fn handle_index(args: SourceIndexArgs, ctx: &CommandCtx) -> Result<CommandOutcome> {
    let repo_root = ctx.require_repo_path()?;
    let project_path = crate::service::project::resolve_project(repo_root, ctx.project(), ctx.cwd())?;
    let config = svc_source::advanced::config::load_repository_config(repo_root)?;
    let report = if config.is_lance() {
        // Spec 075 US2: indexing is a disk-adoption and reconcile pass — files
        // on disk but unknown to the store are imported, and entries whose
        // file has vanished are reported as missing, never removed (I-1). The
        // old `sync_repository` dispatch only ever saw the existing
        // `sources:` list and could not see a genuinely new file (D4).
        svc_source::advanced::primary::reconcile_and_adopt(repo_root, &project_path, args.dry_run.dry_run)?
    } else {
        svc_source::reconcile(&project_path, args.dry_run.dry_run)?
    };

    let scanned_count = report.added.len() + report.removed.len() + report.kept_count as usize;

    match ctx.format() {
        Format::Json => {
            let data = serde_json::json!({
                "kind": "source",
                "added": report.added,
                "removed": report.removed,
                "kept_count": report.kept_count,
                "scanned_count": scanned_count,
                "dry_run": args.dry_run.dry_run,
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
                dry_run: args.dry_run.dry_run,
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

fn handle_remove(args: SourceRemoveArgs, ctx: &mut CommandCtx) -> Result<CommandOutcome> {
    let is_path = args.name_or_path.contains('/') || args.name_or_path.starts_with(defaults::SOURCES_DIR);
    if !is_path {
        ctx.warn_subject("positional NAME", "full PATH (e.g., sources/yuque/foo.md)");
    }
    let project_path = svc_util::resolve_project(ctx.require_repo_path()?, ctx.project(), ctx.cwd())?;
    identity::validate_entity_path(&project_path, &args.name_or_path)?;

    require_confirmation(&ConfirmArgs {
        verb_label: "removal",
        kind: "source",
        identity: &args.name_or_path,
        yes: args.yes.yes,
        force: args.force.force,
    })?;

    let repo_root = ctx.require_repo_path()?;
    let config = svc_source::advanced::config::load_repository_config(repo_root)?;
    let report = if config.is_lance() {
        svc_source::advanced::primary::remove_registration(
            repo_root,
            &project_path,
            &args.name_or_path,
            args.keep_file,
            args.force.force,
            args.dry_run.dry_run,
        )?
    } else {
        svc_source::remove_source(
            &project_path,
            &args.name_or_path,
            args.keep_file,
            args.force.force,
            args.dry_run.dry_run,
        )?
    };

    let result = VerbResult {
        verb: Verb::Remove,
        kind: "source",
        identity: report.source.name.clone(),
        old_identity: None,
        path: report.source.path.clone(),
        dry_run: args.dry_run.dry_run,
        details: serde_json::json!({"removed": true}),
    };
    match ctx.format() {
        Format::Json => Ok(CommandOutcome::Success(verb_json(&result), Vec::new(), None)),
        Format::Text => Ok(CommandOutcome::Success(
            serde_json::Value::String(verb_text(&result, &VerbOpts::from_repo_root(Some(project_path.as_path())))),
            Vec::new(),
            None,
        )),
    }
}

fn handle_clean(args: SourceCleanArgs, ctx: &CommandCtx) -> Result<CommandOutcome> {
    let repo_root = ctx.require_repo_path()?;
    let project_path = svc_util::resolve_project(repo_root, ctx.project(), ctx.cwd())?;

    let config = svc_source::advanced::config::load_repository_config(repo_root)?;
    let report = if config.is_lance() {
        svc_source::advanced::primary::clean_registrations(repo_root, &project_path, args.dry_run.dry_run)?
    } else {
        svc_source::clean(&project_path, args.dry_run.dry_run)?
    };

    match ctx.format() {
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
            let prefix = if args.dry_run.dry_run { "[dry-run] " } else { "" };

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

fn handle_move(args: SourceMoveArgs, ctx: &CommandCtx) -> Result<CommandOutcome> {
    let root = ctx.require_repo_path()?;
    let source_project = svc_util::resolve_project(root, ctx.project(), ctx.cwd())?;
    let target_project = svc_util::resolve_project(root, Some(&args.to_project), ctx.cwd())?;
    let report = svc_source::move_source(&source_project, &target_project, &args.path, args.dry_run.dry_run)?;
    let mut rag_indexed = false;
    let mut warnings = Vec::new();
    if !report.dry_run
        && let Ok(config) = svc_source::advanced::config::load_repository_config(root)
        && config.is_lance()
    {
        match svc_source::advanced::sync::sync_repository(root, &config, None, None, false, true) {
            Ok(_) => rag_indexed = true,
            Err(error) => warnings.push(format!("RAG re-key deferred; run `mf source sync` ({error})")),
        }
    }
    let result = VerbResult {
        verb: Verb::Move,
        kind: "source",
        identity: report.name.clone(),
        old_identity: Some(report.old_path.clone()),
        path: Some(report.new_path.clone()),
        dry_run: report.dry_run,
        details: serde_json::json!({"name": report.name, "old_path": report.old_path, "new_path": report.new_path, "dry_run": report.dry_run, "rag_indexed": rag_indexed}),
    };
    match ctx.format() {
        Format::Json => Ok(CommandOutcome::Success(verb_json(&result), warnings.clone(), None)),
        Format::Text => Ok(CommandOutcome::Success(
            serde_json::Value::String(verb_text(&result, &VerbOpts::from_repo_root(Some(root)))),
            warnings,
            None,
        )),
    }
}

fn handle_rename(args: SourceRenameArgs, ctx: &CommandCtx) -> Result<CommandOutcome> {
    let repo_root = ctx.require_repo_path()?;
    let project_path = svc_util::resolve_project(repo_root, ctx.project(), ctx.cwd())?;
    identity::validate_entity_path(&project_path, &args.old_path)?;
    identity::validate_entity_path(&project_path, &args.new_path)?;

    let config = svc_source::advanced::config::load_repository_config(repo_root)?;
    if config.is_lance() {
        let report = svc_source::advanced::primary::rename_registration(
            repo_root,
            &project_path,
            &args.old_path,
            &args.new_path,
            args.force.force,
            args.dry_run.dry_run,
        )?;
        let result = VerbResult {
            verb: Verb::Rename,
            kind: "source",
            identity: report.after.name.clone(),
            old_identity: Some(report.before.name.clone()),
            path: report.after.path.clone(),
            dry_run: report.dry_run,
            details: serde_json::json!({}),
        };
        return match ctx.format() {
            Format::Json => Ok(CommandOutcome::Success(verb_json(&result), Vec::new(), None)),
            Format::Text => Ok(CommandOutcome::Success(
                serde_json::Value::String(verb_text(&result, &VerbOpts::from_repo_root(Some(project_path.as_path())))),
                Vec::new(),
                None,
            )),
        };
    }

    if args.dry_run.dry_run {
        let result = VerbResult {
            verb: Verb::Rename,
            kind: "source",
            identity: args.new_path.clone(),
            old_identity: Some(args.old_path.clone()),
            path: None,
            dry_run: true,
            details: serde_json::json!({}),
        };
        return match ctx.format() {
            Format::Json => Ok(CommandOutcome::Success(verb_json(&result), Vec::new(), None)),
            Format::Text => Ok(CommandOutcome::Success(
                serde_json::Value::String(verb_text(&result, &VerbOpts::from_repo_root(Some(project_path.as_path())))),
                Vec::new(),
                None,
            )),
        };
    }

    let report = svc_source::rename_source(&project_path, &args.old_path, &args.new_path, args.force.force, false)?;

    let result = VerbResult {
        verb: Verb::Rename,
        kind: "source",
        identity: report.after.name.clone(),
        old_identity: Some(report.before.name.clone()),
        path: report.after.path.clone(),
        dry_run: false,
        details: serde_json::json!({}),
    };
    match ctx.format() {
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

fn handle_source_show(args: SourceShowArgs, ctx: &CommandCtx) -> Result<CommandOutcome> {
    let repo_root = ctx.require_repo_path()?;
    let project_path = svc_util::resolve_project(repo_root, ctx.project(), ctx.cwd())?;
    identity::validate_entity_path(&project_path, &args.path)?;
    let config = svc_source::advanced::config::load_repository_config(repo_root)?;
    let sources = if config.is_lance() {
        let store = svc_source::advanced::sync::open_active_store(repo_root)?;
        let catalog = svc_source::advanced::catalog::SourceCatalog::discover(&config, repo_root)?;
        let project_path_rel =
            project_path.strip_prefix(repo_root).unwrap_or(&project_path).to_string_lossy().replace('\\', "/");
        catalog
            .registrations(Some(&store))?
            .into_iter()
            .filter(|registration| registration.project_path == project_path_rel)
            .filter_map(|registration| {
                let kind = match registration.source_type.as_str() {
                    "pdf" => FileKind::Pdf,
                    "rss" => FileKind::Rss,
                    "web" => FileKind::Web,
                    "file" => FileKind::File,
                    _ => return None,
                };
                let is_url = registration.registered_location.starts_with("http://")
                    || registration.registered_location.starts_with("https://");
                let source_kind = match registration.source_kind.as_deref() {
                    Some("yuque") => Some(SourceKind::Yuque),
                    Some("meeting") => Some(SourceKind::Meeting),
                    Some("misc") => Some(SourceKind::Misc),
                    _ => None,
                };
                Some(crate::model::source::Source {
                    name: registration.source_identity,
                    kind,
                    source_kind,
                    url: is_url.then(|| registration.registered_location.clone()),
                    path: (!is_url).then_some(registration.registered_location),
                    tags: serde_json::from_str(&registration.tags_json).unwrap_or_default(),
                    added_at: String::new(),
                    updated_at: String::new(),
                })
            })
            .collect()
    } else {
        svc_source::list(&project_path, None, None)?
    };

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

            match ctx.format() {
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

fn handle_add(args: SourceAddArgs, ctx: &CommandCtx) -> Result<CommandOutcome> {
    let repo_root = ctx.require_repo_path()?;
    let project_path = svc_util::resolve_project(repo_root, ctx.project(), ctx.cwd())?;

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
        force: args.force.force,
    };

    let config = svc_source::advanced::config::load_repository_config(repo_root)?;
    if config.is_lance() {
        let outcome = svc_source::advanced::primary::add_registration(
            repo_root,
            &project_path,
            ctx.cwd(),
            &add_args,
            args.register_only,
            args.dry_run.dry_run,
            !args.no_index,
            args.article.as_deref(),
        )?;
        return source_add_outcome(outcome, args.dry_run.dry_run, &project_path, ctx);
    }

    if args.register_only {
        let outcome = svc_source::register_only(repo_root, &project_path, ctx.cwd(), &add_args, args.dry_run.dry_run)?;
        return source_add_outcome(outcome, args.dry_run.dry_run, &project_path, ctx);
    }

    if args.dry_run.dry_run {
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
        return match ctx.format() {
            Format::Json => Ok(CommandOutcome::Success(verb_json(&result), Vec::new(), None)),
            Format::Text => Ok(CommandOutcome::Success(
                serde_json::Value::String(verb_text(&result, &VerbOpts::from_repo_root(Some(project_path.as_path())))),
                Vec::new(),
                None,
            )),
        };
    }

    let outcome = svc_source::add(repo_root, &project_path, ctx.cwd(), &add_args)?;

    source_add_outcome(outcome, false, &project_path, ctx)
}

fn source_add_outcome(
    outcome: svc_source::add::AddOutcome,
    dry_run: bool,
    project_path: &std::path::Path,
    ctx: &CommandCtx,
) -> Result<CommandOutcome> {
    let mut details = serde_json::json!({
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
            svc_source::AddMode::Register => "register",
        },
        "replaced": outcome.replaced,
        "rag_indexed": outcome.indexing.as_ref().is_some_and(|indexing| indexing.indexed),
    });
    if let Some(ref key) = outcome.registration_key {
        details["registration_key"] = serde_json::Value::String(key.clone());
    }

    let mut warnings = Vec::new();
    if outcome.projection_degraded {
        warnings.push("compatibility projection has drift — run `mf source sync` to reconcile".to_string());
    }
    if let Some(ref idx) = outcome.indexing {
        details["indexing"] = serde_json::json!({
            "indexed": idx.indexed,
            "chunks": idx.chunks,
            "backend": idx.backend,
            "warning": idx.warning,
        });
        if let Some(ref w) = idx.warning {
            warnings.push(w.clone());
        }
    }

    let result = VerbResult {
        verb: Verb::Add,
        kind: "source",
        identity: outcome.source.name.clone(),
        old_identity: None,
        path: outcome.source.path.clone(),
        dry_run,
        details,
    };
    match ctx.format() {
        Format::Json => Ok(CommandOutcome::Success(verb_json(&result), warnings, None)),
        Format::Text => Ok(CommandOutcome::Success(
            serde_json::Value::String(verb_text(&result, &VerbOpts::from_repo_root(Some(project_path)))),
            warnings,
            None,
        )),
    }
}

pub fn dispatch_global_search(args: GlobalSearchArgs, ctx: &mut CommandCtx) -> Result<CommandOutcome> {
    handle_search(args.into(), ctx, true)
}

fn handle_search(args: SourceSearchArgs, ctx: &mut CommandCtx, canonical: bool) -> Result<CommandOutcome> {
    let repo = ctx.require_repo_path()?;
    let source_config = crate::service::source::advanced::config::load_repository_config(repo)?;

    let mode = match args.mode {
        Some(SearchModeArg::Basic) => crate::model::source_search::SearchMode::Basic,
        Some(SearchModeArg::Advanced) => crate::model::source_search::SearchMode::Advanced,
        Some(SearchModeArg::Both) => crate::model::source_search::SearchMode::Both,
        None => match source_config.default_search_mode {
            crate::model::manifest::SearchDefaultMode::Basic => crate::model::source_search::SearchMode::Basic,
            crate::model::manifest::SearchDefaultMode::Advanced => crate::model::source_search::SearchMode::Advanced,
            crate::model::manifest::SearchDefaultMode::Both => crate::model::source_search::SearchMode::Both,
        },
    };

    // Parse `--label key=value` selectors; a bare key means "any value present".
    let label_filter: Vec<(String, String)> = args
        .labels
        .iter()
        .filter_map(|entry| entry.split_once('=').map(|(k, v)| (k.trim().to_string(), v.trim().to_string())))
        .collect();

    let report = crate::service::source::advanced::retrieval::search_repository(
        repo,
        &args.query,
        mode,
        args.project.as_deref(),
        args.file_kind.as_deref(),
        args.source.as_deref(),
        &label_filter,
        args.revision.as_deref(),
        args.limit,
    )?;

    let warnings = report.warnings.clone();
    match ctx.format() {
        Format::Json => {
            let inner = serde_json::to_value(&report)?;
            Ok(CommandOutcome::Success(
                serde_json::json!({"status": "ok", "command": if canonical { "search" } else { "source.search" }, "data": inner}),
                warnings,
                None,
            ))
        }
        Format::Text => {
            let opts = ListOpts::from_flags(false, false).with_repo_root(Some(repo.to_path_buf()));
            let mut rows: Vec<ListRow> = Vec::new();
            for (i, r) in report.results.iter().enumerate() {
                let source_label = if let Some(reg) = r.registrations.first() {
                    // Compact context summary (spec 071): attribution plus lifecycle
                    // status and relation count for articles, or import provenance
                    // for source bindings. Full structure is in the JSON output.
                    let mut label = format!("{}:{}", reg.project_identity, reg.source_identity);
                    if let Some(status) = &reg.context.lifecycle_status {
                        label.push_str(&format!(" [{status}]"));
                    }
                    if !reg.context.relations.is_empty() {
                        label.push_str(&format!(" →{}", reg.context.relations.len()));
                    }
                    if let Some(article) = reg.context.imported_by.as_ref().and_then(|p| p.article.as_deref()) {
                        label.push_str(&format!(" ←{article}"));
                    }
                    label
                } else {
                    String::new()
                };
                let snippet: String = r
                    .snippet
                    .chars()
                    .take(80)
                    .collect::<String>()
                    .replace('\n', " ")
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ");
                rows.push(ListRow {
                    cells: vec![
                        ListCell::Number((i + 1).to_string()),
                        ListCell::Number(format!("{:.2}", r.combined_score)),
                        ListCell::Text(r.source_type.clone()),
                        ListCell::Text(source_label),
                        ListCell::Text(snippet),
                    ],
                });
            }
            let view = ListView { headers: &["#", "SCORE", "TYPE", "SOURCE", "SNIPPET"], rows, plural_noun: "results" };
            let header = format!(
                "query: {}  mode: {}{}  results: {}\n",
                report.query,
                report.resolved_mode,
                if report.degraded { " (degraded)" } else { "" },
                report.results.len(),
            );
            Ok(CommandOutcome::Raw(header + &render_text(&view, &opts), None))
        }
    }
}
