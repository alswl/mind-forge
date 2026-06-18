use assert_cmd::Command;
use std::path::PathBuf;

mod common;

/// Run `mf` with controlled terminal-related environment variables.
///
/// Always removes `MF_FORCE_TTY` and `NO_COLOR` from the inherited env first,
/// then applies the given overrides.
fn run_with_term_env(args: &[&str], env_vars: &[(&str, &str)]) -> (String, String, i32) {
    let mut cmd = Command::cargo_bin("mf").expect("binary exists");
    cmd.env_remove("MF_FORCE_TTY");
    cmd.env_remove("NO_COLOR");
    cmd.env_remove("TERM");
    cmd.env_remove("COLORTERM");
    cmd.env_remove("TERM_PROGRAM");
    for (k, v) in env_vars {
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

fn seed_project_with_entities(repo: &common::TempDir, name: &str) -> PathBuf {
    common::create_project(repo, name);
    let dir = repo.path().join(name);
    let src_dir = dir.join("sources");
    let asset_dir = dir.join("assets");
    let docs_dir = dir.join("docs");
    std::fs::create_dir_all(&src_dir).unwrap();
    std::fs::create_dir_all(&asset_dir).unwrap();
    std::fs::create_dir_all(&docs_dir).unwrap();

    let yaml = format!(
        r#"schema_version: '1'
sources:
  - name: research-paper
    type: pdf
    path: sources/pdf/paper.pdf
    tags: []
    added_at: '2026-05-08T10:00:00Z'
    updated_at: '2026-05-08T10:00:00Z'
assets:
  - name: banner.jpg
    type: image
    path: assets/banner.jpg
    size: 24576
    hash: aabb1122
    tags: []
    added_at: '2026-05-07T18:00:00Z'
  - name: intro.mp4
    type: video
    path: assets/intro.mp4
    size: 1048576
    hash: eeff5566
    tags: [demo]
    added_at: '2026-05-07T20:00:00Z'
terms:
  - term: RAG
    definition: Retrieval-Augmented Generation
    aliases: []
    tags: []
    corrections: []
articles:
  - title: "Getting Started Guide"
    project: '{name}'
    article_type: blog
    article_path: docs/getting-started.md
    status: draft
    created_at: '2026-05-07T12:00:00Z'
    updated_at: '2026-05-07T12:00:00Z'
  - title: "API Reference"
    project: '{name}'
    article_type: blog
    article_path: docs/reference.md
    status: draft
    created_at: '2026-05-08T18:00:00Z'
    updated_at: '2026-05-08T18:00:00Z'
publish:
  records: []
"#,
        name = name
    );
    common::write_index(repo, name, &yaml);

    std::fs::write(docs_dir.join("getting-started.md"), "# Getting Started\n\nWelcome.\n").unwrap();
    std::fs::write(docs_dir.join("reference.md"), "# API Reference\n\nDetails.\n").unwrap();

    dir
}

fn setup() -> (common::TempDir, String) {
    let repo = common::setup_repo();
    seed_project_with_entities(&repo, "alpha");
    let root = repo.path().to_string_lossy().to_string();
    (repo, root)
}

// =============================================================================
// US1: Modern terminal capabilities are recognized
// =============================================================================

/// T013: Ghostty-like truecolor detection via `mf config terminal` text output
#[test]
fn us1_ghostty_truecolor_detection_text() {
    let (stdout, stderr, code) = run_with_term_env(
        &["config", "terminal"],
        &[("MF_FORCE_TTY", "1"), ("TERM", "xterm-ghostty"), ("COLORTERM", "truecolor"), ("TERM_PROGRAM", "Ghostty")],
    );
    assert_eq!(code, 0, "stderr: {stderr:?}");
    assert!(stdout.contains("Color: truecolor"), "expected truecolor in text output, got: {stdout}");
    assert!(stdout.contains("Hyperlinks: yes"), "expected hyperlinks=yes in text output, got: {stdout}");
    assert!(stdout.contains("TTY: yes"), "expected TTY=yes in text output, got: {stdout}");
}

/// T014: Ghostty-like diagnostic JSON envelope fields
#[test]
fn us1_ghostty_diagnostic_json() {
    let (stdout, stderr, code) = run_with_term_env(
        &["--json", "config", "terminal"],
        &[("MF_FORCE_TTY", "1"), ("TERM", "xterm-ghostty"), ("COLORTERM", "truecolor"), ("TERM_PROGRAM", "Ghostty")],
    );
    assert_eq!(code, 0, "stderr: {stderr:?}");
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    assert_eq!(v["status"], "ok");
    let data = &v["data"];
    assert_eq!(data["profile"]["color_mode"], "truecolor");
    assert_eq!(data["profile"]["truecolor"], true);
    assert_eq!(data["profile"]["hyperlinks"], true);
    assert_eq!(data["profile"]["term"], "xterm-ghostty");
    assert_eq!(data["profile"]["term_program"], "Ghostty");
    assert!(data["profile"]["terminal_width"].as_u64().unwrap_or(0) > 0);
}

/// T024: xterm-256color fallback text output
#[test]
fn us2_xterm256_fallback_text() {
    let (stdout, stderr, code) =
        run_with_term_env(&["config", "terminal"], &[("MF_FORCE_TTY", "1"), ("TERM", "xterm-256color")]);
    assert_eq!(code, 0, "stderr: {stderr:?}");
    assert!(stdout.contains("Color:"), "expected color field, got: {stdout}");
    // 256-color should not report truecolor
    assert!(!stdout.contains("Color: truecolor"), "256-color terminal should not report truecolor: {stdout}");
}

/// T024: TERM=dumb fallback behavior
#[test]
fn us2_term_dumb_fallback() {
    let (stdout, stderr, code) = run_with_term_env(&["config", "terminal"], &[("MF_FORCE_TTY", "1"), ("TERM", "dumb")]);
    assert_eq!(code, 0, "stderr: {stderr:?}");
    assert!(stdout.contains("Color: none"), "dumb terminal should report no color, got: {stdout}");
    assert!(stdout.contains("Hyperlinks: no"), "dumb terminal should report no hyperlinks, got: {stdout}");
}

/// T025: NO_COLOR disables terminal escapes in list output
#[test]
fn us2_no_color_disables_rich_output() {
    let (_repo, root) = setup();
    let (stdout, stderr, code) = run_with_term_env(
        &["--root", &root, "article", "list", "--project", "alpha"],
        &[("MF_FORCE_TTY", "1"), ("NO_COLOR", "1"), ("TERM", "xterm-ghostty"), ("COLORTERM", "truecolor")],
    );
    assert_eq!(code, 0, "stderr: {stderr:?}");
    assert!(!stdout.contains("\x1b["), "NO_COLOR should suppress ANSI escapes, got: {stdout:?}");
    assert!(stdout.contains("docs/getting-started.md"), "should still contain data, got: {stdout:?}");
}

/// T026: JSON output contains no ANSI or OSC 8 escapes
#[test]
fn us2_json_no_terminal_escapes() {
    let (stdout, stderr, code) = run_with_term_env(
        &["--json", "config", "terminal"],
        &[("MF_FORCE_TTY", "1"), ("TERM", "xterm-ghostty"), ("COLORTERM", "truecolor"), ("TERM_PROGRAM", "Ghostty")],
    );
    assert_eq!(code, 0, "stderr: {stderr:?}");
    assert!(!stdout.contains("\x1b["), "JSON output must not contain ANSI escapes");
    assert!(!stdout.contains("\x1b]8;"), "JSON output must not contain OSC 8 escapes");
    // Must still parse as valid JSON
    let _: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
}

/// T027: mf config terminal is read-only (no filesystem side effects)
#[test]
fn us3_config_terminal_read_only() {
    let repo = common::setup_repo();
    common::create_project(&repo, "test-project");
    let root = repo.path().to_string_lossy().to_string();

    // Record mtimes before
    let before = collect_mtimes(repo.path());

    let (stdout, stderr, code) = run_with_term_env(&["--root", &root, "config", "terminal"], &[("MF_FORCE_TTY", "1")]);
    assert_eq!(code, 0, "stderr: {stderr:?} stdout: {stdout}");

    // Record mtimes after
    let after = collect_mtimes(repo.path());

    assert_eq!(before, after, "config terminal must not modify any files");
}

fn collect_mtimes(root: &std::path::Path) -> Vec<(String, u64)> {
    let mut mtimes = Vec::new();
    for entry in walkdir::WalkDir::new(root).sort_by_file_name() {
        let entry = entry.unwrap();
        if entry.file_type().is_file() {
            let mtime = entry.metadata().unwrap().modified().unwrap();
            let secs = mtime.duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
            mtimes.push((entry.path().to_string_lossy().to_string(), secs));
        }
    }
    mtimes
}

/// T034: mf config terminal text summary fields
#[test]
fn us3_config_terminal_text_summary() {
    let (stdout, stderr, code) =
        run_with_term_env(&["config", "terminal"], &[("MF_FORCE_TTY", "1"), ("TERM", "xterm-256color")]);
    assert_eq!(code, 0, "stderr: {stderr:?}");
    assert!(stdout.contains("Terminal:"), "should show terminal identity");
    assert!(stdout.contains("TTY:"), "should show TTY status");
    assert!(stdout.contains("Color:"), "should show color mode");
    assert!(stdout.contains("Hyperlinks:"), "should show hyperlink status");
    assert!(stdout.contains("Fallback:"), "should show fallback reason");
}

/// T035: mf --json config terminal contract fields
#[test]
fn us3_config_terminal_json_contract() {
    let (stdout, stderr, code) =
        run_with_term_env(&["--json", "config", "terminal"], &[("MF_FORCE_TTY", "1"), ("TERM", "xterm-256color")]);
    assert_eq!(code, 0, "stderr: {stderr:?}");
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    let data = &v["data"];

    // Envelope shape
    assert_eq!(v["status"], "ok");
    assert_eq!(v["command"], "mf");

    // Profile fields
    let profile = &data["profile"];
    assert!(profile["stdout_is_tty"].as_bool().is_some());
    assert!(profile["terminal_width"].as_u64().is_some());
    assert!(profile["color_mode"].as_str().is_some());
    assert!(profile["truecolor"].as_bool().is_some());
    assert!(profile["hyperlinks"].as_bool().is_some());
    assert!(profile["styling"].as_bool().is_some());
    assert!(profile["detection_source"].as_str().is_some());

    // Policy fields
    let policy = &data["policy"];
    assert!(policy["format"].as_str().is_some());
    assert!(policy["plain_output"].as_bool().is_some());
    assert!(policy["emit_ansi_color"].as_bool().is_some());
    assert!(policy["emit_truecolor"].as_bool().is_some());
    assert!(policy["emit_hyperlinks"].as_bool().is_some());

    // Environment fields
    let env = &data["environment"];
    assert!(env["no_color"].as_bool().is_some());

    // Checks and recommendations
    assert!(data["checks"].is_array());
    assert!(data["recommendations"].is_array());
}

/// T036: mf config terminal works outside a Mind Repo
#[test]
fn us3_config_terminal_no_repo() {
    let (stdout, stderr, code) =
        run_with_term_env(&["config", "terminal"], &[("MF_FORCE_TTY", "1"), ("TERM", "xterm-256color")]);
    assert_eq!(code, 0, "stderr: {stderr:?}");
    assert!(stdout.contains("Terminal:"), "should work outside a repo, got: {stdout}");
    // JSON path also
    let (stdout2, _, code2) =
        run_with_term_env(&["--json", "config", "terminal"], &[("MF_FORCE_TTY", "1"), ("TERM", "xterm-256color")]);
    assert_eq!(code2, 0, "JSON form should also work outside repo");
    let v: serde_json::Value = serde_json::from_str(&stdout2).expect("valid JSON");
    assert_eq!(v["status"], "ok");
}
