//! Local embedding generation using FastEmbed with ONNX Runtime.
//!
//! Uses intfloat/multilingual-e5-small (384-dim) with Mean pooling,
//! L2 normalization, and E5 query/passage prefixes.
//!
//! The model loads lazily on first use from the FastEmbed cache directory.
//! Sync/search never trigger implicit model downloads — the model must be
//! installed explicitly via `mf source advanced model install`.

use std::sync::Mutex;

use crate::error::{MfError, Result};

pub const EMBEDDING_DIMENSION: usize = 384;
pub const QUERY_PREFIX: &str = "query: ";
pub const PASSAGE_PREFIX: &str = "passage: ";

/// A lazily-initialized, thread-safe embedding model handle.
pub struct EmbeddingModel {
    inner: Mutex<Option<fastembed::TextEmbedding>>,
}

impl EmbeddingModel {
    pub fn new() -> Self {
        Self { inner: Mutex::new(None) }
    }

    fn ensure_loaded(&self) -> Result<()> {
        let mut guard =
            self.inner.lock().map_err(|_| MfError::advanced_store("embedding lock poisoned".to_string(), None))?;
        if guard.is_none() {
            let model = fastembed::TextEmbedding::try_new(fastembed::InitOptions::new(
                fastembed::EmbeddingModel::MultilingualE5Small,
            ))
            .map_err(|e| MfError::advanced_store(format!("failed to load embedding model: {e}"), None))?;
            *guard = Some(model);
        }
        Ok(())
    }

    pub fn embed_passages(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        self.ensure_loaded()?;
        let mut guard =
            self.inner.lock().map_err(|_| MfError::advanced_store("embedding lock poisoned".to_string(), None))?;
        let model = guard.as_mut().unwrap();
        let prefixed: Vec<String> = texts.iter().map(|t| format!("{PASSAGE_PREFIX}{t}")).collect();
        let refs: Vec<&str> = prefixed.iter().map(|s| s.as_str()).collect();
        model.embed(refs, None).map_err(|e| MfError::advanced_store(format!("embedding failed: {e}"), None))
    }

    pub fn embed_query(&self, query: &str) -> Result<Vec<f32>> {
        self.ensure_loaded()?;
        let mut guard =
            self.inner.lock().map_err(|_| MfError::advanced_store("embedding lock poisoned".to_string(), None))?;
        let model = guard.as_mut().unwrap();
        let results = model
            .embed(&[format!("{QUERY_PREFIX}{query}")], None)
            .map_err(|e| MfError::advanced_store(format!("query embedding failed: {e}"), None))?;
        results.into_iter().next().ok_or_else(|| MfError::advanced_store("no embedding returned".to_string(), None))
    }
}

/// Fallback zero-vector embeddings for when no model is available.
pub fn embed_passages_fallback(texts: &[&str]) -> Result<Vec<Vec<f32>>> {
    Ok(texts.iter().map(|_| vec![0.0f32; EMBEDDING_DIMENSION]).collect())
}

/// Fallback zero-vector query embedding.
pub fn embed_query_fallback(_query: &str) -> Result<Vec<f32>> {
    Ok(vec![0.0f32; EMBEDDING_DIMENSION])
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
    fn fallback_returns_correct_dimension() {
        let embeddings = embed_passages_fallback(&["hello", "world"]).unwrap();
        assert_eq!(embeddings.len(), 2);
        assert_eq!(embeddings[0].len(), EMBEDDING_DIMENSION);
    }

    #[test]
    fn cosine_similarity_identical_is_one() {
        let v = vec![1.0f32, 2.0, 3.0];
        assert!((cosine_similarity(&v, &v) - 1.0).abs() < 0.001);
    }

    #[test]
    fn cosine_similarity_orthogonal_is_zero() {
        assert!((cosine_similarity(&[1.0, 0.0, 0.0], &[0.0, 1.0, 0.0]) - 0.0).abs() < 0.001);
    }
}
