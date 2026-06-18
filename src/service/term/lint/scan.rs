use std::collections::BTreeSet;

use crate::model::term::{CandidateTerm, FixKind, MatchKind, TermFinding};

fn is_ascii_word_string(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| matches!(b, b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'_' | b'-'))
}

fn is_word_boundary_byte(b: u8) -> bool {
    !matches!(b, b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'_')
}

pub(crate) fn is_cjk_ideograph(c: char) -> bool {
    matches!(c,
        '\u{4E00}'..='\u{9FFF}'   // CJK Unified Ideographs
        | '\u{3400}'..='\u{4DBF}' // CJK Extension A
        | '\u{20000}'..='\u{3134F}' // CJK Extension B-I
        | '\u{3040}'..='\u{30FF}' // Hiragana + Katakana
        | '\u{AC00}'..='\u{D7AF}' // Hangul
    )
}

fn char_before(content: &str, byte_offset: usize) -> Option<char> {
    if byte_offset == 0 {
        return None;
    }
    for (i, c) in content.char_indices() {
        let next = i + c.len_utf8();
        if next == byte_offset {
            return Some(c);
        }
        if next > byte_offset {
            break;
        }
    }
    None
}

fn char_after(content: &str, byte_offset: usize) -> Option<char> {
    content[byte_offset..].chars().next()
}

/// Check word boundaries for a match at `offset` with `original_len` bytes.
/// Returns true if the match passes boundary requirements.
fn apply_word_boundary(
    content: &str,
    sanitized: &[u8],
    match_kind: MatchKind,
    original: &str,
    offset: usize,
    original_len: usize,
) -> bool {
    match match_kind {
        MatchKind::Substring => true,
        MatchKind::Word => {
            if is_ascii_word_string(original) {
                let before_ok = offset == 0 || is_word_boundary_byte(sanitized[offset - 1]);
                let end = offset + original_len;
                let after_ok = end >= sanitized.len() || is_word_boundary_byte(sanitized[end]);
                before_ok && after_ok
            } else {
                // CJK word boundary: match only when at least one side is a non-CJK
                // neighbour. Beginning/end of text is not a boundary — a match at
                // position 0 followed by CJK (机器 in 机器人) is embedded.
                let left_neighbor = char_before(content, offset);
                let right_end = offset + original_len;
                let right_neighbor = char_after(content, right_end);
                let left_boundary = left_neighbor.is_some_and(|c| !is_cjk_ideograph(c));
                let right_boundary = right_neighbor.is_some_and(|c| !is_cjk_ideograph(c));
                left_boundary || right_boundary
            }
        }
        MatchKind::Pinyin => unreachable!("pinyin matches are dispatched through the pinyin scanner"),
    }
}

pub(crate) struct InternalFinding {
    pub(crate) path: String,
    pub(crate) byte_offset: usize,
    pub(crate) original_len: usize,
    pub(crate) original: String,
    pub(crate) correct: String,
    pub(crate) is_ambiguous: bool,
    pub(crate) fix_kind: crate::model::term::FixKind,
}

pub(super) fn byte_offset_to_line_col(content: &str, byte_offset: usize) -> (u32, u32) {
    let mut line: u32 = 1;
    let mut col: u32 = 1;
    for (i, c) in content.char_indices() {
        if i >= byte_offset {
            break;
        }
        if c == '\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
    }
    (line, col)
}

pub(crate) struct CorrectionRef<'a> {
    pub original: &'a str,
    pub correct: &'a str,
    pub term_name: &'a str,
    pub description: Option<&'a str>,
    pub confidence: Option<f64>,
    pub is_ambiguous: bool,
    pub candidates: &'a [CandidateTerm],
    pub match_kind: crate::model::term::MatchKind,
    pub fix_kind: crate::model::term::FixKind,
    pub pinyin: Option<&'a str>,
}

pub(crate) fn scan_file_for_corrections(
    content: &str,
    sanitized: &[u8],
    corrections: &[CorrectionRef<'_>],
    rel_path: &str,
    findings: &mut Vec<TermFinding>,
    internal_findings: &mut Vec<InternalFinding>,
    claimed: &mut BTreeSet<(String, usize)>,
) {
    for c in corrections {
        // Pinyin matches are handled by the pinyin scanner; literal scan never emits pinyin.
        if c.match_kind == MatchKind::Pinyin {
            continue;
        }
        let orig_bytes = c.original.as_bytes();
        if orig_bytes.is_empty() {
            continue;
        }
        let is_ambiguous = c.is_ambiguous;
        let mut search_start = 0;
        while search_start < sanitized.len() {
            let Some(rel_offset) = find_subseq(&sanitized[search_start..], orig_bytes) else {
                break;
            };
            let abs_offset = search_start + rel_offset;
            let key = (rel_path.to_string(), abs_offset);
            if claimed.contains(&key) {
                search_start = abs_offset + 1;
                continue;
            }

            // Word-boundary check dispatched on MatchKind
            if !apply_word_boundary(content, sanitized, c.match_kind, c.original, abs_offset, orig_bytes.len()) {
                search_start = abs_offset + 1;
                continue;
            }

            claimed.insert(key);

            let (line, col) = byte_offset_to_line_col(content, abs_offset);

            let mk_str = match c.match_kind {
                MatchKind::Word => "word",
                MatchKind::Substring => "substring",
                MatchKind::Pinyin => "pinyin",
            };
            let fk_str = match c.fix_kind {
                FixKind::Required => "required",
                FixKind::Suggested => "suggested",
            };

            findings.push(TermFinding {
                path: rel_path.to_string(),
                line,
                column: col,
                original: c.original.to_string(),
                correct: c.correct.to_string(),
                term: c.term_name.to_string(),
                description: c.description.map(String::from),
                confidence: c.confidence,
                replacement_eligible: !is_ambiguous,
                safety_reason: if is_ambiguous { Some("ambiguous".to_string()) } else { None },
                candidates: if is_ambiguous { c.candidates.to_vec() } else { vec![] },
                match_kind: mk_str.to_string(),
                fix_kind: fk_str.to_string(),
            });

            internal_findings.push(InternalFinding {
                path: rel_path.to_string(),
                byte_offset: abs_offset,
                original_len: orig_bytes.len(),
                original: c.original.to_string(),
                correct: c.correct.to_string(),
                is_ambiguous,
                fix_kind: c.fix_kind,
            });

            search_start = abs_offset + 1;
        }
    }
}

/// Find subsequence `needle` in `haystack`, accounting for \0 placeholders.
fn find_subseq(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() {
        return None;
    }
    haystack.windows(needle.len()).position(|w| {
        if w[0] == 0 {
            return false;
        }
        w.iter().zip(needle.iter()).all(|(&h, &n)| h == n || h == 0)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn byte_offset_basic() {
        let content = "hello\nworld";
        let (line, col) = byte_offset_to_line_col(content, 6);
        assert_eq!(line, 2);
        assert_eq!(col, 1);
    }

    #[test]
    fn byte_offset_first_line() {
        let content = "hello";
        let (line, col) = byte_offset_to_line_col(content, 0);
        assert_eq!(line, 1);
        assert_eq!(col, 1);
    }

    #[test]
    fn find_subseq_exact() {
        let haystack = b"hello world";
        let needle = b"world";
        assert_eq!(find_subseq(haystack, needle), Some(6));
    }

    #[test]
    fn find_subseq_rejects_all_zeroes() {
        let haystack = b"\0\0\0\0\0\0\0\0";
        let needle = b"mindrepo";
        assert_eq!(find_subseq(haystack, needle), None);
    }

    #[test]
    fn find_subseq_allows_zero_in_middle() {
        let haystack = b"mind\0epo";
        let needle = b"mindrepo";
        assert_eq!(find_subseq(haystack, needle), Some(0));
    }

    #[test]
    fn find_subseq_not_found() {
        let haystack = b"hello world";
        let needle = b"xyz";
        assert_eq!(find_subseq(haystack, needle), None);
    }
}
