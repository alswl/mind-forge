use serde::{Deserialize, Serialize};

/// A prompt's declared writing mode, mirrored from frontmatter `mode:`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PromptMode {
    Editorial,
    Research,
    DecisionResearch,
}

impl PromptMode {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Editorial => "editorial",
            Self::Research => "research",
            Self::DecisionResearch => "decision-research",
        }
    }

    /// Parse a frontmatter `mode:` value. Returns `None` for anything that is
    /// not one of the three declared values (tolerated, not an error).
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim() {
            "editorial" => Some(Self::Editorial),
            "research" => Some(Self::Research),
            "decision-research" => Some(Self::DecisionResearch),
            _ => None,
        }
    }
}

impl std::fmt::Display for PromptMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Derived projection of a `prompts/<key>.md` control-plane file.
///
/// The Markdown file is the source of truth: `article` and `mode` are
/// mirrored from its YAML frontmatter. This struct never persists
/// `binding_status` — that relation is computed at query time against the
/// current `articles` set (see `service::index::resolve_prompt_bindings`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Prompt {
    #[serde(default)]
    pub path: String,
    /// Bound article path, mirrored from frontmatter `article:`. Empty when
    /// the prompt declares no binding.
    #[serde(default)]
    pub article: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<PromptMode>,
    #[serde(default)]
    pub updated_at: String,
}

/// Binding relation between a prompt/thinking projection and the `articles`
/// set, computed at query time. Never persisted in `mind-index.yaml`.
///
/// `Duplicate` only ever applies to prompts (two prompts bound to one
/// article); thinking bindings resolve to `Bound` or `Orphan` only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BindingStatus {
    Bound,
    Orphan,
    Duplicate,
}

impl BindingStatus {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Bound => "bound",
            Self::Orphan => "orphan",
            Self::Duplicate => "duplicate",
        }
    }
}

impl std::fmt::Display for BindingStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Report from `mf prompt index` reconciling the `prompts:` projection with
/// `prompts/` on disk.
#[derive(Debug, Clone, Serialize)]
pub struct PromptIndexReport {
    pub added: Vec<Prompt>,
    pub removed: Vec<Prompt>,
    pub kept_count: u64,
    pub dry_run: bool,
}
