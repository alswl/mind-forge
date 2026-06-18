use std::collections::BTreeSet;

use crate::model::term::{Boundary, FixKind, MatchKind, TermFinding};

use super::scan::{byte_offset_to_line_col, is_cjk_ideograph, CorrectionRef, InternalFinding};

/// Convert CJK characters in `s` to pinyin (first reading, no tone), joined by hyphens.
/// Non-CJK characters are skipped. Returns an empty string if no CJK chars found.
pub(crate) fn to_pinyin_no_tone(s: &str) -> String {
    use pinyin::ToPinyin;
    let parts: Vec<&str> = s.to_pinyin().filter_map(|p| p.map(|pp| pp.plain())).collect();
    parts.join("-")
}

struct PinyinEntry<'a> {
    cref: &'a CorrectionRef<'a>,
    pinyin: String,
    char_len: usize,
}

/// Scan `content` for pinyin matches from `corrections` whose `match_kind == Pinyin`.
/// `sanitized` is the byte-mask produced by `strip_exempt_regions`; bytes that are
/// 0 in `sanitized` (fenced code, inline code, HTML comments, URLs) are skipped
/// per FR-407.
pub(crate) fn scan_for_pinyin(
    content: &str,
    sanitized: &[u8],
    rel_path: &str,
    corrections: &[CorrectionRef<'_>],
    findings: &mut Vec<TermFinding>,
    internal_findings: &mut Vec<InternalFinding>,
    claimed: &mut BTreeSet<(String, usize)>,
) {
    let entries: Vec<PinyinEntry<'_>> = corrections
        .iter()
        .filter(|c| c.match_kind == MatchKind::Pinyin)
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

    for run in cjk_runs(content, sanitized) {
        for entry in &entries {
            if run.len() < entry.char_len {
                continue;
            }
            let max_start = run.len() - entry.char_len;
            for window_start in 0..=max_start {
                let (window_byte_start, _) = run[window_start];
                let key = (rel_path.to_string(), window_byte_start);
                if claimed.contains(&key) {
                    continue;
                }

                let window_text: String =
                    run[window_start..window_start + entry.char_len].iter().map(|(_, c)| *c).collect();
                let window_pinyin = to_pinyin_no_tone(&window_text);
                if window_pinyin.is_empty() || window_pinyin != entry.pinyin {
                    continue;
                }

                // Skip if window text equals original (already typed correctly).
                if window_text == entry.cref.original {
                    continue;
                }

                claimed.insert(key);

                let (line, col) = byte_offset_to_line_col(content, window_byte_start);
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
                    match_kind: MatchKind::Pinyin,
                    fix_kind: FixKind::Suggested, // FR-404: pinyin is always suggested
                    boundary: Boundary::Loose,    // pinyin never opts into standalone
                });

                internal_findings.push(InternalFinding {
                    path: rel_path.to_string(),
                    byte_offset: window_byte_start,
                    original_len: window_byte_len,
                    original: window_text,
                    correct: entry.cref.correct.to_string(),
                    is_ambiguous: entry.cref.is_ambiguous,
                    fix_kind: FixKind::Suggested,
                    yaml_index: entry.cref.yaml_index,
                });
            }
        }
    }
}

/// Return runs of consecutive CJK characters in `content`, excluding any character
/// whose start byte in `sanitized` is 0 (exempt regions per `strip_exempt_regions`).
/// Each run is `Vec<(byte_offset, char)>` in source order.
fn cjk_runs(content: &str, sanitized: &[u8]) -> Vec<Vec<(usize, char)>> {
    let mut runs = Vec::new();
    let mut current: Vec<(usize, char)> = Vec::new();
    for (i, c) in content.char_indices() {
        let exempt = sanitized.get(i).copied() == Some(0);
        if is_cjk_ideograph(c) && !exempt {
            current.push((i, c));
        } else if !current.is_empty() {
            runs.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        runs.push(current);
    }
    runs
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
    fn cjk_runs_basic() {
        let content = "hello 你好 world 测试";
        let sanitized = content.as_bytes().to_vec();
        let runs = cjk_runs(content, &sanitized);
        assert_eq!(runs.len(), 2);
        let first: String = runs[0].iter().map(|(_, c)| *c).collect();
        let second: String = runs[1].iter().map(|(_, c)| *c).collect();
        assert_eq!(first, "你好");
        assert_eq!(second, "测试");
    }

    #[test]
    fn cjk_runs_no_cjk() {
        let content = "hello world";
        let runs = cjk_runs(content, content.as_bytes());
        assert!(runs.is_empty());
    }

    #[test]
    fn cjk_runs_skip_exempt_bytes() {
        // Simulate: "你好" inside an exempt region — sanitized has 0s on those bytes.
        let content = "前 你好 后";
        let mut sanitized = content.as_bytes().to_vec();
        // Blank "你好" (3 bytes each in UTF-8).
        let start = content.find('你').unwrap();
        for b in &mut sanitized[start..start + "你好".len()] {
            *b = 0;
        }
        let runs = cjk_runs(content, &sanitized);
        // 前 and 后 are still kept as two separate single-char runs.
        let collected: Vec<String> = runs.iter().map(|r| r.iter().map(|(_, c)| *c).collect()).collect();
        assert_eq!(collected, vec!["前".to_string(), "后".to_string()]);
    }
}
