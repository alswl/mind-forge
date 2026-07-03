use std::path::Path;

use clap::{Args, Subcommand};
use serde::Serialize;

use crate::cli::shared_flags::DryRunFlag;
use crate::cli::shared_flags::ForceFlag;
use crate::cli::shared_flags::LintFlags;
use crate::cli::shared_flags::NoHeadersFlag;
use crate::cli::shared_flags::NoTruncFlag;
use crate::cli::shared_flags::YesFlag;
use crate::cli::CommandCtx;
use crate::cli::CommandOutcome;
use crate::defaults;
use crate::error::{MfError, Result};
use crate::model::article::{
    ArticleStatus, ConversionDirection, ConversionResult, ConversionStatus, ConversionSummary, DirectionSource,
};
use crate::model::config::TemplateMode;
use crate::output::confirm::{prompt_confirmation, require_confirmation, ConfirmArgs, ConfirmOutcome};
use crate::output::list::{json_collection, render_text, ListCell, ListOpts, ListRow, ListView};
use crate::output::show::{json_envelope, render_text as render_show_text, ShowBlock, ShowField, ShowOpts, ShowValue};
use crate::output::verb::{json_envelope as verb_json, render_text as verb_text, Verb, VerbOpts, VerbResult};
use crate::output::Format;
use crate::service::{article as article_svc, config as config_svc, identity, util as svc_util};

use self::block::handle_block_rename;
use self::convert::handle_convert;
use self::lint::handle_lint;
use self::show::handle_article_show;

mod block;
mod convert;
mod lint;
mod show;

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
    #[command(about = "Index articles")]
    Index(ArticleIndexArgs),
    #[command(about = "Rename an article")]
    Rename(ArticleRenameArgs),
    #[command(about = "Remove an article", visible_alias = "rm")]
    Remove(ArticleRemoveArgs),
    #[command(about = "Show article details")]
    Show(ArticleShowArgs),
    #[command(about = "Update article metadata")]
    Update(ArticleUpdateArgs),
    #[command(subcommand, about = "Manage blocks within a directory article")]
    Block(ArticleBlockSubcommand),
    #[command(about = "Convert article shape between directory and single-file")]
    Convert(ArticleConvertArgs),
}

#[derive(Debug, Clone, Args)]
pub struct ArticleNewArgs {
    /// Article title (sole positional)
    pub title: String,
    /// Template: built-in schema name (blank/arch/prd/blog) or path under project root. Default: blank.
    #[arg(short = 't', long, default_value = "blank")]
    pub template: String,
    /// Write a single file instead of a directory
    #[arg(long = "file", visible_alias = "single-file", default_value_t = false)]
    pub file: bool,
    #[arg(long = "tag")]
    pub tag: Vec<String>,
    #[arg(long, default_value_t = true)]
    pub draft: bool,
    #[command(flatten)]
    pub force: ForceFlag,
    #[command(flatten)]
    pub dry_run: DryRunFlag,
}

#[derive(Debug, Clone, Args, Serialize)]
pub struct ArticleListArgs {
    #[command(flatten)]
    pub no_headers: NoHeadersFlag,
    #[command(flatten)]
    pub no_trunc: NoTruncFlag,
}

#[derive(Debug, Clone, Args, Serialize)]
pub struct ArticleLintArgs {
    #[command(flatten)]
    pub lint: LintFlags,
}

#[derive(Debug, Clone, Args, Serialize)]
pub struct ArticleIndexArgs {
    #[command(flatten)]
    pub dry_run: DryRunFlag,
}

#[derive(Debug, Clone, Args)]
pub struct ArticleShowArgs {
    /// Article path (e.g. docs/weekly.md) or title
    pub path: String,
}

#[derive(Debug, Clone, Args)]
pub struct ArticleUpdateArgs {
    /// Article path (e.g. docs/weekly.md) or title
    pub path: String,
    /// Article publication status
    #[arg(long)]
    pub status: Option<String>,
    /// New article title (metadata only, does not rename files)
    #[arg(long)]
    pub title: Option<String>,
    #[command(flatten)]
    pub dry_run: DryRunFlag,
}

#[derive(Debug, Clone, Args)]
pub struct ArticleRemoveArgs {
    /// Article path (e.g. docs/weekly.md) or title
    pub path: String,
    #[command(flatten)]
    pub force: ForceFlag,
    #[command(flatten)]
    pub yes: YesFlag,
    #[command(flatten)]
    pub dry_run: DryRunFlag,
}

#[derive(Debug, Clone, Args)]
pub struct ArticleRenameArgs {
    /// Current article path or title
    pub old_path: String,
    /// New slug (e.g. "new-slug") — renames the file/directory, title is unchanged
    pub new_path: String,
    #[command(flatten)]
    pub force: ForceFlag,
    #[command(flatten)]
    pub dry_run: DryRunFlag,
}

#[derive(Debug, Clone, Args)]
pub struct ArticleConvertArgs {
    /// Convert directory articles to single-file articles
    #[arg(long = "to-single-file", conflicts_with = "to_directory")]
    pub to_single_file: bool,
    /// Convert single-file articles to directory articles
    #[arg(long = "to-directory", conflicts_with = "to_single_file")]
    pub to_directory: bool,
    /// Preview conversions without writing changes
    #[command(flatten)]
    pub dry_run: DryRunFlag,
}

#[derive(Debug, Clone, Subcommand)]
pub enum ArticleBlockSubcommand {
    #[command(about = "Rename a block within a directory article")]
    Rename(ArticleBlockRenameArgs),
}

#[derive(Debug, Clone, Args)]
pub struct ArticleBlockRenameArgs {
    /// Article path (e.g. docs/my-article) or title
    pub article: String,
    /// Current block filename (e.g. "02-notes.md") or slug (e.g. "notes")
    pub old_block: String,
    /// New slug — the number prefix is preserved (e.g. "thoughts" → "02-thoughts.md")
    pub new_slug: String,
    #[command(flatten)]
    pub force: ForceFlag,
    #[command(flatten)]
    pub dry_run: DryRunFlag,
}

pub fn dispatch(command: ArticleCmd, ctx: &mut CommandCtx) -> Result<CommandOutcome> {
    let root = ctx.require_repo_path()?;

    let cwd = ctx.cwd();
    let format = ctx.format();
    let project = ctx.project();

    match command.command {
        None => Ok(CommandOutcome::GroupHelp("article")),
        Some(ArticleSubcommand::New(args)) => {
            let project_path = svc_util::resolve_project(root, project, cwd)?;

            if args.dry_run.dry_run {
                let filename = svc_util::to_filename(&args.title);
                let layout = config_svc::effective_layout(&project_path)?;
                let identity = if args.file {
                    format!("{}/{}.{}", layout.articles, filename, defaults::MARKDOWN_EXTENSION)
                } else {
                    format!("{}/{}", layout.articles, filename)
                };

                // Bug #10 fix: dry-run must run the same parse + block-slug
                // validation as the real run so outcomes always agree.
                let resolved = article_svc::resolve_template(&project_path, &args.template, &args.title)?;
                article_svc::validate_template_blocks(&resolved)?;

                let result = VerbResult {
                    verb: Verb::Create,
                    kind: "article",
                    identity,
                    old_identity: None,
                    path: None,
                    dry_run: true,
                    details: serde_json::json!({"title": args.title, "template": args.template}),
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

            let svc_result = article_svc::new_article(
                &project_path,
                &args.title,
                &args.template,
                args.file,
                &args.tag,
                args.draft,
                args.force.force,
            )?;

            let article_path = if svc_result.shape == "directory" {
                format!("{}/{}/", svc_result.docs_dir, svc_result.filename)
            } else {
                format!("{}/{}.{}", svc_result.docs_dir, svc_result.filename, defaults::MARKDOWN_EXTENSION)
            };

            let result = VerbResult {
                verb: Verb::Create,
                kind: "article",
                identity: article_path.clone(),
                old_identity: None,
                path: Some(article_path.clone()),
                dry_run: false,
                details: serde_json::json!({
                    "title": args.title,
                    "filename": svc_result.filename,
                    "draft": args.draft,
                    "template": svc_result.template,
                    "shape": svc_result.shape,
                    "path": article_path,
                    "files": svc_result.files,
                    "typora_front_matter_injected": svc_result.typora_front_matter_injected,
                    "typora_copy_images_to": svc_result.typora_copy_images_to,
                }),
            };
            match format {
                Format::Json => Ok(CommandOutcome::Success(verb_json(&result), Vec::new(), None)),
                Format::Text => Ok(CommandOutcome::Success(
                    serde_json::Value::String(verb_text(
                        &result,
                        &VerbOpts::from_repo_root(Some(project_path.as_path())),
                    )),
                    Vec::new(),
                    None,
                )),
            }
        }
        Some(ArticleSubcommand::List(args)) => {
            let detected = project.map_or_else(|| svc_util::detect_current_project(root, cwd), |p| Some(p.to_string()));

            match detected {
                None => {
                    // No --project and not in a project directory: list all projects
                    let all = article_svc::list_articles_all_projects(root)?;
                    let mut rows = Vec::with_capacity(all.len());
                    let mut json_items = Vec::with_capacity(all.len());
                    for (article, project_path, mtime) in &all {
                        let repo_rel = project_path.strip_prefix(root).unwrap_or(project_path);
                        let full_path = repo_rel.join(&article.article_path);
                        let path_str = full_path.to_string_lossy().replace('\\', "/");
                        let status = match article.status {
                            ArticleStatus::Draft => "draft",
                            ArticleStatus::Published => "published",
                        };
                        rows.push(ListRow {
                            cells: vec![
                                ListCell::Path(path_str),
                                ListCell::Text(article.project.clone()),
                                ListCell::Text(article.title.clone()),
                                status_cell(status),
                                ListCell::Text(format_mtime(*mtime)),
                            ],
                        });
                        let mut v = serde_json::to_value(article).unwrap_or_default();
                        v["mtime"] = serde_json::Value::Number((*mtime).into());
                        json_items.push(v);
                    }
                    let view = ListView {
                        headers: &["PATH", "PROJECT", "TITLE", "STATUS", "UPDATED"],
                        rows,
                        plural_noun: "articles",
                    };
                    let opts = ListOpts::from_flags(args.no_headers.no_headers, args.no_trunc.no_trunc)
                        .with_repo_root(Some(root.to_path_buf()));
                    match format {
                        Format::Json => {
                            Ok(CommandOutcome::Success(json_collection("articles", json_items), Vec::new(), None))
                        }
                        Format::Text => Ok(CommandOutcome::Raw(render_text(&view, &opts), None)),
                    }
                }
                Some(resolved) => {
                    // --project specified or detected from cwd: single-project behavior
                    let project_path = svc_util::resolve_project(root, Some(&resolved), cwd)?;
                    let articles = article_svc::list_articles(&project_path)?;
                    let config = config_svc::load_project(&project_path, Some(root))?;

                    let mut enriched: Vec<(serde_json::Value, u64)> = articles
                        .iter()
                        .map(|a| {
                            let article_dir =
                                config.as_ref().map(|cfg| article_svc::effective_article_dir(&project_path, cfg, a));
                            let mut v = serde_json::to_value(a).unwrap_or_default();
                            if let Some(dir) = article_dir {
                                v["article_dir"] = serde_json::Value::String(dir);
                            }
                            if let Ok(key) = crate::service::index::article_key(a) {
                                v["id"] = serde_json::Value::String(key);
                            }
                            let origin = if a.template_origin.is_some() {
                                "generated"
                            } else {
                                let is_declared = config.as_ref().is_some_and(|cfg| {
                                    let short_key = crate::service::index::article_output_stem(&a.article_path);
                                    cfg.build.articles.contains_key(short_key)
                                        || cfg
                                            .articles
                                            .as_ref()
                                            .and_then(|v| v.as_object())
                                            .is_some_and(|map| map.contains_key(short_key))
                                });
                                if is_declared {
                                    "declared"
                                } else {
                                    "docs"
                                }
                            };
                            v["origin"] = serde_json::Value::String(origin.to_string());
                            v["article_present"] = serde_json::Value::Bool(project_path.join(&a.article_path).exists());
                            let content_kind = article_content_kind(&project_path, &a.article_path);
                            v["content_kind"] = serde_json::Value::String(content_kind.to_string());
                            v["identity"] = serde_json::Value::String(a.article_path.clone());
                            v["path"] = serde_json::Value::String(a.article_path.clone());
                            let mtime = article_svc::article_file_mtime(&project_path, &a.article_path);
                            v["mtime"] = serde_json::Value::Number(mtime.into());
                            (v, mtime)
                        })
                        .collect();

                    enriched.sort_by(|a, b| {
                        b.1.cmp(&a.1).then_with(|| a.0["identity"].as_str().cmp(&b.0["identity"].as_str()))
                    });

                    let opts = ListOpts::from_flags(args.no_headers.no_headers, args.no_trunc.no_trunc)
                        .with_repo_root(Some(project_path.clone()));

                    match format {
                        Format::Json => {
                            let items: Vec<serde_json::Value> = enriched.into_iter().map(|(v, _)| v).collect();
                            Ok(CommandOutcome::Success(json_collection("articles", items), Vec::new(), None))
                        }
                        Format::Text => {
                            let mut rows = Vec::with_capacity(enriched.len());
                            for (v, mtime) in &enriched {
                                let identity = v["identity"].as_str().unwrap_or("").to_string();
                                let title = v["title"].as_str().unwrap_or("").to_string();
                                let status = v["status"].as_str().unwrap_or("draft");
                                rows.push(ListRow {
                                    cells: vec![
                                        ListCell::Path(identity),
                                        ListCell::Text(title),
                                        status_cell(status),
                                        ListCell::Text(format_mtime(*mtime)),
                                    ],
                                });
                            }
                            let view = ListView {
                                headers: &["PATH", "TITLE", "STATUS", "UPDATED"],
                                rows,
                                plural_noun: "articles",
                            };
                            Ok(CommandOutcome::Raw(render_text(&view, &opts), None))
                        }
                    }
                }
            }
        }
        Some(ArticleSubcommand::Lint(args)) => {
            let project_path = svc_util::resolve_project(root, project, cwd)?;
            handle_lint(args, &project_path, format)
        }
        Some(ArticleSubcommand::Index(args)) => {
            let project_path = svc_util::resolve_project(root, project, cwd)?;
            let config = config_svc::load_project(&project_path, Some(root))?;

            let templates_scanned = config
                .as_ref()
                .and_then(|c| c.templates.as_ref())
                .map(|t| t.items.iter().filter(|(_, tmpl)| matches!(tmpl.mode, TemplateMode::Generated)).count())
                .unwrap_or(0);

            // Phase 1: Docs scan + diff + reconcile
            let scanned = article_svc::scan_docs(&project_path)?;
            let index = crate::service::index::load(&project_path)?;
            let layout = config_svc::effective_layout(&project_path)?;
            let diff = article_svc::compute_article_diff(&index, &scanned, &layout.articles);

            let added_for_json: Vec<_> = diff
                .added
                .iter()
                .map(|a| serde_json::json!({"identity": a.article_path, "path": a.article_path}))
                .collect();
            let removed_for_json: Vec<_> = diff
                .removed
                .iter()
                .map(|a| serde_json::json!({"identity": a.article_path, "path": a.article_path}))
                .collect();
            let _added_count = diff.added.len();
            let removed_count = diff.removed.len();
            let kept_count = index.articles.as_ref().map(|a| a.len()).unwrap_or(0).saturating_sub(removed_count);
            let scanned_count = scanned.len();

            if args.dry_run.dry_run {
                let details = serde_json::json!({
                    "added": diff.added,
                    "removed": diff.removed,
                    "kept_count": kept_count,
                    "scanned_count": scanned_count,
                    "templates_scanned": templates_scanned,
                });
                let result = VerbResult {
                    verb: Verb::Index,
                    kind: "article",
                    identity: String::new(),
                    old_identity: None,
                    path: None,
                    dry_run: true,
                    details,
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

            let mut updated = article_svc::reconcile_articles(&project_path, index, diff)?;

            // Phase 2: Merge declared articles
            if let Some(ref config) = config {
                let declared = article_svc::scan_declared(&project_path, config)?;
                for da in declared {
                    let articles = updated.articles.get_or_insert_with(Vec::new);
                    if !articles.iter().any(|a| a.article_path == da.article_path) {
                        articles.push(da);
                    }
                }

                let declared_prefixes: Vec<String> = config
                    .build
                    .articles
                    .values()
                    .filter_map(|cfg| cfg.article_dir.as_ref().map(|d| d.trim_end_matches('/').to_string() + "/"))
                    .collect();

                let template_articles = article_svc::scan_templates(&project_path, config)?;
                for ta in template_articles {
                    if updated.articles.as_ref().is_some_and(|a| a.iter().any(|e| e.article_path == ta.article_path)) {
                        continue;
                    }
                    if declared_prefixes.iter().any(|p| ta.article_path.starts_with(p)) {
                        continue;
                    }
                    let articles = updated.articles.get_or_insert_with(Vec::new);
                    let pos = articles.iter().position(|a| a.article_path == ta.article_path);
                    if let Some(pos) = pos {
                        articles[pos].template_origin = ta.template_origin;
                    } else {
                        articles.push(ta);
                    }
                }
                if let Some(ref mut articles) = updated.articles {
                    articles.sort_by(|a, b| a.article_path.cmp(&b.article_path));
                }
            }

            crate::service::index::save(&project_path, &updated)?;
            let final_count = updated.articles.as_ref().map(|a| a.len()).unwrap_or(0);

            let details = serde_json::json!({
                "added": added_for_json,
                "removed": removed_for_json,
                "kept_count": final_count,
                "scanned_count": scanned_count,
                "templates_scanned": templates_scanned,
            });
            let result = VerbResult {
                verb: Verb::Index,
                kind: "article",
                identity: String::new(),
                old_identity: None,
                path: None,
                dry_run: false,
                details,
            };
            match format {
                Format::Json => Ok(CommandOutcome::Success(verb_json(&result), Vec::new(), None)),
                Format::Text => Ok(CommandOutcome::Success(
                    serde_json::Value::String(verb_text(
                        &result,
                        &VerbOpts::from_repo_root(Some(project_path.as_path())),
                    )),
                    Vec::new(),
                    None,
                )),
            }
        }
        Some(ArticleSubcommand::Show(args)) => {
            let project_path = svc_util::resolve_project(root, project, cwd)?;
            handle_article_show(args, &project_path, format)
        }
        Some(ArticleSubcommand::Update(args)) => {
            let project_path = svc_util::resolve_project(root, project, cwd)?;
            let status = match args.status.as_deref() {
                Some("draft") => Some(ArticleStatus::Draft),
                Some("published") => Some(ArticleStatus::Published),
                Some(other) => {
                    return Err(MfError::usage(
                        format!("invalid status '{other}'"),
                        Some("use --status draft or --status published".to_string()),
                    ));
                }
                None => None,
            };

            let report = article_svc::update_article(
                &project_path,
                article_svc::ArticleUpdate {
                    selector: &args.path,
                    status,
                    title: args.title.as_deref(),
                    dry_run: args.dry_run.dry_run,
                },
            )?;

            let result = VerbResult {
                verb: Verb::Update,
                kind: "article",
                identity: report.article.article_path.clone(),
                old_identity: None,
                path: Some(report.article.article_path.clone()),
                dry_run: report.dry_run,
                details: serde_json::json!({"changes": report.changes, "article": report.article}),
            };
            match format {
                Format::Json => Ok(CommandOutcome::Success(verb_json(&result), Vec::new(), None)),
                Format::Text => Ok(CommandOutcome::Success(
                    serde_json::Value::String(verb_text(
                        &result,
                        &VerbOpts::from_repo_root(Some(project_path.as_path())),
                    )),
                    Vec::new(),
                    None,
                )),
            }
        }
        Some(ArticleSubcommand::Rename(args)) => {
            let project_path = svc_util::resolve_project(root, project, cwd)?;
            identity::validate_entity_path(&project_path, &args.old_path)?;
            identity::validate_entity_path(&project_path, &args.new_path)?;

            if args.dry_run.dry_run {
                let result = VerbResult {
                    verb: Verb::Rename,
                    kind: "article",
                    identity: args.new_path.clone(),
                    old_identity: Some(args.old_path.clone()),
                    path: None,
                    dry_run: true,
                    details: serde_json::json!({}),
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

            let report = article_svc::rename_article(&project_path, &args.old_path, &args.new_path, args.force.force)?;

            let result = VerbResult {
                verb: Verb::Rename,
                kind: "article",
                identity: report.new_article_path.clone(),
                old_identity: Some(report.old_article_path.clone()),
                path: Some(report.new_article_path.clone()),
                dry_run: false,
                details: serde_json::json!({"old_title": report.old_title, "new_title": report.new_title}),
            };
            match format {
                Format::Json => Ok(CommandOutcome::Success(verb_json(&result), Vec::new(), None)),
                Format::Text => Ok(CommandOutcome::Success(
                    serde_json::Value::String(verb_text(
                        &result,
                        &VerbOpts::from_repo_root(Some(project_path.as_path())),
                    )),
                    Vec::new(),
                    None,
                )),
            }
        }
        Some(ArticleSubcommand::Remove(args)) => {
            let project_path = svc_util::resolve_project(root, project, cwd)?;
            identity::validate_entity_path(&project_path, &args.path)?;

            require_confirmation(&ConfirmArgs {
                verb_label: "removal",
                kind: "article",
                identity: &args.path,
                yes: args.yes.yes,
                force: args.force.force,
            })?;

            let report =
                article_svc::remove_article(&project_path, &args.path, args.force.force, args.dry_run.dry_run)?;

            let result = VerbResult {
                verb: Verb::Remove,
                kind: "article",
                identity: args.path.clone(),
                old_identity: None,
                path: Some(report.before.article_path.clone()),
                dry_run: args.dry_run.dry_run,
                details: serde_json::json!({"removed": true}),
            };
            match format {
                Format::Json => Ok(CommandOutcome::Success(verb_json(&result), Vec::new(), None)),
                Format::Text => Ok(CommandOutcome::Success(
                    serde_json::Value::String(verb_text(
                        &result,
                        &VerbOpts::from_repo_root(Some(project_path.as_path())),
                    )),
                    Vec::new(),
                    None,
                )),
            }
        }
        Some(ArticleSubcommand::Block(block_cmd)) => match block_cmd {
            ArticleBlockSubcommand::Rename(args) => handle_block_rename(args, ctx),
        },
        Some(ArticleSubcommand::Convert(args)) => handle_convert(args, ctx),
    }
}

fn article_content_kind(project_path: &Path, article_path: &str) -> &'static str {
    let path = project_path.join(article_path);
    if path.is_dir() {
        "blocked"
    } else if path.is_file() {
        "single_file"
    } else {
        "missing"
    }
}

fn status_cell(status: &str) -> ListCell {
    match status {
        "published" => ListCell::Styled { text: "published".to_string(), ansi_prefix: "\x1b[32m", ansi_suffix: "" },
        _ => ListCell::Styled { text: "draft".to_string(), ansi_prefix: "\x1b[2m", ansi_suffix: "" },
    }
}

fn format_mtime(mtime_secs: u64) -> String {
    if mtime_secs == 0 {
        return "-".to_string();
    }
    chrono::DateTime::from_timestamp(mtime_secs as i64, 0)
        .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
        .unwrap_or_else(|| "-".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_mtime_zero_returns_dash() {
        assert_eq!(format_mtime(0), "-");
    }

    #[test]
    fn format_mtime_known_timestamp() {
        // 2024-01-15 08:30 UTC
        let ts: u64 = 1705307400;
        assert_eq!(format_mtime(ts), "2024-01-15 08:30");
    }

    #[test]
    fn format_mtime_epoch_boundary() {
        // Unix epoch: 1970-01-01 00:00 UTC
        assert_eq!(format_mtime(0), "-"); // 0 is special-cased
        assert_eq!(format_mtime(1), "1970-01-01 00:00");
    }
}
