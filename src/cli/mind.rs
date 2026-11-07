use clap::{Args, Subcommand};
use serde_json::json;

use crate::cli::{CommandCtx, CommandOutcome};
use crate::error::{MfError, Result};
use crate::model::mind::{ContentLayout, MindRegistration};
use crate::output::Format;
use crate::service::mind as svc;

#[derive(Debug, Clone, Args)]
pub struct MindCmd {
    #[command(subcommand)]
    pub command: MindCommand,
}

#[derive(Debug, Clone, Subcommand)]
pub enum MindCommand {
    /// Register a mind without requiring an output
    New {
        name: String,
        #[arg(long)]
        title: Option<String>,
        #[arg(long)]
        default_output: Option<String>,
        #[arg(long, short = 'n')]
        dry_run: bool,
    },
    List,
    Show {
        identity: String,
    },
    /// Reconcile the derived output index from authored files
    Index {
        /// Compare disk, projections and explicit bindings without writing (exit 1 on differences)
        #[arg(long)]
        check: bool,
        #[arg(long, short = 'n')]
        dry_run: bool,
    },
}

#[derive(Debug, Clone, Args)]
pub struct OutputCmd {
    #[command(subcommand)]
    pub command: OutputCommand,
}

#[derive(Debug, Clone, Subcommand)]
pub enum OutputCommand {
    /// Create an editable output owned by an existing mind
    New {
        mind: String,
        name: String,
        #[arg(long)]
        title: Option<String>,
        /// Use one Markdown file instead of ordered blocks
        #[arg(long)]
        file: bool,
        #[arg(long, short = 'n')]
        dry_run: bool,
    },
    List {
        mind: Option<String>,
    },
    Show {
        identity: String,
    },
    Index {
        /// Compare disk, projections and explicit bindings without writing (exit 1 on differences)
        #[arg(long)]
        check: bool,
        #[arg(long, short = 'n')]
        dry_run: bool,
    },
}

fn outcome(data: serde_json::Value, ctx: &CommandCtx) -> Result<CommandOutcome> {
    if ctx.format() == Format::Json {
        return Ok(CommandOutcome::Success(data, vec![], None));
    }
    let text = if data.get("created_at").is_some() {
        format!(
            "{} {}: {}",
            if data["dry_run"] == true { "would create" } else { "✓ created" },
            data["kind"].as_str().unwrap_or_default(),
            data["identity"].as_str().unwrap_or_default()
        )
    } else if let Some(items) = data.get("minds").or_else(|| data.get("outputs")).and_then(|v| v.as_array()) {
        use crate::output::list::{ListCell, ListOpts, ListRow, ListView, render_text};
        let rows = items
            .iter()
            .map(|item| ListRow {
                cells: vec![
                    ListCell::Text(item["identity"].as_str().unwrap_or_default().into()),
                    ListCell::Text(item["path"].as_str().unwrap_or("-").into()),
                ],
            })
            .collect();
        render_text(
            &ListView {
                headers: &["IDENTITY", "PATH"],
                rows,
                plural_noun: if data.get("minds").is_some() { "minds" } else { "outputs" },
            },
            &ListOpts::from_flags(false, false),
        )
    } else {
        data.as_object()
            .into_iter()
            .flatten()
            .map(|(key, value)| {
                format!("{key}:  {}", value.as_str().map(String::from).unwrap_or_else(|| value.to_string()))
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    Ok(CommandOutcome::Raw(text, None))
}

pub fn dispatch(cmd: MindCmd, ctx: &mut CommandCtx) -> Result<CommandOutcome> {
    let root = ctx.require_repo_path()?;
    let selector = match &cmd.command {
        MindCommand::Show { identity } => Some(identity.as_str()),
        _ => None,
    };
    let project = svc::project_for(root, ctx.cwd(), ctx.project(), selector)?;
    let data = match cmd.command {
        MindCommand::List => json!({"minds": svc::discover(root, &project)?}),
        MindCommand::Show { identity } => serde_json::to_value(svc::resolve_mind(root, &project, &identity)?)?,
        MindCommand::Index { dry_run, check } => {
            if check {
                return check_outcome(crate::service::content_check::inspect(root, &project)?);
            }
            crate::service::content_check::reconcile(root, &project, dry_run)?
        }
        MindCommand::New { name, title, default_output, dry_run } => {
            svc::validate_name(&name)?;
            if let Some(default) = &default_output {
                svc::validate_name(default)?;
            }
            if svc::discover(root, &project)?.iter().any(|m| m.name.eq_ignore_ascii_case(&name)) {
                return Err(MfError::file_exists(project.join(&name)));
            }
            let cfg = svc::config_for(&project)?;
            let mode = cfg.content_layout.ok_or_else(|| MfError::usage("project has no content_layout; create a project with --layout category|bundle; legacy ownership requires explicit migration", None))?;
            // Validate reserved names with the same discovery rules before writing.
            svc::validate_mind_name(&cfg, &name)?;
            let identity = format!("{}::{name}", crate::service::util::rel_posix_path(root, &project)?);
            let proposed_path = (mode == ContentLayout::Bundle).then_some(name.clone());
            if let Some(path) = &proposed_path {
                svc::checked_path(&project, path)?;
            }
            if !dry_run {
                let mut raw = svc::raw_config(&project)?;
                raw["content_layout"] = serde_yaml::to_value(mode).map_err(|e| MfError::Internal(e.into()))?;
                if raw.get("minds").is_none() {
                    raw["minds"] = serde_yaml::Value::Mapping(Default::default());
                }
                raw["minds"][&name] = serde_yaml::to_value(MindRegistration { title, default_output })
                    .map_err(|e| MfError::Internal(e.into()))?;
                if let Some(path) = &proposed_path {
                    std::fs::create_dir(project.join(path))?;
                }
                svc::save_config_value(&project, &raw)?;
            }
            json!({"kind":"mind", "identity":identity, "created_at":chrono::Utc::now().to_rfc3339(), "dry_run":dry_run, "path":if dry_run {None} else {proposed_path.clone()}, "details":{"proposed_path":proposed_path}})
        }
    };
    outcome(data, ctx)
}

pub fn dispatch_output(cmd: OutputCmd, ctx: &mut CommandCtx) -> Result<CommandOutcome> {
    let root = ctx.require_repo_path()?;
    let selector = match &cmd.command {
        OutputCommand::Show { identity } => Some(identity.as_str()),
        OutputCommand::New { mind, .. } => Some(mind.as_str()),
        OutputCommand::List { mind } => mind.as_deref(),
        _ => None,
    };
    let project = svc::project_for(root, ctx.cwd(), ctx.project(), selector)?;
    let data = match cmd.command {
        OutputCommand::List { mind } => {
            let minds = if let Some(mind) = mind {
                vec![svc::resolve_mind(root, &project, &mind)?]
            } else {
                svc::discover(root, &project)?
            };
            json!({"outputs":minds.into_iter().flat_map(|m|m.outputs).collect::<Vec<_>>()})
        }
        OutputCommand::Show { identity } => serde_json::to_value(svc::resolve_output(root, &project, &identity)?)?,
        OutputCommand::Index { dry_run, check } => {
            if check {
                return check_outcome(crate::service::content_check::inspect(root, &project)?);
            }
            crate::service::content_check::reconcile(root, &project, dry_run)?
        }
        OutputCommand::New { mind, name, title, file, dry_run } => {
            svc::validate_name(&name)?;
            let mind = svc::resolve_mind(root, &project, &mind)?;
            if mind.outputs.iter().any(|o| o.name.eq_ignore_ascii_case(&name)) {
                return Err(MfError::file_exists(project.join(&name)));
            }
            let cfg = svc::config_for(&project)?;
            let base = format!("{}/{name}", svc::output_root(cfg.content_layout.unwrap_or_default(), &mind.name));
            let relative = if file { format!("{base}.md") } else { base.clone() };
            let path = svc::checked_path(&project, &relative)?;
            for candidate in [&base, &format!("{base}.md")] {
                if svc::checked_path(&project, candidate)?.exists() {
                    return Err(MfError::file_exists(project.join(candidate)));
                }
            }
            if !dry_run {
                let destination = if file { path.clone() } else { path.join("01-opening.md") };
                std::fs::create_dir_all(destination.parent().expect("output parent"))?;
                use std::io::Write;
                let mut handle = std::fs::OpenOptions::new().write(true).create_new(true).open(destination)?;
                writeln!(handle, "# {}\n", title.unwrap_or_else(|| name.clone()))?;
            }
            json!({"kind":"output", "identity":format!("{}::{name}",mind.identity), "created_at":chrono::Utc::now().to_rfc3339(), "dry_run":dry_run, "path":if dry_run {None} else {Some(relative.clone())}, "details":{"proposed_path":relative, "shape":if file {"file"} else {"directory"}, "mind_identity":mind.identity}})
        }
    };
    outcome(data, ctx)
}

fn check_outcome(data: serde_json::Value) -> Result<CommandOutcome> {
    let exit = (data["consistent"] != true).then_some(1);
    Ok(CommandOutcome::Success(data, vec![], exit))
}
