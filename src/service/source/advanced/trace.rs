//! Source trace-link generation.
//!
//! For each live source registration, produce a stable Markdown link pointing to
//! the original source location — a repo-relative `sources/…` path for local
//! files, or the original URL for web/rss sources.

use std::path::Path;

use serde::Serialize;

use crate::error::{MfError, Result};
use crate::model::source_advanced::RegistrationState;

/// A trace link for one source registration.
#[derive(Debug, Clone, Serialize)]
pub struct SourceTraceLink {
    pub registration_key: String,
    pub project: String,
    pub title: String,
    /// Link target: repo-relative path for local files, URL for web/rss.
    pub target: String,
    /// Safe Markdown: `[title](target)`.
    pub markdown: String,
}

/// Generate a trace link for a source registration.
///
/// Local files (and Bug B web sources stored under `sources/web/`) produce a
/// repo-relative path.  Pure URL locations (legacy, pre-Bug B) produce the
/// original URL.  The title is escaped so Markdown renderers treat it as plain
/// text.
pub fn trace_link(
    registration_key: &str,
    project: &str,
    source_identity: &str,
    _source_type: &str,
    registered_location: &str,
    state: RegistrationState,
) -> Option<SourceTraceLink> {
    if state != RegistrationState::Live {
        return None;
    }
    let title = escape_markdown_text(source_identity);
    let target = escape_markdown_target(registered_location);
    let markdown = format!("[{title}]({target})");
    Some(SourceTraceLink {
        registration_key: registration_key.to_string(),
        project: project.to_string(),
        title,
        target,
        markdown,
    })
}

/// Escape a string for use in a Markdown link's text portion.
///
/// Brackets are escaped to prevent injection.  The source identity is always
/// treated as data, never executed.
fn escape_markdown_text(s: &str) -> String {
    s.replace('[', "\\[").replace(']', "\\]")
}

/// Escape a string for use in a Markdown link's target (URL or path).
///
/// Parentheses are escaped; spaces are preserved (the link renderer will
/// percent-encode if it's a URL).  Source bytes are never interpreted as
/// instructions.
fn escape_markdown_target(s: &str) -> String {
    s.replace('(', "\\(").replace(')', "\\)")
}

/// Resolve the target for trace purposes.
///
/// Local-file sources (path that does not start with `http://` or `https://`)
/// and Bug B web sources (stored under `sources/web/` or `sources/rss/`) use
/// their `registered_location` directly.  Legacy URL-only registrations use the
/// URL.
pub fn trace_target(registered_location: &str) -> Result<String> {
    if registered_location.is_empty() || registered_location == "unknown" {
        return Err(MfError::advanced_store(
            format!("cannot trace source: registered_location is '{registered_location}'"),
            Some("re-add the source with `mf source new` to fix the location".to_string()),
        ));
    }
    Ok(registered_location.to_string())
}

/// Article-kind literals for source_kind filtering.
pub mod article_kind {
    /// Article prose from `outputs/<month>/<key>.md`.
    pub const ARTICLE: &str = "article";
    /// Prompt from `prompts/<key>.md`.
    pub const ARTICLE_PROMPT: &str = "article_prompt";
    /// Thinking from `thinking/<key>.md`.
    pub const ARTICLE_THINKING: &str = "article_thinking";

    /// Repository-authored `project` goal (from `mind.yaml`).
    pub const PROJECT: &str = "project";
    /// Repository-authored `term` definition.
    pub const TERM: &str = "term";

    /// Return true when `kind` is an article-kind.
    pub fn is_article_kind(kind: &str) -> bool {
        matches!(kind, ARTICLE | ARTICLE_PROMPT | ARTICLE_THINKING)
    }

    /// Return true when `kind` is repository-authored/derived content whose
    /// bytes are assembled at sync (article, project, term), not a raw source
    /// file. Such kinds are excluded from export/import bundles and from the
    /// raw-file change detection during export (spec 071).
    pub fn is_derived_kind(kind: &str) -> bool {
        is_article_kind(kind) || matches!(kind, PROJECT | TERM)
    }

    /// Return true when `kind` is a raw source-kind (eligible for export/import).
    pub fn is_source_kind(kind: &str) -> bool {
        !is_derived_kind(kind)
    }
}

/// Generate trace links for all live source registrations in the repository.
pub fn trace_links(repo_root: &Path, project_filter: Option<&str>) -> Result<Vec<SourceTraceLink>> {
    use super::catalog::SourceCatalog;
    use super::config::load_repository_config;
    use super::sync;

    let config = load_repository_config(repo_root)?;
    if !config.is_lance() {
        return Ok(Vec::new());
    }
    let store = sync::open_active_store(repo_root)?;
    let catalog = SourceCatalog::discover(&config, repo_root)?;
    let rows = catalog.registrations(Some(&store))?;

    let mut links = Vec::new();
    for row in &rows {
        // Skip derived kinds — trace is for raw sources only.
        if article_kind::is_derived_kind(&row.source_type) {
            continue;
        }
        // Filter by project.
        if let Some(project) = project_filter
            && row.project_identity != project
        {
            continue;
        }
        // Only live registrations.
        let state = match row.state.as_str() {
            "live" => RegistrationState::Live,
            "pending" => RegistrationState::Pending,
            "failed" => RegistrationState::Failed,
            "orphaned" => RegistrationState::Orphaned,
            _ => RegistrationState::Pending,
        };
        if let Some(link) = trace_link(
            &row.registration_key,
            &row.project_identity,
            &row.source_identity,
            &row.source_type,
            &row.registered_location,
            state,
        ) {
            links.push(link);
        }
    }
    links.sort_by(|a, b| a.registration_key.cmp(&b.registration_key));
    Ok(links)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trace_link_for_live_local_source() {
        let link =
            trace_link("rk", "alpha", "My Notes", "file", "sources/file/notes.md", RegistrationState::Live).unwrap();
        assert_eq!(link.markdown, "[My Notes](sources/file/notes.md)");
    }

    #[test]
    fn trace_link_for_live_web_source() {
        let link =
            trace_link("rk", "alpha", "Research", "web", "https://example.com/page", RegistrationState::Live).unwrap();
        assert_eq!(link.markdown, "[Research](https://example.com/page)");
    }

    #[test]
    fn trace_link_escapes_brackets_in_title() {
        let link =
            trace_link("rk", "alpha", "[IMPORTANT] Notes", "file", "sources/file/notes.md", RegistrationState::Live)
                .unwrap();
        assert_eq!(link.markdown, r"[\[IMPORTANT\] Notes](sources/file/notes.md)");
    }

    #[test]
    fn trace_link_skips_non_live() {
        assert!(trace_link("rk", "alpha", "x", "file", "sources/f", RegistrationState::Failed).is_none());
    }

    #[test]
    fn article_kind_filter() {
        assert!(article_kind::is_article_kind("article"));
        assert!(article_kind::is_article_kind("article_prompt"));
        assert!(article_kind::is_article_kind("article_thinking"));
        assert!(!article_kind::is_article_kind("file"));
        assert!(!article_kind::is_article_kind("web"));
        assert!(article_kind::is_source_kind("file"));
        assert!(!article_kind::is_source_kind("article"));
    }

    #[test]
    fn trace_target_rejects_unknown() {
        assert!(trace_target("unknown").is_err());
        assert!(trace_target("").is_err());
    }
}
