use std::collections::HashSet;
use std::path::Path;

use crate::defaults;
use crate::error::Result;
use crate::model::thinking::{Thinking, ThinkingIndexReport};
use crate::service::index;
use crate::service::util::{self, file_mtime_rfc3339};

/// Reconcile the `thinking:` projection with `thinking/*.md` on disk.
///
/// Every present file is re-associated by key alignment against the current
/// `articles` set (no thinking-specific frontmatter is required, per
/// research D4): `added` covers files newly discovered, `removed` covers
/// projection entries whose backing file is gone, `kept_count` covers files
/// present both before and after. `--dry-run` reports the plan without
/// writing. Tolerates a missing `thinking/` directory (empty result).
pub fn reconcile(project_path: &Path, dry_run: bool) -> Result<ThinkingIndexReport> {
    let mut idx = index::load(project_path)?;
    // Same as `prompt::index::reconcile` (spec 079 FR-011): a kept entry
    // reuses its own previous `updated_at` rather than the file's mtime, which
    // `git checkout`/a fresh worktree bumps regardless of content.
    let previous: std::collections::HashMap<String, String> =
        idx.thinking.iter().flatten().map(|t| (t.path.clone(), t.updated_at.clone())).collect();

    let article_by_key: std::collections::HashMap<String, String> = idx
        .articles
        .iter()
        .flatten()
        .map(|a| (index::article_output_stem(&a.article_path).to_string(), a.article_path.clone()))
        .collect();

    let thinking_dir = project_path.join(defaults::THINKING_DIR);
    let disk_paths = util::scan_md_paths(project_path, &thinking_dir)?;

    let mut new_entries = Vec::with_capacity(disk_paths.len());
    let mut added = Vec::new();
    let mut kept_count: u64 = 0;

    for path in &disk_paths {
        let full = project_path.join(path);
        let key = index::derive_store_key(path);
        let article = article_by_key.get(&key).cloned().unwrap_or_default();

        let entry = if let Some(existing_updated_at) = previous.get(path) {
            kept_count += 1;
            Thinking { path: path.clone(), article, updated_at: existing_updated_at.clone() }
        } else {
            let updated_at = file_mtime_rfc3339(&full)?;
            let entry = Thinking { path: path.clone(), article, updated_at };
            added.push(entry.clone());
            entry
        };
        new_entries.push(entry);
    }

    let disk_path_set: HashSet<&String> = disk_paths.iter().collect();
    let removed: Vec<Thinking> =
        idx.thinking.clone().into_iter().flatten().filter(|t| !disk_path_set.contains(&t.path)).collect();

    if !dry_run {
        new_entries.sort_by(|a, b| a.path.cmp(&b.path));
        idx.thinking = Some(new_entries);
        index::save(project_path, &idx)?;
    }

    Ok(ThinkingIndexReport { added, removed, kept_count, dry_run })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::thinking::Thinking;

    // ── spec 079 US4 (#53): same guarantee as prompt/index.rs ──────────────

    #[test]
    fn reconcile_keeps_existing_updated_at_even_after_mtime_changes() {
        let dir = tempfile::tempdir().unwrap();
        let project_path = dir.path();
        std::fs::create_dir_all(project_path.join("thinking")).unwrap();
        std::fs::write(project_path.join("thinking/notes.md"), "Some thinking notes.\n").unwrap();

        let old_updated_at = "2020-01-01T00:00:00Z".to_string();
        let idx = crate::model::index::IndexFile {
            thinking: Some(vec![Thinking {
                path: "thinking/notes.md".to_string(),
                article: String::new(),
                updated_at: old_updated_at.clone(),
            }]),
            ..crate::model::index::IndexFile::create_default()
        };
        index::save(project_path, &idx).unwrap();

        let file = project_path.join("thinking/notes.md");
        let now = std::time::SystemTime::now() + std::time::Duration::from_secs(3600);
        std::fs::File::open(&file).unwrap().set_modified(now).unwrap();

        reconcile(project_path, false).unwrap();

        let reloaded = index::load(project_path).unwrap();
        let kept = reloaded.thinking.unwrap();
        assert_eq!(kept.len(), 1);
        assert_eq!(
            kept[0].updated_at, old_updated_at,
            "a kept entry's updated_at must not be refreshed from file mtime (before fix: it was)"
        );
    }
}
