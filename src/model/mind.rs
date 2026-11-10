use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum ContentLayout {
    #[default]
    Category,
    Bundle,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct MindRegistration {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub default_output: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mind {
    pub identity: String,
    pub project_identity: String,
    pub mind_identity: String,
    pub name: String,
    pub title: String,
    pub path: Option<String>,
    pub default_output: Option<String>,
    pub roles: BTreeMap<String, String>,
    pub outputs: Vec<Output>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Output {
    pub identity: String,
    pub project_identity: String,
    pub mind_identity: String,
    pub output_identity: String,
    pub mind: String,
    pub name: String,
    pub path: String,
    pub shape: String,
    pub build_path: String,
}
