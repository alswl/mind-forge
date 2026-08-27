//! Byte-scoped splicing of a single top-level YAML key within a larger
//! document, so a writer confined to one key can leave every other key
//! byte-identical (spec 075 FR-013/I-2).
//!
//! Parsing a whole document into a `serde_yaml::Value` and re-serializing it
//! — even touching only one key of the resulting mapping — reformats every
//! *other* key too (list-item indent style, scalar quoting), because the
//! reformatting happens at serialize time for the whole tree, not per key.
//! Splicing at the raw-text level is the only way to leave untouched keys
//! byte-for-byte identical.

/// Byte range of a top-level (column-0) YAML key's block within `text`,
/// spanning from its `key:` line through the line before the next top-level
/// key (or end of file). `None` when the key is absent.
pub(crate) fn top_level_key_span(text: &str, key: &str) -> Option<(usize, usize)> {
    let bare = format!("{key}:");
    let prefixed = format!("{key}: ");
    let mut start = None;
    let mut end = None;
    // Start of the current run of blank/comment lines. Such a run that
    // immediately precedes the next top-level key (or EOF) belongs to what
    // follows, so it must stay outside the replaced span — otherwise the
    // blank line separating one key from the next is silently deleted.
    let mut trailer: Option<usize> = None;
    let mut offset = 0usize;
    for line in text.split_inclusive('\n') {
        let trimmed = line.trim_end_matches(['\n', '\r']);
        match start {
            None => {
                if trimmed == bare || trimmed.starts_with(&prefixed) {
                    start = Some(offset);
                }
            }
            Some(_) => {
                if trimmed.is_empty() || trimmed.starts_with('#') {
                    trailer.get_or_insert(offset);
                } else if trimmed.starts_with([' ', '\t', '-']) {
                    // A column-0 `- item` is a valid, and exactly serde_yaml's
                    // own, style for a sequence directly under its parent key —
                    // it continues the block, it is not a new top-level key.
                    // Anything indented continues it too.
                    trailer = None;
                } else {
                    end = Some(trailer.unwrap_or(offset));
                    break;
                }
            }
        }
        offset += line.len();
    }
    start.map(|s| (s, end.unwrap_or_else(|| trailer.unwrap_or(text.len()))))
}

/// Replace (or append) a single top-level key's block in raw YAML text,
/// leaving every byte outside that block untouched.
pub(crate) fn splice_top_level_key(original: &str, key: &str, rendered_block: &str) -> String {
    match top_level_key_span(original, key) {
        Some((start, end)) => {
            let mut out = String::with_capacity(original.len() + rendered_block.len());
            out.push_str(&original[..start]);
            out.push_str(rendered_block);
            out.push_str(&original[end..]);
            out
        }
        None => {
            let mut out = original.to_string();
            if !out.is_empty() && !out.ends_with('\n') {
                out.push('\n');
            }
            out.push_str(rendered_block);
            out
        }
    }
}

/// Extract a top-level key's raw block verbatim, if present.
pub(crate) fn extract_top_level_key<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    top_level_key_span(text, key).map(|(s, e)| &text[s..e])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_top_level_key_returns_the_key_s_own_block() {
        let text = "project: alpha\nsources:\n  - name: a\nterms:\n  - term: API\n";
        assert_eq!(extract_top_level_key(text, "sources"), Some("sources:\n  - name: a\n"));
    }

    #[test]
    fn extract_top_level_key_is_none_when_absent() {
        let text = "project: alpha\nterms:\n  - term: API\n";
        assert_eq!(extract_top_level_key(text, "sources"), None);
    }
}
