use assert_cmd::Command;
use std::path::PathBuf;

mod common;

fn run(args: &[&str]) -> (String, String, i32) {
    let output = Command::cargo_bin("mf")
        .expect("binary exists")
        .env_remove("MF_FORCE_TTY")
        .env_remove("NO_COLOR")
        .args(args)
        .output()
        .expect("command runs");
    (
        String::from_utf8(output.stdout).expect("stdout utf8"),
        String::from_utf8(output.stderr).expect("stderr utf8"),
        output.status.code().unwrap_or_default(),
    )
}

fn run_with_env(args: &[&str], env_vars: &[(&str, &str)]) -> (String, String, i32) {
    let mut cmd = Command::cargo_bin("mf").expect("binary exists");
    cmd.env_remove("MF_FORCE_TTY").env_remove("NO_COLOR");
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

    // Use separate write_index calls for different entity types to match the
    // expected per-file schemas, or write a combined index. We use the common
    // helpers where possible.
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

    // Write actual article files
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

fn has_ansi(s: &str) -> bool {
    s.contains("\x1b[")
}

// =============================================================================
// T097: List commands — pipe vs TTY
// =============================================================================

#[test]
fn list_pipe_mode_no_headers() {
    let (_repo, root) = setup();

    let (stdout, stderr, code) = run(&["--root", &root, "article", "list", "--project", "alpha"]);
    assert_eq!(code, 0, "command failed, stderr: {stderr:?}, stdout: {stdout:?}");
    // Pipe mode: no headers, data lines start with identity
    assert!(!stdout.starts_with("PATH"), "pipe mode should not have headers, got: {stdout:?}");
    assert!(!has_ansi(&stdout), "pipe mode should not have ANSI");
    assert!(stdout.contains("docs/getting-started.md"), "should contain getting-started, got: {stdout:?}");
    assert!(stdout.contains("docs/reference.md"), "should contain reference, got: {stdout:?}");
}

#[test]
fn list_tty_mode_has_headers() {
    let (_repo, root) = setup();

    let (stdout, stderr, code) =
        run_with_env(&["--root", &root, "article", "list", "--project", "alpha"], &[("MF_FORCE_TTY", "1")]);
    assert_eq!(code, 0, "command failed, stderr: {stderr:?}, stdout: {stdout:?}");
    // TTY mode: headers present (article list uses PATH, CONTENT, STATUS)
    assert!(stdout.starts_with("PATH"), "TTY mode should have headers, got: {stdout:?}");
    assert!(stdout.contains("docs/getting-started.md"), "should contain getting-started: {stdout:?}");
}

#[test]
fn list_tty_mode_has_ansi_coloring() {
    let (_repo, root) = setup();

    let (stdout, stderr, code) =
        run_with_env(&["--root", &root, "article", "list", "--project", "alpha"], &[("MF_FORCE_TTY", "1")]);
    assert_eq!(code, 0, "command failed, stderr: {stderr:?}");
    // Content kind labels should be colored in TTY mode
    assert!(has_ansi(&stdout), "TTY mode should have ANSI coloring, got: {stdout:?}");
}

#[test]
fn list_pipe_mode_data_rows_match_tty_data_rows() {
    let (_repo, root) = setup();

    let (pipe_out, _, _) = run(&["--root", &root, "article", "list", "--project", "alpha"]);
    let (tty_out, _, _) =
        run_with_env(&["--root", &root, "article", "list", "--project", "alpha"], &[("MF_FORCE_TTY", "1")]);

    // Both should contain the same identities
    assert!(pipe_out.contains("docs/getting-started.md"));
    assert!(tty_out.contains("docs/getting-started.md"));
    assert!(pipe_out.contains("docs/reference.md"));
    assert!(tty_out.contains("docs/reference.md"));
}

// =============================================================================
// T097: Show commands — no ANSI in pipe
// =============================================================================

#[test]
fn show_pipe_mode_no_ansi() {
    let (_repo, root) = setup();

    let (stdout, stderr, code) =
        run(&["--root", &root, "article", "show", "docs/getting-started.md", "--project", "alpha"]);
    assert_eq!(code, 0, "command failed, stderr: {stderr:?}");
    assert!(!has_ansi(&stdout), "pipe mode show should not have ANSI: {stdout:?}");
    assert!(stdout.contains("Getting Started"), "should contain title: {stdout:?}");
}

#[test]
fn show_tty_mode_respects_no_color() {
    let (_repo, root) = setup();

    let (stdout, stderr, code) = run_with_env(
        &["--root", &root, "article", "show", "docs/getting-started.md", "--project", "alpha"],
        &[("MF_FORCE_TTY", "1"), ("NO_COLOR", "1")],
    );
    assert_eq!(code, 0, "command failed, stderr: {stderr:?}");
    assert!(!has_ansi(&stdout), "NO_COLOR should disable ANSI even in TTY mode");
}

// =============================================================================
// T098: NO_COLOR honored across commands
// =============================================================================

#[test]
fn no_color_disables_ansi_in_list() {
    let (_repo, root) = setup();

    let (stdout, stderr, code) = run_with_env(
        &["--root", &root, "article", "list", "--project", "alpha"],
        &[("MF_FORCE_TTY", "1"), ("NO_COLOR", "1")],
    );
    assert_eq!(code, 0, "command failed, stderr: {stderr:?}");
    assert!(!has_ansi(&stdout), "NO_COLOR should disable ANSI");
    // Headers should still be present (TTY but no color)
    assert!(stdout.starts_with("PATH"), "headers should still appear with NO_COLOR, got: {stdout:?}");
}

#[test]
fn no_color_disables_ansi_in_term_list() {
    let (_repo, root) = setup();

    let (stdout, stderr, code) = run_with_env(
        &["--root", &root, "term", "list", "--project", "alpha"],
        &[("MF_FORCE_TTY", "1"), ("NO_COLOR", "1")],
    );
    assert_eq!(code, 0, "command failed, stderr: {stderr:?}");
    assert!(!has_ansi(&stdout), "should have no ANSI");
    assert!(stdout.contains("RAG"), "should contain RAG, got: {stdout:?}");
}

#[test]
fn no_color_disables_ansi_in_source_list() {
    let (_repo, root) = setup();

    let (stdout, stderr, code) = run_with_env(
        &["--root", &root, "source", "list", "--project", "alpha"],
        &[("MF_FORCE_TTY", "1"), ("NO_COLOR", "1")],
    );
    assert_eq!(code, 0, "command failed, stderr: {stderr:?}");
    assert!(!has_ansi(&stdout), "should have no ANSI");
    assert!(stdout.contains("research-paper"), "should contain research-paper, got: {stdout:?}");
}

#[test]
fn no_color_disables_ansi_in_asset_list() {
    let (_repo, root) = setup();

    let (stdout, stderr, code) = run_with_env(
        &["--root", &root, "asset", "list", "--project", "alpha"],
        &[("MF_FORCE_TTY", "1"), ("NO_COLOR", "1")],
    );
    assert_eq!(code, 0, "command failed, stderr: {stderr:?}");
    assert!(!has_ansi(&stdout), "should have no ANSI");
    assert!(stdout.contains("banner.jpg"), "should contain banner.jpg, got: {stdout:?}");
}

// =============================================================================
// T099: --no-headers and --no-trunc flags
// =============================================================================

#[test]
fn no_headers_flag_suppresses_headers_in_tty() {
    let (_repo, root) = setup();

    let (stdout, stderr, code) = run_with_env(
        &["--root", &root, "article", "list", "--project", "alpha", "--no-headers"],
        &[("MF_FORCE_TTY", "1")],
    );
    assert_eq!(code, 0, "command failed, stderr: {stderr:?}");
    assert!(!stdout.starts_with("PATH"), "--no-headers should suppress headers even in TTY: {stdout:?}");
    assert!(stdout.contains("docs/getting-started.md"), "should still contain identities: {stdout:?}");
}

#[test]
fn no_headers_flag_works_in_pipe_mode_too() {
    let (_repo, root) = setup();

    // Both should produce same output (headers already suppressed in pipe)
    let (stdout1, _, _) = run(&["--root", &root, "article", "list", "--project", "alpha"]);
    let (stdout2, _, _) = run(&["--root", &root, "article", "list", "--project", "alpha", "--no-headers"]);
    assert_eq!(stdout1, stdout2, "--no-headers in pipe mode should be idempotent");
}

#[test]
fn no_trunc_flag_allows_full_output() {
    let (_repo, root) = setup();

    // Should not truncate even in pipe mode
    let (stdout, stderr, code) = run(&["--root", &root, "article", "list", "--project", "alpha", "--no-trunc"]);
    assert_eq!(code, 0, "command failed, stderr: {stderr:?}");
    assert!(stdout.contains("docs/getting-started.md"), "should contain identity: {stdout:?}");
    assert!(stdout.contains("docs/reference.md"), "should contain identity: {stdout:?}");
}

#[test]
fn mf_force_tty_only_affects_formatting_not_functionality() {
    let (_repo, root) = setup();

    let (pipe_out, _, pipe_code) = run(&["--root", &root, "source", "list", "--project", "alpha"]);
    let (tty_out, _, tty_code) =
        run_with_env(&["--root", &root, "source", "list", "--project", "alpha"], &[("MF_FORCE_TTY", "1")]);

    assert_eq!(pipe_code, tty_code, "exit codes should match regardless of TTY");
    // Both should report the same identity (source list has NAME header)
    assert!(pipe_out.contains("research-paper"), "pipe output should contain research-paper: {pipe_out:?}");
    assert!(tty_out.contains("research-paper"), "tty output should contain research-paper: {tty_out:?}");
}
