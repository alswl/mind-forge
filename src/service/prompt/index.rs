use std::collections::HashSet;
use std::fs;
use std::path::Path;

use crate::defaults;
use crate::error::{MfError, Result};
use crate::model::prompt::{Prompt, PromptIndexReport, PromptMode};
use crate::service::index;
use crate::service::util::{self, file_mtime_rfc3339};

/// Reconcile the `prompts:` projection with `prompts/*.md` on disk.
///
/// Every present file is re-parsed from its current content (the Markdown
/// file is the source of truth, FR-002): `added` covers files newly
/// discovered since the last reconcile, `removed` covers projection entries
/// whose backing file is gone, and `kept_count` covers files present both
/// before and after. `--dry-run` reports the plan without writing.
pub fn reconcile(project_path: &Path, dry_run: bool) -> Result<PromptIndexReport> {
    let mut idx = index::load(project_path)?;
    let previous_paths: HashSet<String> = idx.prompts.iter().flatten().map(|p| p.path.clone()).collect();

    let prompts_dir = project_path.join(defaults::PROMPTS_DIR);
    let disk_paths = util::scan_md_paths(project_path, &prompts_dir)?;

    let mut new_prompts = Vec::with_capacity(disk_paths.len());
    let mut added = Vec::new();
    let mut kept_count: u64 = 0;

    for path in &disk_paths {
        let full = project_path.join(path);
        let content = fs::read_to_string(&full).map_err(MfError::Io)?;
        let (article, mode) = parse_prompt_frontmatter(&content);
        let updated_at = file_mtime_rfc3339(&full)?;
        let prompt = Prompt { path: path.clone(), article: article.unwrap_or_default(), mode, updated_at };

        if previous_paths.contains(path) {
            kept_count += 1;
        } else {
            added.push(prompt.clone());
        }
        new_prompts.push(prompt);
    }

    let disk_path_set: HashSet<&String> = disk_paths.iter().collect();
    let removed: Vec<Prompt> =
        idx.prompts.clone().into_iter().flatten().filter(|p| !disk_path_set.contains(&p.path)).collect();

    if !dry_run {
        new_prompts.sort_by(|a, b| a.path.cmp(&b.path));
        idx.prompts = Some(new_prompts);
        index::save(project_path, &idx)?;
    }

    Ok(PromptIndexReport { added, removed, kept_count, dry_run })
}

/// Parse `article:` and `mode:` from a prompt file's YAML frontmatter.
///
/// Tolerant by design (FR-002 edge cases): missing frontmatter, a missing
/// closing delimiter, or an unrecognized `mode:` value never error — they
/// simply leave the corresponding field unset. The file is still projected
/// by its filename; an unset `article` resolves to `orphan` downstream.
fn parse_prompt_frontmatter(content: &str) -> (Option<String>, Option<PromptMode>) {
    let mut lines = content.lines();
    if lines.next() != Some("---") {
        return (None, None);
    }

    let mut article = None;
    let mut mode = None;
    for line in lines {
        if line.trim() == "---" {
            break;
        }
        if let Some(rest) = line.strip_prefix("article:") {
            let value = rest.trim().trim_matches(|c| c == '"' || c == '\'');
            if !value.is_empty() {
                article = Some(value.to_string());
            }
        } else if let Some(rest) = line.strip_prefix("mode:") {
            let value = rest.trim().trim_matches(|c| c == '"' || c == '\'');
            mode = PromptMode::parse(value);
        }
    }
    (article, mode)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_article_and_mode() {
        let content = "---\narticle: docs/my-post.md\nmode: research\n---\n\nBody.\n";
        let (article, mode) = parse_prompt_frontmatter(content);
        assert_eq!(article.as_deref(), Some("docs/my-post.md"));
        assert_eq!(mode, Some(PromptMode::Research));
    }

    #[test]
    fn tolerates_missing_frontmatter() {
        let (article, mode) = parse_prompt_frontmatter("Just prose, no frontmatter.\n");
        assert_eq!(article, None);
        assert_eq!(mode, None);
    }

    #[test]
    fn tolerates_unclosed_frontmatter() {
        let content = "---\narticle: docs/my-post.md\n\nno closing delimiter\n";
        let (article, _mode) = parse_prompt_frontmatter(content);
        assert_eq!(article.as_deref(), Some("docs/my-post.md"));
    }

    #[test]
    fn tolerates_unknown_mode_value() {
        let content = "---\narticle: docs/my-post.md\nmode: not-a-real-mode\n---\n";
        let (article, mode) = parse_prompt_frontmatter(content);
        assert_eq!(article.as_deref(), Some("docs/my-post.md"));
        assert_eq!(mode, None);
    }
}
