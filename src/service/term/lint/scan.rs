use std::collections::BTreeSet;

use crate::model::term::TermFinding;

pub(super) struct InternalFinding {
    pub(super) path: String,
    pub(super) byte_offset: usize,
    pub(super) original_len: usize,
    pub(super) original: String,
    pub(super) correct: String,
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

pub(super) fn scan_file_for_corrections(
    content: &str,
    sanitized: &[u8],
    corrections: &[(String, String, String)],
    rel_path: &str,
    findings: &mut Vec<TermFinding>,
    internal_findings: &mut Vec<InternalFinding>,
    claimed: &mut BTreeSet<(String, usize)>,
) {
    for (original, correct, term_name) in corrections {
        let orig_bytes = original.as_bytes();
        if orig_bytes.is_empty() {
            continue;
        }
        let mut search_start = 0;
        while search_start < sanitized.len() {
            match find_subseq(&sanitized[search_start..], orig_bytes) {
                Some(rel_offset) => {
                    let abs_offset = search_start + rel_offset;
                    let key = (rel_path.to_string(), abs_offset);
                    if claimed.contains(&key) {
                        search_start = abs_offset + 1;
                        continue;
                    }
                    claimed.insert(key);

                    let (line, col) = byte_offset_to_line_col(content, abs_offset);
                    findings.push(TermFinding {
                        path: rel_path.to_string(),
                        line,
                        column: col,
                        original: original.clone(),
                        correct: correct.clone(),
                        term: term_name.clone(),
                    });

                    internal_findings.push(InternalFinding {
                        path: rel_path.to_string(),
                        byte_offset: abs_offset,
                        original_len: orig_bytes.len(),
                        original: original.clone(),
                        correct: correct.clone(),
                    });

                    search_start = abs_offset + 1;
                }
                None => break,
            }
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
