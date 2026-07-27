//! Content acquisition for explicit advanced sync. Local files and registered
//! HTTP(S) URLs share the same bounded, credential-redacting boundary.

use std::fs;
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::time::Duration;

use crate::error::{MfError, Result};

/// Acquired content ready for extraction.
#[derive(Debug)]
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
    let abs_path = resolve_local_source(project_path, source_path)?;
    let raw_bytes = fs::read(&abs_path)
        .map_err(|e| MfError::advanced_store(format!("cannot read source file {}: {e}", abs_path.display()), None))?;

    Ok(AcquiredContent {
        raw_bytes,
        acquisition_kind: "local".to_string(),
        canonical_locator: source_path.to_string(),
        registered_location: source_path.to_string(),
    })
}

/// Resolve a registered local Source without allowing it to escape its project.
///
/// Registrations are persisted data and may have been hand-edited, so both
/// lexical traversal and symlink traversal are rejected before reading bytes.
fn resolve_local_source(project_path: &Path, source_path: &str) -> Result<PathBuf> {
    let relative = Path::new(source_path);
    if source_path.is_empty()
        || relative.is_absolute()
        || relative.components().any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(MfError::advanced_store(
            format!("local Source path must be a non-empty relative path without traversal: {source_path:?}"),
            None,
        ));
    }

    let project = project_path.canonicalize().map_err(|e| {
        MfError::advanced_store(format!("cannot resolve project path {}: {e}", project_path.display()), None)
    })?;
    let candidate = project.join(relative);
    let resolved = candidate.canonicalize().map_err(|e| {
        MfError::advanced_store(format!("cannot resolve source file {}: {e}", candidate.display()), None)
    })?;
    if !resolved.starts_with(&project) {
        return Err(MfError::advanced_store(
            format!("local Source path escapes project directory: {source_path:?}"),
            None,
        ));
    }
    Ok(resolved)
}

/// Acquire a mind-forge article (`blog` Source) as assembled Markdown.
///
/// A directory article concatenates its `.md` blocks in filename order (matching
/// `mf build`); a single-file article is read directly. Typora front-matter is
/// stripped so it does not pollute chunks. Network is never involved.
pub fn acquire_article(project_path: &Path, article_path: &str) -> Result<AcquiredContent> {
    let abs = project_path.join(article_path);
    let files: Vec<std::path::PathBuf> = if abs.is_dir() {
        let mut files: Vec<_> = fs::read_dir(&abs)
            .map_err(|e| {
                MfError::advanced_store(format!("cannot read article directory {}: {e}", abs.display()), None)
            })?
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "md"))
            .collect();
        files.sort();
        files
    } else if abs.is_file() {
        vec![abs.clone()]
    } else {
        return Err(MfError::advanced_store(format!("article path not found: {}", abs.display()), None));
    };
    if files.is_empty() {
        return Err(MfError::advanced_store(format!("no Markdown blocks in article {}", abs.display()), None));
    }
    let mut content = String::new();
    for file in &files {
        let part = fs::read_to_string(file)
            .map_err(|e| MfError::advanced_store(format!("cannot read article block {}: {e}", file.display()), None))?;
        content.push_str(crate::service::util::markdown::strip_typora_front_matter(&part).as_str());
        if !content.ends_with('\n') {
            content.push('\n');
        }
    }
    Ok(AcquiredContent {
        raw_bytes: content.into_bytes(),
        acquisition_kind: "article".to_string(),
        canonical_locator: article_path.to_string(),
        registered_location: article_path.to_string(),
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
    fn acquire_local_rejects_path_traversal() {
        let root = tempfile::tempdir().unwrap();
        let project = root.path().join("project");
        fs::create_dir(&project).unwrap();
        fs::write(root.path().join("outside.md"), "outside").unwrap();

        let error = acquire_local(&project, "../outside.md").unwrap_err();
        assert!(error.to_string().contains("without traversal"));
    }

    #[cfg(unix)]
    #[test]
    fn acquire_local_rejects_symlink_escape() {
        let root = tempfile::tempdir().unwrap();
        let project = root.path().join("project");
        fs::create_dir(&project).unwrap();
        let outside = root.path().join("outside.md");
        fs::write(&outside, "outside").unwrap();
        std::os::unix::fs::symlink(&outside, project.join("escape.md")).unwrap();

        let error = acquire_local(&project, "escape.md").unwrap_err();
        assert!(error.to_string().contains("escapes project directory"));
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
