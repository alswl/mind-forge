use std::path::PathBuf;

use clap::{Args, Subcommand};
use serde::Serialize;

use crate::cli::CommandOutcome;
use crate::error::{MfError, Result};
use crate::model::article::ArticleStatus;
use crate::output::Format;
use crate::service::{article as article_svc, util as svc_util};

#[derive(Debug, Clone, Args)]
pub struct ArticleCmd {
    #[command(subcommand)]
    pub command: Option<ArticleSubcommand>,
}

#[derive(Debug, Clone, Subcommand)]
pub enum ArticleSubcommand {
    #[command(about = "Create an article")]
    New(ArticleNewArgs),
    #[command(about = "List articles")]
    List(ArticleListArgs),
    #[command(about = "Lint articles")]
    Lint(ArticleLintArgs),
    #[command(about = "Index articles")]
    Index(ArticleIndexArgs),
}

#[derive(Debug, Clone, Args)]
pub struct ArticleNewArgs {
    pub title: String,
    #[arg(long)]
    pub project: Option<String>,
    #[arg(long)]
    pub template: Option<String>,
    #[arg(long = "tag")]
    pub tag: Vec<String>,
    #[arg(long, default_value_t = true)]
    pub draft: bool,
    #[arg(long)]
    pub force: bool,
}

#[derive(Debug, Clone, Args, Serialize)]
pub struct ArticleListArgs {
    #[arg(long)]
    pub project: Option<String>,
}

#[derive(Debug, Clone, Args, Serialize)]
pub struct ArticleLintArgs {
    #[arg(long)]
    pub fix: bool,
}

#[derive(Debug, Clone, Args, Serialize)]
pub struct ArticleIndexArgs {
    #[arg(long)]
    pub project: Option<String>,
    #[arg(long, short = 'n')]
    pub dry_run: bool,
}

pub fn dispatch(
    command: ArticleCmd,
    repo_root: Option<&PathBuf>,
    format: Format,
) -> Result<CommandOutcome> {
    let root = repo_root.ok_or_else(MfError::not_in_mind_repo)?;

    let cwd = std::env::current_dir().map_err(MfError::Io)?;

    match command.command {
        None => Ok(CommandOutcome::GroupHelp(super::HelpTarget::Article)),
        Some(ArticleSubcommand::New(args)) => {
            let project_path = svc_util::resolve_project(root, args.project.as_deref(), &cwd)?;

            let template_text = match args.template {
                Some(ref path) => {
                    let tmpl_path = project_path.join(path);
                    Some(std::fs::read_to_string(&tmpl_path).map_err(|e| {
                        MfError::usage(
                            format!("cannot read template '{}': {e}", tmpl_path.display()),
                            Some("use a path relative to the project root".to_string()),
                        )
                    })?)
                }
                None => None,
            };

            let filename = article_svc::new_article(
                &project_path,
                &args.title,
                template_text.as_deref(),
                &args.tag,
                args.draft,
                args.force,
            )?;

            let data = serde_json::json!({
                "filename": filename,
                "path": format!("docs/{}.md", filename),
                "draft": args.draft,
            });
            Ok(CommandOutcome::Success(data, None))
        }
        Some(ArticleSubcommand::List(args)) => {
            let project_path = svc_util::resolve_project(root, args.project.as_deref(), &cwd)?;
            let articles = article_svc::list_articles(&project_path)?;

            match format {
                Format::Json => Ok(CommandOutcome::Success(serde_json::to_value(&articles)?, None)),
                Format::Text => {
                    if articles.is_empty() {
                        return Ok(CommandOutcome::Success(
                            serde_json::json!("No articles found."),
                            None,
                        ));
                    }
                    let mut lines = Vec::new();
                    for a in &articles {
                        let status = match a.status {
                            ArticleStatus::Draft => "draft",
                            ArticleStatus::Published => "published",
                        };
                        lines.push(format!("{}  {}  {}", a.title, a.source_path, status));
                    }
                    Ok(CommandOutcome::Raw(lines.join("\n"), None))
                }
            }
        }
        Some(ArticleSubcommand::Lint(args)) => {
            let project_path = svc_util::resolve_project(root, None, &cwd)?;
            let issues = article_svc::lint_articles(&project_path, args.fix)?;

            match format {
                Format::Json => Ok(CommandOutcome::Success(serde_json::to_value(&issues)?, None)),
                Format::Text => {
                    if issues.is_empty() {
                        return Ok(CommandOutcome::Success(
                            serde_json::json!("No issues found."),
                            None,
                        ));
                    }
                    let mut lines = Vec::new();
                    for issue in &issues {
                        lines.push(format!(
                            "[{}] {}: {}  ({})",
                            issue.severity, issue.kind, issue.message, issue.path
                        ));
                    }
                    Ok(CommandOutcome::Raw(lines.join("\n"), None))
                }
            }
        }
        Some(ArticleSubcommand::Index(args)) => {
            let project_path = svc_util::resolve_project(root, args.project.as_deref(), &cwd)?;
            let scanned = article_svc::scan_docs(&project_path)?;
            let index = article_svc::load_index(&project_path)?;
            let diff = article_svc::compute_article_diff(&index, &scanned);

            if args.dry_run {
                let data = serde_json::json!({
                    "added": diff.added,
                    "removed": diff.removed,
                    "dry_run": true,
                });
                return Ok(CommandOutcome::Success(data, None));
            }

            let updated = article_svc::reconcile_articles(&project_path, index, diff)?;
            article_svc::save_index(&updated, &project_path)?;
            let article_count = updated.articles.as_ref().map(|a| a.len()).unwrap_or(0);

            let data = serde_json::json!({
                "articles_count": article_count,
                "project_path": project_path.to_string_lossy().to_string(),
            });
            Ok(CommandOutcome::Success(data, None))
        }
    }
}
