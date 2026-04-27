use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
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
