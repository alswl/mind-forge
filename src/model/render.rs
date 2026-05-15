//! Data model types for the `mf render` command group.

use clap::ValueEnum;
use serde::Serialize;

/// HTML output form requested in the render prompt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HtmlForm {
    /// Full HTML document with `<html>`, `<head>`, and `<body>`.
    Document,
    /// Embeddable HTML fragment without outer document structure.
    Fragment,
}

/// Scope of a render request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RenderScope {
    Article,
    Project,
}

/// Template source: built-in or custom (from `.mind-forge/renders/`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TemplateSource {
    BuiltIn,
    Custom,
}

/// A render template definition.
#[derive(Debug, Clone, Serialize)]
pub struct RenderTemplate {
    pub name: String,
    pub label: String,
    pub description: String,
    pub source: TemplateSource,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    pub default: bool,
}

/// Content of one renderable output artifact.
#[derive(Debug, Clone)]
pub struct OutputContent {
    pub path: String,
    pub content: String,
    pub size_bytes: u64,
}

/// Generated render prompt.
#[derive(Debug, Clone, Serialize)]
pub struct GeneratedPrompt {
    pub prompt: String,
    pub template: String,
    pub template_source: TemplateSource,
    pub scope: RenderScope,
    pub project: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub article: Option<String>,
    pub outputs: Vec<OutputInfo>,
}

/// Output info for JSON serialization.
#[derive(Debug, Clone, Serialize)]
pub struct OutputInfo {
    pub path: String,
    pub size_bytes: u64,
}

/// Render request parameters.
#[derive(Debug, Clone)]
pub struct RenderRequest {
    pub scope: RenderScope,
    pub project: String,
    pub article: Option<String>,
    pub template: String,
}
