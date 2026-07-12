use std::fs;
use std::path::Path;

use serde::Serialize;

use crate::error::{MfError, Result};

use super::list_section_files;
use super::rename::resolve_block_filename;

/// Report from a block removal within a directory article.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct BlockRemoveReport {
    pub article_path: String,
    pub removed_filename: String,
    pub remaining_blocks: usize,
    #[serde(default)]
    pub dry_run: bool,
}

/// Remove a block file within a directory article.
///
/// `article_path` is the project-relative path to the directory article
/// (e.g. `docs/my-article`). `block` matches the block by filename
/// (e.g. `02-notes.md`), filename stem (e.g. `02-notes`), or slug (e.g.
/// `notes`) — the same resolution rules as `mf article block rename`.
///
/// Refuses to remove the last remaining block: a directory article with a
/// single block should be converted (`mf article convert --to-single-file`)
/// or removed outright (`mf article remove`) instead of left with zero
/// blocks. All validation (directory shape, block resolution, last-block
/// guard) runs regardless of `dry_run`, so a dry-run accurately previews
/// whether the real removal would succeed; only the filesystem write is
/// skipped when `dry_run` is true.
pub fn remove_block(project_path: &Path, article_path: &str, block: &str, dry_run: bool) -> Result<BlockRemoveReport> {
    // 1. Verify article is a directory
    let article_full = project_path.join(article_path);
    if !article_full.is_dir() {
        return Err(MfError::usage(
            format!("'{}' is not a directory article", article_path),
            Some(
                "block removal only works on directory articles. \
                 Use `mf article remove` to remove a single-file article."
                    .to_string(),
            ),
        ));
    }

    // 2. Find the block file(s)
    let section_files = list_section_files(project_path, article_path)?;
    if section_files.is_empty() {
        return Err(MfError::not_found(
            format!("no block files found in article '{}'", article_path),
            Some("directory articles must have at least one .md block file".to_string()),
        ));
    }

    // 3. Refuse to remove the last remaining block
    if section_files.len() == 1 {
        return Err(MfError::usage(
            format!("cannot remove the last remaining block in article '{}'", article_path),
            Some(
                "use `mf article remove` to delete the whole article, or \
                 `mf article convert --to-single-file` to collapse it into a single file"
                    .to_string(),
            ),
        ));
    }

    // 4. Resolve the block identifier to a filename
    let filename = resolve_block_filename(&section_files, article_path, block)?;

    // 5. Verify the block file exists on disk
    let block_full_path = article_full.join(&filename);
    if !block_full_path.exists() {
        return Err(MfError::not_found(
            format!("block file '{}' not found on disk", filename),
            Some("the file may have been moved or deleted".to_string()),
        ));
    }

    let remaining_blocks = section_files.len() - 1;

    if dry_run {
        return Ok(BlockRemoveReport {
            article_path: article_path.to_string(),
            removed_filename: filename,
            remaining_blocks,
            dry_run: true,
        });
    }

    // 6. Remove it
    fs::remove_file(&block_full_path).map_err(MfError::Io)?;

    Ok(BlockRemoveReport {
        article_path: article_path.to_string(),
        removed_filename: filename,
        remaining_blocks,
        dry_run: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remove_block_by_filename() {
        let tmp = tempfile::tempdir().unwrap();
        let proj = tmp.path();
        let article_dir = proj.join("docs/my-article");
        fs::create_dir_all(&article_dir).unwrap();
        fs::write(article_dir.join("01-opening.md"), "# Title\n\nintro\n").unwrap();
        fs::write(article_dir.join("02-notes.md"), "## Notes\n\nnotes body\n").unwrap();

        let report = remove_block(proj, "docs/my-article", "02-notes.md", false).unwrap();
        assert_eq!(report.removed_filename, "02-notes.md");
        assert_eq!(report.remaining_blocks, 1);
        assert!(!article_dir.join("02-notes.md").exists());
        assert!(article_dir.join("01-opening.md").exists());
    }

    #[test]
    fn remove_block_by_slug() {
        let tmp = tempfile::tempdir().unwrap();
        let proj = tmp.path();
        let article_dir = proj.join("docs/my-article");
        fs::create_dir_all(&article_dir).unwrap();
        fs::write(article_dir.join("01-opening.md"), "# Title\n\nintro\n").unwrap();
        fs::write(article_dir.join("02-refs.md"), "## Refs\n\nrefs body\n").unwrap();

        let report = remove_block(proj, "docs/my-article", "refs", false).unwrap();
        assert_eq!(report.removed_filename, "02-refs.md");
    }

    #[test]
    fn remove_block_refuses_last_remaining_block() {
        let tmp = tempfile::tempdir().unwrap();
        let proj = tmp.path();
        let article_dir = proj.join("docs/my-article");
        fs::create_dir_all(&article_dir).unwrap();
        fs::write(article_dir.join("01-opening.md"), "# Title\n\nintro\n").unwrap();

        let err = remove_block(proj, "docs/my-article", "01-opening.md", false).unwrap_err();
        assert!(matches!(err, MfError::Usage { .. }));
        let msg = err.to_string();
        assert!(msg.contains("last remaining block"), "message: {msg}");
        // File must be untouched.
        assert!(article_dir.join("01-opening.md").exists());
    }

    #[test]
    fn remove_block_not_directory_article() {
        let tmp = tempfile::tempdir().unwrap();
        let proj = tmp.path();
        fs::create_dir_all(proj.join("docs")).unwrap();
        fs::write(proj.join("docs/my-article.md"), "# Title\n\ncontent\n").unwrap();

        let err = remove_block(proj, "docs/my-article.md", "02-notes", false).unwrap_err();
        assert!(matches!(err, MfError::Usage { .. }));
        assert!(err.to_string().contains("not a directory article"));
    }

    #[test]
    fn remove_block_not_found() {
        let tmp = tempfile::tempdir().unwrap();
        let proj = tmp.path();
        let article_dir = proj.join("docs/my-article");
        fs::create_dir_all(&article_dir).unwrap();
        fs::write(article_dir.join("01-opening.md"), "# Title\n\nintro\n").unwrap();
        fs::write(article_dir.join("02-notes.md"), "## Notes\n\nbody\n").unwrap();

        let err = remove_block(proj, "docs/my-article", "03-missing", false).unwrap_err();
        assert!(matches!(err, MfError::NotFound { .. }));
    }

    #[test]
    fn remove_block_dry_run_does_not_write() {
        let tmp = tempfile::tempdir().unwrap();
        let proj = tmp.path();
        let article_dir = proj.join("docs/my-article");
        fs::create_dir_all(&article_dir).unwrap();
        fs::write(article_dir.join("01-opening.md"), "# Title\n\nintro\n").unwrap();
        fs::write(article_dir.join("02-notes.md"), "## Notes\n\nnotes body\n").unwrap();

        let report = remove_block(proj, "docs/my-article", "02-notes.md", true).unwrap();
        assert_eq!(report.removed_filename, "02-notes.md");
        assert_eq!(report.remaining_blocks, 1);
        assert!(report.dry_run);
        // Nothing was actually removed.
        assert!(article_dir.join("02-notes.md").exists());
    }

    #[test]
    fn remove_block_dry_run_still_refuses_last_remaining_block() {
        let tmp = tempfile::tempdir().unwrap();
        let proj = tmp.path();
        let article_dir = proj.join("docs/my-article");
        fs::create_dir_all(&article_dir).unwrap();
        fs::write(article_dir.join("01-opening.md"), "# Title\n\nintro\n").unwrap();

        let err = remove_block(proj, "docs/my-article", "01-opening.md", true).unwrap_err();
        assert!(matches!(err, MfError::Usage { .. }));
    }

    #[test]
    fn remove_block_ambiguous_slug() {
        let tmp = tempfile::tempdir().unwrap();
        let proj = tmp.path();
        let article_dir = proj.join("docs/my-article");
        fs::create_dir_all(&article_dir).unwrap();
        fs::write(article_dir.join("01-notes.md"), "# Notes A\n").unwrap();
        fs::write(article_dir.join("02-notes.md"), "# Notes B\n").unwrap();
        fs::write(article_dir.join("03-third.md"), "# Third\n").unwrap();

        let err = remove_block(proj, "docs/my-article", "notes", false).unwrap_err();
        assert!(matches!(err, MfError::Usage { .. }));
        assert!(err.to_string().contains("multiple blocks match"));
    }
}
