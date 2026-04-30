use std::fs;
use std::path::Path;

use serde::Serialize;

use crate::error::{MfError, Result};
use crate::service::util;

/// A build plan describing what would happen during a build.
#[derive(Debug, Serialize)]
pub struct BuildPlan {
    pub article: String,
    pub project: String,
    pub source_path: String,
    pub size_bytes: u64,
    pub dry_run: bool,
}

/// Build an article: read the markdown source and return its content.
///
/// `article` is the article name (filename stem within `docs/`).
/// `project_path` is the resolved project directory.
pub fn build_article(project_path: &Path, article: &str, dry_run: bool) -> Result<BuildOutput> {
    let source_path = project_path.join("docs").join(format!("{article}.md"));
    if !source_path.exists() {
        return Err(MfError::usage(
            format!("article not found: '{article}'"),
            Some("use `mf article list` to see available articles".to_string()),
        ));
    }

    let metadata = fs::metadata(&source_path).map_err(MfError::Io)?;
    let size_bytes = metadata.len();

    if dry_run {
        return Ok(BuildOutput::Plan(BuildPlan {
            article: article.to_string(),
            project: util::dir_name(project_path),
            source_path: source_path.to_string_lossy().to_string(),
            size_bytes,
            dry_run: true,
        }));
    }

    let content = fs::read_to_string(&source_path).map_err(MfError::Io)?;
    Ok(BuildOutput::Content(content))
}

/// Build output: either the rendered content or a dry-run plan.
#[derive(Debug)]
pub enum BuildOutput {
    Content(String),
    Plan(BuildPlan),
}
