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

    let (stdout, stderr, code) =
        run(&["--root", &repo.path().to_string_lossy(), "term", "show", "API", "--project", "alpha"]);

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

    let (stdout, stderr, code) = run(&[
        "--root",
        &repo.path().to_string_lossy(),
        "--format",
        "json",
        "term",
        "show",
        "API",
        "--project",
        "alpha",
    ]);

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
        run(&["--root", &repo.path().to_string_lossy(), "term", "show", "NonExistent", "--project", "alpha"]);

    assert_eq!(code, 2, "should error with exit code 2: stderr={stderr:?}");
    assert!(stderr.contains("not found"), "error should mention 'not found': {stderr:?}");
}

// ---------------------------------------------------------------------------
// T091: mf term list --term "API" routes to show + D5 warning
// ---------------------------------------------------------------------------

// term list --term flag removed (use term show directly)
