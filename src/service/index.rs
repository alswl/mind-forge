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
pub fn load(project_root: &Path) -> Result<IndexFile> {
    let path = project_root.join(INDEX_FILENAME);
    let content = match fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            return Ok(IndexFile::create_default());
        }
        Err(e) => return Err(MfError::from(e)),
    };
    let index: IndexFile = serde_yaml::from_str(&content).map_err(|e| MfError::ParseError {
        kind: "yaml".to_string(),
        path: path.clone(),
        detail: e.to_string(),
    })?;
    validate_schema_version(&index.schema_version, &path)?;
    Ok(index)
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
}
