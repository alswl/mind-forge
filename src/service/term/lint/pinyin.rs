use std::collections::BTreeSet;

use crate::model::term::{FixKind, MatchKind, TermFinding};

use super::scan::{byte_offset_to_line_col, CorrectionRef, InternalFinding};

/// Convert CJK characters in `s` to pinyin (first reading, no tone), joined by hyphens.
/// Non-CJK characters are skipped. Returns an empty string if no CJK chars found.
pub(crate) fn to_pinyin_no_tone(s: &str) -> String {
    use pinyin::ToPinyin;
    let parts: Vec<&str> = s.to_pinyin().filter_map(|p| p.map(|pp| pp.plain())).collect();
    parts.join("-")
}

/// Scan `content` for pinyin matches from `corrections` whose `match_kind == Pinyin`.
/// Findings are pushed to `findings` and `internal_findings`; matched positions
/// are recorded in `claimed`.
pub(crate) fn scan_for_pinyin(
    content: &str,
    rel_path: &str,
    corrections: &[CorrectionRef<'_>],
    findings: &mut Vec<TermFinding>,
    internal_findings: &mut Vec<InternalFinding>,
    claimed: &mut BTreeSet<(String, usize)>,
) {
    let pinyin_corrections: Vec<&CorrectionRef<'_>> =
        corrections.iter().filter(|c| c.match_kind == MatchKind::Pinyin).collect();

    if pinyin_corrections.is_empty() {
        return;
    }

    // Pre-compute pinyin for each correction
    struct PinyinEntry<'a> {
        cref: &'a CorrectionRef<'a>,
        pinyin: String,
        char_len: usize,
    }
    let entries: Vec<PinyinEntry<'_>> = pinyin_corrections
        .iter()
        .filter_map(|c| {
            let py = c.pinyin.map(String::from).unwrap_or_else(|| to_pinyin_no_tone(c.original));
            if py.is_empty() {
                return None; // no CJK chars in original — skip
            }
            let char_len = c.original.chars().count();
            Some(PinyinEntry { cref: c, pinyin: py, char_len })
        })
        .collect();

    if entries.is_empty() {
        return;
    }

    // Collect CJK spans: regions of consecutive CJK characters
    let spans = cjk_spans(content);
    let content_chars: Vec<char> = content.chars().collect();

    for (start_byte, end_byte) in &spans {
        // Convert byte positions to char indices
        let start_char = content[..*start_byte].chars().count();
        let end_char = content[..*end_byte].chars().count();

        for entry in &entries {
            if end_char - start_char < entry.char_len {
                continue;
            }
            let max_start = end_char - entry.char_len;
            for window_start in start_char..=max_start {
                // Check if this window is within exempt regions by scanning the claimed set
                let window_bytes_start = content.char_indices().nth(window_start).map(|(bi, _)| bi).unwrap_or(0);

                let key = (rel_path.to_string(), window_bytes_start);
                if claimed.contains(&key) {
                    continue;
                }

                // Get pinyin for this window
                let window_text: String = content_chars[window_start..window_start + entry.char_len].iter().collect();
                let window_pinyin = to_pinyin_no_tone(&window_text);
                if window_pinyin.is_empty() {
                    continue;
                }

                // Pinyin distance 0: exact match ignoring tone
                if window_pinyin != entry.pinyin {
                    continue;
                }

                // Skip if window text equals original (already found by literal scan)
                if window_text == entry.cref.original {
                    continue;
                }

                claimed.insert(key);

                let (line, col) = byte_offset_to_line_col(content, window_bytes_start);

                let window_byte_len = window_text.len();

                findings.push(TermFinding {
                    path: rel_path.to_string(),
                    line,
                    column: col,
                    original: window_text.clone(),
                    correct: entry.cref.correct.to_string(),
                    term: entry.cref.term_name.to_string(),
                    description: entry.cref.description.map(String::from),
                    confidence: entry.cref.confidence,
                    replacement_eligible: !entry.cref.is_ambiguous,
                    safety_reason: if entry.cref.is_ambiguous { Some("ambiguous".to_string()) } else { None },
                    candidates: if entry.cref.is_ambiguous { entry.cref.candidates.to_vec() } else { vec![] },
                    match_kind: "pinyin".to_string(),
                    fix_kind: "suggested".to_string(), // always suggested for pinyin
                });

                internal_findings.push(InternalFinding {
                    path: rel_path.to_string(),
                    byte_offset: window_bytes_start,
                    original_len: window_byte_len,
                    original: window_text,
                    correct: entry.cref.correct.to_string(),
                    is_ambiguous: entry.cref.is_ambiguous,
                    #[allow(dead_code)]
                    match_kind: MatchKind::Pinyin,
                    fix_kind: FixKind::Suggested, // always suggested for pinyin
                });
            }
        }
    }
}

/// Return byte-offset spans of consecutive CJK character runs in `content`.
fn cjk_spans(content: &str) -> Vec<(usize, usize)> {
    let mut spans = Vec::new();
    let mut in_cjk = false;
    let mut span_start = 0;

    for (i, c) in content.char_indices() {
        let is_cjk = is_cjk_ideograph(c);
        if is_cjk && !in_cjk {
            span_start = i;
            in_cjk = true;
        } else if !is_cjk && in_cjk {
            spans.push((span_start, i));
            in_cjk = false;
        }
    }
    if in_cjk {
        spans.push((span_start, content.len()));
    }

    spans
}

fn is_cjk_ideograph(c: char) -> bool {
    matches!(
        c,
        '\u{4E00}'..='\u{9FFF}'
            | '\u{3400}'..='\u{4DBF}'
            | '\u{20000}'..='\u{3134F}'
            | '\u{3040}'..='\u{30FF}'
            | '\u{AC00}'..='\u{D7AF}'
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pinyin_kai_fei_di() {
        let py = to_pinyin_no_tone("凯飞迪");
        assert_eq!(py, "kai-fei-di");
    }

    #[test]
    fn pinyin_kai_fei_di_variant() {
        let py = to_pinyin_no_tone("开飞地");
        assert_eq!(py, "kai-fei-di");
    }

    #[test]
    fn pinyin_jing_yan() {
        let py1 = to_pinyin_no_tone("精研");
        let py2 = to_pinyin_no_tone("精盐");
        assert_eq!(py1, py2);
    }

    #[test]
    fn pinyin_non_cjk_returns_empty() {
        let py = to_pinyin_no_tone("hello");
        assert!(py.is_empty());
    }

    #[test]
    fn cjk_spans_basic() {
        let content = "hello 你好 world 测试";
        let spans = cjk_spans(content);
        assert_eq!(spans.len(), 2);
        // "你好" starts at byte offset of "hello "
        assert_eq!(&content[spans[0].0..spans[0].1], "你好");
        assert_eq!(&content[spans[1].0..spans[1].1], "测试");
    }

    #[test]
    fn cjk_spans_no_cjk() {
        let spans = cjk_spans("hello world");
        assert!(spans.is_empty());
    }
}
