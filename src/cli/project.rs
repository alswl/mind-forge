use std::path::PathBuf;

use clap::{Args, Subcommand};
use serde::Serialize;

use crate::cli::{placeholder, CommandOutcome, HelpTarget};
use crate::error::{MfError, Result};
use crate::output::Format;
use crate::runtime::repo;

#[derive(Debug, Clone, Args)]
pub struct ProjectCmd {
    #[command(subcommand)]
    pub command: Option<ProjectSubcommand>,
}

#[derive(Debug, Clone, Subcommand)]
pub enum ProjectSubcommand {
    #[command(about = "Create a project")]
    New(ProjectNewArgs),
    #[command(about = "List projects")]
    List(ProjectListArgs),
    #[command(about = "Archive a project")]
    Archive(ProjectArchiveArgs),
    #[command(about = "Show project status")]
    Status(ProjectStatusArgs),
    #[command(about = "Lint a project")]
    Lint(ProjectLintArgs),
    #[command(about = "Index projects")]
    Index(ProjectIndexArgs),
}

#[derive(Debug, Clone, Args, Serialize)]
pub struct ProjectNewArgs {
    pub name: String,
    #[arg(long)]
    pub path: Option<PathBuf>,
    #[arg(long)]
    pub template: Option<String>,
    #[arg(long)]
    pub force: bool,
}

#[derive(Debug, Clone, Args, Serialize)]
pub struct ProjectListArgs {}

#[derive(Debug, Clone, Args, Serialize)]
pub struct ProjectArchiveArgs {
    pub name_or_path: String,
}

#[derive(Debug, Clone, Args, Serialize)]
pub struct ProjectStatusArgs {
    pub name_or_path: Option<String>,
    #[arg(long = "output-format")]
    pub output_format: Option<String>,
}

#[derive(Debug, Clone, Args, Serialize)]
pub struct ProjectLintArgs {
    pub path: Option<PathBuf>,
    #[arg(long)]
    pub fix: bool,
    #[arg(long = "rule")]
    pub rule: Vec<String>,
}

#[derive(Debug, Clone, Args, Serialize)]
pub struct ProjectIndexArgs {
    #[arg(long)]
    pub dry_run: bool,
}

/// dispatch 现在接受 repo_root 参数用于需要文件系统操作的子命令
pub fn dispatch(
    command: ProjectCmd,
    repo_root: Option<&PathBuf>,
    format: Format,
) -> Result<CommandOutcome> {
    match command.command {
        None => Ok(CommandOutcome::GroupHelp(HelpTarget::Project)),
        Some(ProjectSubcommand::New(args)) => {
            let resolved_path =
                args.path.clone().unwrap_or_else(|| PathBuf::from(format!("./{}", args.name)));
            if resolved_path.exists() && !args.force {
                return Err(MfError::usage(
                    format!(
                        "directory '{}' already exists; pass '--force' to overwrite",
                        resolved_path.display()
                    ),
                    None,
                ));
            }
            placeholder("mf project new", ProjectNewPayload::from(args))
        }
        Some(ProjectSubcommand::List(args)) => placeholder("mf project list", args),
        Some(ProjectSubcommand::Archive(args)) => placeholder("mf project archive", args),
        Some(ProjectSubcommand::Status(args)) => placeholder("mf project status", args),
        Some(ProjectSubcommand::Lint(args)) => placeholder("mf project lint", args),
        Some(ProjectSubcommand::Index(args)) => handle_index(args, repo_root, format),
    }
}

fn handle_index(
    args: ProjectIndexArgs,
    repo_root: Option<&PathBuf>,
    format: Format,
) -> Result<CommandOutcome> {
    // 无 repo_root 时使用 cwd（mf project index 可在 repo 外运行）
    let root = repo_root
        .cloned()
        .or_else(|| std::env::current_dir().ok())
        .ok_or_else(|| MfError::not_in_mind_repo())?;

    // 扫描项目目录
    let scanned = repo::scan_project_dirs(&root);

    // 加载或创建 minds.yaml
    let minds_path = root.join("minds.yaml");
    let manifest = if minds_path.exists() {
        repo::load_manifest(&minds_path)?
    } else {
        crate::model::manifest::MindsManifest::create_default()
    };

    // 计算 diff
    let diff = repo::compute_diff(&manifest, &scanned);

    if args.dry_run {
        // dry-run 模式：输出 diff，不写入
        let output = match format {
            Format::Json => serde_json::to_string_pretty(&diff).map_err(MfError::Json)?,
            Format::Text => repo::render_diff_text(&diff),
        };
        return Ok(CommandOutcome::Placeholder(crate::output::PlaceholderInvocation::new(
            "mf project index (dry-run)",
            serde_json::json!({"diff": output}),
        )));
    }

    // 执行 reconcile
    let updated = repo::reconcile(manifest, diff);

    // 原子写入
    repo::save_manifest(&updated, &minds_path)?;

    // 返回成功输出
    let payload = serde_json::json!({
        "projects_count": updated.projects.len(),
        "minds_path": minds_path.to_string_lossy().to_string(),
    });
    Ok(CommandOutcome::Placeholder(crate::output::PlaceholderInvocation::new(
        "mf project index",
        payload,
    )))
}

#[derive(Debug, Serialize)]
struct ProjectNewPayload {
    name: String,
    path: PathBuf,
    template: Option<String>,
    force: bool,
}

impl From<ProjectNewArgs> for ProjectNewPayload {
    fn from(value: ProjectNewArgs) -> Self {
        let path = value.path.unwrap_or_else(|| PathBuf::from(format!("./{}", value.name)));
        Self { name: value.name, path, template: value.template, force: value.force }
    }
}
