use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub name: String,
    pub path: String,
    pub created_at: String,
    pub archived_at: Option<String>,
    pub article_count: usize,
    pub source_count: usize,
    pub asset_count: usize,
    pub term_count: usize,
    pub last_activity: Option<String>,
}
