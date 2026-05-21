pub(crate) enum FrontMatterDecision {
    None,
    Skip,
    Present { end_byte_offset: usize },
}

/// Parse front-matter block to detect `mf_term_lint: skip` / `mf-term-lint: skip`.
pub(crate) fn parse_front_matter_skip_flag(content: &str) -> FrontMatterDecision {
    let bytes = content.as_bytes();
    if bytes.len() < 5 {
        return FrontMatterDecision::None;
    }
    if !(bytes[0] == b'-' && bytes[1] == b'-' && bytes[2] == b'-') {
        return FrontMatterDecision::None;
    }
    let after_opener = if bytes[3] == b'\n' {
        4
    } else if bytes.len() > 4 && bytes[3] == b'\r' && bytes[4] == b'\n' {
        5
    } else {
        return FrontMatterDecision::None;
    };

    let closing = find_front_matter_close(bytes, after_opener);
    let end_offset = match closing {
        Some(pos) => pos,
        None => return FrontMatterDecision::None,
    };

    let fm_text = &content[after_opener..end_offset];
    for line in fm_text.lines() {
        let trimmed = line.trim();
        if trimmed == "mf_term_lint: skip" || trimmed == "mf-term-lint: skip" {
            return FrontMatterDecision::Skip;
        }
    }

    let close_end =
        if end_offset + 4 <= bytes.len() && bytes[end_offset + 3] == b'\r' { end_offset + 5 } else { end_offset + 4 };
    FrontMatterDecision::Present { end_byte_offset: close_end }
}

fn find_front_matter_close(bytes: &[u8], start: usize) -> Option<usize> {
    let mut i = start;
    while i + 3 < bytes.len() {
        if bytes[i] == b'-' && bytes[i + 1] == b'-' && bytes[i + 2] == b'-' && (i == start || bytes[i - 1] == b'\n') {
            return Some(i);
        }
        i += 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fm_none_when_no_front_matter() {
        let content = "Just text\nno front matter\n";
        assert!(matches!(parse_front_matter_skip_flag(content), FrontMatterDecision::None));
    }

    #[test]
    fn fm_present_no_skip() {
        let content = "---\ntitle: test\n---\nbody text";
        match parse_front_matter_skip_flag(content) {
            FrontMatterDecision::Present { end_byte_offset } => {
                assert!(end_byte_offset > 0);
            }
            _ => panic!("expected Present"),
        }
    }

    #[test]
    fn fm_skip_mf_term_lint() {
        let content = "---\nmf_term_lint: skip\n---\nbody text";
        assert!(matches!(parse_front_matter_skip_flag(content), FrontMatterDecision::Skip));
    }

    #[test]
    fn fm_skip_mf_dash_term_lint() {
        let content = "---\nmf-term-lint: skip\n---\nbody text";
        assert!(matches!(parse_front_matter_skip_flag(content), FrontMatterDecision::Skip));
    }

    #[test]
    fn fm_crlf_handling() {
        let content = "---\r\nmf_term_lint: skip\r\n---\r\nbody";
        assert!(matches!(parse_front_matter_skip_flag(content), FrontMatterDecision::Skip));
    }

    #[test]
    fn fm_skip_leading_spaces() {
        let content = "---\n  mf_term_lint: skip\n---\nbody";
        assert!(matches!(parse_front_matter_skip_flag(content), FrontMatterDecision::Skip));
    }
}
