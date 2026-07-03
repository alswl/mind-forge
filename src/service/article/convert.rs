use std::path::Path;

use chrono::Utc;

use crate::defaults;
use crate::error::{MfError, Result};
use crate::model::article::{skip_reason, ArticleShape, ConversionDirection, ConversionResult, ConversionStatus};
use crate::service::index;
use crate::service::util;

/// Derive the target single-file path from a directory article path.
///
/// `docs/daily` → `docs/daily.md`
pub fn derive_target_single_file_path(source_dir: &str) -> String {
    format!("{}.{}", source_dir, defaults::MARKDOWN_EXTENSION)
}

/// Derive the target directory path from a single-file article path.
///
/// `docs/daily.md` → `docs/daily`
pub fn derive_target_directory_path(source_file: &str) -> String {
    source_file.strip_suffix(&format!(".{}", defaults::MARKDOWN_EXTENSION)).unwrap_or(source_file).to_string()
}

/// Derive the target opening section path for a directory article.
///
/// `docs/daily` → `docs/daily/01-opening.md`
pub fn derive_opening_section_path(target_dir: &str) -> String {
    format!("{}/{}", target_dir, defaults::OPENING_SECTION_FILENAME)
}

/// Classify the on-disk shape of an article.
pub fn classify_article_shape(project_root: &Path, article_path: &str) -> ArticleShape {
    let full = project_root.join(article_path);
    if full.is_dir() {
        ArticleShape::Directory
    } else {
        ArticleShape::SingleFile
    }
}

/// List markdown section files in a directory article, sorted by name.
/// Returns the project-relative paths (e.g. `docs/daily/01-opening.md`).
pub fn list_section_files(project_root: &Path, article_path: &str) -> Result<Vec<String>> {
    let dir = project_root.join(article_path);
    let mut files: Vec<String> = Vec::new();
    for entry in std::fs::read_dir(&dir).map_err(MfError::Io)? {
        let entry = entry.map_err(MfError::Io)?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some(defaults::MARKDOWN_EXTENSION) {
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                files.push(format!("{}/{}", article_path, name));
            }
        }
    }
    files.sort();
    Ok(files)
}

/// Check if a directory article has non-section files that would prevent safe deletion.
pub fn has_extra_files(project_root: &Path, article_path: &str) -> Result<bool> {
    let dir = project_root.join(article_path);
    for entry in std::fs::read_dir(&dir).map_err(MfError::Io)? {
        let entry = entry.map_err(MfError::Io)?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some(defaults::MARKDOWN_EXTENSION) {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Candidate inspection result for a single direction.
pub struct CandidateInspection {
    pub eligible: bool,
    pub skip_reason: Option<String>,
    pub source_shape: ArticleShape,
    pub source_path: String,
    pub source_content_path: String,
    pub target_path: String,
    pub target_content_path: String,
}

impl CandidateInspection {
    /// Build a `ConversionResult` carrying this inspection's paths and shapes.
    pub fn to_result(
        &self,
        status: ConversionStatus,
        direction: ConversionDirection,
        reason: Option<String>,
        index_updated: bool,
        source_removed: bool,
    ) -> ConversionResult {
        ConversionResult {
            status,
            direction,
            source_shape: self.source_shape,
            target_shape: direction.target_shape(),
            source_path: self.source_path.clone(),
            source_content_path: self.source_content_path.clone(),
            target_path: self.target_path.clone(),
            target_content_path: self.target_content_path.clone(),
            reason,
            index_updated,
            source_removed,
        }
    }
}

/// Build an ineligible inspection sharing the common path/shape boilerplate.
fn ineligible(
    reason: &'static str,
    source_shape: ArticleShape,
    source_path: &str,
    source_content_path: String,
    target_path: String,
    target_content_path: String,
) -> CandidateInspection {
    CandidateInspection {
        eligible: false,
        skip_reason: Some(reason.to_string()),
        source_shape,
        source_path: source_path.to_string(),
        source_content_path,
        target_path,
        target_content_path,
    }
}

/// Inspect a single article for a given conversion direction and return eligibility.
pub fn inspect_candidate(
    project_root: &Path,
    article_path: &str,
    direction: ConversionDirection,
) -> Result<CandidateInspection> {
    let source_shape = classify_article_shape(project_root, article_path);

    match direction {
        ConversionDirection::ToSingleFile => inspect_to_single_file(project_root, article_path, source_shape),
        ConversionDirection::ToDirectory => inspect_to_directory(project_root, article_path, source_shape),
    }
}

fn inspect_to_single_file(
    project_root: &Path,
    article_path: &str,
    source_shape: ArticleShape,
) -> Result<CandidateInspection> {
    let target_path = derive_target_single_file_path(article_path);
    let target_content_path = target_path.clone();
    let bail = |reason, content_path: String| {
        ineligible(reason, source_shape, article_path, content_path, target_path.clone(), target_content_path.clone())
    };

    if source_shape != ArticleShape::Directory {
        return Ok(bail(skip_reason::NOT_DIRECTORY_ARTICLE, article_path.to_string()));
    }

    let section_files = list_section_files(project_root, article_path)?;
    if section_files.is_empty() {
        return Ok(bail(skip_reason::NO_SECTION_FILES, article_path.to_string()));
    }
    if section_files.len() > 1 {
        return Ok(bail(skip_reason::MULTIPLE_SECTION_FILES, article_path.to_string()));
    }
    let section = section_files.into_iter().next().expect("section_files has exactly one entry");

    if project_root.join(&target_path).exists() {
        return Ok(bail(skip_reason::TARGET_EXISTS, section));
    }
    if has_extra_files(project_root, article_path)? {
        return Ok(bail(skip_reason::EXTRA_FILES, section));
    }

    Ok(CandidateInspection {
        eligible: true,
        skip_reason: None,
        source_shape,
        source_path: article_path.to_string(),
        source_content_path: section,
        target_path,
        target_content_path,
    })
}

fn inspect_to_directory(
    project_root: &Path,
    article_path: &str,
    source_shape: ArticleShape,
) -> Result<CandidateInspection> {
    let target_dir = derive_target_directory_path(article_path);
    let target_content_path = derive_opening_section_path(&target_dir);
    let bail = |reason| {
        ineligible(
            reason,
            source_shape,
            article_path,
            article_path.to_string(),
            target_dir.clone(),
            target_content_path.clone(),
        )
    };

    if source_shape != ArticleShape::SingleFile {
        return Ok(bail(skip_reason::NOT_SINGLE_FILE_ARTICLE));
    }
    if project_root.join(&target_dir).exists() {
        return Ok(bail(skip_reason::TARGET_EXISTS));
    }

    Ok(CandidateInspection {
        eligible: true,
        skip_reason: None,
        source_shape,
        source_path: article_path.to_string(),
        source_content_path: article_path.to_string(),
        target_path: target_dir,
        target_content_path,
    })
}

/// Determine plausible directions and per-direction eligible counts for a set
/// of articles, in stable (ToSingleFile, ToDirectory) order. Directions with
/// zero eligible candidates are omitted.
pub fn plausible_directions(
    project_root: &Path,
    article_paths: &[String],
) -> Result<Vec<(ConversionDirection, usize)>> {
    let mut to_single = 0usize;
    let mut to_directory = 0usize;
    for ap in article_paths {
        let shape = classify_article_shape(project_root, ap);
        match shape {
            ArticleShape::Directory => {
                if inspect_to_single_file(project_root, ap, shape)?.eligible {
                    to_single += 1;
                }
            }
            ArticleShape::SingleFile => {
                if inspect_to_directory(project_root, ap, shape)?.eligible {
                    to_directory += 1;
                }
            }
        }
    }
    let mut directions = Vec::new();
    if to_single > 0 {
        directions.push((ConversionDirection::ToSingleFile, to_single));
    }
    if to_directory > 0 {
        directions.push((ConversionDirection::ToDirectory, to_directory));
    }
    Ok(directions)
}

/// Plan conversion: inspect all article paths for a given direction, return
/// results sorted by source_path.
pub fn plan_conversion(
    project_root: &Path,
    article_paths: &[String],
    direction: ConversionDirection,
) -> Result<Vec<CandidateInspection>> {
    let mut results = Vec::new();
    for ap in article_paths {
        results.push(inspect_candidate(project_root, ap, direction)?);
    }
    results.sort_by(|a, b| a.source_path.cmp(&b.source_path));
    Ok(results)
}

/// Execute to_single_file conversion: write target file, remove source directory.
/// Index update is the caller's responsibility.
pub fn execute_to_single_file(project_root: &Path, inspection: &CandidateInspection) -> Result<ConversionResult> {
    let source_content =
        std::fs::read_to_string(project_root.join(&inspection.source_content_path)).map_err(MfError::Io)?;

    let target_abs = project_root.join(&inspection.target_content_path);
    if let Some(parent) = target_abs.parent() {
        std::fs::create_dir_all(parent).map_err(MfError::Io)?;
    }
    std::fs::write(&target_abs, &source_content).map_err(MfError::Io)?;

    std::fs::remove_dir_all(project_root.join(&inspection.source_path)).map_err(MfError::Io)?;

    Ok(inspection.to_result(ConversionStatus::Converted, ConversionDirection::ToSingleFile, None, false, true))
}

/// Execute to_directory conversion: write target directory with the opening
/// section, remove the source file. Index update is the caller's responsibility.
pub fn execute_to_directory(project_root: &Path, inspection: &CandidateInspection) -> Result<ConversionResult> {
    let source_content =
        std::fs::read_to_string(project_root.join(&inspection.source_content_path)).map_err(MfError::Io)?;

    let target_dir = project_root.join(&inspection.target_path);
    util::atomic_write_directory(&target_dir, &[(defaults::OPENING_SECTION_FILENAME, &source_content)])?;

    std::fs::remove_file(project_root.join(&inspection.source_path)).map_err(MfError::Io)?;

    Ok(inspection.to_result(ConversionStatus::Converted, ConversionDirection::ToDirectory, None, false, true))
}

/// Update the index for a converted article. No-op (still `Ok`) if the old
/// path is not present — the caller already trusts the planning pass.
pub fn update_index_for_conversion(project_root: &Path, old_article_path: &str, new_article_path: &str) -> Result<()> {
    let mut index = index::load(project_root)?;
    if let Some(ref mut articles) = index.articles {
        for article in articles.iter_mut() {
            if article.article_path == old_article_path {
                article.article_path = new_article_path.to_string();
                article.updated_at = Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
                break;
            }
        }
    }
    index::save(project_root, &index)?;
    Ok(())
}
