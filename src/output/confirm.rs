use std::io::{BufRead, BufReader, Write};

use crate::error::{MfError, Result};

pub enum ConfirmOutcome {
    Confirmed,
    Aborted,
    NotTty,
}

pub struct ConfirmArgs<'a> {
    pub verb_label: &'a str,
    pub kind: &'a str,
    pub identity: &'a str,
    pub yes: bool,
    pub force: bool,
}

/// Run the destructive-verb confirmation protocol and map the outcome to a
/// `Result`. `Ok(())` means proceed. The prompt and the abort message both use
/// `args.verb_label` (e.g. "removal", "archiving").
pub fn require_confirmation(args: &ConfirmArgs) -> Result<()> {
    match maybe_confirm(args) {
        ConfirmOutcome::Confirmed => Ok(()),
        ConfirmOutcome::Aborted => Err(MfError::usage(format!("{} aborted", args.verb_label), None)),
        ConfirmOutcome::NotTty => Err(MfError::usage(
            format!("destructive operation requires confirmation for '{}'", args.identity),
            Some("pass --yes to confirm".to_string()),
        )),
    }
}

fn maybe_confirm(args: &ConfirmArgs) -> ConfirmOutcome {
    if args.yes || args.force {
        return ConfirmOutcome::Confirmed;
    }
    let prompt = format!("Confirm {} of {} \"{}\"? [y/N] ", args.verb_label, args.kind, args.identity);
    prompt_confirmation(&prompt)
}

/// Show `prompt` on stderr, read a y/N answer from `/dev/tty`.
///
/// `Confirmed` only on "y"/"yes" (case-insensitive). Empty input, "n", EOF, and
/// IO errors all map to `Aborted`. `NotTty` when `/dev/tty` cannot be opened.
pub fn prompt_confirmation(prompt: &str) -> ConfirmOutcome {
    let tty_file = match std::fs::File::open("/dev/tty") {
        Ok(f) => f,
        Err(_) => return ConfirmOutcome::NotTty,
    };

    let stderr = std::io::stderr();
    let mut stderr_handle = stderr.lock();
    let _ = write!(stderr_handle, "{}", prompt);
    let _ = stderr_handle.flush();
    drop(stderr_handle);

    let mut reader = BufReader::new(&tty_file);
    let mut answer = String::new();
    match reader.read_line(&mut answer) {
        Ok(0) => ConfirmOutcome::Aborted,
        Ok(_) => match answer.trim().to_lowercase().as_str() {
            "y" | "yes" => ConfirmOutcome::Confirmed,
            _ => ConfirmOutcome::Aborted,
        },
        Err(_) => ConfirmOutcome::Aborted,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn yes_flag_skips_prompt() {
        let args = ConfirmArgs { verb_label: "removal", kind: "project", identity: "demo", yes: true, force: false };
        assert!(matches!(maybe_confirm(&args), ConfirmOutcome::Confirmed));
    }

    #[test]
    fn force_flag_skips_prompt() {
        let args = ConfirmArgs { verb_label: "removal", kind: "project", identity: "demo", yes: false, force: true };
        assert!(matches!(maybe_confirm(&args), ConfirmOutcome::Confirmed));
    }

    /// Without `--yes`/`--force` and with no openable `/dev/tty`, the prompt is
    /// skipped and the caller gets `NotTty`. The real interactive path is
    /// exercised by integration tests and manual verification.
    #[test]
    fn no_tty_returns_not_tty() {
        let args = ConfirmArgs { verb_label: "removal", kind: "project", identity: "demo", yes: false, force: false };
        let outcome = maybe_confirm(&args);
        assert!(matches!(outcome, ConfirmOutcome::NotTty));
    }
}
