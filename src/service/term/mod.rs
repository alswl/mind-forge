// Term service — implemented in 012-term-core.
// Directory module facade: re-exports sub-module public items.

pub mod fix;
pub mod learn;
pub mod lint;
pub mod list;
pub mod lookup;
pub mod new;
pub mod show;

use std::collections::BTreeSet;

use crate::error::MfError;

pub use self::fix::fix_term;
pub use self::learn::learn_correction;
pub use self::lint::{lint_file, lint_terms};
pub use self::list::list_terms;
pub(crate) use self::lookup::find_term_by_correct;
pub use self::new::new_term;
pub use self::show::show_term;

// ── Helpers shared by sub-modules ────────────────────────────────────────────

pub(crate) fn dedup_preserve_first(items: &[String]) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut result = Vec::new();
    for item in items {
        if seen.insert(item.clone()) {
            result.push(item.clone());
        }
    }
    result
}

pub(crate) fn sort_terms_by_name(terms: &mut [crate::model::term::Term]) {
    terms.sort_by(|a, b| a.term.cmp(&b.term));
}

// ── Helper: MfError::internal for service module ─────────────────────────────

impl MfError {
    pub(crate) fn internal(msg: impl Into<String>) -> Self {
        MfError::Internal(anyhow::anyhow!(msg.into()))
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dedup_removes_duplicates() {
        let items = vec!["a".into(), "b".into(), "a".into(), "c".into()];
        let result = dedup_preserve_first(&items);
        assert_eq!(result, vec!["a", "b", "c"]);
    }

    #[test]
    fn dedup_empty() {
        assert_eq!(dedup_preserve_first(&[]), Vec::<String>::new());
    }
}
