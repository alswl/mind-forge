use std::path::{Path, PathBuf};

use clap::{Args, Subcommand};
use serde::Serialize;

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
    #[command(about = "List assets")]
    List(AssetListArgs),
    #[command(about = "Add an asset")]
    Add(AssetAddArgs),
    #[command(about = "Update assets")]
    Update(AssetUpdateArgs),
    #[command(about = "Index assets")]
    Index(AssetIndexArgs),
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
    #[arg(long)]
    pub project: Option<String>,
}

// ---------------------------------------------------------------------------
// Asset add args
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Args)]
pub struct AssetAddArgs {
    pub path: PathBuf,
    #[arg(long = "tag")]
    pub tag: Vec<String>,
    #[arg(long, conflicts_with = "link")]
    pub copy: bool,
    #[arg(long, conflicts_with = "copy")]
    pub link: bool,
    #[arg(long)]
    pub force: bool,
    #[arg(long)]
    pub project: Option<String>,
}

// ---------------------------------------------------------------------------
// Asset update args
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Args, Serialize)]
pub struct AssetUpdateArgs {
    pub path: Option<PathBuf>,
    #[arg(long)]
    pub all: bool,
    #[arg(long)]
    pub project: Option<String>,
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
    #[arg(long)]
    pub project: Option<String>,
}

// ---------------------------------------------------------------------------
// Dispatch
// ---------------------------------------------------------------------------

pub fn dispatch(command: AssetCmd, repo_root: Option<&PathBuf>, format: Format) -> Result<CommandOutcome> {
    let root = repo_root.ok_or_else(MfError::not_in_mind_repo)?;
    let cwd = std::env::current_dir().map_err(MfError::Io)?;

    match command.command {
        None => Ok(CommandOutcome::GroupHelp("asset")),
        Some(AssetSubcommand::Add(args)) => handle_add(args, root, &cwd, format),
        Some(AssetSubcommand::List(args)) => handle_list(args, root, &cwd, format),
        Some(AssetSubcommand::Update(args)) => handle_update(args, root, &cwd, format),
        Some(AssetSubcommand::Index(args)) => handle_index(args, root, &cwd, format),
    }
}

// ---------------------------------------------------------------------------
// Handle: mf asset add
// ---------------------------------------------------------------------------

fn handle_add(args: AssetAddArgs, root: &Path, cwd: &Path, format: Format) -> Result<CommandOutcome> {
    let project_path = svc_util::resolve_project(root, args.project.as_deref(), cwd)?;
    let add_args = asset_svc::AddArgs { source: args.path, tags: args.tag, link_mode: args.link, force: args.force };
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

fn handle_list(args: AssetListArgs, root: &Path, cwd: &Path, format: Format) -> Result<CommandOutcome> {
    let project_path = svc_util::resolve_project(root, args.project.as_deref(), cwd)?;
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

fn handle_update(args: AssetUpdateArgs, root: &Path, cwd: &Path, format: Format) -> Result<CommandOutcome> {
    let project_path = svc_util::resolve_project(root, args.project.as_deref(), cwd)?;

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

fn handle_index(args: AssetIndexArgs, root: &Path, cwd: &Path, format: Format) -> Result<CommandOutcome> {
    let project_path = svc_util::resolve_project(root, args.project.as_deref(), cwd)?;

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
