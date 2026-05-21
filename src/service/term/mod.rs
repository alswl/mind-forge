// Term service — implemented in 012-term-core.
// Directory module facade: re-exports sub-module public items.

pub mod fix;
pub mod global;
pub mod learn;
pub mod lint;
pub mod list;
pub mod lookup;
pub mod new;
pub mod remove;
pub mod rename;
pub mod repo_format;
pub mod show;

use std::collections::BTreeSet;

use crate::error::MfError;

pub use self::fix::fix_term;
pub use self::learn::learn_correction;
pub use self::lint::{lint_path, lint_terms};
pub use self::list::list_terms;
pub(crate) use self::lookup::find_term_by_correct;
pub use self::new::new_term;
pub use self::remove::{remove_term, remove_term_global};
pub use self::rename::rename_term;
pub use self::show::show_term;

// ── Input/patch types for new/update ─────────────────────────────────────────

/// Fields available when creating a new term entry.
#[derive(Debug, Default, Clone, Copy)]
pub struct TermInput<'a> {
    pub definition: Option<&'a str>,
    pub description: Option<&'a str>,
    pub confidence: Option<f64>,
    pub aliases: &'a [String],
    pub tags: &'a [String],
}

/// Patch applied when updating an existing term entry.
///
/// `clear_description` / `clear_confidence` remove the field even when the
/// corresponding value is `None` (callers must reject combining `Some` and
/// `clear_*` at the input boundary).
#[derive(Debug, Default, Clone, Copy)]
pub struct TermUpdate<'a> {
    pub definition: Option<&'a str>,
    pub description: Option<&'a str>,
    pub clear_description: bool,
    pub confidence: Option<f64>,
    pub clear_confidence: bool,
    pub aliases: &'a [String],
    pub tags: &'a [String],
}

impl<'a> TermUpdate<'a> {
    pub fn has_legacy_flags(&self) -> bool {
        self.definition.is_some() || !self.aliases.is_empty() || !self.tags.is_empty()
    }
    pub fn has_metadata_flags(&self) -> bool {
        self.description.is_some() || self.clear_description || self.confidence.is_some() || self.clear_confidence
    }
}

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

// ── Confidence validation ────────────────────────────────────────────────────

/// Validate `confidence` is a finite `f64` in \[0.0, 1.0\].
///
/// Returns `Ok(())` when `None` (unspecified is valid). Non-finite,
/// out-of-range values return a usage error that callers should emit
/// before writing any file.
pub fn validate_confidence(value: Option<f64>) -> crate::error::Result<()> {
    let Some(v) = value else {
        return Ok(());
    };
    if v.is_nan() || v.is_infinite() {
        return Err(MfError::usage("confidence must be a finite number between 0.0 and 1.0", None));
    }
    if !(0.0..=1.0).contains(&v) {
        return Err(MfError::usage(
            format!("confidence {v} is out of range; valid range is 0.0 to 1.0"),
            Some("choose a value between 0.0 and 1.0".to_string()),
        ));
    }
    Ok(())
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

    // ── Confidence validation ──────────────────────────────────────────────

    #[test]
    fn confidence_none_is_valid() {
        assert!(validate_confidence(None).is_ok());
    }

    #[test]
    fn confidence_boundaries_are_valid() {
        assert!(validate_confidence(Some(0.0)).is_ok());
        assert!(validate_confidence(Some(1.0)).is_ok());
        assert!(validate_confidence(Some(0.5)).is_ok());
    }

    #[test]
    fn confidence_below_range_is_usage_error() {
        let err = validate_confidence(Some(-0.1)).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("out of range"), "got: {msg}");
    }

    #[test]
    fn confidence_above_range_is_usage_error() {
        let err = validate_confidence(Some(1.1)).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("out of range"), "got: {msg}");
    }

    #[test]
    fn confidence_nan_is_usage_error() {
        let err = validate_confidence(Some(f64::NAN)).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("finite"), "got: {msg}");
    }

    #[test]
    fn confidence_infinity_is_usage_error() {
        let err = validate_confidence(Some(f64::INFINITY)).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("finite"), "got: {msg}");
    }

    #[test]
    fn confidence_neg_infinity_is_usage_error() {
        let err = validate_confidence(Some(f64::NEG_INFINITY)).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("finite"), "got: {msg}");
    }
}
