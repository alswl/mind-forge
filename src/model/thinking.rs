use serde::{Deserialize, Serialize};

/// Derived projection of a `thinking/<key>.md` working-ledger file.
///
/// The Markdown file is the source of truth. `article` associates the ledger
/// with an article by key alignment (no thinking-specific frontmatter is
/// required). `binding_status` is never persisted — it is computed at query
/// time against the current `articles` set (see
/// `service::index::resolve_thinking_bindings`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Thinking {
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub article: String,
    #[serde(default)]
    pub updated_at: String,
}

/// Report from `mf thinking index` reconciling the `thinking:` projection
/// with `thinking/` on disk.
#[derive(Debug, Clone, Serialize)]
pub struct ThinkingIndexReport {
    pub added: Vec<Thinking>,
    pub removed: Vec<Thinking>,
    pub kept_count: u64,
    pub dry_run: bool,
}
