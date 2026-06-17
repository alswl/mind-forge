use std::collections::BTreeSet;

use crate::model::term::{CandidateTerm, TermFinding};

fn is_ascii_word_string(s: &str) -> bool {
    !s.is_empty()
        && s.bytes()
            .all(|b| matches!(b, b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'_' | b'-'))
}

fn is_word_boundary_byte(b: u8) -> bool {
    !matches!(b, b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'_')
}

pub(crate) struct InternalFinding {
    pub(crate) path: String,
    pub(crate) byte_offset: usize,
    pub(crate) original_len: usize,
    pub(crate) original: String,
    pub(crate) correct: String,
    pub(crate) is_ambiguous: bool,
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

            // Word-boundary check for pure-ASCII originals
            if is_ascii_word_string(c.original) {
                let before_ok =
                    abs_offset == 0 || is_word_boundary_byte(sanitized[abs_offset - 1]);
                let end = abs_offset + orig_bytes.len();
                let after_ok = end >= sanitized.len() || is_word_boundary_byte(sanitized[end]);
                if !before_ok || !after_ok {
                    search_start = abs_offset + 1;
                    continue;
                }
            }

            claimed.insert(key);

            let (line, col) = byte_offset_to_line_col(content, abs_offset);

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
            });

            internal_findings.push(InternalFinding {
                path: rel_path.to_string(),
                byte_offset: abs_offset,
                original_len: orig_bytes.len(),
                original: c.original.to_string(),
                correct: c.correct.to_string(),
                is_ambiguous,
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
