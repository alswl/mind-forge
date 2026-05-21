use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
#[clap(rename_all = "lowercase")]
pub enum AssetKind {
    Image,
    Video,
    Audio,
    #[default]
    Other,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Asset {
    pub name: String,
    #[serde(rename = "type", default)]
    pub kind: AssetKind,
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub size: u64,
    #[serde(default)]
    pub hash: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub added_at: String,
}

// ---------------------------------------------------------------------------
// View models (not persisted, only used for command JSON output)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct AssetUpdateResult {
    pub path: String,
    pub changed: bool,
    pub old_size: u64,
    pub new_size: u64,
    pub old_hash: String,
    pub new_hash: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct AssetIndexEntry {
    pub name: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct AssetIndexReport {
    pub added: Vec<AssetIndexEntry>,
    pub removed: Vec<AssetIndexEntry>,
    pub kept_count: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refreshed: Option<Vec<AssetUpdateResult>>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct AssetCleanReport {
    pub stale_entries: Vec<String>,
    pub removed_count: u64,
    pub dry_run: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct AssetRemoveReport {
    pub removed: String,
    pub was_referenced: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub references: Vec<crate::model::lifecycle::Reference>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub side_effects: Vec<crate::model::lifecycle::PlannedChange>,
    #[serde(default)]
    pub force: bool,
    #[serde(default)]
    pub dry_run: bool,
}
