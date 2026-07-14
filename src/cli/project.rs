use clap::{Args, Subcommand};
use serde::Serialize;

use clap::ValueEnum;

use crate::cli::CommandCtx;
use crate::cli::shared_flags::DryRunFlag;
use crate::cli::shared_flags::ForceFlag;
use crate::cli::shared_flags::LintFlags;
use crate::cli::shared_flags::NoHeadersFlag;
use crate::cli::shared_flags::NoTruncFlag;
use crate::cli::shared_flags::YesFlag;
use crate::cli::{CommandOutcome, RepoRequirement};
use crate::error::{MfError, Result};
use crate::model::Resource;
use crate::model::project::LintKind;
use crate::output::Format;
use crate::output::confirm::{ConfirmArgs, require_confirmation};
use crate::output::list::{ListCell, ListOpts, ListRow, ListView, json_collection, render_text};
use crate::output::show::{
    ShowBlock, ShowField, ShowOpts, ShowSection, ShowValue, json_envelope, render_text as render_show_text,
};
use crate::output::verb::{Verb, VerbOpts, VerbResult, json_envelope as verb_json, render_text as verb_text};
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
    #[command(about = "Lint a project")]
    Lint(ProjectLintArgs),
    #[command(about = "Index projects")]
    Index(ProjectIndexArgs),
    #[command(about = "Show project details")]
    Show(ProjectShowArgs),
    #[command(about = "Update project metadata")]
    Update(ProjectUpdateArgs),
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
    #[command(flatten)]
    pub force: ForceFlag,
    #[command(flatten)]
    pub dry_run: DryRunFlag,
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
    #[command(flatten)]
    pub force: ForceFlag,
    #[command(flatten)]
    pub yes: YesFlag,
    #[command(flatten)]
    pub dry_run: DryRunFlag,
}

#[derive(Debug, Clone, Args, Serialize)]
pub struct ProjectLintArgs {
    #[command(flatten)]
    pub lint: LintFlags,
}

#[derive(Debug, Clone, Args, Serialize)]
pub struct ProjectIndexArgs {
    #[command(flatten)]
    pub dry_run: DryRunFlag,
}

#[derive(Debug, Clone, Args, Serialize)]
pub struct ProjectShowArgs {
    pub path: String,
}

#[derive(Debug, Clone, Args, Serialize)]
pub struct ProjectUpdateArgs {
    pub path: String,
    #[arg(long)]
    pub description: Option<String>,
    #[arg(long = "clear-description")]
    pub clear_description: bool,
    #[command(flatten)]
    pub dry_run: DryRunFlag,
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
    #[command(flatten)]
    pub force: ForceFlag,
    #[command(flatten)]
    pub yes: YesFlag,
}

#[derive(Debug, Clone, Args, Serialize)]
pub struct ProjectRenameArgs {
    pub old_path: String,
    pub new_path: String,
    #[command(flatten)]
    pub force: ForceFlag,
    #[command(flatten)]
    pub dry_run: DryRunFlag,
}

#[derive(Debug, Clone, Args)]
pub struct ProjectRemoveArgs {
    pub path: String,
    #[command(flatten)]
    pub force: ForceFlag,
    #[command(flatten)]
    pub yes: YesFlag,
    #[command(flatten)]
    pub dry_run: DryRunFlag,
}

impl ProjectCmd {
    pub fn requires_repo(&self) -> RepoRequirement {
        match self.command {
            Some(ProjectSubcommand::Index(_)) => RepoRequirement::NotRequired,
            _ => RepoRequirement::Required,
        }
    }
}

pub fn dispatch(command: ProjectCmd, ctx: &mut CommandCtx) -> Result<CommandOutcome> {
    match command.command {
        None => Ok(CommandOutcome::GroupHelp("project")),
        Some(ProjectSubcommand::New(args)) => handle_new(args, ctx),
        Some(ProjectSubcommand::List(args)) => handle_list(args, ctx),
        Some(ProjectSubcommand::Archive(args)) => handle_archive(args, ctx),
        Some(ProjectSubcommand::Lint(args)) => handle_lint(args, ctx),
        Some(ProjectSubcommand::Index(args)) => handle_index(args, ctx),
        Some(ProjectSubcommand::Show(args)) => handle_show(args, ctx),
        Some(ProjectSubcommand::Update(args)) => handle_update(args, ctx),
        Some(ProjectSubcommand::Import(args)) => handle_import(args, ctx),
        Some(ProjectSubcommand::Rename(args)) => handle_rename(args, ctx),
        Some(ProjectSubcommand::Remove(args)) => handle_remove(args, ctx),
    }
}

// ---------------------------------------------------------------------------
// US3: mf project new
// ---------------------------------------------------------------------------

fn handle_new(args: ProjectNewArgs, ctx: &CommandCtx) -> Result<CommandOutcome> {
    let root = ctx.require_repo_path()?;
    let format = ctx.format();
    let cwd = ctx.cwd();

    // Normalize the user-provided selector into a canonical project identity
    let identity = svc::identity::normalize_project_selector(root, &args.path, cwd)?;

    // Reject if the resolved path is inside another existing project (nested).
    if let Some(parent) = identity.resolved_path.parent()
        && let Some(parent_project) = svc::util::detect_current_project(root, parent)
    {
        return Err(MfError::usage(
            format!("project path '{}' is inside another project '{}'", identity.path, parent_project),
            Some("create the project outside the existing project root".to_string()),
        ));
    }

    if args.dry_run.dry_run {
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
            Format::Json => Ok(CommandOutcome::Success(verb_json(&result), Vec::new(), None)),
            Format::Text => Ok(CommandOutcome::Success(
                serde_json::Value::String(verb_text(&result, &VerbOpts::from_repo_root(Some(root.as_path())))),
                Vec::new(),
                None,
            )),
        };
    }

    let report = svc::project::scaffold(
        root,
        &identity.requested_path,
        &identity.path,
        &identity.resolved_path,
        args.force.force,
    )?;
    let entry = svc::project::upsert_project_entry(root, &identity.path, &report.created_at, args.force.force)?;

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
        Format::Json => Ok(CommandOutcome::Success(verb_json(&result), Vec::new(), None)),
        Format::Text => Ok(CommandOutcome::Success(
            serde_json::Value::String(verb_text(&result, &VerbOpts::from_repo_root(Some(root.as_path())))),
            Vec::new(),
            None,
        )),
    }
}

// ---------------------------------------------------------------------------
// US2: mf project list
// ---------------------------------------------------------------------------

fn handle_list(args: ProjectListArgs, ctx: &CommandCtx) -> Result<CommandOutcome> {
    let root = ctx.require_repo_path()?;
    let format = ctx.format();
    let entries = svc::project::list_projects(root)?;

    let opts =
        ListOpts::from_flags(args.no_headers.no_headers, args.no_trunc.no_trunc).with_repo_root(Some(root.clone()));

    match format {
        Format::Json => {
            let items: Vec<serde_json::Value> = entries
                .iter()
                .map(|e| {
                    let path = e.path.strip_prefix("./").unwrap_or(&e.path);
                    serde_json::json!({
                        "identity": e.identity(),
                        "name": e.name,
                        "path": path,
                        "created_at": e.created_at,
                        "archived_at": e.archived_at,
                        "document_count": e.document_count,
                        "last_activity_at": e.last_activity_at,
                    })
                })
                .collect();
            Ok(CommandOutcome::Success(json_collection("projects", items), Vec::new(), None))
        }
        Format::Text => {
            let mut rows = Vec::with_capacity(entries.len());
            for e in &entries {
                let status = if e.archived_at.is_some() { "archived" } else { "active" };
                let created_at = format_iso_date(&e.created_at);
                let updated_at = format_iso_date(e.last_activity_at.as_deref().unwrap_or(""));
                let path = e.path.strip_prefix("./").unwrap_or(&e.path);
                rows.push(ListRow {
                    cells: vec![
                        ListCell::Path(path.to_string()),
                        ListCell::Text(e.name.clone()),
                        ListCell::Number(e.document_count.to_string()),
                        status_cell(status),
                        ListCell::Text(updated_at),
                        ListCell::Text(created_at),
                    ],
                });
            }
            let view = ListView {
                headers: &["PATH", "NAME", "ITEMS", "STATUS", "UPDATED", "CREATED"],
                rows,
                plural_noun: "projects",
            };
            Ok(CommandOutcome::Raw(render_text(&view, &opts), None))
        }
    }
}

fn status_cell(status: &str) -> ListCell {
    match status {
        "archived" => ListCell::Styled { text: "archived".to_string(), ansi_prefix: "\x1b[2m", ansi_suffix: "" },
        _ => ListCell::Styled { text: "active".to_string(), ansi_prefix: "\x1b[32m", ansi_suffix: "" },
    }
}

fn format_iso_date(iso: &str) -> String {
    if iso.is_empty() {
        return "-".to_string();
    }
    chrono::DateTime::parse_from_rfc3339(iso)
        .map(|dt| dt.with_timezone(&chrono::Utc).format("%Y-%m-%d %H:%M").to_string())
        .or_else(|_| {
            chrono::NaiveDateTime::parse_from_str(iso, "%Y-%m-%dT%H:%M:%SZ")
                .map(|dt| dt.and_utc().format("%Y-%m-%d %H:%M").to_string())
        })
        .unwrap_or_else(|_| "-".to_string())
}

// ---------------------------------------------------------------------------
// US4: mf project lint
// ---------------------------------------------------------------------------

fn handle_lint(args: ProjectLintArgs, ctx: &CommandCtx) -> Result<CommandOutcome> {
    let root = ctx.require_repo_path()?;
    let format = ctx.format();
    let project = ctx.project();

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
                            "available rules: missing_directory, stale_index_entry, name_convention, missing_manifest, duplicate_key, orphan_prompt, duplicate_binding, missing_thinking"
                                .to_string(),
                        ),
                    )
                })
            })
            .collect::<std::result::Result<Vec<_>, _>>()?
    };

    let (mut issues, mut summary) = if let Some(project_name) = project {
        let cwd = ctx.cwd();
        let project_path = svc::project::resolve_project(root, Some(project_name), cwd)?;
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
            Ok(CommandOutcome::Success(data, Vec::new(), exit_code))
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

fn handle_index(args: ProjectIndexArgs, ctx: &CommandCtx) -> Result<CommandOutcome> {
    let root = ctx.repo_root().cloned().or_else(|| Some(ctx.cwd().clone())).ok_or_else(MfError::not_in_mind_repo)?;
    let format = ctx.format();

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

    // FR-009: also reconcile each on-disk project's doc-article index so a single
    // `mf project index` prunes stale per-project article entries (references to
    // files that no longer exist), consistent with `mf article index`.
    let mut article_added: Vec<serde_json::Value> = Vec::new();
    let mut article_removed: Vec<serde_json::Value> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();
    for sp in &scanned {
        let project_path = root.join(&sp.path);
        match svc::article::reconcile_project_docs(&project_path, args.dry_run.dry_run) {
            Ok((added, removed)) => {
                for p in added {
                    article_added.push(serde_json::json!({"identity": p.clone(), "path": p, "project": sp.name}));
                }
                for p in removed {
                    article_removed.push(serde_json::json!({"identity": p.clone(), "path": p, "project": sp.name}));
                }
            }
            Err(e) => {
                // Surface per-project reconcile failures instead of skipping
                // silently: a corrupt mind-index.yaml should be visible to the
                // operator (stderr) and to agents (JSON data.warnings).
                let message = format!("skipped article reconcile for project '{}': {e}", sp.name);
                eprintln!("warning: {message}");
                warnings.push(message);
            }
        }
    }

    if args.dry_run.dry_run {
        let mut added: Vec<serde_json::Value> =
            diff.added.iter().map(|p| serde_json::to_value(p).unwrap_or_default()).collect();
        added.extend(article_added.clone());
        let mut removed: Vec<serde_json::Value> =
            diff.removed.iter().map(|p| serde_json::to_value(p).unwrap_or_default()).collect();
        removed.extend(article_removed.clone());
        let details = serde_json::json!({
            "added": added,
            "removed": removed,
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
            Format::Json => Ok(CommandOutcome::Success(verb_json(&result), warnings, None)),
            Format::Text => Ok(CommandOutcome::Success(
                serde_json::Value::String(verb_text(&result, &VerbOpts::from_repo_root(Some(root.as_path())))),
                warnings,
                None,
            )),
        };
    }

    let updated = repo::reconcile(manifest, diff);
    repo::save_manifest(&updated, &minds_path)?;

    let mut added: Vec<serde_json::Value> =
        updated.projects.iter().map(|p| serde_json::json!({"identity": p.name, "path": p.path})).collect();
    added.extend(article_added);
    let details = serde_json::json!({
        "added": added,
        "removed": article_removed,
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
        Format::Json => Ok(CommandOutcome::Success(verb_json(&result), warnings, None)),
        Format::Text => Ok(CommandOutcome::Success(
            serde_json::Value::String(verb_text(&result, &VerbOpts::from_repo_root(Some(root.as_path())))),
            warnings,
            None,
        )),
    }
}

// ---------------------------------------------------------------------------
// Handle: mf project show
// ---------------------------------------------------------------------------

fn handle_show(args: ProjectShowArgs, ctx: &CommandCtx) -> Result<CommandOutcome> {
    let root = ctx.require_repo_path()?;
    let format = ctx.format();
    let cwd = ctx.cwd();
    let project_path = svc::project::resolve_project(root, Some(&args.path), cwd)?;
    let details = svc::project::show(&project_path, &args.path)?;

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
            Ok(CommandOutcome::Success(json_envelope(&block, extra), Vec::new(), None))
        }
        Format::Text => Ok(CommandOutcome::Raw(render_show_text(&block, &ShowOpts::from_repo_root(Some(root))), None)),
    }
}

// ---------------------------------------------------------------------------
// Handle: mf project archive
// ---------------------------------------------------------------------------

fn handle_archive(args: ProjectArchiveArgs, ctx: &CommandCtx) -> Result<CommandOutcome> {
    let root = ctx.require_repo_path()?;
    let format = ctx.format();

    require_confirmation(&ConfirmArgs {
        verb_label: "archiving",
        kind: "project",
        identity: &args.name_or_path,
        yes: args.yes.yes,
        force: args.force.force,
    })?;

    // --force with non-existent target: no-op success
    if args.force.force {
        let projects_dir = crate::service::repo::projects_dir_for(root)?;
        let project_path = crate::service::util::project_dir_for(root, &projects_dir, &args.name_or_path);
        if !project_path.exists() {
            let result = VerbResult {
                verb: Verb::Remove,
                kind: "project",
                identity: args.name_or_path.clone(),
                old_identity: None,
                path: None,
                dry_run: args.dry_run.dry_run,
                details: serde_json::json!({"archived": false, "reason": "not found"}),
            };
            return match format {
                Format::Json => Ok(CommandOutcome::Success(verb_json(&result), Vec::new(), None)),
                Format::Text => Ok(CommandOutcome::Success(
                    serde_json::Value::String(verb_text(&result, &VerbOpts::from_repo_root(Some(root.as_path())))),
                    Vec::new(),
                    None,
                )),
            };
        }
    }

    if args.dry_run.dry_run {
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
            Format::Json => Ok(CommandOutcome::Success(verb_json(&result), Vec::new(), None)),
            Format::Text => {
                let msg = format!("[dry-run] would archive project: {}", args.name_or_path);
                Ok(CommandOutcome::Success(serde_json::Value::String(msg), Vec::new(), None))
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
        Format::Json => Ok(CommandOutcome::Success(verb_json(&result), Vec::new(), None)),
        Format::Text => {
            let msg = format!("✓ archived project: {}", report.name);
            Ok(CommandOutcome::Success(serde_json::Value::String(msg), Vec::new(), None))
        }
    }
}

// ---------------------------------------------------------------------------
// Handle: mf project import
// ---------------------------------------------------------------------------

fn handle_import(args: ProjectImportArgs, ctx: &CommandCtx) -> Result<CommandOutcome> {
    let root = ctx.require_repo_path()?;
    let format = ctx.format();
    let report = svc::project::import_project(
        root,
        &args.directory,
        args.r#type.as_deref(),
        args.source.as_deref(),
        args.assets.as_deref(),
        args.force.force,
        args.yes.yes,
    )?;

    match format {
        Format::Json => {
            Ok(CommandOutcome::Success(serde_json::to_value(&report).map_err(MfError::Json)?, Vec::new(), None))
        }
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

fn handle_update(args: ProjectUpdateArgs, ctx: &CommandCtx) -> Result<CommandOutcome> {
    let root = ctx.require_repo_path()?;
    let format = ctx.format();

    let report = svc::project::update_project(
        root,
        svc::project::ProjectUpdate {
            name: &args.path,
            description: args.description.as_deref(),
            clear_description: args.clear_description,
            dry_run: args.dry_run.dry_run,
        },
    )?;

    let result = VerbResult {
        verb: Verb::Update,
        kind: "project",
        identity: report.name.clone(),
        old_identity: None,
        path: Some(report.path.clone()),
        dry_run: report.dry_run,
        details: serde_json::json!({"changes": report.changes}),
    };

    match format {
        Format::Json => Ok(CommandOutcome::Success(verb_json(&result), Vec::new(), None)),
        Format::Text => Ok(CommandOutcome::Success(
            serde_json::Value::String(verb_text(&result, &VerbOpts::from_repo_root(Some(root.as_path())))),
            Vec::new(),
            None,
        )),
    }
}

// ---------------------------------------------------------------------------
// Handle: mf project rename
// ---------------------------------------------------------------------------

fn handle_rename(args: ProjectRenameArgs, ctx: &CommandCtx) -> Result<CommandOutcome> {
    let root = ctx.require_repo_path()?;
    let format = ctx.format();

    if args.dry_run.dry_run {
        let result = VerbResult {
            verb: Verb::Rename,
            kind: "project",
            identity: args.new_path.clone(),
            old_identity: Some(args.old_path.clone()),
            path: None,
            dry_run: true,
            details: serde_json::json!({}),
        };
        return match format {
            Format::Json => Ok(CommandOutcome::Success(verb_json(&result), Vec::new(), None)),
            Format::Text => Ok(CommandOutcome::Success(
                serde_json::Value::String(verb_text(&result, &VerbOpts::from_repo_root(Some(root.as_path())))),
                Vec::new(),
                None,
            )),
        };
    }

    let report = svc::project::rename_project(root, &args.old_path, &args.new_path)?;

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
        Format::Json => Ok(CommandOutcome::Success(verb_json(&result), Vec::new(), None)),
        Format::Text => Ok(CommandOutcome::Success(
            serde_json::Value::String(verb_text(&result, &VerbOpts::from_repo_root(Some(root.as_path())))),
            Vec::new(),
            None,
        )),
    }
}

// ---------------------------------------------------------------------------
// Handle: mf project remove
// ---------------------------------------------------------------------------

fn handle_remove(args: ProjectRemoveArgs, ctx: &CommandCtx) -> Result<CommandOutcome> {
    let root = ctx.require_repo_path()?;
    let format = ctx.format();

    require_confirmation(&ConfirmArgs {
        verb_label: "removal",
        kind: "project",
        identity: &args.path,
        yes: args.yes.yes,
        force: args.force.force,
    })?;

    // --force with non-existent target: no-op success
    if args.force.force {
        let projects_dir = crate::service::repo::projects_dir_for(root)?;
        let project_path = crate::service::util::project_dir_for(root, &projects_dir, &args.path);
        if !project_path.exists() {
            let result = VerbResult {
                verb: Verb::Remove,
                kind: "project",
                identity: args.path.clone(),
                old_identity: None,
                path: None,
                dry_run: args.dry_run.dry_run,
                details: serde_json::json!({"removed": false, "reason": "not found"}),
            };
            return match format {
                Format::Json => Ok(CommandOutcome::Success(verb_json(&result), Vec::new(), None)),
                Format::Text => Ok(CommandOutcome::Success(
                    serde_json::Value::String(verb_text(&result, &VerbOpts::from_repo_root(Some(root.as_path())))),
                    Vec::new(),
                    None,
                )),
            };
        }
    }

    let report = svc::project::remove_project(root, &args.path, args.force.force, args.dry_run.dry_run)?;

    let result = VerbResult {
        verb: Verb::Remove,
        kind: "project",
        identity: report.before.name.clone(),
        old_identity: None,
        path: Some(report.before.path.clone()),
        dry_run: args.dry_run.dry_run,
        details: serde_json::json!({"removed": true, "path": report.before.path}),
    };
    match format {
        Format::Json => Ok(CommandOutcome::Success(verb_json(&result), Vec::new(), None)),
        Format::Text => Ok(CommandOutcome::Success(
            serde_json::Value::String(verb_text(&result, &VerbOpts::from_repo_root(Some(root.as_path())))),
            Vec::new(),
            None,
        )),
    }
}
