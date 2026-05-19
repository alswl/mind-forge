use assert_cmd::Command;

mod common;

fn run(args: &[&str]) -> (String, String, i32) {
    let output = Command::cargo_bin("mf").expect("binary exists").args(args).output().expect("command runs");
    (
        String::from_utf8(output.stdout).expect("stdout utf8"),
        String::from_utf8(output.stderr).expect("stderr utf8"),
        output.status.code().unwrap_or_default(),
    )
}

fn setup_with_term() -> common::TempDir {
    let repo = common::setup_repo();
    common::create_project(&repo, "alpha");
    let index_yaml = r#"schema_version: '1'
terms:
  - term: API
    definition: Application Programming Interface
    aliases:
      - application-api
    tags:
      - tech
    corrections:
      - original: ap-i
        correct: API
  - term: CLI
    definition: Command Line Interface
    aliases: []
    corrections: []
"#;
    common::write_index(&repo, "alpha", index_yaml);
    repo
}

// ---------------------------------------------------------------------------
// T089: mf term show "API" happy path
// ---------------------------------------------------------------------------

#[test]
fn term_show_happy_path() {
    let repo = setup_with_term();

    let (stdout, stderr, code) = run(&["--root", &repo.path().to_string_lossy(), "term", "show", "API", "-p", "alpha"]);

    assert_eq!(code, 0, "should succeed, stderr: {stderr:?}");
    assert!(stderr.is_empty(), "stderr should be clean: {stderr:?}");

    // Text output should contain key fields
    assert!(stdout.contains("API"), "output should contain term name: {stdout}");
    assert!(stdout.contains("Application Programming Interface"), "output should contain definition: {stdout}");
    assert!(stdout.contains("application-api"), "output should contain alias: {stdout}");
    assert!(stdout.contains("tech"), "output should contain tag: {stdout}");
    assert!(stdout.contains("ap-i"), "output should contain correction: {stdout}");
}

#[test]
fn term_show_json() {
    let repo = setup_with_term();

    let (stdout, stderr, code) =
        run(&["--root", &repo.path().to_string_lossy(), "--format", "json", "term", "show", "API", "-p", "alpha"]);

    assert_eq!(code, 0, "should succeed, stderr: {stderr:?}");
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    assert_eq!(parsed["status"], "ok", "envelope ok: {stdout}");
    assert_eq!(parsed["data"]["term"], "API", "term name in data: {stdout}");
    assert_eq!(parsed["data"]["definition"], "Application Programming Interface", "definition: {stdout}");
}

// ---------------------------------------------------------------------------
// T090: mf term show nonexistent (error)
// ---------------------------------------------------------------------------

#[test]
fn term_show_nonexistent_errors() {
    let repo = setup_with_term();

    let (_stdout, stderr, code) =
        run(&["--root", &repo.path().to_string_lossy(), "term", "show", "NonExistent", "-p", "alpha"]);

    assert_eq!(code, 2, "should error with exit code 2: stderr={stderr:?}");
    assert!(stderr.contains("not found"), "error should mention 'not found': {stderr:?}");
}

// ---------------------------------------------------------------------------
// T091: mf term list --term "API" routes to show + D5 warning
// ---------------------------------------------------------------------------

#[test]
fn term_list_term_routes_to_show() {
    let repo = setup_with_term();

    let (stdout_list_term, stderr_list_term, code) =
        run(&["--root", &repo.path().to_string_lossy(), "term", "list", "--term", "API", "-p", "alpha"]);

    // Should succeed
    assert_eq!(code, 0, "should succeed, stderr: {stderr_list_term:?}");

    // D5 deprecation warning on stderr
    assert!(stderr_list_term.contains("[deprecated]"), "stderr should have deprecation warning: {stderr_list_term:?}");

    // Output should match term show output
    let (stdout_show, _stderr_show, _code_show) =
        run(&["--root", &repo.path().to_string_lossy(), "term", "show", "API", "-p", "alpha"]);

    assert_eq!(stdout_list_term, stdout_show, "--term output should match term show output");
}

#[test]
fn term_list_term_json_shape() {
    let repo = setup_with_term();

    let (stdout, stderr, code) = run(&[
        "--root",
        &repo.path().to_string_lossy(),
        "--format",
        "json",
        "term",
        "list",
        "--term",
        "API",
        "-p",
        "alpha",
    ]);

    assert_eq!(code, 0, "should succeed, stderr: {stderr:?}");
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    assert_eq!(parsed["status"], "ok");
    assert_eq!(parsed["data"]["term"], "API");
}

// ── FR-003: show on repo-format global terms file ────────────────────────────

fn repo_format_fixture(name: &str) -> String {
    std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/term_repo_format").join(name),
    )
    .unwrap()
}

#[test]
fn show_repo_format() {
    let repo = common::setup_repo();
    let fixture = repo_format_fixture("simple.yaml");
    std::fs::write(repo.path().join("minds-terms.yaml"), &fixture).unwrap();

    let (stdout, stderr, code) = run(&["--root", &repo.path().to_string_lossy(), "term", "show", "cafed"]);

    assert_eq!(code, 0, "should succeed, stderr: {stderr:?}");
    assert!(stderr.is_empty(), "stderr should be clean: {stderr:?}");
    assert!(stdout.contains("cafed"), "should show term name: {stdout}");
    assert!(stdout.contains("凯飞迪"), "should show misrecognition as correction: {stdout}");
}

#[test]
fn show_repo_format_not_found() {
    let repo = common::setup_repo();
    let fixture = repo_format_fixture("simple.yaml");
    std::fs::write(repo.path().join("minds-terms.yaml"), &fixture).unwrap();

    let (_stdout, stderr, code) = run(&["--root", &repo.path().to_string_lossy(), "term", "show", "nonexistent"]);

    assert_eq!(code, 2, "should exit with usage error");
    assert!(stderr.contains("not found"), "stderr should say not found: {stderr:?}");
}

#[test]
fn show_repo_format_json() {
    let repo = common::setup_repo();
    let fixture = repo_format_fixture("simple.yaml");
    std::fs::write(repo.path().join("minds-terms.yaml"), &fixture).unwrap();

    let (stdout, stderr, code) =
        run(&["--root", &repo.path().to_string_lossy(), "--format", "json", "term", "show", "cafed"]);

    assert_eq!(code, 0, "should succeed, stderr: {stderr:?}");
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    assert_eq!(parsed["status"], "ok");
    assert_eq!(parsed["data"]["term"], "cafed");
    assert_eq!(parsed["data"]["definition"], serde_json::Value::Null);
    let corr = parsed["data"]["corrections"].as_array().unwrap();
    assert!(!corr.is_empty(), "should have corrections from misrecognitions");
}
