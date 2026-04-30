//! Project lifecycle service (US1–US4).

use std::path::{Path, PathBuf};

use chrono::Utc;

use crate::error::{MfError, Result};
use crate::model::index::IndexFile;
use crate::model::manifest::ProjectEntry;
use crate::model::project::{
    LintKind, LintSeverity, ProjectLintIssue, ProjectListEntry, ProjectStatusSnapshot,
};
use crate::service::repo;
use crate::service::util;

// ---------------------------------------------------------------------------
// Report types
// ---------------------------------------------------------------------------

/// Report from a successful scaffold operation.
pub struct ScaffoldReport {
    pub name: String,
    pub project_path: String,
    pub created_at: String,
    pub scaffolded: Vec<String>,
}

// ---------------------------------------------------------------------------
// US1: mf project new — scaffold & upsert
// ---------------------------------------------------------------------------

/// Create the standard project skeleton under `repo_root/<name>`.
pub fn scaffold(repo_root: &Path, name: &str, force: bool) -> Result<ScaffoldReport> {
    util::validate_project_name(name)?;

    let target = repo_root.join(name);
    let resolved = util::canonicalize_within(repo_root, &target)?;

    if resolved.exists() && !force {
        return Err(MfError::file_exists(resolved));
    }

    let now = Utc::now();
    let created_at = now.format("%Y-%m-%dT%H:%M:%SZ").to_string();
    let mut scaffolded: Vec<String> = Vec::new();

    let dirs = ["docs", "docs/images", "sources", "assets"];
    for dir in &dirs {
        let dir_path = resolved.join(dir);
        if !dir_path.exists() {
            std::fs::create_dir_all(&dir_path).map_err(MfError::Io)?;
            scaffolded.push(dir.to_string());
        }
    }

    let mind_path = resolved.join("mind.yaml");
    if !mind_path.exists() {
        let mind_yaml = "schema_version: '1'\n".to_string();
        util::atomic_write(&mind_path, &mind_yaml)?;
        scaffolded.push("mind.yaml".to_string());
    }

    let mind_index_path = resolved.join("mind-index.yaml");
    if !mind_index_path.exists() {
        let mind_index_yaml = "schema_version: '1'\n".to_string();
        util::atomic_write(&mind_index_path, &mind_index_yaml)?;
        scaffolded.push("mind-index.yaml".to_string());
    }

    Ok(ScaffoldReport {
        name: name.to_string(),
        project_path: format!("./{}", name),
        created_at,
        scaffolded,
    })
}

/// Upsert a project entry into `minds.yaml`.
pub fn upsert_project_entry(
    repo_root: &Path,
    name: &str,
    created_at: &str,
) -> Result<ProjectEntry> {
    let minds_path = repo_root.join("minds.yaml");
    let mut manifest = if minds_path.exists() {
        repo::load_manifest(&minds_path)?
    } else {
        crate::model::manifest::MindsManifest::create_default()
    };

    let entry_path = format!("./{}", name);
    let existing = manifest.projects.iter_mut().find(|p| p.name == name);
    let entry = match existing {
        Some(existing_entry) => {
            if existing_entry.path != entry_path {
                existing_entry.path = entry_path;
            }
            existing_entry.clone()
        }
        None => {
            let entry = ProjectEntry {
                name: name.to_string(),
                path: entry_path,
                created_at: created_at.to_string(),
                archived_at: None,
            };
            manifest.projects.push(entry.clone());
            entry
        }
    };

    repo::save_manifest(&manifest, &minds_path)?;
    Ok(entry)
}

// ---------------------------------------------------------------------------
// US2: mf project list
// ---------------------------------------------------------------------------

/// List all projects in the Mind Repo with document counts and last activity.
pub fn list_projects(repo_root: &Path) -> Result<Vec<ProjectListEntry>> {
    let minds_path = repo_root.join("minds.yaml");
    let manifest = if minds_path.exists() {
        repo::load_manifest(&minds_path)?
    } else {
        return Ok(Vec::new());
    };

    let mut entries: Vec<ProjectListEntry> = manifest
        .projects
        .iter()
        .map(|p| {
            let (count, last_activity) = read_index_counts(&repo_root.join(&p.name));
            ProjectListEntry {
                name: p.name.clone(),
                path: p.path.clone(),
                created_at: p.created_at.clone(),
                archived_at: p.archived_at.clone(),
                document_count: count,
                last_activity_at: last_activity,
            }
        })
        .collect();

    entries.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(entries)
}

/// Read a project's mind-index.yaml and return (document_count, last_activity_at).
fn read_index_counts(project_path: &Path) -> (u64, Option<String>) {
    let index_path = project_path.join("mind-index.yaml");
    let index = if index_path.exists() {
        match std::fs::read_to_string(&index_path) {
            Ok(content) if !content.trim().is_empty() => {
                match serde_yaml::from_str::<IndexFile>(&content) {
                    Ok(idx) => idx,
                    Err(_) => {
                        tracing::warn!("failed to parse {}", index_path.display());
                        IndexFile::create_default()
                    }
                }
            }
            _ => IndexFile::create_default(),
        }
    } else {
        tracing::warn!("missing index file: {}", index_path.display());
        IndexFile::create_default()
    };

    let articles = index.articles.as_ref().map(|v| v.len() as u64).unwrap_or(0);
    let assets = index.assets.as_ref().map(|v| v.len() as u64).unwrap_or(0);
    let sources = index.sources.as_ref().map(|v| v.len() as u64).unwrap_or(0);
    let terms = index.terms.as_ref().map(|v| v.len() as u64).unwrap_or(0);
    let count = articles + assets + sources + terms;

    // Compute max updated_at across all entries
    let mut max_ts: Option<String> = None;
    for entry in index.articles.iter().flatten() {
        if max_ts.as_ref().map_or(true, |m| &entry.updated_at > m) {
            max_ts = Some(entry.updated_at.clone());
        }
    }
    for entry in index.assets.iter().flatten() {
        if max_ts.as_ref().map_or(true, |m| &entry.added_at > m) {
            max_ts = Some(entry.added_at.clone());
        }
    }
    for entry in index.sources.iter().flatten() {
        if max_ts.as_ref().map_or(true, |m| &entry.updated_at > m) {
            max_ts = Some(entry.updated_at.clone());
        }
    }

    (count, max_ts)
}

// ---------------------------------------------------------------------------
// US3: mf project status
// ---------------------------------------------------------------------------

/// Resolve a project path within the repo root, with boundary checking.
pub fn resolve_project(repo_root: &Path, name: Option<&str>, cwd: &Path) -> Result<PathBuf> {
    match name {
        Some(name) => {
            util::validate_project_name(name)?;
            let target = repo_root.join(name);
            let resolved = util::canonicalize_within(repo_root, &target)?;
            // Verify that the project actually exists (has a mind.yaml marker)
            if !resolved.join("mind.yaml").exists() {
                return Err(MfError::usage(
                    format!("project '{name}' not found in Mind Repo"),
                    Some("use `mf project list` to see available projects".to_string()),
                ));
            }
            Ok(resolved)
        }
        None => {
            let detected = util::detect_current_project(repo_root, cwd).ok_or_else(|| {
                MfError::usage(
                    "could not detect current project; run from a project directory or specify --project",
                    Some("use `mf project list` to see available projects".to_string()),
                )
            })?;
            let target = repo_root.join(&detected);
            util::canonicalize_within(repo_root, &target)
        }
    }
}

/// Get status snapshot for a project path.
pub fn status_for(project_path: &Path) -> Result<ProjectStatusSnapshot> {
    let name =
        project_path.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();

    let index_path = project_path.join("mind-index.yaml");
    let index = if index_path.exists() {
        match std::fs::read_to_string(&index_path) {
            Ok(content) if !content.trim().is_empty() => {
                serde_yaml::from_str::<IndexFile>(&content).unwrap_or_else(|_| {
                    tracing::warn!("failed to parse {}", index_path.display());
                    IndexFile::create_default()
                })
            }
            _ => IndexFile::create_default(),
        }
    } else {
        IndexFile::create_default()
    };

    let articles = index.articles.as_ref().map(|v| v.len() as u64).unwrap_or(0);
    let assets = index.assets.as_ref().map(|v| v.len() as u64).unwrap_or(0);
    let sources = index.sources.as_ref().map(|v| v.len() as u64).unwrap_or(0);
    let terms = index.terms.as_ref().map(|v| v.len() as u64).unwrap_or(0);

    // Compute max updated_at
    let mut max_ts: Option<String> = None;
    for entry in index.articles.iter().flatten() {
        if max_ts.as_ref().map_or(true, |m| &entry.updated_at > m) {
            max_ts = Some(entry.updated_at.clone());
        }
    }
    for entry in index.assets.iter().flatten() {
        if max_ts.as_ref().map_or(true, |m| &entry.added_at > m) {
            max_ts = Some(entry.added_at.clone());
        }
    }
    for entry in index.sources.iter().flatten() {
        if max_ts.as_ref().map_or(true, |m| &entry.updated_at > m) {
            max_ts = Some(entry.updated_at.clone());
        }
    }

    Ok(ProjectStatusSnapshot {
        name: name.clone(),
        path: format!("./{}", name.clone()),
        articles,
        assets,
        sources,
        terms,
        updated_at: max_ts,
    })
}

// ---------------------------------------------------------------------------
// US4: mf project lint
// ---------------------------------------------------------------------------

/// Lint a single project, returning issues and summary.
pub fn lint_project(
    project_path: &Path,
    rules: &[LintKind],
    fix: bool,
) -> Result<(Vec<serde_json::Value>, serde_json::Value)> {
    let active_rules: Vec<LintKind> = if rules.is_empty() {
        vec![
            LintKind::MissingDirectory,
            LintKind::StaleIndexEntry,
            LintKind::NameConvention,
            LintKind::MissingManifest,
        ]
    } else {
        rules.to_vec()
    };

    let mut issues: Vec<ProjectLintIssue> = Vec::new();
    let mut fixable_count = 0u64;
    let mut unfixed_count = 0u64;

    for rule in &active_rules {
        match rule {
            LintKind::MissingDirectory => {
                check_missing_directory(
                    project_path,
                    fix,
                    &mut issues,
                    &mut fixable_count,
                    &mut unfixed_count,
                )?;
            }
            LintKind::StaleIndexEntry => {
                check_stale_index_entry(
                    project_path,
                    fix,
                    &mut issues,
                    &mut fixable_count,
                    &mut unfixed_count,
                )?;
            }
            LintKind::NameConvention => {
                check_name_convention(project_path, &mut issues);
            }
            LintKind::MissingManifest => {
                check_missing_manifest(
                    project_path,
                    fix,
                    &mut issues,
                    &mut fixable_count,
                    &mut unfixed_count,
                )?;
            }
        }
    }

    let errors =
        issues.iter().filter(|i| matches!(i.severity, LintSeverity::Error) && !i.fixed).count()
            as u64;
    let warnings =
        issues.iter().filter(|i| matches!(i.severity, LintSeverity::Warning) && !i.fixed).count()
            as u64;

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
    let _minds_path = repo_root.join("minds.yaml");
    let manifest = crate::model::manifest::MindsManifest::create_default();

    // Collect project paths from manifest + on-disk dirs
    let mut project_names: Vec<String> = manifest.projects.iter().map(|p| p.name.clone()).collect();
    if let Ok(entries) = std::fs::read_dir(repo_root) {
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
        let project_path = repo_root.join(name);
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

// ---------------------------------------------------------------------------
// Lint rule checks
// ---------------------------------------------------------------------------

const REQUIRED_DIRS: &[&str] = &["docs", "docs/images", "sources", "assets"];

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
        let filtered_assets =
            all_assets.into_iter().filter(|a| !stale_entries.contains(&a.path)).collect::<Vec<_>>();

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
        let filename = std::path::Path::new(&article.source_path)
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
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
