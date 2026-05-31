use std::path::Path;

/// Build an OSC 8 hyperlink escape sequence.
///
/// When the policy allows hyperlinks, wraps `visible` in an OSC 8 sequence
/// targeting `uri`. The visible text always includes the copyable target so
/// plain-text fallback preserves usefulness.
///
/// When hyperlinks are disabled, returns `visible` unchanged.
#[allow(dead_code)]
pub fn render_link(visible: &str, uri: &str, emit_hyperlinks: bool) -> String {
    if !emit_hyperlinks {
        return visible.to_string();
    }
    format!("\x1b]8;;{uri}\x1b\\{visible}\x1b]8;;\x1b\\")
}

/// Wrap a file path for display. When hyperlinks are enabled and `base_dir`
/// is available, emits a `file://` OSC 8 link. `path` is a repo-relative
/// path; `base_dir` is the repo root used to resolve it to an absolute
/// `file://` URI. The visible label always shows the original relative path.
///
/// When `base_dir` is `None`, returns the path unchanged — a `file://` URI
/// cannot be constructed without a known base directory.
pub fn render_path_link(path: &str, base_dir: Option<&Path>, emit_hyperlinks: bool) -> String {
    let Some(base) = base_dir else {
        return path.to_string();
    };
    if !emit_hyperlinks {
        return path.to_string();
    }
    let abs = base.join(path);
    let uri = format!("file://{}", encode_file_path(&abs));
    render_link(path, &uri, true)
}

/// Percent-encode a file path for use in a `file://` URI.
/// Preserves `/` separators and unreserved characters; encodes everything else.
fn encode_file_path(path: &Path) -> String {
    let mut out = String::new();
    for b in path.as_os_str().as_encoded_bytes() {
        match b {
            b'/' => out.push('/'),
            b'A'..=b'Z'
            | b'a'..=b'z'
            | b'0'..=b'9'
            | b'-'
            | b'.'
            | b'_'
            | b'~'
            | b':'
            | b'@'
            | b'!'
            | b'$'
            | b'&'
            | b'\''
            | b'('
            | b')'
            | b'*'
            | b'+'
            | b','
            | b';'
            | b'=' => out.push(*b as char),
            _ => {
                use std::fmt::Write;
                let _ = write!(out, "%{:02X}", b);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hyperlink_wraps_with_osc8() {
        let result = render_link("docs/getting-started.md", "file://docs/getting-started.md", true);
        assert!(result.starts_with("\x1b]8;;"));
        assert!(result.contains("docs/getting-started.md"));
        assert!(result.ends_with("\x1b]8;;\x1b\\"));
    }

    #[test]
    fn plain_fallback_returns_visible_only() {
        let result = render_link("docs/getting-started.md", "file://docs/getting-started.md", false);
        assert_eq!(result, "docs/getting-started.md");
        assert!(!result.contains('\x1b'));
    }

    #[test]
    fn path_link_uses_file_uri() {
        let base = Path::new("/home/user");
        let result = render_path_link("docs/readme.md", Some(base), true);
        assert!(result.contains("file:///home/user/docs/readme.md"));
        assert!(result.contains("docs/readme.md"));
    }

    #[test]
    fn path_link_plain_fallback() {
        let base = Path::new("/home/user");
        let result = render_path_link("docs/readme.md", Some(base), false);
        assert_eq!(result, "docs/readme.md");
        assert!(!result.contains('\x1b'));
    }

    #[test]
    fn path_link_absolute_path_passthrough() {
        let base = Path::new("/repo");
        let result = render_path_link("/etc/hosts", Some(base), true);
        assert!(result.contains("file:///etc/hosts"));
        assert!(result.contains("/etc/hosts"));
    }

    #[test]
    fn path_link_no_base_returns_plain() {
        let result = render_path_link("projects/demo", None, true);
        assert_eq!(result, "projects/demo");
        assert!(!result.contains('\x1b'));
    }

    #[test]
    fn visible_text_is_always_present() {
        for emit in [true, false] {
            let result = render_link("click here: https://example.com", "https://example.com", emit);
            assert!(result.contains("click here: https://example.com"));
        }
    }

    #[test]
    fn encode_file_path_spaces() {
        let result = encode_file_path(Path::new("/tmp/my file.md"));
        assert_eq!(result, "/tmp/my%20file.md");
    }

    #[test]
    fn encode_file_path_unicode() {
        let result = encode_file_path(Path::new("/tmp/设计.md"));
        // Each UTF-8 byte of the CJK chars is percent-encoded
        assert!(result.starts_with("/tmp/"));
        assert!(result.ends_with(".md"));
        assert!(!result.contains("设计"), "unicode chars must be percent-encoded");
    }

    #[test]
    fn path_link_encodes_spaces_in_uri() {
        let base = Path::new("/home/user");
        let result = render_path_link("my docs/read me.md", Some(base), true);
        assert!(result.contains("file:///home/user/my%20docs/read%20me.md"));
        // visible text is unchanged
        assert!(result.contains("my docs/read me.md"));
    }
}
