use std::fs;
use std::io;
use std::path::Path;

use crate::defaults;
use crate::error::{MfError, Result};
use crate::model::article::Article;
use crate::model::index::IndexFile;
use crate::service::util::{atomic_write, validate_schema_version};

const INDEX_FILENAME: &str = "mind-index.yaml";

/// Load `mind-index.yaml` from `project_root`.
///
/// Returns `IndexFile::create_default()` if the file does not exist.
/// Otherwise reads, parses YAML, and validates `schema_version`.
///
/// If the YAML contains duplicate top-level keys (which `serde_yaml` strictly
/// rejects), this function falls back to deduplicating the content (keeping the
/// last occurrence) and re-parsing.
pub fn load(project_root: &Path) -> Result<IndexFile> {
    let path = project_root.join(INDEX_FILENAME);
    let content = match fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            return Ok(IndexFile::create_default());
        }
        Err(e) => return Err(MfError::from(e)),
    };

    match load_from_str(&content, &path) {
        Ok(index) => {
            validate_schema_version(&index.schema_version, &path)?;
            Ok(index)
        }
        Err(e) => try_recover_duplicate_key(&content, &path, e),
    }
}

/// Attempt to recover from a duplicate-key parse error by deduplicating
/// top-level keys and re-parsing. If the error is not a duplicate-key error
/// (or recovery fails), returns the original error.
fn try_recover_duplicate_key(content: &str, path: &Path, err: MfError) -> Result<IndexFile> {
    let detail = match &err {
        MfError::ParseError { detail, .. } => detail.clone(),
        _ => return Err(err),
    };

    let key_name = match extract_duplicate_key(&detail) {
        Some(k) => k,
        None => return Err(err),
    };

    let cleaned = deduplicate_duplicate_keys_last_wins(content);
    match load_from_str(&cleaned, path) {
        Ok(index) => {
            let line = extract_error_line(&detail).map(|l| l.to_string()).unwrap_or_default();
            let hint = if !line.is_empty() {
                format!("key '{}' at line {}", key_name, line)
            } else {
                format!("key '{}'", key_name)
            };
            tracing::warn!(
                "duplicate top-level key in mind-index.yaml, using last occurrence ({}): {}",
                hint,
                path.display()
            );
            validate_schema_version(&index.schema_version, path)?;
            Ok(index)
        }
        Err(_) => Err(err),
    }
}

/// Internal helper: parse YAML content into an IndexFile.
fn load_from_str(content: &str, path: &Path) -> Result<IndexFile> {
    let index: IndexFile = serde_yaml::from_str(content).map_err(|e| MfError::ParseError {
        kind: "yaml".to_string(),
        path: path.to_path_buf(),
        detail: e.to_string(),
    })?;
    Ok(index)
}

/// Extract the duplicate key name from a serde_yaml duplicate-key error message.
///
/// serde_yaml 0.9 error format: `"duplicate entry with key \"<key>\" at line <N> column <M>"`
///
/// Note: This relies on the Display format of serde_yaml errors, which is not
/// a stable API. Tests verify the current format; if serde_yaml upgrades and
/// the format changes, these extractors will return None and the duplicate-key
/// recovery path degrades gracefully (still returns the original error).
pub fn extract_duplicate_key(error_detail: &str) -> Option<String> {
    let needle = "duplicate entry with key \"";
    let start = error_detail.find(needle)?;
    let rest = &error_detail[start + needle.len()..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

/// Extract the line number from a serde_yaml error message, if present.
pub fn extract_error_line(error_detail: &str) -> Option<usize> {
    let needle = " at line ";
    let start = error_detail.find(needle)?;
    let rest = &error_detail[start + needle.len()..];
    let end = rest.find(|c: char| !c.is_ascii_digit())?;
    rest[..end].parse::<usize>().ok()
}

/// Check if a line is a top-level YAML key declaration.
fn is_top_level_key(line: &str) -> bool {
    if line.is_empty() || line.starts_with(' ') || line.starts_with('\t') {
        return false;
    }
    if let Some(pos) = line.find(':') {
        let key = &line[..pos];
        !key.is_empty()
            && key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
            && key.chars().next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
    } else {
        false
    }
}

fn yaml_key_at_indent(line: &str) -> Option<(usize, String)> {
    if line.trim().is_empty() || line.trim_start().starts_with('#') || line.trim_start().starts_with('-') {
        return None;
    }
    let indent = line.len() - line.trim_start().len();
    let trimmed = line.trim_start();
    let pos = trimmed.find(':')?;
    let key = &trimmed[..pos];
    if key.is_empty()
        || !key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        || !key.chars().next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
    {
        return None;
    }
    Some((indent, key.to_string()))
}

/// Remove duplicate mapping keys at any indentation level, keeping the last
/// occurrence within the same parent mapping.
pub fn deduplicate_duplicate_keys_last_wins(content: &str) -> String {
    #[derive(Clone)]
    struct Entry {
        line: usize,
        indent: usize,
        key: String,
        parent: Vec<String>,
    }

    let lines: Vec<&str> = content.lines().collect();
    let mut entries: Vec<Entry> = Vec::new();
    let mut stack: Vec<(usize, String)> = Vec::new();

    for (line_idx, line) in lines.iter().enumerate() {
        let Some((indent, key)) = yaml_key_at_indent(line) else {
            continue;
        };
        while stack.last().is_some_and(|(stack_indent, _)| *stack_indent >= indent) {
            stack.pop();
        }
        let parent = stack.iter().map(|(_, key)| key.clone()).collect::<Vec<_>>();
        entries.push(Entry { line: line_idx, indent, key: key.clone(), parent });
        stack.push((indent, key));
    }

    let mut to_skip = std::collections::HashSet::new();
    let mut seen = std::collections::HashSet::new();
    for (entry_idx, entry) in entries.iter().enumerate().rev() {
        let group = (entry.indent, entry.parent.join("\u{1f}"), entry.key.clone());
        if seen.insert(group) {
            continue;
        }
        let end = entries
            .iter()
            .skip(entry_idx + 1)
            .find(|candidate| candidate.indent <= entry.indent)
            .map(|candidate| candidate.line)
            .unwrap_or(lines.len());
        for line_idx in entry.line..end {
            to_skip.insert(line_idx);
        }
    }

    let mut result = lines
        .iter()
        .enumerate()
        .filter(|(idx, _)| !to_skip.contains(idx))
        .map(|(_, line)| *line)
        .collect::<Vec<_>>()
        .join("\n");
    if content.ends_with('\n') && !result.ends_with('\n') {
        result.push('\n');
    }
    result
}

/// Pre-process YAML content to remove duplicate top-level keys (keeping the last occurrence).
/// Returns the deduplicated content.
#[cfg(test)]
pub fn deduplicate_top_level_keys(content: &str) -> String {
    let lines: Vec<&str> = content.lines().collect();
    let n = lines.len();

    // Find all top-level key line indices
    let top_indices: Vec<usize> =
        lines.iter().enumerate().filter(|(_, l)| is_top_level_key(l)).map(|(i, _)| i).collect();

    // Build (start, end, key_name) ranges for each top-level block
    let ranges: Vec<(usize, usize, &str)> = top_indices
        .iter()
        .enumerate()
        .map(|(j, &start)| {
            let end = top_indices.get(j + 1).copied().unwrap_or(n);
            let key_name = lines[start].split(':').next().unwrap_or("");
            (start, end, key_name)
        })
        .collect();

    // Mark line indices to skip — for duplicate keys, skip all occurrences
    // except the last one
    // Iterate in reverse so the first encounter of a key is the last occurrence
    // (which we keep). `seen.insert()` returns false for subsequent (earlier)
    // duplicates, marking them for removal.
    let mut to_skip = std::collections::HashSet::new();
    let mut seen = std::collections::HashSet::new();
    for &(start, end, key) in ranges.iter().rev() {
        if !seen.insert(key) {
            // Duplicate — skip this (earlier) occurrence's block
            for line_idx in start..end {
                to_skip.insert(line_idx);
            }
        }
    }

    // Rebuild content skipping duplicate blocks
    let mut result =
        lines.iter().enumerate().filter(|(i, _)| !to_skip.contains(i)).map(|(_, l)| *l).collect::<Vec<_>>().join("\n");
    // Preserve trailing newline the original had
    if content.ends_with('\n') && !result.ends_with('\n') {
        result.push('\n');
    }
    result
}

pub fn merge_duplicate_top_level_keys(content: &str) -> Result<String> {
    let lines: Vec<&str> = content.lines().collect();
    let n = lines.len();
    let top_indices: Vec<usize> =
        lines.iter().enumerate().filter(|(_, l)| is_top_level_key(l)).map(|(i, _)| i).collect();

    let mut order: Vec<String> = Vec::new();
    let mut merged = serde_yaml::Mapping::new();

    for (idx, &start) in top_indices.iter().enumerate() {
        let end = top_indices.get(idx + 1).copied().unwrap_or(n);
        let key = lines[start].split(':').next().unwrap_or("").to_string();
        if !order.contains(&key) {
            order.push(key.clone());
        }
        let block = lines[start..end].join("\n");
        let cleaned_block = deduplicate_duplicate_keys_last_wins(&block);
        let parsed: serde_yaml::Value = serde_yaml::from_str(&cleaned_block).map_err(|e| MfError::ParseError {
            kind: "yaml".to_string(),
            path: Path::new(INDEX_FILENAME).to_path_buf(),
            detail: e.to_string(),
        })?;
        let serde_yaml::Value::Mapping(block_map) = parsed else {
            continue;
        };
        let key_value = yaml_key(&key);
        let incoming = block_map.get(&key_value).cloned().unwrap_or(serde_yaml::Value::Null);
        match merged.get_mut(&key_value) {
            Some(existing) => merge_top_level_value(&key, existing, incoming),
            None => {
                merged.insert(key_value, incoming);
            }
        }
    }

    let mut ordered = serde_yaml::Mapping::new();
    for key in order {
        let key_value = yaml_key(&key);
        if let Some(value) = merged.remove(&key_value) {
            ordered.insert(key_value, value);
        }
    }
    serde_yaml::to_string(&serde_yaml::Value::Mapping(ordered))
        .map_err(|e| MfError::Internal(anyhow::anyhow!("serialize cleaned mind-index.yaml: {e}")))
}

fn merge_top_level_value(key: &str, existing: &mut serde_yaml::Value, incoming: serde_yaml::Value) {
    match (key, existing, incoming) {
        (
            "articles" | "sources" | "assets",
            serde_yaml::Value::Mapping(existing),
            serde_yaml::Value::Mapping(incoming),
        ) => {
            for (key, value) in incoming {
                existing.insert(key, value);
            }
        }
        (
            "terms" | "publish_records",
            serde_yaml::Value::Sequence(existing),
            serde_yaml::Value::Sequence(mut incoming),
        ) => {
            existing.append(&mut incoming);
        }
        (_, existing, incoming) => {
            *existing = incoming;
        }
    }
}

/// Atomically write `index` to `project_root/mind-index.yaml`.
pub fn save(project_root: &Path, index: &IndexFile) -> Result<()> {
    let path = project_root.join(INDEX_FILENAME);
    let yaml = serialize_mind_index(index)
        .map_err(|e| MfError::Internal(anyhow::anyhow!("serialize {}: {e}", path.display())))?;
    atomic_write(&path, &yaml)
}

pub fn serialize_mind_index(index: &IndexFile) -> std::result::Result<String, serde_yaml::Error> {
    let mut map = serde_yaml::Mapping::new();
    if let Some(extra) = &index.extra {
        for (key, value) in extra {
            if key != "schema_version" && key != "extra" {
                map.insert(yaml_key(key), json_to_yaml(value));
            }
        }
    }
    map.insert(yaml_key("schema"), serde_yaml::Value::String(index.schema_version.clone()));
    insert_mapping_collection(&mut map, "sources", index.sources.as_ref(), source_key)?;
    insert_mapping_collection(&mut map, "assets", index.assets.as_ref(), |asset| Ok(asset.name.clone()))?;
    insert_mapping_collection(&mut map, "articles", index.articles.as_ref(), article_key)?;
    insert_sequence_collection(&mut map, "terms", index.terms.as_ref())?;
    insert_sequence_collection(&mut map, "publish_records", index.publish_records.as_ref())?;
    serde_yaml::to_string(&serde_yaml::Value::Mapping(map))
}

fn yaml_key(key: &str) -> serde_yaml::Value {
    serde_yaml::Value::String(key.to_string())
}

fn insert_mapping_collection<T, F>(
    map: &mut serde_yaml::Mapping,
    field: &str,
    items: Option<&Vec<T>>,
    key_fn: F,
) -> std::result::Result<(), serde_yaml::Error>
where
    T: serde::Serialize,
    F: Fn(&T) -> std::result::Result<String, serde_yaml::Error>,
{
    let Some(items) = items else {
        return Ok(());
    };
    if items.is_empty() {
        return Ok(());
    }
    let mut collection = serde_yaml::Mapping::new();
    for item in items {
        collection.insert(yaml_key(&key_fn(item)?), serde_yaml::to_value(item)?);
    }
    map.insert(yaml_key(field), serde_yaml::Value::Mapping(collection));
    Ok(())
}

fn insert_sequence_collection<T: serde::Serialize>(
    map: &mut serde_yaml::Mapping,
    field: &str,
    items: Option<&Vec<T>>,
) -> std::result::Result<(), serde_yaml::Error> {
    let Some(items) = items else {
        return Ok(());
    };
    if items.is_empty() {
        return Ok(());
    }
    map.insert(yaml_key(field), serde_yaml::to_value(items)?);
    Ok(())
}

fn source_key(source: &crate::model::source::Source) -> std::result::Result<String, serde_yaml::Error> {
    Ok(source.path.clone().unwrap_or_else(|| source.name.clone()))
}

pub fn article_key(article: &crate::model::article::Article) -> std::result::Result<String, serde_yaml::Error> {
    // Primary key is source_path without .md extension.
    // For docs/ articles: "docs/2026-05-monthly"
    // For generated articles: "outputs/2026-05/2026-05-15"
    // For non-docs directory articles: "specs/quarterly"
    let path = article.source_path.strip_suffix(".md").unwrap_or(&article.source_path);
    Ok(path.trim_end_matches('/').to_string())
}

/// Derive the build artifact filename stem.
///
/// Strips `docs/` or `outputs/` prefix and `.md` extension.
/// Used by build output and publish artifact lookup to keep
/// `_build/<short-key>.<format>` consistent and avoid double-prefix
/// paths like `outputs/outputs/...`.
pub fn article_output_stem(source_path: &str) -> &str {
    let path = source_path.strip_suffix(".md").unwrap_or(source_path);
    path.strip_prefix(defaults::DOCS_PATH_PREFIX)
        .or_else(|| path.strip_prefix(defaults::BUILD_OUTPUT_PATH_PREFIX))
        .unwrap_or(path)
}

fn json_to_yaml(value: &serde_json::Value) -> serde_yaml::Value {
    match value {
        serde_json::Value::Null => serde_yaml::Value::Null,
        serde_json::Value::Bool(value) => serde_yaml::Value::Bool(*value),
        serde_json::Value::Number(value) => serde_yaml::to_value(value).unwrap_or(serde_yaml::Value::Null),
        serde_json::Value::String(value) => serde_yaml::Value::String(value.clone()),
        serde_json::Value::Array(values) => serde_yaml::Value::Sequence(values.iter().map(json_to_yaml).collect()),
        serde_json::Value::Object(values) => {
            let mut map = serde_yaml::Mapping::new();
            for (key, value) in values {
                map.insert(yaml_key(key), json_to_yaml(value));
            }
            serde_yaml::Value::Mapping(map)
        }
    }
}

/// A resolved article reference with its canonical key.
#[derive(Debug)]
pub struct ResolvedArticle<'a> {
    pub article: &'a Article,
    #[allow(dead_code)]
    pub canonical_key: String,
}

/// Resolve an article argument to its canonical index entry.
///
/// Resolution order:
/// 1. Exact canonical article key (path全名, e.g. `docs/2026-05-monthly`)
/// 2. Exact indexed source_path
/// 3. DOCS_DIR relative name (e.g. `2026-05-monthly` for docs/ articles)
/// 4. Template alias (`<template_name>/<slot_value>`, e.g. `daily_report/2026-05-15`)
/// 5. Unambiguous basename (strip first directory component + .md extension)
///
/// Display title is intentionally NOT used for resolution — it is
/// presentation-only and must not determine filesystem paths.
pub fn resolve_article<'a>(index: &'a IndexFile, article_arg: &str) -> Result<ResolvedArticle<'a>> {
    let articles = match index.articles.as_ref() {
        Some(a) if !a.is_empty() => a,
        _ => {
            return Err(MfError::not_found(
                format!("no articles indexed; article '{}' not found", article_arg),
                Some("run `mf article index` to index articles".to_string()),
            ));
        }
    };

    // 1. Exact canonical article key match (path全名)
    for a in articles {
        if let Ok(key) = article_key(a) {
            if key == article_arg {
                return Ok(ResolvedArticle { article: a, canonical_key: key });
            }
        }
    }

    // 2. Exact source_path match
    for a in articles {
        if a.source_path == article_arg {
            let key = article_key(a).unwrap_or_else(|_| article_arg.to_string());
            return Ok(ResolvedArticle { article: a, canonical_key: key });
        }
    }

    // 3. DOCS_DIR relative name match (e.g. "2026-05-monthly" for docs/2026-05-monthly)
    for a in articles {
        if let Ok(key) = article_key(a) {
            if key != article_arg && article_output_stem(&a.source_path) == article_arg {
                return Ok(ResolvedArticle { article: a, canonical_key: key });
            }
        }
    }

    // 4. Template alias match (<template_name>/<slot_value>)
    for a in articles {
        if let Some(ref to) = a.template_origin {
            let alias = format!("{}/{}", to.template_name, to.slot_value);
            if alias == article_arg {
                let key = article_key(a).unwrap_or_else(|_| article_arg.to_string());
                return Ok(ResolvedArticle { article: a, canonical_key: key });
            }
        }
    }

    // 5. Unambiguous short name / basename.
    // Try matching the article_arg against the source_path after stripping:
    // - any first directory component (e.g. docs/, notes/, specs/)
    // - .md extension
    // Also try constructing expected paths from the arg.
    let matches: Vec<&Article> = articles
        .iter()
        .filter(|a| {
            let sp = &a.source_path;
            sp == &format!("{}/{}", defaults::DOCS_DIR, article_arg)
                || sp == &format!("{}/{}.{}", defaults::DOCS_DIR, article_arg, defaults::MARKDOWN_EXTENSION)
                || sp
                    .split_once('/')
                    .map(|(_, rest)| rest.strip_suffix(".md").unwrap_or(rest))
                    .is_some_and(|stem| stem == article_arg)
        })
        .collect();

    match matches.len() {
        1 => {
            let a = matches[0];
            let key = article_key(a).unwrap_or_else(|_| article_arg.to_string());
            Ok(ResolvedArticle { article: a, canonical_key: key })
        }
        0 => Err(MfError::not_found(
            format!("article '{article_arg}' not found"),
            Some("run `mf article list` to see available articles".to_string()),
        )),
        _ => Err(MfError::usage(
            format!("ambiguous article '{}'; found {} matches", article_arg, matches.len()),
            Some("use the canonical article key from `mf article list`".to_string()),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::article::{Article, ArticleStatus, ArticleType, TemplateOrigin};

    fn make_article(source_path: &str, template_origin: Option<TemplateOrigin>) -> Article {
        Article {
            title: "test".to_string(),
            project: "test".to_string(),
            article_type: ArticleType::Blog,
            source_path: source_path.to_string(),
            status: ArticleStatus::Draft,
            created_at: "2026-05-15T00:00:00Z".to_string(),
            updated_at: "2026-05-15T00:00:00Z".to_string(),
            template_origin,
        }
    }

    #[test]
    fn article_key_uses_source_path_not_template_origin() {
        let article = make_article(
            "outputs/2026-05/2026-05-15.md",
            Some(TemplateOrigin { template_name: "daily_report".to_string(), slot_value: "2026-05-15".to_string() }),
        );
        let key = article_key(&article).unwrap();
        assert_eq!(
            key, "outputs/2026-05/2026-05-15",
            "primary key should be derived from source_path, not template_origin"
        );
    }

    #[test]
    fn article_key_returns_full_path_for_docs_article() {
        let article = make_article("docs/weekly-summary.md", None);
        let key = article_key(&article).unwrap();
        assert_eq!(key, "docs/weekly-summary", "key should be path全名: docs/weekly-summary");
    }

    #[test]
    fn article_key_strips_md_keeps_docs_prefix() {
        let article = make_article("docs/my-article.md", None);
        let key = article_key(&article).unwrap();
        assert_eq!(key, "docs/my-article");
    }

    #[test]
    fn article_key_generated_has_source_path_key() {
        let article = make_article(
            "outputs/2026-05/2026-05-15.md",
            Some(TemplateOrigin { template_name: "daily_report".to_string(), slot_value: "2026-05-15".to_string() }),
        );
        let key = article_key(&article).unwrap();
        assert_eq!(key, "outputs/2026-05/2026-05-15", "generated article primary key is its source_path without .md");
    }

    #[test]
    fn article_key_handles_directory_source_path() {
        let article = make_article("docs/2026-05-monthly", None);
        let key = article_key(&article).unwrap();
        assert_eq!(key, "docs/2026-05-monthly", "directory source_path should produce key without trailing slash");
    }

    #[test]
    fn article_key_handles_non_docs_directory_source_path() {
        let article = make_article("specs/quarterly", None);
        let key = article_key(&article).unwrap();
        assert_eq!(key, "specs/quarterly", "non-docs source_path should produce key verbatim");
    }

    // ── resolve_article tests ──

    fn make_index(articles: Vec<Article>) -> IndexFile {
        IndexFile {
            schema_version: "1".to_string(),
            articles: Some(articles),
            publish_records: None,
            sources: None,
            assets: None,
            terms: None,
            extra: None,
        }
    }

    #[test]
    fn resolve_by_exact_key_declared() {
        let index = make_index(vec![make_article("docs/2026-05-monthly", None)]);
        let resolved = resolve_article(&index, "docs/2026-05-monthly").unwrap();
        assert_eq!(resolved.canonical_key, "docs/2026-05-monthly");
        assert_eq!(resolved.article.source_path, "docs/2026-05-monthly");
    }

    #[test]
    fn resolve_by_docs_dir_relative_name() {
        let index = make_index(vec![make_article("docs/2026-05-monthly", None)]);
        let resolved = resolve_article(&index, "2026-05-monthly").unwrap();
        assert_eq!(resolved.canonical_key, "docs/2026-05-monthly");
        assert_eq!(resolved.article.source_path, "docs/2026-05-monthly");
    }

    #[test]
    fn resolve_by_template_alias() {
        let article = make_article(
            "outputs/2026-05/2026-05-15.md",
            Some(TemplateOrigin { template_name: "daily_report".to_string(), slot_value: "2026-05-15".to_string() }),
        );
        let index = make_index(vec![article]);
        let resolved = resolve_article(&index, "daily_report/2026-05-15").unwrap();
        assert_eq!(resolved.canonical_key, "outputs/2026-05/2026-05-15");
        assert_eq!(resolved.article.source_path, "outputs/2026-05/2026-05-15.md");
    }

    #[test]
    fn resolve_by_exact_key_generated() {
        let article = make_article(
            "outputs/2026-05/2026-05-15.md",
            Some(TemplateOrigin { template_name: "daily_report".to_string(), slot_value: "2026-05-15".to_string() }),
        );
        let index = make_index(vec![article]);
        let resolved = resolve_article(&index, "outputs/2026-05/2026-05-15").unwrap();
        assert_eq!(resolved.canonical_key, "outputs/2026-05/2026-05-15");
        assert_eq!(resolved.article.source_path, "outputs/2026-05/2026-05-15.md");
    }

    #[test]
    fn resolve_by_exact_source_path() {
        let index = make_index(vec![make_article("docs/my-article.md", None)]);
        let resolved = resolve_article(&index, "docs/my-article.md").unwrap();
        assert_eq!(resolved.canonical_key, "docs/my-article");
    }

    #[test]
    fn resolve_not_found() {
        let index = make_index(vec![make_article("docs/only-article.md", None)]);
        let err = resolve_article(&index, "missing-article").unwrap_err();
        assert!(matches!(err, MfError::NotFound { .. }));
    }

    #[test]
    fn resolve_title_not_used() {
        // The title "2026 05 monthly" is NOT a valid lookup key
        let mut article = make_article("docs/2026-05-monthly", None);
        article.title = "2026 05 monthly".to_string();
        let index = make_index(vec![article]);
        let err = resolve_article(&index, "2026 05 monthly").unwrap_err();
        assert!(matches!(err, MfError::NotFound { .. }));
    }

    #[test]
    fn load_returns_default_when_file_missing() {
        let dir = tempfile::tempdir().unwrap();
        let index = load(dir.path()).unwrap();
        assert_eq!(index, IndexFile::create_default());
    }

    #[test]
    fn load_returns_incompatible_schema_on_unsupported_version() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("mind-index.yaml"), "schema_version: \"99\"\n").unwrap();
        let err = load(dir.path()).unwrap_err();
        assert!(matches!(err, MfError::IncompatibleSchema { .. }));
        assert_eq!(err.kind(), "incompatible-schema");
    }

    #[test]
    fn load_returns_parse_error_on_corrupt_yaml() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("mind-index.yaml"), "not: : valid: yaml").unwrap();
        let err = load(dir.path()).unwrap_err();
        assert!(matches!(err, MfError::ParseError { .. }));
        assert_eq!(err.kind(), "parse-error");
    }

    #[test]
    fn save_returns_io_on_readonly_parent() {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let dir = tempfile::tempdir().unwrap();
            let mut perms = std::fs::metadata(dir.path()).unwrap().permissions();
            perms.set_mode(0o555);
            std::fs::set_permissions(dir.path(), perms).unwrap();
            let err = save(dir.path(), &IndexFile::create_default()).unwrap_err();
            assert!(matches!(err, MfError::Io(_)));
            // Restore so tempdir can clean up
            let mut perms = std::fs::metadata(dir.path()).unwrap().permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(dir.path(), perms).unwrap();
        }
    }

    #[test]
    fn dedup_removes_duplicate_top_level_keys() {
        let input = r#"schema: '1'
articles:
  first:
    title: First
    source_path: docs/first.md
articles:
  second:
    title: Second
    source_path: docs/second.md
"#;
        let result = deduplicate_top_level_keys(input);
        // Should produce parseable YAML
        let parsed: serde_yaml::Value = serde_yaml::from_str(&result).unwrap();
        let map = parsed.as_mapping().unwrap();
        let articles_count = map.keys().filter(|k| k.as_str() == Some("articles")).count();
        assert_eq!(articles_count, 1, "articles key should appear exactly once: {result}");
        // Verify the value is the LAST occurrence
        let articles = map.get(serde_yaml::Value::String("articles".to_string())).unwrap();
        let articles_map = articles.as_mapping().unwrap();
        assert!(
            articles_map.contains_key(serde_yaml::Value::String("second".to_string())),
            "should keep last occurrence: {result}"
        );
    }

    #[test]
    fn load_succeeds_with_duplicate_keys() {
        let dir = tempfile::tempdir().unwrap();
        let content = r#"schema: '1'
articles:
  first:
    title: First
    source_path: docs/first.md
articles:
  second:
    title: Second
    source_path: docs/second.md
"#;
        std::fs::write(dir.path().join("mind-index.yaml"), content).unwrap();
        let index = load(dir.path()).unwrap();
        let articles = index.articles.unwrap();
        // After dedup, only the last occurrence's articles should survive
        assert_eq!(articles.len(), 1, "should load with 1 article (last occurrence)");
        assert_eq!(articles[0].title, "Second");
    }

    #[test]
    fn load_succeeds_with_nested_duplicate_keys() {
        let dir = tempfile::tempdir().unwrap();
        let content = r#"schema: '1'
articles:
  first:
    title: Old
    title: New
    source_path: docs/first.md
"#;
        std::fs::write(dir.path().join("mind-index.yaml"), content).unwrap();
        let index = load(dir.path()).unwrap();
        let articles = index.articles.unwrap();
        assert_eq!(articles.len(), 1);
        assert_eq!(articles[0].title, "New");
    }

    #[test]
    fn merge_duplicate_top_level_keys_combines_article_mappings() {
        let input = r#"schema: '1'
articles:
  first:
    title: First
    source_path: docs/first.md
articles:
  second:
    title: Second
    source_path: docs/second.md
"#;
        let merged = merge_duplicate_top_level_keys(input).unwrap();
        let index: IndexFile = serde_yaml::from_str(&merged).unwrap();
        let articles = index.articles.unwrap();
        assert_eq!(articles.len(), 2, "merged index should keep both article entries: {merged}");
        assert!(articles.iter().any(|a| a.title == "First"));
        assert!(articles.iter().any(|a| a.title == "Second"));
    }

    #[test]
    fn extract_duplicate_key_from_error() {
        let err_msg = "duplicate entry with key \"articles\" at line 5 column 3";
        let key = extract_duplicate_key(err_msg);
        assert_eq!(key, Some("articles".to_string()));
    }

    #[test]
    fn extract_duplicate_key_returns_none_for_non_duplicate_error() {
        let err_msg = "expected a mapping at line 1 column 1";
        let key = extract_duplicate_key(err_msg);
        assert!(key.is_none());
    }

    #[test]
    fn extract_error_line_from_duplicate_key_error() {
        let err_msg = "duplicate entry with key \"articles\" at line 5 column 3";
        let line = extract_error_line(err_msg);
        assert_eq!(line, Some(5));
    }

    #[test]
    fn extract_error_line_returns_none_for_no_line() {
        let err_msg = "expected a mapping";
        let line = extract_error_line(err_msg);
        assert!(line.is_none());
    }
}
