use serde::Serialize;

use crate::model::prompt::BindingStatus;

pub use self::index::reconcile;
pub use self::list::list;
pub use self::show::show;

pub mod index;
pub mod list;
pub mod show;

/// An owned, sorted, binding-status-annotated view of a `thinking/<key>.md`
/// projection. `binding_status` (`Bound`/`Orphan` only) is computed fresh on
/// every call — see `service::index::resolve_thinking_bindings` — and is
/// never persisted.
#[derive(Debug, Clone, Serialize)]
pub struct ThinkingRecord {
    pub path: String,
    pub article: String,
    pub updated_at: String,
    pub binding_status: BindingStatus,
}

impl ThinkingRecord {
    /// Identity for CLI list/show round-trip: the entry's own path.
    pub fn identity(&self) -> String {
        self.path.clone()
    }
}
