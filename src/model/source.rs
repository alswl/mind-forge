use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::BTreeMap;

// ---------------------------------------------------------------------------
// SourceKind — source channel/origin (mind primary)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub enum SourceKind {
    Yuque,
    Meeting,
    Misc,
    Other(String),
}

impl SourceKind {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Yuque => "yuque",
            Self::Meeting => "meeting",
            Self::Misc => "misc",
            Self::Other(value) => value,
        }
    }
}

impl Serialize for SourceKind {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for SourceKind {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Ok(match value.as_str() {
            "yuque" => Self::Yuque,
            "meeting" => Self::Meeting,
            "misc" => Self::Misc,
            _ => Self::Other(value),
        })
    }
}

impl std::fmt::Display for SourceKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::SourceKind;

    #[test]
    fn unknown_source_kind_round_trips_losslessly() {
        let parsed: SourceKind = serde_yaml::from_str("article_prompt\n").unwrap();
        assert_eq!(parsed, SourceKind::Other("article_prompt".to_string()));
        assert_eq!(serde_yaml::to_string(&parsed).unwrap(), "article_prompt\n");
    }
}

// ---------------------------------------------------------------------------
// FileKind — file format type (prev. SourceKind in mf)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileKind {
    Auto,
    Pdf,
    Rss,
    Web,
    #[default]
    File,
}

impl FileKind {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Pdf => "pdf",
            Self::File => "file",
            Self::Rss => "rss",
            Self::Web => "web",
        }
    }
}

impl std::fmt::Display for FileKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Source {
    pub name: String,
    #[serde(rename = "type", alias = "file_kind", default)]
    pub kind: FileKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_kind: Option<SourceKind>,
    pub url: Option<String>,
    pub path: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub added_at: String,
    #[serde(default)]
    pub updated_at: String,
    /// Spec 075 FR-011: fields the system does not otherwise interpret must
    /// round-trip through every load → save cycle instead of being dropped
    /// by the typed rebuild. The Lance-side registration already stores them
    /// as `extras_json` (see `compatibility::source_entry_extras`); this
    /// catch-all keeps the YAML projection lossless too.
    #[serde(flatten, default)]
    pub extra: BTreeMap<String, serde_yaml::Value>,
}

// ---------------------------------------------------------------------------
// T003: SourceIndexEntry — used by `mf source index` / `mf source clean`
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct SourceIndexEntry {
    pub name: String,
    #[serde(rename = "type")]
    pub kind: FileKind,
    pub path: String,
}

// ---------------------------------------------------------------------------
// T004: SourceIndexReport — used by `mf source index` / `mf source clean`
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct SourceIndexReport {
    pub added: Vec<SourceIndexEntry>,
    pub removed: Vec<SourceIndexEntry>,
    pub kept_count: u64,
    pub dry_run: bool,
}

// ---------------------------------------------------------------------------
// T005: SourceRemoveReport — used by `mf source remove`
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct SourceRemoveReport {
    #[serde(flatten)]
    pub source: Source,
    pub file_deleted: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub references: Vec<crate::model::lifecycle::Reference>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub side_effects: Vec<crate::model::lifecycle::PlannedChange>,
    #[serde(default)]
    pub force: bool,
    #[serde(default)]
    pub dry_run: bool,
}
