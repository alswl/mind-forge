use std::io::IsTerminal;

/// Single source of truth for terminal capability detection.
///
/// `MF_FORCE_TTY` forces TTY-on so golden tests can capture aligned/colored
/// output through a pipe. `NO_COLOR` (https://no-color.org) disables ANSI.
pub struct TtyProbe {
    pub stdout_is_tty: bool,
    pub color_enabled: bool,
    pub width: usize,
}

pub fn probe() -> TtyProbe {
    let force_tty = std::env::var("MF_FORCE_TTY").map(|v| !v.is_empty()).unwrap_or(false);
    let stdout_is_tty = force_tty || std::io::stdout().is_terminal();
    let no_color = std::env::var("NO_COLOR").map(|v| !v.is_empty()).unwrap_or(false);
    let width = terminal_size::terminal_size().map(|(w, _)| w.0 as usize).unwrap_or(200);
    TtyProbe { stdout_is_tty, color_enabled: stdout_is_tty && !no_color, width }
}
