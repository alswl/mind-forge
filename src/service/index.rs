use std::fs;
use std::io;
use std::path::Path;

use crate::error::{MfError, Result};
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

    let cleaned = deduplicate_top_level_keys(content);
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

/// Pre-process YAML content to remove duplicate top-level keys (keeping the last occurrence).
/// Returns the deduplicated content.
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

fn article_key(article: &crate::model::article::Article) -> std::result::Result<String, serde_yaml::Error> {
    let path = article.source_path.trim_start_matches("docs/");
    Ok(path.strip_suffix(".md").unwrap_or(path).trim_end_matches('/').to_string())
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

#[cfg(test)]
mod tests {
    use super::*;

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
