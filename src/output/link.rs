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

/// Wrap a file path for display. When hyperlinks are enabled, emits a
/// `file://` OSC 8 link. `path` is a repo-relative path; `base_dir` is the
/// repo root used to resolve it to an absolute `file://` URI. The visible
/// label always shows the original relative path for readability.
#[allow(dead_code)]
pub fn render_path_link(path: &str, base_dir: &Path, emit_hyperlinks: bool) -> String {
    if !emit_hyperlinks {
        return path.to_string();
    }
    let abs = base_dir.join(path);
    let uri = format!("file://{}", abs.display());
    render_link(path, &uri, emit_hyperlinks)
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
        let result = render_path_link("docs/readme.md", base, true);
        assert!(result.contains("file:///home/user/docs/readme.md"));
        assert!(result.contains("docs/readme.md"));
    }

    #[test]
    fn path_link_plain_fallback() {
        let base = Path::new("/home/user");
        let result = render_path_link("docs/readme.md", base, false);
        assert_eq!(result, "docs/readme.md");
        assert!(!result.contains('\x1b'));
    }

    #[test]
    fn path_link_absolute_path_passthrough() {
        let base = Path::new("/repo"); // ignored when path is absolute
        let result = render_path_link("/etc/hosts", base, true);
        assert!(result.contains("file:///etc/hosts"));
        assert!(result.contains("/etc/hosts"));
    }

    #[test]
    fn path_link_empty_base() {
        let base = Path::new("");
        let result = render_path_link("projects/demo", base, true);
        // With an empty base, join treats path as relative to cwd-equivalent
        assert!(result.contains("file://projects/demo"));
        assert!(result.contains("projects/demo"));
    }

    #[test]
    fn visible_text_is_always_present() {
        for emit in [true, false] {
            let result = render_link("click here: https://example.com", "https://example.com", emit);
            assert!(result.contains("click here: https://example.com"));
        }
    }
}
