use serde::Serialize;

use crate::model::prompt::{BindingStatus, PromptMode};

pub use self::index::reconcile;
pub use self::list::list;
pub use self::show::show;

pub mod index;
pub mod list;
pub mod show;

/// An owned, sorted, binding-status-annotated view of a `prompts/<key>.md`
/// projection. `binding_status` is computed fresh on every call — see
/// `service::index::resolve_prompt_bindings` — and is never persisted.
#[derive(Debug, Clone, Serialize)]
pub struct PromptRecord {
    pub path: String,
    pub article: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<PromptMode>,
    pub updated_at: String,
    pub binding_status: BindingStatus,
}

impl PromptRecord {
    /// Identity for CLI list/show round-trip: the prompt's own path.
    pub fn identity(&self) -> String {
        self.path.clone()
    }
}
