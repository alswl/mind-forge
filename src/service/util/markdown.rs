//! Shared Markdown helpers: fenced-code-block state tracking.
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
}
