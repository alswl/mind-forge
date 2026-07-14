//! Types for repository-wide Source search results and reports.
//!
//! Shared by the basic and advanced retrieval paths regardless of backend mode.

#![allow(dead_code)]

use serde::{Deserialize, Serialize};

/// Search mode: basic metadata, advanced content, or fused both.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SearchMode {
    Basic,
    Advanced,
    Both,
}

/// Backend mode for Source operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ResolvedBackend {
    Legacy,
    Lance,
}

/// Effective search context: repository-wide or scoped to a project.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EffectiveSearchMode {
    pub backend: ResolvedBackend,
    pub requested_mode: SearchMode,
    pub resolved_mode: SearchMode,
    pub degraded: bool,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub degradation_reasons: Vec<String>,
}

/// Retrieval path marker for provenance tracking.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RetrievalPath {
    Basic,
    AdvancedKeyword,
    AdvancedSemantic,
}
