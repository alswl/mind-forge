use std::collections::HashMap;

use serde::{Deserialize, Deserializer, Serialize};

use crate::defaults;

use super::article::Article;
use super::asset::Asset;
use super::source::Source;
use super::term::Term;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PublishStatus {
    Draft,
    Published,
    Archived,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PublishRecord {
    pub path: String,
    pub target_name: String,
    pub status: PublishStatus,
    pub target_url: Option<String>,
    pub published_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct IndexFile {
    #[serde(alias = "schema")]
    pub schema_version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sources: Option<Vec<Source>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assets: Option<Vec<Asset>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub articles: Option<Vec<Article>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub terms: Option<Vec<Term>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub publish_records: Option<Vec<PublishRecord>>,
    /// Extra top-level fields preserved from original YAML (e.g. `project`,
    /// `updated`, `docs`, `prompts`). Populated on deserialization.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extra: Option<HashMap<String, serde_json::Value>>,
}

impl IndexFile {
    pub fn create_default() -> Self {
        Self {
            schema_version: defaults::SCHEMA_VERSION.to_string(),
            sources: None,
            assets: None,
            articles: None,
            terms: None,
            publish_records: None,
            extra: None,
        }
    }
}

/// Custom deserializer that accepts both list and dictionary forms for
/// `sources`, `assets`, `articles`, and preserves extra unknown fields.
impl<'de> Deserialize<'de> for IndexFile {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        use serde::de::Error;

        // Parse as raw YAML value first
        let raw = serde_yaml::Value::deserialize(deserializer)?;
        let map = match raw {
            serde_yaml::Value::Mapping(m) => m,
            _ => return Err(D::Error::custom("expected a YAML mapping for IndexFile")),
        };

        // Convert dictionary-valued source entries to list, adding defaults.
        fn dict_to_sources_list(value: &serde_yaml::Value, file_entries: bool) -> serde_yaml::Value {
            match value {
                serde_yaml::Value::Mapping(m) => {
                    let items: Vec<serde_yaml::Value> = m
                        .iter()
                        .map(|(k, v)| {
                            let mut entry = match v {
                                serde_yaml::Value::Mapping(e) => e.clone(),
                                serde_yaml::Value::Null => serde_yaml::Mapping::new(),
                                _ => serde_yaml::Mapping::new(),
                            };
                            insert_default_str(&mut entry, "name", key_string(k));
                            let key = key_string(k);
                            if (file_entries || key.contains('/') || key.starts_with('.'))
                                && !entry.contains_key(yaml_key("path"))
                            {
                                entry.insert(yaml_key("path"), k.clone());
                            }
                            if file_entries && entry.get(yaml_key("type")).and_then(|v| v.as_str()) == Some("markdown")
                            {
                                entry.insert(yaml_key("type"), serde_yaml::Value::String("file".to_string()));
                            }
                            default_str(&mut entry, "added_at", "");
                            default_str(&mut entry, "updated_at", "");
                            serde_yaml::Value::Mapping(entry)
                        })
                        .collect();
                    serde_yaml::Value::Sequence(items)
                }
                serde_yaml::Value::Sequence(_) => value.clone(),
                serde_yaml::Value::Null => serde_yaml::Value::Sequence(vec![]),
                _ => value.clone(),
            }
        }

        // Convert dictionary-valued asset entries to list, adding defaults.
        fn dict_to_assets_list(value: &serde_yaml::Value) -> serde_yaml::Value {
            match value {
                serde_yaml::Value::Mapping(m) => {
                    let items: Vec<serde_yaml::Value> = m
                        .iter()
                        .map(|(k, v)| {
                            let mut entry = match v {
                                serde_yaml::Value::Mapping(e) => e.clone(),
                                serde_yaml::Value::Null => serde_yaml::Mapping::new(),
                                _ => serde_yaml::Mapping::new(),
                            };
                            insert_default_str(&mut entry, "name", key_string(k));
                            default_str(&mut entry, "added_at", "");
                            default_str(&mut entry, "hash", "");
                            default_u64(&mut entry, "size", 0);
                            serde_yaml::Value::Mapping(entry)
                        })
                        .collect();
                    serde_yaml::Value::Sequence(items)
                }
                serde_yaml::Value::Sequence(_) => value.clone(),
                serde_yaml::Value::Null => serde_yaml::Value::Sequence(vec![]),
                _ => value.clone(),
            }
        }

        // Convert dictionary-valued article entries to list, adding defaults.
        fn dict_articles_to_list(value: &serde_yaml::Value) -> serde_yaml::Value {
            match value {
                serde_yaml::Value::Mapping(m) => {
                    let items: Vec<serde_yaml::Value> = m
                        .iter()
                        .map(|(k, v)| {
                            let mut entry = match v {
                                serde_yaml::Value::Mapping(e) => {
                                    let mut e = e.clone();
                                    if !e.contains_key(serde_yaml::Value::String("title".to_string())) {
                                        e.insert(
                                            serde_yaml::Value::String("title".to_string()),
                                            serde_yaml::Value::String(k.as_str().unwrap_or("").replace('-', " ")),
                                        );
                                    }
                                    e
                                }
                                serde_yaml::Value::Null => serde_yaml::Mapping::new(),
                                _ => serde_yaml::Mapping::new(),
                            };
                            entry.entry(yaml_key("title")).or_insert_with(|| serde_yaml::Value::String(key_string(k)));
                            default_str(&mut entry, "project", "");
                            // Infer source_path from dict key when absent
                            let inferred_source = format!("docs/{}", key_string(k));
                            let sp_key = yaml_key("source_path");
                            if !entry.contains_key(&sp_key) {
                                entry.insert(sp_key, serde_yaml::Value::String(inferred_source));
                            }
                            default_str(&mut entry, "created_at", "");
                            default_str(&mut entry, "updated_at", "");
                            default_str(&mut entry, "type", "blog");
                            default_str(&mut entry, "status", "draft");
                            serde_yaml::Value::Mapping(entry)
                        })
                        .collect();
                    serde_yaml::Value::Sequence(items)
                }
                serde_yaml::Value::Sequence(_) => value.clone(),
                serde_yaml::Value::Null => serde_yaml::Value::Sequence(vec![]),
                _ => value.clone(),
            }
        }

        // Normalize fields
        let known_keys = ["schema_version", "schema", "sources", "assets", "articles", "terms", "publish_records"];

        let mut extra = serde_yaml::Mapping::new();
        let mut normalized = serde_yaml::Mapping::new();

        for (k, v) in &map {
            let key_str = k.as_str().unwrap_or("");
            match key_str {
                "sources" => {
                    merge_sequence_field(&mut normalized, "sources", dict_to_sources_list(v, false));
                }
                "files" => {
                    merge_sequence_field(&mut normalized, "sources", dict_to_sources_list(v, true));
                }
                "assets" => {
                    normalized.insert(yaml_key("assets"), dict_to_assets_list(v));
                }
                "articles" => {
                    normalized.insert(yaml_key("articles"), dict_articles_to_list(v));
                }
                "schema_version" | "schema" => {
                    normalized.insert(yaml_key("schema_version"), v.clone());
                }
                "terms" | "publish_records" => {
                    normalized.insert(k.clone(), v.clone());
                }
                _ => {
                    if !known_keys.contains(&key_str) {
                        extra.insert(k.clone(), v.clone());
                    } else {
                        normalized.insert(k.clone(), v.clone());
                    }
                }
            }
        }

        if !extra.is_empty() {
            normalized.insert(yaml_key("extra"), serde_yaml::Value::Mapping(extra));
        }

        // Deserialize normalized value into IndexFile
        let file = IndexFile::deserialize_value(serde_yaml::Value::Mapping(normalized)).map_err(D::Error::custom)?;
        Ok(file)
    }
}

impl IndexFile {
    /// Deserialize from a pre-normalized YAML value.
    fn deserialize_value(value: serde_yaml::Value) -> Result<Self, String> {
        // Manually extract fields from the mapping
        let map = match &value {
            serde_yaml::Value::Mapping(m) => m,
            _ => return Err("expected mapping".to_string()),
        };

        let get_string =
            |key: &str| -> Option<String> { map.get(yaml_key(key)).and_then(|v| v.as_str().map(|s| s.to_string())) };

        let schema_version = get_string("schema_version").unwrap_or_else(|| defaults::SCHEMA_VERSION.to_string());

        let vec_from_field = |key: &str| -> Option<Vec<serde_yaml::Value>> {
            map.get(yaml_key(key)).and_then(|v| v.as_sequence().cloned())
        };

        let sources = parse_vec_field::<Source>(vec_from_field("sources"), "sources")?;
        let assets = parse_vec_field::<Asset>(vec_from_field("assets"), "assets")?;
        let articles = parse_vec_field::<Article>(vec_from_field("articles"), "articles")?;
        let terms = parse_vec_field::<Term>(vec_from_field("terms"), "terms")?;
        let publish_records = parse_vec_field::<PublishRecord>(vec_from_field("publish_records"), "publish_records")?;

        let extra = map.get(yaml_key("extra")).and_then(|v| v.as_mapping()).map(|m| {
            m.iter()
                .map(|(k, v)| {
                    let key = k.as_str().map(|s| s.to_string()).unwrap_or_default();
                    (key, yaml_to_json(v))
                })
                .collect::<HashMap<_, _>>()
        });

        Ok(IndexFile { schema_version, sources, assets, articles, terms, publish_records, extra })
    }
}

fn yaml_key(key: &str) -> serde_yaml::Value {
    serde_yaml::Value::String(key.to_string())
}

fn key_string(key: &serde_yaml::Value) -> String {
    key.as_str().unwrap_or("").to_string()
}

fn default_str(map: &mut serde_yaml::Mapping, key: &str, value: &str) {
    insert_default_str(map, key, value.to_string());
}

fn insert_default_str(map: &mut serde_yaml::Mapping, key: &str, value: String) {
    let key = yaml_key(key);
    if !map.contains_key(&key) {
        map.insert(key, serde_yaml::Value::String(value));
    }
}

fn default_u64(map: &mut serde_yaml::Mapping, key: &str, value: u64) {
    let key = yaml_key(key);
    if !map.contains_key(&key) {
        map.insert(key, serde_yaml::Value::Number(serde_yaml::Number::from(value)));
    }
}

fn merge_sequence_field(map: &mut serde_yaml::Mapping, field: &str, value: serde_yaml::Value) {
    let key = yaml_key(field);
    let serde_yaml::Value::Sequence(mut incoming) = value else {
        map.insert(key, value);
        return;
    };
    match map.get_mut(&key) {
        Some(serde_yaml::Value::Sequence(existing)) => existing.append(&mut incoming),
        _ => {
            map.insert(key, serde_yaml::Value::Sequence(incoming));
        }
    }
}

fn parse_vec_field<T>(items: Option<Vec<serde_yaml::Value>>, field: &str) -> Result<Option<Vec<T>>, String>
where
    T: serde::de::DeserializeOwned,
{
    let Some(items) = items else {
        return Ok(None);
    };
    items
        .iter()
        .enumerate()
        .map(|(idx, item)| {
            let parsed = serde_yaml::from_value(item.clone()).or_else(|_| match serde_yaml::to_string(item) {
                Ok(text) => serde_yaml::from_str(&text),
                Err(err) => Err(err),
            });
            parsed.map_err(|e| format!("invalid {field}[{idx}]: {e}"))
        })
        .collect::<Result<Vec<_>, _>>()
        .map(Some)
}

fn yaml_to_json(v: &serde_yaml::Value) -> serde_json::Value {
    match v {
        serde_yaml::Value::Null => serde_json::Value::Null,
        serde_yaml::Value::Bool(b) => serde_json::Value::Bool(*b),
        serde_yaml::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                serde_json::Value::Number(i.into())
            } else if let Some(f) = n.as_f64() {
                serde_json::Value::Number(serde_json::Number::from_f64(f).unwrap_or(0.into()))
            } else {
                serde_json::Value::Null
            }
        }
        serde_yaml::Value::String(s) => serde_json::Value::String(s.clone()),
        serde_yaml::Value::Sequence(seq) => serde_json::Value::Array(seq.iter().map(yaml_to_json).collect()),
        serde_yaml::Value::Mapping(m) => {
            let mut obj = serde_json::Map::new();
            for (k, v) in m {
                let key = k.as_str().map(|s| s.to_string()).unwrap_or_default();
                obj.insert(key, yaml_to_json(v));
            }
            serde_json::Value::Object(obj)
        }
        _ => serde_json::Value::Null,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_dictionary_sources() {
        let yaml = r#"
schema_version: '1'
sources:
  docs/api-spec.md:
    name: API Spec
    type: file
    path: docs/api-spec.md
  docs/arch.md:
    name: Architecture
    type: file
    path: docs/arch.md
"#;
        let index: IndexFile = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(index.schema_version, "1");
        let sources = index.sources.unwrap();
        assert_eq!(sources.len(), 2);
    }

    #[test]
    fn test_deserialize_files_dictionary_as_sources() {
        let yaml = r#"
schema_version: '1'
files:
  docs/api-spec.md:
    type: markdown
    description: API reference
"#;
        let index: IndexFile = serde_yaml::from_str(yaml).unwrap();
        let sources = index.sources.unwrap();
        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0].name, "docs/api-spec.md");
        assert_eq!(sources[0].path.as_deref(), Some("docs/api-spec.md"));
    }

    #[test]
    fn test_invalid_known_index_entry_returns_error() {
        let yaml = r#"
schema_version: '1'
sources:
  docs/api-spec.md:
    type: not_a_file_kind
"#;
        let err = serde_yaml::from_str::<IndexFile>(yaml).unwrap_err().to_string();
        assert!(err.contains("invalid sources[0]"), "{err}");
    }

    #[test]
    fn test_deserialize_dictionary_assets() {
        let yaml = r#"
schema_version: '1'
assets:
  logo.png:
    name: logo.png
    type: image
    path: assets/logo.png
  banner.jpg:
    name: banner.jpg
    type: image
    path: assets/banner.jpg
"#;
        let index: IndexFile = serde_yaml::from_str(yaml).unwrap();
        let assets = index.assets.unwrap();
        assert_eq!(assets.len(), 2);
    }

    #[test]
    fn test_deserialize_dictionary_articles() {
        let yaml = r#"
schema_version: '1'
articles:
  getting-started:
    title: Getting Started
    source_path: docs/getting-started.md
  advanced-guide:
    title: Advanced Guide
    source_path: docs/advanced-guide.md
"#;
        let index: IndexFile = serde_yaml::from_str(yaml).unwrap();
        let articles = index.articles.unwrap();
        assert_eq!(articles.len(), 2);
    }

    #[test]
    fn test_deserialize_null_entries() {
        let yaml = r#"
schema_version: '1'
sources:
  null-source:
articles:
  null-article:
"#;
        let index: IndexFile = serde_yaml::from_str(yaml).unwrap();
        // Null entries should normalize to empty entries with name/key
        let sources = index.sources.unwrap();
        assert_eq!(sources.len(), 1, "null source should produce one entry");
        let articles = index.articles.unwrap();
        assert_eq!(articles.len(), 1, "null article should produce one entry");
    }

    #[test]
    fn test_preserve_extra_fields() {
        let yaml = r#"
schema_version: '1'
project: my-project
updated: 2026-05-01
docs: active
prompts: active
"#;
        let index: IndexFile = serde_yaml::from_str(yaml).unwrap();
        let extra = index.extra.expect("extra should be populated");
        assert!(extra.contains_key("project"), "extra should contain project");
        assert!(extra.contains_key("updated"), "extra should contain updated");
        assert!(extra.contains_key("docs"), "extra should contain docs");
        assert!(extra.contains_key("prompts"), "extra should contain prompts");
    }

    #[test]
    fn test_schema_alias() {
        let yaml = r#"
schema: '1'
"#;
        let index: IndexFile = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(index.schema_version, "1");
    }

    #[test]
    fn test_list_format_still_works() {
        let yaml = r#"
schema_version: '1'
sources:
  - name: Source 1
    type: file
    path: docs/source1.md
"#;
        let index: IndexFile = serde_yaml::from_str(yaml).unwrap();
        let sources = index.sources.unwrap();
        assert_eq!(sources.len(), 1);
    }

    #[test]
    fn test_parse_list_format_assets_with_type_other_succeeds() {
        let yaml = r#"schema_version: '1'
assets:
  - name: notes.pdf
    type: other
    path: assets/notes.pdf
    size: 4096
    hash: '77889900'
    tags: []
    added_at: '2026-05-07T21:00:00Z'
  - name: banner.jpg
    type: image
    path: assets/banner.jpg
    size: 24576
    hash: aabb1122
    tags: [hero]
    added_at: '2026-05-07T18:00:00Z'
"#;
        let index: IndexFile = serde_yaml::from_str(yaml).unwrap();
        let assets = index.assets.unwrap();
        assert_eq!(assets.len(), 2, "should parse both assets: {assets:?}");
    }
}
