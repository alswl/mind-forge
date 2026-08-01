use std::fs;
use std::path::Path;

use crate::defaults;
use crate::error::{MfError, Result};
use crate::model::article::LintIssue;
use crate::service::config as config_svc;
use crate::service::util::markdown;

/// Lint articles in the project: check filenames and content quality.
/// When `fix` is true, auto-fix fixable issues.
pub fn lint_articles(project_path: &Path, fix: bool) -> Result<Vec<LintIssue>> {
    let layout = config_svc::effective_layout(project_path)?;
    let docs_dir = project_path.join(&layout.articles);
    if !docs_dir.exists() {
        return Ok(Vec::new());
    }

    let mut issues = Vec::new();

    let entries = fs::read_dir(&docs_dir).map_err(MfError::Io)?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some(defaults::MARKDOWN_EXTENSION) {
            continue;
        }
        if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
            let rel_path = format!("{}/{}.{}", layout.articles, stem, defaults::MARKDOWN_EXTENSION);

            // Check content before filename (filename may rename the file)
            check_content(&mut issues, &path, &rel_path)?;
            // Check filename convention: lowercase with hyphens only
            check_filename(&mut issues, stem, &rel_path, fix, &path)?;
        }
    }

    Ok(issues)
}

fn check_filename(issues: &mut Vec<LintIssue>, stem: &str, rel_path: &str, fix: bool, full_path: &Path) -> Result<()> {
    if !stem.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-') {
        let expected = stem
            .to_lowercase()
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '-' })
            .collect::<String>();

        issues.push(LintIssue {
            severity: "warning".to_string(),
            kind: "filename_convention".to_string(),
            message: format!("filename '{}' should be lowercase with hyphens (suggest: '{}.md')", stem, expected),
            path: rel_path.to_string(),
            fixable: true,
        });

        if fix {
            let new_path = full_path.with_file_name(format!("{}.md", expected));
            if !new_path.exists() {
                fs::rename(full_path, &new_path).map_err(MfError::Io)?;
            }
        }
    }
    Ok(())
}

fn check_content(issues: &mut Vec<LintIssue>, full_path: &Path, rel_path: &str) -> Result<()> {
    let content = fs::read_to_string(full_path).map_err(MfError::Io)?;

    if content.trim().is_empty() {
        issues.push(LintIssue {
            severity: "error".to_string(),
            kind: "empty_file".to_string(),
            message: "article file is empty".to_string(),
            path: rel_path.to_string(),
            fixable: false,
        });
    }

    // mind-forge-visibility validation (spec 073, FR-006/FR-007). `lint_articles`
    // only scans top-level docs/*.md files, so every file reaching this check is
    // a single-file article's own (and only) block — i.e. its title block. A
    // directory article's block files (docs/<article>/NN-*.md) are not walked
    // here; `mf build` is authoritative for validating those (it fails the
    // build before writing an artifact on the same conditions).
    match markdown::block_visibility(&content) {
        Ok(markdown::Visibility::Private) => {
            issues.push(LintIssue {
                severity: "error".to_string(),
                kind: "mind_forge_private_title_block".to_string(),
                message:
                    "the article's only block carries the H1 title and cannot be marked mind-forge-visibility: private"
                        .to_string(),
                path: rel_path.to_string(),
                fixable: false,
            });
        }
        Ok(markdown::Visibility::Public) => {}
        Err(e) => {
            issues.push(LintIssue {
                severity: "error".to_string(),
                kind: "mind_forge_visibility_invalid".to_string(),
                message: e.to_string(),
                path: rel_path.to_string(),
                fixable: false,
            });
        }
    }

    Ok(())
}
