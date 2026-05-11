use std::path::Path;

use crate::error::{MfError, Result};
use crate::model::index::IndexFile;
use crate::model::project::{LintKind, LintSeverity, ProjectLintIssue};
use crate::service::util;

const REQUIRED_DIRS: &[&str] = &["docs", "docs/images", "sources", "assets"];

/// Lint a single project, returning issues and summary.
pub fn lint_project(
    project_path: &Path,
    rules: &[LintKind],
    fix: bool,
) -> Result<(Vec<serde_json::Value>, serde_json::Value)> {
    let active_rules: Vec<LintKind> = if rules.is_empty() {
        vec![LintKind::MissingDirectory, LintKind::StaleIndexEntry, LintKind::NameConvention, LintKind::MissingManifest]
    } else {
        rules.to_vec()
    };

    let mut issues: Vec<ProjectLintIssue> = Vec::new();
    let mut fixable_count = 0u64;
    let mut unfixed_count = 0u64;

    for rule in &active_rules {
        match rule {
            LintKind::MissingDirectory => {
                check_missing_directory(project_path, fix, &mut issues, &mut fixable_count, &mut unfixed_count)?;
            }
            LintKind::StaleIndexEntry => {
                check_stale_index_entry(project_path, fix, &mut issues, &mut fixable_count, &mut unfixed_count)?;
            }
            LintKind::NameConvention => {
                check_name_convention(project_path, &mut issues);
            }
            LintKind::MissingManifest => {
                check_missing_manifest(project_path, fix, &mut issues, &mut fixable_count, &mut unfixed_count)?;
            }
        }
    }

    let errors = issues.iter().filter(|i| matches!(i.severity, LintSeverity::Error) && !i.fixed).count() as u64;
    let warnings = issues.iter().filter(|i| matches!(i.severity, LintSeverity::Warning) && !i.fixed).count() as u64;

    let summary = serde_json::json!({
        "errors": errors,
        "warnings": warnings,
        "fixed": fixable_count,
        "unfixed": unfixed_count,
    });

    let issues_json: Vec<serde_json::Value> =
        issues.iter().map(|i| serde_json::to_value(i).unwrap_or_default()).collect();
    Ok((issues_json, summary))
}

/// Lint all projects in the repo.
pub fn lint_repo(
    repo_root: &Path,
    rules: &[LintKind],
    fix: bool,
) -> Result<(Vec<serde_json::Value>, serde_json::Value)> {
    let projects_dir = crate::service::repo::projects_dir_for(repo_root)?;
    let scan_root = {
        let trimmed = projects_dir.trim_matches('/');
        if trimmed.is_empty() || trimmed == "." { repo_root.to_path_buf() } else { repo_root.join(trimmed) }
    };

    let mut project_names: Vec<String> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&scan_root) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() && path.join("mind.yaml").exists() {
                let name = entry.file_name().to_string_lossy().to_string();
                if !project_names.contains(&name) {
                    project_names.push(name);
                }
            }
        }
    }

    let mut all_issues: Vec<serde_json::Value> = Vec::new();
    let mut total_errors = 0u64;
    let mut total_warnings = 0u64;
    let mut total_fixed = 0u64;
    let mut total_unfixed = 0u64;

    for name in &project_names {
        let project_path = util::project_dir_for(repo_root, &projects_dir, name);
        let (issues, summary) = lint_project(&project_path, rules, fix)?;
        for issue in &issues {
            let mut with_project = issue.clone();
            if let Some(obj) = with_project.as_object_mut() {
                obj.insert("project".to_string(), serde_json::Value::String(name.clone()));
            }
            all_issues.push(with_project);
        }
        total_errors += summary["errors"].as_u64().unwrap_or(0);
        total_warnings += summary["warnings"].as_u64().unwrap_or(0);
        total_fixed += summary["fixed"].as_u64().unwrap_or(0);
        total_unfixed += summary["unfixed"].as_u64().unwrap_or(0);
    }

    let summary = serde_json::json!({
        "errors": total_errors,
        "warnings": total_warnings,
        "fixed": total_fixed,
        "unfixed": total_unfixed,
    });

    Ok((all_issues, summary))
}

fn check_missing_directory(
    project_path: &Path,
    fix: bool,
    issues: &mut Vec<ProjectLintIssue>,
    fixable_count: &mut u64,
    unfixed_count: &mut u64,
) -> Result<()> {
    for dir in REQUIRED_DIRS {
        let dir_path = project_path.join(dir);
        if !dir_path.exists() {
            let fixed = fix && std::fs::create_dir_all(&dir_path).is_ok();
            issues.push(ProjectLintIssue {
                severity: LintSeverity::Error,
                kind: LintKind::MissingDirectory,
                message: format!("missing directory '{dir}'"),
                path: format!("{dir}/"),
                fixable: true,
                fixed,
            });
            if fixed {
                *fixable_count += 1;
            } else {
                *unfixed_count += 1;
            }
        }
    }
    Ok(())
}

fn check_stale_index_entry(
    project_path: &Path,
    fix: bool,
    issues: &mut Vec<ProjectLintIssue>,
    fixable_count: &mut u64,
    _unfixed_count: &mut u64,
) -> Result<()> {
    let index_path = project_path.join("mind-index.yaml");
    if !index_path.exists() {
        return Ok(());
    }
    let content = match std::fs::read_to_string(&index_path) {
        Ok(c) => c,
        Err(_) => return Ok(()),
    };
    let index: IndexFile = match serde_yaml::from_str(&content) {
        Ok(idx) => idx,
        Err(_) => return Ok(()),
    };

    let mut stale_entries: Vec<String> = Vec::new();

    for article in index.articles.iter().flatten() {
        let source_path = project_path.join(&article.source_path);
        if !source_path.exists() {
            stale_entries.push(article.source_path.clone());
        }
    }
    for asset in index.assets.iter().flatten() {
        let asset_path = project_path.join(&asset.path);
        if !asset_path.exists() {
            stale_entries.push(asset.path.clone());
        }
    }
    for source in index.sources.iter().flatten() {
        if let Some(ref path) = source.path {
            let source_path = project_path.join(path);
            if !source_path.exists() {
                stale_entries.push(path.clone());
            }
        }
    }

    for entry_path in &stale_entries {
        issues.push(ProjectLintIssue {
            severity: LintSeverity::Warning,
            kind: LintKind::StaleIndexEntry,
            message: format!("index references '{entry_path}' which does not exist"),
            path: entry_path.clone(),
            fixable: true,
            fixed: false,
        });
    }

    if fix && !stale_entries.is_empty() {
        let mut articles: Vec<crate::model::article::Article> = index.articles.unwrap_or_default();
        articles.retain(|a| !stale_entries.contains(&a.source_path));

        let all_assets: Vec<crate::model::asset::Asset> = index.assets.unwrap_or_default();
        let filtered_assets = all_assets.into_iter().filter(|a| !stale_entries.contains(&a.path)).collect::<Vec<_>>();

        let all_sources: Vec<crate::model::source::Source> = index.sources.unwrap_or_default();
        let filtered_sources = all_sources
            .into_iter()
            .filter(|s| s.path.as_ref().map_or(true, |p| !stale_entries.contains(p)))
            .collect::<Vec<_>>();

        let terms = index.terms;

        let new_index = IndexFile {
            schema_version: "1".to_string(),
            sources: Some(filtered_sources),
            assets: Some(filtered_assets),
            articles: Some(articles),
            terms,
            publish_records: None,
        };

        let yaml = serde_yaml::to_string(&new_index).map_err(|e| MfError::Internal(e.into()))?;
        util::atomic_write(&index_path, &yaml)?;

        for issue in issues.iter_mut().rev().take(stale_entries.len()) {
            issue.fixed = true;
        }
        *fixable_count += stale_entries.len() as u64;
    }

    Ok(())
}

fn check_name_convention(project_path: &Path, issues: &mut Vec<ProjectLintIssue>) {
    let index_path = project_path.join("mind-index.yaml");
    if !index_path.exists() {
        return;
    }
    let content = match std::fs::read_to_string(&index_path) {
        Ok(c) => c,
        Err(_) => return,
    };
    let index: IndexFile = match serde_yaml::from_str(&content) {
        Ok(idx) => idx,
        Err(_) => return,
    };

    for article in index.articles.iter().flatten() {
        let filename =
            Path::new(&article.source_path).file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
        if util::to_filename(&filename) != filename {
            issues.push(ProjectLintIssue {
                severity: LintSeverity::Warning,
                kind: LintKind::NameConvention,
                message: format!("filename '{filename}' should be kebab-case"),
                path: article.source_path.clone(),
                fixable: false,
                fixed: false,
            });
        }
    }
}

fn check_missing_manifest(
    project_path: &Path,
    fix: bool,
    issues: &mut Vec<ProjectLintIssue>,
    fixable_count: &mut u64,
    unfixed_count: &mut u64,
) -> Result<()> {
    let mind_yaml = project_path.join("mind.yaml");
    if !mind_yaml.exists() {
        let fixed = if fix {
            let content = "schema_version: '1'\n".to_string();
            util::atomic_write(&mind_yaml, &content).is_ok()
        } else {
            false
        };
        issues.push(ProjectLintIssue {
            severity: LintSeverity::Error,
            kind: LintKind::MissingManifest,
            message: "missing mind.yaml".to_string(),
            path: "mind.yaml".to_string(),
            fixable: true,
            fixed,
        });
        if fixed {
            *fixable_count += 1;
        } else {
            *unfixed_count += 1;
        }
    }
    Ok(())
}
