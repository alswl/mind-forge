//! Path template parsing, expansion, and slot-matched compilation.
//!
//! Supports `{date:YYYY}`, `{date:YYYY-MM}`, `{date:YYYY-MM-DD}` placeholders.
//! No `regex` dependency — hand-rolled scanner per research.md §1.

use std::path::Path;

use crate::error::{MfError, Result};

/// A parsed segment of a path template.
#[derive(Debug, Clone, PartialEq)]
pub enum Segment {
    Literal(String),
    DatePlaceholder { fmt: String },
}

/// A parsed path template consisting of ordered segments.
#[derive(Debug, Clone)]
pub struct PathTemplate {
    pub segments: Vec<Segment>,
}

/// A compiled matcher that can test filesystem paths against a template.
#[derive(Debug, Clone)]
pub struct PatternMatcher {
    compiled: Vec<CompiledSegment>,
    #[allow(dead_code)]
    most_specific_fmt: String,
}

#[derive(Debug, Clone)]
enum CompiledSegment {
    Literal(String),
    DateCapture { fmt: String, width: usize },
}

/// Result of matching a filesystem path against a pattern.
#[derive(Debug, Clone)]
pub struct PatternMatch {
    #[allow(dead_code)]
    pub captured_slots: Vec<(String, String)>,
    pub most_specific_slot_value: String,
}

fn date_format_to_strftime(fmt: &str) -> Result<&'static str> {
    match fmt {
        "YYYY" => Ok("%Y"),
        "YYYY-MM" => Ok("%Y-%m"),
        "YYYY-MM-DD" => Ok("%Y-%m-%d"),
        other => Err(MfError::UnknownPlaceholder { token: format!("{{date:{other}}}") }),
    }
}

fn date_format_width(fmt: &str) -> Option<usize> {
    match fmt {
        "YYYY" => Some(4),
        "YYYY-MM" => Some(7),
        "YYYY-MM-DD" => Some(10),
        _ => None,
    }
}

/// Returns true if `a`'s format is a prefix/subset of `b`'s format.
/// E.g. `YYYY-MM` ⊂ `YYYY-MM-DD` → true, `YYYY` ⊂ `YYYY-MM` → true.
fn is_subset_format(a: &str, b: &str) -> bool {
    b.starts_with(a) && b != a
}

impl PathTemplate {
    /// Parse a pattern string into a PathTemplate.
    ///
    /// Supported tokens: `{date:YYYY}`, `{date:YYYY-MM}`, `{date:YYYY-MM-DD}`.
    /// Unknown tokens return `MfError::UnknownPlaceholder`.
    pub fn parse(input: &str) -> Result<Self> {
        let mut segments = Vec::new();
        let mut remaining = input;

        while !remaining.is_empty() {
            if let Some(pos) = remaining.find('{') {
                // Push literal text before '{'
                if pos > 0 {
                    segments.push(Segment::Literal(remaining[..pos].to_string()));
                }
                remaining = &remaining[pos..];

                // Find the matching '}'
                let close = remaining
                    .find('}')
                    .ok_or_else(|| MfError::UnknownPlaceholder { token: "{unclosed".to_string() })?;

                let inner = &remaining[1..close]; // content between { and }

                // Parse `kind:format`
                let colon_pos = inner
                    .find(':')
                    .ok_or_else(|| MfError::UnknownPlaceholder { token: remaining[..=close].to_string() })?;

                let kind = &inner[..colon_pos];
                let fmt = &inner[colon_pos + 1..];

                if kind != "date" {
                    return Err(MfError::UnknownPlaceholder { token: format!("{{{inner}}}") });
                }

                // Validate date format
                date_format_to_strftime(fmt)?;

                segments.push(Segment::DatePlaceholder { fmt: fmt.to_string() });
                remaining = &remaining[close + 1..];
            } else {
                segments.push(Segment::Literal(remaining.to_string()));
                break;
            }
        }

        Ok(PathTemplate { segments })
    }

    /// Expand the template by substituting date placeholders.
    pub fn expand(&self, date: chrono::NaiveDate) -> String {
        let mut result = String::new();
        for segment in &self.segments {
            match segment {
                Segment::Literal(s) => result.push_str(s),
                Segment::DatePlaceholder { fmt } => {
                    let strfmt = date_format_to_strftime(fmt).expect("validated at parse time");
                    result.push_str(&date.format(strfmt).to_string());
                }
            }
        }
        result
    }

    /// Returns `true` if the template contains at least one date placeholder.
    pub fn has_date_placeholders(&self) -> bool {
        self.segments.iter().any(|s| matches!(s, Segment::DatePlaceholder { .. }))
    }

    /// Compile this template into a `PatternMatcher` for applying against filesystem paths.
    pub fn compile_matcher(&self) -> PatternMatcher {
        let mut compiled = Vec::new();
        let mut most_specific_fmt = String::new();
        let mut longest_width = 0usize;

        for segment in &self.segments {
            match segment {
                Segment::Literal(s) => compiled.push(CompiledSegment::Literal(s.clone())),
                Segment::DatePlaceholder { fmt } => {
                    let width = date_format_width(fmt).unwrap_or(10);
                    if width > longest_width {
                        longest_width = width;
                        most_specific_fmt = fmt.clone();
                    }
                    compiled.push(CompiledSegment::DateCapture { fmt: fmt.clone(), width });
                }
            }
        }

        PatternMatcher { compiled, most_specific_fmt }
    }

    /// Validate that at most one non-date slot exists, and all date slots are subset-related.
    pub fn validate_slot_redundancy(&self) -> Result<()> {
        let date_slots: Vec<&str> = self
            .segments
            .iter()
            .filter_map(|s| match s {
                Segment::DatePlaceholder { fmt } => Some(fmt.as_str()),
                _ => None,
            })
            .collect();

        if date_slots.len() <= 1 {
            return Ok(());
        }

        // Check pairwise subset relation: for any two date slots,
        // one must be a subset of the other (e.g. YYYY-MM ⊂ YYYY-MM-DD).
        for i in 0..date_slots.len() {
            for j in 0..date_slots.len() {
                if i != j
                    && !is_subset_format(date_slots[i], date_slots[j])
                    && !is_subset_format(date_slots[j], date_slots[i])
                {
                    return Err(MfError::MultiSlotTemplate { template_name: "(unknown)".to_string() });
                }
            }
        }

        Ok(())
    }
}

impl PatternMatcher {
    /// Try to match a filesystem path against this compiled pattern.
    ///
    /// Each segment is matched in order: literals must match exactly, date captures
    /// consume the expected number of characters.
    pub fn try_match(&self, path: &Path) -> Option<PatternMatch> {
        let path_str = path.to_string_lossy();
        let mut pos = 0usize;
        let mut captured_slots = Vec::new();

        for segment in &self.compiled {
            match segment {
                CompiledSegment::Literal(lit) => {
                    if path_str[pos..].starts_with(lit) {
                        pos += lit.len();
                    } else {
                        return None;
                    }
                }
                CompiledSegment::DateCapture { fmt, width } => {
                    let end = pos + width;
                    if end > path_str.len() {
                        return None;
                    }
                    let value = &path_str[pos..end];
                    // Validate it looks like a date component
                    if value.chars().all(|c| c.is_ascii_digit() || c == '-') {
                        captured_slots.push((fmt.clone(), value.to_string()));
                        pos = end;
                    } else {
                        return None;
                    }
                }
            }
        }

        // Must consume entire path
        if pos != path_str.len() {
            return None;
        }

        let most_specific = captured_slots
            .iter()
            .max_by_key(|(fmt, _)| date_format_width(fmt).unwrap_or(0))
            .map(|(_, v)| v.clone())
            .unwrap_or_default();

        Some(PatternMatch { captured_slots, most_specific_slot_value: most_specific })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    // ── parse tests ──

    #[test]
    fn parse_literal_only() {
        let tmpl = PathTemplate::parse("/tmp/output/").unwrap();
        assert_eq!(tmpl.segments.len(), 1);
        assert_eq!(tmpl.segments[0], Segment::Literal("/tmp/output/".to_string()));
    }

    #[test]
    fn parse_date_yyyy() {
        let tmpl = PathTemplate::parse("{date:YYYY}").unwrap();
        assert_eq!(tmpl.segments.len(), 1);
        assert_eq!(tmpl.segments[0], Segment::DatePlaceholder { fmt: "YYYY".to_string() });
    }

    #[test]
    fn parse_date_yyyy_mm() {
        let tmpl = PathTemplate::parse("{date:YYYY-MM}").unwrap();
        assert_eq!(tmpl.segments.len(), 1);
        assert_eq!(tmpl.segments[0], Segment::DatePlaceholder { fmt: "YYYY-MM".to_string() });
    }

    #[test]
    fn parse_date_yyyy_mm_dd() {
        let tmpl = PathTemplate::parse("{date:YYYY-MM-DD}").unwrap();
        assert_eq!(tmpl.segments.len(), 1);
        assert_eq!(tmpl.segments[0], Segment::DatePlaceholder { fmt: "YYYY-MM-DD".to_string() });
    }

    #[test]
    fn parse_mixed_literal_and_placeholder() {
        let tmpl = PathTemplate::parse("/reports/{date:YYYY-MM}/daily/").unwrap();
        assert_eq!(tmpl.segments.len(), 3);
        assert_eq!(tmpl.segments[0], Segment::Literal("/reports/".to_string()));
        assert_eq!(tmpl.segments[1], Segment::DatePlaceholder { fmt: "YYYY-MM".to_string() });
        assert_eq!(tmpl.segments[2], Segment::Literal("/daily/".to_string()));
    }

    #[test]
    fn parse_unknown_token_errors() {
        let err = PathTemplate::parse("{quarter:QQ}").unwrap_err();
        assert!(matches!(err, MfError::UnknownPlaceholder { .. }));
    }

    #[test]
    fn parse_empty_pattern() {
        let tmpl = PathTemplate::parse("").unwrap();
        assert!(tmpl.segments.is_empty());
    }

    #[test]
    fn parse_unclosed_brace_errors() {
        let err = PathTemplate::parse("/reports/{date:YYYY").unwrap_err();
        assert!(matches!(err, MfError::UnknownPlaceholder { .. }));
    }

    #[test]
    fn parse_unknown_kind_errors() {
        let err = PathTemplate::parse("{lang:en}").unwrap_err();
        assert!(matches!(err, MfError::UnknownPlaceholder { .. }));
    }

    // ── expand tests ──

    #[test]
    fn expand_date_yyyy() {
        let tmpl = PathTemplate::parse("{date:YYYY}").unwrap();
        let date = NaiveDate::from_ymd_opt(2026, 5, 15).unwrap();
        assert_eq!(tmpl.expand(date), "2026");
    }

    #[test]
    fn expand_date_yyyy_mm() {
        let tmpl = PathTemplate::parse("{date:YYYY-MM}").unwrap();
        let date = NaiveDate::from_ymd_opt(2026, 5, 15).unwrap();
        assert_eq!(tmpl.expand(date), "2026-05");
    }

    #[test]
    fn expand_date_yyyy_mm_dd() {
        let tmpl = PathTemplate::parse("{date:YYYY-MM-DD}").unwrap();
        let date = NaiveDate::from_ymd_opt(2026, 5, 15).unwrap();
        assert_eq!(tmpl.expand(date), "2026-05-15");
    }

    #[test]
    fn expand_mixed_literal_and_placeholder() {
        let tmpl = PathTemplate::parse("/reports/{date:YYYY-MM}/daily/").unwrap();
        let date = NaiveDate::from_ymd_opt(2026, 5, 15).unwrap();
        assert_eq!(tmpl.expand(date), "/reports/2026-05/daily/");
    }

    #[test]
    fn expand_multiple_placeholders() {
        let tmpl = PathTemplate::parse("outputs/{date:YYYY-MM}/{date:YYYY-MM-DD}.md").unwrap();
        let date = NaiveDate::from_ymd_opt(2026, 5, 15).unwrap();
        assert_eq!(tmpl.expand(date), "outputs/2026-05/2026-05-15.md");
    }

    // ── compile_matcher / PatternMatcher tests ──

    #[test]
    fn matcher_single_date_slot() {
        let tmpl = PathTemplate::parse("outputs/{date:YYYY-MM-DD}.md").unwrap();
        let matcher = tmpl.compile_matcher();
        let matched = matcher.try_match(Path::new("outputs/2026-05-15.md")).unwrap();
        assert_eq!(matched.most_specific_slot_value, "2026-05-15");
        assert_eq!(matched.captured_slots.len(), 1);
    }

    #[test]
    fn matcher_nested_date_slots() {
        let tmpl = PathTemplate::parse("outputs/{date:YYYY-MM}/{date:YYYY-MM-DD}.md").unwrap();
        let matcher = tmpl.compile_matcher();
        let matched = matcher.try_match(Path::new("outputs/2026-05/2026-05-15.md")).unwrap();
        assert_eq!(matched.most_specific_slot_value, "2026-05-15");
        assert_eq!(matched.captured_slots.len(), 2);
    }

    #[test]
    fn matcher_no_match() {
        let tmpl = PathTemplate::parse("outputs/{date:YYYY-MM-DD}.md").unwrap();
        let matcher = tmpl.compile_matcher();
        assert!(matcher.try_match(Path::new("other/2026-05-15.md")).is_none());
    }

    // ── validate_slot_redundancy tests ──

    #[test]
    fn single_slot_accepts() {
        let tmpl = PathTemplate::parse("{date:YYYY-MM-DD}.md").unwrap();
        assert!(tmpl.validate_slot_redundancy().is_ok());
    }

    #[test]
    fn nested_date_slots_accepts() {
        let tmpl = PathTemplate::parse("outputs/{date:YYYY-MM}/{date:YYYY-MM-DD}.md").unwrap();
        assert!(tmpl.validate_slot_redundancy().is_ok());
    }

    #[test]
    fn non_date_multi_slot_rejects() {
        // This would be rejected at parse time (unknown kind), but if we had
        // two non-date date slots that are not subset-related, it would fail here.
        // For now, this is a parse-level error since we only support `date` kind.
        let err = PathTemplate::parse("{lang:en}/{date:YYYY-MM-DD}.md").unwrap_err();
        assert!(matches!(err, MfError::UnknownPlaceholder { .. }));
    }

    #[test]
    fn matcher_picks_most_specific_format() {
        let tmpl = PathTemplate::parse("outputs/{date:YYYY-MM}/{date:YYYY-MM-DD}.md").unwrap();
        let matcher = tmpl.compile_matcher();
        assert_eq!(matcher.most_specific_fmt, "YYYY-MM-DD");
    }
}
