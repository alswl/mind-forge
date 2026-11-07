use super::lifecycle::{PlanFlags, execute};
use crate::cli::{CommandCtx, CommandOutcome};
use crate::error::Result;
use crate::service::mind_lifecycle::planner::Operation;
use crate::service::{additional as svc, mind};
use clap::{Args, Subcommand};
use serde_json::json;

#[derive(Debug, Args)]
pub struct AdditionalCmd {
    #[command(subcommand)]
    pub command: AdditionalCommand,
}

#[derive(Debug, Subcommand)]
pub enum AdditionalCommand {
    /// List structural extensions, optionally for one mind
    List { mind: Option<String> },
    /// Inspect one exact identity or project-relative resource path
    Show { identity: String },
    /// Rename one file/directory to a concrete project-relative path (same mind and shape)
    Rename {
        identity: String,
        destination: String,
        #[command(flatten)]
        flags: PlanFlags,
    },
    /// Delete one resource, refusing inbound references
    Remove {
        identity: String,
        #[command(flatten)]
        flags: PlanFlags,
    },
}

pub fn dispatch(cmd: AdditionalCmd, ctx: &mut CommandCtx) -> Result<CommandOutcome> {
    let root = ctx.require_repo_path()?;
    let selector = match &cmd.command {
        AdditionalCommand::List { mind } => mind.as_deref(),
        AdditionalCommand::Show { identity }
        | AdditionalCommand::Rename { identity, .. }
        | AdditionalCommand::Remove { identity, .. } => Some(identity.as_str()),
    };
    let project = mind::project_for(root, ctx.cwd(), ctx.project(), selector)?;
    let data = match cmd.command {
        AdditionalCommand::List { mind } => {
            let owner = mind.map(|m| mind::resolve_mind(root, &project, &m)).transpose()?;
            let items: Vec<_> = svc::discover(root, &project)?
                .into_iter()
                .filter(|i| owner.as_ref().is_none_or(|m| m.name == i.mind))
                .collect();
            json!({"kind":"additional", "additional":items})
        }
        AdditionalCommand::Show { identity } => {
            serde_json::to_value(svc::select(&svc::discover(root, &project)?, &identity)?)?
        }
        AdditionalCommand::Rename { identity, destination, flags } => execute(
            ctx,
            Operation::AdditionalRename { identity: identity.clone(), destination },
            Some(&identity),
            flags,
        )?,
        AdditionalCommand::Remove { identity, flags } => {
            execute(ctx, Operation::AdditionalRemove { identity: identity.clone() }, Some(&identity), flags)?
        }
    };
    Ok(CommandOutcome::Success(data, vec![], None))
}
