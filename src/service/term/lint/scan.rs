use std::collections::BTreeSet;

use crate::model::term::{Boundary, CandidateTerm, MatchKind, TermFinding};

fn is_word_boundary_byte(b: u8) -> bool {
    !matches!(b, b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'_')
}

/// Bytes that compose an identifier or path token. Under `Boundary::Standalone`,
/// finding any of these as the left or right neighbour suppresses the match.
pub(crate) fn is_identifier_neighbour(b: u8) -> bool {
    matches!(b,
        b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' |
        b'_' | b'-' | b'/' | b'\\' | b'.'
    )
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

/// Per-correction word-boundary policy. Computed once per correction so the
/// match-kind + boundary + ASCII-ness decision is not redone for every
/// candidate offset in the scan loop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WordCheck {
    /// Substring match — every position passes.
    AlwaysAccept,
    /// ASCII original, loose boundary (today's behaviour).
    AsciiLoose,
    /// ASCII original, standalone boundary (FR-002).
    AsciiStandalone,
    /// CJK original — boundary defined by char-level scan over `content`.
    Cjk,
}

impl WordCheck {
    pub(crate) fn boundary_mode(&self) -> &'static str {
        match self {
            WordCheck::AlwaysAccept => "loose",
            WordCheck::AsciiLoose => "loose",
            WordCheck::AsciiStandalone => "standalone",
            WordCheck::Cjk => "cjk",
        }
    }

    pub(crate) fn for_correction(match_kind: MatchKind, boundary: Boundary, original: &str) -> Self {
        match match_kind {
            MatchKind::Substring => WordCheck::AlwaysAccept,
            MatchKind::Word => {
                if original.is_ascii() {
                    match boundary {
                        Boundary::Loose => WordCheck::AsciiLoose,
                        Boundary::Standalone => WordCheck::AsciiStandalone,
                    }
                } else {
                    WordCheck::Cjk
                }
            }
            MatchKind::Pinyin => {
                unreachable!("pinyin matches are dispatched through the pinyin scanner")
            }
        }
    }
}

/// Check word boundaries for a match at `offset` with `original_len` bytes.
/// Returns true if the match passes boundary requirements under `check`.
fn apply_word_boundary(content: &str, sanitized: &[u8], check: WordCheck, offset: usize, original_len: usize) -> bool {
    match check {
        WordCheck::AlwaysAccept => true,
        WordCheck::AsciiLoose => {
            let before_ok = offset == 0 || is_word_boundary_byte(sanitized[offset - 1]);
            let end = offset + original_len;
            let after_ok = end >= sanitized.len() || is_word_boundary_byte(sanitized[end]);
            before_ok && after_ok
        }
        WordCheck::AsciiStandalone => {
            let before_ok = offset == 0 || !is_identifier_neighbour(sanitized[offset - 1]);
            let end = offset + original_len;
            let after_ok = end >= sanitized.len() || !is_identifier_neighbour(sanitized[end]);
            before_ok && after_ok
        }
        WordCheck::Cjk => {
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
}

pub(crate) struct InternalFinding {
    pub(crate) path: String,
    pub(crate) byte_offset: usize,
    pub(crate) original_len: usize,
    pub(crate) original: String,
    pub(crate) correct: String,
    pub(crate) is_ambiguous: bool,
    pub(crate) fix_kind: crate::model::term::FixKind,
    /// Position of the source Correction in the YAML `corrections:` list.
    /// Used by `deduplicate_spans` as the tie-breaker when two corrections
    /// share the same byte span: lower wins (i.e., the earlier-declared rule).
    pub(crate) yaml_index: usize,
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
    /// Position in the YAML `corrections:` list. Threaded through to
    /// `InternalFinding.yaml_index` so dedup can break ties by declaration
    /// order rather than scan emit order.
    pub yaml_index: usize,
    pub original: &'a str,
    pub correct: &'a str,
    pub term_name: &'a str,
    pub description: Option<&'a str>,
    pub confidence: Option<f64>,
    pub is_ambiguous: bool,
    pub candidates: &'a [CandidateTerm],
    pub match_kind: crate::model::term::MatchKind,
    pub fix_kind: crate::model::term::FixKind,
    pub boundary: Boundary,
    pub pinyin: Option<&'a str>,
}

pub(crate) fn scan_file_for_corrections(
    content: &str,
    sanitized: &[u8],
    corrections: &[CorrectionRef<'_>],
    rel_path: &str,
    findings: &mut Vec<TermFinding>,
    internal_findings: &mut Vec<InternalFinding>,
    claimed: &mut BTreeSet<(String, usize, usize)>,
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
        let check = WordCheck::for_correction(c.match_kind, c.boundary, c.original);
        let is_ambiguous = c.is_ambiguous;
        let mut search_start = 0;
        while search_start < sanitized.len() {
            let Some(rel_offset) = find_subseq(&sanitized[search_start..], orig_bytes) else {
                break;
            };
            let abs_offset = search_start + rel_offset;
            if claimed.iter().any(|(path, off, _)| path == rel_path && *off == abs_offset) {
                search_start = abs_offset + 1;
                continue;
            }

            if !apply_word_boundary(content, sanitized, check, abs_offset, orig_bytes.len()) {
                search_start = abs_offset + 1;
                continue;
            }

            claimed.insert((rel_path.to_string(), abs_offset, orig_bytes.len()));

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
                match_kind: c.match_kind,
                fix_kind: c.fix_kind,
                boundary: c.boundary,
                boundary_mode: check.boundary_mode(),
            });

            internal_findings.push(InternalFinding {
                path: rel_path.to_string(),
                byte_offset: abs_offset,
                original_len: orig_bytes.len(),
                original: c.original.to_string(),
                correct: c.correct.to_string(),
                is_ambiguous,
                fix_kind: c.fix_kind,
                yaml_index: c.yaml_index,
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

    // ── WordCheck factory ────────────────────────────────────────────────────

    #[test]
    fn word_check_for_substring_is_always_accept() {
        assert_eq!(
            WordCheck::for_correction(MatchKind::Substring, Boundary::Loose, "anything"),
            WordCheck::AlwaysAccept
        );
        assert_eq!(
            WordCheck::for_correction(MatchKind::Substring, Boundary::Standalone, "anything"),
            WordCheck::AlwaysAccept
        );
    }

    #[test]
    fn word_check_for_ascii_word_picks_boundary() {
        assert_eq!(WordCheck::for_correction(MatchKind::Word, Boundary::Loose, "aidc"), WordCheck::AsciiLoose);
        assert_eq!(
            WordCheck::for_correction(MatchKind::Word, Boundary::Standalone, "aidc"),
            WordCheck::AsciiStandalone
        );
    }

    #[test]
    fn word_check_for_ascii_phrase_with_space_is_ascii_boundary() {
        // RED until T006: "foo dr" is all-ASCII but contains a space, so
        // is_ascii_word_string returns false → wrongly classified as Cjk.
        assert_eq!(
            WordCheck::for_correction(MatchKind::Word, Boundary::Loose, "foo dr"),
            WordCheck::AsciiLoose,
            "all-ASCII phrase with space must use ASCII loose boundary"
        );
        assert_eq!(
            WordCheck::for_correction(MatchKind::Word, Boundary::Standalone, "foo dr"),
            WordCheck::AsciiStandalone,
            "all-ASCII phrase with space must use ASCII standalone boundary"
        );
    }

    #[test]
    fn word_check_for_cjk_original_is_cjk_regardless_of_boundary() {
        assert_eq!(WordCheck::for_correction(MatchKind::Word, Boundary::Loose, "机器人"), WordCheck::Cjk);
        assert_eq!(WordCheck::for_correction(MatchKind::Word, Boundary::Standalone, "机器人"), WordCheck::Cjk);
    }

    #[test]
    fn word_check_for_cjk_and_mixed_source_still_cjk() {
        // FR-006: pure CJK and mixed ASCII+CJK source text must keep CJK path.
        assert_eq!(
            WordCheck::for_correction(MatchKind::Word, Boundary::Loose, "机器"),
            WordCheck::Cjk,
            "pure CJK source must stay Cjk"
        );
        assert_eq!(
            WordCheck::for_correction(MatchKind::Word, Boundary::Loose, "foo 机器"),
            WordCheck::Cjk,
            "mixed ASCII+CJK source must stay Cjk"
        );
        // standalone boundary also respects the same gate.
        assert_eq!(
            WordCheck::for_correction(MatchKind::Word, Boundary::Standalone, "foo 机器"),
            WordCheck::Cjk,
            "mixed source under standalone must stay Cjk"
        );
    }

    #[test]
    #[should_panic(expected = "pinyin matches are dispatched through the pinyin scanner")]
    fn word_check_for_pinyin_panics() {
        let _ = WordCheck::for_correction(MatchKind::Pinyin, Boundary::Loose, "ji-qi-ren");
    }

    // ── is_identifier_neighbour byte class (FR-002 helper) ───────────────────

    #[test]
    fn is_identifier_neighbour_letters_digits_underscore() {
        for b in b'A'..=b'Z' {
            assert!(is_identifier_neighbour(b), "{} should be identifier byte", b as char);
        }
        for b in b'a'..=b'z' {
            assert!(is_identifier_neighbour(b), "{} should be identifier byte", b as char);
        }
        for b in b'0'..=b'9' {
            assert!(is_identifier_neighbour(b), "{} should be identifier byte", b as char);
        }
        assert!(is_identifier_neighbour(b'_'));
    }

    #[test]
    fn is_identifier_neighbour_kebab_path_dot() {
        // The new bytes added by Boundary::Standalone over today's word class.
        assert!(is_identifier_neighbour(b'-'));
        assert!(is_identifier_neighbour(b'/'));
        assert!(is_identifier_neighbour(b'\\'));
        assert!(is_identifier_neighbour(b'.'));
    }

    #[test]
    fn is_identifier_neighbour_excludes_whitespace_and_punct() {
        for b in
            [b' ', b'\t', b'\n', b'\r', b',', b';', b':', b'!', b'?', b'(', b')', b'[', b']', b'{', b'}', b'"', b'\'']
        {
            assert!(!is_identifier_neighbour(b), "{:?} must not be identifier byte", b as char);
        }
    }

    // ── Boundary::Loose preserves today's behaviour (regression guards) ──────

    fn build_inputs(s: &str) -> (String, Vec<u8>) {
        (s.to_string(), s.as_bytes().to_vec())
    }

    #[test]
    fn loose_matches_inside_kebab_today() {
        // Regression guard for the current (loose) behaviour: kebab neighbours
        // pass the word boundary because `-` is a boundary byte.
        let (content, sanitized) = build_inputs("xxx-aidc-test");
        let offset = content.find("aidc").unwrap();
        assert!(
            apply_word_boundary(&content, &sanitized, WordCheck::AsciiLoose, offset, 4),
            "loose mode must keep matching inside kebab (today's behaviour)"
        );
    }

    #[test]
    fn loose_matches_standalone() {
        let (content, sanitized) = build_inputs("the aidc site");
        let offset = content.find("aidc").unwrap();
        assert!(apply_word_boundary(&content, &sanitized, WordCheck::AsciiLoose, offset, 4));
    }

    // ── Boundary::Standalone — these tests RED until US1 lands ───────────────

    #[test]
    fn standalone_rejects_kebab_left_neighbour() {
        let (content, sanitized) = build_inputs("xxx-aidc test");
        let offset = content.find("aidc").unwrap();
        assert!(
            !apply_word_boundary(&content, &sanitized, WordCheck::AsciiStandalone, offset, 4),
            "standalone must reject left neighbour '-'"
        );
    }

    #[test]
    fn standalone_rejects_kebab_right_neighbour() {
        let (content, sanitized) = build_inputs("test aidc-suffix");
        let offset = content.find("aidc").unwrap();
        assert!(
            !apply_word_boundary(&content, &sanitized, WordCheck::AsciiStandalone, offset, 4),
            "standalone must reject right neighbour '-'"
        );
    }

    #[test]
    fn standalone_rejects_kebab_both_sides() {
        let (content, sanitized) = build_inputs("xxx-aidc-test");
        let offset = content.find("aidc").unwrap();
        assert!(
            !apply_word_boundary(&content, &sanitized, WordCheck::AsciiStandalone, offset, 4),
            "standalone must reject kebab identifier xxx-aidc-test"
        );
    }

    #[test]
    fn standalone_rejects_snake_case() {
        // Underscore is ALSO an identifier neighbour, just as it is today for loose.
        let (content, sanitized) = build_inputs("my_aidc_db");
        let offset = content.find("aidc").unwrap();
        assert!(
            !apply_word_boundary(&content, &sanitized, WordCheck::AsciiStandalone, offset, 4),
            "standalone must reject snake_case neighbours"
        );
    }

    #[test]
    fn standalone_rejects_path_slash_neighbour() {
        let (content, sanitized) = build_inputs("./docs/aidc/intro.md");
        let offset = content.find("aidc").unwrap();
        assert!(
            !apply_word_boundary(&content, &sanitized, WordCheck::AsciiStandalone, offset, 4),
            "standalone must reject path-internal occurrences"
        );
    }

    #[test]
    fn standalone_rejects_dot_neighbour() {
        let (content, sanitized) = build_inputs("module.aidc.handler");
        let offset = content.find("aidc").unwrap();
        assert!(
            !apply_word_boundary(&content, &sanitized, WordCheck::AsciiStandalone, offset, 4),
            "standalone must reject dotted-module neighbours"
        );
    }

    #[test]
    fn standalone_rejects_backslash_neighbour() {
        let (content, sanitized) = build_inputs(r"win\aidc\file");
        let offset = content.find("aidc").unwrap();
        assert!(
            !apply_word_boundary(&content, &sanitized, WordCheck::AsciiStandalone, offset, 4),
            "standalone must reject backslash neighbours"
        );
    }

    #[test]
    fn standalone_accepts_whitespace_neighbours() {
        let (content, sanitized) = build_inputs("the aidc site");
        let offset = content.find("aidc").unwrap();
        assert!(
            apply_word_boundary(&content, &sanitized, WordCheck::AsciiStandalone, offset, 4),
            "standalone must keep matching standalone-in-prose occurrences"
        );
    }

    #[test]
    fn standalone_accepts_punctuation_neighbours() {
        // ASCII punctuation OTHER than the suppressed set must remain boundary.
        let (content, sanitized) = build_inputs("(aidc) and aidc, then aidc.");
        let offset = content.find("(aidc)").unwrap() + 1;
        assert!(
            apply_word_boundary(&content, &sanitized, WordCheck::AsciiStandalone, offset, 4),
            "standalone must accept '(' / ')' neighbours"
        );
    }

    #[test]
    fn standalone_accepts_start_and_end_of_input() {
        let (content, sanitized) = build_inputs("aidc");
        assert!(
            apply_word_boundary(&content, &sanitized, WordCheck::AsciiStandalone, 0, 4),
            "standalone must accept BOF + EOF as boundaries"
        );
    }

    #[test]
    fn standalone_accepts_cjk_right_neighbour() {
        // Right neighbour is CJK, left is whitespace — current word logic short-circuits
        // because the original is ASCII, so byte-level boundary applies and CJK first
        // byte is non-ASCII (>= 0x80), which is not an identifier neighbour.
        let (content, sanitized) = build_inputs("the aidc 站点");
        let offset = content.find("aidc").unwrap();
        assert!(
            apply_word_boundary(&content, &sanitized, WordCheck::AsciiStandalone, offset, 4),
            "standalone must keep matching when right neighbour is CJK"
        );
    }
}
