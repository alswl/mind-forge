//! Content acquisition: read local files and fetch remote resources.
//!
//! Local files are read directly. Web/RSS sources require explicit sync
//! and are bounded by configured timeouts, byte limits, and redirect counts.
//! Offline mode forbids all network access.

use std::fs;
use std::path::Path;

use crate::error::{MfError, Result};

/// Acquired content ready for extraction.
pub struct AcquiredContent {
    /// Raw bytes as read/retrieved.
    pub raw_bytes: Vec<u8>,
    /// Kind of acquisition: `local` or `http`.
    pub acquisition_kind: String,
    /// The canonical locator used (repo-relative path or sanitized URL).
    pub canonical_locator: String,
    /// Original registered location for provenance.
    pub registered_location: String,
}

/// Acquire content from a local file.
pub fn acquire_local(project_path: &Path, source_path: &str) -> Result<AcquiredContent> {
    let abs_path = project_path.join(source_path);
    let raw_bytes = fs::read(&abs_path)
        .map_err(|e| MfError::advanced_store(format!("cannot read source file {}: {e}", abs_path.display()), None))?;

    Ok(AcquiredContent {
        raw_bytes,
        acquisition_kind: "local".to_string(),
        canonical_locator: source_path.to_string(),
        registered_location: source_path.to_string(),
    })
}

/// Acquire content from a URL. Respects configured byte limit, timeout,
/// and redirect count. Offline mode is enforced by the caller.
pub fn acquire_http(_url: &str, _max_bytes: u64, _timeout_secs: u32, _max_redirects: u32) -> Result<AcquiredContent> {
    // HTTP acquisition is deferred to Phase 4 (US2) when search integrates
    // with web sources. For now, web/RSS sources are marked pending.
    Err(MfError::advanced_store(
        "HTTP acquisition not yet implemented — web/RSS sources will be marked pending during sync".to_string(),
        Some("use `mf source advanced sync` with a local file source instead".to_string()),
    ))
}

/// Detect if a path looks like a URL.
pub fn is_url(location: &str) -> bool {
    location.starts_with("http://") || location.starts_with("https://")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn acquire_local_reads_file() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test.md");
        fs::write(&file_path, "# Hello\n\nWorld").unwrap();

        let content = acquire_local(dir.path(), "test.md").unwrap();
        assert_eq!(content.acquisition_kind, "local");
        assert_eq!(content.raw_bytes, b"# Hello\n\nWorld");
    }

    #[test]
    fn acquire_local_missing_file_is_error() {
        let dir = tempfile::tempdir().unwrap();
        let result = acquire_local(dir.path(), "nonexistent.md");
        assert!(result.is_err());
    }

    #[test]
    fn is_url_detects_http() {
        assert!(is_url("https://example.com/doc.pdf"));
        assert!(is_url("http://example.com"));
        assert!(!is_url("sources/file.md"));
        assert!(!is_url("/absolute/path"));
    }
}
