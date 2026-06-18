pub(crate) struct FixSpan {
    pub(crate) start: usize,
    pub(crate) end: usize,
    pub(crate) replacement: String,
}

/// Filter out overlapping spans. Spans must be sorted by start. When two spans
/// overlap (e.g. "mini pass" and "pass" matching the same text region), the first
/// (longer / earlier-starting) span wins and the nested one is dropped. This
/// prevents slice-index panics in `apply_fixes` when `last_end > span.start`.
pub(crate) fn deduplicate_spans(spans: &mut Vec<FixSpan>) {
    let mut last_end = 0;
    spans.retain(|s| {
        if s.start < last_end {
            false
        } else {
            last_end = s.end;
            true
        }
    });
}

pub(crate) fn apply_fixes(content: &[u8], spans: &[FixSpan]) -> Vec<u8> {
    let mut result = Vec::with_capacity(content.len());
    let mut last_end = 0;
    for span in spans {
        // Defensive: skip spans that overlap with an already-applied fix.
        if span.start < last_end {
            continue;
        }
        result.extend_from_slice(&content[last_end..span.start]);
        result.extend_from_slice(span.replacement.as_bytes());
        last_end = span.end;
    }
    if last_end < content.len() {
        result.extend_from_slice(&content[last_end..]);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_single_fix() {
        let content = b"hello world";
        let spans = vec![FixSpan { start: 6, end: 11, replacement: "there".to_string() }];
        let result = apply_fixes(content, &spans);
        assert_eq!(result, b"hello there");
    }

    #[test]
    fn apply_multiple_fixes() {
        let content = b"aa bb aa";
        let spans = vec![
            FixSpan { start: 0, end: 2, replacement: "x".to_string() },
            FixSpan { start: 6, end: 8, replacement: "y".to_string() },
        ];
        let result = apply_fixes(content, &spans);
        assert_eq!(result, b"x bb y");
    }

    #[test]
    fn overlapping_spans_first_wins() {
        // "mini pass" at [0..9] and nested "pass" at [5..9]
        let content = b"mini pass";
        let mut spans = vec![
            FixSpan { start: 0, end: 9, replacement: "mini PaaS".to_string() },
            FixSpan { start: 5, end: 9, replacement: "PaaS".to_string() },
        ];
        spans.sort_by_key(|s| s.start);
        deduplicate_spans(&mut spans);
        assert_eq!(spans.len(), 1, "nested span should be removed");
        assert_eq!(spans[0].start, 0);
        let result = apply_fixes(content, &spans);
        assert_eq!(result, b"mini PaaS");
    }

    #[test]
    fn overlapping_spans_apply_fixes_skips_nested() {
        // Even without dedup, apply_fixes must survive overlapping spans
        let content = b"mini pass end";
        let spans = vec![
            FixSpan { start: 0, end: 9, replacement: "mini PaaS".to_string() },
            FixSpan { start: 5, end: 9, replacement: "PaaS".to_string() },
        ];
        let result = apply_fixes(content, &spans);
        assert_eq!(result, b"mini PaaS end");
    }

    #[test]
    fn deduplicate_spans_non_overlapping_kept() {
        let mut spans = vec![
            FixSpan { start: 0, end: 2, replacement: "X".to_string() },
            FixSpan { start: 5, end: 7, replacement: "Y".to_string() },
        ];
        spans.sort_by_key(|s| s.start);
        deduplicate_spans(&mut spans);
        assert_eq!(spans.len(), 2);
    }

    #[test]
    fn deduplicate_spans_empty() {
        let mut spans: Vec<FixSpan> = vec![];
        deduplicate_spans(&mut spans);
        assert!(spans.is_empty());
    }
}
