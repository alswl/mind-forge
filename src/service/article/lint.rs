use std::fs;
use std::path::Path;

use crate::defaults;
use crate::error::{MfError, Result};
use crate::model::article::LintIssue;
use crate::service::config as config_svc;

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

    Ok(())
}
