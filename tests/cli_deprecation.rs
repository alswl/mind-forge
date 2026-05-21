use assert_cmd::Command;

mod common;

/// Helper: run `mf` with `--root` and collect output.
fn run(args: &[&str]) -> (String, String, i32) {
    let output = Command::cargo_bin("mf").expect("binary exists").args(args).output().expect("command runs");
    (
        String::from_utf8(output.stdout).expect("stdout utf8"),
        String::from_utf8(output.stderr).expect("stderr utf8"),
        output.status.code().unwrap_or_default(),
    )
}

// ---------------------------------------------------------------------------
// T014: Subcommand deprecation warning — D3 (positional NAME in source remove)
// ---------------------------------------------------------------------------

#[test]
fn deprecation_subcommand_d3_warning() {
    let dir = common::setup_repo();
    common::create_project(&dir, "alpha");

    // source remove with NAME (not PATH) triggers D3 deprecation
    let (_stdout, stderr, _code) =
        run(&["--root", &dir.path().to_string_lossy(), "source", "remove", "some-name", "--project", "alpha"]);
    assert!(stderr.contains("[deprecated]"), "stderr should contain deprecation marker: {stderr:?}");
    assert!(stderr.contains("positional NAME"), "stderr should mention 'positional NAME': {stderr:?}");
}

// ---------------------------------------------------------------------------
// T015: Flag deprecation warning — D1a (--status in publish update)
// ---------------------------------------------------------------------------

#[test]
fn deprecation_flag_d1a_warning() {
    let dir = common::setup_repo();
    common::create_project(&dir, "alpha");

    // publish update --status triggers D1a deprecation (command will fail after deprecation
    // because article/target don't exist, but deprecation warning is emitted first)
    let (_stdout, stderr, _code) = run(&[
        "--root",
        &dir.path().to_string_lossy(),
        "publish",
        "update",
        "my-article",
        "--target",
        "local",
        "--status",
        "draft",
        "--project",
        "alpha",
    ]);
    assert!(stderr.contains("[deprecated]"), "stderr should contain deprecation marker: {stderr:?}");
    assert!(stderr.contains("--status"), "stderr should mention '--status': {stderr:?}");
    assert!(stderr.contains("--set status"), "stderr should mention '--set status': {stderr:?}");
}

// ---------------------------------------------------------------------------
// T016: Positional deprecation warning — D3 (source remove NAME)
// ---------------------------------------------------------------------------

#[test]
fn deprecation_positional_d3_warning() {
    let dir = common::setup_repo();
    common::create_project(&dir, "alpha");

    let (_stdout, stderr, _code) =
        run(&["--root", &dir.path().to_string_lossy(), "source", "remove", "some-name", "--project", "alpha"]);
    assert!(stderr.contains("[deprecated]"));
    assert!(stderr.contains("NAME"));
}

// ---------------------------------------------------------------------------
// T017: Value deprecation warning — D2a (--type with source-kind value)
// ---------------------------------------------------------------------------

#[test]
fn deprecation_value_d2a_warning() {
    let dir = common::setup_repo();
    common::create_project(&dir, "alpha");

    // source add --type auto triggers deprecation warning (--type is the deprecated flag)
    let (_stdout, stderr, _code) = run(&[
        "--root",
        &dir.path().to_string_lossy(),
        "source",
        "add",
        "--name",
        "test-source",
        "--type",
        "auto",
        "https://example.com/doc",
        "--project",
        "alpha",
    ]);
    assert!(stderr.contains("[deprecated]"), "stderr should contain deprecation marker: {stderr:?}");
    assert!(stderr.contains("--type"), "stderr should mention '--type': {stderr:?}");
}

// ---------------------------------------------------------------------------
// T018: Multiple deprecation warnings — D1a + D1b together
// ---------------------------------------------------------------------------

#[test]
fn deprecation_multiple_warnings() {
    let dir = common::setup_repo();
    common::create_project(&dir, "alpha");

    let (_stdout, stderr, _code) = run(&[
        "--root",
        &dir.path().to_string_lossy(),
        "publish",
        "update",
        "my-article",
        "--target",
        "local",
        "--status",
        "draft",
        "--target-url",
        "https://example.com",
        "--project",
        "alpha",
    ]);
    // Should have two deprecation lines on stderr
    let lines: Vec<&str> = stderr.lines().filter(|l| l.contains("[deprecated]")).collect();
    assert!(lines.len() >= 2, "expected at least 2 deprecation lines, got {}: {stderr:?}", lines.len());
    assert!(stderr.lines().any(|l| l.contains("--status")), "stderr should contain --status deprecation: {stderr:?}");
    assert!(
        stderr.lines().any(|l| l.contains("--target-url")),
        "stderr should contain --target-url deprecation: {stderr:?}"
    );
}

// ---------------------------------------------------------------------------
// T019: --format json envelope unchanged with deprecation on stderr
// ---------------------------------------------------------------------------

#[test]
fn deprecation_json_envelope_unchanged() {
    let dir = common::setup_repo();
    common::create_project(&dir, "alpha");

    // Use source add with --type (deprecated) that should succeed with URL input
    let (stdout, stderr, code) = run(&[
        "--root",
        &dir.path().to_string_lossy(),
        "--format",
        "json",
        "source",
        "add",
        "--name",
        "test-source",
        "--type",
        "auto",
        "https://example.com/doc",
        "--project",
        "alpha",
    ]);
    // Deprecation should be on stderr
    assert!(stderr.contains("[deprecated]"), "stderr should have deprecation: {stderr:?}");

    // Command should succeed
    assert_eq!(code, 0, "source add should succeed, stderr: {stderr:?}");

    // JSON envelope should be valid and unchanged format
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("stdout should be valid JSON");
    assert_eq!(parsed["status"], "ok", "envelope should have status=ok: {stdout}");
    assert!(parsed["data"].is_object(), "envelope should have data object: {stdout}");
}

// ---------------------------------------------------------------------------
// T020: --quiet does NOT suppress deprecation warning
// ---------------------------------------------------------------------------

#[test]
fn deprecation_quiet_does_not_suppress() {
    let dir = common::setup_repo();
    common::create_project(&dir, "alpha");

    let (_stdout, stderr, _code) = run(&[
        "--root",
        &dir.path().to_string_lossy(),
        "--quiet",
        "publish",
        "update",
        "my-article",
        "--target",
        "local",
        "--status",
        "draft",
        "--project",
        "alpha",
    ]);
    // Deprecation warning should still be on stderr even with --quiet
    assert!(stderr.contains("[deprecated]"), "--quiet should not suppress deprecation on stderr: {stderr:?}");
}

// ---------------------------------------------------------------------------
// T021: --no-color strips ANSI from deprecation
// ---------------------------------------------------------------------------

#[test]
fn deprecation_no_color_strips_ansi() {
    let dir = common::setup_repo();
    common::create_project(&dir, "alpha");

    let (_stdout, stderr, _code) = run(&[
        "--root",
        &dir.path().to_string_lossy(),
        "--no-color",
        "source",
        "remove",
        "some-name",
        "--project",
        "alpha",
    ]);
    assert!(stderr.contains("[deprecated]"), "stderr should have deprecation: {stderr:?}");
    // With --no-color, there should be no ANSI escape sequences
    assert!(!stderr.contains('\x1b'), "stderr should not contain ANSI escapes: {stderr:?}");
}

// ---------------------------------------------------------------------------
// T022: SC-005 — all 5 deprecation classes (D1-D5) produce correct stderr format
// ---------------------------------------------------------------------------

#[test]
fn deprecation_d1a_warning_format() {
    let dir = common::setup_repo();
    common::create_project(&dir, "alpha");

    let (_stdout, stderr, _code) = run(&[
        "--root",
        &dir.path().to_string_lossy(),
        "publish",
        "update",
        "my-article",
        "--target",
        "local",
        "--status",
        "draft",
        "--project",
        "alpha",
    ]);
    assert!(stderr.contains("[deprecated] --status is deprecated, use --set status=<value> instead"));
}

#[test]
fn deprecation_d1b_warning_format() {
    let dir = common::setup_repo();
    common::create_project(&dir, "alpha");

    let (_stdout, stderr, _code) = run(&[
        "--root",
        &dir.path().to_string_lossy(),
        "publish",
        "update",
        "my-article",
        "--target",
        "local",
        "--target-url",
        "https://example.com",
        "--project",
        "alpha",
    ]);
    assert!(stderr.contains("[deprecated] --target-url is deprecated, use --set url=<value> instead"));
}

#[test]
fn deprecation_d2_subject_warning_format() {
    let dir = common::setup_repo();
    common::create_project(&dir, "alpha");

    // --type triggers the subject-level warning
    let (_stdout, stderr, _code) = run(&[
        "--root",
        &dir.path().to_string_lossy(),
        "source",
        "add",
        "--name",
        "test-source",
        "--type",
        "auto",
        "https://example.com/doc",
        "--project",
        "alpha",
    ]);
    assert!(stderr.contains("[deprecated] --type is deprecated, use --file-kind or --source-kind instead"));
}

#[test]
fn deprecation_d3_warning_format() {
    let dir = common::setup_repo();
    common::create_project(&dir, "alpha");

    let (_stdout, stderr, _code) =
        run(&["--root", &dir.path().to_string_lossy(), "source", "remove", "some-name", "--project", "alpha"]);
    assert!(stderr
        .contains("[deprecated] positional NAME is deprecated, use full PATH (e.g., sources/yuque/foo.md) instead"));
}

#[test]
fn deprecation_d4a_warning_format() {
    let dir = common::setup_repo();
    common::create_project(&dir, "alpha");

    let (_stdout, stderr, _code) = run(&[
        "--root",
        &dir.path().to_string_lossy(),
        "term",
        "learn",
        "--original",
        "old-term",
        "--correct",
        "Mind Repo",
        "--project",
        "alpha",
    ]);
    assert!(stderr.contains("[deprecated] --original is deprecated, use --alias <variant> instead"));
}

#[test]
fn deprecation_d4b_warning_format() {
    let dir = common::setup_repo();
    common::create_project(&dir, "alpha");

    let (_stdout, stderr, _code) = run(&[
        "--root",
        &dir.path().to_string_lossy(),
        "term",
        "learn",
        "--original",
        "old-term",
        "--correct",
        "Mind Repo",
        "--project",
        "alpha",
    ]);
    assert!(stderr.contains("[deprecated] --correct is deprecated, use --term <canonical> instead"));
}

#[test]
fn deprecation_d5_warning_format() {
    let dir = common::setup_repo();
    common::create_project(&dir, "alpha");
    // Add a term so term list --term has something to find
    let index_yaml = r#"schema_version: '1'
terms:
  - term: API
    definition: Application Programming Interface
    aliases: []
    corrections: []
"#;
    common::write_index(&dir, "alpha", index_yaml);

    let (_stdout, stderr, _code) =
        run(&["--root", &dir.path().to_string_lossy(), "term", "list", "--term", "API", "--project", "alpha"]);
    assert!(stderr.contains("[deprecated] term list --term <X> is deprecated, use term show <X> instead"));
}
