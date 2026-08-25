//! RAG-backed Source CLI handlers.

use clap::Args;

use crate::cli::CommandCtx;
use crate::cli::CommandOutcome;
use crate::cli::shared_flags::DryRunFlag;
use crate::error::Result;
use crate::service as svc;

// ── sync ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Args)]
pub struct AdvancedSyncArgs {
    /// Limit sync to a specific project
    #[arg(short = 'p', long)]
    pub project: Option<String>,
    /// Limit sync to a specific Source identity (requires unambiguous scope)
    #[arg(long)]
    pub source: Option<String>,
    /// Forbid all network access (Web/RSS acquisition disabled)
    #[arg(long)]
    pub offline: bool,
    /// Regenerate the Lance Source index to the current storage schema (full
    /// re-index/re-embed). Recovers from a schema-drift refusal in one command
    /// family instead of detouring to `source admin rebuild`.
    #[arg(long)]
    pub rebuild: bool,
    #[command(flatten)]
    pub dry_run: DryRunFlag,
}

// ── status ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Args)]
pub struct AdvancedStatusArgs {
    #[arg(short = 'p', long)]
    pub project: Option<String>,
    #[arg(long)]
    pub source: Option<String>,
}

// ── rebuild ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Args)]
pub struct AdvancedRebuildArgs {
    #[arg(long)]
    pub offline: bool,
    #[command(flatten)]
    pub dry_run: DryRunFlag,
}

// ── clear ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Args)]
pub struct AdvancedClearArgs {
    pub source: Option<String>,
    #[arg(short = 'p', long)]
    pub project: Option<String>,
    #[arg(long)]
    pub all: bool,
    #[command(flatten)]
    pub dry_run: DryRunFlag,
    #[command(flatten)]
    pub yes: crate::cli::shared_flags::YesFlag,
}

// ── recover ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Args)]
pub struct AdvancedRecoverArgs {
    #[arg(long)]
    pub snapshot: String,
    #[command(flatten)]
    pub dry_run: DryRunFlag,
    #[command(flatten)]
    pub yes: crate::cli::shared_flags::YesFlag,
}

// ── export ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Args)]
pub struct AdvancedExportArgs {
    /// Destination bundle directory
    #[arg(long = "output-dir", value_name = "DIR")]
    pub output: String,
    /// Allow writing into an existing/non-empty output directory
    #[arg(long)]
    pub force: bool,
    #[command(flatten)]
    pub dry_run: DryRunFlag,
}

// ── import ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Args)]
pub struct AdvancedImportArgs {
    /// Bundle directory produced by `export`
    pub bundle: String,
    /// Required when the target already has a published corpus
    #[arg(long)]
    pub overwrite: bool,
    #[command(flatten)]
    pub dry_run: DryRunFlag,
}

// ── trace ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Args)]
pub struct AdvancedTraceArgs {
    /// Limit to one project
    #[arg(short = 'p', long)]
    pub project: Option<String>,
}

pub fn handle_sync(args: AdvancedSyncArgs, ctx: &mut CommandCtx) -> Result<CommandOutcome> {
    let repo = ctx.require_repo_path()?;
    let mut config = svc::source::advanced::config::load_repository_config(repo)?;
    let dry_run = args.dry_run.dry_run;

    // `--rebuild` is the explicit schema-recovery path (spec 074 #33): regenerate
    // the index to the current storage schema, bypassing the drift refusal below.
    // It reuses the admin-rebuild sequence so there is no second rebuild impl.
    if args.rebuild {
        let report = svc::source::advanced::sync::rebuild_repository(repo, &config, dry_run, args.offline)?;
        // Schema compatibility is read from the tables themselves (FR-002), and
        // the migration inside `sync_repository` already ran unconditionally on
        // every non-dry-run rebuild. There is nothing left to gate on
        // `registrations_failed` (spec 075 dissolves #36).
        let json = serde_json::to_value(&report).unwrap_or_default();
        let mut warnings: Vec<String> = if report.registrations_failed > 0 {
            vec![format!(
                "{} of {} registration(s) failed rebuild; fix them and re-run `mf source sync --rebuild` to refresh their derived context",
                report.registrations_failed, report.registrations_total
            )]
        } else {
            vec![]
        };
        if !dry_run {
            warnings.push("full re-index (re-embed) ran; storage schema updated".to_string());
        }
        return Ok(CommandOutcome::Success(
            serde_json::json!({"status": "ok", "command": "source.sync", "data": json}),
            warnings,
            None,
        ));
    }

    // An out-of-date snapshot must be rebuilt before incremental sync; fresh
    // activation below writes the current schema, so gate only existing indexes.
    config.require_current_schema(repo)?;

    if !config.is_lance() {
        if dry_run {
            let preview = svc::source::advanced::activation::preview_activation(repo, &config)?;
            let json = serde_json::to_value(&preview).unwrap_or_default();
            return Ok(CommandOutcome::Success(
                serde_json::json!({"status":"ok","command":"source.sync","data":{"activation":json,"dry_run":true}}),
                vec![],
                None,
            ));
        }
        svc::source::advanced::activation::activate(repo, &config)?;
        config = svc::source::advanced::config::load_repository_config(repo)?;
    } else if !dry_run && !config.activated_here {
        // The corpus pointer already resolved (e.g. shared by another
        // worktree, or this machine's state was lost and self-healed) without
        // this machine ever running `activate()`. Record activation here too,
        // so `status` reports this machine accurately from now on (FR-001)
        // — purely informational, never a precondition for anything above.
        crate::service::repo::save_local_state(repo, &crate::model::manifest::LocalSourceState { activated: true })?;
    }

    let report = svc::source::advanced::sync::sync_repository(
        repo,
        &config,
        args.project.as_deref(),
        args.source.as_deref(),
        dry_run,
        args.offline,
    )?;

    let json = serde_json::to_value(&report).unwrap_or_default();
    let warnings: Vec<String> = if report.registrations_failed > 0 {
        vec![format!("{} of {} registration(s) failed sync", report.registrations_failed, report.registrations_total)]
    } else {
        vec![]
    };

    Ok(CommandOutcome::Success(
        serde_json::json!({"status": "ok", "command": "source.sync", "data": json}),
        warnings,
        None,
    ))
}

pub fn handle_status(_args: AdvancedStatusArgs, ctx: &mut CommandCtx) -> Result<CommandOutcome> {
    let repo = ctx.require_repo_path()?;
    let config = svc::source::advanced::config::load_repository_config(repo)?;
    let report = svc::source::advanced::status::build_status(repo, &config)?;
    let json = serde_json::to_value(&report).unwrap_or_default();
    let warnings = report.warnings.clone();
    Ok(CommandOutcome::Success(
        serde_json::json!({"status": "ok", "command": "source.status", "data": json}),
        warnings,
        None,
    ))
}

pub fn handle_rebuild(args: AdvancedRebuildArgs, ctx: &mut CommandCtx) -> Result<CommandOutcome> {
    let repo = ctx.require_repo_path()?;
    let config = svc::source::advanced::config::load_repository_config(repo)?;
    let report = svc::source::advanced::sync::rebuild_repository(repo, &config, args.dry_run.dry_run, args.offline)?;
    // Rebuild is the schema migration path: the table migration inside
    // `sync_repository` already ran unconditionally, and schema compatibility
    // is read from the tables themselves — nothing to gate here (spec 075
    // dissolves #36).
    let json = serde_json::to_value(&report).unwrap_or_default();
    let warnings: Vec<String> = if report.registrations_failed > 0 {
        vec![format!(
            "{} of {} registration(s) failed rebuild; fix them and re-run `mf source sync --rebuild` to refresh their derived context",
            report.registrations_failed, report.registrations_total
        )]
    } else {
        vec![]
    };
    Ok(CommandOutcome::Success(
        serde_json::json!({"status": "ok", "command": "source.admin.rebuild", "data": json}),
        warnings,
        None,
    ))
}

pub fn handle_clear(args: AdvancedClearArgs, ctx: &mut CommandCtx) -> Result<CommandOutcome> {
    let repo = ctx.require_repo_path()?;
    if !args.yes.yes && !args.dry_run.dry_run {
        return Err(crate::error::MfError::usage(
            "clear requires --yes for real mutation".to_string(),
            Some("use --dry-run to preview, then --yes to execute".to_string()),
        ));
    }
    let config = svc::source::advanced::config::load_repository_config(repo)?;
    let report = svc::source::advanced::sync::clear_derived(
        repo,
        &config,
        args.project.as_deref(),
        args.source.as_deref(),
        args.all,
        args.dry_run.dry_run,
    )?;
    let json = serde_json::to_value(&report).unwrap_or_default();
    Ok(CommandOutcome::Success(
        serde_json::json!({"status": "ok", "command": "source.admin.clear", "data": json}),
        vec![],
        None,
    ))
}

pub fn handle_recover(args: AdvancedRecoverArgs, ctx: &mut CommandCtx) -> Result<CommandOutcome> {
    let repo = ctx.require_repo_path()?;
    if !args.yes.yes && !args.dry_run.dry_run {
        return Err(crate::error::MfError::usage(
            "recover requires --yes for real mutation".to_string(),
            Some("use --dry-run to preview, then --yes to execute".to_string()),
        ));
    }
    let advanced_dir = svc::source::advanced::advanced_store_dir(repo);
    let pointer =
        svc::source::advanced::sync::recover_from_snapshot(&advanced_dir, &args.snapshot, args.dry_run.dry_run)?;
    let json = serde_json::to_value(&pointer).unwrap_or_default();
    Ok(CommandOutcome::Success(
        serde_json::json!({"status": "ok", "command": "source.admin.recover", "data": json}),
        vec![],
        None,
    ))
}

pub fn handle_export(args: AdvancedExportArgs, ctx: &mut CommandCtx) -> Result<CommandOutcome> {
    let repo = ctx.require_repo_path()?;
    let output = std::path::Path::new(&args.output);
    let report = svc::source::advanced::export::export_bundle(repo, output, args.force, args.dry_run.dry_run)?;
    let json = serde_json::to_value(&report).unwrap_or_default();
    Ok(CommandOutcome::Success(
        serde_json::json!({"status": "ok", "command": "source.export", "data": json}),
        vec![],
        None,
    ))
}

pub fn handle_import(args: AdvancedImportArgs, ctx: &mut CommandCtx) -> Result<CommandOutcome> {
    let repo = ctx.require_repo_path()?;
    let bundle = std::path::Path::new(&args.bundle);
    let report = svc::source::advanced::import::import_bundle(repo, bundle, args.overwrite, args.dry_run.dry_run)?;
    let json = serde_json::to_value(&report).unwrap_or_default();
    Ok(CommandOutcome::Success(
        serde_json::json!({"status": "ok", "command": "source.import", "data": json}),
        vec![],
        None,
    ))
}

pub fn handle_trace(args: AdvancedTraceArgs, ctx: &mut CommandCtx) -> Result<CommandOutcome> {
    let repo = ctx.require_repo_path()?;
    let links = svc::source::advanced::trace::trace_links(repo, args.project.as_deref())?;
    let json = serde_json::to_value(&links).unwrap_or_default();
    Ok(CommandOutcome::Success(
        serde_json::json!({"status": "ok", "command": "source.trace", "data": {"links": json}}),
        vec![],
        None,
    ))
}
