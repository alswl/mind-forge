//! Content acquisition for explicit advanced sync. Local files and registered
//! HTTP(S) URLs share the same bounded, credential-redacting boundary.

use std::fs;
use std::io::Read;
use std::path::Path;
use std::time::Duration;

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

/// Fetch a registered HTTP(S) Source. This function is deliberately only
/// called by sync/rebuild; search and ordinary Source commands never acquire
/// URLs. Both HTTP and HTTPS are accepted because local fixture/proxy services
/// are a supported POC use case.
pub fn acquire_http(
    location: &str,
    max_bytes: u64,
    timeout_seconds: u32,
    max_redirects: u32,
) -> Result<AcquiredContent> {
    let url = reqwest::Url::parse(location)
        .map_err(|_| MfError::advanced_store("registered URL is invalid".to_string(), None))?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err(MfError::advanced_store("registered URL must use HTTP or HTTPS".to_string(), None));
    }
    let canonical_locator = redact_url(&url);
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(timeout_seconds as u64))
        .redirect(reqwest::redirect::Policy::limited(max_redirects as usize))
        .build()
        .map_err(|e| MfError::advanced_store(format!("failed to initialize HTTP client: {e}"), None))?;
    let response =
        client.get(url).send().map_err(|e| MfError::advanced_store(format!("HTTP acquisition failed: {e}"), None))?;
    if !response.status().is_success() {
        return Err(MfError::advanced_store(format!("HTTP acquisition returned {}", response.status()), None));
    }
    if let Some(length) = response.content_length()
        && length > max_bytes
    {
        return Err(MfError::advanced_store("HTTP response exceeds configured byte limit".to_string(), None));
    }
    let mut raw_bytes = Vec::new();
    response
        .take(max_bytes.saturating_add(1))
        .read_to_end(&mut raw_bytes)
        .map_err(|e| MfError::advanced_store(format!("failed to read HTTP response: {e}"), None))?;
    if raw_bytes.len() as u64 > max_bytes {
        return Err(MfError::advanced_store("HTTP response exceeds configured byte limit".to_string(), None));
    }
    Ok(AcquiredContent {
        raw_bytes,
        acquisition_kind: "http".to_string(),
        canonical_locator: canonical_locator.clone(),
        registered_location: canonical_locator,
    })
}

/// Redact credentials/fragments from URL locators; non-URL locators (local
/// paths) pass through unchanged.
pub fn redact_locator(location: &str) -> String {
    if is_url(location)
        && let Ok(url) = reqwest::Url::parse(location)
    {
        redact_url(&url)
    } else {
        location.to_string()
    }
}

/// Removes userinfo and fragments before a locator reaches reports or storage.
pub fn redact_url(url: &reqwest::Url) -> String {
    let mut safe = url.clone();
    let _ = safe.set_username("");
    let _ = safe.set_password(None);
    safe.set_fragment(None);
    safe.to_string()
}

/// Detect if a path looks like a URL.
pub fn is_url(location: &str) -> bool {
    location.starts_with("http://") || location.starts_with("https://")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::net::TcpListener;

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

    #[test]
    fn redact_url_removes_credentials_and_fragment() {
        let url = reqwest::Url::parse("http://user:secret@example.test/a#fragment").unwrap();
        assert_eq!(redact_url(&url), "http://example.test/a");
    }

    #[test]
    fn acquire_http_reads_a_bounded_local_http_response() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0_u8; 512];
            let _ = stream.read(&mut request).unwrap();
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nConnection: close\r\nContent-Length: 11\r\nContent-Type: text/html\r\n\r\nhello world")
                .unwrap();
        });
        let content = acquire_http(&format!("http://{address}/doc"), 64, 2, 0).unwrap();
        server.join().unwrap();
        assert_eq!(content.acquisition_kind, "http");
        assert_eq!(content.raw_bytes, b"hello world");
    }
}
