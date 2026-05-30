//! Publish service (US1–US4 of feature 009).

use std::fs;
use std::path::{Path, PathBuf};

use chrono::{SecondsFormat, Utc};

use crate::cli::publish::{PublishRunArgs, PublishUpdateArgs};
use crate::defaults;
use crate::error::{MfError, Result};
use crate::model::article::Article;
use crate::model::config::{MindConfig, PublishTarget, PublishTargetType};
use crate::model::index::{PublishRecord, PublishStatus};
use crate::model::publish::{
    EffectiveDateOut, LocalRunOutcome, PublishRunOutcome, PublishUpdateOutcome, UpdateAction, YuquePromptRunOutcome,
};
use crate::service::effective_date as effective_date_svc;
use crate::service::index;
use crate::service::publisher as publisher_svc;
use crate::service::util::path_template::PathTemplate;
use crate::service::{config as config_svc, util};

pub fn run(args: &PublishRunArgs, repo_root: &Path, cwd: &Path, project: Option<&str>) -> Result<PublishRunOutcome> {
    if args.article.is_empty() || args.article.contains('\\') {
        return Err(MfError::usage(
            format!("invalid article name: '{}'", args.article),
            Some("article must be kebab-case or template-name/slot-value".to_string()),
        ));
    }

    let project_path = util::resolve_project(repo_root, project, cwd)?;
    let config = config_svc::load_project(&project_path, Some(repo_root))?.ok_or_else(|| {
        MfError::usage("project missing mind.yaml".to_string(), Some("run `mf config init` to create one".to_string()))
    })?;

    let mut index = index::load(&project_path)?;
    let article_entry = match index::resolve_article(&index, &args.article) {
        Ok(resolved) => resolved.article.clone(),
        Err(_) => {
            // Auto-reindex on cache miss
            let refreshed = crate::service::article::refresh_index(&project_path, &config)?;
            index = refreshed;
            index::resolve_article(&index, &args.article).map(|r| r.article.clone()).map_err(|_| {
                MfError::not_found(
                    format!("article '{}' not found in mind-index.yaml", args.article),
                    Some("run `mf article index` to refresh the index".to_string()),
                )
            })?
        }
    };

    let resolved = resolve_target(args, &config, repo_root, &project_path)?;

    match resolved.target.target_type {
        PublishTargetType::Local => {
            let outcome =
                run_local(args, &resolved.target, &resolved.base_dir, &project_path, &config, &article_entry)?;
            Ok(PublishRunOutcome::Local(outcome))
        }
        PublishTargetType::YuquePrompt => {
            let outcome = run_yuque_prompt(args, &resolved.target, &project_path, &config, &article_entry)?;
            Ok(PublishRunOutcome::YuquePrompt(outcome))
        }
        PublishTargetType::Yuque
        | PublishTargetType::GithubPages
        | PublishTargetType::Custom
        | PublishTargetType::YuqueCc => {
            let type_name = target_type_kebab(&resolved.target.target_type);
            Err(MfError::not_implemented_with_hint(
                format!("publish target type '{type_name}'"),
                "tracked in upcoming ROADMAP-004; use type 'local' or 'yuque-prompt' instead",
            ))
        }
    }
}

pub fn update(
    args: &PublishUpdateArgs,
    repo_root: &Path,
    cwd: &Path,
    project: Option<&str>,
) -> Result<PublishUpdateOutcome> {
    if args.article.is_empty() || args.article.contains('/') || args.article.contains('\\') {
        return Err(MfError::usage(
            format!("invalid article name: '{}'", args.article),
            Some("article must be kebab-case with no path separators".to_string()),
        ));
    }

    let status_arg: Option<PublishStatus> = args.status.map(|s| s.into());
    if status_arg.is_none() && args.target_url.is_none() {
        return Err(MfError::usage(
            "--status and --target-url cannot both be omitted",
            Some("provide at least one of --status or --target-url".to_string()),
        ));
    }

    let project_path = util::resolve_project(repo_root, project, cwd)?;
    let config = config_svc::load_project(&project_path, Some(repo_root))?.ok_or_else(|| {
        MfError::usage("project missing mind.yaml".to_string(), Some("run `mf config init` to create one".to_string()))
    })?;

    let mut index = index::load(&project_path)?;

    let layout = config_svc::effective_layout(&project_path)?;
    let indexed_article_path = index
        .articles
        .iter()
        .flat_map(|a| a.iter())
        .find(|a| a.article_path == format!("{}/{}.{}", layout.articles, args.article, defaults::MARKDOWN_EXTENSION))
        .map(|a| a.article_path.clone())
        .ok_or_else(|| {
            MfError::not_found(
                format!("article '{}' not found in mind-index.yaml", args.article),
                Some("run `mf article index` to refresh the index".to_string()),
            )
        })?;

    let targets = config.publish.targets.as_deref().unwrap_or(&[]);
    if !targets.iter().any(|t| t.name == args.target) {
        return Err(MfError::not_found(
            format!("publish target '{}' not found in mind.yaml", args.target),
            Some("check `publish.targets[].name` in mind.yaml".to_string()),
        ));
    }

    let existing = index
        .publish_records
        .as_ref()
        .and_then(|recs| recs.iter().find(|r| r.path == indexed_article_path && r.target_name == args.target));

    let (record, action) =
        upsert_decision(existing, &indexed_article_path, &args.target, status_arg, args.target_url.as_deref())?;

    if !args.dry_run {
        let recs = index.publish_records.get_or_insert_with(Vec::new);
        if let Some(pos) = recs.iter().position(|r| r.path == indexed_article_path && r.target_name == args.target) {
            recs[pos] = record.clone();
        } else {
            recs.push(record.clone());
        }
        index::save(&project_path, &index)?;
    }

    Ok(PublishUpdateOutcome {
        article: args.article.clone(),
        target_name: args.target.clone(),
        action,
        record,
        dry_run: args.dry_run,
    })
}

fn now_utc_iso8601() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)
}

fn upsert_decision(
    existing: Option<&PublishRecord>,
    path: &str,
    target_name: &str,
    status_arg: Option<PublishStatus>,
    url_arg: Option<&str>,
) -> Result<(PublishRecord, UpdateAction)> {
    if let Some(existing) = existing {
        let mut record = existing.clone();
        if let Some(s) = status_arg {
            record.status = s;
        }
        if let Some(u) = url_arg {
            record.target_url = Some(u.to_string());
        }
        return Ok((record, UpdateAction::Updated));
    }

    let status = status_arg.ok_or_else(|| {
        MfError::usage(
            "cannot create record without --status",
            Some("provide --status draft|published|archived".to_string()),
        )
    })?;

    let (published_at, target_url) = match (&status, url_arg) {
        (PublishStatus::Published, url) => (Some(now_utc_iso8601()), url.map(String::from)),
        (PublishStatus::Draft, url) => (None, url.map(String::from)),
        (PublishStatus::Archived, Some(url)) => (Some(now_utc_iso8601()), Some(url.to_string())),
        (PublishStatus::Archived, None) => {
            return Err(MfError::usage(
                "cannot create initial 'archived' record without --target-url",
                Some(
                    "provide --target-url <URL>, or first record with --status published and then --status archived"
                        .to_string(),
                ),
            ));
        }
    };

    Ok((
        PublishRecord {
            path: path.to_string(),
            target_name: target_name.to_string(),
            status,
            target_url,
            published_at,
        },
        UpdateAction::Created,
    ))
}

/// Render a `YuquePromptRunOutcome` as the two-section text layout (FR-013, research D2).
///
/// Section 1 prints the natural-language prompt body. Section 2 emits the full
/// outcome (sans the duplicate `prompt` field) inside a fenced JSON block so a
/// downstream tool can pipe the section through `jq` or feed it to an LLM.
pub fn render_prompt_text(o: &YuquePromptRunOutcome) -> String {
    let mut value = serde_json::to_value(o).unwrap_or(serde_json::Value::Null);
    if let Some(map) = value.as_object_mut() {
        map.remove("prompt");
    }
    let json = serde_json::to_string_pretty(&value).unwrap_or_else(|_| "{}".to_string());
    format!("### Publish Prompt\n\n{prompt}\n\n### Envelope\n\n```json\n{json}\n```", prompt = o.prompt,)
}

// ---------------------------------------------------------------------------
// Helpers (T023–T026)
// ---------------------------------------------------------------------------

/// Resolved target carrying the [`PublishTarget`] and the directory against
/// which relative paths should be resolved.
///
/// - Project-level targets (from `mind.yaml`) use the project root as base.
/// - Repo-level publisher files (from `.mind-forge/publisher/*.yaml`) use
///   the repo root as base.
pub struct ResolvedTarget {
    pub target: PublishTarget,
    pub base_dir: PathBuf,
}

fn resolve_target(
    args: &PublishRunArgs,
    config: &MindConfig,
    repo_root: &Path,
    project_root: &Path,
) -> Result<ResolvedTarget> {
    let name = match args.target.as_deref() {
        Some(n) => n,
        None => config.publish.default_target.as_deref().ok_or_else(|| {
            MfError::usage(
                "no --target specified and no publish.default_target configured",
                Some("set publish.default_target in mind.yaml or pass --target <NAME>".to_string()),
            )
        })?,
    };

    // Try file-based publisher first (repo-level, base = repo_root).
    if args.target.is_some() {
        let discovery = publisher_svc::discover(repo_root)?;
        if discovery.publishers.iter().any(|p| p.name == name) {
            let resolved = publisher_svc::resolve_target(repo_root, name, config)?;
            return Ok(ResolvedTarget { target: resolved.target, base_dir: repo_root.to_path_buf() });
        }
        // Publisher has config errors (invalid name, secret field, etc.) —
        // signal the block as a usage error for proper exit code (2).
        if discovery.diagnostics.iter().any(|d| d.publisher_name.as_deref() == Some(name)) {
            return Err(MfError::usage(
                format!("publisher '{name}' has configuration errors and cannot be used"),
                Some("run `mf publish target list` for details".to_string()),
            ));
        }
    }

    // mind.yaml targets (project-scoped, base = project_root).
    let targets = config.publish.targets.as_deref().unwrap_or(&[]);
    let target = targets.iter().find(|t| t.name == name).ok_or_else(|| {
        let msg = if args.target.is_some() {
            format!("publish target '{name}' not found in mind.yaml or .mind-forge/publisher/")
        } else {
            format!("publish default target '{name}' not found in mind.yaml")
        };
        MfError::not_found(msg, Some("check the publisher name or `publish.targets[].name` in mind.yaml".to_string()))
    })?;

    if !target.enabled {
        return Err(MfError::usage(
            format!("publish target '{}' is disabled", target.name),
            Some("set `enabled: true` on the target in mind.yaml".to_string()),
        ));
    }

    Ok(ResolvedTarget { target: target.clone(), base_dir: project_root.to_path_buf() })
}

fn resolve_local_path(
    base_dir: &Path,
    target: &PublishTarget,
    article: &Article,
) -> Result<(PathBuf, Option<EffectiveDateOut>)> {
    // Determine the path string: prefer target.path, fall back to target.config.path
    let path_str = match target.path.as_deref() {
        Some(p) => p.to_string(),
        None => {
            let config = target.config.as_ref().ok_or_else(|| {
                MfError::usage(
                    format!("local target '{}' missing path", target.name),
                    Some("set `path: <directory>` or `config.path: <directory>` on the target".to_string()),
                )
            })?;
            config
                .get("path")
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    MfError::usage(
                        format!("local target '{}' missing config.path", target.name),
                        Some("set `path: <directory>` or `config.path: <directory>` on the target".to_string()),
                    )
                })?
                .to_string()
        }
    };

    // Parse as PathTemplate and conditionally compute effective date
    let tmpl = PathTemplate::parse(&path_str)?;

    let (expanded, effective_out) = if tmpl.has_date_placeholders() {
        let effective = effective_date_svc::for_article(article)?;
        let origin_str = match effective.origin {
            effective_date_svc::EffectiveDateOrigin::TemplateSlot => "template_slot",
            effective_date_svc::EffectiveDateOrigin::FilenamePrefix => "filename_prefix",
        };
        let expanded = tmpl.expand(effective.date);
        let out = EffectiveDateOut { date: effective.date.to_string(), origin: origin_str.to_string() };
        (expanded, Some(out))
    } else {
        // No date placeholders — expand with a dummy date (unused)
        (tmpl.expand(chrono::Utc::now().date_naive()), None)
    };

    let path = PathBuf::from(&expanded);
    if path.is_absolute() {
        Ok((path, effective_out))
    } else {
        Ok((base_dir.join(path), effective_out))
    }
}

fn locate_build_artifact(project_path: &Path, config: &MindConfig, article_entry: &Article) -> Result<(PathBuf, u64)> {
    // FR-002 (R2): generated article — the article file IS the artifact
    if article_entry.template_origin.is_some() {
        let path = project_path.join(&article_entry.article_path);
        let metadata = fs::metadata(&path).map_err(|_| {
            MfError::build_artifact_missing(
                format!("build artifact (template source) missing at {}", path.display()),
                Some(
                    "the template-matched file was removed; regenerate it or remove the entry from mind-index.yaml"
                        .to_string(),
                ),
            )
        })?;
        return Ok((path, metadata.len()));
    }

    // Non-generated: artifact lives at <output_dir>/<stem>.<format>
    let layout = config_svc::effective_layout(project_path)?;
    let format =
        if config.build.format.is_empty() { defaults::DEFAULT_BUILD_FORMAT } else { config.build.format.as_str() };
    let stem = index::article_output_stem(&article_entry.article_path);
    let path = project_path.join(&layout.build_output).join(format!("{stem}.{format}"));
    let metadata = fs::metadata(&path).map_err(|_| {
        // Check if the article file is also missing — indicates no article files (FR-005)
        let article_path = project_path.join(&article_entry.article_path);
        if !article_path.exists() {
            return MfError::NoArticleFiles {
                article: article_entry.title.clone(),
                article_path: article_entry.article_path.clone(),
            };
        }
        MfError::build_artifact_missing(
            format!("build artifact not found: {}", path.display()),
            Some(format!("run `mf build {stem}` first")),
        )
    })?;
    Ok((path, metadata.len()))
}

fn run_local(
    args: &PublishRunArgs,
    target: &PublishTarget,
    base_dir: &Path,
    project_path: &Path,
    config: &MindConfig,
    article_entry: &Article,
) -> Result<LocalRunOutcome> {
    let (artifact_path, size_bytes) = locate_build_artifact(project_path, config, article_entry)?;

    let (dest_dir, effective_out) = resolve_local_path(base_dir, target, article_entry)?;
    let format =
        if config.build.format.is_empty() { defaults::DEFAULT_BUILD_FORMAT } else { config.build.format.as_str() };
    let prefix = target.prefix.as_deref().unwrap_or("");
    let article_stem = args.article.rsplit_once('/').map(|(_, stem)| stem).unwrap_or(&args.article);
    let article_stem = article_stem.strip_suffix(".md").unwrap_or(article_stem);
    let dest_file = dest_dir.join(format!("{prefix}{article_stem}.{format}"));

    if size_bytes == 0 {
        eprintln!("warning: build artifact is empty");
    }

    if args.dry_run {
        return Ok(LocalRunOutcome {
            target_name: target.name.clone(),
            article: args.article.clone(),
            source: artifact_path.to_string_lossy().to_string(),
            destination: dest_file.to_string_lossy().to_string(),
            size_bytes,
            dry_run: true,
            effective_date: effective_out,
        });
    }

    fs::create_dir_all(&dest_dir).map_err(MfError::Io)?;

    if dest_file.exists() && !args.force {
        return Err(MfError::file_exists(dest_file));
    }

    let bytes = fs::read(&artifact_path).map_err(MfError::Io)?;
    let content = String::from_utf8(bytes)
        .map_err(|e| MfError::Internal(anyhow::anyhow!("build artifact is not valid UTF-8: {e}")))?;
    util::atomic_write(&dest_file, &content)?;

    Ok(LocalRunOutcome {
        target_name: target.name.clone(),
        article: args.article.clone(),
        source: artifact_path.to_string_lossy().to_string(),
        destination: dest_file.to_string_lossy().to_string(),
        size_bytes,
        dry_run: false,
        effective_date: effective_out,
    })
}

fn run_yuque_prompt(
    args: &PublishRunArgs,
    target: &PublishTarget,
    project_path: &Path,
    config: &MindConfig,
    article_entry: &Article,
) -> Result<YuquePromptRunOutcome> {
    let (artifact_path, _size_bytes) = locate_build_artifact(project_path, config, article_entry)?;

    let content = fs::read_to_string(&artifact_path).map_err(MfError::Io)?;

    let envelope = target.config.clone().unwrap_or_else(|| serde_json::json!({}));

    let suggested_update_command =
        format!("mf publish update {} --target {} --status published --target-url <URL>", args.article, target.name);

    let prompt = format!(
        "Please publish the following article to yuque-prompt (target: {tgt}).\n\
Article: {article}\n\
Source: {source}\n\
After publishing, run:\n\
\n\
    {suggested}",
        tgt = target.name,
        article = args.article,
        source = article_entry.article_path,
        suggested = suggested_update_command,
    );

    Ok(YuquePromptRunOutcome {
        target_name: target.name.clone(),
        article: args.article.clone(),
        article_path: article_entry.article_path.clone(),
        build_artifact_path: artifact_path.to_string_lossy().to_string(),
        content,
        prompt,
        envelope,
        suggested_update_command,
        dry_run: args.dry_run,
    })
}

fn target_type_kebab(t: &PublishTargetType) -> &'static str {
    match t {
        PublishTargetType::Local => "local",
        PublishTargetType::YuquePrompt => "yuque-prompt",
        PublishTargetType::Yuque => "yuque",
        PublishTargetType::GithubPages => "github_pages",
        PublishTargetType::Custom => "custom",
        PublishTargetType::YuqueCc => "yuque_cc",
    }
}
