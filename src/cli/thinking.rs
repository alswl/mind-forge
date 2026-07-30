use crate::cli::{CommandCtx, CommandOutcome};
use crate::error::{MfError, Result};
use crate::output::Format;
use crate::output::list::{ListCell, ListOpts, ListRow, ListView, json_collection, render_text};
use crate::service::{index, util};
use clap::{Args, Subcommand};

#[derive(Debug, Clone, Args)]
pub struct ThinkingCmd {
    #[command(subcommand)]
    pub command: Option<ThinkingSubcommand>,
}
#[derive(Debug, Clone, Subcommand)]
pub enum ThinkingSubcommand {
    List,
    Show { path: String },
}

pub fn dispatch(cmd: ThinkingCmd, ctx: &mut CommandCtx) -> Result<CommandOutcome> {
    let root = ctx.require_repo_path()?;
    let project = util::resolve_project(root, ctx.project(), ctx.cwd())?;
    let idx = index::load(&project)?;
    let thinking = idx.thinking.unwrap_or_default();
    match cmd.command.unwrap_or(ThinkingSubcommand::List) {
        ThinkingSubcommand::List => {
            let mut thinking = thinking;
            thinking.sort_by(|a, b| a.path.cmp(&b.path));
            match ctx.format() {
                Format::Json => Ok(CommandOutcome::Success(
                    json_collection(
                        "thinking",
                        thinking.iter().map(|t| serde_json::to_value(t).unwrap_or_default()).collect(),
                    ),
                    Vec::new(),
                    None,
                )),
                Format::Text => {
                    let view = ListView {
                        headers: &["PATH", "ARTICLE", "UPDATED"],
                        rows: thinking
                            .iter()
                            .map(|t| ListRow {
                                cells: vec![
                                    ListCell::Path(t.path.clone()),
                                    ListCell::Path(t.article.clone()),
                                    ListCell::Text(t.updated_at.clone()),
                                ],
                            })
                            .collect(),
                        plural_noun: "thinking entries",
                    };
                    Ok(CommandOutcome::Raw(render_text(&view, &ListOpts::from_flags(false, false)), None))
                }
            }
        }
        ThinkingSubcommand::Show { path } => {
            let entry = thinking.iter().find(|t| t.path == path).ok_or_else(|| {
                MfError::usage(
                    format!("thinking '{path}' not found"),
                    Some("use `mf thinking list` to see available entries".to_string()),
                )
            })?;
            let data = serde_json::to_value(entry).unwrap_or_default();
            match ctx.format() {
                Format::Json => Ok(CommandOutcome::Success(data, Vec::new(), None)),
                Format::Text => Ok(CommandOutcome::Success(
                    serde_json::Value::String(format!(
                        "path  {}\narticle  {}\nupdated  {}",
                        entry.path, entry.article, entry.updated_at
                    )),
                    Vec::new(),
                    None,
                )),
            }
        }
    }
}
