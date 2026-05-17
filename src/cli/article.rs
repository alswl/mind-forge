use std::path::PathBuf;

use clap::{Args, Subcommand};
use serde::Serialize;

use crate::cli::deprecation::DeprecationContext;
use crate::cli::CommandOutcome;
use crate::error::{MfError, Result};
use crate::model::config::TemplateMode;
use crate::output::Format;
use crate::service::{article as article_svc, config as config_svc, util as svc_util};

#[derive(Debug, Clone, Args)]
pub struct ArticleCmd {
    #[command(subcommand)]
    pub command: Option<ArticleSubcommand>,
}

#[derive(Debug, Clone, Subcommand)]
pub enum ArticleSubcommand {
    #[command(about = "Create an article")]
    New(ArticleNewArgs),
    #[command(about = "List articles", visible_alias = "ls")]
    List(ArticleListArgs),
    #[command(about = "Lint articles")]
    Lint(ArticleLintArgs),
    #[command(about = "Index articles (mf extension)")]
    Index(ArticleIndexArgs),
    #[command(about = "Rename an article")]
    Rename(ArticleRenameArgs),
}

#[derive(Debug, Clone, Args)]
pub struct ArticleNewArgs {
    /// Article type name (e.g. 'arch', 'blog')
    pub r#type: String,
    /// Article title
    pub title: String,
    #[arg(short = 'p', long)]
    pub project: Option<String>,
    #[arg(short = 't', long)]
    pub template: Option<String>,
    #[arg(long = "tag")]
    pub tag: Vec<String>,
    #[arg(long, default_value_t = true)]
    pub draft: bool,
    #[arg(short = 'f', long)]
    pub force: bool,
}

#[derive(Debug, Clone, Args, Serialize)]
pub struct ArticleListArgs {
    #[arg(short = 'p', long)]
    pub project: Option<String>,
}

#[derive(Debug, Clone, Args, Serialize)]
pub struct ArticleLintArgs {
    #[arg(long)]
    pub fix: bool,
}

#[derive(Debug, Clone, Args, Serialize)]
pub struct ArticleIndexArgs {
    #[arg(short = 'p', long)]
    pub project: Option<String>,
    #[arg(long, short = 'n')]
    pub dry_run: bool,
}

#[derive(Debug, Clone, Args)]
pub struct ArticleRenameArgs {
    /// Current article title
    pub old_title: String,
    /// New article title
    pub new_title: String,
    #[arg(short = 'p', long)]
    pub project: Option<String>,
    #[arg(short = 'f', long)]
    pub force: bool,
}

pub fn dispatch(
    command: ArticleCmd,
    repo_root: Option<&PathBuf>,
    format: Format,
    _deprecation: &mut DeprecationContext,
) -> Result<CommandOutcome> {
    let root = repo_root.ok_or_else(MfError::not_in_mind_repo)?;

    let cwd = std::env::current_dir().map_err(MfError::Io)?;

    match command.command {
        None => Ok(CommandOutcome::GroupHelp("article")),
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
                "type": args.r#type,
                "filename": filename,
                "path": format!("docs/{}.md", filename),
                "draft": args.draft,
            });
            Ok(CommandOutcome::Success(data, None))
        }
        Some(ArticleSubcommand::List(args)) => {
            let project_path = svc_util::resolve_project(root, args.project.as_deref(), &cwd)?;
            let articles = article_svc::list_articles(&project_path)?;
            let config = config_svc::load_project(&project_path, Some(root))?;

            // Compute source_dir for each article based on config
            let enriched: Vec<serde_json::Value> = articles
                .iter()
                .map(|a| {
                    let source_dir = config.as_ref().map(|cfg| article_svc::effective_source_dir(cfg, a));
                    let mut v = serde_json::to_value(a).unwrap_or_default();
                    if let Some(dir) = source_dir {
                        v["source_dir"] = serde_json::Value::String(dir);
                    }
                    v
                })
                .collect();

            match format {
                Format::Json => Ok(CommandOutcome::Success(serde_json::Value::Array(enriched), None)),
                Format::Text => {
                    if enriched.is_empty() {
                        return Ok(CommandOutcome::Success(serde_json::json!("No articles found."), None));
                    }
                    let mut lines = Vec::new();
                    for v in &enriched {
                        let title = v["title"].as_str().unwrap_or("");
                        let source_path = v["source_path"].as_str().unwrap_or("");
                        let source_dir = v["source_dir"].as_str().unwrap_or("");
                        let status = v["status"].as_str().unwrap_or("draft");
                        lines.push(format!("{title}  {source_path}  {source_dir}  {status}"));
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
                        return Ok(CommandOutcome::Success(serde_json::json!("No issues found."), None));
                    }
                    let mut lines = Vec::new();
                    for issue in &issues {
                        lines.push(format!("[{}] {}: {}  ({})", issue.severity, issue.kind, issue.message, issue.path));
                    }
                    Ok(CommandOutcome::Raw(lines.join("\n"), None))
                }
            }
        }
        Some(ArticleSubcommand::Index(args)) => {
            let project_path = svc_util::resolve_project(root, args.project.as_deref(), &cwd)?;
            let config = config_svc::load_project(&project_path, Some(root))?;

            let templates_scanned = config
                .as_ref()
                .and_then(|c| c.templates.as_ref())
                .map(|t| t.items.iter().filter(|(_, tmpl)| matches!(tmpl.mode, TemplateMode::Generated)).count())
                .unwrap_or(0);

            // Phase 1: Docs scan + diff + reconcile (existing behavior)
            let scanned = article_svc::scan_docs(&project_path)?;
            let index = crate::service::index::load(&project_path)?;
            let paths = config_svc::project_paths(&project_path)?;
            let diff = article_svc::compute_article_diff(&index, &scanned, &paths.docs);

            if args.dry_run {
                let data = serde_json::json!({
                    "added": diff.added,
                    "removed": diff.removed,
                    "dry_run": true,
                    "templates_scanned": templates_scanned,
                });
                return Ok(CommandOutcome::Success(data, None));
            }

            let mut updated = article_svc::reconcile_articles(&project_path, index, diff)?;

            // Phase 2: Merge template-discovered articles (US2)
            if let Some(ref config) = config {
                let template_articles = article_svc::scan_templates(&project_path, config)?;
                for ta in template_articles {
                    let articles = updated.articles.get_or_insert_with(Vec::new);
                    let pos = articles.iter().position(|a| a.source_path == ta.source_path);
                    if let Some(pos) = pos {
                        articles[pos].template_origin = ta.template_origin;
                    } else {
                        articles.push(ta);
                    }
                }
                if let Some(ref mut articles) = updated.articles {
                    articles.sort_by(|a, b| a.source_path.cmp(&b.source_path));
                }
            }

            crate::service::index::save(&project_path, &updated)?;
            let article_count = updated.articles.as_ref().map(|a| a.len()).unwrap_or(0);

            let data = serde_json::json!({
                "articles_count": article_count,
                "project_path": project_path.to_string_lossy().to_string(),
                "templates_scanned": templates_scanned,
            });
            Ok(CommandOutcome::Success(data, None))
        }
        Some(ArticleSubcommand::Rename(args)) => {
            let project_path = svc_util::resolve_project(root, args.project.as_deref(), &cwd)?;
            let report = article_svc::rename_article(&project_path, &args.old_title, &args.new_title, args.force)?;

            match format {
                Format::Json => {
                    Ok(CommandOutcome::Success(serde_json::to_value(&report).map_err(MfError::Json)?, None))
                }
                Format::Text => {
                    let msg = format!(
                        "Renamed article\n  title: {} → {}\n  file: {} → {}",
                        report.old_title, report.new_title, report.old_source_path, report.new_source_path
                    );
                    Ok(CommandOutcome::Raw(msg, None))
                }
            }
        }
    }
}
