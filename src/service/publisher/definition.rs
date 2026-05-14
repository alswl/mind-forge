use serde::Deserialize;

/// YAML schema for `.mind-forge/publisher/<file>.yaml`.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct PublisherDefinition {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(rename = "type")]
    pub target_type: Option<crate::model::config::PublishTargetType>,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default)]
    pub config: Option<serde_json::Value>,
    #[serde(default)]
    pub required_inputs: Vec<String>,
}

fn default_enabled() -> bool {
    true
}
