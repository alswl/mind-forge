//! Conservative authored-reference editing. Byte ranges preserve every byte
//! outside destinations, including CRLF, titles, code, and the final newline.
use crate::error::{MfError, Result};
use crate::service::util::markdown::{FenceStatus, FenceTracker, normalize_lexical, relative_path_from};
use std::collections::BTreeMap;
use std::ops::Range;
use std::path::Path;

pub type Moves = BTreeMap<String, Option<String>>;

pub fn relocated(path: &str, moves: &Moves) -> Option<String> {
    let matched = moves.iter().filter(|(from, _)| Path::new(path).starts_with(from)).max_by_key(|(from, _)| from.len());
    match matched {
        None => Some(path.into()),
        Some((from, to)) => to.as_ref().map(|to| format!("{to}{}", &path[from.len()..])),
    }
}

fn error(code: &'static str, origin: &str, offset: usize, detail: &str) -> MfError {
    MfError::Lifecycle { code, message: format!("{origin} at byte {offset}: {detail}") }
}

fn decode(target: &str) -> Option<String> {
    let mut result = Vec::new();
    let mut bytes = target.bytes();
    while let Some(b) = bytes.next() {
        if b == b'%' {
            let a = (bytes.next()? as char).to_digit(16)?;
            let b = (bytes.next()? as char).to_digit(16)?;
            result.push((a * 16 + b) as u8);
        } else if b == b'\\' {
            result.push(bytes.next()?);
        } else {
            result.push(b);
        }
    }
    String::from_utf8(result).ok()
}

fn encode(target: &str) -> String {
    target
        .chars()
        .map(|c| match c {
            ' ' => "%20".into(),
            '(' => "%28".into(),
            ')' => "%29".into(),
            '#' => "%23".into(),
            '?' => "%3F".into(),
            '%' => "%25".into(),
            '"' => "%22".into(),
            '\'' => "%27".into(),
            '<' => "%3C".into(),
            '>' => "%3E".into(),
            _ => c.to_string(),
        })
        .collect()
}

fn replacement(origin: &str, new_origin: &str, value: &str, offset: usize, moves: &Moves) -> Result<Option<String>> {
    if value.starts_with('/') && !value.starts_with("//") {
        return Err(error(
            "reference_unsupported",
            origin,
            offset,
            "site-root references have no verified repository base",
        ));
    }
    if value.is_empty()
        || value.starts_with('#')
        || value.starts_with("//")
        || value.split(['/', '#', '?']).next().is_some_and(|p| p.contains(':'))
    {
        return Ok(None);
    }
    if value.contains(['{', '}', '$', '&']) {
        return Err(error("reference_unsupported", origin, offset, "dynamic or entity-encoded local reference"));
    }
    let split = value.find(['#', '?']).unwrap_or(value.len());
    let (raw, suffix) = value.split_at(split);
    let decoded =
        decode(raw).ok_or_else(|| error("reference_unsupported", origin, offset, "invalid destination escaping"))?;
    let target = normalize_lexical(&Path::new(origin).parent().unwrap_or(Path::new("")).join(decoded));
    let target = target.to_string_lossy();
    let mapped = relocated(&target, moves)
        .ok_or_else(|| error("inbound_reference", origin, offset, &format!("references deleted {target}")))?;
    if mapped == target && origin == new_origin {
        return Ok(None);
    }
    let relative = relative_path_from(Path::new(new_origin).parent().unwrap_or(Path::new("")), Path::new(&mapped))
        .ok_or_else(|| error("reference_unsupported", origin, offset, "cannot rebase destination"))?;
    let trailing = if raw.ends_with('/') && !relative.ends_with('/') { "/" } else { "" };
    Ok(Some(format!("{}{trailing}{suffix}", encode(&relative))))
}

fn skip_space(bytes: &[u8], mut at: usize) -> usize {
    while at < bytes.len() && bytes[at].is_ascii_whitespace() {
        at += 1;
    }
    at
}

/// Return the exact destination range, without brackets or optional title.
fn destination(bytes: &[u8], start: usize) -> Option<Range<usize>> {
    let start = skip_space(bytes, start);
    if bytes.get(start) == Some(&b'<') {
        let end = bytes[start + 1..].iter().position(|b| *b == b'>')? + start + 1;
        return Some(start + 1..end);
    }
    let mut at = start;
    let mut depth = 0;
    while at < bytes.len() {
        match bytes[at] {
            b'\\' => {
                at += 2;
                continue;
            }
            b'(' => depth += 1,
            b')' if depth == 0 => break,
            b')' => depth -= 1,
            b if b.is_ascii_whitespace() && depth == 0 => break,
            _ => {}
        }
        at += 1;
    }
    (depth == 0 && at <= bytes.len()).then_some(start..at)
}

fn mask_code(text: &str) -> Vec<u8> {
    let mut masked = text.as_bytes().to_vec();
    let mut fence = FenceTracker::new();
    let mut offset = 0;
    for line in text.split_inclusive('\n') {
        let mut logical = line.trim_start_matches(' ');
        while let Some(rest) = logical.strip_prefix('>') {
            logical = rest.trim_start_matches(' ');
        }
        if let Some(rest) =
            logical.strip_prefix("- ").or_else(|| logical.strip_prefix("* ")).or_else(|| logical.strip_prefix("+ "))
        {
            logical = rest;
        }
        if fence.process_line(logical) == FenceStatus::Inside || line.starts_with("    ") || line.starts_with('\t') {
            masked[offset..offset + line.len()].fill(b' ');
        }
        offset += line.len();
    }
    let mut at = 0;
    while at < masked.len() {
        if masked[at] != b'`' || (at > 0 && masked[..at].iter().rev().take_while(|b| **b == b'\\').count() % 2 == 1) {
            at += 1;
            continue;
        }
        let start = at;
        while at < masked.len() && masked[at] == b'`' {
            at += 1;
        }
        let size = at - start;
        let mut end = at;
        while end < masked.len() {
            if masked[end] != b'`' {
                end += 1;
                continue;
            }
            let run = end;
            while end < masked.len() && masked[end] == b'`' {
                end += 1;
            }
            if end - run == size {
                masked[start..end].fill(b' ');
                at = end;
                break;
            }
        }
    }
    // HTML comments and raw code elements are not authored link destinations.
    let lower = text.to_ascii_lowercase();
    for (open, close) in
        [("<!--", "-->"), ("<script", "</script>"), ("<style", "</style>"), ("<code", "</code>"), ("<pre", "</pre>")]
    {
        let mut offset = 0;
        while let Some(start) = lower[offset..].find(open).map(|n| offset + n) {
            let end = lower[start..].find(close).map(|n| start + n + close.len()).unwrap_or(text.len());
            masked[start..end].fill(b' ');
            offset = end;
        }
    }
    masked
}

pub fn rewrite(text: &str, origin: &str, new_origin: &str, moves: &Moves) -> Result<String> {
    let masked = mask_code(text);
    let mut ranges = Vec::new();
    let mut at = 0;
    let mut brackets = 0usize;
    while at < masked.len() {
        if masked[at..].starts_with(b"[[") {
            return Err(error("reference_unsupported", origin, at, "wiki links have no verified path semantics"));
        }
        if masked[at] == b'\\' {
            at += 2;
            continue;
        }
        if brackets > 0 && masked[at..].starts_with(b"](") {
            let range = destination(&masked, at + 2)
                .ok_or_else(|| error("reference_unsupported", origin, at, "unbalanced Markdown destination"))?;
            let mut tail =
                skip_space(&masked, range.end + usize::from(masked.get(range.start.wrapping_sub(1)) == Some(&b'<')));
            if let Some(quote @ (b'\'' | b'"')) = masked.get(tail).copied() {
                tail += 1;
                loop {
                    match masked.get(tail) {
                        Some(b'\\') => tail += 2,
                        Some(b) if *b == quote => {
                            tail = skip_space(&masked, tail + 1);
                            break;
                        }
                        Some(_) => tail += 1,
                        None => return Err(error("reference_unsupported", origin, at, "unterminated link title")),
                    }
                }
            }
            if masked.get(tail) != Some(&b')') {
                return Err(error("reference_unsupported", origin, at, "malformed Markdown link"));
            }
            at = tail + 1;
            brackets -= 1;
            ranges.push(range);
            continue;
        }
        // Reference definitions, including destinations on the following line.
        if masked[at] == b'['
            && (at == 0 || masked[..at].iter().rev().take_while(|b| **b != b'\n').all(|b| *b == b' '))
            && let Some(end) = masked[at..].iter().position(|b| matches!(b, b']' | b'\n'))
            && masked.get(at + end) == Some(&b']')
            && masked.get(at + end + 1) == Some(&b':')
        {
            let end = at + end + 2;
            let range = destination(&masked, end)
                .ok_or_else(|| error("reference_unsupported", origin, at, "malformed reference definition"))?;
            at = range.end.max(end);
            ranges.push(range);
            continue;
        }
        if masked[at..].starts_with(b"]:") {
            let range = destination(&masked, at + 2)
                .ok_or_else(|| error("reference_unsupported", origin, at, "unrecognized reference definition"))?;
            if replacement(origin, new_origin, &text[range], at, moves)?.is_some() {
                return Err(error(
                    "reference_unsupported",
                    origin,
                    at,
                    "reference definition in unsupported container",
                ));
            }
        }
        if masked[at] == b'<' && masked.get(at + 1).is_some_and(u8::is_ascii_alphabetic) {
            let mut cursor = at + 1;
            let mut quote = None;
            while cursor < masked.len() {
                let b = masked[cursor];
                if let Some(q) = quote {
                    if b == q {
                        quote = None;
                    }
                } else if matches!(b, b'\'' | b'"') {
                    quote = Some(b);
                } else if b == b'>' {
                    break;
                }
                cursor += 1;
            }
            if cursor == masked.len() {
                return Err(error("reference_unsupported", origin, at, "unterminated HTML tag"));
            }
            let mut pos = at + 1;
            while pos < cursor {
                let start = pos;
                while pos < cursor && (masked[pos].is_ascii_alphanumeric() || matches!(masked[pos], b'-' | b':')) {
                    pos += 1;
                }
                if pos == start {
                    pos += 1;
                    continue;
                }
                let attr = &masked[start..pos];
                pos = skip_space(&masked, pos);
                if masked.get(pos) != Some(&b'=') {
                    continue;
                }
                pos = skip_space(&masked, pos + 1);
                let q = masked.get(pos).copied().filter(|b| matches!(b, b'\'' | b'"'));
                if q.is_some() {
                    pos += 1;
                }
                let value_start = pos;
                while pos < cursor && q.map_or(!masked[pos].is_ascii_whitespace(), |q| masked[pos] != q) {
                    pos += 1;
                }
                if attr.eq_ignore_ascii_case(b"src") || attr.eq_ignore_ascii_case(b"href") {
                    ranges.push(value_start..pos);
                } else if attr.eq_ignore_ascii_case(b"srcset")
                    || attr.eq_ignore_ascii_case(b"style")
                    || attr.ends_with(b":href")
                    || attr.ends_with(b":src")
                {
                    return Err(error(
                        "reference_unsupported",
                        origin,
                        start,
                        "srcset/CSS references require explicit maintenance",
                    ));
                }
                if q.is_some() {
                    pos += 1;
                }
            }
            at = cursor + 1;
            continue;
        }
        if masked[at] == b'[' {
            brackets += 1;
        }
        if masked[at] == b']' {
            brackets = brackets.saturating_sub(1);
        }
        at += 1;
    }
    ranges.sort_by_key(|r| r.start);
    ranges.dedup();
    let mut result = text.to_string();
    for range in ranges.into_iter().rev() {
        if let Some(value) = replacement(origin, new_origin, &text[range.clone()], range.start, moves)? {
            result.replace_range(range, &value);
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn updates_inline_images_reference_definitions_and_html_without_touching_code() {
        let moves = Moves::from([("p/assets/a".into(), Some("p/brief/assets".into()))]);
        let text = "[one](../assets/a/x.png \"title\")\r\n![two](<../assets/a/x.png>)\r\n[three][id]\r\n[id]: ../assets/a/x.png#part\r\n<img src='../assets/a/x.png'><a href=../assets/a/x.png>link</a>\r\n`[code](../assets/a/x.png)`\r\n```md\r\n![code](../assets/a/x.png)\r\n```";
        let result = rewrite(text, "p/docs/a.md", "p/docs/a.md", &moves).unwrap();
        assert_eq!(result.matches("../brief/assets/x.png").count(), 5);
        assert!(result.contains("`[code](../assets/a/x.png)`"));
        assert!(result.ends_with("```"));
        assert_eq!(result.matches("\r\n").count(), text.matches("\r\n").count());
    }
    #[test]
    fn rebases_outbound_links_and_preserves_query_fragment_and_encoded_spaces() {
        let moves = Moves::from([("p/a.md".into(), Some("q/x/a.md".into()))]);
        assert_eq!(
            rewrite("[x](assets/a%20b.png?q=1#p)", "p/a.md", "q/x/a.md", &moves).unwrap(),
            "[x](../../p/assets/a%20b.png?q=1#p)"
        );
    }
    #[test]
    fn preserves_quoted_code_escaped_labels_and_link_titles() {
        let moves = Moves::from([("p/a.md".into(), Some("p/b.md".into()))]);
        let code = "> ```md\n> [x](a.md)\n> ```\n<code>[x](a.md)</code>\n\\[escaped](a.md)";
        assert_eq!(rewrite(code, "p/index.md", "p/index.md", &moves).unwrap(), code);
        assert_eq!(
            rewrite("[x](a.md \"title [x](a.md)\")", "p/index.md", "p/index.md", &moves).unwrap(),
            "[x](b.md \"title [x](a.md)\")"
        );
        assert_eq!(
            rewrite("[[a.md]]", "p/index.md", "p/index.md", &moves).unwrap_err().kind(),
            "reference_unsupported"
        );
    }

    #[test]
    fn refuses_inbound_deletion_and_dynamic_destinations() {
        let moves = Moves::from([("p/assets/a".into(), None)]);
        assert_eq!(rewrite("[x](assets/a/file)", "p/a.md", "p/a.md", &moves).unwrap_err().kind(), "inbound_reference");
        assert_eq!(
            rewrite("[x]({{assets}}/a/file)", "p/a.md", "p/a.md", &moves).unwrap_err().kind(),
            "reference_unsupported"
        );
    }
}
