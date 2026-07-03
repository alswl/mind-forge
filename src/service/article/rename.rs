use std::fs;
use std::path::Path;

use chrono::Utc;
use serde::Serialize;

use crate::defaults;
use crate::error::{MfError, Result};
use crate::service::config as config_svc;
use crate::service::index;
use crate::service::util;

use super::list_section_files;

/// Report from a successful article rename.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct ArticleRenameReport {
    pub old_title: String,
    pub new_title: String,
    pub old_article_path: String,
    pub new_article_path: String,
    #[serde(default)]
    pub references: Vec<crate::model::lifecycle::Reference>,
    #[serde(default)]
    pub side_effects: Vec<crate::model::lifecycle::PlannedChange>,
    #[serde(default)]
    pub force: bool,
    #[serde(default)]
    pub dry_run: bool,
}

/// Rename an article: renames the file/directory on disk and updates the index.
///
/// `old_selector` is matched against article title or path in the index.
/// `new_slug` is the new filename/directory slug (NOT a title); the title is
/// left unchanged. The article shape (single-file or directory) is preserved.
pub fn rename_article(
    project_path: &Path,
    old_selector: &str,
    new_slug: &str,
    force: bool,
) -> Result<ArticleRenameReport> {
    let layout = config_svc::effective_layout(project_path)?;

    // Load the index and find the article
    let mut index = index::load(project_path)?;
    let articles = index.articles.as_mut().ok_or_else(|| {
        MfError::not_found(
            format!("article '{old_selector}' not found"),
            Some("use `mf article list --project <project>` to see available articles".to_string()),
        )
    })?;

    let article =
        articles.iter_mut().find(|a| a.title == old_selector || a.article_path == old_selector).ok_or_else(|| {
            MfError::not_found(
                format!("article '{old_selector}' not found"),
                Some("use `mf article list --project <project>` to see available articles".to_string()),
            )
        })?;

    let old_article_path = article.article_path.clone();
    let old_article_title = article.title.clone();

    // Detect article shape: directory or single-file
    let old_full = project_path.join(&old_article_path);
    let is_directory = old_full.is_dir();

    // Sanitize slug via to_filename (lowercase, hyphens, etc.)
    let slug = util::to_filename(new_slug);

    // Construct new path preserving the original shape
    let new_article_path = if is_directory {
        format!("{}/{}", layout.articles, slug)
    } else {
        format!("{}/{}.{}", layout.articles, slug, defaults::MARKDOWN_EXTENSION)
    };

    // Rename on disk (only if the path actually differs)
    let mut side_effects: Vec<crate::model::lifecycle::PlannedChange> = Vec::new();
    if old_article_path != new_article_path {
        let new_full = project_path.join(&new_article_path);

        if !old_full.exists() {
            return Err(MfError::not_found(
                format!("article not found at {}", old_full.display()),
                Some("the index may be out of date; try `mf article index`".to_string()),
            ));
        }

        if new_full.exists() {
            if force {
                if new_full.is_dir() {
                    fs::remove_dir_all(&new_full).map_err(MfError::Io)?;
                } else {
                    fs::remove_file(&new_full).map_err(MfError::Io)?;
                }
            } else {
                return Err(MfError::file_exists(new_full));
            }
        }

        fs::rename(&old_full, &new_full).map_err(MfError::Io)?;

        let rename_op = if is_directory {
            crate::model::lifecycle::PlannedOp::RenameDir
        } else {
            crate::model::lifecycle::PlannedOp::RenameFile
        };
        side_effects.push(crate::model::lifecycle::PlannedChange {
            op: rename_op,
            path: new_full.to_string_lossy().to_string(),
            old: Some(old_article_path.clone()),
            new: Some(new_article_path.clone()),
        });

        // Rename associated prompt file if it exists
        let old_key = index::article_output_stem(&old_article_path);
        let new_key = &slug;
        let old_prompt_path = project_path.join("prompts").join(format!("{old_key}.md"));
        if old_prompt_path.exists() {
            let new_prompt_path = project_path.join("prompts").join(format!("{new_key}.md"));
            if new_prompt_path.exists() && force {
                fs::remove_file(&new_prompt_path).map_err(MfError::Io)?;
            }
            if !new_prompt_path.exists() {
                fs::rename(&old_prompt_path, &new_prompt_path).map_err(MfError::Io)?;
                // Update the article: frontmatter binding in the prompt
                if let Ok(content) = fs::read_to_string(&new_prompt_path) {
                    let updated = update_prompt_article_binding(&content, &old_article_path, &new_article_path);
                    let _ = fs::write(&new_prompt_path, updated);
                }
                side_effects.push(crate::model::lifecycle::PlannedChange {
                    op: crate::model::lifecycle::PlannedOp::RenameFile,
                    path: new_prompt_path.to_string_lossy().to_string(),
                    old: Some(old_prompt_path.to_string_lossy().to_string()),
                    new: Some(new_prompt_path.to_string_lossy().to_string()),
                });
            }
        }
    }

    // Update the index entry — title is left unchanged
    let now = Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
    article.article_path = new_article_path.clone();
    article.updated_at = now;
    index::save(project_path, &index)?;

    Ok(ArticleRenameReport {
        old_title: old_article_title.clone(),
        new_title: old_article_title,
        old_article_path,
        new_article_path,
        references: vec![],
        side_effects,
        force,
        dry_run: false,
    })
}

/// Report from a successful block rename within a directory article.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct BlockRenameReport {
    pub old_filename: String,
    pub new_filename: String,
    pub article_path: String,
    #[serde(default)]
    pub force: bool,
}

/// Rename a block file within a directory article.
///
/// `article_path` is the project-relative path to the directory article
/// (e.g. `docs/my-article`). `old_block` matches the block by filename
/// (e.g. `02-notes.md`) or by slug (e.g. `notes`). `new_slug` is the new
/// slug — the number prefix is preserved, producing `02-{new_slug}.md`.
/// The H2 heading within the file is NOT changed (by analogy with article
/// rename which keeps the title unchanged).
pub fn rename_block(
    project_path: &Path,
    article_path: &str,
    old_block: &str,
    new_slug: &str,
    force: bool,
) -> Result<BlockRenameReport> {
    // 1. Verify article is a directory
    let article_full = project_path.join(article_path);
    if !article_full.is_dir() {
        return Err(MfError::usage(
            format!("'{}' is not a directory article", article_path),
            Some(
                "block rename only works on directory articles. \
                 Use `mf article rename` to rename a single-file article."
                    .to_string(),
            ),
        ));
    }

    // 2. Find the block file
    let section_files = list_section_files(project_path, article_path)?;
    if section_files.is_empty() {
        return Err(MfError::not_found(
            format!("no block files found in article '{}'", article_path),
            Some("directory articles must have at least one .md block file".to_string()),
        ));
    }

    // Parse old_block: full filename like "02-notes.md", filename without extension
    // like "02-notes", or just the slug like "notes"
    let old_block_filename = if old_block.contains('.') {
        if !old_block.ends_with(".md") {
            return Err(MfError::usage(
                format!("block identifier '{}' must be a .md file", old_block),
                Some("use the filename (e.g. '02-notes.md') or the slug (e.g. 'notes')".to_string()),
            ));
        }
        old_block.to_string()
    } else {
        let sanitized_old = util::to_filename(old_block);
        // First pass: exact match against filename without extension (e.g. "02-notes")
        let mut candidates: Vec<&String> = section_files
            .iter()
            .filter(|f| {
                let name = Path::new(f).file_name().and_then(|n| n.to_str()).unwrap_or("");
                name.strip_suffix(&format!(".{}", defaults::MARKDOWN_EXTENSION)) == Some(&sanitized_old)
            })
            .collect();
        // Second pass: match by slug only if the first pass found nothing
        if candidates.is_empty() {
            candidates = section_files
                .iter()
                .filter(|f| {
                    let name = Path::new(f).file_name().and_then(|n| n.to_str()).unwrap_or("");
                    extract_slug_from_filename(name).as_deref() == Some(&sanitized_old)
                })
                .collect();
        }
        match candidates.len() {
            0 => {
                return Err(MfError::not_found(
                    format!("block '{}' not found in article '{}'", old_block, article_path),
                    Some("use `mf article show <article>` to list blocks".to_string()),
                ));
            }
            1 => {
                let full_path = candidates[0];
                Path::new(full_path).file_name().and_then(|n| n.to_str()).unwrap_or("").to_string()
            }
            _ => {
                return Err(MfError::usage(
                    format!("multiple blocks match '{}' in article '{}'", old_block, article_path),
                    Some("use the full filename (e.g. '02-notes.md') to disambiguate".to_string()),
                ));
            }
        }
    };

    // 3. Verify the block file exists on disk
    let old_block_full_path = article_full.join(&old_block_filename);
    if !old_block_full_path.exists() {
        return Err(MfError::not_found(
            format!("block file '{}' not found on disk", old_block_filename),
            Some("the file may have been moved or deleted".to_string()),
        ));
    }

    // 4. Extract the number prefix from the old filename
    let prefix = extract_number_prefix(&old_block_filename).ok_or_else(|| {
        MfError::Internal(anyhow::anyhow!("cannot extract number prefix from block filename '{}'", old_block_filename))
    })?;

    // 5. Sanitize the new slug
    let sanitized_slug = util::to_filename(new_slug);
    if sanitized_slug.is_empty() || sanitized_slug == "untitled" {
        return Err(MfError::usage(
            format!("new slug '{}' produces an empty filename", new_slug),
            Some("provide a non-empty slug with at least one alphanumeric character".to_string()),
        ));
    }

    // 6. Construct the new filename preserving the number prefix
    let new_block_filename = format!("{}-{}.{}", prefix, sanitized_slug, defaults::MARKDOWN_EXTENSION);
    let new_block_full_path = article_full.join(&new_block_filename);

    // 7. Check no-op case
    if old_block_filename == new_block_filename {
        return Ok(BlockRenameReport {
            old_filename: old_block_filename,
            new_filename: new_block_filename,
            article_path: article_path.to_string(),
            force,
        });
    }

    // 8. Check for collision
    if new_block_full_path.exists() {
        if force {
            fs::remove_file(&new_block_full_path).map_err(MfError::Io)?;
        } else {
            return Err(MfError::file_exists(new_block_full_path));
        }
    }

    // 9. Rename the file on disk
    fs::rename(&old_block_full_path, &new_block_full_path).map_err(MfError::Io)?;

    Ok(BlockRenameReport {
        old_filename: old_block_filename,
        new_filename: new_block_filename,
        article_path: article_path.to_string(),
        force,
    })
}

/// Extract the numeric prefix from a block filename like "02-notes.md" → "02".
fn extract_number_prefix(filename: &str) -> Option<String> {
    let name = filename.strip_suffix(&format!(".{}", defaults::MARKDOWN_EXTENSION)).unwrap_or(filename);
    let prefix: String = name.chars().take_while(|c| c.is_ascii_digit()).collect();
    if prefix.is_empty() {
        None
    } else {
        Some(prefix)
    }
}

/// Extract the slug from a block filename like "02-notes.md" → "notes"
/// or "01-opening.md" → "opening".
fn extract_slug_from_filename(filename: &str) -> Option<String> {
    let name = filename.strip_suffix(&format!(".{}", defaults::MARKDOWN_EXTENSION)).unwrap_or(filename);
    let non_digit_pos = name.find(|c: char| !c.is_ascii_digit())?;
    let slug = name[non_digit_pos..].trim_start_matches('-');
    if slug.is_empty() {
        None
    } else {
        Some(slug.to_string())
    }
}

/// Replace the `article:` binding in a prompt's YAML frontmatter.
fn update_prompt_article_binding(content: &str, old_path: &str, new_path: &str) -> String {
    let mut lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();
    let mut in_frontmatter = false;
    let mut frontmatter_open = false;
    for line in &mut lines {
        if line.trim() == "---" {
            if !frontmatter_open {
                frontmatter_open = true;
                in_frontmatter = true;
                continue;
            } else if in_frontmatter {
                break;
            }
        }
        if in_frontmatter {
            if let Some(rest) = line.strip_prefix("article:") {
                if rest.trim() == old_path {
                    *line = format!("article: {}", new_path);
                }
            }
        }
    }
    lines.join("\n") + if content.ends_with('\n') { "\n" } else { "" }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── extract_number_prefix tests ──

    #[test]
    fn extract_prefix_basic() {
        assert_eq!(extract_number_prefix("02-notes.md"), Some("02".to_string()));
        assert_eq!(extract_number_prefix("01-opening.md"), Some("01".to_string()));
        assert_eq!(extract_number_prefix("10-body.md"), Some("10".to_string()));
    }

    #[test]
    fn extract_prefix_no_extension() {
        assert_eq!(extract_number_prefix("02-notes"), Some("02".to_string()));
    }

    #[test]
    fn extract_prefix_no_prefix() {
        assert_eq!(extract_number_prefix("notes.md"), None);
        assert_eq!(extract_number_prefix("introduction.md"), None);
    }

    // ── extract_slug_from_filename tests ──

    #[test]
    fn extract_slug_basic() {
        assert_eq!(extract_slug_from_filename("02-notes.md"), Some("notes".to_string()));
        assert_eq!(extract_slug_from_filename("01-opening.md"), Some("opening".to_string()));
        assert_eq!(
            extract_slug_from_filename("05-alternatives-considered.md"),
            Some("alternatives-considered".to_string())
        );
    }

    #[test]
    fn extract_slug_no_prefix() {
        assert_eq!(extract_slug_from_filename("notes.md"), Some("notes".to_string()));
    }

    // ── rename_block tests ──

    #[test]
    fn rename_block_by_filename() {
        let tmp = tempfile::tempdir().unwrap();
        let proj = tmp.path();
        let article_dir = proj.join("docs/my-article");
        fs::create_dir_all(&article_dir).unwrap();
        fs::write(article_dir.join("01-opening.md"), "# Title\n\nintro\n").unwrap();
        fs::write(article_dir.join("02-notes.md"), "## Notes\n\nnotes body\n").unwrap();

        let report = rename_block(proj, "docs/my-article", "02-notes.md", "thoughts", false).unwrap();
        assert_eq!(report.old_filename, "02-notes.md");
        assert_eq!(report.new_filename, "02-thoughts.md");
        assert!(article_dir.join("02-thoughts.md").exists());
        assert!(!article_dir.join("02-notes.md").exists());
        // Content is unchanged
        let content = fs::read_to_string(article_dir.join("02-thoughts.md")).unwrap();
        assert_eq!(content, "## Notes\n\nnotes body\n");
    }

    #[test]
    fn rename_block_by_slug() {
        let tmp = tempfile::tempdir().unwrap();
        let proj = tmp.path();
        let article_dir = proj.join("docs/my-article");
        fs::create_dir_all(&article_dir).unwrap();
        fs::write(article_dir.join("01-opening.md"), "# Title\n\nintro\n").unwrap();
        fs::write(article_dir.join("02-refs.md"), "## Refs\n\nrefs body\n").unwrap();

        let report = rename_block(proj, "docs/my-article", "refs", "references", false).unwrap();
        assert_eq!(report.old_filename, "02-refs.md");
        assert_eq!(report.new_filename, "02-references.md");
        assert!(article_dir.join("02-references.md").exists());
    }

    #[test]
    fn rename_block_not_directory_article() {
        let tmp = tempfile::tempdir().unwrap();
        let proj = tmp.path();
        fs::create_dir_all(proj.join("docs")).unwrap();
        fs::write(proj.join("docs/my-article.md"), "# Title\n\ncontent\n").unwrap();

        let err = rename_block(proj, "docs/my-article.md", "02-notes", "new-notes", false).unwrap_err();
        assert!(matches!(err, MfError::Usage { .. }));
        assert!(err.to_string().contains("not a directory article"));
    }

    #[test]
    fn rename_block_not_found() {
        let tmp = tempfile::tempdir().unwrap();
        let proj = tmp.path();
        let article_dir = proj.join("docs/my-article");
        fs::create_dir_all(&article_dir).unwrap();
        fs::write(article_dir.join("01-opening.md"), "# Title\n\nintro\n").unwrap();

        let err = rename_block(proj, "docs/my-article", "03-missing", "new-slug", false).unwrap_err();
        assert!(matches!(err, MfError::NotFound { .. }));
        assert!(err.to_string().contains("not found"));
    }

    #[test]
    fn rename_block_target_exists_without_force() {
        let tmp = tempfile::tempdir().unwrap();
        let proj = tmp.path();
        let article_dir = proj.join("docs/my-article");
        fs::create_dir_all(&article_dir).unwrap();
        fs::write(article_dir.join("01-opening.md"), "# T\n\nintro\n").unwrap();
        fs::write(article_dir.join("02-old.md"), "## Old\n\nold\n").unwrap();
        fs::write(article_dir.join("02-existing.md"), "## Existing\n\ntext\n").unwrap();

        let err = rename_block(proj, "docs/my-article", "02-old.md", "existing", false).unwrap_err();
        assert!(matches!(err, MfError::FileExists { .. }));
    }

    #[test]
    fn rename_block_target_exists_with_force() {
        let tmp = tempfile::tempdir().unwrap();
        let proj = tmp.path();
        let article_dir = proj.join("docs/my-article");
        fs::create_dir_all(&article_dir).unwrap();
        fs::write(article_dir.join("01-opening.md"), "# T\n\nintro\n").unwrap();
        fs::write(article_dir.join("02-old.md"), "## Old\n\nold\n").unwrap();
        fs::write(article_dir.join("02-existing.md"), "## Existing\n\ntext\n").unwrap();

        let report = rename_block(proj, "docs/my-article", "02-old.md", "existing", true).unwrap();
        assert_eq!(report.new_filename, "02-existing.md");
        assert!(article_dir.join("02-existing.md").exists());
        assert!(!article_dir.join("02-old.md").exists());
    }

    #[test]
    fn rename_block_noop() {
        let tmp = tempfile::tempdir().unwrap();
        let proj = tmp.path();
        let article_dir = proj.join("docs/my-article");
        fs::create_dir_all(&article_dir).unwrap();
        fs::write(article_dir.join("01-opening.md"), "# Title\n\nintro\n").unwrap();
        fs::write(article_dir.join("02-notes.md"), "## Notes\n\nbody\n").unwrap();

        let report = rename_block(proj, "docs/my-article", "02-notes.md", "notes", false).unwrap();
        assert_eq!(report.old_filename, "02-notes.md");
        assert_eq!(report.new_filename, "02-notes.md");
    }
}
