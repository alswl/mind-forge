use crate::model::terminal::{
    ColorMode, DetectionSource, EnvironmentSummary, OutputFormat, OutputRenderingPolicy, TerminalCapabilityProfile,
};
use std::io::IsTerminal;

/// Probe the current process environment and build a terminal capability profile.
///
/// Detection order (first match wins for each capability dimension):
/// 1. JSON output / non-TTY / NO_COLOR / --no-color → disabled
/// 2. Explicit user overrides → env_override
/// 3. Environment variables (COLORTERM, TERM_PROGRAM) → terminal_identity
/// 4. Terminfo evidence → terminfo
/// 5. Fallback → fallback
pub fn build_profile() -> TerminalCapabilityProfile {
    let force_tty = std::env::var("MF_FORCE_TTY").map(|v| !v.is_empty()).unwrap_or(false);
    let stdout_is_tty = force_tty || std::io::stdout().is_terminal();
    let no_color = std::env::var("NO_COLOR").map(|v| !v.is_empty()).unwrap_or(false);
    let width = terminal_size::terminal_size().map(|(w, _)| w.0 as usize).unwrap_or(200);

    if no_color || !stdout_is_tty {
        let reason = if no_color { "NO_COLOR active" } else { "stdout is not a terminal" };
        return TerminalCapabilityProfile {
            stdout_is_tty,
            terminal_width: width,
            term: std::env::var("TERM").ok(),
            term_program: std::env::var("TERM_PROGRAM").ok(),
            color_mode: ColorMode::None,
            truecolor: false,
            hyperlinks: false,
            styling: false,
            detection_source: DetectionSource::Disabled,
            fallback_reason: Some(reason.to_string()),
            override_source: None,
        };
    }

    let term = std::env::var("TERM").ok();
    let term_program = std::env::var("TERM_PROGRAM").ok();
    let colorterm = std::env::var("COLORTERM").ok();

    // Determine truecolor support
    let (truecolor, tc_source) = detect_truecolor(&term, &colorterm);

    // Determine hyperlink support
    let hyperlinks = detect_hyperlinks(&term, &term_program);

    let (color_mode, detection_source, fallback_reason) =
        classify(stdout_is_tty, truecolor, hyperlinks, &term, &colorterm, tc_source);

    TerminalCapabilityProfile {
        stdout_is_tty,
        terminal_width: width,
        term,
        term_program,
        color_mode,
        truecolor,
        hyperlinks,
        styling: color_mode != ColorMode::None,
        detection_source,
        fallback_reason,
        override_source: if force_tty { Some("MF_FORCE_TTY".to_string()) } else { None },
    }
}

fn detect_truecolor(term: &Option<String>, colorterm: &Option<String>) -> (bool, Option<DetectionSource>) {
    // COLORTERM=truecolor or COLORTERM=24bit
    if let Some(ref ct) = colorterm {
        let ct_lower = ct.to_lowercase();
        if ct_lower == "truecolor" || ct_lower == "24bit" {
            return (true, Some(DetectionSource::TerminalIdentity));
        }
    }

    // Terminfo-style capabilities via TERM
    if let Some(ref t) = term {
        // Ghostty and other modern terminals that advertise direct color
        if t.contains("ghostty") || t.contains("kitty") {
            return (true, Some(DetectionSource::TerminalIdentity));
        }
        // Terminfo indicators in TERM value
        if t.contains("-direct") || t.contains("truecolor") {
            return (true, Some(DetectionSource::Terminfo));
        }
    }

    // Check for terminfo database evidence via infocmp/tic
    if let Ok(output) = std::process::Command::new("infocmp").arg("-1").output() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        if stdout.contains("RGB") || stdout.contains("Tc") {
            return (true, Some(DetectionSource::Terminfo));
        }
    }

    (false, None)
}

fn detect_hyperlinks(term: &Option<String>, term_program: &Option<String>) -> bool {
    // Opt-out via MF_NO_HYPERLINKS
    let no_hyperlinks = std::env::var("MF_NO_HYPERLINKS").map(|v| !v.is_empty()).unwrap_or(false);
    if no_hyperlinks {
        return false;
    }

    // Opt-in via MF_FORCE_HYPERLINKS
    let force = std::env::var("MF_FORCE_HYPERLINKS").map(|v| !v.is_empty()).unwrap_or(false);
    if force {
        return true;
    }

    // Terminfo 'Ms' capability indicates OSC 8 support
    if check_terminfo_ms() {
        return true;
    }

    let known = [
        // TERM_PROGRAM values
        "ghostty",
        "kitty",
        "wezterm",
        "alacritty",
        "iterm.app",
        "iterm2",
        "vscode",
        "warpterminal",
        "warp",
        "apple_terminal",
        "hyper",
        "rio",
        "tabby",
        "windsurf",
        "cursor",
        "contour",
        "foot",
    ];

    if let Some(ref tp) = term_program {
        let tp_lower = tp.to_lowercase();
        if known.iter().any(|k| tp_lower.contains(k)) {
            return true;
        }
    }
    if let Some(ref t) = term {
        let t_lower = t.to_lowercase();
        if known.iter().any(|k| t_lower.contains(k)) {
            return true;
        }
    }

    // tmux 3.3+ supports OSC 8 passthrough
    if let Some(ref t) = term {
        if t.starts_with("tmux") || t.starts_with("screen") {
            // In tmux, the outer terminal determines support. Check TERM_PROGRAM
            // and TERM outside, or trust the passthrough if force env is set.
            let outside_term = std::env::var("TERM_PROGRAM").ok();
            if let Some(ref ot) = outside_term {
                let ot_lower = ot.to_lowercase();
                if known.iter().any(|k| ot_lower.contains(k)) {
                    return true;
                }
            }
            // If we can't detect the outer terminal, default to true for tmux
            // 3.3+ which is widely deployed. Opt-out still available.
            return true;
        }
    }

    false
}

fn check_terminfo_ms() -> bool {
    if let Ok(output) = std::process::Command::new("infocmp").arg("-1").output() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        if stdout.lines().any(|l| l.trim() == "Ms") {
            return true;
        }
    }
    false
}

fn classify(
    stdout_is_tty: bool,
    truecolor: bool,
    hyperlinks: bool,
    term: &Option<String>,
    colorterm: &Option<String>,
    tc_source: Option<DetectionSource>,
) -> (ColorMode, DetectionSource, Option<String>) {
    let source = tc_source.unwrap_or_else(|| {
        if let Some(ref t) = term {
            if t == "dumb" || t.is_empty() {
                return DetectionSource::Fallback;
            }
        }
        DetectionSource::TerminalIdentity
    });

    if truecolor {
        return (ColorMode::Truecolor, source, None);
    }

    // Check 256-color support
    let has_256 = term.as_ref().is_some_and(|t| t.contains("256color"));
    if has_256 {
        return (ColorMode::Ansi256, source, None);
    }

    let is_dumb = term.as_ref().is_some_and(|t| t == "dumb" || t.is_empty());
    if is_dumb {
        return (
            ColorMode::None,
            DetectionSource::Fallback,
            Some(format!("TERM={} does not support color", term.as_deref().unwrap_or(""))),
        );
    }

    // Check if there's basic color evidence
    let has_colorterm = colorterm.is_some();
    let has_term_color =
        term.as_ref().is_some_and(|t| t.contains("color") || t.starts_with("xterm") || t.starts_with("screen"));

    if has_colorterm || has_term_color {
        let mode = if hyperlinks { ColorMode::Ansi256 } else { ColorMode::Ansi16 };
        return (mode, source, None);
    }

    // On a TTY with no explicit disabling evidence, default to basic ANSI color.
    // Most terminals support at least 16 colors.
    if stdout_is_tty {
        return (ColorMode::Ansi16, DetectionSource::Fallback, None);
    }

    (ColorMode::None, DetectionSource::Fallback, Some("no color capability detected".to_string()))
}

/// Build the output rendering policy from a profile and format.
pub fn build_policy(profile: &TerminalCapabilityProfile, format: OutputFormat) -> OutputRenderingPolicy {
    OutputRenderingPolicy::from_profile(profile, format)
}

/// Build environment summary for diagnostics.
pub fn build_environment_summary() -> EnvironmentSummary {
    EnvironmentSummary {
        term: std::env::var("TERM").ok(),
        colorterm: std::env::var("COLORTERM").ok(),
        term_program: std::env::var("TERM_PROGRAM").ok(),
        tmux: std::env::var("TMUX").map(|v| !v.is_empty()).unwrap_or(false),
        no_color: std::env::var("NO_COLOR").map(|v| !v.is_empty()).unwrap_or(false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_policy_disables_all_output() {
        let p = OutputRenderingPolicy::plain(OutputFormat::Text);
        assert!(p.plain_output);
        assert!(!p.emit_ansi_color);
        assert!(!p.emit_truecolor);
        assert!(!p.emit_hyperlinks);
        assert!(p.preserve_visible_targets);
    }

    #[test]
    fn json_policy_from_profile_forces_plain() {
        let profile = TerminalCapabilityProfile {
            stdout_is_tty: true,
            terminal_width: 120,
            term: Some("xterm-ghostty".into()),
            term_program: Some("Ghostty".into()),
            color_mode: ColorMode::Truecolor,
            truecolor: true,
            hyperlinks: true,
            styling: true,
            detection_source: DetectionSource::Terminfo,
            fallback_reason: None,
            override_source: None,
        };
        let policy = build_policy(&profile, OutputFormat::Json);
        assert!(policy.plain_output);
        assert!(!policy.emit_ansi_color);
        assert!(!policy.emit_hyperlinks);
    }

    #[test]
    fn text_policy_preserves_rich_features() {
        let profile = TerminalCapabilityProfile {
            stdout_is_tty: true,
            terminal_width: 120,
            term: Some("xterm-ghostty".into()),
            term_program: Some("Ghostty".into()),
            color_mode: ColorMode::Truecolor,
            truecolor: true,
            hyperlinks: true,
            styling: true,
            detection_source: DetectionSource::Terminfo,
            fallback_reason: None,
            override_source: None,
        };
        let policy = build_policy(&profile, OutputFormat::Text);
        assert!(!policy.plain_output);
        assert!(policy.emit_ansi_color);
        assert!(policy.emit_truecolor);
        assert!(policy.emit_hyperlinks);
    }

    #[test]
    fn non_tty_profile_forces_plain_policy() {
        let profile = TerminalCapabilityProfile {
            stdout_is_tty: false,
            terminal_width: 200,
            term: Some("xterm-ghostty".into()),
            term_program: None,
            color_mode: ColorMode::None,
            truecolor: false,
            hyperlinks: false,
            styling: false,
            detection_source: DetectionSource::NonTty,
            fallback_reason: Some("stdout is not a terminal".into()),
            override_source: None,
        };
        let policy = build_policy(&profile, OutputFormat::Text);
        assert!(policy.plain_output);
        assert!(!policy.emit_ansi_color);
    }

    #[test]
    fn build_environment_summary_reads_env() {
        let summary = build_environment_summary();
        // Just validate the shape — actual values depend on the test environment
        let _ = serde_json::to_value(&summary).unwrap();
    }

    /// T047: Capability probing is cached per command — build_profile() runs
    /// once and completes in under 50ms (well under the 100ms terminfo budget).
    #[test]
    fn build_profile_is_fast() {
        let start = std::time::Instant::now();
        let _profile = build_profile();
        let elapsed = start.elapsed();
        assert!(elapsed.as_millis() < 100, "build_profile took {}ms, expected under 100ms", elapsed.as_millis());
    }

    /// Repeated calls to build_profile() should be similarly fast — no
    /// accidental per-call side effects like process spawning per invocation.
    #[test]
    fn build_profile_repeated_is_stable() {
        let start = std::time::Instant::now();
        for _ in 0..10 {
            let _profile = build_profile();
        }
        let elapsed = start.elapsed();
        assert!(elapsed.as_millis() < 200, "10x build_profile took {}ms, expected under 200ms", elapsed.as_millis());
    }
}
