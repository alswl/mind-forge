use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
#[clap(rename_all = "lowercase")]
pub enum AssetKind {
    Image,
    Video,
    Audio,
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Asset {
    pub name: String,
    #[serde(rename = "type")]
    pub kind: AssetKind,
    pub path: String,
    pub size: u64,
    pub hash: String,
    #[serde(default)]
    pub tags: Vec<String>,
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
