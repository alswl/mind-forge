use std::io::Write;

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
}

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
    fn test_warn_no_color_strips_ansi() {
        let mut buf: Vec<u8> = Vec::new();
        let mut ctx = DeprecationContext::new(&mut buf, true);
        ctx.warn_subject("\x1b[31m--status\x1b[0m", "--set status=<value>");
        let output = String::from_utf8(buf).unwrap();
        assert!(!output.contains('\x1b'));
        assert!(output.contains("[deprecated] --status is deprecated"));
    }
}
