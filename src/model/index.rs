use serde::{Deserialize, Serialize};

use super::article::Article;
use super::asset::Asset;
use super::source::Source;
use super::term::Term;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PublishStatus {
    Draft,
    Published,
    Archived,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublishRecord {
    pub path: String,
    pub target_name: String,
    pub status: PublishStatus,
    pub target_url: Option<String>,
    pub published_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexFile {
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
}

impl IndexFile {
    pub fn create_default() -> Self {
        Self {
            schema_version: "1".to_string(),
            sources: None,
            assets: None,
            articles: None,
            terms: None,
            publish_records: None,
        }
    }
}
