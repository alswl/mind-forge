pub(super) struct FixSpan {
    pub(super) start: usize,
    pub(super) end: usize,
    pub(super) replacement: String,
}

pub(super) fn apply_fixes(content: &[u8], spans: &[FixSpan]) -> Vec<u8> {
    let mut result = Vec::with_capacity(content.len());
    let mut last_end = 0;
    for span in spans {
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
}
