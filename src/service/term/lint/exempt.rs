#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScanCursor {
    Body,
    FencedCodeBacktick,
    FencedCodeTilde,
    InlineCode,
    HtmlComment,
    LinkUrl,
    BareUrl,
    BlockExempt,
}

/// Single-pass byte-level state machine that replaces exempt regions with `\0`.
/// Output is the same length as `content.as_bytes()`.
pub(crate) fn strip_exempt_regions(content: &str, fm_end: Option<usize>) -> Vec<u8> {
    let bytes = content.as_bytes();
    let len = bytes.len();
    let mut out = vec![0u8; len];
    let mut state = ScanCursor::Body;

    let start_offset = fm_end.unwrap_or_default();

    let mut i = start_offset;
    while i < len {
        match state {
            ScanCursor::Body => {
                if i + 3 < len
                    && bytes[i] == b'<'
                    && bytes[i + 1] == b'!'
                    && bytes[i + 2] == b'-'
                    && bytes[i + 3] == b'-'
                {
                    let is_line_start = i == start_offset || bytes[i - 1] == b'\n';
                    if is_line_start {
                        if let Some(comment_end) = find_comment_close(bytes, i + 4) {
                            if content[i + 4..comment_end].trim() == "mf-term-lint:off" {
                                let end = (comment_end + 3).min(len);
                                out[i..end].copy_from_slice(&bytes[i..end]);
                                i = comment_end + 3;
                                state = ScanCursor::BlockExempt;
                                continue;
                            }
                        }
                    }
                    state = ScanCursor::HtmlComment;
                    out[i] = bytes[i];
                    i += 1;
                    continue;
                }

                if (bytes[i] == b'`' || bytes[i] == b'~') && is_line_start_pos(bytes, i, start_offset) {
                    let fence_len = count_repeated(&bytes[i..], bytes[i]);
                    if fence_len >= 3 {
                        state = match bytes[i] {
                            b'`' => ScanCursor::FencedCodeBacktick,
                            _ => ScanCursor::FencedCodeTilde,
                        };
                        let end = (i + fence_len).min(len);
                        out[i..end].copy_from_slice(&bytes[i..end]);
                        i += fence_len;
                        continue;
                    }
                }

                if bytes[i] == b'`' {
                    state = ScanCursor::InlineCode;
                    out[i] = bytes[i];
                    i += 1;
                    continue;
                }

                if i + 1 < len && bytes[i] == b']' && bytes[i + 1] == b'(' {
                    state = ScanCursor::LinkUrl;
                    out[i] = bytes[i];
                    i += 1;
                    continue;
                }

                if starts_with_url_scheme(bytes, i, b"http://") || starts_with_url_scheme(bytes, i, b"https://") {
                    // The scheme's first byte is URL *content* (not a delimiter like
                    // `` ` `` or `<`), so it must be zeroed too. Leaving it visible let a
                    // correction whose `original` shares that leading byte (e.g. `hcs` vs
                    // `https`) match `h\0\0` via the `\0` wildcard in `find_subseq`.
                    state = ScanCursor::BareUrl;
                    i += 1;
                    continue;
                }

                out[i] = bytes[i];
                i += 1;
            }
            ScanCursor::FencedCodeBacktick | ScanCursor::FencedCodeTilde => {
                let fence_char = match state {
                    ScanCursor::FencedCodeBacktick => b'`',
                    _ => b'~',
                };
                if i == start_offset || bytes[i] == b'\n' {
                    let check_start = if bytes[i] == b'\n' { i + 1 } else { i };
                    if check_start < len && bytes[check_start] == fence_char {
                        let fence_len = count_repeated(&bytes[check_start..], fence_char);
                        if fence_len >= 3 {
                            let end = (check_start + fence_len).min(len);
                            out[check_start..end].copy_from_slice(&bytes[check_start..end]);
                            i = check_start + fence_len;
                            state = ScanCursor::Body;
                            continue;
                        }
                    }
                }
                if bytes[i] == b'\r' || bytes[i] == b'\n' {
                    out[i] = bytes[i];
                }
                i += 1;
            }
            ScanCursor::InlineCode => {
                if bytes[i] == b'`' {
                    out[i] = bytes[i];
                    state = ScanCursor::Body;
                    i += 1;
                    continue;
                }
                if bytes[i] == b'\n' {
                    out[i] = bytes[i];
                    state = ScanCursor::Body;
                    i += 1;
                    continue;
                }
                i += 1;
            }
            ScanCursor::HtmlComment => {
                if bytes[i] == b'-' && i + 2 < len && bytes[i + 1] == b'-' && bytes[i + 2] == b'>' {
                    out[i] = bytes[i];
                    out[i + 1] = bytes[i + 1];
                    out[i + 2] = bytes[i + 2];
                    i += 3;
                    state = ScanCursor::Body;
                    continue;
                }
                i += 1;
            }
            ScanCursor::LinkUrl => {
                if bytes[i] == b')' {
                    out[i] = bytes[i];
                    state = ScanCursor::Body;
                    i += 1;
                    continue;
                }
                if bytes[i] == b'\n' {
                    out[i] = bytes[i];
                    state = ScanCursor::Body;
                    i += 1;
                    continue;
                }
                i += 1;
            }
            ScanCursor::BareUrl => {
                if bytes[i] == b' ' || bytes[i] == b'\t' || bytes[i] == b'\n' || bytes[i] == b'\r' {
                    out[i] = bytes[i];
                    state = ScanCursor::Body;
                    i += 1;
                    continue;
                }
                if bytes[i] == b'>' || bytes[i] == b')' {
                    out[i] = bytes[i];
                    state = ScanCursor::Body;
                    i += 1;
                    continue;
                }
                i += 1;
            }
            ScanCursor::BlockExempt => {
                if i + 3 < len
                    && bytes[i] == b'<'
                    && bytes[i + 1] == b'!'
                    && bytes[i + 2] == b'-'
                    && bytes[i + 3] == b'-'
                {
                    let is_line_start = i == start_offset || bytes[i - 1] == b'\n';
                    if is_line_start {
                        if let Some(comment_end) = find_comment_close(bytes, i + 4) {
                            if content[i + 4..comment_end].trim() == "mf-term-lint:on" {
                                let end = (comment_end + 3).min(len);
                                out[i..end].copy_from_slice(&bytes[i..end]);
                                i = comment_end + 3;
                                state = ScanCursor::Body;
                                continue;
                            }
                        }
                    }
                }
                if bytes[i] == b'\n' || bytes[i] == b'\r' {
                    out[i] = bytes[i];
                }
                i += 1;
            }
        }
    }

    out
}

fn starts_with_url_scheme(bytes: &[u8], i: usize, scheme: &[u8]) -> bool {
    i + scheme.len() < bytes.len() && bytes[i..].starts_with(scheme)
}

fn find_comment_close(bytes: &[u8], start: usize) -> Option<usize> {
    let mut i = start;
    while i + 2 < bytes.len() {
        if bytes[i] == b'-' && bytes[i + 1] == b'-' && bytes[i + 2] == b'>' {
            return Some(i);
        }
        i += 1;
    }
    None
}

fn is_line_start_pos(bytes: &[u8], i: usize, start_offset: usize) -> bool {
    i == start_offset || (i > 0 && bytes[i - 1] == b'\n')
}

fn count_repeated(slice: &[u8], byte: u8) -> usize {
    slice.iter().take_while(|&&b| b == byte).count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::term::lint::front_matter::{parse_front_matter_skip_flag, FrontMatterDecision};

    #[test]
    fn strip_plain_text_preserved() {
        let content = "hello world";
        let result = strip_exempt_regions(content, None);
        assert_eq!(result.len(), content.len());
        assert_eq!(result, content.as_bytes());
    }

    #[test]
    fn strip_fenced_code_block_exempt() {
        let content = "before\n```\ncode mindrepo\n```\nafter";
        let result = strip_exempt_regions(content, None);
        assert_eq!(result.len(), content.len());
        assert_eq!(&result[..7], b"before\n");
        let code_start = 11;
        let code_end = code_start + "code mindrepo".len();
        for &b in &result[code_start..code_end] {
            assert_eq!(b, 0, "code block should be zeroed");
        }
        let after_start = content.rfind("after").unwrap();
        assert_eq!(&result[after_start..], b"after");
    }

    #[test]
    fn strip_inline_code_exempt() {
        let content = "text `code mindrepo` more";
        let result = strip_exempt_regions(content, None);
        assert_eq!(result.len(), content.len());
        let start = content.find('`').unwrap();
        let end = content.rfind('`').unwrap();
        for &b in &result[start + 1..end] {
            assert_eq!(b, 0, "inline code should be zeroed");
        }
    }

    #[test]
    fn strip_link_url_exempt() {
        let content = "[text](https://example.com/mindrepo)";
        let result = strip_exempt_regions(content, None);
        assert_eq!(result.len(), content.len());
        let paren_start = content.find('(').unwrap();
        let paren_end = content.find(')').unwrap();
        for &b in &result[paren_start + 1..paren_end] {
            assert_eq!(b, 0, "link URL should be zeroed");
        }
    }

    #[test]
    fn strip_bare_url_exempt() {
        let content = "visit https://example.com/mindrepo for info";
        let result = strip_exempt_regions(content, None);
        assert_eq!(result.len(), content.len());
        // The entire URL is zeroed, including the scheme's leading byte.
        let url_content_start = content.find("https://").unwrap();
        let url_end = content[url_content_start..].find(' ').map(|p| url_content_start + p).unwrap_or(content.len());
        for &b in &result[url_content_start..url_end] {
            assert_eq!(b, 0, "bare URL should be zeroed");
        }
    }

    #[test]
    fn strip_front_matter_exempt() {
        let content = "---\ntitle: test\n---\nbody mindrepo";
        let fm_result = parse_front_matter_skip_flag(content);
        let fm_end = match fm_result {
            FrontMatterDecision::Present { end_byte_offset } => Some(end_byte_offset),
            _ => None,
        };
        let result = strip_exempt_regions(content, fm_end);
        assert_eq!(result.len(), content.len());
        if let Some(end) = fm_end {
            for &b in &result[..end] {
                assert_eq!(b, 0, "front matter should be zeroed");
            }
        }
        let body_start = content.find("body").unwrap();
        assert_eq!(&result[body_start..], b"body mindrepo");
    }

    #[test]
    fn strip_block_exempt_markers() {
        let content = "before\n<!-- mf-term-lint:off -->\nsecret mindrepo\n<!-- mf-term-lint:on -->\nafter";
        let result = strip_exempt_regions(content, None);
        assert_eq!(result.len(), content.len());
        assert_eq!(&result[..7], b"before\n");
        let after_start = content.rfind("after").unwrap();
        assert_eq!(&result[after_start..], b"after");
        let off_pos = content.find("<!-- mf-term-lint:off -->").unwrap();
        let secret_start = off_pos + "<!-- mf-term-lint:off -->\n".len();
        let on_pos = content.find("<!-- mf-term-lint:on -->").unwrap();
        for &b in &result[secret_start..on_pos] {
            if b != b'\n' {
                assert_eq!(b, 0, "block-exempt region should be zeroed");
            }
        }
    }

    #[test]
    fn strip_output_len_equals_input_len() {
        let cases = vec![
            "plain text",
            "before\n```\ncode\n```\nafter",
            "text `inline` more",
            "a <!-- comment --> b",
            "[link](url) text",
            "visit http://example.com",
            "---\nkey: val\n---\nbody",
            "before\n<!-- mf-term-lint:off -->\nhidden\n<!-- mf-term-lint:on -->\nafter",
        ];
        for content in cases {
            let fm_end = match parse_front_matter_skip_flag(content) {
                FrontMatterDecision::Present { end_byte_offset } => Some(end_byte_offset),
                _ => None,
            };
            let result = strip_exempt_regions(content, fm_end);
            assert_eq!(result.len(), content.len(), "length mismatch for: {content:?}");
        }
    }

    #[test]
    fn fenced_code_prevents_match() {
        let content = "text mindrepo\n```\nmindrepo in code\n```\nmindrepo after";
        let result = strip_exempt_regions(content, None);
        let mid = content.find("```").unwrap();
        let close = content.rfind("```").unwrap();
        let inside_start = mid + 4;
        for &b in &result[inside_start..close] {
            if b != 0 && b != b'\n' {
                panic!("expected zeroed content in code block");
            }
        }
    }

    #[test]
    fn tmux_fence_works() {
        let content = "text\n~~~\nfenced mindrepo\n~~~\nmore";
        let result = strip_exempt_regions(content, None);
        assert_eq!(result.len(), content.len());
        let fence_start = content.find("~~~").unwrap();
        let fence_close = content.rfind("~~~").unwrap();
        let between_start = fence_start + 4;
        for &b in &result[between_start..fence_close] {
            if b != 0 && b != b'\n' {
                panic!("expected zeroed content in tilde-fenced block");
            }
        }
    }
}
