use std::path::{Path, PathBuf};

use clap::{Args, Subcommand};
use serde::Serialize;

use crate::cli::CommandCtx;
use crate::cli::CommandOutcome;
use crate::cli::shared_flags::DryRunFlag;
use crate::cli::shared_flags::ForceFlag;
use crate::cli::shared_flags::NoHeadersFlag;
use crate::cli::shared_flags::NoTruncFlag;
use crate::cli::shared_flags::YesFlag;
use crate::error::{MfError, Result};
use crate::model::Resource;
use crate::model::asset::AssetKind;
use crate::output::Format;
use crate::output::confirm::{ConfirmArgs, require_confirmation};
use crate::output::list::{ListCell, ListOpts, ListRow, ListView, json_collection, render_text};
use crate::output::show::{
    ShowBlock, ShowField, ShowOpts, ShowValue, json_envelope as show_json, render_text as render_show_text,
};
use crate::output::verb::{Verb, VerbOpts, VerbResult, json_envelope as verb_json, render_text as verb_text};
use crate::service::{asset as asset_svc, identity, util as svc_util};

#[derive(Debug, Clone, Args)]
pub struct AssetCmd {
    #[command(subcommand)]
    pub command: Option<AssetSubcommand>,
}

#[derive(Debug, Clone, Subcommand)]
pub enum AssetSubcommand {
    #[command(about = "List assets", visible_alias = "ls")]
    List(AssetListArgs),
    #[command(about = "Create an asset")]
    New(AssetAddArgs),
    #[command(about = "Update assets")]
    Update(AssetUpdateArgs),
    #[command(about = "Index assets")]
    Index(AssetIndexArgs),
    #[command(about = "Clean stale asset index entries")]
    Clean(AssetCleanArgs),
    #[command(about = "Remove an asset", visible_alias = "rm")]
    Remove(AssetRemoveArgs),
    #[command(about = "Rename an asset")]
    Rename(AssetRenameArgs),
    #[command(about = "Show asset details")]
    Show(AssetShowArgs),
}

// ---------------------------------------------------------------------------
// Asset list args
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Args, Serialize)]
pub struct AssetListArgs {
    #[arg(long)]
    pub filter: Option<String>,
    #[arg(long = "type", value_enum)]
    pub asset_type: Option<AssetKind>,
    #[command(flatten)]
    pub no_headers: NoHeadersFlag,
    #[command(flatten)]
    pub no_trunc: NoTruncFlag,
}

// ---------------------------------------------------------------------------
// Asset add args
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Args)]
pub struct AssetAddArgs {
    pub path: PathBuf,
    #[arg(long = "name")]
    pub name: Option<String>,
    #[arg(long = "tag")]
    pub tag: Vec<String>,
    #[arg(long, conflicts_with = "link")]
    pub copy: bool,
    #[arg(long, conflicts_with = "copy")]
    pub link: bool,
    #[command(flatten)]
    pub force: ForceFlag,
    #[command(flatten)]
    pub dry_run: DryRunFlag,
}

// ---------------------------------------------------------------------------
// Asset update args
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Args, Serialize)]
pub struct AssetUpdateArgs {
    pub path: Option<PathBuf>,
    #[arg(long = "set-url")]
    pub set_url: Option<String>,
    #[arg(long)]
    pub channel: Option<String>,
    #[arg(long)]
    pub all: bool,
    #[command(flatten)]
    pub dry_run: DryRunFlag,
}

// ---------------------------------------------------------------------------
// Asset index args
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Args, Serialize)]
pub struct AssetIndexArgs {
    #[command(flatten)]
    pub dry_run: DryRunFlag,
    #[arg(long = "refresh-metadata")]
    pub refresh_metadata: bool,
}

// ---------------------------------------------------------------------------
// Asset clean args
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Args, Serialize)]
pub struct AssetCleanArgs {
    #[command(flatten)]
    pub dry_run: DryRunFlag,
}

// ---------------------------------------------------------------------------
// Asset remove args
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Args, Serialize)]
pub struct AssetShowArgs {
    /// Asset path (e.g. assets/chart.png) or name
    pub path: String,
}

#[derive(Debug, Clone, Args, Serialize)]
pub struct AssetRemoveArgs {
    pub path: String,
    #[command(flatten)]
    pub force: ForceFlag,
    #[command(flatten)]
    pub yes: YesFlag,
    #[command(flatten)]
    pub dry_run: DryRunFlag,
}

#[derive(Debug, Clone, Args, Serialize)]
pub struct AssetRenameArgs {
    pub old_path: String,
    pub new_path: String,
    #[command(flatten)]
    pub force: ForceFlag,
    #[command(flatten)]
    pub dry_run: DryRunFlag,
}

// ---------------------------------------------------------------------------
// Dispatch
// ---------------------------------------------------------------------------

pub fn dispatch(command: AssetCmd, ctx: &mut CommandCtx) -> Result<CommandOutcome> {
    let root = ctx.require_repo_path()?;
    let cwd = ctx.cwd();
    let format = ctx.format();
    let project = ctx.project();

    match command.command {
        None => Ok(CommandOutcome::GroupHelp("asset")),
        Some(AssetSubcommand::New(args)) => handle_add(args, root, cwd, format, project),
        Some(AssetSubcommand::List(args)) => handle_list(args, root, cwd, format, project),
        Some(AssetSubcommand::Update(args)) => handle_update(args, root, cwd, format, project),
        Some(AssetSubcommand::Index(args)) => handle_index(args, root, cwd, format, project),
        Some(AssetSubcommand::Clean(args)) => handle_clean(args, root, cwd, format, project),
        Some(AssetSubcommand::Show(args)) => handle_asset_show(args, root, cwd, format, project),
        Some(AssetSubcommand::Remove(args)) => handle_remove(args, root, cwd, format, project),
        Some(AssetSubcommand::Rename(args)) => handle_rename(args, root, cwd, format, project),
    }
}

// ---------------------------------------------------------------------------
// Handle: mf asset add
// ---------------------------------------------------------------------------

fn handle_add(
    args: AssetAddArgs,
    root: &Path,
    cwd: &Path,
    format: Format,
    project: Option<&str>,
) -> Result<CommandOutcome> {
    let project_path = svc_util::resolve_project(root, project, cwd)?;

    if args.dry_run.dry_run {
        let name =
            args.name.as_deref().unwrap_or_else(|| args.path.file_name().and_then(|s| s.to_str()).unwrap_or("unknown"));
        let result = VerbResult {
            verb: Verb::Add,
            kind: "asset",
            identity: name.to_string(),
            old_identity: None,
            path: Some(name.to_string()),
            dry_run: true,
            details: serde_json::json!({"source": args.path.to_string_lossy()}),
        };
        return match format {
            Format::Json => Ok(CommandOutcome::Success(verb_json(&result), Vec::new(), None)),
            Format::Text => Ok(CommandOutcome::Success(
                serde_json::Value::String(verb_text(&result, &VerbOpts::from_repo_root(Some(project_path.as_path())))),
                Vec::new(),
                None,
            )),
        };
    }

    let add_args = asset_svc::AddArgs {
        source: args.path,
        name: args.name,
        tags: args.tag,
        link_mode: args.link,
        force: args.force.force,
    };
    let asset = asset_svc::add(&project_path, cwd, &add_args)?;
    let mode = if add_args.link_mode { "link" } else { "copy" };

    let result = VerbResult {
        verb: Verb::Add,
        kind: "asset",
        identity: asset.path.clone(),
        old_identity: None,
        path: Some(asset.path.clone()),
        dry_run: false,
        details: serde_json::json!({
            "name": asset.name,
            "type": asset.kind,
            "path": asset.path,
            "size": asset.size,
            "hash": asset.hash,
            "tags": asset.tags,
            "added_at": asset.added_at,
            "mode": mode,
        }),
    };
    match format {
        Format::Json => Ok(CommandOutcome::Success(verb_json(&result), Vec::new(), None)),
        Format::Text => Ok(CommandOutcome::Success(
            serde_json::Value::String(verb_text(&result, &VerbOpts::from_repo_root(Some(project_path.as_path())))),
            Vec::new(),
            None,
        )),
    }
}

// ---------------------------------------------------------------------------
// Handle: mf asset list
// ---------------------------------------------------------------------------

fn handle_list(
    args: AssetListArgs,
    root: &Path,
    cwd: &Path,
    format: Format,
    project: Option<&str>,
) -> Result<CommandOutcome> {
    let project_path = svc_util::resolve_project(root, project, cwd)?;
    let assets = asset_svc::list(&project_path, args.filter.as_deref(), args.asset_type)?;

    let opts = ListOpts::from_flags(args.no_headers.no_headers, args.no_trunc.no_trunc)
        .with_repo_root(Some(project_path.to_path_buf()));

    match format {
        Format::Json => {
            let items: Vec<serde_json::Value> = assets
                .iter()
                .map(|a| {
                    let mut v = serde_json::to_value(a).map_err(MfError::Json)?;
                    if let Some(obj) = v.as_object_mut() {
                        obj.insert("identity".to_string(), serde_json::Value::String(a.identity()));
                    }
                    Ok(v)
                })
                .collect::<Result<Vec<_>>>()?;
            Ok(CommandOutcome::Success(json_collection("assets", items), Vec::new(), None))
        }
        Format::Text => {
            let mut rows = Vec::with_capacity(assets.len());
            for a in &assets {
                let kind_str =
                    serde_json::to_value(a.kind).ok().and_then(|v| v.as_str().map(String::from)).unwrap_or_default();
                rows.push(ListRow {
                    cells: vec![
                        ListCell::Text(a.name.clone()),
                        ListCell::Text(kind_str),
                        ListCell::Path(a.path.clone()),
                        ListCell::Number(a.size.to_string()),
                    ],
                });
            }
            let view = ListView { headers: &["NAME", "TYPE", "PATH", "SIZE"], rows, plural_noun: "assets" };
            Ok(CommandOutcome::Raw(render_text(&view, &opts), None))
        }
    }
}

// ---------------------------------------------------------------------------
// Handle: mf asset update
// ---------------------------------------------------------------------------

fn handle_update(
    args: AssetUpdateArgs,
    root: &Path,
    cwd: &Path,
    format: Format,
    project: Option<&str>,
) -> Result<CommandOutcome> {
    let project_path = svc_util::resolve_project(root, project, cwd)?;

    // Mind form: --set-url + --channel (set publish URL)
    if args.set_url.is_some() || args.channel.is_some() {
        let url = args.set_url.as_deref().unwrap_or_default();
        let channel = args.channel.as_deref().unwrap_or_default();
        let result = asset_svc::set_publish_url(&project_path, url, channel)?;
        return match format {
            Format::Json => Ok(CommandOutcome::Success(
                serde_json::json!({
                    "url": result.url,
                    "channel": result.channel,
                }),
                Vec::new(),
                None,
            )),
            Format::Text => {
                let msg = format!("Set publish URL: {} (channel: {})", result.url, result.channel);
                Ok(CommandOutcome::Success(serde_json::Value::String(msg), Vec::new(), None))
            }
        };
    }

    // Validate mutual exclusivity
    let has_path = args.path.is_some();
    let has_all = args.all;
    match (has_path, has_all) {
        (true, true) => {
            return Err(MfError::usage(
                "<PATH> and --all are mutually exclusive",
                Some("provide either <PATH> or --all, not both".to_string()),
            ));
        }
        (false, false) => {
            return Err(MfError::usage(
                "provide <PATH> or --all",
                Some("provide a path to update a single asset, or --all to update all".to_string()),
            ));
        }
        _ => {}
    }

    if let Some(path) = args.path {
        if args.dry_run.dry_run {
            let result = VerbResult {
                verb: Verb::Update,
                kind: "asset",
                identity: path.to_string_lossy().to_string(),
                old_identity: None,
                path: None,
                dry_run: true,
                details: serde_json::json!({"changes": {}}),
            };
            return match format {
                Format::Json => Ok(CommandOutcome::Success(verb_json(&result), Vec::new(), None)),
                Format::Text => Ok(CommandOutcome::Success(
                    serde_json::Value::String(verb_text(
                        &result,
                        &VerbOpts::from_repo_root(Some(project_path.as_path())),
                    )),
                    Vec::new(),
                    None,
                )),
            };
        }

        let update_result = asset_svc::update_one(&project_path, cwd, &path)?;
        let changes = if update_result.changed {
            serde_json::json!({
                "size": {"from": update_result.old_size, "to": update_result.new_size},
                "hash": {"from": update_result.old_hash, "to": update_result.new_hash},
            })
        } else {
            serde_json::json!({})
        };
        let path_str = update_result.path.clone();
        let result = VerbResult {
            verb: Verb::Update,
            kind: "asset",
            identity: path_str.clone(),
            old_identity: None,
            path: Some(path_str),
            dry_run: false,
            details: serde_json::json!({"changes": changes, "changed": update_result.changed}),
        };
        match format {
            Format::Json => Ok(CommandOutcome::Success(verb_json(&result), Vec::new(), None)),
            Format::Text => Ok(CommandOutcome::Success(
                serde_json::Value::String(verb_text(&result, &VerbOpts::from_repo_root(Some(project_path.as_path())))),
                Vec::new(),
                None,
            )),
        }
    } else {
        // --all mode
        let results = asset_svc::update_all(&project_path)?;
        match format {
            Format::Json => {
                let items = serde_json::to_value(&results).map_err(MfError::Json)?;
                let total = results.len();
                let changed = results.iter().filter(|r| r.changed).count();
                let missing = results.iter().filter(|r| r.error.is_some()).count();
                let data = serde_json::json!({
                    "items": items,
                    "summary": { "total": total, "changed": changed, "missing": missing }
                });
                Ok(CommandOutcome::Success(data, Vec::new(), None))
            }
            Format::Text => {
                let mut lines = Vec::new();
                for r in &results {
                    if r.error.is_some() {
                        lines.push(format!("✗ missing  {}", r.path));
                    } else if r.changed {
                        lines.push(format!("✓ updated {}", r.path));
                    } else {
                        lines.push(format!("= unchanged {}", r.path));
                    }
                }
                let total = results.len();
                let changed = results.iter().filter(|r| r.changed).count();
                let missing = results.iter().filter(|r| r.error.is_some()).count();
                lines.push(format!("{total} assets, {changed} changed, {missing} missing"));
                Ok(CommandOutcome::Raw(lines.join("\n"), None))
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Handle: mf asset index
// ---------------------------------------------------------------------------

fn handle_index(
    args: AssetIndexArgs,
    root: &Path,
    cwd: &Path,
    format: Format,
    project: Option<&str>,
) -> Result<CommandOutcome> {
    let project_path = svc_util::resolve_project(root, project, cwd)?;

    let report = asset_svc::reconcile(&project_path, args.dry_run.dry_run, args.refresh_metadata)?;

    let scanned_count = report.added.len() + report.removed.len() + report.kept_count as usize;

    match format {
        Format::Json => {
            let data = serde_json::json!({
                "kind": "asset",
                "added": report.added,
                "removed": report.removed,
                "kept_count": report.kept_count,
                "scanned_count": scanned_count,
                "dry_run": args.dry_run.dry_run,
                "details": {
                    "refreshed": report.refreshed,
                },
            });
            Ok(CommandOutcome::Success(data, Vec::new(), None))
        }
        Format::Text => {
            let details = serde_json::json!({
                "added": report.added,
                "removed": report.removed,
                "kept_count": report.kept_count,
                "scanned_count": scanned_count,
                "refreshed": report.refreshed,
            });
            let result = VerbResult {
                verb: Verb::Index,
                kind: "asset",
                identity: String::new(),
                old_identity: None,
                path: None,
                dry_run: args.dry_run.dry_run,
                details,
            };
            Ok(CommandOutcome::Success(
                serde_json::Value::String(verb_text(&result, &VerbOpts::from_repo_root(Some(project_path.as_path())))),
                Vec::new(),
                None,
            ))
        }
    }
}

// ---------------------------------------------------------------------------
// Handle: mf asset clean
// ---------------------------------------------------------------------------

fn handle_clean(
    args: AssetCleanArgs,
    root: &Path,
    cwd: &Path,
    format: Format,
    project: Option<&str>,
) -> Result<CommandOutcome> {
    let project_path = svc_util::resolve_project(root, project, cwd)?;
    let report = asset_svc::clean(&project_path, args.dry_run.dry_run)?;

    match format {
        Format::Json => {
            let data = serde_json::json!({
                "stale_entries": report.stale_entries,
                "removed_count": report.removed_count,
                "dry_run": report.dry_run,
            });
            Ok(CommandOutcome::Success(data, Vec::new(), None))
        }
        Format::Text => {
            let dry_msg = if args.dry_run.dry_run { "[dry-run] " } else { "" };
            if report.stale_entries.is_empty() {
                Ok(CommandOutcome::Success(
                    serde_json::Value::String(format!("{dry_msg}No stale entries found.")),
                    Vec::new(),
                    None,
                ))
            } else {
                let mut lines = Vec::new();
                lines.push(format!("{dry_msg}Cleaned {} stale entries from index.", report.removed_count));
                for e in &report.stale_entries {
                    lines.push(format!("  removed: {}", e));
                }
                Ok(CommandOutcome::Raw(lines.join("\n"), None))
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Handle: mf asset remove
// ---------------------------------------------------------------------------

fn handle_remove(
    args: AssetRemoveArgs,
    root: &Path,
    cwd: &Path,
    format: Format,
    project: Option<&str>,
) -> Result<CommandOutcome> {
    let project_path = svc_util::resolve_project(root, project, cwd)?;
    identity::validate_entity_path(&project_path, &args.path)?;

    require_confirmation(&ConfirmArgs {
        verb_label: "removal",
        kind: "asset",
        identity: &args.path,
        yes: args.yes.yes,
        force: args.force.force,
    })?;

    let _report = asset_svc::remove_asset(&project_path, &args.path, args.force.force, args.dry_run.dry_run)?;

    let result = VerbResult {
        verb: Verb::Remove,
        kind: "asset",
        identity: args.path.clone(),
        old_identity: None,
        path: None,
        dry_run: args.dry_run.dry_run,
        details: serde_json::json!({"removed": true}),
    };
    match format {
        Format::Json => Ok(CommandOutcome::Success(verb_json(&result), Vec::new(), None)),
        Format::Text => Ok(CommandOutcome::Success(
            serde_json::Value::String(verb_text(&result, &VerbOpts::from_repo_root(Some(project_path.as_path())))),
            Vec::new(),
            None,
        )),
    }
}

// ── Handle: mf asset rename ─────────────────────────────────────────────────

fn handle_rename(
    args: AssetRenameArgs,
    root: &Path,
    cwd: &Path,
    format: Format,
    project: Option<&str>,
) -> Result<CommandOutcome> {
    let project_path = svc_util::resolve_project(root, project, cwd)?;
    identity::validate_entity_path(&project_path, &args.old_path)?;
    identity::validate_entity_path(&project_path, &args.new_path)?;

    if args.dry_run.dry_run {
        let result = VerbResult {
            verb: Verb::Rename,
            kind: "asset",
            identity: args.new_path.clone(),
            old_identity: Some(args.old_path.clone()),
            path: None,
            dry_run: true,
            details: serde_json::json!({}),
        };
        return match format {
            Format::Json => Ok(CommandOutcome::Success(verb_json(&result), Vec::new(), None)),
            Format::Text => Ok(CommandOutcome::Success(
                serde_json::Value::String(verb_text(&result, &VerbOpts::from_repo_root(Some(project_path.as_path())))),
                Vec::new(),
                None,
            )),
        };
    }

    let report = asset_svc::rename_asset(&project_path, &args.old_path, &args.new_path, args.force.force, false)?;

    let result = VerbResult {
        verb: Verb::Rename,
        kind: "asset",
        identity: report.after.path.clone(),
        old_identity: Some(report.before.path.clone()),
        path: Some(report.after.path.clone()),
        dry_run: false,
        details: serde_json::json!({}),
    };
    match format {
        Format::Json => Ok(CommandOutcome::Success(verb_json(&result), Vec::new(), None)),
        Format::Text => Ok(CommandOutcome::Success(
            serde_json::Value::String(verb_text(&result, &VerbOpts::from_repo_root(Some(project_path.as_path())))),
            Vec::new(),
            None,
        )),
    }
}

fn handle_asset_show(
    args: AssetShowArgs,
    root: &Path,
    cwd: &Path,
    format: Format,
    project: Option<&str>,
) -> Result<CommandOutcome> {
    let project_path = svc_util::resolve_project(root, project, cwd)?;
    identity::validate_entity_path(&project_path, &args.path)?;
    let assets = asset_svc::list(&project_path, None, None)?;

    // Prefer exact path match, then fall back to name match (legacy)
    let resolved = assets
        .iter()
        .find(|a| a.path == args.path)
        .or_else(|| assets.iter().find(|a| a.name.eq_ignore_ascii_case(&args.path)));

    match resolved {
        None => Err(MfError::usage(
            format!("asset '{}' not found", args.path),
            Some("use `mf asset list` to see available assets".to_string()),
        )),
        Some(asset) => {
            let kind_str =
                serde_json::to_value(asset.kind).ok().and_then(|v| v.as_str().map(String::from)).unwrap_or_default();
            let mut fields = vec![
                ShowField { label: "Name", value: ShowValue::Text(asset.name.clone()) },
                ShowField { label: "Type", value: ShowValue::Text(kind_str) },
                ShowField { label: "Path", value: ShowValue::Path(asset.path.clone()) },
                ShowField { label: "Size", value: ShowValue::Text(format!("{} bytes", asset.size)) },
            ];
            if !asset.hash.is_empty() {
                fields.push(ShowField { label: "Hash", value: ShowValue::Text(asset.hash.clone()) });
            }
            if !asset.tags.is_empty() {
                fields.push(ShowField { label: "Tags", value: ShowValue::Text(asset.tags.join(", ")) });
            }
            fields.push(ShowField { label: "Added", value: ShowValue::Text(asset.added_at.clone()) });

            let block = ShowBlock { kind: "asset", identity: asset.path.clone(), fields, sections: vec![] };

            match format {
                Format::Json => {
                    let asset_json = serde_json::to_value(asset).map_err(MfError::Json)?;
                    let extra = asset_json.as_object().cloned().unwrap_or_default();
                    Ok(CommandOutcome::Success(show_json(&block, extra), Vec::new(), None))
                }
                Format::Text => Ok(CommandOutcome::Raw(
                    render_show_text(&block, &ShowOpts::from_repo_root(Some(project_path.as_path()))),
                    None,
                )),
            }
        }
    }
}
