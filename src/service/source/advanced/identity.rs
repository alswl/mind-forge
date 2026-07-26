//! Deterministic identity and fingerprint functions for advanced Sources.
//!
//! All identities are content-addressed (SHA-256). Repository moves
//! preserve identities because they use canonical repo-relative paths.
//! Project rename does change project and registration keys, making old
//! bindings immediately non-live after the fact commit.

use sha2::{Digest, Sha256};

/// Canonical hex-encoded SHA-256 digest.
fn hash_hex(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hex::encode(hasher.finalize())
}

/// Simple hex encoding (no external crate dependency for now).
mod hex {
    pub fn encode(bytes: impl AsRef<[u8]>) -> String {
        let mut s = String::with_capacity(bytes.as_ref().len() * 2);
        for b in bytes.as_ref() {
            s.push(HEX_CHARS[(b >> 4) as usize] as char);
            s.push(HEX_CHARS[(b & 0x0f) as usize] as char);
        }
        s
    }
    const HEX_CHARS: &[u8] = b"0123456789abcdef";
}

// ── Identity constructors ─────────────────────────────────────────────────

/// `SHA256(canonical repo-relative project path)`.
/// The path must be stable and deterministic (forward slashes, no trailing slash).
pub fn project_key(repo_relative_path: &str) -> String {
    hash_hex(repo_relative_path.as_bytes())
}

/// `SHA256(project_key || kind || canonical registered locator)`.
/// `kind` is the lowercase file kind string (`file`, `pdf`, `web`, `rss`).
/// `locator` is the project-relative path (forward slashes) or sanitized URL.
pub fn registration_key(project_key: &str, kind: &str, locator: &str) -> String {
    let mut input = String::with_capacity(project_key.len() + kind.len() + locator.len() + 2);
    input.push_str(project_key);
    input.push('\0');
    input.push_str(kind);
    input.push('\0');
    input.push_str(locator);
    hash_hex(input.as_bytes())
}

/// `SHA256(kind || canonical locator || security_context)`.
/// Coordinates safe acquisition deduplication; does not replace registration identity.
/// `security_context` is empty for local files, or a canonical request identity for HTTP.
pub fn acquisition_key(kind: &str, locator: &str, security_context: &str) -> String {
    let mut input = String::with_capacity(kind.len() + locator.len() + security_context.len() + 3);
    input.push_str(kind);
    input.push('\0');
    input.push_str(locator);
    input.push('\0');
    input.push_str(security_context);
    hash_hex(input.as_bytes())
}

/// `SHA256(raw_fingerprint || extracted_fingerprint || content_fingerprint)`.
/// Identifies shared content independent of project/article.
pub fn document_key(raw_fingerprint: &str, extracted_fingerprint: &str, content_fingerprint: &str) -> String {
    let mut input =
        String::with_capacity(raw_fingerprint.len() + extracted_fingerprint.len() + content_fingerprint.len() + 3);
    input.push_str(raw_fingerprint);
    input.push('\0');
    input.push_str(extracted_fingerprint);
    input.push('\0');
    input.push_str(content_fingerprint);
    hash_hex(input.as_bytes())
}

/// `SHA256(document_key || content_revision || canonical_locator || chunk_policy || text_fingerprint)`.
/// Chunk identities are project/article-free.
pub fn chunk_id(
    document_key: &str,
    content_revision: i64,
    locator_json: &str,
    chunk_policy_version: &str,
    text_fingerprint: &str,
) -> String {
    let revision_str = content_revision.to_string();
    let mut input = String::with_capacity(
        document_key.len()
            + revision_str.len()
            + locator_json.len()
            + chunk_policy_version.len()
            + text_fingerprint.len()
            + 5,
    );
    input.push_str(document_key);
    input.push('\0');
    input.push_str(&revision_str);
    input.push('\0');
    input.push_str(locator_json);
    input.push('\0');
    input.push_str(chunk_policy_version);
    input.push('\0');
    input.push_str(text_fingerprint);
    hash_hex(input.as_bytes())
}

/// `SHA256(document_key || content_revision || schema_version || prompt_version)`.
pub fn enrichment_key(document_key: &str, content_revision: i64, schema_version: &str, prompt_version: &str) -> String {
    let revision_str = content_revision.to_string();
    let mut input = String::with_capacity(
        document_key.len() + revision_str.len() + schema_version.len() + prompt_version.len() + 4,
    );
    input.push_str(document_key);
    input.push('\0');
    input.push_str(&revision_str);
    input.push('\0');
    input.push_str(schema_version);
    input.push('\0');
    input.push_str(prompt_version);
    hash_hex(input.as_bytes())
}

/// Raw content fingerprint: `SHA256(raw bytes)`.
pub fn raw_fingerprint(bytes: &[u8]) -> String {
    hash_hex(bytes)
}

/// Extracted content fingerprint: `SHA256(extractor_identity || normalized_utf8_text)`.
pub fn extracted_fingerprint(extractor: &str, text: &str) -> String {
    let mut input = String::with_capacity(extractor.len() + 1 + text.len());
    input.push_str(extractor);
    input.push('\0');
    input.push_str(text);
    hash_hex(input.as_bytes())
}

/// Full content fingerprint covering extraction, chunk, model, and tokenizer identity.
pub fn content_fingerprint(parts: &[&str]) -> String {
    let joined = parts.join("\0");
    hash_hex(joined.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_key_is_stable() {
        let k1 = project_key("projects/alpha");
        let k2 = project_key("projects/alpha");
        assert_eq!(k1, k2);
        assert_eq!(k1.len(), 64); // SHA-256 hex
    }

    #[test]
    fn project_key_distinguishes_paths() {
        assert_ne!(project_key("projects/alpha"), project_key("projects/beta"));
    }

    #[test]
    fn registration_key_includes_project_kind_and_location() {
        let pk = project_key("projects/alpha");
        let rk1 = registration_key(&pk, "file", "sources/notes.md");
        let rk2 = registration_key(&pk, "file", "sources/other.md");
        assert_ne!(rk1, rk2);
    }

    #[test]
    fn registration_key_is_deterministic() {
        let pk = project_key("projects/alpha");
        assert_eq!(registration_key(&pk, "pdf", "papers/ref.pdf"), registration_key(&pk, "pdf", "papers/ref.pdf"),);
    }

    #[test]
    fn document_key_is_stable() {
        let raw = raw_fingerprint(b"hello");
        let ext = extracted_fingerprint("markdown-extractor", "hello");
        let cf = content_fingerprint(&["v1", "e5-small", "384"]);
        let dk1 = document_key(&raw, &ext, &cf);
        let dk2 = document_key(&raw, &ext, &cf);
        assert_eq!(dk1, dk2);
    }

    #[test]
    fn chunk_id_is_deterministic() {
        let dk = document_key(&raw_fingerprint(b"x"), &extracted_fingerprint("e", "x"), &content_fingerprint(&["v1"]));
        let c1 = chunk_id(&dk, 1, r#"{"kind":"text","start_line":1}"#, "v1", &raw_fingerprint(b"chunk"));
        let c2 = chunk_id(&dk, 1, r#"{"kind":"text","start_line":1}"#, "v1", &raw_fingerprint(b"chunk"));
        assert_eq!(c1, c2);
    }

    #[test]
    fn chunk_id_differs_by_ordinal() {
        let dk = document_key(&raw_fingerprint(b"x"), &extracted_fingerprint("e", "x"), &content_fingerprint(&["v1"]));
        let c1 = chunk_id(&dk, 1, r#"{"kind":"text","start_line":1}"#, "v1", &raw_fingerprint(b"chunk"));
        let c2 = chunk_id(&dk, 2, r#"{"kind":"text","start_line":5}"#, "v1", &raw_fingerprint(b"chunk"));
        assert_ne!(c1, c2);
    }

    #[test]
    fn acquisition_key_includes_security_context() {
        let a1 = acquisition_key("web", "https://example.com/feed.xml", "");
        let a2 = acquisition_key("web", "https://example.com/feed.xml", "auth:token1");
        assert_ne!(a1, a2);
    }

    #[test]
    fn raw_fingerprint_is_sha256() {
        let fp = raw_fingerprint(b"test");
        assert_eq!(fp.len(), 64);
        // Verify known SHA-256 value
        assert_eq!(fp, "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08");
    }
}
