use clap::{Args, Subcommand};

use crate::cli::shared_flags::{DryRunFlag, NoHeadersFlag, NoTruncFlag};
use crate::cli::CommandCtx;
use crate::cli::CommandOutcome;
use crate::error::{MfError, Result};
use crate::output::list::{json_collection, render_text, ListCell, ListOpts, ListRow, ListView};
use crate::output::show::{json_envelope, render_text as render_show_text, ShowBlock, ShowField, ShowOpts, ShowValue};
use crate::output::verb::{Verb, VerbOpts, VerbResult};
use crate::output::Format;
use crate::service::thinking::ThinkingRecord;
use crate::service::{thinking as svc_thinking, util as svc_util};

#[derive(Debug, Clone, Args)]
pub struct ThinkingCmd {
    #[command(subcommand)]
    pub command: Option<ThinkingSubcommand>,
}

#[derive(Debug, Clone, Subcommand)]
pub enum ThinkingSubcommand {
    #[command(about = "List thinking ledger entries", visible_alias = "ls")]
    List(ThinkingListArgs),
    #[command(about = "Show thinking entry details")]
    Show(ThinkingShowArgs),
    #[command(about = "Reconcile the thinking projection with thinking/ on disk")]
    Index(ThinkingIndexArgs),
}

#[derive(Debug, Clone, Args)]
pub struct ThinkingListArgs {
    #[command(flatten)]
    pub no_headers: NoHeadersFlag,
    #[command(flatten)]
    pub no_trunc: NoTruncFlag,
}

#[derive(Debug, Clone, Args)]
pub struct ThinkingShowArgs {
    /// Thinking entry identity (path), e.g. thinking/my-post.md — as emitted by `mf thinking list`
    pub selector: String,
}

#[derive(Debug, Clone, Args)]
pub struct ThinkingIndexArgs {
    #[command(flatten)]
    pub dry_run: DryRunFlag,
}

pub fn dispatch(command: ThinkingCmd, ctx: &mut CommandCtx) -> Result<CommandOutcome> {
    match command.command {
        None => Ok(CommandOutcome::GroupHelp("thinking")),
        Some(ThinkingSubcommand::List(args)) => handle_list(args, ctx),
        Some(ThinkingSubcommand::Show(args)) => handle_show(args, ctx),
        Some(ThinkingSubcommand::Index(args)) => handle_index(args, ctx),
    }
}

fn handle_list(args: ThinkingListArgs, ctx: &CommandCtx) -> Result<CommandOutcome> {
    let project_path = svc_util::resolve_project(ctx.require_repo_path()?, ctx.project(), ctx.cwd())?;
    let records = svc_thinking::list(&project_path)?;

    let opts = ListOpts::from_flags(args.no_headers.no_headers, args.no_trunc.no_trunc)
        .with_repo_root(Some(project_path.to_path_buf()));

    match ctx.format() {
        Format::Json => {
            let items: Vec<serde_json::Value> = records
                .iter()
                .map(|r| {
                    let mut v = serde_json::to_value(r).map_err(MfError::Json)?;
                    if let Some(obj) = v.as_object_mut() {
                        obj.insert("identity".to_string(), serde_json::Value::String(r.identity()));
                    }
                    Ok(v)
                })
                .collect::<Result<Vec<_>>>()?;
            Ok(CommandOutcome::Success(json_collection("thinking", items), Vec::new(), None))
        }
        Format::Text => {
            let mut rows = Vec::with_capacity(records.len());
            for r in &records {
                rows.push(ListRow {
                    cells: vec![
                        ListCell::Path(r.path.clone()),
                        ListCell::Text(if r.article.is_empty() { "-".to_string() } else { r.article.clone() }),
                        ListCell::Text(r.binding_status.as_str().to_string()),
                        ListCell::Text(r.updated_at.clone()),
                    ],
                });
            }
            let view =
                ListView { headers: &["IDENTITY", "ARTICLE", "STATUS", "UPDATED"], rows, plural_noun: "thinking" };
            Ok(CommandOutcome::Raw(render_text(&view, &opts), None))
        }
    }
}

fn handle_show(args: ThinkingShowArgs, ctx: &CommandCtx) -> Result<CommandOutcome> {
    let project_path = svc_util::resolve_project(ctx.require_repo_path()?, ctx.project(), ctx.cwd())?;
    let record: ThinkingRecord = svc_thinking::show(&project_path, &args.selector)?;

    let block = ShowBlock {
        kind: "thinking",
        identity: record.identity(),
        fields: vec![
            ShowField { label: "Path", value: ShowValue::Path(record.path.clone()) },
            ShowField {
                label: "Article",
                value: ShowValue::Optional(if record.article.is_empty() { None } else { Some(record.article.clone()) }),
            },
            ShowField { label: "Binding status", value: ShowValue::Text(record.binding_status.as_str().to_string()) },
            ShowField { label: "Updated", value: ShowValue::Text(record.updated_at.clone()) },
        ],
        sections: vec![],
    };

    match ctx.format() {
        Format::Json => {
            let record_json = serde_json::to_value(&record).map_err(MfError::Json)?;
            let mut extra = record_json.as_object().cloned().unwrap_or_default();
            extra.insert("identity".to_string(), serde_json::Value::String(record.identity()));
            Ok(CommandOutcome::Success(json_envelope(&block, extra), Vec::new(), None))
        }
        Format::Text => Ok(CommandOutcome::Raw(
            render_show_text(&block, &ShowOpts::from_repo_root(Some(project_path.as_path()))),
            None,
        )),
    }
}

fn handle_index(args: ThinkingIndexArgs, ctx: &CommandCtx) -> Result<CommandOutcome> {
    let project_path = svc_util::resolve_project(ctx.require_repo_path()?, ctx.project(), ctx.cwd())?;
    let report = svc_thinking::reconcile(&project_path, args.dry_run.dry_run)?;
    let scanned_count = report.added.len() + report.removed.len() + report.kept_count as usize;

    match ctx.format() {
        Format::Json => {
            let data = serde_json::json!({
                "kind": "thinking",
                "added": report.added,
                "removed": report.removed,
                "kept_count": report.kept_count,
                "scanned_count": scanned_count,
                "dry_run": args.dry_run.dry_run,
            });
            Ok(CommandOutcome::Success(data, Vec::new(), None))
        }
        Format::Text => {
            let details = serde_json::json!({
                "added": report.added,
                "removed": report.removed,
                "kept_count": report.kept_count,
                "scanned_count": scanned_count,
            });
            let result = VerbResult {
                verb: Verb::Index,
                kind: "thinking",
                identity: String::new(),
                old_identity: None,
                path: None,
                dry_run: args.dry_run.dry_run,
                details,
            };
            Ok(CommandOutcome::Success(
                serde_json::Value::String(crate::output::verb::render_text(
                    &result,
                    &VerbOpts::from_repo_root(Some(project_path.as_path())),
                )),
                Vec::new(),
                None,
            ))
        }
    }
}
