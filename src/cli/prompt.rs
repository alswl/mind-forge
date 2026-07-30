use crate::cli::{CommandCtx, CommandOutcome};
use crate::error::{MfError, Result};
use crate::output::Format;
use crate::output::list::{ListCell, ListOpts, ListRow, ListView, json_collection, render_text};
use crate::service::{index, util};
use clap::{Args, Subcommand};

#[derive(Debug, Clone, Args)]
pub struct PromptCmd {
    #[command(subcommand)]
    pub command: Option<PromptSubcommand>,
}
#[derive(Debug, Clone, Subcommand)]
pub enum PromptSubcommand {
    List,
    Show { path: String },
}

pub fn dispatch(cmd: PromptCmd, ctx: &mut CommandCtx) -> Result<CommandOutcome> {
    let root = ctx.require_repo_path()?;
    let project = util::resolve_project(root, ctx.project(), ctx.cwd())?;
    let idx = index::load(&project)?;
    let prompts = idx.prompts.unwrap_or_default();
    match cmd.command.unwrap_or(PromptSubcommand::List) {
        PromptSubcommand::List => {
            let mut prompts = prompts;
            prompts.sort_by(|a, b| a.path.cmp(&b.path));
            let items: Vec<_> = prompts.iter().map(|p| serde_json::to_value(p).unwrap_or_default()).collect();
            match ctx.format() {
                Format::Json => Ok(CommandOutcome::Success(json_collection("prompts", items), Vec::new(), None)),
                Format::Text => {
                    let view = ListView {
                        headers: &["PATH", "ARTICLE", "MODE", "UPDATED"],
                        rows: prompts
                            .iter()
                            .map(|p| ListRow {
                                cells: vec![
                                    ListCell::Path(p.path.clone()),
                                    ListCell::Path(p.article.clone()),
                                    ListCell::Text(p.mode.as_ref().map(ToString::to_string).unwrap_or_default()),
                                    ListCell::Text(p.updated_at.clone()),
                                ],
                            })
                            .collect(),
                        plural_noun: "prompts",
                    };
                    Ok(CommandOutcome::Raw(render_text(&view, &ListOpts::from_flags(false, false)), None))
                }
            }
        }
        PromptSubcommand::Show { path } => {
            let prompt = prompts.iter().find(|p| p.path == path).ok_or_else(|| {
                MfError::usage(
                    format!("prompt '{path}' not found"),
                    Some("use `mf prompt list` to see available prompts".to_string()),
                )
            })?;
            let data = serde_json::to_value(prompt).unwrap_or_default();
            match ctx.format() {
                Format::Json => Ok(CommandOutcome::Success(data, Vec::new(), None)),
                Format::Text => Ok(CommandOutcome::Success(
                    serde_json::Value::String(format!(
                        "path  {}\narticle  {}\nmode  {}\nupdated  {}",
                        prompt.path,
                        prompt.article,
                        prompt.mode.as_ref().map(ToString::to_string).unwrap_or_default(),
                        prompt.updated_at
                    )),
                    Vec::new(),
                    None,
                )),
            }
        }
    }
}
