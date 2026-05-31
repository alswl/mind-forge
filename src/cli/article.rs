use std::path::{Path, PathBuf};

use clap::{Args, Subcommand};
use serde::Serialize;

use crate::cli::deprecation::DeprecationContext;
use crate::cli::shared_flags::LintFlags;
use crate::cli::shared_flags::NoHeadersFlag;
use crate::cli::shared_flags::NoTruncFlag;
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
    #[arg(short = 'f', long)]
    pub force: bool,
    #[arg(long = "dry-run")]
    pub dry_run: bool,
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
    #[arg(long, short = 'n')]
    pub dry_run: bool,
}

#[derive(Debug, Clone, Args)]
pub struct ArticleShowArgs {
    /// Article path (e.g. docs/weekly.md) or title
    pub path: String,
}

#[derive(Debug, Clone, Args)]
pub struct ArticleRemoveArgs {
    /// Article path (e.g. docs/weekly.md) or title
    pub path: String,
    #[arg(short = 'f', long)]
    pub force: bool,
    #[arg(short = 'y', long)]
    pub yes: bool,
    #[arg(long = "dry-run")]
    pub dry_run: bool,
}

#[derive(Debug, Clone, Args)]
pub struct ArticleRenameArgs {
    /// Current article path or title
    pub old_path: String,
    /// New article path or title
    pub new_path: String,
    #[arg(short = 'f', long)]
    pub force: bool,
    #[arg(long = "dry-run")]
    pub dry_run: bool,
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
    #[arg(long = "dry-run")]
    pub dry_run: bool,
}

pub fn dispatch(
    command: ArticleCmd,
    repo_root: Option<&PathBuf>,
    format: Format,
    project: Option<&str>,
    _deprecation: &mut DeprecationContext,
) -> Result<CommandOutcome> {
    let root = repo_root.ok_or_else(MfError::not_in_mind_repo)?;

    let cwd = std::env::current_dir().map_err(MfError::Io)?;

    match command.command {
        None => Ok(CommandOutcome::GroupHelp("article")),
        Some(ArticleSubcommand::New(args)) => {
            let project_path = svc_util::resolve_project(root, project, &cwd)?;

            if args.dry_run {
                let filename = svc_util::to_filename(&args.title);
                let layout = config_svc::effective_layout(&project_path)?;
                let identity = if args.file {
                    format!("{}/{}.{}", layout.articles, filename, defaults::MARKDOWN_EXTENSION)
                } else {
                    format!("{}/{}", layout.articles, filename)
                };
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
                    Format::Json => Ok(CommandOutcome::Success(verb_json(&result), None)),
                    Format::Text => Ok(CommandOutcome::Success(
                        serde_json::Value::String(verb_text(&result, &VerbOpts::from_repo_root(Some(root)))),
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
                args.force,
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
                Format::Json => Ok(CommandOutcome::Success(verb_json(&result), None)),
                Format::Text => Ok(CommandOutcome::Success(
                    serde_json::Value::String(verb_text(&result, &VerbOpts::from_repo_root(Some(root)))),
                    None,
                )),
            }
        }
        Some(ArticleSubcommand::List(args)) => {
            let project_path = svc_util::resolve_project(root, project, &cwd)?;
            let articles = article_svc::list_articles(&project_path)?;
            let config = config_svc::load_project(&project_path, Some(root))?;

            // Compute article_dir for each article based on config
            let enriched: Vec<serde_json::Value> = articles
                .iter()
                .map(|a| {
                    let article_dir =
                        config.as_ref().map(|cfg| article_svc::effective_article_dir(&project_path, cfg, a));
                    let mut v = serde_json::to_value(a).unwrap_or_default();

                    // Add article_dir
                    if let Some(dir) = article_dir {
                        v["article_dir"] = serde_json::Value::String(dir);
                    }

                    // Article key (id)
                    if let Ok(key) = crate::service::index::article_key(a) {
                        v["id"] = serde_json::Value::String(key);
                    }

                    // Discovery origin
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

                    // Article content existence
                    v["article_present"] = serde_json::Value::Bool(project_path.join(&a.article_path).exists());
                    let content_kind = article_content_kind(&project_path, &a.article_path);
                    v["content_kind"] = serde_json::Value::String(content_kind.to_string());

                    // Path identity contract
                    v["identity"] = serde_json::Value::String(a.article_path.clone());
                    v["path"] = serde_json::Value::String(a.article_path.clone());

                    v
                })
                .collect();

            let opts = ListOpts::from_flags(args.no_headers.no_headers, args.no_trunc.no_trunc)
                .with_repo_root(Some(root.clone()));

            match format {
                Format::Json => Ok(CommandOutcome::Success(json_collection("articles", enriched), None)),
                Format::Text => {
                    let mut rows = Vec::with_capacity(enriched.len());
                    for v in &enriched {
                        let identity = v["identity"].as_str().unwrap_or("").to_string();
                        let content_kind = v["content_kind"].as_str().unwrap_or("missing");
                        let status = v["status"].as_str().unwrap_or("draft");
                        rows.push(ListRow {
                            cells: vec![ListCell::Path(identity), content_kind_cell(content_kind), status_cell(status)],
                        });
                    }
                    let view = ListView { headers: &["PATH", "CONTENT", "STATUS"], rows, plural_noun: "articles" };
                    Ok(CommandOutcome::Raw(render_text(&view, &opts), None))
                }
            }
        }
        Some(ArticleSubcommand::Lint(args)) => {
            let project_path = svc_util::resolve_project(root, project, &cwd)?;
            handle_lint(args, &project_path, root, format)
        }
        Some(ArticleSubcommand::Index(args)) => {
            let project_path = svc_util::resolve_project(root, project, &cwd)?;
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

            if args.dry_run {
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
                    Format::Json => Ok(CommandOutcome::Success(verb_json(&result), None)),
                    Format::Text => Ok(CommandOutcome::Success(
                        serde_json::Value::String(verb_text(&result, &VerbOpts::from_repo_root(Some(root)))),
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
                Format::Json => Ok(CommandOutcome::Success(verb_json(&result), None)),
                Format::Text => Ok(CommandOutcome::Success(
                    serde_json::Value::String(verb_text(&result, &VerbOpts::from_repo_root(Some(root)))),
                    None,
                )),
            }
        }
        Some(ArticleSubcommand::Show(args)) => {
            let project_path = svc_util::resolve_project(root, project, &cwd)?;
            handle_article_show(args, &project_path, root, format)
        }
        Some(ArticleSubcommand::Rename(args)) => {
            let project_path = svc_util::resolve_project(root, project, &cwd)?;
            identity::validate_entity_path(&project_path, &args.old_path)?;
            identity::validate_entity_path(&project_path, &args.new_path)?;

            if args.dry_run {
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
                    Format::Json => Ok(CommandOutcome::Success(verb_json(&result), None)),
                    Format::Text => Ok(CommandOutcome::Success(
                        serde_json::Value::String(verb_text(&result, &VerbOpts::from_repo_root(Some(root)))),
                        None,
                    )),
                };
            }

            let report = article_svc::rename_article(&project_path, &args.old_path, &args.new_path, args.force)?;

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
                Format::Json => Ok(CommandOutcome::Success(verb_json(&result), None)),
                Format::Text => Ok(CommandOutcome::Success(
                    serde_json::Value::String(verb_text(&result, &VerbOpts::from_repo_root(Some(root)))),
                    None,
                )),
            }
        }
        Some(ArticleSubcommand::Remove(args)) => {
            let project_path = svc_util::resolve_project(root, project, &cwd)?;
            identity::validate_entity_path(&project_path, &args.path)?;

            require_confirmation(&ConfirmArgs {
                verb_label: "removal",
                kind: "article",
                identity: &args.path,
                yes: args.yes,
                force: args.force,
            })?;

            let report = article_svc::remove_article(&project_path, &args.path, args.force, args.dry_run)?;

            let result = VerbResult {
                verb: Verb::Remove,
                kind: "article",
                identity: args.path.clone(),
                old_identity: None,
                path: Some(report.before.article_path.clone()),
                dry_run: args.dry_run,
                details: serde_json::json!({"removed": true}),
            };
            match format {
                Format::Json => Ok(CommandOutcome::Success(verb_json(&result), None)),
                Format::Text => Ok(CommandOutcome::Success(
                    serde_json::Value::String(verb_text(&result, &VerbOpts::from_repo_root(Some(root)))),
                    None,
                )),
            }
        }
        Some(ArticleSubcommand::Convert(args)) => handle_convert(args, root, format, project, &cwd),
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

fn content_kind_cell(kind: &str) -> ListCell {
    match kind {
        "blocked" => ListCell::Styled { text: "BLOCKED".to_string(), ansi_prefix: "\x1b[33m", ansi_suffix: "" },
        "single_file" => ListCell::Styled { text: "Single File".to_string(), ansi_prefix: "\x1b[32m", ansi_suffix: "" },
        "missing" => ListCell::Styled { text: "Missing".to_string(), ansi_prefix: "\x1b[31m", ansi_suffix: "" },
        _ => ListCell::Text("Unknown".to_string()),
    }
}

fn status_cell(status: &str) -> ListCell {
    match status {
        "published" => ListCell::Styled { text: "published".to_string(), ansi_prefix: "\x1b[32m", ansi_suffix: "" },
        _ => ListCell::Styled { text: "draft".to_string(), ansi_prefix: "\x1b[2m", ansi_suffix: "" },
    }
}

fn handle_article_show(
    args: ArticleShowArgs,
    project_path: &Path,
    repo_root: &Path,
    format: Format,
) -> Result<CommandOutcome> {
    identity::validate_entity_path(project_path, &args.path)?;
    let config = config_svc::load_project(project_path, None)?;
    let articles = article_svc::list_articles(project_path)?;

    // Prefer exact article_path match (path selector), then fall back to
    // title/stem/contains for legacy title compatibility.
    let resolved = articles.iter().find(|a| a.article_path == args.path).or_else(|| {
        articles.iter().find(|a| {
            let stem = crate::service::index::article_output_stem(&a.article_path);
            a.title.eq_ignore_ascii_case(&args.path)
                || stem.eq_ignore_ascii_case(&args.path)
                || a.article_path.contains(&args.path)
        })
    });

    match resolved {
        None => Err(MfError::usage(
            format!("article '{}' not found", args.path),
            Some("use `mf article list` to see available articles".to_string()),
        )),
        Some(article) => {
            let article_dir = config.as_ref().map(|cfg| article_svc::effective_article_dir(project_path, cfg, article));
            let content_kind = article_content_kind(project_path, &article.article_path);
            let status_str = match article.status {
                ArticleStatus::Draft => "draft",
                ArticleStatus::Published => "published",
            };

            let mut fields = vec![
                ShowField { label: "Path", value: ShowValue::Path(article.article_path.clone()) },
                ShowField { label: "Title", value: ShowValue::Text(article.title.clone()) },
                ShowField { label: "Status", value: ShowValue::Text(status_str.to_string()) },
                ShowField { label: "Content", value: ShowValue::Text(content_kind.to_string()) },
            ];
            if let Some(ref dir) = article_dir {
                fields.push(ShowField { label: "Article dir", value: ShowValue::Path(dir.clone()) });
            }
            if let Some(ref origin) = article.template_origin {
                fields.push(ShowField {
                    label: "Template",
                    value: ShowValue::Text(format!("{} ({})", origin.template_name, origin.slot_value)),
                });
            }
            fields.push(ShowField { label: "Created", value: ShowValue::Text(article.created_at.clone()) });
            fields.push(ShowField { label: "Updated", value: ShowValue::Text(article.updated_at.clone()) });

            let block = ShowBlock { kind: "article", identity: article.article_path.clone(), fields, sections: vec![] };

            match format {
                Format::Json => {
                    let article_json = serde_json::to_value(article).map_err(MfError::Json)?;
                    let extra = article_json.as_object().cloned().unwrap_or_default();
                    Ok(CommandOutcome::Success(json_envelope(&block, extra), None))
                }
                Format::Text => {
                    Ok(CommandOutcome::Raw(render_show_text(&block, &ShowOpts::from_repo_root(Some(repo_root))), None))
                }
            }
        }
    }
}

// ── Handle: mf article lint ────────────────────────────────────────────────

fn handle_lint(args: ArticleLintArgs, project_path: &Path, repo_root: &Path, format: Format) -> Result<CommandOutcome> {
    let fix = args.lint.fix;
    let dry_run = args.lint.dry_run;

    let issues = article_svc::lint_articles(project_path, fix && !dry_run)?;

    // Apply --rule filter
    let filtered: Vec<_> = if args.lint.rule.is_empty() {
        issues
    } else {
        issues.into_iter().filter(|i| args.lint.rule.iter().any(|r| r == &i.kind)).collect()
    };

    // Apply --severity filter
    let severity_level = severity_rank(args.lint.severity.as_deref());
    let filtered: Vec<_> =
        filtered.into_iter().filter(|i| severity_rank(Some(&i.severity)) <= severity_level).collect();

    // Compute summary
    let errors = filtered.iter().filter(|i| i.severity == "error").count() as u64;
    let warnings = filtered.iter().filter(|i| i.severity == "warning").count() as u64;
    let info = filtered.iter().filter(|i| i.severity == "info").count() as u64;
    let fixed_count = 0u64;

    let json_issues: Vec<serde_json::Value> =
        filtered.iter().map(|i| serde_json::to_value(i).unwrap_or_default()).collect();

    let details = serde_json::json!({
        "issues": json_issues,
        "summary": { "errors": errors, "warnings": warnings, "info": info, "fixed": fixed_count },
    });

    let exit_code =
        if args.lint.max_warnings.is_some_and(|max| warnings as i32 > max) || errors > 0 { Some(1) } else { None };

    match format {
        Format::Json => {
            let data = serde_json::json!({ "kind": "article", "issues": json_issues, "summary": { "errors": errors, "warnings": warnings, "info": info, "fixed": fixed_count }, "dry_run": dry_run });
            Ok(CommandOutcome::Success(data, exit_code))
        }
        Format::Text => {
            let result = VerbResult {
                verb: Verb::Lint,
                kind: "article",
                identity: String::new(),
                old_identity: None,
                path: None,
                dry_run,
                details,
            };
            Ok(CommandOutcome::Raw(verb_text(&result, &VerbOpts::from_repo_root(Some(repo_root))), exit_code))
        }
    }
}

fn severity_rank(severity: Option<&str>) -> u8 {
    match severity {
        Some("error") => 0,
        Some("warning") => 1,
        Some("info") => 2,
        _ => 2,
    }
}

// ── Handle: mf article convert ────────────────────────────────────────────

fn handle_convert(
    args: ArticleConvertArgs,
    root: &Path,
    format: Format,
    project: Option<&str>,
    cwd: &Path,
) -> Result<CommandOutcome> {
    let project_path = svc_util::resolve_project(root, project, cwd)?;

    let index = crate::service::index::load(&project_path)?;
    let article_paths: Vec<String> = index
        .articles
        .as_ref()
        .map(|articles| articles.iter().map(|a| a.article_path.clone()).collect())
        .unwrap_or_default();

    let (direction, direction_source) = match resolve_direction(&args, &project_path, &article_paths)? {
        DirectionDecision::Use { direction, source } => (direction, source),
        DirectionDecision::Declined => {
            return Ok(CommandOutcome::Success(serde_json::Value::String("conversion declined".to_string()), None));
        }
    };

    let inspections = article_svc::plan_conversion(&project_path, &article_paths, direction)?;

    let mut converted: Vec<ConversionResult> = Vec::new();
    let mut skipped: Vec<ConversionResult> = Vec::new();
    let mut failed: Vec<ConversionResult> = Vec::new();

    for inspection in &inspections {
        if !inspection.eligible {
            skipped.push(inspection.to_result(
                ConversionStatus::Skipped,
                direction,
                inspection.skip_reason.clone(),
                false,
                false,
            ));
            continue;
        }

        if args.dry_run {
            converted.push(inspection.to_result(ConversionStatus::WouldConvert, direction, None, false, false));
            continue;
        }

        let exec = match direction {
            ConversionDirection::ToSingleFile => article_svc::execute_to_single_file(&project_path, inspection),
            ConversionDirection::ToDirectory => article_svc::execute_to_directory(&project_path, inspection),
        };
        match exec {
            Ok(mut conv) => {
                match article_svc::update_index_for_conversion(&project_path, &conv.source_path, &conv.target_path) {
                    Ok(()) => {
                        conv.index_updated = true;
                        converted.push(conv);
                    }
                    Err(e) => {
                        conv.status = ConversionStatus::Failed;
                        conv.reason = Some(format!("index update failed: {}", e));
                        failed.push(conv);
                    }
                }
            }
            Err(e) => {
                failed.push(inspection.to_result(
                    ConversionStatus::Failed,
                    direction,
                    Some(format!("{}", e)),
                    false,
                    false,
                ));
            }
        }
    }

    let summary = ConversionSummary {
        kind: "article".to_string(),
        direction,
        direction_source,
        dry_run: args.dry_run,
        converted_count: converted.len(),
        skipped_count: skipped.len(),
        failed_count: failed.len(),
        scanned_count: inspections.len(),
        converted,
        skipped,
        failed,
    };

    match format {
        Format::Json => {
            let data = serde_json::to_value(&summary).map_err(MfError::Json)?;
            Ok(CommandOutcome::Success(data, None))
        }
        Format::Text => Ok(CommandOutcome::Success(serde_json::Value::String(render_convert_text(&summary)), None)),
    }
}

enum DirectionDecision {
    Use { direction: ConversionDirection, source: DirectionSource },
    Declined,
}

fn resolve_direction(
    args: &ArticleConvertArgs,
    project_path: &Path,
    article_paths: &[String],
) -> Result<DirectionDecision> {
    if args.to_single_file {
        return Ok(DirectionDecision::Use {
            direction: ConversionDirection::ToSingleFile,
            source: DirectionSource::Explicit,
        });
    }
    if args.to_directory {
        return Ok(DirectionDecision::Use {
            direction: ConversionDirection::ToDirectory,
            source: DirectionSource::Explicit,
        });
    }

    let plausible = article_svc::plausible_directions(project_path, article_paths)?;
    match plausible.as_slice() {
        [] => Err(MfError::usage(
            "no eligible articles found for conversion",
            Some("verify that the project has articles that can be converted".to_string()),
        )),
        [(direction, count)] => confirm_inferred_direction(*direction, *count),
        _ => Err(MfError::usage(
            "ambiguous conversion direction; both --to-single-file and --to-directory are possible",
            Some("pass --to-single-file or --to-directory to specify the desired direction".to_string()),
        )),
    }
}

fn confirm_inferred_direction(direction: ConversionDirection, count: usize) -> Result<DirectionDecision> {
    let prompt = format!(
        "No conversion direction specified.\nSuggested direction: {} ({} article{} can be converted)\nProceed? [y/N]: ",
        direction,
        count,
        if count == 1 { "" } else { "s" },
    );
    match prompt_confirmation(&prompt) {
        ConfirmOutcome::Confirmed => Ok(DirectionDecision::Use { direction, source: DirectionSource::Inferred }),
        ConfirmOutcome::Aborted => Ok(DirectionDecision::Declined),
        ConfirmOutcome::NotTty => Err(MfError::usage(
            format!("no conversion direction specified; eligible direction is {}", direction),
            Some(format!("pass {} or run in a terminal for interactive confirmation", direction)),
        )),
    }
}

fn render_convert_text(summary: &ConversionSummary) -> String {
    let prefix = if summary.dry_run { "[dry-run] " } else { "" };
    let convert_verb = if summary.dry_run { "would convert" } else { "converted" };
    let mut lines: Vec<String> = Vec::new();

    for r in &summary.converted {
        lines.push(format!("{prefix}{convert_verb} article: {} -> {}", r.source_path, r.target_path));
    }
    for r in &summary.skipped {
        lines.push(format!("skipped article: {} ({})", r.source_path, r.reason.as_deref().unwrap_or("unknown")));
    }
    for r in &summary.failed {
        lines.push(format!("failed article: {} ({})", r.source_path, r.reason.as_deref().unwrap_or("unknown error")));
    }

    lines.push(format!(
        "{prefix}article convert {}: {} {}, {} skipped, {} failed",
        summary.direction, summary.converted_count, convert_verb, summary.skipped_count, summary.failed_count
    ));

    lines.join("\n")
}
