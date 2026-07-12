//! Shared Markdown helpers: fenced-code-block state tracking, front-matter
//! stripping, and link/image-target scanning and rewriting.
//!
//! Build and template-block splitting both need to know whether a line
//! sits inside a code fence (``` or ~~~) so they can skip rewriting or
//! heading detection within those blocks.
//!
//! The scanner implements a simple CommonMark-compatible fence detector:
//! - A line starting with ≥3 backticks or ≥3 tildes opens a fence.
//! - The closing fence must match the opening char and have at least as
//!   many characters as the open fence.
//! - Fences cannot be indented more than 3 spaces (CommonMark spec §4.4).
//! - An unterminated fence is treated as open to end of input.
//!
//! Build (depth-rewriting assets for the output location) and publish
//! (SVG→PNG payload substitution) both need to scan Markdown link/image
//! targets outside fenced blocks; [`rewrite_references`] provides that
//! scanning with a pluggable per-target mapping callback so each caller
//! supplies its own rewrite policy.

use std::path::{Component, Path, PathBuf};

/// Whether the current line is inside a fenced code block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FenceStatus {
    /// Line is outside any fenced block.
    Outside,
    /// Line is inside a fenced code block.
    Inside,
}

/// Tracks fenced-code-block state across lines.
///
/// Feed lines via [`process_line`](FenceTracker::process_line) and
/// inspect whether each line is inside a fence.
///
/// # Example
///
/// ```ignore
/// use crate::service::util::markdown::FenceTracker;
///
/// let mut tracker = FenceTracker::new();
/// for line in text.lines() {
///     let status = tracker.process_line(line);
///     if matches!(status, FenceStatus::Outside) && line.starts_with("## ") {
///         // This is a real heading, not a fenced one
///     }
/// }
/// ```
pub struct FenceTracker {
    /// None = outside a fence; Some((fence_char, fence_len)) = inside.
    open_fence: Option<(char, usize)>,
}

impl FenceTracker {
    pub fn new() -> Self {
        Self { open_fence: None }
    }

    /// Feed a single line and return its fence status.
    ///
    /// Lines that open or close a fence are classified as `Inside`.
    /// (The fence markers themselves are part of the code block.)
    pub fn process_line(&mut self, line: &str) -> FenceStatus {
        let trimmed = line.trim_start();
        let indent = line.len() - trimmed.len();

        // Only lines indented ≤3 spaces can be fence markers (CommonMark §4.4).
        if indent <= 3 {
            if let Some((fc, flen)) = self.detect_fence(trimmed) {
                match self.open_fence {
                    // Not inside a fence — open one.
                    None => {
                        self.open_fence = Some((fc, flen));
                        return FenceStatus::Inside;
                    }
                    // Inside a fence of the same char — close if long enough.
                    Some((open_char, open_len)) if fc == open_char && flen >= open_len => {
                        self.open_fence = None;
                        return FenceStatus::Inside;
                    }
                    // Different char or too-short fence — still inside.
                    _ => {}
                }
            }
        }

        if self.open_fence.is_some() {
            FenceStatus::Inside
        } else {
            FenceStatus::Outside
        }
    }

    /// Returns true if currently inside a fence.
    #[allow(dead_code)]
    pub fn is_inside(&self) -> bool {
        self.open_fence.is_some()
    }

    /// Attempt to detect a code-fence marker at the start of `trimmed`.
    ///
    /// Returns `Some((fence_char, fence_len))` if the line is a valid
    /// opening or closing fence:
    /// - 3+ of the same char (`` ` `` or `~`).
    /// - Only the fence chars, optional trailing spaces, and an optional
    ///   trailing info string (preceded by a space; backtick-only).
    ///
    /// Returns `None` for everything else.
    fn detect_fence(&self, trimmed: &str) -> Option<(char, usize)> {
        let fc = trimmed.chars().next()?;
        if fc != '`' && fc != '~' {
            return None;
        }

        let flen = trimmed.chars().take_while(|&c| c == fc).count();
        if flen < 3 {
            return None;
        }

        let remainder = &trimmed[flen..];

        match fc {
            // Tildes: nothing but trailing spaces allowed after the fence chars.
            '~' => {
                if remainder.chars().all(|c| c.is_ascii_whitespace()) {
                    Some(('~', flen))
                } else {
                    None
                }
            }
            // Backticks: trailing spaces + optional info string allowed.
            '`' => {
                let rest = remainder.trim_end_matches(|c: char| c.is_ascii_whitespace());
                // Info string: must not contain any backtick.
                if rest.is_empty() || rest.chars().all(|c| c != '`') {
                    Some(('`', flen))
                } else {
                    None
                }
            }
            _ => unreachable!(),
        }
    }
}

impl Default for FenceTracker {
    fn default() -> Self {
        Self::new()
    }
}

/// Remove Typora-only metadata from generated build content.
///
/// Source files keep the `typora-copy-images-to` key for editor convenience,
/// but build artifacts should not publish that local editor setting.
pub fn strip_typora_front_matter(content: &str) -> String {
    if let Some((front, body, eol)) = split_initial_yaml_front_matter(content) {
        let mut kept = String::new();
        let mut removed = false;

        for line in front.split_inclusive('\n') {
            if is_typora_copy_images_to_line(line) {
                removed = true;
            } else {
                kept.push_str(line);
            }
        }

        if !removed {
            return content.to_string();
        }

        if kept.lines().any(|line| !line.trim().is_empty()) {
            let mut result = String::new();
            result.push_str("---");
            result.push_str(eol);
            result.push_str(&kept);
            if !kept.is_empty() && !kept.ends_with('\n') {
                result.push_str(eol);
            }
            result.push_str("---");
            result.push_str(eol);
            result.push_str(body);
            return result;
        }

        return body.strip_prefix(eol).unwrap_or(body).to_string();
    }

    content.to_string()
}

fn split_initial_yaml_front_matter(content: &str) -> Option<(&str, &str, &'static str)> {
    let (opening_len, eol) = if content.starts_with("---\r\n") {
        (5, "\r\n")
    } else if content.starts_with("---\n") {
        (4, "\n")
    } else {
        return None;
    };

    let remaining = &content[opening_len..];
    let mut offset = 0;
    for line in remaining.split_inclusive('\n') {
        let line_body = line.trim_end_matches(['\r', '\n']);
        let next_offset = offset + line.len();
        if line_body == "---" {
            let front = &remaining[..offset];
            let body = &remaining[next_offset..];
            return Some((front, body, eol));
        }
        offset = next_offset;
    }

    let trailing = &remaining[offset..];
    if trailing == "---" {
        let front = &remaining[..offset];
        return Some((front, "", eol));
    }

    None
}

fn is_typora_copy_images_to_line(line: &str) -> bool {
    let line = line.trim_start().trim_end_matches(['\r', '\n']);
    line.starts_with("typora-copy-images-to:")
}

/// Determine whether a link/image target should be rewritten at all.
///
/// Absolute paths, URLs, `mailto:`/`data:` URIs, and anchors are left
/// untouched by every caller (build's depth-rewrite, publish's SVG→PNG
/// substitution).
pub fn should_rewrite_target(target: &str) -> bool {
    if target.is_empty() {
        return false;
    }
    if target.starts_with('/') || target.starts_with("http://") || target.starts_with("https://") {
        return false;
    }
    if target.starts_with("mailto:") || target.starts_with("data:") || target.starts_with('#') {
        return false;
    }
    true
}

/// Lexically normalise a path: drop `.` components and collapse `foo/..` pairs
/// without touching the filesystem. Leading `..` (escaping the base) are kept.
pub fn normalize_lexical(path: &Path) -> PathBuf {
    let mut out: Vec<Component<'_>> = Vec::new();
    for comp in path.components() {
        match comp {
            Component::CurDir => {}
            Component::ParentDir => {
                if matches!(out.last(), Some(Component::Normal(_))) {
                    out.pop();
                } else {
                    out.push(comp);
                }
            }
            other => out.push(other),
        }
    }
    out.iter().collect()
}

/// Compute a relative path from `from` to `to`. Returns `None` if impossible,
/// including when `from` and `to` disagree on absoluteness (Bug #22: mixing
/// an absolute path with a relative one produces a malformed concatenation
/// rather than a valid relative path, so callers must not attempt it).
pub fn relative_path_from(from: &Path, to: &Path) -> Option<String> {
    if from.is_absolute() != to.is_absolute() {
        return None;
    }

    let from_comps: Vec<Component<'_>> = from.components().collect();
    let to_comps: Vec<Component<'_>> = to.components().collect();

    // Find common prefix length
    let common_len = from_comps.iter().zip(to_comps.iter()).take_while(|(a, b)| a == b).count();
    if from.is_absolute() {
        let anchor_len = absolute_anchor_len(&from_comps).max(absolute_anchor_len(&to_comps));
        if common_len <= anchor_len {
            return None;
        }
    }

    let up_count = from_comps.len() - common_len;
    let mut parts: Vec<&str> = vec![".."; up_count];
    for comp in &to_comps[common_len..] {
        if let Some(s) = comp.as_os_str().to_str() {
            parts.push(s);
        }
    }
    if parts.is_empty() {
        return Some(".".to_string());
    }
    Some(parts.join("/"))
}

fn absolute_anchor_len(comps: &[Component<'_>]) -> usize {
    comps.iter().take_while(|comp| matches!(comp, Component::Prefix(_) | Component::RootDir)).count()
}

/// Resolve a relative link/image `target` (interpreted relative to
/// `source_dir`) and re-express it relative to `base_dir` — an already
/// lexically-normalised output directory. This is the depth-rewrite shared by
/// build output generation and article merge.
///
/// Returns `None` when `target` must be left untouched: either it is not a
/// rewritable relative reference ([`should_rewrite_target`]) or no valid
/// relative path exists between the two bases (mixed absolute/relative bases,
/// Bug #22). Callers that need to warn specifically on the mixed-base case
/// should gate on [`should_rewrite_target`] themselves first, so an
/// intentionally-skipped absolute/URL target is not mistaken for a failure.
pub fn rebase_relative_target(target: &str, source_dir: &Path, base_dir: &Path) -> Option<String> {
    if !should_rewrite_target(target) {
        return None;
    }
    let resolved = normalize_lexical(&source_dir.join(target));
    relative_path_from(base_dir, &resolved)
}

/// Rewrite every link/image/reference-definition target in `content` using
/// `map_target`. Skips fenced code blocks entirely. `map_target` receives the
/// raw target string and returns `Some(new_target)` to replace it, or `None`
/// to leave the original reference untouched.
pub fn rewrite_references(content: &str, mut map_target: impl FnMut(&str) -> Option<String>) -> String {
    let mut result = String::with_capacity(content.len());
    let mut fence = FenceTracker::new();

    for line in content.lines() {
        let inside_fence = matches!(fence.process_line(line), FenceStatus::Inside);

        if inside_fence {
            result.push_str(line);
            result.push('\n');
            continue;
        }

        let rewritten = rewrite_line_references(line, &mut map_target);
        result.push_str(&rewritten);
        result.push('\n');
    }
    result
}

/// Rewrite targets in a single Markdown line via `map_target`.
/// Handles inline images `![alt](path)`, inline links `[text](path)`,
/// reference definitions `[id]: path`, and HTML `<img src="path">` tags.
fn rewrite_line_references(line: &str, map_target: &mut impl FnMut(&str) -> Option<String>) -> String {
    // Reference definitions (`[id]: path`) are whole-line constructs and never
    // contain an inline `](`, so dispatch on them first and return. (HTML
    // `<img>` tags cannot appear in a reference-definition line either.)
    let trimmed = line.trim_start();
    if trimmed.starts_with('[') && trimmed.contains("]: ") {
        if let Some(bracket_end) = trimmed.find(']') {
            let after = &trimmed[bracket_end + 1..];
            if let Some(after_colon) = after.strip_prefix(": ") {
                let target = after_colon.split_whitespace().next().unwrap_or("");
                if let Some(new_target) = map_target(target) {
                    let indent = &line[..line.len() - trimmed.len()];
                    let rest = &after[2 + target.len()..];
                    return format!("{indent}[{}]: {new_target}{rest}", &trimmed[1..bracket_end]);
                }
            }
        }
        return line.to_string();
    }

    let rewritten = rewrite_markdown_brackets(line, map_target);
    rewrite_html_img_src(&rewritten, map_target)
}

/// Rewrite ONLY the `(path)` portion of inline Markdown images/links
/// (`![alt](path)` / `[text](path)`), copying the surrounding text (incl. the
/// `![alt]`/`[text]` bracket) verbatim.
fn rewrite_markdown_brackets(line: &str, map_target: &mut impl FnMut(&str) -> Option<String>) -> String {
    let bytes = line.as_bytes();
    let mut result = String::with_capacity(line.len());
    // `last` marks how far of `line` has already been flushed into `result`.
    let mut last = 0usize;
    let mut seen_open = false;
    let mut i = 0usize;
    while i + 1 < bytes.len() {
        match bytes[i] {
            b'[' => seen_open = true,
            // A real link/image `]` must be preceded by a `[` and followed by `(`.
            b']' if seen_open && bytes[i + 1] == b'(' => {
                let paren_open = i + 1;
                if let Some(rel_end) = line[paren_open + 1..].find(')') {
                    let target = &line[paren_open + 1..paren_open + 1 + rel_end];
                    if let Some(new_target) = map_target(target) {
                        // Flush verbatim up to and including `(`, then the new
                        // target, then `)`.
                        result.push_str(&line[last..=paren_open]);
                        result.push_str(&new_target);
                        result.push(')');
                        i = paren_open + 1 + rel_end + 1; // past the closing `)`
                        last = i;
                        continue;
                    }
                }
            }
            _ => {}
        }
        i += 1;
    }
    if last == 0 {
        return line.to_string(); // nothing rewritten — avoid a redundant copy
    }
    result.push_str(&line[last..]);
    result
}

/// Rewrite `src="path"` / `src='path'` attributes inside `<img ...>` tags on
/// a single line via `map_target`. Line-scoped (does not model a full HTML
/// parser); a tag split across multiple lines is left untouched.
fn rewrite_html_img_src(line: &str, map_target: &mut impl FnMut(&str) -> Option<String>) -> String {
    if !line.contains("<img") {
        return line.to_string();
    }
    let mut result = String::with_capacity(line.len());
    let mut rest = line;
    loop {
        let Some(tag_pos) = rest.find("<img") else {
            result.push_str(rest);
            break;
        };
        result.push_str(&rest[..tag_pos]);
        let from_tag = &rest[tag_pos..];
        let Some(tag_end) = from_tag.find('>') else {
            result.push_str(from_tag);
            break;
        };
        result.push_str(&rewrite_img_tag_src(&from_tag[..=tag_end], map_target));
        rest = &from_tag[tag_end + 1..];
    }
    result
}

/// Rewrite the `src="..."`/`src='...'` attribute value inside a single
/// `<img ...>` tag (including the trailing `>`).
fn rewrite_img_tag_src(tag: &str, map_target: &mut impl FnMut(&str) -> Option<String>) -> String {
    let Some(src_pos) = tag.find("src=") else {
        return tag.to_string();
    };
    let after_src = &tag[src_pos + 4..];
    let Some(quote) = after_src.chars().next().filter(|c| *c == '"' || *c == '\'') else {
        return tag.to_string();
    };
    let Some(close_rel) = after_src[1..].find(quote) else {
        return tag.to_string();
    };
    let target = &after_src[1..1 + close_rel];
    let Some(new_target) = map_target(target) else {
        return tag.to_string();
    };
    let mut out = String::with_capacity(tag.len());
    out.push_str(&tag[..src_pos + 4]);
    out.push(quote);
    out.push_str(&new_target);
    out.push(quote);
    out.push_str(&after_src[1 + close_rel + 1..]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: collect status of each line as a vec of bool (true = inside).
    fn inside_map(text: &str) -> Vec<bool> {
        let mut ft = FenceTracker::new();
        text.lines().map(|line| matches!(ft.process_line(line), FenceStatus::Inside)).collect()
    }

    // ----- basic fence -----

    #[test]
    fn simple_backtick_fence() {
        let result = inside_map("before\n```\ninside\n```\nafter");
        assert_eq!(result, vec![false, true, true, true, false]);
    }

    #[test]
    fn simple_tilde_fence() {
        let result = inside_map("before\n~~~\ninside\n~~~\nafter");
        assert_eq!(result, vec![false, true, true, true, false]);
    }

    // ----- longer fence runs -----

    #[test]
    fn longer_backtick_fence() {
        // Open with 4 backticks, close with 4+.
        let result = inside_map("````\nfenced\n````\noutside");
        assert_eq!(result, vec![true, true, true, false]);
    }

    #[test]
    fn longer_open_short_close_does_not_close() {
        // Open with 4, close with 3 is not enough — stays inside.
        let result = inside_map("````\na\n```\nb");
        assert_eq!(result, vec![true, true, true, true]);
    }

    #[test]
    fn longer_close_than_open_is_fine() {
        // Open with 3, close with 5 is fine.
        let result = inside_map("```\na\n`````\nb");
        assert_eq!(result, vec![true, true, true, false]);
    }

    // ----- nested / mixed fences -----

    #[test]
    fn backtick_inside_tilde_stays_open() {
        // A ``` line inside a ~~~ fence is just body text.
        let result = inside_map("~~~\nbody\n```\nmore\n~~~");
        assert_eq!(result, vec![true, true, true, true, true]);
    }

    #[test]
    fn tilde_inside_backtick_stays_open() {
        let result = inside_map("```\nbody\n~~~\nmore\n```");
        assert_eq!(result, vec![true, true, true, true, true]);
    }

    // ----- indentation -----

    #[test]
    fn indented_4_spaces_is_not_fence() {
        // CommonMark: a fence cannot be indented more than 3 spaces.
        let result = inside_map("    ```\nnot fenced\n    ```");
        assert_eq!(result, vec![false, false, false]);
    }

    #[test]
    fn indented_3_spaces_is_fence() {
        let result = inside_map("   ```\ninside\n   ```\noutside");
        assert_eq!(result, vec![true, true, true, false]);
    }

    // ----- unterminated fence -----

    #[test]
    fn unterminated_fence_stays_open() {
        let result = inside_map("```\na\nb");
        assert_eq!(result, vec![true, true, true]);
    }

    // ----- info strings on backtick fences -----

    #[test]
    fn backtick_fence_with_info_string() {
        let result = inside_map("```rust\nfn main() {}\n```");
        assert_eq!(result, vec![true, true, true]);
    }

    #[test]
    fn backtick_fence_with_leading_spaces_and_info() {
        let result = inside_map("  ``` rust,ignore\ncode\n```");
        assert_eq!(result, vec![true, true, true]);
    }

    // ----- tildes do not accept info strings -----

    #[test]
    fn tilde_with_extra_text_is_not_fence() {
        // ~~~ followed by non-whitespace is not a valid fence.
        let result = inside_map("~~~rust\nthis is not a fence\n~~~");
        // The first line is NOT a fence, so we're always outside.
        // The last ~~~ opens a fence.
        assert_eq!(result, vec![false, false, true]);
    }

    // ----- backtick line with backticks in it is not a fence -----

    #[test]
    fn backtick_in_info_string_is_not_fence() {
        // "``` `" has a backtick after the fence chars → not a fence.
        let result = inside_map("``` `\ncode\n```");
        assert_eq!(result, vec![false, false, true]);
    }

    // ----- fence preceded by non-whitespace is not a fence -----

    #[test]
    fn fence_chars_mid_line_are_not_fence() {
        let result = inside_map("text ```\nnot a fence\n```");
        assert_eq!(result, vec![false, false, true]);
    }

    // ── Typora front-matter stripping (moved from service::build, spec 064) ──

    #[test]
    fn strip_typora_removes_only_that_key() {
        let content = "---\ntitle: Foo\ntypora-copy-images-to: ../assets\n---\nbody\n";
        let out = strip_typora_front_matter(content);
        assert_eq!(out, "---\ntitle: Foo\n---\nbody\n");
    }

    #[test]
    fn strip_typora_noop_without_frontmatter() {
        assert_eq!(strip_typora_front_matter("no frontmatter here\n"), "no frontmatter here\n");
    }

    #[test]
    fn strip_typora_noop_without_the_key() {
        let content = "---\ntitle: Foo\n---\nbody\n";
        assert_eq!(strip_typora_front_matter(content), content);
    }

    #[test]
    fn strip_typora_drops_frontmatter_entirely_when_key_is_only_content() {
        let content = "---\ntypora-copy-images-to: ../assets\n---\nbody\n";
        assert_eq!(strip_typora_front_matter(content), "body\n");
    }

    // ── Path-rewrite helpers (moved from service::build, spec 059/064) ──────

    #[test]
    fn normalize_lexical_collapses_interior_dotdot() {
        assert_eq!(normalize_lexical(Path::new("docs/slug/../../assets/x.png")), Path::new("assets/x.png"));
        assert_eq!(normalize_lexical(Path::new("docs/art/./assets/y.png")), Path::new("docs/art/assets/y.png"));
        // Leading `..` that escapes the base is preserved.
        assert_eq!(normalize_lexical(Path::new("../shared/z.png")), Path::new("../shared/z.png"));
    }

    #[test]
    fn relative_path_from_computes_up_and_down() {
        assert_eq!(relative_path_from(Path::new("outputs"), Path::new("assets/x.png")).unwrap(), "../assets/x.png");
        assert_eq!(relative_path_from(Path::new("a/b"), Path::new("a/b")).unwrap(), ".");
        assert_eq!(relative_path_from(Path::new("a/b/out"), Path::new("a/b/c/d.png")).unwrap(), "../c/d.png");
    }

    #[test]
    fn relative_path_from_rejects_mixed_absolute_and_relative_bases() {
        // Bug #22: mixing an absolute `from`/`to` with a relative counterpart
        // must never produce a "..".."/"/abs/path" concatenation.
        assert_eq!(relative_path_from(Path::new("/abs/outputs"), Path::new("assets/x.png")), None);
        assert_eq!(relative_path_from(Path::new("outputs"), Path::new("/abs/assets/x.png")), None);
        // Absolute + absolute still works normally.
        assert_eq!(
            relative_path_from(Path::new("/repo/outputs"), Path::new("/repo/assets/x.png")).unwrap(),
            "../assets/x.png"
        );
    }

    #[test]
    fn relative_path_from_rejects_root_only_absolute_common_prefix() {
        assert_eq!(relative_path_from(Path::new("/tmp/out"), Path::new("/Users/me/repo/assets/x.png")), None);
    }

    #[test]
    fn should_rewrite_target_skips_absolute_and_urls() {
        for t in
            ["/abs/x.png", "http://x/y.png", "https://x/y.png", "mailto:a@b.com", "data:image/png,AA", "#anchor", ""]
        {
            assert!(!should_rewrite_target(t), "must not rewrite {t:?}");
        }
        assert!(should_rewrite_target("assets/p.png"));
    }

    // ── Generic reference-scanning (moved from service::build, spec 059/064) ─

    /// Test-only alias for the production depth-rewrite, used to exercise the
    /// generic scanner end-to-end with realistic behavior.
    fn depth_rewrite_for_test(target: &str, source_dir: &Path, base_dir: &Path) -> Option<String> {
        rebase_relative_target(target, source_dir, base_dir)
    }

    #[test]
    fn rewrite_line_preserves_alt_text_and_rewrites_only_path() {
        let src = Path::new("docs/art");
        let base = Path::new("outputs");
        // Bug 1A: CJK alt text is preserved, never nested inside another `![`.
        let out =
            rewrite_line_references("![工作流全景](assets/p.png)", &mut |t| depth_rewrite_for_test(t, src, base));
        assert_eq!(out, "![工作流全景](../docs/art/assets/p.png)");
        assert!(!out.contains("![工作流全景!["), "alt text must not be nested: {out}");
        assert_eq!(out.matches("](").count(), 1, "exactly one link: {out}");
    }

    #[test]
    fn rewrite_line_handles_multiple_links_on_one_line() {
        let src = Path::new("docs/art");
        let base = Path::new("outputs");
        let out =
            rewrite_line_references("see ![a](x.png) and [b](y.md) too", &mut |t| depth_rewrite_for_test(t, src, base));
        assert_eq!(out, "see ![a](../docs/art/x.png) and [b](../docs/art/y.md) too");
    }

    #[test]
    fn rewrite_line_rewrites_reference_definition() {
        let src = Path::new("docs/art");
        let base = Path::new("outputs");
        assert_eq!(
            rewrite_line_references("[img]: assets/p.png", &mut |t| depth_rewrite_for_test(t, src, base)),
            "[img]: ../docs/art/assets/p.png"
        );
        // Indentation and trailing title are preserved.
        assert_eq!(
            rewrite_line_references("  [img]: assets/p.png \"Title\"", &mut |t| depth_rewrite_for_test(t, src, base)),
            "  [img]: ../docs/art/assets/p.png \"Title\""
        );
    }

    #[test]
    fn rewrite_line_leaves_non_links_untouched() {
        let src = Path::new("docs/art");
        let base = Path::new("outputs");
        let map = &mut |t: &str| depth_rewrite_for_test(t, src, base);
        // No link at all.
        assert_eq!(rewrite_line_references("plain text, no links here", map), "plain text, no links here");
        // A `]` with no preceding `[` is not a link.
        assert_eq!(rewrite_line_references("stray ](x.png) bracket", map), "stray ](x.png) bracket");
        // Absolute / URL targets are left verbatim.
        assert_eq!(rewrite_line_references("[a](https://x/y) ![b](/abs.png)", map), "[a](https://x/y) ![b](/abs.png)");
    }

    #[test]
    fn rewrite_references_skips_fenced_code_blocks() {
        let src = Path::new("docs/art");
        let base = Path::new("outputs");
        let content = "before ![a](x.png)\n```\n![b](y.png)\n```\nafter ![c](z.png)\n";
        let out = rewrite_references(content, |t| depth_rewrite_for_test(t, src, base));
        assert_eq!(out, "before ![a](../docs/art/x.png)\n```\n![b](y.png)\n```\nafter ![c](../docs/art/z.png)\n");
    }

    // ── HTML `<img src>` rewriting (spec 064, FR-001) ────────────────────────

    #[test]
    fn rewrite_html_img_src_double_quotes() {
        let src = Path::new("docs/art");
        let base = Path::new("outputs");
        let out = rewrite_line_references(r#"<img src="assets/p.png" alt="pic">"#, &mut |t| {
            depth_rewrite_for_test(t, src, base)
        });
        assert_eq!(out, r#"<img src="../docs/art/assets/p.png" alt="pic">"#);
    }

    #[test]
    fn rewrite_html_img_src_single_quotes() {
        let src = Path::new("docs/art");
        let base = Path::new("outputs");
        let out = rewrite_line_references("<img src='assets/p.png'>", &mut |t| depth_rewrite_for_test(t, src, base));
        assert_eq!(out, "<img src='../docs/art/assets/p.png'>");
    }

    #[test]
    fn rewrite_html_img_src_preserves_absolute_and_urls() {
        let src = Path::new("docs/art");
        let base = Path::new("outputs");
        let map = &mut |t: &str| depth_rewrite_for_test(t, src, base);
        assert_eq!(rewrite_line_references(r#"<img src="/abs/logo.png">"#, map), r#"<img src="/abs/logo.png">"#);
        assert_eq!(rewrite_line_references(r#"<img src="https://x/y.png">"#, map), r#"<img src="https://x/y.png">"#);
    }

    #[test]
    fn rewrite_html_img_src_alongside_markdown_image_on_same_line() {
        let src = Path::new("docs/art");
        let base = Path::new("outputs");
        let out = rewrite_line_references(r#"![a](x.png) and <img src="y.png">"#, &mut |t| {
            depth_rewrite_for_test(t, src, base)
        });
        assert_eq!(out, r#"![a](../docs/art/x.png) and <img src="../docs/art/y.png">"#);
    }

    #[test]
    fn rewrite_html_img_src_multiple_tags_on_one_line() {
        let src = Path::new("docs/art");
        let base = Path::new("outputs");
        let out = rewrite_line_references(r#"<img src="a.png"> text <img src="b.png">"#, &mut |t| {
            depth_rewrite_for_test(t, src, base)
        });
        assert_eq!(out, r#"<img src="../docs/art/a.png"> text <img src="../docs/art/b.png">"#);
    }

    #[test]
    fn rewrite_html_img_src_no_tag_returns_original() {
        let mut map = |_: &str| Some("unused".to_string());
        assert_eq!(rewrite_line_references("plain text", &mut map), "plain text");
    }
}
