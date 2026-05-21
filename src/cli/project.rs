use std::path::PathBuf;

use clap::{Args, Subcommand};
use serde::Serialize;

use clap::ValueEnum;

use crate::cli::deprecation::DeprecationContext;
use crate::cli::{CommandOutcome, RepoRequirement};
use crate::error::{MfError, Result};
use crate::model::project::LintKind;
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
    pub name: String,
    #[arg(long)]
    pub template: Option<String>,
    #[arg(long)]
    pub force: bool,
}

#[derive(Debug, Clone, Args, Serialize)]
pub struct ProjectListArgs {}

#[derive(Debug, Clone, Args, Serialize)]
pub struct ProjectArchiveArgs {
    pub name_or_path: String,
}

#[derive(Debug, Clone, Args, Serialize)]
pub struct ProjectStatusArgs {
    #[arg(short = 'p', long)]
    pub project: Option<String>,
}

#[derive(Debug, Clone, Args, Serialize)]
pub struct ProjectLintArgs {
    #[arg(short = 'p', long)]
    pub project: Option<String>,
    #[arg(long)]
    pub fix: bool,
    #[arg(long = "rule")]
    pub rule: Vec<String>,
}

#[derive(Debug, Clone, Args, Serialize)]
pub struct ProjectIndexArgs {
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Debug, Clone, Args, Serialize)]
pub struct ProjectShowArgs {
    pub project: String,
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
}

#[derive(Debug, Clone, Args)]
pub struct ProjectRemoveArgs {
    pub name: String,
    #[arg(short = 'f', long)]
    pub force: bool,
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
    deprecation: &mut DeprecationContext,
) -> Result<CommandOutcome> {
    match command.command {
        None => Ok(CommandOutcome::GroupHelp("project")),
        Some(ProjectSubcommand::New(args)) => handle_new(args, repo_root, format),
        Some(ProjectSubcommand::List(args)) => handle_list(args, repo_root, format),
        Some(ProjectSubcommand::Archive(args)) => handle_archive(args, repo_root, format),
        Some(ProjectSubcommand::Status(args)) => {
            deprecation.warn_subject("project status", "project show");
            handle_status(args, repo_root, format)
        }
        Some(ProjectSubcommand::Lint(args)) => handle_lint(args, repo_root, format),
        Some(ProjectSubcommand::Index(args)) => handle_index(args, repo_root, format),
        Some(ProjectSubcommand::Show(args)) => handle_show(args, repo_root, format),
        Some(ProjectSubcommand::Import(args)) => handle_import(args, repo_root, format),
        Some(ProjectSubcommand::Rename(args)) => handle_rename(args, repo_root, format),
        Some(ProjectSubcommand::Remove(args)) => handle_remove(args, repo_root, format),
    }
}

// ---------------------------------------------------------------------------
// US1: mf project new
// ---------------------------------------------------------------------------

fn handle_new(args: ProjectNewArgs, repo_root: Option<&PathBuf>, _format: Format) -> Result<CommandOutcome> {
    let root = repo_root.ok_or_else(MfError::not_in_mind_repo)?;
    let report = svc::project::scaffold(root, &args.name, args.force)?;
    let entry = svc::project::upsert_project_entry(root, &args.name, &report.created_at)?;

    let data = serde_json::json!({
        "name": report.name,
        "path": report.project_path,
        "created_at": entry.created_at,
        "scaffolded": report.scaffolded,
    });
    Ok(CommandOutcome::Success(data, None))
}

// ---------------------------------------------------------------------------
// US2: mf project list
// ---------------------------------------------------------------------------

fn handle_list(_args: ProjectListArgs, repo_root: Option<&PathBuf>, format: Format) -> Result<CommandOutcome> {
    let root = repo_root.ok_or_else(MfError::not_in_mind_repo)?;
    let entries = svc::project::list_projects(root)?;

    match format {
        Format::Json => {
            let data = serde_json::json!({ "projects": entries });
            Ok(CommandOutcome::Success(data, None))
        }
        Format::Text => {
            if entries.is_empty() {
                return Ok(CommandOutcome::Raw("(no projects)".to_string(), None));
            }
            let mut lines = Vec::new();
            lines.push(format!("{:<8} {:>4}  {:<24}  {}", "NAME", "DOCS", "LAST ACTIVITY", "CREATED"));
            for e in &entries {
                let last_act = e.last_activity_at.as_deref().unwrap_or("-");
                lines.push(format!("{:<8} {:>4}  {:<24}  {}", e.name, e.document_count, last_act, e.created_at));
            }
            Ok(CommandOutcome::Raw(lines.join("\n"), None))
        }
    }
}

// ---------------------------------------------------------------------------
// US3: mf project status
// ---------------------------------------------------------------------------

fn handle_status(args: ProjectStatusArgs, repo_root: Option<&PathBuf>, _format: Format) -> Result<CommandOutcome> {
    let root = repo_root.ok_or_else(MfError::not_in_mind_repo)?;
    let cwd = std::env::current_dir().map_err(MfError::Io)?;
    let project_path = svc::project::resolve_project(root, args.project.as_deref(), &cwd)?;
    let snapshot = svc::project::status_for(root, &project_path)?;

    let data = serde_json::json!(snapshot);
    Ok(CommandOutcome::Success(data, None))
}

// ---------------------------------------------------------------------------
// US4: mf project lint
// ---------------------------------------------------------------------------

fn handle_lint(args: ProjectLintArgs, repo_root: Option<&PathBuf>, format: Format) -> Result<CommandOutcome> {
    let root = repo_root.ok_or_else(MfError::not_in_mind_repo)?;

    // Parse --rule args into LintKind values
    let rules: Vec<LintKind> = if args.rule.is_empty() {
        Vec::new()
    } else {
        args.rule
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

    let (issues, summary) = if let Some(ref project_name) = args.project {
        let cwd = std::env::current_dir().map_err(MfError::Io)?;
        let project_path = svc::project::resolve_project(root, Some(project_name.as_str()), &cwd)?;
        svc::project::lint_project(&project_path, &rules, args.fix)?
    } else {
        svc::project::lint_repo(root, &rules, args.fix)?
    };

    let has_errors = summary["errors"].as_u64().unwrap_or(0) > 0;
    let exit_code = if has_errors { Some(1) } else { None };

    match format {
        Format::Json => {
            let data = serde_json::json!({ "issues": issues, "summary": summary });
            Ok(CommandOutcome::Success(data, exit_code))
        }
        Format::Text => {
            if issues.is_empty() {
                return Ok(CommandOutcome::Raw("(no issues)".to_string(), exit_code));
            }
            let mut lines = Vec::new();
            for issue in &issues {
                let severity = if issue.get("fixed").and_then(|v| v.as_bool()).unwrap_or(false) {
                    "FIX"
                } else if issue.get("severity").and_then(|v| v.as_str()) == Some("error") {
                    "ERR"
                } else {
                    "WARN"
                };
                let kind = issue.get("kind").and_then(|v| v.as_str()).unwrap_or("");
                let path = issue.get("path").and_then(|v| v.as_str()).unwrap_or("");
                lines.push(format!("  {severity:<4} {kind:<20} {path}"));
            }
            let errors = summary["errors"].as_u64().unwrap_or(0);
            let warnings = summary["warnings"].as_u64().unwrap_or(0);
            let fixed = summary["fixed"].as_u64().unwrap_or(0);
            let project_count = 1;
            lines.push(format!("\n{errors} errors, {warnings} warnings across {project_count} project."));
            if fixed > 0 {
                lines.push(format!("{fixed} fixed."));
            }
            Ok(CommandOutcome::Raw(lines.join("\n"), exit_code))
        }
    }
}

// ---------------------------------------------------------------------------
// mf project index (existing, unchanged)
// ---------------------------------------------------------------------------

fn handle_index(args: ProjectIndexArgs, repo_root: Option<&PathBuf>, format: Format) -> Result<CommandOutcome> {
    // 无 repo_root 时使用 cwd（mf project index 可在 repo 外运行）
    let root = repo_root.cloned().or_else(|| std::env::current_dir().ok()).ok_or_else(MfError::not_in_mind_repo)?;

    let minds_path = root.join("minds.yaml");
    let manifest = if minds_path.exists() {
        repo::load_manifest(&minds_path)?
    } else {
        crate::model::manifest::MindsManifest::create_default()
    };

    let scanned = repo::scan_project_dirs(&root, &manifest.projects_dir);
    let diff = repo::compute_diff(&manifest, &scanned);

    if args.dry_run {
        let output = match format {
            Format::Json => serde_json::to_string_pretty(&diff).map_err(MfError::Json)?,
            Format::Text => repo::render_diff_text(&diff),
        };
        return Ok(CommandOutcome::Success(serde_json::json!({"diff": output, "dry_run": true}), None));
    }

    let updated = repo::reconcile(manifest, diff);
    repo::save_manifest(&updated, &minds_path)?;

    let payload = serde_json::json!({
        "projects_count": updated.projects.len(),
        "minds_path": minds_path.to_string_lossy().to_string(),
    });
    Ok(CommandOutcome::Success(payload, None))
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
    let project_path = svc::project::resolve_project(root, Some(&args.project), &cwd)?;
    let details = svc::project::show(&project_path, &args.project)?;

    match format {
        Format::Json => Ok(CommandOutcome::Success(serde_json::to_value(&details).map_err(MfError::Json)?, None)),
        Format::Text => {
            let mut lines = Vec::new();
            lines.push(format!("Name:           {}", details.name));
            lines.push(format!("Path:           {}", details.path));
            lines.push(format!("Articles:       {}", details.article_count));
            lines.push(format!("Sources:        {}", details.source_count));
            lines.push(format!("Assets:         {}", details.asset_count));
            lines.push(format!("Last active:    {}", details.last_active.as_deref().unwrap_or("-")));
            if let Some(summary) = details.mind_yaml_summary {
                lines.push(format!(
                    "Schema:         v{} (types: {})",
                    summary.schema_version,
                    summary.types.join(", "),
                ));
                lines.push(format!("Source dirs:    {}", summary.source_dirs.join(", ")));
                lines.push(format!("Assets dir:     {}", summary.assets_dir));
            }
            if let Some(ref layout) = details.layout {
                lines.push(String::new());
                lines.push("Layout:".to_string());
                lines.push(format!("  articles:     {}", layout.articles));
                lines.push(format!("  sources:      {}", layout.sources));
                lines.push(format!("  assets:       {}", layout.assets));
                lines.push(format!("  templates:    {}", layout.templates));
                lines.push(format!("  build_output: {}", layout.build_output));
            }
            Ok(CommandOutcome::Raw(lines.join("\n"), None))
        }
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
    let report = svc::project::archive_project(root, &args.name_or_path)?;

    match format {
        Format::Json => Ok(CommandOutcome::Success(serde_json::to_value(&report).map_err(MfError::Json)?, None)),
        Format::Text => {
            let msg = format!("Archived {}\nfrom: {}\nto: {}", report.name, report.from, report.to);
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
    let report = svc::project::rename_project(root, &args.old_name, &args.new_name)?;

    match format {
        Format::Json => Ok(CommandOutcome::Success(serde_json::to_value(&report).map_err(MfError::Json)?, None)),
        Format::Text => {
            let msg = format!(
                "Renamed {} → {}\n  from: {}\n  to: {}",
                report.old_name, report.new_name, report.from, report.to
            );
            Ok(CommandOutcome::Raw(msg, None))
        }
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
    let report = svc::project::remove_project(root, &args.name, args.force, args.dry_run)?;

    match format {
        Format::Json => Ok(CommandOutcome::Success(serde_json::to_value(&report).map_err(MfError::Json)?, None)),
        Format::Text => {
            let prefix = if report.dry_run { "[dry-run] would remove" } else { "✓ removed" };
            let msg = format!("{} project: {} ({})", prefix, report.before.name, report.before.path);
            Ok(CommandOutcome::Success(serde_json::Value::String(msg), None))
        }
    }
}
