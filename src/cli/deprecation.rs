// Dead-code allowed until wired into US4 dispatch handlers
#![allow(dead_code)]

use std::io::Write;

// ---------------------------------------------------------------------------
// DeprecationRegistry — 5 deprecation classes with warning templates
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeprecationId {
    D1a, // --status → --set status=<value>
    D1b, // --target-url → --set url=<value>
    D2a, // --type <source-kind-value> → --source-kind
    D2b, // --type <file-kind-value> → --file-kind
    D3,  // positional NAME → PATH
    D4a, // --original → --alias
    D4b, // --correct → --term
    D5,  // term list --term → term show
}

impl DeprecationId {
    pub fn template(self) -> (&'static str, &'static str) {
        match self {
            Self::D1a => ("--status", "--set status=<value>"),
            Self::D1b => ("--target-url", "--set url=<value>"),
            Self::D2a => ("--type with value '{}'", "--source-kind {}"),
            Self::D2b => ("--type with value '{}'", "--file-kind {}"),
            Self::D3 => ("positional NAME", "full PATH (e.g., sources/yuque/foo.md)"),
            Self::D4a => ("--original", "--alias <variant>"),
            Self::D4b => ("--correct", "--term <canonical>"),
            Self::D5 => ("term list --term <X>", "term show <X>"),
        }
    }
}

// ---------------------------------------------------------------------------
// DeprecationContext — unified stderr deprecation warning writer
// ---------------------------------------------------------------------------

pub struct DeprecationContext<'a> {
    stderr: &'a mut dyn Write,
    no_color: bool,
}

impl<'a> DeprecationContext<'a> {
    pub fn new(stderr: &'a mut dyn Write, no_color: bool) -> Self {
        Self { stderr, no_color }
    }

    /// Emit a subject-level deprecation warning.
    /// Example: `[deprecated] --status is deprecated, use --set status=<value> instead`
    pub fn warn_subject(&mut self, subject: &str, replacement: &str) {
        let msg = format!("[deprecated] {subject} is deprecated, use {replacement} instead\n");
        let output = if self.no_color { strip_ansi(&msg) } else { msg };
        let _ = write!(self.stderr, "{output}");
        let _ = self.stderr.flush();
    }

    /// Emit a value-level deprecation warning with format args.
    /// Example: `[deprecated] --type with value 'yuque' is deprecated, use --source-kind yuque instead`
    pub fn warn_value(&mut self, template_subject: &str, template_replacement: &str, value: &str) {
        let subject = template_subject.replace("{}", value);
        let replacement = template_replacement.replace("{}", value);
        let msg = format!("[deprecated] {subject} is deprecated, use {replacement} instead\n");
        let output = if self.no_color { strip_ansi(&msg) } else { msg };
        let _ = write!(self.stderr, "{output}");
        let _ = self.stderr.flush();
    }

    /// Emit a pre-formatted deprecation warning by ID with value interpolation (for D2/D4).
    pub fn warn_by_id(&mut self, id: DeprecationId, value: Option<&str>) {
        let (subject_tpl, replacement_tpl) = id.template();
        match value {
            Some(v) => self.warn_value(subject_tpl, replacement_tpl, v),
            None => self.warn_subject(subject_tpl, replacement_tpl),
        }
    }
}

// ---------------------------------------------------------------------------
// ANSI stripping helper
// ---------------------------------------------------------------------------

fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_escape = false;
    for c in s.chars() {
        if in_escape {
            if c == 'm' {
                in_escape = false;
            }
            continue;
        }
        if c == '\x1b' {
            in_escape = true;
            continue;
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_ansi() {
        let input = "\x1b[31mhello\x1b[0m world";
        assert_eq!(strip_ansi(input), "hello world");
    }

    #[test]
    fn test_strip_ansi_no_ansi() {
        assert_eq!(strip_ansi("hello world"), "hello world");
    }

    #[test]
    fn test_warn_subject_output() {
        let mut buf: Vec<u8> = Vec::new();
        let mut ctx = DeprecationContext::new(&mut buf, false);
        ctx.warn_subject("--status", "--set status=<value>");
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("[deprecated] --status is deprecated, use --set status=<value> instead"));
    }

    #[test]
    fn test_warn_value_output() {
        let mut buf: Vec<u8> = Vec::new();
        let mut ctx = DeprecationContext::new(&mut buf, false);
        ctx.warn_value("--type with value '{}'", "--source-kind {}", "yuque");
        let output = String::from_utf8(buf).unwrap();
        assert!(
            output.contains("[deprecated] --type with value 'yuque' is deprecated, use --source-kind yuque instead")
        );
    }

    #[test]
    fn test_warn_by_id_d1a() {
        let mut buf: Vec<u8> = Vec::new();
        let mut ctx = DeprecationContext::new(&mut buf, false);
        ctx.warn_by_id(DeprecationId::D1a, None);
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("--status is deprecated"));
    }

    #[test]
    fn test_warn_by_id_d2a() {
        let mut buf: Vec<u8> = Vec::new();
        let mut ctx = DeprecationContext::new(&mut buf, false);
        ctx.warn_by_id(DeprecationId::D2a, Some("yuque"));
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("--type with value 'yuque' is deprecated, use --source-kind yuque instead"));
    }

    #[test]
    fn test_warn_no_color_strips_ansi() {
        let mut buf: Vec<u8> = Vec::new();
        let mut ctx = DeprecationContext::new(&mut buf, true);
        // Simulate a message that could contain ANSI codes
        ctx.warn_subject("\x1b[31m--status\x1b[0m", "--set status=<value>");
        let output = String::from_utf8(buf).unwrap();
        assert!(!output.contains('\x1b'));
        assert!(output.contains("[deprecated] --status is deprecated"));
    }
}
