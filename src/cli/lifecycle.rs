//! Durable plan execution and recovery. Plan construction is kept in services.
use crate::cli::{CommandCtx, CommandOutcome};
use crate::error::Result;
use crate::model::mind::ContentLayout;
use crate::service::mind_lifecycle::planner::{self, Operation};
use crate::service::mind_lifecycle::transaction::{self, Plan};
use clap::{Args, Subcommand};
use serde_json::json;
use std::path::PathBuf;

#[derive(Debug, Args)]
pub struct LifecycleCmd {
    #[command(subcommand)]
    pub command: LifecycleCommand,
}

#[derive(Debug, Args)]
pub struct PlanFlags {
    /// Preview without creating directories, locks, journals, or caches
    #[arg(long, short = 'n')]
    pub dry_run: bool,
    /// Include the full saved plan (contains backed-up file contents) in JSON
    #[arg(long, requires = "dry_run")]
    pub emit_plan: bool,
    #[arg(long, short = 'y')]
    pub yes: bool,
}

#[derive(Debug, Subcommand)]
pub enum LifecycleCommand {
    /// Convert every output, core role, and owned additional resource
    Convert {
        #[arg(long, value_enum)]
        to: ContentLayout,
        #[command(flatten)]
        flags: PlanFlags,
    },
    MindRename {
        identity: String,
        name: String,
        #[command(flatten)]
        flags: PlanFlags,
    },
    MindMove {
        identity: String,
        #[arg(long)]
        to_project: String,
        #[command(flatten)]
        flags: PlanFlags,
    },
    MindRemove {
        identity: String,
        #[command(flatten)]
        flags: PlanFlags,
    },
    OutputRename {
        identity: String,
        name: String,
        #[command(flatten)]
        flags: PlanFlags,
    },
    OutputMove {
        identity: String,
        #[arg(long)]
        to_mind: String,
        #[arg(long)]
        to_project: String,
        #[command(flatten)]
        flags: PlanFlags,
    },
    OutputRemove {
        identity: String,
        #[command(flatten)]
        flags: PlanFlags,
    },
    /// Apply a saved hash-bound plan after revalidating its complete inventory
    Apply {
        plan: PathBuf,
        #[arg(long, short = 'n')]
        dry_run: bool,
        #[arg(long, short = 'y')]
        yes: bool,
    },
    /// Resume an interrupted transaction or safely abort unpublished partial preparation
    Resume {
        #[arg(long)]
        plan: Option<PathBuf>,
    },
    /// Restore backed-up content, refusing to overwrite subsequent edits
    Rollback {
        #[arg(long)]
        plan: Option<PathBuf>,
    },
    /// Inspect the current transaction without writing
    Status,
}

fn load(path: &PathBuf) -> Result<Plan> {
    Ok(serde_json::from_slice(&std::fs::read(path)?)?)
}

pub fn dispatch(cmd: LifecycleCmd, ctx: &mut CommandCtx) -> Result<CommandOutcome> {
    let root = ctx.require_repo_path()?;
    let data = match cmd.command {
        LifecycleCommand::Convert { to, flags } => execute(ctx, Operation::Convert(to), None, flags)?,
        LifecycleCommand::MindRename { identity, name, flags } => {
            execute(ctx, Operation::MindRename { identity: identity.clone(), name }, Some(&identity), flags)?
        }
        LifecycleCommand::MindMove { identity, to_project, flags } => execute(
            ctx,
            Operation::MindMove { identity: identity.clone(), project: to_project },
            Some(&identity),
            flags,
        )?,
        LifecycleCommand::MindRemove { identity, flags } => {
            execute(ctx, Operation::MindRemove { identity: identity.clone() }, Some(&identity), flags)?
        }
        LifecycleCommand::OutputRename { identity, name, flags } => {
            execute(ctx, Operation::OutputRename { identity: identity.clone(), name }, Some(&identity), flags)?
        }
        LifecycleCommand::OutputMove { identity, to_mind, to_project, flags } => execute(
            ctx,
            Operation::OutputMove { identity: identity.clone(), mind: to_mind, project: to_project },
            Some(&identity),
            flags,
        )?,
        LifecycleCommand::OutputRemove { identity, flags } => {
            execute(ctx, Operation::OutputRemove { identity: identity.clone() }, Some(&identity), flags)?
        }
        LifecycleCommand::Apply { plan, dry_run, yes } => {
            let plan = load(&plan)?;
            if dry_run {
                transaction::validate(root, &plan)?;
                json!({"kind":"lifecycle", "dry_run":true, "digest":plan.digest, "writes":plan.preview()})
            } else {
                confirm(&plan, yes)?;
                let journal = transaction::apply(root, &plan, None)?;
                json!({"kind":"lifecycle", "digest":journal.plan.digest, "phase":journal.phase})
            }
        }
        LifecycleCommand::Resume { plan } => recover(root, plan, false)?,
        LifecycleCommand::Rollback { plan } => recover(root, plan, true)?,
        LifecycleCommand::Status => match transaction::read_journal(root)? {
            Some(journal) => {
                json!({"kind":"lifecycle", "digest":journal.plan.digest, "phase":journal.phase, "writes":journal.plan.preview()})
            }
            None => json!({"kind":"lifecycle", "phase":"none"}),
        },
    };
    Ok(CommandOutcome::Success(data, vec![], None))
}

fn recover(root: &std::path::Path, plan: Option<PathBuf>, rollback: bool) -> Result<serde_json::Value> {
    if plan.is_none() && transaction::recover_unpublished(root, rollback)? {
        return Ok(
            json!({"kind":"lifecycle", "phase":"preparation_aborted", "authored_changes":false, "next_action":"rerun original lifecycle command to retry; partial evidence retained"}),
        );
    }
    let journal = if let Some(path) = plan {
        transaction::recover_preparation(root, &load(&path)?, rollback)?
    } else {
        transaction::recover(root, rollback, None)?
    };
    Ok(json!({"kind":"lifecycle", "digest":journal.plan.digest, "phase":journal.phase}))
}

fn confirm(plan: &Plan, yes: bool) -> Result<()> {
    // Saved plans can be inspected or edited outside mf. Require confirmation
    // for every applied plan with removals, independent of its operation label.
    if plan.preview().iter().any(|w| w.action == "remove") {
        crate::output::confirm::require_confirmation(&crate::output::confirm::ConfirmArgs {
            verb_label: "applying lifecycle removals",
            kind: "plan",
            identity: &plan.digest,
            yes,
            force: false,
        })?;
    }
    Ok(())
}

pub(crate) fn execute(
    ctx: &CommandCtx,
    operation: Operation,
    selector: Option<&str>,
    flags: PlanFlags,
) -> Result<serde_json::Value> {
    let root = ctx.require_repo_path()?;
    let project = crate::service::mind::project_for(root, ctx.cwd(), ctx.project(), selector)?;
    let plan = planner::plan(root, &project, operation)?;
    let mut data = json!({"kind":"lifecycle", "dry_run":flags.dry_run, "digest":plan.digest, "writes":plan.preview(), "rag":"stale after apply", "unverified":"references outside this repository"});
    if flags.dry_run {
        if flags.emit_plan {
            data["plan"] = serde_json::to_value(&plan)?;
        }
    } else {
        confirm(&plan, flags.yes)?;
        data["phase"] = serde_json::to_value(transaction::apply(root, &plan, None)?.phase)?;
    }
    Ok(data)
}
