use std::collections::BTreeSet;

use super::segment::JiebaBoundaries;
use crate::model::term::{Boundary, CandidateTerm, FindingSelection, MatchKind, TermFinding};

pub(crate) fn context_excerpt(content: &str, byte_offset: usize, byte_len: usize) -> String {
    let line_start = content[..byte_offset].rfind('\n').map_or(0, |index| index + 1);
    let line_end =
        content[byte_offset + byte_len..].find('\n').map_or(content.len(), |index| byte_offset + byte_len + index);
    let line = content[line_start..line_end].trim().replace(['\r', '\t'], " ");
    let chars: Vec<char> = line.chars().collect();
    if chars.len() <= 120 {
        line
    } else {
        let start_char = content[line_start..byte_offset].chars().count().saturating_sub(40);
        let end_char = (start_char + 120).min(chars.len());
        let mut excerpt: String = chars[start_char..end_char].iter().collect();
        if start_char > 0 {
            excerpt.insert(0, '…');
        }
        if end_char < chars.len() {
            excerpt.push('…');
        }
        excerpt
    }
}

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

/// True when a correction `original` is pure CJK and at most two Han characters
/// — the ambiguous class (e.g. 「以可」) where a jieba segmentation miss could
/// mis-fire. Such `Word`/standalone corrections lint as **advisory** and are
/// never auto-applied by `term fix` without an explicit opt-in (spec 074 #30).
fn is_short_cjk_correction(original: &str) -> bool {
    let mut count = 0;
    for c in original.chars() {
        if !is_cjk_ideograph(c) {
            return false;
        }
        count += 1;
        if count > 2 {
            return false;
        }
    }
    (1..=2).contains(&count)
}

/// A character that would continue a word/token if adjacent to a match: a CJK
/// ideograph or an ASCII alphanumeric. Used for the #24 substring-tail warning.
fn is_word_continuation(c: char) -> bool {
    is_cjk_ideograph(c) || c.is_ascii_alphanumeric()
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

#[allow(dead_code)]
fn char_after(content: &str, byte_offset: usize) -> Option<char> {
    content[byte_offset..].chars().next()
}

/// Add spacing only at the outer boundaries of a replacement. Existing
/// whitespace and punctuation are left untouched; internal replacement text is
/// deliberately not inspected.
pub(crate) fn padded_replacement(content: &str, start: usize, end: usize, replacement: &str) -> String {
    let first = replacement.chars().next();
    let last = replacement.chars().next_back();
    let before = char_before(content, start);
    let after = char_after(content, end);
    let left = before.is_some_and(is_cjk_ideograph) && first.is_some_and(|c| c.is_ascii_alphanumeric())
        || before.is_some_and(|c| c.is_ascii_alphanumeric()) && first.is_some_and(is_cjk_ideograph);
    let right = last.is_some_and(|c| c.is_ascii_alphanumeric()) && after.is_some_and(is_cjk_ideograph)
        || last.is_some_and(is_cjk_ideograph) && after.is_some_and(|c| c.is_ascii_alphanumeric());
    let mut result = String::with_capacity(replacement.len() + 2);
    if left && !replacement.chars().next().is_some_and(char::is_whitespace) {
        result.push(' ');
    }
    result.push_str(replacement);
    if right && !replacement.chars().next_back().is_some_and(char::is_whitespace) {
        result.push(' ');
    }
    result
}

/// Per-correction word-boundary policy. Computed once per correction so the
/// match-kind + boundary + ASCII-ness decision is not redone for every
/// candidate offset in the scan loop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WordCheck {
    /// Literal substring matching with no boundary gate.
    SubstringLoose,
    /// Literal substring matching with identifier/Jieba boundary protection.
    SubstringStandalone,
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
            WordCheck::SubstringLoose => "loose",
            WordCheck::SubstringStandalone => "standalone",
            WordCheck::AsciiLoose => "loose",
            WordCheck::AsciiStandalone => "standalone",
            WordCheck::Cjk => "cjk",
        }
    }

    pub(crate) fn for_correction(match_kind: MatchKind, boundary: Boundary, original: &str) -> Self {
        match match_kind {
            MatchKind::Substring => match boundary {
                Boundary::Loose => WordCheck::SubstringLoose,
                Boundary::Standalone => WordCheck::SubstringStandalone,
            },
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
fn apply_word_boundary(
    content: &str,
    sanitized: &[u8],
    check: WordCheck,
    offset: usize,
    original_len: usize,
    jieba: Option<&JiebaBoundaries>,
) -> bool {
    match check {
        WordCheck::SubstringLoose => true,
        WordCheck::SubstringStandalone => {
            if content[offset..offset + original_len].is_ascii() {
                let before_ok = offset == 0 || !is_identifier_neighbour(sanitized[offset - 1]);
                let end = offset + original_len;
                let after_ok = end >= sanitized.len() || !is_identifier_neighbour(sanitized[end]);
                before_ok && after_ok
            } else if let Some(jb) = jieba {
                jb.span_aligns(offset, original_len)
            } else {
                true
            }
        }
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
            // Use jieba word segmentation as the CJK word-boundary oracle.
            // For the ambiguous **short pure-CJK** class (≤2 Han characters)
            // the matched span must be itself *one exact jieba token* (spec 074
            // #30). Edge alignment alone (`span_aligns`) fired for spans that
            // merely sit between two separately-emitted tokens, e.g. 「以可」 in
            // 「以可独立验证」 where 以 and 可 are separate words — the reported
            // false positive. Longer or mixed CJK+ASCII compounds (e.g.
            // 「网关api」) keep the edge-alignment oracle, which reliably gates
            // on genuine standalone occurrences.
            if let Some(jb) = jieba {
                let span = &content[offset..offset + original_len];
                if is_short_cjk_correction(span) {
                    jb.is_token(offset, original_len)
                } else {
                    jb.span_aligns(offset, original_len)
                }
            } else {
                // Fallback: no jieba boundaries available — accept (should not
                // happen in practice; scan_content always provides boundaries).
                true
            }
        }
    }
}

pub(crate) struct InternalFinding {
    pub(crate) path: String,
    pub(crate) byte_offset: usize,
    pub(crate) original_len: usize,
    pub(crate) original: String,
    pub(crate) correct: String,
    pub(crate) fix_kind: crate::model::term::FixKind,
    pub(crate) term_name: String,
    pub(crate) confidence: Option<f64>,
    pub(crate) replacement_eligible: bool,
    /// True for a short-CJK advisory finding: it may lint but is skipped by
    /// auto-apply unless the user explicitly opts in (`--term NAME[:ORIGINAL]`).
    pub(crate) advisory: bool,
    pub(crate) substring_adjacent_word: bool,
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

/// Spec 075 US5/FR-032: `a` and `b` are in a prefix-or-equal relationship,
/// case-insensitively for ASCII — the same relationship checked at
/// registration time (`correction.rs::is_prefix_or_equal_ci`), applied here
/// per-finding so a misdirected match discloses the terms it could have gone
/// to instead of only the one it happened to match.
fn is_prefix_or_equal_ci(a: &str, b: &str) -> bool {
    let a = a.to_ascii_lowercase();
    let b = b.to_ascii_lowercase();
    a == b || a.starts_with(&b) || b.starts_with(&a)
}

/// Other terms (by name) whose name or registered original is in a
/// prefix-or-equal relationship with `original` — every term this finding's
/// match could plausibly have been claimed by instead of `own_term`.
///
/// `all_term_names` covers terms with zero registered corrections (which
/// have no `CorrectionRef` at all and so would otherwise be invisible here);
/// `corrections` covers every other term's registered originals.
fn competing_terms(
    corrections: &[CorrectionRef<'_>],
    all_term_names: &[&str],
    own_term: &str,
    original: &str,
) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for &name in all_term_names {
        if name != own_term && is_prefix_or_equal_ci(original, name) && seen.insert(name) {
            out.push(name.to_string());
        }
    }
    for c in corrections {
        if c.term_name != own_term && is_prefix_or_equal_ci(original, c.original) && seen.insert(c.term_name) {
            out.push(c.term_name.to_string());
        }
    }
    out
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

#[allow(clippy::too_many_arguments)]
pub(crate) fn scan_file_for_corrections(
    content: &str,
    sanitized: &[u8],
    corrections: &[CorrectionRef<'_>],
    all_term_names: &[&str],
    rel_path: &str,
    findings: &mut Vec<TermFinding>,
    internal_findings: &mut Vec<InternalFinding>,
    claimed: &mut BTreeSet<(String, usize, usize)>,
    jieba: Option<&JiebaBoundaries>,
) {
    // Spec 075 US4/FR-025/FR-027: the most specific (longest) registered
    // correction must win at any position, regardless of declaration order.
    // Scanning corrections in declaration order let a shorter correction
    // (e.g. 机器→装置) claim a start offset before a longer, more specific one
    // (机器人→机械装置) was ever tried at the same position, so the longer
    // correction was silently never reported. Scanning longest-original-first
    // — ties broken by declaration order, matching `deduplicate_spans`'
    // `(start ASC, end DESC, correction_order ASC)` rule on the fix side —
    // means the longer correction claims the position first and the shorter
    // one is later rejected by the overlap check below.
    let mut ordered: Vec<&CorrectionRef<'_>> = corrections.iter().collect();
    ordered.sort_by(|a, b| b.original.len().cmp(&a.original.len()).then(a.yaml_index.cmp(&b.yaml_index)));

    for c in ordered {
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
        // #30 policy backstop: short pure-CJK word corrections are advisory —
        // they lint but are not auto-applied (a future segmentation miss must
        // not corrupt prose). Longer CJK and ASCII corrections are unaffected.
        let short_cjk_advisory = matches!(check, WordCheck::Cjk) && is_short_cjk_correction(c.original);
        // Depends only on this correction, so compute it once rather than
        // once per matched occurrence.
        let competing = competing_terms(corrections, all_term_names, c.term_name, c.original);
        let mut search_start = 0;
        while search_start < sanitized.len() {
            let Some(rel_offset) = find_subseq(&sanitized[search_start..], orig_bytes) else {
                break;
            };
            let abs_offset = search_start + rel_offset;
            // A longer correction scanned earlier may have claimed a byte
            // range that only *overlaps* this candidate's start (not
            // necessarily starts at the same offset) — reject on any overlap,
            // not just an exact-start match, so a shorter correction cannot
            // partially eat the tail of an already-claimed longer one either.
            if claimed.iter().any(|(path, off, len)| {
                path == rel_path && abs_offset < *off + *len && *off < abs_offset + orig_bytes.len()
            }) {
                search_start = abs_offset + 1;
                continue;
            }

            if !apply_word_boundary(content, sanitized, check, abs_offset, orig_bytes.len(), jieba) {
                search_start = abs_offset + 1;
                continue;
            }

            claimed.insert((rel_path.to_string(), abs_offset, orig_bytes.len()));

            let (line, col) = byte_offset_to_line_col(content, abs_offset);

            // #24: a loose substring match whose neighbour is a continuous
            // CJK/alnum char likely swallowed part of a larger word (e.g.
            // `阿卡`→`ARCA` over `阿卡索` yields `ARCA索`). Flag it so lint/fix warn.
            let substring_adjacent_word = matches!(check, WordCheck::SubstringLoose)
                && (char_before(content, abs_offset).is_some_and(is_word_continuation)
                    || char_after(content, abs_offset + orig_bytes.len()).is_some_and(is_word_continuation));

            findings.push(TermFinding {
                path: rel_path.to_string(),
                line,
                column: col,
                original: c.original.to_string(),
                correct: c.correct.to_string(),
                term: c.term_name.to_string(),
                description: c.description.map(String::from),
                confidence: c.confidence,
                replacement_eligible: !is_ambiguous && !short_cjk_advisory,
                safety_reason: if is_ambiguous {
                    Some("ambiguous".to_string())
                } else if short_cjk_advisory {
                    Some("short-cjk-advisory".to_string())
                } else {
                    None
                },
                candidates: if is_ambiguous { c.candidates.to_vec() } else { vec![] },
                match_kind: c.match_kind,
                fix_kind: c.fix_kind,
                boundary: c.boundary,
                boundary_mode: check.boundary_mode(),
                substring_adjacent_word,
                selection: if is_ambiguous { FindingSelection::Ambiguous } else { FindingSelection::Selected },
                context: context_excerpt(content, abs_offset, orig_bytes.len()),
                // Overwritten by `apply_selection` once the fix scope is known.
                held_back: false,
                competing_terms: competing.clone(),
            });

            internal_findings.push(InternalFinding {
                path: rel_path.to_string(),
                byte_offset: abs_offset,
                original_len: orig_bytes.len(),
                original: c.original.to_string(),
                correct: c.correct.to_string(),
                fix_kind: c.fix_kind,
                term_name: c.term_name.to_string(),
                confidence: c.confidence,
                replacement_eligible: !is_ambiguous && !short_cjk_advisory,
                advisory: short_cjk_advisory,
                substring_adjacent_word,
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
    fn word_check_for_substring_uses_requested_boundary() {
        assert_eq!(
            WordCheck::for_correction(MatchKind::Substring, Boundary::Loose, "anything"),
            WordCheck::SubstringLoose
        );
        assert_eq!(
            WordCheck::for_correction(MatchKind::Substring, Boundary::Standalone, "anything"),
            WordCheck::SubstringStandalone
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

    #[test]
    fn padded_replacement_only_adds_outer_cjk_ascii_spaces() {
        assert_eq!(padded_replacement("甲旧乙", 3, 6, "ASCII"), " ASCII ");
        assert_eq!(padded_replacement("甲旧，", 3, 6, "装置"), "装置");
        assert_eq!(padded_replacement("甲旧乙", 3, 6, "DD 站点"), " DD 站点");
        assert_eq!(padded_replacement("甲旧 乙", 3, 6, "ASCII"), " ASCII");
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
        for b in *b" \t\n\r,;:!?()[]{}\"'" {
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
            apply_word_boundary(&content, &sanitized, WordCheck::AsciiLoose, offset, 4, None),
            "loose mode must keep matching inside kebab (today's behaviour)"
        );
    }

    #[test]
    fn loose_matches_standalone() {
        let (content, sanitized) = build_inputs("the aidc site");
        let offset = content.find("aidc").unwrap();
        assert!(apply_word_boundary(&content, &sanitized, WordCheck::AsciiLoose, offset, 4, None));
    }

    // ── Boundary::Standalone — these tests RED until US1 lands ───────────────

    #[test]
    fn standalone_rejects_kebab_left_neighbour() {
        let (content, sanitized) = build_inputs("xxx-aidc test");
        let offset = content.find("aidc").unwrap();
        assert!(
            !apply_word_boundary(&content, &sanitized, WordCheck::AsciiStandalone, offset, 4, None),
            "standalone must reject left neighbour '-'"
        );
    }

    #[test]
    fn standalone_rejects_kebab_right_neighbour() {
        let (content, sanitized) = build_inputs("test aidc-suffix");
        let offset = content.find("aidc").unwrap();
        assert!(
            !apply_word_boundary(&content, &sanitized, WordCheck::AsciiStandalone, offset, 4, None),
            "standalone must reject right neighbour '-'"
        );
    }

    #[test]
    fn standalone_rejects_kebab_both_sides() {
        let (content, sanitized) = build_inputs("xxx-aidc-test");
        let offset = content.find("aidc").unwrap();
        assert!(
            !apply_word_boundary(&content, &sanitized, WordCheck::AsciiStandalone, offset, 4, None),
            "standalone must reject kebab identifier xxx-aidc-test"
        );
    }

    #[test]
    fn standalone_rejects_snake_case() {
        // Underscore is ALSO an identifier neighbour, just as it is today for loose.
        let (content, sanitized) = build_inputs("my_aidc_db");
        let offset = content.find("aidc").unwrap();
        assert!(
            !apply_word_boundary(&content, &sanitized, WordCheck::AsciiStandalone, offset, 4, None),
            "standalone must reject snake_case neighbours"
        );
    }

    #[test]
    fn standalone_rejects_path_slash_neighbour() {
        let (content, sanitized) = build_inputs("./docs/aidc/intro.md");
        let offset = content.find("aidc").unwrap();
        assert!(
            !apply_word_boundary(&content, &sanitized, WordCheck::AsciiStandalone, offset, 4, None),
            "standalone must reject path-internal occurrences"
        );
    }

    #[test]
    fn standalone_rejects_dot_neighbour() {
        let (content, sanitized) = build_inputs("module.aidc.handler");
        let offset = content.find("aidc").unwrap();
        assert!(
            !apply_word_boundary(&content, &sanitized, WordCheck::AsciiStandalone, offset, 4, None),
            "standalone must reject dotted-module neighbours"
        );
    }

    #[test]
    fn standalone_rejects_backslash_neighbour() {
        let (content, sanitized) = build_inputs(r"win\aidc\file");
        let offset = content.find("aidc").unwrap();
        assert!(
            !apply_word_boundary(&content, &sanitized, WordCheck::AsciiStandalone, offset, 4, None),
            "standalone must reject backslash neighbours"
        );
    }

    #[test]
    fn standalone_accepts_whitespace_neighbours() {
        let (content, sanitized) = build_inputs("the aidc site");
        let offset = content.find("aidc").unwrap();
        assert!(
            apply_word_boundary(&content, &sanitized, WordCheck::AsciiStandalone, offset, 4, None),
            "standalone must keep matching standalone-in-prose occurrences"
        );
    }

    #[test]
    fn standalone_accepts_punctuation_neighbours() {
        // ASCII punctuation OTHER than the suppressed set must remain boundary.
        let (content, sanitized) = build_inputs("(aidc) and aidc, then aidc.");
        let offset = content.find("(aidc)").unwrap() + 1;
        assert!(
            apply_word_boundary(&content, &sanitized, WordCheck::AsciiStandalone, offset, 4, None),
            "standalone must accept '(' / ')' neighbours"
        );
    }

    #[test]
    fn standalone_accepts_start_and_end_of_input() {
        let (content, sanitized) = build_inputs("aidc");
        assert!(
            apply_word_boundary(&content, &sanitized, WordCheck::AsciiStandalone, 0, 4, None),
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
            apply_word_boundary(&content, &sanitized, WordCheck::AsciiStandalone, offset, 4, None),
            "standalone must keep matching when right neighbour is CJK"
        );
    }

    fn scan_original(content: &str, original: &str, match_kind: MatchKind) -> Vec<TermFinding> {
        let correction = CorrectionRef {
            yaml_index: 0,
            original,
            correct: "Synthetic",
            term_name: "Synthetic",
            description: None,
            confidence: Some(1.0),
            is_ambiguous: false,
            candidates: &[],
            match_kind,
            fix_kind: crate::model::term::FixKind::Required,
            boundary: Boundary::Standalone,
            pinyin: None,
        };
        let mut findings = Vec::new();
        let mut internal = Vec::new();
        let mut claimed = BTreeSet::new();
        let jieba = JiebaBoundaries::segment(content);
        scan_file_for_corrections(
            content,
            content.as_bytes(),
            &[correction],
            &[],
            "synthetic.md",
            &mut findings,
            &mut internal,
            &mut claimed,
            Some(&jieba),
        );
        findings
    }

    #[test]
    fn cjk_word_matches_complete_token_but_not_embedded_fragment() {
        let findings = scan_original("小文件需要备份。小文 负责。", "小文", MatchKind::Word);
        assert_eq!(findings.len(), 1, "only the complete segmented token should match");
        assert_eq!(findings[0].original, "小文");
    }

    #[test]
    fn word_mode_rejects_embedded_occurrence() {
        let word = scan_original("scatter cat", "cat", MatchKind::Word);
        assert_eq!(word.len(), 1, "word mode must reject the embedded occurrence");
    }

    // ── Spec 074 #30: short-CJK false positive + advisory classification ─────

    /// T003: 「以可」 in 「以可独立验证和回退的方案」 spans a grammatical word
    /// boundary (以 + 可 + verb) and is NOT one jieba token → zero findings.
    #[test]
    fn short_cjk_spanning_word_boundary_is_not_flagged() {
        let findings = scan_original("以可独立验证和回退的方案", "以可", MatchKind::Word);
        assert_eq!(
            findings.len(),
            0,
            "以可 must not be flagged when 以 and 可 are separate words, found: {findings:#?}"
        );
    }

    /// T004: a genuine standalone short-CJK occurrence still surfaces — as an
    /// advisory finding (replacement_eligible=false, safety_reason=short-cjk-advisory).
    #[test]
    fn genuine_short_cjk_standalone_is_advisory() {
        let findings = scan_original("机器 很常见", "机器", MatchKind::Word);
        assert_eq!(findings.len(), 1, "standalone 机器 must still surface");
        let f = &findings[0];
        assert!(!f.replacement_eligible, "short-CJK finding must be advisory (not replacement-eligible)");
        assert_eq!(
            f.safety_reason.as_deref(),
            Some("short-cjk-advisory"),
            "advisory finding must carry safety_reason=short-cjk-advisory"
        );
    }

    /// A longer (≥3 Han-char) CJK word correction stays replacement-eligible
    /// (the advisory downgrade applies only to ≤2 Han-char originals).
    #[test]
    fn longer_cjk_correction_stays_replacement_eligible() {
        let findings = scan_original("机器人 很常见", "机器人", MatchKind::Word);
        assert_eq!(findings.len(), 1);
        assert!(findings[0].replacement_eligible, "longer CJK correction must remain auto-fixable");
        assert_eq!(findings[0].safety_reason, None);
    }

    /// A CJK substring correction (not word/standalone) is unaffected by the
    /// short-CJK advisory downgrade.
    #[test]
    fn cjk_substring_is_not_advisory() {
        let findings = scan_original("以可独立验证", "以可", MatchKind::Substring);
        assert_eq!(findings.len(), 1, "loose/standalone substring must still match");
        assert!(findings[0].replacement_eligible, "substring matches must not be advisory");
    }

    #[test]
    fn substring_standalone_rejects_embedded_occurrence() {
        let substring = scan_original("scatter cat", "cat", MatchKind::Substring);
        assert_eq!(substring.len(), 1, "standalone substring must reject the embedded occurrence");
    }
}
