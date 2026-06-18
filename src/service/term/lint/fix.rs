#[derive(Debug)]
pub(crate) struct FixSpan {
    pub(crate) start: usize,
    pub(crate) end: usize,
    pub(crate) replacement: String,
    /// Monotonic per-file counter recording the order in which this candidate
    /// was emitted by the scanner. Used as the final tie-breaker in
    /// `deduplicate_spans` so that same-(start, end) candidates pick the
    /// earlier-declared rule.
    pub(crate) declaration_order: usize,
}

/// Filter out overlapping spans. Spans are sorted by (start ASC, end DESC,
/// decl_order ASC) before the overlap pass so that when two candidates share
/// the same start offset the longest original wins (FR-006). On a same-(start,
/// end) tie the earlier-declared rule wins.
pub(crate) fn deduplicate_spans(spans: &mut Vec<FixSpan>) {
    spans.sort_by(|a, b| {
        a.start
            .cmp(&b.start)
            .then_with(|| b.end.cmp(&a.end))
            .then_with(|| a.declaration_order.cmp(&b.declaration_order))
    });
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

    fn span(start: usize, end: usize, replacement: &str, declaration_order: usize) -> FixSpan {
        FixSpan { start, end, replacement: replacement.to_string(), declaration_order }
    }

    #[test]
    fn apply_single_fix() {
        let content = b"hello world";
        let spans = vec![span(6, 11, "there", 0)];
        let result = apply_fixes(content, &spans);
        assert_eq!(result, b"hello there");
    }

    #[test]
    fn apply_multiple_fixes() {
        let content = b"aa bb aa";
        let spans = vec![span(0, 2, "x", 0), span(6, 8, "y", 1)];
        let result = apply_fixes(content, &spans);
        assert_eq!(result, b"x bb y");
    }

    #[test]
    fn overlapping_spans_first_wins() {
        // "mini pass" at [0..9] and nested "pass" at [5..9]
        let content = b"mini pass";
        let mut spans = vec![span(0, 9, "mini PaaS", 0), span(5, 9, "PaaS", 1)];
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
        let spans = vec![span(0, 9, "mini PaaS", 0), span(5, 9, "PaaS", 1)];
        let result = apply_fixes(content, &spans);
        assert_eq!(result, b"mini PaaS end");
    }

    #[test]
    fn deduplicate_spans_non_overlapping_kept() {
        let mut spans = vec![span(0, 2, "X", 0), span(5, 7, "Y", 1)];
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

    // ── Longest-match tie-breaker (FR-006) — RED until US4 lands ─────────────

    #[test]
    fn same_start_longest_end_wins_even_when_declared_later() {
        // Input: "XXX 3.0"
        // Rule A (declared first): "X" -> "AIDC"           span [0..1] decl=0
        // Rule B (declared later): "XXX" -> "AIDC-Long"    span [0..3] decl=1
        // Expected: B (longer) wins. Current code keeps A and drops B.
        let content = b"XXX 3.0";
        let mut spans = vec![span(0, 1, "AIDC", 0), span(0, 3, "AIDC-Long", 1)];
        deduplicate_spans(&mut spans);
        assert_eq!(spans.len(), 1, "exactly one span must survive: {:?}", spans);
        assert_eq!(spans[0].start, 0);
        assert_eq!(spans[0].end, 3, "longer-end span must win at the same start");
        assert_eq!(spans[0].replacement, "AIDC-Long");
        let result = apply_fixes(content, &spans);
        assert_eq!(result, b"AIDC-Long 3.0", "output must not double-write the short rule");
    }

    #[test]
    fn same_start_longest_end_wins_when_declared_first() {
        // Same as above but declaration order reversed — winner is identical.
        let mut spans = vec![span(0, 3, "AIDC-Long", 0), span(0, 1, "AIDC", 1)];
        deduplicate_spans(&mut spans);
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].end, 3);
        assert_eq!(spans[0].replacement, "AIDC-Long");
    }

    #[test]
    fn same_start_same_end_earlier_declaration_wins() {
        // Pure tie on (start, end) — declaration_order ASC decides.
        let mut spans = vec![span(0, 4, "B-WINS", 5), span(0, 4, "A-WINS", 2)];
        deduplicate_spans(&mut spans);
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].replacement, "A-WINS", "earlier declaration_order must win");
    }

    #[test]
    fn cross_rule_overlap_keeps_earliest_then_longest() {
        // Three candidates from three different rules. Expected ordering after dedup:
        //   [0..3] "AIDC-Long" — longest at start 0
        //   [4..7] "NEXT"      — non-overlapping
        // The mid-span [1..4] "DROPPED" overlaps with [0..3] and must be dropped.
        let mut spans = vec![span(0, 3, "AIDC-Long", 0), span(1, 4, "DROPPED", 1), span(4, 7, "NEXT", 2)];
        deduplicate_spans(&mut spans);
        assert_eq!(spans.iter().map(|s| s.replacement.as_str()).collect::<Vec<_>>(), vec!["AIDC-Long", "NEXT"]);
    }

    #[test]
    fn no_panic_on_overlapping_candidates_regression_1c05809() {
        // Regression for commit 1c05809: overlapping candidates must not panic
        // through deduplicate_spans + apply_fixes. We additionally assert the
        // NEW semantics (longest wins) on the same input.
        let content = b"mini pass";
        let mut spans = vec![
            span(5, 9, "PaaS", 0),      // shorter, declared first
            span(0, 9, "mini PaaS", 1), // longer, declared later — must win
        ];
        deduplicate_spans(&mut spans);
        let result = apply_fixes(content, &spans);
        assert_eq!(result, b"mini PaaS");
    }
}
