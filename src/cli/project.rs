use std::path::PathBuf;

use clap::{Args, Subcommand};
use serde::Serialize;

use clap::ValueEnum;

use crate::cli::deprecation::DeprecationContext;
use crate::cli::shared_flags::LintFlags;
use crate::cli::shared_flags::NoHeadersFlag;
use crate::cli::shared_flags::NoTruncFlag;
use crate::cli::{CommandOutcome, RepoRequirement};
use crate::error::{MfError, Result};
use crate::model::project::LintKind;
use crate::model::Resource;
use crate::output::confirm::{require_confirmation, ConfirmArgs};
use crate::output::list::{json_collection, render_text, ListCell, ListOpts, ListRow, ListView};
use crate::output::show::{
    json_envelope, render_text as render_show_text, ShowBlock, ShowField, ShowOpts, ShowSection, ShowValue,
};
use crate::output::verb::{json_envelope as verb_json, render_text as verb_text, Verb, VerbOpts, VerbResult};
use crate::output::Format;
use crate::service::repo;
use crate::service::{self as svc};

#[derive(Debug, Clone, Args)]
pub struct ProjectCmd {
    #[command(subcommand)]
    pub command: Option<ProjectSubcommand>,
}

#[derive(Debug, Clone, Subcommand)]
pub enum ProjectSubcommand {
    #[command(about = "Create a project")]
    New(ProjectNewArgs),
    #[command(about = "List projects", visible_alias = "ls")]
    List(ProjectListArgs),
    #[command(about = "Archive a project")]
    Archive(ProjectArchiveArgs),
    #[command(about = "Show project status (deprecated: use `show`)", hide = true)]
    Status(ProjectStatusArgs),
    #[command(about = "Lint a project")]
    Lint(ProjectLintArgs),
    #[command(about = "Index projects")]
    Index(ProjectIndexArgs),
    #[command(about = "Show project details")]
    Show(ProjectShowArgs),
    #[command(about = "Import a directory as a project")]
    Import(ProjectImportArgs),
    #[command(about = "Rename a project")]
    Rename(ProjectRenameArgs),
    #[command(about = "Remove a project", visible_alias = "rm")]
    Remove(ProjectRemoveArgs),
}

#[derive(Debug, Clone, Args, Serialize)]
pub struct ProjectNewArgs {
    /// Project path (cwd-relative, repo-relative, or simple name).
    pub path: String,
    #[arg(long)]
    pub template: Option<String>,
    #[arg(long)]
    pub force: bool,
    #[arg(long = "dry-run")]
    pub dry_run: bool,
}

#[derive(Debug, Clone, Args, Serialize)]
pub struct ProjectListArgs {
    #[command(flatten)]
    pub no_headers: NoHeadersFlag,
    #[command(flatten)]
    pub no_trunc: NoTruncFlag,
}

#[derive(Debug, Clone, Args, Serialize)]
pub struct ProjectArchiveArgs {
    pub name_or_path: String,
    #[arg(short = 'f', long)]
    pub force: bool,
    #[arg(short = 'y', long)]
    pub yes: bool,
    #[arg(long = "dry-run")]
    pub dry_run: bool,
}

#[derive(Debug, Clone, Args, Serialize)]
pub struct ProjectStatusArgs {}

#[derive(Debug, Clone, Args, Serialize)]
pub struct ProjectLintArgs {
    #[command(flatten)]
    pub lint: LintFlags,
}

#[derive(Debug, Clone, Args, Serialize)]
pub struct ProjectIndexArgs {
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Debug, Clone, Args, Serialize)]
pub struct ProjectShowArgs {
    pub name: String,
}

#[derive(Debug, Clone, Args, Serialize)]
pub struct ProjectImportArgs {
    pub directory: String,
    #[arg(long)]
    pub r#type: Option<String>,
    #[arg(long)]
    pub source: Option<String>,
    #[arg(long)]
    pub assets: Option<String>,
    #[arg(short = 'f', long)]
    pub force: bool,
    #[arg(short = 'y', long = "non-interactive")]
    pub non_interactive: bool,
}

#[derive(Debug, Clone, Args, Serialize)]
pub struct ProjectRenameArgs {
    pub old_name: String,
    pub new_name: String,
    #[arg(short = 'f', long)]
    pub force: bool,
    #[arg(long = "dry-run")]
    pub dry_run: bool,
}

#[derive(Debug, Clone, Args)]
pub struct ProjectRemoveArgs {
    pub name: String,
    #[arg(short = 'f', long)]
    pub force: bool,
    #[arg(short = 'y', long)]
    pub yes: bool,
    #[arg(long = "dry-run")]
    pub dry_run: bool,
}

impl ProjectCmd {
    pub fn requires_repo(&self) -> RepoRequirement {
        match self.command {
            Some(ProjectSubcommand::Index(_)) => RepoRequirement::NotRequired,
            _ => RepoRequirement::Required,
        }
    }
}

/// dispatch 现在接受 repo_root 参数用于需要文件系统操作的子命令
pub fn dispatch(
    command: ProjectCmd,
    repo_root: Option<&PathBuf>,
    format: Format,
    project: Option<&str>,
    deprecation: &mut DeprecationContext,
) -> Result<CommandOutcome> {
    match command.command {
        None => Ok(CommandOutcome::GroupHelp("project")),
        Some(ProjectSubcommand::New(args)) => handle_new(args, repo_root, format),
        Some(ProjectSubcommand::List(args)) => handle_list(args, repo_root, format),
        Some(ProjectSubcommand::Archive(args)) => handle_archive(args, repo_root, format),
        Some(ProjectSubcommand::Status(args)) => {
            deprecation.warn_subject("project status", "project show");
            handle_status(args, repo_root, format, project)
        }
        Some(ProjectSubcommand::Lint(args)) => handle_lint(args, repo_root, format, project),
        Some(ProjectSubcommand::Index(args)) => handle_index(args, repo_root, format),
        Some(ProjectSubcommand::Show(args)) => handle_show(args, repo_root, format),
        Some(ProjectSubcommand::Import(args)) => handle_import(args, repo_root, format),
        Some(ProjectSubcommand::Rename(args)) => handle_rename(args, repo_root, format),
        Some(ProjectSubcommand::Remove(args)) => handle_remove(args, repo_root, format),
    }
}

// ---------------------------------------------------------------------------
// US3: mf project new
// ---------------------------------------------------------------------------

fn handle_new(args: ProjectNewArgs, repo_root: Option<&PathBuf>, format: Format) -> Result<CommandOutcome> {
    let root = repo_root.ok_or_else(MfError::not_in_mind_repo)?;
    let cwd = std::env::current_dir().map_err(MfError::Io)?;

    // Normalize the user-provided selector into a canonical project identity
    let identity = svc::identity::normalize_project_selector(root, &args.path, &cwd)?;

    // Reject if the resolved path is inside another existing project (nested).
    if let Some(parent) = identity.resolved_path.parent() {
        if let Some(parent_project) = svc::util::detect_current_project(root, parent) {
            return Err(MfError::usage(
                format!("project path '{}' is inside another project '{}'", identity.path, parent_project),
                Some("create the project outside the existing project root".to_string()),
            ));
        }
    }

    if args.dry_run {
        let result = VerbResult {
            verb: Verb::Create,
            kind: "project",
            identity: identity.path.clone(),
            old_identity: None,
            path: Some(identity.path.clone()),
            dry_run: true,
            details: serde_json::json!({"requested_path": identity.requested_path, "path": identity.path}),
        };
        return match format {
            Format::Json => Ok(CommandOutcome::Success(verb_json(&result), None)),
            Format::Text => Ok(CommandOutcome::Success(
                serde_json::Value::String(verb_text(&result, &VerbOpts::from_repo_root(Some(root.as_path())))),
                None,
            )),
        };
    }

    let report =
        svc::project::scaffold(root, &identity.requested_path, &identity.path, &identity.resolved_path, args.force)?;
    let entry = svc::project::upsert_project_entry(root, &identity.path, &report.created_at, args.force)?;

    let result = VerbResult {
        verb: Verb::Create,
        kind: "project",
        identity: identity.path.clone(),
        old_identity: None,
        path: Some(identity.path.clone()),
        dry_run: false,
        details: serde_json::json!({
            "requested_path": report.requested_path,
            "path": report.path,
            "created_at": entry.created_at,
            "scaffolded": report.scaffolded,
        }),
    };
    match format {
        Format::Json => Ok(CommandOutcome::Success(verb_json(&result), None)),
        Format::Text => Ok(CommandOutcome::Success(
            serde_json::Value::String(verb_text(&result, &VerbOpts::from_repo_root(Some(root.as_path())))),
            None,
        )),
    }
}

// ---------------------------------------------------------------------------
// US2: mf project list
// ---------------------------------------------------------------------------

fn handle_list(args: ProjectListArgs, repo_root: Option<&PathBuf>, format: Format) -> Result<CommandOutcome> {
    let root = repo_root.ok_or_else(MfError::not_in_mind_repo)?;
    let entries = svc::project::list_projects(root)?;

    let opts =
        ListOpts::from_flags(args.no_headers.no_headers, args.no_trunc.no_trunc).with_repo_root(Some(root.clone()));

    match format {
        Format::Json => {
            let items: Vec<serde_json::Value> = entries
                .iter()
                .map(|e| {
                    serde_json::json!({
                        "identity": e.identity(),
                        "name": e.name,
                        "path": e.path,
                        "created_at": e.created_at,
                        "archived_at": e.archived_at,
                        "document_count": e.document_count,
                        "last_activity_at": e.last_activity_at,
                    })
                })
                .collect();
            Ok(CommandOutcome::Success(json_collection("projects", items), None))
        }
        Format::Text => {
            let mut rows = Vec::with_capacity(entries.len());
            for e in &entries {
                rows.push(ListRow {
                    cells: vec![
                        ListCell::Text(e.name.clone()),
                        ListCell::Number(e.document_count.to_string()),
                        ListCell::Optional(e.last_activity_at.clone()),
                        ListCell::Text(e.created_at.clone()),
                    ],
                });
            }
            let view =
                ListView { headers: &["NAME", "DOCS", "LAST ACTIVITY", "CREATED"], rows, plural_noun: "projects" };
            Ok(CommandOutcome::Raw(render_text(&view, &opts), None))
        }
    }
}

// ---------------------------------------------------------------------------
// US3: mf project status
// ---------------------------------------------------------------------------

fn handle_status(
    _args: ProjectStatusArgs,
    repo_root: Option<&PathBuf>,
    _format: Format,
    project: Option<&str>,
) -> Result<CommandOutcome> {
    let root = repo_root.ok_or_else(MfError::not_in_mind_repo)?;
    let cwd = std::env::current_dir().map_err(MfError::Io)?;
    let project_path = svc::project::resolve_project(root, project, &cwd)?;
    let snapshot = svc::project::status_for(root, &project_path)?;

    let data = serde_json::json!(snapshot);
    Ok(CommandOutcome::Success(data, None))
}

// ---------------------------------------------------------------------------
// US4: mf project lint
// ---------------------------------------------------------------------------

fn handle_lint(
    args: ProjectLintArgs,
    repo_root: Option<&PathBuf>,
    format: Format,
    project: Option<&str>,
) -> Result<CommandOutcome> {
    let root = repo_root.ok_or_else(MfError::not_in_mind_repo)?;

    // Parse --rule args into LintKind values
    let rules: Vec<LintKind> = if args.lint.rule.is_empty() {
        Vec::new()
    } else {
        args.lint
            .rule
            .iter()
            .map(|r| {
                LintKind::from_str(r, true).map_err(|e| {
                    MfError::usage(
                        format!("unknown lint rule '{r}': {e}"),
                        Some(
                            "available rules: missing_directory, stale_index_entry, name_convention, missing_manifest, duplicate_key"
                                .to_string(),
                        ),
                    )
                })
            })
            .collect::<std::result::Result<Vec<_>, _>>()?
    };

    let (mut issues, mut summary) = if let Some(project_name) = project {
        let cwd = std::env::current_dir().map_err(MfError::Io)?;
        let project_path = svc::project::resolve_project(root, Some(project_name), &cwd)?;
        svc::project::lint_project(&project_path, &rules, args.lint.fix)?
    } else {
        svc::project::lint_repo(root, &rules, args.lint.fix)?
    };

    // Apply --severity filter — must drop hidden issues from summary too,
    // so counters and --max-warnings agree with what the user sees.
    if let Some(ref sev) = args.lint.severity {
        let rank = severity_rank_str(sev);
        issues.retain(|i| {
            let s = i.get("severity").and_then(|v| v.as_str()).unwrap_or("info");
            severity_rank_str(s) <= rank
        });
        let post_errors = issues.iter().filter(|i| i.get("severity").and_then(|v| v.as_str()) == Some("error")).count();
        let post_warnings =
            issues.iter().filter(|i| i.get("severity").and_then(|v| v.as_str()) == Some("warning")).count();
        if let Some(obj) = summary.as_object_mut() {
            obj.insert("errors".to_string(), serde_json::json!(post_errors));
            obj.insert("warnings".to_string(), serde_json::json!(post_warnings));
        }
    }

    let errors = summary["errors"].as_u64().unwrap_or(0);
    let warnings = summary["warnings"].as_u64().unwrap_or(0);

    let exit_code =
        if args.lint.max_warnings.is_some_and(|max| warnings as i32 > max) || errors > 0 { Some(1) } else { None };

    let dry_run = args.lint.dry_run;

    match format {
        Format::Json => {
            let data =
                serde_json::json!({ "kind": "project", "issues": issues, "summary": summary, "dry_run": dry_run });
            Ok(CommandOutcome::Success(data, exit_code))
        }
        Format::Text => {
            let details = serde_json::json!({ "issues": issues, "summary": summary });
            let result = VerbResult {
                verb: Verb::Lint,
                kind: "project",
                identity: String::new(),
                old_identity: None,
                path: None,
                dry_run,
                details,
            };
            Ok(CommandOutcome::Raw(verb_text(&result, &VerbOpts::from_repo_root(Some(root.as_path()))), exit_code))
        }
    }
}

fn severity_rank_str(s: &str) -> u8 {
    match s {
        "error" => 0,
        "warning" => 1,
        _ => 2,
    }
}

// ---------------------------------------------------------------------------
// mf project index
// ---------------------------------------------------------------------------

fn handle_index(args: ProjectIndexArgs, repo_root: Option<&PathBuf>, format: Format) -> Result<CommandOutcome> {
    let root = repo_root.cloned().or_else(|| std::env::current_dir().ok()).ok_or_else(MfError::not_in_mind_repo)?;

    let minds_path = root.join("minds.yaml");
    let manifest = if minds_path.exists() {
        repo::load_manifest(&minds_path)?
    } else {
        crate::model::manifest::MindsManifest::create_default()
    };

    let scanned = repo::scan_project_dirs(&root, &manifest.projects_dir);
    let diff = repo::compute_diff(&manifest, &scanned);

    let _added_count = diff.added.len();
    let removed_count = diff.removed.len();
    let kept_count = manifest.projects.len().saturating_sub(removed_count);
    let scanned_count = scanned.len();

    if args.dry_run {
        let details = serde_json::json!({
            "added": diff.added,
            "removed": diff.removed,
            "kept_count": kept_count,
            "scanned_count": scanned_count,
            "minds_path": minds_path.to_string_lossy().to_string(),
        });
        let result = VerbResult {
            verb: Verb::Index,
            kind: "project",
            identity: String::new(),
            old_identity: None,
            path: None,
            dry_run: true,
            details,
        };
        return match format {
            Format::Json => Ok(CommandOutcome::Success(verb_json(&result), None)),
            Format::Text => Ok(CommandOutcome::Success(
                serde_json::Value::String(verb_text(&result, &VerbOpts::from_repo_root(Some(root.as_path())))),
                None,
            )),
        };
    }

    let updated = repo::reconcile(manifest, diff);
    repo::save_manifest(&updated, &minds_path)?;

    let details = serde_json::json!({
        "added": updated.projects.iter().map(|p| serde_json::json!({"identity": p.name, "path": p.path})).collect::<Vec<_>>(),
        "removed": [],
        "kept_count": updated.projects.len(),
        "scanned_count": scanned_count,
        "minds_path": minds_path.to_string_lossy().to_string(),
    });
    let result = VerbResult {
        verb: Verb::Index,
        kind: "project",
        identity: String::new(),
        old_identity: None,
        path: None,
        dry_run: false,
        details,
    };
    match format {
        Format::Json => Ok(CommandOutcome::Success(verb_json(&result), None)),
        Format::Text => Ok(CommandOutcome::Success(
            serde_json::Value::String(verb_text(&result, &VerbOpts::from_repo_root(Some(root.as_path())))),
            None,
        )),
    }
}

// ---------------------------------------------------------------------------
// Handle: mf project show
// ---------------------------------------------------------------------------

fn handle_show(
    args: ProjectShowArgs,
    repo_root: Option<&std::path::PathBuf>,
    format: Format,
) -> Result<CommandOutcome> {
    let root = repo_root.ok_or_else(MfError::not_in_mind_repo)?;
    let cwd = std::env::current_dir().map_err(MfError::Io)?;
    let project_path = svc::project::resolve_project(root, Some(&args.name), &cwd)?;
    let details = svc::project::show(&project_path, &args.name)?;

    let mut fields: Vec<ShowField> = vec![
        ShowField { label: "Name", value: ShowValue::Text(details.name.clone()) },
        ShowField { label: "Path", value: ShowValue::Path(details.path.clone()) },
        ShowField { label: "Articles", value: ShowValue::Text(details.article_count.to_string()) },
        ShowField { label: "Sources", value: ShowValue::Text(details.source_count.to_string()) },
        ShowField { label: "Assets", value: ShowValue::Text(details.asset_count.to_string()) },
        ShowField { label: "Last active", value: ShowValue::Optional(details.last_active.clone()) },
    ];
    if let Some(ref summary) = details.mind_yaml_summary {
        let types_str = if summary.types.is_empty() { "-".to_string() } else { summary.types.join(", ") };
        fields.push(ShowField {
            label: "Schema",
            value: ShowValue::Text(format!("v{} (types: {})", summary.schema_version, types_str)),
        });
        fields.push(ShowField { label: "Source dirs", value: ShowValue::Text(summary.source_dirs.join(", ")) });
        fields.push(ShowField { label: "Assets dir", value: ShowValue::Text(summary.assets_dir.clone()) });
    }
    let mut sections = Vec::new();
    if let Some(ref layout) = details.layout {
        sections.push(ShowSection {
            heading: "Layout",
            fields: vec![
                ShowField { label: "Articles", value: ShowValue::Text(layout.articles.clone()) },
                ShowField { label: "Sources", value: ShowValue::Text(layout.sources.clone()) },
                ShowField { label: "Assets", value: ShowValue::Text(layout.assets.clone()) },
                ShowField { label: "Templates", value: ShowValue::Text(layout.templates.clone()) },
                ShowField { label: "Build output", value: ShowValue::Text(layout.build_output.clone()) },
            ],
        });
    }

    let block = ShowBlock { kind: "project", identity: details.name.clone(), fields, sections };

    match format {
        Format::Json => {
            let details_json = serde_json::to_value(&details).map_err(MfError::Json)?;
            let extra = details_json.as_object().cloned().unwrap_or_default();
            Ok(CommandOutcome::Success(json_envelope(&block, extra), None))
        }
        Format::Text => Ok(CommandOutcome::Raw(
            render_show_text(&block, &ShowOpts::from_repo_root(repo_root.map(|r| r.as_path()))),
            None,
        )),
    }
}

// ---------------------------------------------------------------------------
// Handle: mf project archive
// ---------------------------------------------------------------------------

fn handle_archive(
    args: ProjectArchiveArgs,
    repo_root: Option<&std::path::PathBuf>,
    format: Format,
) -> Result<CommandOutcome> {
    let root = repo_root.ok_or_else(MfError::not_in_mind_repo)?;

    require_confirmation(&ConfirmArgs {
        verb_label: "archiving",
        kind: "project",
        identity: &args.name_or_path,
        yes: args.yes,
        force: args.force,
    })?;

    // --force with non-existent target: no-op success
    if args.force {
        let projects_dir = crate::service::repo::projects_dir_for(root)?;
        let project_path = crate::service::util::project_dir_for(root, &projects_dir, &args.name_or_path);
        if !project_path.exists() {
            let result = VerbResult {
                verb: Verb::Remove,
                kind: "project",
                identity: args.name_or_path.clone(),
                old_identity: None,
                path: None,
                dry_run: args.dry_run,
                details: serde_json::json!({"archived": false, "reason": "not found"}),
            };
            return match format {
                Format::Json => Ok(CommandOutcome::Success(verb_json(&result), None)),
                Format::Text => Ok(CommandOutcome::Success(
                    serde_json::Value::String(verb_text(&result, &VerbOpts::from_repo_root(Some(root.as_path())))),
                    None,
                )),
            };
        }
    }

    if args.dry_run {
        let result = VerbResult {
            verb: Verb::Remove,
            kind: "project",
            identity: args.name_or_path.clone(),
            old_identity: None,
            path: None,
            dry_run: true,
            details: serde_json::json!({"archived": true, "action": "archive"}),
        };
        return match format {
            Format::Json => Ok(CommandOutcome::Success(verb_json(&result), None)),
            Format::Text => {
                let msg = format!("[dry-run] would archive project: {}", args.name_or_path);
                Ok(CommandOutcome::Success(serde_json::Value::String(msg), None))
            }
        };
    }

    let report = svc::project::archive_project(root, &args.name_or_path)?;

    let result = VerbResult {
        verb: Verb::Remove,
        kind: "project",
        identity: report.name.clone(),
        old_identity: None,
        path: Some(report.to.clone()),
        dry_run: false,
        details: serde_json::json!({"archived": true, "from": report.from, "to": report.to, "action": "archive"}),
    };
    match format {
        Format::Json => Ok(CommandOutcome::Success(verb_json(&result), None)),
        Format::Text => {
            let msg = format!("✓ archived project: {}", report.name);
            Ok(CommandOutcome::Success(serde_json::Value::String(msg), None))
        }
    }
}

// ---------------------------------------------------------------------------
// Handle: mf project import
// ---------------------------------------------------------------------------

fn handle_import(
    args: ProjectImportArgs,
    repo_root: Option<&std::path::PathBuf>,
    format: Format,
) -> Result<CommandOutcome> {
    let root = repo_root.ok_or_else(MfError::not_in_mind_repo)?;
    let report = svc::project::import_project(
        root,
        &args.directory,
        args.r#type.as_deref(),
        args.source.as_deref(),
        args.assets.as_deref(),
        args.force,
        args.non_interactive,
    )?;

    match format {
        Format::Json => Ok(CommandOutcome::Success(serde_json::to_value(&report).map_err(MfError::Json)?, None)),
        Format::Text => {
            let mut lines = Vec::new();
            lines.push(format!("Imported project: {}", report.name));
            lines.push(format!("  path: {}", report.path));
            if report.scaffolded {
                lines.push("  mind.yaml: created".to_string());
            }
            lines.push(format!("  articles: {}", report.article_count));
            Ok(CommandOutcome::Raw(lines.join("\n"), None))
        }
    }
}

// ---------------------------------------------------------------------------
// Handle: mf project rename
// ---------------------------------------------------------------------------

fn handle_rename(
    args: ProjectRenameArgs,
    repo_root: Option<&std::path::PathBuf>,
    format: Format,
) -> Result<CommandOutcome> {
    let root = repo_root.ok_or_else(MfError::not_in_mind_repo)?;

    if args.dry_run {
        let result = VerbResult {
            verb: Verb::Rename,
            kind: "project",
            identity: args.new_name.clone(),
            old_identity: Some(args.old_name.clone()),
            path: None,
            dry_run: true,
            details: serde_json::json!({}),
        };
        return match format {
            Format::Json => Ok(CommandOutcome::Success(verb_json(&result), None)),
            Format::Text => Ok(CommandOutcome::Success(
                serde_json::Value::String(verb_text(&result, &VerbOpts::from_repo_root(Some(root.as_path())))),
                None,
            )),
        };
    }

    let report = svc::project::rename_project(root, &args.old_name, &args.new_name)?;

    let result = VerbResult {
        verb: Verb::Rename,
        kind: "project",
        identity: report.new_name.clone(),
        old_identity: Some(report.old_name.clone()),
        path: Some(report.to.clone()),
        dry_run: false,
        details: serde_json::json!({"from": report.from, "to": report.to}),
    };
    match format {
        Format::Json => Ok(CommandOutcome::Success(verb_json(&result), None)),
        Format::Text => Ok(CommandOutcome::Success(
            serde_json::Value::String(verb_text(&result, &VerbOpts::from_repo_root(Some(root.as_path())))),
            None,
        )),
    }
}

// ---------------------------------------------------------------------------
// Handle: mf project remove
// ---------------------------------------------------------------------------

fn handle_remove(
    args: ProjectRemoveArgs,
    repo_root: Option<&std::path::PathBuf>,
    format: Format,
) -> Result<CommandOutcome> {
    let root = repo_root.ok_or_else(MfError::not_in_mind_repo)?;

    require_confirmation(&ConfirmArgs {
        verb_label: "removal",
        kind: "project",
        identity: &args.name,
        yes: args.yes,
        force: args.force,
    })?;

    // --force with non-existent target: no-op success
    if args.force {
        let projects_dir = crate::service::repo::projects_dir_for(root)?;
        let project_path = crate::service::util::project_dir_for(root, &projects_dir, &args.name);
        if !project_path.exists() {
            let result = VerbResult {
                verb: Verb::Remove,
                kind: "project",
                identity: args.name.clone(),
                old_identity: None,
                path: None,
                dry_run: args.dry_run,
                details: serde_json::json!({"removed": false, "reason": "not found"}),
            };
            return match format {
                Format::Json => Ok(CommandOutcome::Success(verb_json(&result), None)),
                Format::Text => Ok(CommandOutcome::Success(
                    serde_json::Value::String(verb_text(&result, &VerbOpts::from_repo_root(Some(root.as_path())))),
                    None,
                )),
            };
        }
    }

    let report = svc::project::remove_project(root, &args.name, args.force, args.dry_run)?;

    let result = VerbResult {
        verb: Verb::Remove,
        kind: "project",
        identity: report.before.name.clone(),
        old_identity: None,
        path: Some(report.before.path.clone()),
        dry_run: args.dry_run,
        details: serde_json::json!({"removed": true, "path": report.before.path}),
    };
    match format {
        Format::Json => Ok(CommandOutcome::Success(verb_json(&result), None)),
        Format::Text => Ok(CommandOutcome::Success(
            serde_json::Value::String(verb_text(&result, &VerbOpts::from_repo_root(Some(root.as_path())))),
            None,
        )),
    }
}
