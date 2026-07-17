//! OpenAI-compatible embedding provider integration for advanced Sources.

use std::time::Duration;

use crate::error::{MfError, Result};
use crate::model::manifest::AdvancedSourceConfig;

pub const EMBEDDING_DIMENSION: usize = 384;
pub const QUERY_PREFIX: &str = "query: ";
pub const PASSAGE_PREFIX: &str = "passage: ";

/// Explicit OpenAI-compatible embedding client.  It owns no credentials on
/// disk: the API key is read from the configured environment variable for each
/// construction and only used in the Authorization header.
#[derive(Debug)]
pub struct EmbeddingProvider {
    endpoint: String,
    model: String,
    api_key: Option<String>,
    dimension: usize,
    timeout: Duration,
}

impl EmbeddingProvider {
    pub fn from_config(config: &AdvancedSourceConfig) -> Result<Option<Self>> {
        let Some(endpoint) = config.embedding_endpoint.as_deref().filter(|value| !value.trim().is_empty()) else {
            return Ok(None);
        };
        let model = config.embedding_model.as_deref().filter(|value| !value.trim().is_empty()).ok_or_else(|| {
            MfError::usage(
                "advanced.embedding_model is required when advanced.embedding_endpoint is configured".to_string(),
                Some("set both embedding_endpoint and embedding_model in minds.yaml".to_string()),
            )
        })?;
        let api_key = match config.embedding_api_key_env.as_deref() {
            Some(name) => Some(std::env::var(name).map_err(|_| {
                MfError::usage(
                    format!("embedding credential environment variable '{name}' is not set"),
                    Some("set the configured environment variable before sync or advanced search".to_string()),
                )
            })?),
            None => None,
        };
        let endpoint = endpoint.trim_end_matches('/');
        let endpoint = if endpoint.ends_with("/v1/embeddings") {
            endpoint.to_string()
        } else {
            format!("{endpoint}/v1/embeddings")
        };
        Ok(Some(Self {
            endpoint,
            model: model.to_string(),
            api_key,
            dimension: config.embedding_dimension as usize,
            timeout: Duration::from_secs(config.fetch_timeout_seconds as u64),
        }))
    }

    pub fn embed_passages(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        self.request(texts)
    }

    pub fn embed_query(&self, query: &str) -> Result<Vec<f32>> {
        self.request(&[query])?
            .into_iter()
            .next()
            .ok_or_else(|| MfError::advanced_store("embedding provider returned no vector".to_string(), None))
    }

    fn request(&self, inputs: &[&str]) -> Result<Vec<Vec<f32>>> {
        if inputs.is_empty() {
            return Ok(vec![]);
        }
        let client = reqwest::blocking::Client::builder().timeout(self.timeout).build().map_err(|e| {
            MfError::advanced_store(format!("failed to initialize embedding provider client: {e}"), None)
        })?;
        let mut request = client.post(&self.endpoint).header(reqwest::header::CONTENT_TYPE, "application/json");
        if let Some(api_key) = &self.api_key {
            request = request.bearer_auth(api_key);
        }
        let body = serde_json::json!({"model": self.model, "input": inputs});
        let response = request
            .body(body.to_string())
            .send()
            .map_err(|e| MfError::advanced_store(format!("embedding provider request failed: {e}"), None))?;
        let status = response.status();
        let body = response
            .text()
            .map_err(|e| MfError::advanced_store(format!("failed to read embedding provider response: {e}"), None))?;
        if !status.is_success() {
            return Err(MfError::advanced_store(format!("embedding provider returned HTTP {status}"), None));
        }
        #[derive(serde::Deserialize)]
        struct Response {
            data: Vec<Item>,
        }
        #[derive(serde::Deserialize)]
        struct Item {
            index: usize,
            embedding: Vec<f32>,
        }
        let mut data: Vec<Item> = serde_json::from_str::<Response>(&body)
            .map_err(|_| MfError::advanced_store("embedding provider returned an invalid response".to_string(), None))?
            .data;
        data.sort_by_key(|item| item.index);
        if data.len() != inputs.len() {
            return Err(MfError::advanced_store(
                "embedding provider returned an unexpected vector count".to_string(),
                None,
            ));
        }
        for item in &data {
            if item.embedding.len() != self.dimension || item.embedding.iter().any(|value| !value.is_finite()) {
                return Err(MfError::advanced_store(
                    "embedding provider returned an invalid vector dimension or value".to_string(),
                    None,
                ));
            }
        }
        Ok(data.into_iter().map(|item| item.embedding).collect())
    }
}

/// Resolve the configured provider without persisting credentials.  Absence is
/// normal and callers should expose deterministic degraded behavior.
pub fn provider_for_repo(repo_root: &std::path::Path) -> Result<Option<EmbeddingProvider>> {
    let manifest_path = repo_root.join("minds.yaml");
    if !manifest_path.exists() {
        return Ok(None);
    }
    let manifest = crate::service::repo::load_manifest(&manifest_path)?;
    let Some(advanced) = manifest.source.as_ref().and_then(|source| source.advanced.as_ref()) else {
        return Ok(None);
    };
    EmbeddingProvider::from_config(advanced)
}

/// Cosine similarity.
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let na: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let nb: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if na == 0.0 || nb == 0.0 { 0.0 } else { dot / (na * nb) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cosine_similarity_identical_is_one() {
        let v = vec![1.0f32, 2.0, 3.0];
        assert!((cosine_similarity(&v, &v) - 1.0).abs() < 0.001);
    }

    #[test]
    fn cosine_similarity_orthogonal_is_zero() {
        assert!((cosine_similarity(&[1.0, 0.0, 0.0], &[0.0, 1.0, 0.0]) - 0.0).abs() < 0.001);
    }

    #[test]
    fn provider_requires_a_model_when_endpoint_is_configured() {
        let config = AdvancedSourceConfig {
            embedding_endpoint: Some("http://127.0.0.1:9999".to_string()),
            ..Default::default()
        };
        let error = EmbeddingProvider::from_config(&config).unwrap_err();
        assert!(error.to_string().contains("embedding_model"));
    }

    #[test]
    fn provider_is_absent_without_an_explicit_endpoint() {
        assert!(EmbeddingProvider::from_config(&AdvancedSourceConfig::default()).unwrap().is_none());
    }
}
