use assert_cmd::Command;

mod common;

/// Helper: run a command and return (parsed_json, stderr, exit_code).
///
/// Errors are emitted on stderr as JSON envelopes, so we try stderr first
/// and fall back to stdout for success responses.
fn json_run(args: &[&str], cwd: &std::path::Path) -> (serde_json::Value, String, Option<i32>) {
    let output =
        Command::cargo_bin("mf").expect("binary exists").current_dir(cwd).args(args).output().expect("command runs");
    let code = output.status.code();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let body = if code != Some(0) && !stderr.is_empty() { &stderr } else { &stdout };
    let parsed = serde_json::from_str(body).unwrap_or_else(|_| serde_json::Value::String(body.clone()));
    (parsed, stderr, code)
}

// ---------------------------------------------------------------------------
// Multi-slot template rejection (T017a)
// ---------------------------------------------------------------------------

#[test]
fn multi_slot_template_rejected() {
    // Two date slots with non-subset formats (same width) triggers error
    let repo = common::scaffold_project_with_template(
        "my-project",
        "broken",
        "outputs/{date:YYYY-MM-DD}/{date:YYYY-MM-DD}.md",
        "generated",
        &[],
    );

    let (parsed, _stderr, code) =
        json_run(&["--format", "json", "article", "list"], repo.path().join("my-project").as_path());
    assert_ne!(code, Some(0), "multi-slot template should be rejected");
    assert_eq!(parsed["error"]["kind"], "multi_slot_template", "error kind mismatch");
    let msg = parsed["error"]["message"].as_str().unwrap_or("");
    assert!(msg.contains("broken"), "error message should name the template: {msg}");
}

// ---------------------------------------------------------------------------
// Unknown placeholder in pattern (T017b)
// ---------------------------------------------------------------------------

#[test]
fn unknown_placeholder_in_pattern() {
    let repo = common::scaffold_project_with_template(
        "my-project",
        "bad_pattern",
        "outputs/{quarter:QQ}.md",
        "generated",
        &[],
    );

    let (parsed, _stderr, code) =
        json_run(&["--format", "json", "article", "list"], repo.path().join("my-project").as_path());
    assert_ne!(code, Some(0), "unknown placeholder should be rejected");
    assert_eq!(parsed["error"]["kind"], "unknown_placeholder", "error kind mismatch");
}

// ---------------------------------------------------------------------------
// Invalid template name rejected (T017c)
// ---------------------------------------------------------------------------

#[test]
fn invalid_template_name_rejected() {
    let repo = common::setup_repo();
    common::create_project(&repo, "my-project");

    // Write mind.yaml with a template key that starts with uppercase (invalid)
    let mind_yaml = r#"schema: '1'
templates:
  BadName:
    pattern: "outputs/{date:YYYY-MM-DD}.md"
    mode: generated
"#;
    common::write_mind_yaml(&repo, "my-project", mind_yaml);

    let (parsed, _stderr, code) =
        json_run(&["--format", "json", "article", "list"], repo.path().join("my-project").as_path());
    assert_ne!(code, Some(0), "invalid template name should be rejected");
    assert_eq!(parsed["error"]["kind"], "invalid_template_name", "error kind mismatch");
    let msg = parsed["error"]["message"].as_str().unwrap_or("");
    assert!(msg.contains("BadName"), "error message should name the template: {msg}");
}
