//! Publish service (US1–US4 of feature 009).

use std::fs;
use std::path::{Path, PathBuf};

use chrono::{SecondsFormat, Utc};

use crate::cli::publish::{PublishRunArgs, PublishUpdateArgs};
use crate::error::{MfError, Result};
use crate::model::config::{MindConfig, PublishTarget, PublishTargetType};
use crate::model::index::{PublishRecord, PublishStatus};
use crate::model::publish::{
    LocalRunOutcome, PublishRunOutcome, PublishUpdateOutcome, UpdateAction, YuquePromptRunOutcome,
};
use crate::service::index;
use crate::service::publisher as publisher_svc;
use crate::service::{config as config_svc, util};

pub fn run(args: &PublishRunArgs, repo_root: &Path, cwd: &Path) -> Result<PublishRunOutcome> {
    if args.article.is_empty() || args.article.contains('/') || args.article.contains('\\') {
        return Err(MfError::usage(
            format!("invalid article name: '{}'", args.article),
            Some("article must be kebab-case with no path separators".to_string()),
        ));
    }

    let project_path = util::resolve_project(repo_root, args.project.as_deref(), cwd)?;
    let config = config_svc::load_project(&project_path, Some(repo_root))?.ok_or_else(|| {
        MfError::usage("project missing mind.yaml".to_string(), Some("run `mf config init` to create one".to_string()))
    })?;

    let index = index::load(&project_path)?;
    let article_entry = index
        .articles
        .iter()
        .flat_map(|a| a.iter())
        .find(|a| a.source_path == format!("docs/{}.md", args.article))
        .ok_or_else(|| {
            MfError::not_found(
                format!("article '{}' not found in mind-index.yaml", args.article),
                Some("run `mf article index` to refresh the index".to_string()),
            )
        })?;

    let target = resolve_target(args, &config, repo_root)?;

    match target.target_type {
        PublishTargetType::Local => {
            let outcome = run_local(args, &target, repo_root, &project_path, &config, &article_entry.source_path)?;
            Ok(PublishRunOutcome::Local(outcome))
        }
        PublishTargetType::YuquePrompt => {
            let outcome = run_yuque_prompt(args, &target, &project_path, &config, &article_entry.source_path)?;
            Ok(PublishRunOutcome::YuquePrompt(outcome))
        }
        PublishTargetType::Yuque | PublishTargetType::GithubPages | PublishTargetType::Custom => {
            let type_name = target_type_kebab(&target.target_type);
            drop(target);
            Err(MfError::not_implemented_with_hint(
                format!("publish target type '{type_name}'"),
                "tracked in upcoming ROADMAP-004; use type 'local' or 'yuque-prompt' instead",
            ))
        }
    }
}

pub fn update(args: &PublishUpdateArgs, repo_root: &Path, cwd: &Path) -> Result<PublishUpdateOutcome> {
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

    let project_path = util::resolve_project(repo_root, args.project.as_deref(), cwd)?;
    let config = config_svc::load_project(&project_path, Some(repo_root))?.ok_or_else(|| {
        MfError::usage("project missing mind.yaml".to_string(), Some("run `mf config init` to create one".to_string()))
    })?;

    let mut index = index::load(&project_path)?;

    let article_source_path = index
        .articles
        .iter()
        .flat_map(|a| a.iter())
        .find(|a| a.source_path == format!("docs/{}.md", args.article))
        .map(|a| a.source_path.clone())
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
        .and_then(|recs| recs.iter().find(|r| r.path == article_source_path && r.target_name == args.target));

    let (record, action) =
        upsert_decision(existing, &article_source_path, &args.target, status_arg, args.target_url.as_deref())?;

    if !args.dry_run {
        let recs = index.publish_records.get_or_insert_with(Vec::new);
        if let Some(pos) = recs.iter().position(|r| r.path == article_source_path && r.target_name == args.target) {
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

fn resolve_target(args: &PublishRunArgs, config: &MindConfig, repo_root: &Path) -> Result<PublishTarget> {
    let name = match args.target.as_deref() {
        Some(n) => n,
        None => config.publish.default_target.as_deref().ok_or_else(|| {
            MfError::usage(
                "no --target specified and no publish.default_target configured",
                Some("set publish.default_target in mind.yaml or pass --target <NAME>".to_string()),
            )
        })?,
    };

    if args.target.is_some() {
        // When --target is explicitly specified, try file-based publisher first.
        // Only fall back to mind.yaml on NotFound (unknown target), not on
        // configuration errors (invalid/disabled/duplicate/secret-field).
        match publisher_svc::resolve_target(repo_root, name, config) {
            Ok(resolved) => return Ok(resolved.target),
            Err(e) => {
                if !matches!(&e, MfError::NotFound { .. }) {
                    return Err(e);
                }
                // NotFound — fall through to mind.yaml check
            }
        }
    }

    // Fall back to mind.yaml targets
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

    Ok(target.clone())
}

fn resolve_local_path(repo_root: &Path, target: &PublishTarget) -> Result<PathBuf> {
    let config = target.config.as_ref().ok_or_else(|| {
        MfError::usage(
            format!("local target '{}' missing config.path", target.name),
            Some("set `config.path: <directory>` on the target".to_string()),
        )
    })?;
    let path_str = config.get("path").and_then(|v| v.as_str()).ok_or_else(|| {
        MfError::usage(
            format!("local target '{}' missing config.path", target.name),
            Some("set `config.path: <directory>` on the target".to_string()),
        )
    })?;
    let path = PathBuf::from(path_str);
    if path.is_absolute() {
        Ok(path)
    } else {
        Ok(repo_root.join(path))
    }
}

fn locate_build_artifact(project_path: &Path, config: &MindConfig, article: &str) -> Result<(PathBuf, u64)> {
    let format = if config.build.format.is_empty() { "md" } else { config.build.format.as_str() };
    let path = project_path.join(&config.build.output_dir).join(format!("{article}.{format}"));
    let metadata = fs::metadata(&path).map_err(|_| {
        MfError::not_found(
            format!("build artifact not found: {}", path.display()),
            Some(format!("run `mf build {article}` first")),
        )
    })?;
    Ok((path, metadata.len()))
}

fn run_local(
    args: &PublishRunArgs,
    target: &PublishTarget,
    repo_root: &Path,
    project_path: &Path,
    config: &MindConfig,
    _source_path: &str,
) -> Result<LocalRunOutcome> {
    let (artifact_path, size_bytes) = locate_build_artifact(project_path, config, &args.article)?;

    let dest_dir = resolve_local_path(repo_root, target)?;
    let format = if config.build.format.is_empty() { "md" } else { config.build.format.as_str() };
    let dest_file = dest_dir.join(format!("{}.{format}", args.article));

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
    })
}

fn run_yuque_prompt(
    args: &PublishRunArgs,
    target: &PublishTarget,
    project_path: &Path,
    config: &MindConfig,
    source_path: &str,
) -> Result<YuquePromptRunOutcome> {
    let (artifact_path, _size_bytes) = locate_build_artifact(project_path, config, &args.article)?;

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
        source = source_path,
        suggested = suggested_update_command,
    );

    Ok(YuquePromptRunOutcome {
        target_name: target.name.clone(),
        article: args.article.clone(),
        source_path: source_path.to_string(),
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
    }
}
