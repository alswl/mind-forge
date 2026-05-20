use std::path::Path;

use chrono::Utc;

use crate::defaults;
use crate::error::{MfError, Result};
use crate::model::project::ProjectImportReport;
use crate::service::{repo, util};

/// Import a directory as a project: create mind.yaml, build index.
pub fn import_project(
    repo_root: &Path,
    directory: &str,
    project_type: Option<&str>,
    source_dir: Option<&str>,
    assets_dir: Option<&str>,
    force: bool,
    _non_interactive: bool,
) -> Result<ProjectImportReport> {
    let dir_path = Path::new(directory);
    let canonical_dir = dir_path.canonicalize().map_err(|e| {
        MfError::usage(
            format!("directory '{directory}' not found: {e}"),
            Some("provide a valid directory path to import".to_string()),
        )
    })?;

    if !canonical_dir.is_dir() {
        return Err(MfError::usage(format!("'{directory}' is not a directory"), None));
    }

    // Check not inside existing project
    let mind_yaml_path = canonical_dir.join("mind.yaml");
    if mind_yaml_path.exists() && !force {
        return Err(MfError::usage(
            format!("'{}' already has a mind.yaml", canonical_dir.display()),
            Some("use --force to overwrite".to_string()),
        ));
    }

    // Determine project name from directory name
    let project_name = canonical_dir
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .ok_or_else(|| MfError::usage("cannot determine project name from directory path", None))?;

    // Validate project name
    util::validate_project_name(&project_name)?;

    // Determine source and assets dirs
    let sources_dir = source_dir.unwrap_or(defaults::SOURCES_DIR).to_string();
    let assets = assets_dir.unwrap_or(defaults::ASSETS_DIR).to_string();

    // Infer type from project_name or explicit --type
    let inferred_type = project_type.unwrap_or("arch");

    // Build mind.yaml content
    let mut doc = vec![format!("schema: '{}'", defaults::SCHEMA_VERSION)];
    doc.push(format!("type: {}", inferred_type));
    doc.push(format!("source_dirs: [{}]", sources_dir));
    doc.push(format!("assets_dir: {}", assets));
    if !canonical_dir.join(&sources_dir).exists() {
        std::fs::create_dir_all(canonical_dir.join(&sources_dir)).map_err(MfError::Io)?;
    }
    if !canonical_dir.join(&assets).exists() {
        std::fs::create_dir_all(canonical_dir.join(&assets)).map_err(MfError::Io)?;
    }

    let content = doc.join("\n") + "\n";
    util::atomic_write(&mind_yaml_path, &content)?;

    // Scan for articles in source dir
    let source_full_path = canonical_dir.join(&sources_dir);
    let mut article_count: u64 = 0;
    if source_full_path.exists() {
        for entry in walkdir::WalkDir::new(&source_full_path).follow_links(true).into_iter().filter_map(|e| e.ok()) {
            if entry.file_type().is_file() {
                article_count += 1;
            }
        }
    }

    // Create mind-index.yaml
    let index_path = canonical_dir.join("mind-index.yaml");
    if !index_path.exists() || force {
        let now = Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
        let article_type = serde_yaml::from_str(inferred_type).unwrap_or_default();
        let index = crate::model::index::IndexFile {
            schema_version: defaults::SCHEMA_VERSION.to_string(),
            sources: None,
            assets: None,
            articles: Some(vec![crate::model::article::Article {
                title: "Welcome".to_string(),
                project: project_name.clone(),
                article_type,
                article_path: String::new(),
                status: crate::model::article::ArticleStatus::Draft,
                created_at: now.clone(),
                updated_at: now,
                template_origin: None,
            }]),
            terms: None,
            publish_records: None,
            extra: None,
        };
        crate::service::index::save(&canonical_dir, &index)?;
    }

    // Register in minds.yaml
    let minds_path = repo_root.join("minds.yaml");
    let mut manifest = if minds_path.exists() {
        repo::load_manifest(&minds_path)?
    } else {
        crate::model::manifest::MindsManifest::create_default()
    };

    let projects_dir = repo::projects_dir_for(repo_root)?;
    // Only register if the imported dir is inside the projects dir
    let is_inside_repo = canonical_dir.strip_prefix(repo_root).is_ok();
    if is_inside_repo {
        if let Ok(rel) = canonical_dir.strip_prefix(repo_root) {
            let entry_path = if projects_dir.trim_matches('/').is_empty() || projects_dir.trim_matches('/') == "." {
                rel.to_string_lossy().to_string()
            } else {
                format!("./{}", rel.display())
            };
            let now = Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
            let entry = crate::model::manifest::ProjectEntry {
                name: project_name.clone(),
                path: entry_path,
                created_at: now,
                archived_at: None,
            };
            if !manifest.projects.iter().any(|p| p.name == project_name) {
                manifest.projects.push(entry);
            }
            repo::save_manifest(&manifest, &minds_path)?;
        }
    }

    Ok(ProjectImportReport {
        name: project_name,
        path: canonical_dir.to_string_lossy().to_string(),
        scaffolded: true,
        article_count,
    })
}
