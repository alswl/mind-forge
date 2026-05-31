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
/// `file://` OSC 8 link with the path as both target and label.
#[allow(dead_code)]
pub fn render_path_link(path: &str, emit_hyperlinks: bool) -> String {
    if !emit_hyperlinks {
        return path.to_string();
    }
    let uri = format!("file://{path}");
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
        let result = render_path_link("/home/user/docs/readme.md", true);
        assert!(result.contains("file:///home/user/docs/readme.md"));
        assert!(result.contains("/home/user/docs/readme.md"));
    }

    #[test]
    fn path_link_plain_fallback() {
        let result = render_path_link("/home/user/docs/readme.md", false);
        assert_eq!(result, "/home/user/docs/readme.md");
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
