use std::path::Path;

use crate::error::{MfError, Result};

pub use self::add::{add, AddArgs, AddMode};
pub(crate) use self::add::{classify_input, InputForm};
pub use self::clean::clean;
pub use self::index::reconcile;
pub use self::list::list;
pub use self::remove::remove;
pub use self::update::{update, UpdateArgs};

// ── URL validation ───────────────────────────────────────────────────────────

pub(crate) fn validate_url(s: &str) -> Result<()> {
    if !(s.starts_with("http://") || s.starts_with("https://")) {
        return Err(MfError::usage(
            format!("invalid URL '{s}': must start with http:// or https:// and include a host"),
            None as Option<String>,
        ));
    }
    let after_scheme = s.strip_prefix("https://").or_else(|| s.strip_prefix("http://")).unwrap_or("");
    if after_scheme.is_empty() {
        return Err(MfError::usage(
            format!("invalid URL '{s}': must start with http:// or https:// and include a host"),
            None as Option<String>,
        ));
    }
    Ok(())
}

// ── Derive name from path ────────────────────────────────────────────────────

pub(crate) fn derive_name_from_path(p: &Path) -> Result<String> {
    let stem = p.file_stem().and_then(|s| s.to_str()).map(|s| s.to_string()).ok_or_else(|| {
        MfError::usage(
            format!("cannot derive name from path '{}'", p.display()),
            Some("pass --name <STRING>".to_string()),
        )
    })?;
    if stem.is_empty() {
        return Err(MfError::usage(
            format!("cannot derive name from path '{}'", p.display()),
            Some("pass --name <STRING>".to_string()),
        ));
    }
    Ok(stem)
}

// ── Infer SourceKind from path ───────────────────────────────────────────────

pub(crate) fn infer_kind_from_path(p: &Path) -> crate::model::source::SourceKind {
    let ext = p.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase());
    match ext.as_deref() {
        Some("pdf") => crate::model::source::SourceKind::Pdf,
        _ => crate::model::source::SourceKind::File,
    }
}

pub mod add;
pub mod clean;
pub mod index;
pub mod list;
pub mod remove;
pub mod update;
