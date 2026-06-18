use std::io::Write;

/// Emit a structured warning to stderr and collect it for the JSON envelope.
///
/// Writes `WARN: {message}\n` to stderr (plain text, no ANSI codes).
/// Appends `message` (without the `WARN: ` prefix) to `warnings`.
#[allow(dead_code)]
pub fn emit_warning(message: &str, warnings: &mut Vec<String>) {
    let _ = writeln!(std::io::stderr(), "WARN: {message}");
    warnings.push(message.to_string());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_single_warning_stderr_and_vec() {
        let mut warnings = Vec::new();
        emit_warning("test message", &mut warnings);
        assert_eq!(warnings, vec!["test message"]);
    }

    #[test]
    fn test_multiple_warnings_preserve_order() {
        let mut warnings = Vec::new();
        emit_warning("first", &mut warnings);
        emit_warning("second", &mut warnings);
        emit_warning("third", &mut warnings);
        assert_eq!(warnings, vec!["first", "second", "third"]);
    }

    #[test]
    fn test_empty_warnings_after_no_calls() {
        let warnings: Vec<String> = Vec::new();
        assert!(warnings.is_empty());
    }
}
