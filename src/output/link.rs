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
    let uri = format!("file://{}", abs.display());
    render_link(path, &uri, true)
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
        let base = Path::new("/repo"); // ignored when path is absolute
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
}
