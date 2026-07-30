use std::fs;
use std::path::Path;

use serde::Serialize;

use crate::error::{MfError, Result};

use super::list_section_files;
use super::rename::resolve_block_filename;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct BlockEditReport {
    pub article_path: String,
    pub old_path: Option<String>,
    pub new_path: Option<String>,
    pub order: Vec<String>,
    pub dry_run: bool,
}

fn block_slug(filename: &str) -> String {
    filename.trim_end_matches(".md").split_once('-').map(|(_, slug)| slug).unwrap_or(filename).to_string()
}

fn rewrite_blocks(project_path: &Path, article_path: &str, blocks: &[(String, String)], dry_run: bool) -> Result<()> {
    if dry_run {
        return Ok(());
    }
    let article_dir = project_path.join(article_path);
    let tmp = article_dir.with_file_name(format!(".{}.rewrite", article_dir.file_name().unwrap().to_string_lossy()));
    if tmp.exists() {
        fs::remove_dir_all(&tmp).map_err(MfError::Io)?;
    }
    let refs: Vec<(&str, &str)> = blocks.iter().map(|(name, body)| (name.as_str(), body.as_str())).collect();
    crate::service::util::atomic_write_directory(&tmp, &refs)?;
    let backup = article_dir.with_file_name(format!(".{}.backup", article_dir.file_name().unwrap().to_string_lossy()));
    if backup.exists() {
        fs::remove_dir_all(&backup).map_err(MfError::Io)?;
    }
    fs::rename(&article_dir, &backup).map_err(MfError::Io)?;
    if let Err(error) = fs::rename(&tmp, &article_dir).map_err(MfError::Io) {
        let _ = fs::rename(&backup, &article_dir);
        return Err(error);
    }
    fs::remove_dir_all(&backup).map_err(MfError::Io)
}

pub fn new_block(
    project_path: &Path,
    article_path: &str,
    slug: &str,
    after: Option<&str>,
    start: usize,
    dry_run: bool,
) -> Result<BlockEditReport> {
    let files: Vec<String> = list_section_files(project_path, article_path)?
        .into_iter()
        .filter_map(|path| Path::new(&path).file_name().and_then(|name| name.to_str()).map(str::to_string))
        .collect();
    if files.is_empty() {
        return Err(MfError::not_found("no blocks found", None::<String>));
    }
    let new_slug = crate::service::util::to_filename(slug);
    if new_slug.is_empty() || new_slug == "untitled" {
        return Err(MfError::usage("block slug must contain an alphanumeric character", None::<String>));
    }
    if files.iter().any(|file| block_slug(file) == new_slug) {
        return Err(MfError::usage(
            format!("block slug '{}' already exists", slug),
            Some("choose a unique block slug".to_string()),
        ));
    }
    let insert_at = match after {
        Some(block) => files
            .iter()
            .position(|file| file == block || block_slug(file) == block)
            .map(|i| i + 1)
            .ok_or_else(|| MfError::not_found(format!("block '{block}' not found"), None::<String>))?,
        None => files.len(),
    };
    let mut bodies: Vec<(String, String)> = files
        .iter()
        .map(|file| {
            Ok((file.clone(), fs::read_to_string(project_path.join(article_path).join(file)).map_err(MfError::Io)?))
        })
        .collect::<Result<_>>()?;
    let mut new_body = format!("# {}\n\n", slug.trim());
    if let Ok(config) = crate::service::config::load_project(project_path, Some(project_path))
        && let Some(config) = config
        && super::effective_typora_enabled(config.plugins.as_ref())
    {
        let layout = crate::service::config::effective_layout(project_path)?;
        let assets_path =
            super::compute_typora_assets_path(project_path, &layout.assets, &project_path.join(article_path));
        new_body = super::inject_typora_front_matter(&new_body, &assets_path);
    }
    bodies.insert(insert_at, (String::new(), new_body));
    let mut normalized = Vec::new();
    for (i, (old, body)) in bodies.into_iter().enumerate() {
        let block_name = if old.is_empty() { new_slug.clone() } else { block_slug(&old) };
        normalized.push((format!("{:02}-{}.md", start + i, block_name), body));
    }
    let order = normalized.iter().map(|(name, _)| name.clone()).collect();
    rewrite_blocks(project_path, article_path, &normalized, dry_run)?;
    Ok(BlockEditReport {
        article_path: article_path.to_string(),
        old_path: None,
        new_path: Some(format!("{article_path}/{:02}-{}.md", start + insert_at, new_slug)),
        order,
        dry_run,
    })
}

pub fn move_block(
    project_path: &Path,
    article_path: &str,
    block: &str,
    after: Option<&str>,
    start: usize,
    dry_run: bool,
) -> Result<BlockEditReport> {
    let files: Vec<String> = list_section_files(project_path, article_path)?
        .into_iter()
        .filter_map(|path| Path::new(&path).file_name().and_then(|name| name.to_str()).map(str::to_string))
        .collect();
    let from = resolve_block_filename(&files, article_path, block)?;
    let moved_body = fs::read_to_string(project_path.join(article_path).join(&from)).map_err(MfError::Io)?;
    let mut selected = files.clone();
    let item = selected.remove(selected.iter().position(|f| f == &from).unwrap());
    let at = match after {
        Some(value) => selected
            .iter()
            .position(|f| f == value || block_slug(f) == value)
            .map(|i| i + 1)
            .ok_or_else(|| MfError::not_found(format!("block '{value}' not found"), None::<String>))?,
        None => selected.len(),
    };
    selected.insert(at, item);
    let mut normalized = Vec::new();
    for (i, old) in selected.iter().enumerate() {
        normalized.push((
            format!("{:02}-{}.md", start + i, block_slug(old)),
            fs::read_to_string(project_path.join(article_path).join(old)).map_err(MfError::Io)?,
        ));
    }
    let order = normalized.iter().map(|(name, _)| name.clone()).collect();
    rewrite_blocks(project_path, article_path, &normalized, dry_run)?;
    let new_name = normalized.iter().find(|(_, body)| body == &moved_body).map(|(name, _)| name.clone());
    Ok(BlockEditReport {
        article_path: article_path.to_string(),
        old_path: Some(format!("{article_path}/{from}")),
        new_path: new_name.map(|n| format!("{article_path}/{n}")),
        order,
        dry_run,
    })
}

pub fn renumber_blocks(
    project_path: &Path,
    article_path: &str,
    start: usize,
    dry_run: bool,
) -> Result<BlockEditReport> {
    let files: Vec<String> = list_section_files(project_path, article_path)?
        .into_iter()
        .filter_map(|path| Path::new(&path).file_name().and_then(|name| name.to_str()).map(str::to_string))
        .collect();
    let mut normalized = Vec::new();
    for (i, old) in files.iter().enumerate() {
        normalized.push((
            format!("{:02}-{}.md", start + i, block_slug(old)),
            fs::read_to_string(project_path.join(article_path).join(old)).map_err(MfError::Io)?,
        ));
    }
    let order = normalized.iter().map(|(name, _)| name.clone()).collect();
    rewrite_blocks(project_path, article_path, &normalized, dry_run)?;
    Ok(BlockEditReport { article_path: article_path.to_string(), old_path: None, new_path: None, order, dry_run })
}

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

    #[test]
    fn new_and_renumber_blocks_are_collision_safe() {
        let tmp = tempfile::tempdir().unwrap();
        let article = tmp.path().join("docs/article");
        fs::create_dir_all(&article).unwrap();
        fs::write(article.join("01-first.md"), "first\n").unwrap();
        fs::write(article.join("03-third.md"), "third\n").unwrap();
        let created = new_block(tmp.path(), "docs/article", "second", Some("first"), 1, false).unwrap();
        assert_eq!(created.order, vec!["01-first.md", "02-second.md", "03-third.md"]);
        assert!(article.join("02-second.md").exists());
        let report = renumber_blocks(tmp.path(), "docs/article", 1, false).unwrap();
        assert_eq!(report.order, vec!["01-first.md", "02-second.md", "03-third.md"]);
    }

    #[test]
    fn move_block_dry_run_preserves_tree() {
        let tmp = tempfile::tempdir().unwrap();
        let article = tmp.path().join("docs/article");
        fs::create_dir_all(&article).unwrap();
        fs::write(article.join("01-first.md"), "first\n").unwrap();
        fs::write(article.join("02-second.md"), "second\n").unwrap();
        let report = move_block(tmp.path(), "docs/article", "first", Some("second"), 1, true).unwrap();
        assert!(report.dry_run);
        assert!(article.join("01-first.md").exists());
        assert!(article.join("02-second.md").exists());
    }
}
