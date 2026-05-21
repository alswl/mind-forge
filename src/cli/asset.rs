use std::path::{Path, PathBuf};

use clap::{Args, Subcommand};
use serde::Serialize;

use crate::cli::deprecation::DeprecationContext;
use crate::cli::CommandOutcome;
use crate::error::{MfError, Result};
use crate::model::asset::AssetKind;
use crate::output::Format;
use crate::service::{asset as asset_svc, util as svc_util};

#[derive(Debug, Clone, Args)]
pub struct AssetCmd {
    #[command(subcommand)]
    pub command: Option<AssetSubcommand>,
}

#[derive(Debug, Clone, Subcommand)]
pub enum AssetSubcommand {
    #[command(about = "List assets", visible_alias = "ls")]
    List(AssetListArgs),
    #[command(about = "Add an asset")]
    Add(AssetAddArgs),
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
    #[arg(short = 'f', long)]
    pub force: bool,
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
}

// ---------------------------------------------------------------------------
// Asset index args
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Args, Serialize)]
pub struct AssetIndexArgs {
    #[arg(long)]
    pub dry_run: bool,
    #[arg(long = "refresh-metadata")]
    pub refresh_metadata: bool,
}

// ---------------------------------------------------------------------------
// Asset clean args
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Args, Serialize)]
pub struct AssetCleanArgs {
    #[arg(long)]
    pub dry_run: bool,
}

// ---------------------------------------------------------------------------
// Asset remove args
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Args, Serialize)]
pub struct AssetShowArgs {
    pub name: String,
}

#[derive(Debug, Clone, Args, Serialize)]
pub struct AssetRemoveArgs {
    pub file: String,
    #[arg(short = 'f', long)]
    pub force: bool,
    #[arg(long = "dry-run")]
    pub dry_run: bool,
}

#[derive(Debug, Clone, Args, Serialize)]
pub struct AssetRenameArgs {
    pub old_path: String,
    pub new_path: String,
    #[arg(short = 'f', long)]
    pub force: bool,
    #[arg(long = "dry-run")]
    pub dry_run: bool,
}

// ---------------------------------------------------------------------------
// Dispatch
// ---------------------------------------------------------------------------

pub fn dispatch(
    command: AssetCmd,
    repo_root: Option<&PathBuf>,
    format: Format,
    project: Option<&str>,
    _deprecation: &mut DeprecationContext,
) -> Result<CommandOutcome> {
    let root = repo_root.ok_or_else(MfError::not_in_mind_repo)?;
    let cwd = std::env::current_dir().map_err(MfError::Io)?;

    match command.command {
        None => Ok(CommandOutcome::GroupHelp("asset")),
        Some(AssetSubcommand::Add(args)) => handle_add(args, root, &cwd, format, project),
        Some(AssetSubcommand::List(args)) => handle_list(args, root, &cwd, format, project),
        Some(AssetSubcommand::Update(args)) => handle_update(args, root, &cwd, format, project),
        Some(AssetSubcommand::Index(args)) => handle_index(args, root, &cwd, format, project),
        Some(AssetSubcommand::Clean(args)) => handle_clean(args, root, format, project),
        Some(AssetSubcommand::Show(args)) => handle_asset_show(args, root, &cwd, format, project),
        Some(AssetSubcommand::Remove(args)) => handle_remove(args, root, &cwd, format, project),
        Some(AssetSubcommand::Rename(args)) => handle_rename(args, root, &cwd, format, project),
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
    let add_args = asset_svc::AddArgs {
        source: args.path,
        name: args.name,
        tags: args.tag,
        link_mode: args.link,
        force: args.force,
    };
    let asset = asset_svc::add(&project_path, cwd, &add_args)?;
    let mode = if add_args.link_mode { "link" } else { "copy" };

    match format {
        Format::Json => {
            let data = serde_json::json!({
                "name": asset.name,
                "type": asset.kind,
                "path": asset.path,
                "size": asset.size,
                "hash": asset.hash,
                "tags": asset.tags,
                "added_at": asset.added_at,
                "mode": mode,
            });
            Ok(CommandOutcome::Success(data, None))
        }
        Format::Text => {
            let msg = format!(
                "✓ added asset: {} ({}, {} bytes)",
                asset.path,
                serde_json::to_value(asset.kind)
                    .map(|v| v.as_str().unwrap_or("unknown").to_string())
                    .unwrap_or_else(|_| "unknown".to_string()),
                asset.size,
            );
            Ok(CommandOutcome::Success(serde_json::Value::String(msg), None))
        }
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

    match format {
        Format::Json => {
            let data = serde_json::to_value(&assets).map_err(MfError::Json)?;
            Ok(CommandOutcome::Success(data, None))
        }
        Format::Text => {
            if assets.is_empty() {
                return Ok(CommandOutcome::Success(serde_json::Value::String("No assets found.".to_string()), None));
            }
            let mut lines = Vec::new();
            lines.push(format!("{:<24} {:<8} {:<28} {}", "NAME", "TYPE", "PATH", "SIZE"));
            for a in &assets {
                let kind_str =
                    serde_json::to_value(a.kind).ok().and_then(|v| v.as_str().map(String::from)).unwrap_or_default();
                lines.push(format!("{:<24} {:<8} {:<28} {}", a.name, kind_str, a.path, a.size,));
            }
            Ok(CommandOutcome::Raw(lines.join("\n"), None))
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
                None,
            )),
            Format::Text => {
                let msg = format!("Set publish URL: {} (channel: {})", result.url, result.channel);
                Ok(CommandOutcome::Success(serde_json::Value::String(msg), None))
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
        let result = asset_svc::update_one(&project_path, cwd, &path)?;
        match format {
            Format::Json => {
                let data = serde_json::to_value(&result).map_err(MfError::Json)?;
                Ok(CommandOutcome::Success(data, None))
            }
            Format::Text => {
                let msg = if result.changed {
                    format!(
                        "✓ updated {} (size {} → {}, hash {} → {})",
                        result.path,
                        result.old_size,
                        result.new_size,
                        &result.old_hash[..8.min(result.old_hash.len())],
                        &result.new_hash[..8.min(result.new_hash.len())],
                    )
                } else {
                    format!("= unchanged {}", result.path)
                };
                Ok(CommandOutcome::Success(serde_json::Value::String(msg), None))
            }
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
                Ok(CommandOutcome::Success(data, None))
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

    let report = asset_svc::reconcile(&project_path, args.dry_run, args.refresh_metadata)?;

    match format {
        Format::Json => {
            let mut data = serde_json::json!({
                "added": report.added,
                "removed": report.removed,
                "kept_count": report.kept_count,
                "refreshed": report.refreshed,
            });
            if args.dry_run {
                if let Some(m) = data.as_object_mut() {
                    m.insert("dry_run".to_string(), serde_json::Value::Bool(true));
                }
            }
            Ok(CommandOutcome::Success(data, None))
        }
        Format::Text => {
            let mut lines = Vec::new();
            let dry_run_prefix = if args.dry_run { "[dry-run] " } else { "" };
            for a in &report.added {
                lines.push(format!("{dry_run_prefix}+ added   {}", a.path));
            }
            for r in &report.removed {
                lines.push(format!("{dry_run_prefix}- removed {}", r.path));
            }
            if let Some(ref refreshed) = report.refreshed {
                for r in refreshed {
                    if r.changed {
                        lines.push(format!(
                            "{dry_run_prefix}~ refreshed {} (size {} → {})",
                            r.path, r.old_size, r.new_size,
                        ));
                    }
                }
            }
            let kept_msg = if args.dry_run {
                format!("{dry_run_prefix}{} kept (no changes written)", report.kept_count)
            } else {
                format!("{} kept", report.kept_count)
            };
            lines.push(kept_msg);
            Ok(CommandOutcome::Raw(lines.join("\n"), None))
        }
    }
}

// ---------------------------------------------------------------------------
// Handle: mf asset clean
// ---------------------------------------------------------------------------

fn handle_clean(args: AssetCleanArgs, root: &Path, format: Format, project: Option<&str>) -> Result<CommandOutcome> {
    let cwd = std::env::current_dir().map_err(MfError::Io)?;
    let project_path = svc_util::resolve_project(root, project, &cwd)?;
    let report = asset_svc::clean(&project_path, args.dry_run)?;

    match format {
        Format::Json => {
            let data = serde_json::json!({
                "stale_entries": report.stale_entries,
                "removed_count": report.removed_count,
                "dry_run": report.dry_run,
            });
            Ok(CommandOutcome::Success(data, None))
        }
        Format::Text => {
            let dry_msg = if args.dry_run { "[dry-run] " } else { "" };
            if report.stale_entries.is_empty() {
                Ok(CommandOutcome::Success(
                    serde_json::Value::String(format!("{dry_msg}No stale entries found.")),
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
    let report = asset_svc::remove_asset(&project_path, &args.file, args.force, args.dry_run)?;

    match format {
        Format::Json => {
            let data = serde_json::to_value(&report).map_err(MfError::Json)?;
            Ok(CommandOutcome::Success(data, None))
        }
        Format::Text => {
            let prefix = if report.dry_run { "[dry-run] would remove" } else { "✓ removed" };
            let msg = format!("{prefix} asset: {} (referenced: {})", report.removed, report.was_referenced);
            Ok(CommandOutcome::Success(serde_json::Value::String(msg), None))
        }
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
    let report = asset_svc::rename_asset(&project_path, &args.old_path, &args.new_path, args.force, args.dry_run)?;

    match format {
        Format::Json => {
            let data = serde_json::to_value(&report).map_err(MfError::Json)?;
            Ok(CommandOutcome::Success(data, None))
        }
        Format::Text => {
            let prefix = if report.dry_run { "[dry-run] would rename" } else { "✓ renamed" };
            let msg = format!("{} asset: {} → {}", prefix, report.before.path, report.after.path);
            Ok(CommandOutcome::Success(serde_json::Value::String(msg), None))
        }
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
    let assets = asset_svc::list(&project_path, None, None)?;

    let resolved = assets.iter().find(|a| a.name.eq_ignore_ascii_case(&args.name));

    match resolved {
        None => Err(MfError::usage(
            format!("asset '{}' not found", args.name),
            Some("use `mf asset list` to see available assets".to_string()),
        )),
        Some(asset) => match format {
            Format::Json => Ok(CommandOutcome::Success(serde_json::to_value(asset).map_err(MfError::Json)?, None)),
            Format::Text => {
                let kind_str = serde_json::to_value(asset.kind)
                    .ok()
                    .and_then(|v| v.as_str().map(String::from))
                    .unwrap_or_default();
                let mut lines = Vec::new();
                lines.push(format!("Name: {}", asset.name));
                lines.push(format!("Type: {kind_str}"));
                lines.push(format!("Path: {}", asset.path));
                lines.push(format!("Size: {} bytes", asset.size));
                if !asset.hash.is_empty() {
                    lines.push(format!("Hash: {}", asset.hash));
                }
                if !asset.tags.is_empty() {
                    lines.push(format!("Tags: {}", asset.tags.join(", ")));
                }
                Ok(CommandOutcome::Raw(lines.join("\n"), None))
            }
        },
    }
}
