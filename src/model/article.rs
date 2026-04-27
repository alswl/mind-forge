use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArticleType {
    Arch,
    Prd,
    Blog,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Article {
    pub title: String,
    pub project: String,
    #[serde(rename = "type")]
    pub article_type: ArticleType,
    pub source_path: String,
    pub created_at: String,
    pub updated_at: String,
}
