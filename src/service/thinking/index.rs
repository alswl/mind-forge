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
    let previous_paths: HashSet<String> = idx.thinking.iter().flatten().map(|t| t.path.clone()).collect();

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
        let updated_at = file_mtime_rfc3339(&full)?;
        let entry = Thinking { path: path.clone(), article, updated_at };

        if previous_paths.contains(path) {
            kept_count += 1;
        } else {
            added.push(entry.clone());
        }
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
