use std::process::Command as StdCommand;

use crate::helpers::*;
use serde_json::Value;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn run_with_term_env(dir: impl AsRef<std::path::Path>, args: &[&str], envs: &[(&str, &str)]) -> (String, String, i32) {
    let mut cmd = mf_in(dir);
    // Clear inherited terminal env so tests are deterministic
    cmd.env_remove("MF_FORCE_TTY");
    cmd.env_remove("NO_COLOR");
    cmd.env_remove("TERM");
    cmd.env_remove("COLORTERM");
    cmd.env_remove("TERM_PROGRAM");
    for (k, v) in envs {
        cmd.env(k, v);
    }
    cmd.args(args);
    let output = cmd.output().expect("command runs");
    (
        String::from_utf8(output.stdout).expect("stdout utf8"),
        String::from_utf8(output.stderr).expect("stderr utf8"),
        output.status.code().unwrap_or_default(),
    )
}

fn has_ansi(s: &str) -> bool {
    s.contains("\x1b[")
}

fn has_osc8(s: &str) -> bool {
    s.contains("\x1b]8;")
}

/// Check if infocmp is available in this environment.
fn has_infocmp() -> bool {
    StdCommand::new("infocmp").arg("-V").output().map(|o| o.status.success()).unwrap_or(false)
}

// ---------------------------------------------------------------------------
// Text output: modern terminal (US1)
// ---------------------------------------------------------------------------

#[test]
fn e2e_ghostty_text_truecolor_and_hyperlinks() {
    let (_stdout, stderr, code) = run_with_term_env(
        TempDir::new().unwrap().path(),
        &["config", "terminal"],
        &[("MF_FORCE_TTY", "1"), ("TERM", "xterm-ghostty"), ("COLORTERM", "truecolor"), ("TERM_PROGRAM", "Ghostty")],
    );
    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(_stdout.contains("Color: truecolor"), "expected truecolor, got: {_stdout}");
    assert!(_stdout.contains("Hyperlinks: yes"), "expected hyperlinks=yes, got: {_stdout}");
    assert!(_stdout.contains("TTY: yes"), "expected TTY=yes, got: {_stdout}");
    assert!(_stdout.contains("Terminal: xterm-ghostty"), "expected terminal identity, got: {_stdout}");
}

#[test]
fn e2e_kitty_term_detected_as_truecolor() {
    let (_stdout, stderr, code) = run_with_term_env(
        TempDir::new().unwrap().path(),
        &["config", "terminal"],
        &[("MF_FORCE_TTY", "1"), ("TERM", "xterm-kitty"), ("TERM_PROGRAM", "kitty")],
    );
    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(_stdout.contains("Color: truecolor"), "kitty should get truecolor via term identity, got: {_stdout}");
}

// ---------------------------------------------------------------------------
// Text output: fallback (US2)
// ---------------------------------------------------------------------------

#[test]
fn e2e_xterm_256color_falls_back_to_ansi256() {
    let (_stdout, stderr, code) = run_with_term_env(
        TempDir::new().unwrap().path(),
        &["config", "terminal"],
        &[("MF_FORCE_TTY", "1"), ("TERM", "xterm-256color")],
    );
    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(!_stdout.contains("Color: truecolor"), "256-color should not be truecolor, got: {_stdout}");
    assert!(_stdout.contains("Hyperlinks: no"), "should not have hyperlinks, got: {_stdout}");
}

#[test]
fn e2e_term_dumb_disables_everything() {
    let (_stdout, stderr, code) = run_with_term_env(
        TempDir::new().unwrap().path(),
        &["config", "terminal"],
        &[("MF_FORCE_TTY", "1"), ("TERM", "dumb")],
    );
    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(_stdout.contains("Color: none"), "dumb terminal should have no color, got: {_stdout}");
    assert!(_stdout.contains("Hyperlinks: no"), "dumb terminal should have no hyperlinks, got: {_stdout}");
}

#[test]
fn e2e_no_color_env_disables_everything() {
    let (_stdout, stderr, code) = run_with_term_env(
        TempDir::new().unwrap().path(),
        &["config", "terminal"],
        &[("MF_FORCE_TTY", "1"), ("NO_COLOR", "1"), ("TERM", "xterm-ghostty"), ("COLORTERM", "truecolor")],
    );
    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(_stdout.contains("Color: none"), "NO_COLOR must disable color, got: {_stdout}");
    assert!(_stdout.contains("Hyperlinks: no"), "NO_COLOR must disable hyperlinks, got: {_stdout}");
    assert!(_stdout.contains("Fallback: NO_COLOR active"), "should show reason, got: {_stdout}");
}

#[test]
fn e2e_pipe_mode_has_no_ansi() {
    let outside = TempDir::new().unwrap();
    // No MF_FORCE_TTY → stdout is NOT a terminal (pipe mode)
    let (_stdout, stderr, code) = run_with_term_env(
        outside.path(),
        &["config", "terminal"],
        &[("TERM", "xterm-ghostty"), ("COLORTERM", "truecolor")],
    );
    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(!has_ansi(&_stdout), "pipe mode must not have ANSI");
    assert!(!has_osc8(&_stdout), "pipe mode must not have OSC 8");
    assert!(_stdout.contains("TTY: no"), "should report TTY=no");
}

// ---------------------------------------------------------------------------
// JSON output (US3)
// ---------------------------------------------------------------------------

#[test]
fn e2e_json_ghostty_diagnostic_envelope() {
    let (_stdout, stderr, code) = run_with_term_env(
        TempDir::new().unwrap().path(),
        &["--json", "config", "terminal"],
        &[("MF_FORCE_TTY", "1"), ("TERM", "xterm-ghostty"), ("COLORTERM", "truecolor"), ("TERM_PROGRAM", "Ghostty")],
    );
    assert_eq!(code, 0, "stderr: {stderr}");

    let v: Value = serde_json::from_str(&_stdout).expect("valid JSON");
    assert_eq!(v["status"], "ok");
    assert_eq!(v["command"], "mf");

    let data = &v["data"];
    assert_eq!(data["profile"]["color_mode"], "truecolor");
    assert_eq!(data["profile"]["truecolor"], true);
    assert_eq!(data["profile"]["hyperlinks"], true);
    assert_eq!(data["profile"]["stdout_is_tty"], true);
    assert_eq!(data["profile"]["detection_source"], "terminal_identity");

    // Policy must preserve rich features for text
    assert_eq!(data["policy"]["format"], "json");
    assert_eq!(data["policy"]["plain_output"], true);

    // Environment
    assert_eq!(data["environment"]["term"], "xterm-ghostty");
    assert_eq!(data["environment"]["colorterm"], "truecolor");
    assert!(data["checks"].is_array());
    assert!(data["recommendations"].is_array());
}

#[test]
fn e2e_json_no_ansi_or_osc8() {
    let (_stdout, stderr, code) = run_with_term_env(
        TempDir::new().unwrap().path(),
        &["--json", "config", "terminal"],
        &[("MF_FORCE_TTY", "1"), ("TERM", "xterm-ghostty"), ("COLORTERM", "truecolor"), ("TERM_PROGRAM", "Ghostty")],
    );
    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(!has_ansi(&_stdout), "JSON output must not contain ANSI escapes");
    assert!(!has_osc8(&_stdout), "JSON output must not contain OSC 8 escapes");
    // Must be valid JSON
    serde_json::from_str::<Value>(&_stdout).expect("valid JSON");
}

#[test]
fn e2e_json_fallback_fields() {
    let (_stdout, stderr, code) = run_with_term_env(
        TempDir::new().unwrap().path(),
        &["--json", "config", "terminal"],
        &[("MF_FORCE_TTY", "1"), ("NO_COLOR", "1")],
    );
    assert_eq!(code, 0, "stderr: {stderr}");

    let v: Value = serde_json::from_str(&_stdout).expect("valid JSON");
    let data = &v["data"];
    assert_eq!(data["profile"]["color_mode"], "none");
    assert_eq!(data["profile"]["detection_source"], "disabled");
    assert_eq!(data["environment"]["no_color"], true);
}

// ---------------------------------------------------------------------------
// Outside repo (US3: no-repo requirement)
// ---------------------------------------------------------------------------

#[test]
fn e2e_config_terminal_works_outside_repo_text() {
    let outside = TempDir::new().unwrap();
    let (_stdout, stderr, code) = run_with_term_env(
        outside.path(),
        &["config", "terminal"],
        &[("MF_FORCE_TTY", "1"), ("TERM", "xterm-256color")],
    );
    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(_stdout.contains("Terminal:"), "should show terminal identity, got: {_stdout}");
}

#[test]
fn e2e_config_terminal_works_outside_repo_json() {
    let outside = TempDir::new().unwrap();
    let (_stdout, stderr, code) = run_with_term_env(
        outside.path(),
        &["--json", "config", "terminal"],
        &[("MF_FORCE_TTY", "1"), ("TERM", "xterm-256color")],
    );
    assert_eq!(code, 0, "stderr: {stderr}");
    let v: Value = serde_json::from_str(&_stdout).expect("valid JSON");
    assert_eq!(v["status"], "ok");
}

// ---------------------------------------------------------------------------
// Detection precedence: COLORTERM > TERM
// ---------------------------------------------------------------------------

#[test]
fn e2e_detection_precedence_colorterm_over_term() {
    // TERM only provides 256-color but COLORTERM=truecolor takes priority
    let (_stdout, stderr, code) = run_with_term_env(
        TempDir::new().unwrap().path(),
        &["config", "terminal"],
        &[("MF_FORCE_TTY", "1"), ("TERM", "xterm-256color"), ("COLORTERM", "truecolor")],
    );
    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(_stdout.contains("Color: truecolor"), "COLORTERM=truecolor must override TERM, got: {_stdout}");
}

#[test]
fn e2e_detection_precedence_no_color_over_colorterm() {
    // NO_COLOR must win over truecolor evidence
    let (_stdout, stderr, code) = run_with_term_env(
        TempDir::new().unwrap().path(),
        &["config", "terminal"],
        &[
            ("MF_FORCE_TTY", "1"),
            ("NO_COLOR", "1"),
            ("TERM", "xterm-ghostty"),
            ("COLORTERM", "truecolor"),
            ("TERM_PROGRAM", "Ghostty"),
        ],
    );
    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(_stdout.contains("Color: none"), "NO_COLOR wins over everything, got: {_stdout}");
}

// ---------------------------------------------------------------------------
// infocmp terminfo detection (US1: terminfo path)
// ---------------------------------------------------------------------------

#[test]
fn e2e_terminfo_detection_when_available() {
    if !has_infocmp() {
        eprintln!("SKIP: infocmp not available in this environment");
        return;
    }
    // Use a known truecolor-capable term; detection will fall through to
    // infocmp if env vars alone are not conclusive.
    let (_stdout, stderr, code) = run_with_term_env(
        TempDir::new().unwrap().path(),
        &["config", "terminal"],
        &[("MF_FORCE_TTY", "1"), ("TERM", "xterm-direct")],
    );
    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(_stdout.contains("Color: truecolor"), "xterm-direct should be truecolor, got: {_stdout}");
}

// ---------------------------------------------------------------------------
// Styling is independently derived from color_mode
// ---------------------------------------------------------------------------

#[test]
fn e2e_styling_disabled_when_color_is_none() {
    let (_stdout, stderr, code) = run_with_term_env(
        TempDir::new().unwrap().path(),
        &["--json", "config", "terminal"],
        &[("MF_FORCE_TTY", "1"), ("NO_COLOR", "1")],
    );
    assert_eq!(code, 0, "stderr: {stderr}");

    let v: Value = serde_json::from_str(&_stdout).expect("valid JSON");
    let data = &v["data"];
    assert_eq!(data["profile"]["styling"], false, "styling should be disabled when color_mode=none");
}

#[test]
fn e2e_styling_enabled_when_color_is_present() {
    let (_stdout, stderr, code) = run_with_term_env(
        TempDir::new().unwrap().path(),
        &["--json", "config", "terminal"],
        &[("MF_FORCE_TTY", "1"), ("TERM", "xterm-ghostty"), ("COLORTERM", "truecolor")],
    );
    assert_eq!(code, 0, "stderr: {stderr}");

    let v: Value = serde_json::from_str(&_stdout).expect("valid JSON");
    let data = &v["data"];
    assert_eq!(data["profile"]["styling"], true, "styling should be enabled when color is available");
}

// ---------------------------------------------------------------------------
// STDERR cleanliness: diagnostics go to stderr only when appropriate
// ---------------------------------------------------------------------------

#[test]
fn e2e_diagnostics_output_split() {
    // In text mode, diagnostic report goes to stdout, recommendations/warnings to stderr
    let (_stdout, stderr, code) = run_with_term_env(
        TempDir::new().unwrap().path(),
        &["config", "terminal"],
        &[("MF_FORCE_TTY", "1"), ("TERM", "xterm-256color")],
    );
    assert_eq!(code, 0, "stderr should not contain errors: {stderr}");
    assert!(_stdout.contains("Terminal:"), "text report on stdout");
}

// ---------------------------------------------------------------------------
// Idempotency: repeated calls produce deterministic output
// ---------------------------------------------------------------------------

#[test]
fn e2e_deterministic_output() {
    let dir = TempDir::new().unwrap();
    let envs = &[("MF_FORCE_TTY", "1"), ("TERM", "xterm-256color")];

    let (out1, _, code1) = run_with_term_env(dir.path(), &["--json", "config", "terminal"], envs);
    let (out2, _, code2) = run_with_term_env(dir.path(), &["--json", "config", "terminal"], envs);

    assert_eq!(code1, 0);
    assert_eq!(code2, 0);
    assert_eq!(out1, out2, "output must be deterministic");
}
