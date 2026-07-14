//! Advanced Source CLI — LanceDB-backed repository-level Source commands.
//!
//! Wired under `mf source advanced ...`. These commands manage the
//! repository-global LanceDB Source catalog, content sync, model
//! installation, enrichment workflow, and Skill bundle installation.

use clap::{Args, Subcommand};

use crate::cli::CommandCtx;
use crate::cli::CommandOutcome;
use crate::cli::shared_flags::DryRunFlag;
use crate::error::Result;
use crate::service as svc;

#[derive(Debug, Clone, Args)]
pub struct SourceAdvancedCmd {
    #[command(subcommand)]
    pub command: SourceAdvancedSubcommand,
}

#[derive(Debug, Clone, Subcommand)]
pub enum SourceAdvancedSubcommand {
    /// Activate LanceDB-backed Sources (imports all legacy registrations)
    Enable(AdvancedEnableArgs),
    /// Manage the local embedding model bundle
    #[command(subcommand)]
    Model(ModelCmd),
    /// Reconcile content for all or selected Source registrations
    Sync(AdvancedSyncArgs),
    /// List, show, or apply Claude enrichment for shared Source content
    #[command(subcommand)]
    Enrich(EnrichCmd),
    /// Install the /mf-source Claude Code Skill into this repo
    SkillInstall(AdvancedSkillInstallArgs),
    /// Report aggregate status of the advanced Source index
    Status(AdvancedStatusArgs),
    /// Rebuild the entire LanceDB index from all live registrations
    Rebuild(AdvancedRebuildArgs),
    /// Clear derived content (documents, chunks, enrichments)
    Clear(AdvancedClearArgs),
    /// Recover the LanceDB pointer from a retained snapshot
    Recover(AdvancedRecoverArgs),
    /// Manage legacy compatibility projections
    #[command(subcommand)]
    Legacy(LegacyCmd),
    /// Disable Lance backend and switch to legacy mode
    Disable(AdvancedDisableArgs),
}

// ── enable ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Args)]
pub struct AdvancedEnableArgs {
    /// Model ID for embeddings (default: intfloat/multilingual-e5-small)
    #[arg(long)]
    pub model: Option<String>,
    /// Model revision (default: main)
    #[arg(long)]
    pub model_revision: Option<String>,
    /// Path to a pre-installed model bundle
    #[arg(long)]
    pub model_path: Option<String>,
    #[command(flatten)]
    pub dry_run: DryRunFlag,
}

// ── model ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Subcommand)]
pub enum ModelCmd {
    /// Download and install the embedding model (requires network)
    Install(ModelInstallArgs),
    /// Import a local model bundle (network-free)
    Import(ModelImportArgs),
    /// Report model installation status (read-only)
    Status(ModelStatusArgs),
}

#[derive(Debug, Clone, Args)]
pub struct ModelInstallArgs {
    /// Model ID
    #[arg(long)]
    pub model: Option<String>,
    /// Model revision
    #[arg(long)]
    pub model_revision: Option<String>,
    #[command(flatten)]
    pub dry_run: DryRunFlag,
}

#[derive(Debug, Clone, Args)]
pub struct ModelImportArgs {
    /// Path to the local model bundle directory
    pub bundle_dir: String,
    #[command(flatten)]
    pub dry_run: DryRunFlag,
}

#[derive(Debug, Clone, Args)]
pub struct ModelStatusArgs {
    /// Model ID to check
    #[arg(long)]
    pub model: Option<String>,
}

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
    #[command(flatten)]
    pub dry_run: DryRunFlag,
}

// ── enrich ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Subcommand)]
pub enum EnrichCmd {
    /// List pending/stale enrichment jobs
    List(EnrichListArgs),
    /// Show one bounded chunk batch for a document (for Claude enrichment)
    Show(EnrichShowArgs),
    /// Apply a validated enrichment JSON to a document
    Apply(EnrichApplyArgs),
}

#[derive(Debug, Clone, Args)]
pub struct EnrichListArgs {
    /// Filter by enrichment state
    #[arg(long)]
    pub state: Option<String>,
    /// Maximum number of jobs to return
    #[arg(long, default_value = "50")]
    pub limit: u32,
}

#[derive(Debug, Clone, Args)]
pub struct EnrichShowArgs {
    /// Document key (SHA-256 hex)
    pub document_key: String,
    /// Batch size (number of chunks per batch)
    #[arg(long, default_value = "8")]
    pub batch: u32,
}

#[derive(Debug, Clone, Args)]
pub struct EnrichApplyArgs {
    /// Document key (SHA-256 hex)
    pub document_key: String,
    /// Path to the enrichment JSON file
    #[arg(long)]
    pub input: String,
    #[command(flatten)]
    pub dry_run: DryRunFlag,
}

// ── skill install ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Args)]
pub struct AdvancedSkillInstallArgs {
    /// Force overwrite of existing conflicting Skill files
    #[arg(long)]
    pub force: bool,
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

// ── legacy ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Subcommand)]
pub enum LegacyCmd {
    Status(LegacyStatusArgs),
    Export(LegacyExportArgs),
    Import(LegacyImportArgs),
}

#[derive(Debug, Clone, Args)]
pub struct LegacyStatusArgs {
    #[arg(short = 'p', long)]
    pub project: Option<String>,
}

#[derive(Debug, Clone, Args)]
pub struct LegacyExportArgs {
    #[arg(short = 'p', long)]
    pub project: Option<String>,
    #[command(flatten)]
    pub dry_run: DryRunFlag,
}

#[derive(Debug, Clone, Args)]
pub struct LegacyImportArgs {
    #[arg(short = 'p', long)]
    pub project: Option<String>,
    #[command(flatten)]
    pub dry_run: DryRunFlag,
    #[arg(long)]
    pub allow_removals: bool,
}

// ── disable ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Args)]
pub struct AdvancedDisableArgs {
    #[command(flatten)]
    pub dry_run: DryRunFlag,
}

// ── Dispatch ───────────────────────────────────────────────────────────────

pub fn dispatch(cmd: SourceAdvancedCmd, ctx: &mut CommandCtx) -> Result<CommandOutcome> {
    match cmd.command {
        SourceAdvancedSubcommand::Enable(args) => handle_enable(args, ctx),
        SourceAdvancedSubcommand::Model(args) => handle_model(args, ctx),
        SourceAdvancedSubcommand::Sync(args) => handle_sync(args, ctx),
        SourceAdvancedSubcommand::Enrich(args) => handle_enrich(args, ctx),
        SourceAdvancedSubcommand::SkillInstall(args) => handle_skill_install(args, ctx),
        SourceAdvancedSubcommand::Status(args) => handle_status(args, ctx),
        SourceAdvancedSubcommand::Rebuild(args) => handle_rebuild(args, ctx),
        SourceAdvancedSubcommand::Clear(args) => handle_clear(args, ctx),
        SourceAdvancedSubcommand::Recover(args) => handle_recover(args, ctx),
        SourceAdvancedSubcommand::Legacy(args) => handle_legacy(args, ctx),
        SourceAdvancedSubcommand::Disable(args) => handle_disable(args, ctx),
    }
}

fn handle_enable(args: AdvancedEnableArgs, ctx: &mut CommandCtx) -> Result<CommandOutcome> {
    let repo = ctx.require_repo_path()?;
    let config = svc::source::advanced::config::load_repository_config(repo)?;

    if args.dry_run.dry_run {
        let preview = svc::source::advanced::activation::preview_activation(repo, &config)?;
        let json = serde_json::to_value(&preview).unwrap_or_default();
        return Ok(CommandOutcome::Success(
            serde_json::json!({"status": "ok", "command": "source.advanced.enable", "data": json}),
            vec![],
            None,
        ));
    }

    let result = svc::source::advanced::activation::activate(repo, &config)?;
    let json = serde_json::to_value(&result).unwrap_or_default();
    Ok(CommandOutcome::Success(
        serde_json::json!({"status": "ok", "command": "source.advanced.enable", "data": json}),
        vec![],
        None,
    ))
}

fn handle_model(cmd: ModelCmd, ctx: &mut CommandCtx) -> Result<CommandOutcome> {
    let repo = ctx.require_repo_path()?;
    match cmd {
        ModelCmd::Install(args) => {
            let result = svc::source::advanced::model_store::install_model(
                repo,
                args.model.as_deref(),
                args.model_revision.as_deref(),
                args.dry_run.dry_run,
            )?;
            let json = serde_json::to_value(&result).unwrap_or_default();
            Ok(CommandOutcome::Success(
                serde_json::json!({"status": "ok", "command": "source.advanced.model.install", "data": json}),
                vec![],
                None,
            ))
        }
        ModelCmd::Import(args) => {
            let result =
                svc::source::advanced::model_store::import_model(repo, &args.bundle_dir, args.dry_run.dry_run)?;
            let json = serde_json::to_value(&result).unwrap_or_default();
            Ok(CommandOutcome::Success(
                serde_json::json!({"status": "ok", "command": "source.advanced.model.import", "data": json}),
                vec![],
                None,
            ))
        }
        ModelCmd::Status(args) => {
            let result = svc::source::advanced::model_store::model_status(repo, args.model.as_deref())?;
            let json = serde_json::to_value(&result).unwrap_or_default();
            Ok(CommandOutcome::Success(
                serde_json::json!({"status": "ok", "command": "source.advanced.model.status", "data": json}),
                vec![],
                None,
            ))
        }
    }
}

fn handle_sync(args: AdvancedSyncArgs, ctx: &mut CommandCtx) -> Result<CommandOutcome> {
    let repo = ctx.require_repo_path()?;
    let config = svc::source::advanced::config::load_repository_config(repo)?;
    let dry_run = args.dry_run.dry_run;

    let report =
        svc::source::advanced::sync::sync_repository(repo, &config, args.project.as_deref(), dry_run, args.offline)?;

    let json = serde_json::to_value(&report).unwrap_or_default();
    let warnings: Vec<String> = if report.registrations_failed > 0 {
        vec![format!("{} of {} registration(s) failed sync", report.registrations_failed, report.registrations_total)]
    } else {
        vec![]
    };

    Ok(CommandOutcome::Success(
        serde_json::json!({"status": "ok", "command": "source.advanced.sync", "data": json}),
        warnings,
        None,
    ))
}

fn handle_enrich(cmd: EnrichCmd, ctx: &mut CommandCtx) -> Result<CommandOutcome> {
    let repo = ctx.require_repo_path()?;
    match cmd {
        EnrichCmd::List(args) => {
            let jobs = svc::source::advanced::enrichment::list_jobs(repo, args.state.as_deref(), args.limit)?;
            let json = serde_json::to_value(&jobs).unwrap_or_default();
            Ok(CommandOutcome::Success(
                serde_json::json!({"status": "ok", "command": "source.advanced.enrich.list", "data": {"jobs": json}}),
                vec![],
                None,
            ))
        }
        EnrichCmd::Show(args) => {
            let result = svc::source::advanced::enrichment::show_document(&args.document_key, args.batch, repo)?;
            let json = serde_json::to_value(&result).unwrap_or_default();
            Ok(CommandOutcome::Success(
                serde_json::json!({"status": "ok", "command": "source.advanced.enrich.show", "data": json}),
                vec![],
                None,
            ))
        }
        EnrichCmd::Apply(args) => {
            let input_data = std::fs::read_to_string(&args.input)?;
            let input: svc::source::advanced::enrichment::EnrichmentInput = serde_json::from_str(&input_data)?;
            let result = svc::source::advanced::enrichment::apply_enrichment(&input, repo, args.dry_run.dry_run)?;
            let json = serde_json::to_value(&result).unwrap_or_default();
            Ok(CommandOutcome::Success(
                serde_json::json!({"status": "ok", "command": "source.advanced.enrich.apply", "data": json}),
                vec![],
                None,
            ))
        }
    }
}

fn handle_skill_install(args: AdvancedSkillInstallArgs, ctx: &mut CommandCtx) -> Result<CommandOutcome> {
    let repo = ctx.require_repo_path()?;
    let result = svc::source::advanced::skill_install::install_skill(repo, args.force, args.dry_run.dry_run)?;
    let json = serde_json::to_value(&result).unwrap_or_default();
    Ok(CommandOutcome::Success(
        serde_json::json!({"status": "ok", "command": "source.advanced.skill_install", "data": json}),
        vec![],
        None,
    ))
}

fn handle_status(_args: AdvancedStatusArgs, ctx: &mut CommandCtx) -> Result<CommandOutcome> {
    let repo = ctx.require_repo_path()?;
    let config = svc::source::advanced::config::load_repository_config(repo)?;
    let report = svc::source::advanced::status::build_status(repo, &config)?;
    let json = serde_json::to_value(&report).unwrap_or_default();
    let warnings = report.warnings.clone();
    Ok(CommandOutcome::Success(
        serde_json::json!({"status": "ok", "command": "source.advanced.status", "data": json}),
        warnings,
        None,
    ))
}

fn handle_rebuild(args: AdvancedRebuildArgs, ctx: &mut CommandCtx) -> Result<CommandOutcome> {
    let repo = ctx.require_repo_path()?;
    let config = svc::source::advanced::config::load_repository_config(repo)?;
    let report = svc::source::advanced::sync::rebuild_repository(repo, &config, args.dry_run.dry_run, args.offline)?;
    let json = serde_json::to_value(&report).unwrap_or_default();
    let warnings: Vec<String> = if report.registrations_failed > 0 {
        vec![format!(
            "{} of {} registration(s) failed rebuild",
            report.registrations_failed, report.registrations_total
        )]
    } else {
        vec![]
    };
    Ok(CommandOutcome::Success(
        serde_json::json!({"status": "ok", "command": "source.advanced.rebuild", "data": json}),
        warnings,
        None,
    ))
}

fn handle_clear(args: AdvancedClearArgs, ctx: &mut CommandCtx) -> Result<CommandOutcome> {
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
        serde_json::json!({"status": "ok", "command": "source.advanced.clear", "data": json}),
        vec![],
        None,
    ))
}

fn handle_recover(args: AdvancedRecoverArgs, ctx: &mut CommandCtx) -> Result<CommandOutcome> {
    let repo = ctx.require_repo_path()?;
    if !args.yes.yes && !args.dry_run.dry_run {
        return Err(crate::error::MfError::usage(
            "recover requires --yes for real mutation".to_string(),
            Some("use --dry-run to preview, then --yes to execute".to_string()),
        ));
    }
    let advanced_dir = repo.join(".mind").join("source").join("advanced");
    let pointer =
        svc::source::advanced::sync::recover_from_snapshot(&advanced_dir, &args.snapshot, args.dry_run.dry_run)?;
    let json = serde_json::to_value(&pointer).unwrap_or_default();
    Ok(CommandOutcome::Success(
        serde_json::json!({"status": "ok", "command": "source.advanced.recover", "data": json}),
        vec![],
        None,
    ))
}

fn handle_legacy(cmd: LegacyCmd, ctx: &mut CommandCtx) -> Result<CommandOutcome> {
    let repo = ctx.require_repo_path()?;
    match cmd {
        LegacyCmd::Status(_args) => {
            let results = svc::source::advanced::compatibility::status_all(repo)?;
            let json = serde_json::to_value(&results).unwrap_or_default();
            Ok(CommandOutcome::Success(
                serde_json::json!({"status": "ok", "command": "source.advanced.legacy.status", "data": {"projects": json}}),
                vec![],
                None,
            ))
        }
        LegacyCmd::Export(args) => {
            let project = args.project.as_deref().unwrap_or("all");
            let result = svc::source::advanced::compatibility::export_project(repo, project, args.dry_run.dry_run)?;
            let json = serde_json::to_value(&result).unwrap_or_default();
            Ok(CommandOutcome::Success(
                serde_json::json!({"status": "ok", "command": "source.advanced.legacy.export", "data": json}),
                vec![],
                None,
            ))
        }
        LegacyCmd::Import(args) => {
            if !args.allow_removals {
                let preview = svc::source::advanced::compatibility::export_project(
                    repo,
                    args.project.as_deref().unwrap_or("all"),
                    true,
                )?;
                let would_remove = preview.drift_details.len();
                if would_remove > 0 {
                    return Err(crate::error::MfError::usage(
                        format!("import would remove {would_remove} legacy entries — use --allow-removals to confirm"),
                        Some("use --dry-run first to preview the full import plan".to_string()),
                    ));
                }
            }
            let result = svc::source::advanced::compatibility::export_project(
                repo,
                args.project.as_deref().unwrap_or("all"),
                args.dry_run.dry_run,
            )?;
            let json = serde_json::to_value(&result).unwrap_or_default();
            Ok(CommandOutcome::Success(
                serde_json::json!({"status": "ok", "command": "source.advanced.legacy.import", "data": json}),
                vec![],
                None,
            ))
        }
    }
}

fn handle_disable(args: AdvancedDisableArgs, ctx: &mut CommandCtx) -> Result<CommandOutcome> {
    let repo = ctx.require_repo_path()?;
    // Check projection parity before allowing disable
    let projections = svc::source::advanced::compatibility::status_all(repo)?;
    let drift_count =
        projections.iter().filter(|p| p.state != crate::model::source_advanced::ProjectionStatus::Current).count();
    if drift_count > 0 && !args.dry_run.dry_run {
        return Err(crate::error::MfError::disable_blocked(
            format!("{drift_count} project(s) have projection drift — run `mf source advanced legacy export` first"),
            Some("disable requires all projections to be current before switching to legacy backend".to_string()),
        ));
    }
    if !args.dry_run.dry_run {
        svc::source::advanced::activation::disable_backend(repo)?;
    }
    Ok(CommandOutcome::Success(
        serde_json::json!({
            "status": "ok",
            "command": "source.advanced.disable",
            "data": {
                "dry_run": args.dry_run.dry_run,
                "projections_current": projections.len(),
                "projections_drifted": drift_count,
                "ready": drift_count == 0,
                "disabled": !args.dry_run.dry_run && drift_count == 0
            }
        }),
        vec![],
        None,
    ))
}
