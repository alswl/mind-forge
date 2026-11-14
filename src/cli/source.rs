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
    // `-n` was this flag's short form until #51 gave it to `--dry-run`
    // CLI-wide; this was the only collision. Deliberately a plain comment, not
    // `///`: a multi-paragraph doc comment here flips clap's entire `--help`
    // for this command to the long format, wrecking every other flag's layout.
    /// Override the source name derived from the input
    #[arg(long)]
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
    /// Project-relative source path or registered source name
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
    /// Project-relative source path or registered source name
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
    /// Current project-relative source path or registered source name
    pub old_path: String,
    /// New location as a path (cwd-relative or absolute) — e.g.
    /// "sources/yuque/renamed.md" or "renamed.md" for the current
    /// directory. `--project` does not change how this resolves; it only
    /// requires the result to fall inside that project.
    pub new_path: String,
    #[command(flatten)]
    pub force: ForceFlag,
    #[command(flatten)]
    pub dry_run: DryRunFlag,
}

#[derive(Debug, Clone, Args, Serialize)]
pub struct SourceMoveArgs {
    /// Project-relative source path or registered source name
    pub path: String,
    /// Destination project name or path (cwd-relative, repo-relative, or absolute)
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
    /// Project-relative source path or registered source name
    pub path: String,
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

    let sources = svc_source::list(&project_path, args.filter.as_deref(), type_filter)?;

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
    let source_name = svc_source::resolve_selector(&project_path, &args.path)?;

    if args.dry_run.dry_run {
        let mut changes = serde_json::Map::new();
        if let Some(ref rename) = args.rename {
            changes.insert("rename".to_string(), serde_json::json!({"from": args.path, "to": rename}));
        }
        if let Some(ref url) = args.url {
            changes.insert("url".to_string(), serde_json::json!({"to": url}));
        }
        let identity = args.rename.as_ref().unwrap_or(&source_name).clone();
        let old_identity = args.rename.as_ref().map(|_| source_name.clone());
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
        svc_source::UpdateArgs { name: &source_name, rename: args.rename.as_deref(), url: args.url.as_deref() };

    let source = svc_source::update(&project_path, &update_args)?;

    let mut changes = serde_json::Map::new();
    if let Some(ref rename) = args.rename {
        changes.insert("rename".to_string(), serde_json::json!({"from": args.path, "to": rename}));
    }
    if let Some(ref url) = args.url {
        changes.insert("url".to_string(), serde_json::json!({"to": url}));
    }

    let identity = args.rename.as_ref().unwrap_or(&source_name).clone();
    let old_identity = args.rename.as_ref().map(|_| source_name.clone());
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
    let report = svc_source::reconcile(&project_path, args.dry_run.dry_run)?;

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

    let report = svc_source::remove_source(
        &project_path,
        &args.name_or_path,
        args.keep_file,
        args.force.force,
        args.dry_run.dry_run,
    )?;

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

    let report = svc_source::clean(&project_path, args.dry_run.dry_run)?;

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
    identity::validate_entity_path(&source_project, &args.path)?;
    let source_name = svc_source::resolve_selector(&source_project, &args.path)?;
    let report = svc_source::move_source(&source_project, &target_project, &source_name, args.dry_run.dry_run)?;
    let result = VerbResult {
        verb: Verb::Move,
        kind: "source",
        identity: report.name.clone(),
        old_identity: Some(report.old_path.clone()),
        path: Some(report.new_path.clone()),
        dry_run: report.dry_run,
        details: serde_json::json!({"name": report.name, "old_path": report.old_path, "new_path": report.new_path, "dry_run": report.dry_run}),
    };
    match ctx.format() {
        Format::Json => Ok(CommandOutcome::Success(verb_json(&result), Vec::new(), None)),
        Format::Text => Ok(CommandOutcome::Success(
            serde_json::Value::String(verb_text(&result, &VerbOpts::from_repo_root(Some(root)))),
            Vec::new(),
            None,
        )),
    }
}

fn handle_rename(args: SourceRenameArgs, ctx: &CommandCtx) -> Result<CommandOutcome> {
    let repo_root = ctx.require_repo_path()?;
    let project_path = svc_util::resolve_project(repo_root, ctx.project(), ctx.cwd())?;
    // `old_path` is an identity lookup (project-relative, matched against
    // the registration's stored path) — `validate_entity_path` is the right
    // check for it. `new_path`/target is a **destination**, resolved
    // cwd-relative by `rename_source`/`rename_registration` themselves
    // (spec 082 FR-002, `research.md` §2) — validating it project-relative
    // here would apply the wrong base, so it is no longer checked at this
    // layer.
    identity::validate_entity_path(&project_path, &args.old_path)?;
    let old_name = svc_source::resolve_selector(&project_path, &args.old_path)?;

    // Let `rename_source` handle dry-run itself (it already
    // resolves and validates the target either way) rather than a
    // CLI-level stub that echoed the raw argument back without resolving
    // it — that stub never reported the true destination (spec FR-018) and
    // silently ignored `args.dry_run.dry_run` on the executing call below it.
    let report = svc_source::rename_source(
        &project_path,
        ctx.cwd(),
        &old_name,
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
    let source_name = svc_source::resolve_selector(&project_path, &args.path)?;
    let sources = svc_source::list(&project_path, None, None)?;

    let resolved = sources.iter().find(|source| source.name == source_name);

    match resolved {
        None => Err(MfError::not_found(
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
    });
    if let Some(ref overwritten) = outcome.replaced_location {
        details["replaced_location"] = serde_json::Value::String(overwritten.clone());
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
        Format::Json => Ok(CommandOutcome::Success(verb_json(&result), Vec::new(), None)),
        Format::Text => {
            let mut text = verb_text(&result, &VerbOpts::from_repo_root(Some(project_path)));
            if let Some(ref overwritten) = outcome.replaced_location {
                text.push_str(&format!("\n  replaced registration at {overwritten}"));
            }
            Ok(CommandOutcome::Success(serde_json::Value::String(text), Vec::new(), None))
        }
    }
}
