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

// ---------------------------------------------------------------------------
// T086: mf config compile removed — returns unknown subcommand error
// ---------------------------------------------------------------------------

#[test]
fn config_compile_removed() {
    let dir = common::setup_repo();
    common::create_project(&dir, "my-project");

    let (_stdout, stderr_compile, code_compile) = run(&["--root", &dir.path().to_string_lossy(), "config", "compile"]);

    assert_eq!(code_compile, 2, "config compile should fail as unknown subcommand, got stderr: {stderr_compile:?}");
    assert!(stderr_compile.contains("compile"), "error should mention 'compile', got: {stderr_compile:?}");
}

// ---------------------------------------------------------------------------
// T087: mf config generate writes mind.gen.yaml
// ---------------------------------------------------------------------------

#[test]
fn config_generate_writes_output_file() {
    let dir = common::setup_repo();
    common::create_project(&dir, "my-project");
    let output_path = dir.path().join("mind.gen.yaml");

    let (_stdout, stderr, code) =
        run(&["--root", &dir.path().to_string_lossy(), "config", "generate", "--out", &output_path.to_string_lossy()]);

    assert_eq!(code, 0, "generate should succeed, stderr: {stderr:?}");
    assert!(stderr.is_empty(), "generate should have clean stderr, got: {stderr:?}");

    // Verify the output file was created and has content
    assert!(output_path.exists(), "output file should exist");
    let content = std::fs::read_to_string(&output_path).unwrap();
    assert!(!content.is_empty(), "output file should not be empty");
    assert!(content.contains("schema_version"), "output should contain YAML content: {content}");
}

// ---------------------------------------------------------------------------
// T088: mf config default outputs valid YAML defaults
// ---------------------------------------------------------------------------

#[test]
fn config_default_outputs_valid_defaults() {
    let (stdout, stderr, code) = run(&["config", "default"]);

    assert_eq!(code, 0, "default should succeed, stderr: {stderr:?}");
    assert!(stderr.is_empty(), "default should have clean stderr, got: {stderr:?}");
    assert!(!stdout.is_empty(), "default output should not be empty");
    assert!(
        stdout.contains("schema_version") || stdout.contains("projects_dir"),
        "default output should contain config fields: {stdout}"
    );
}

#[test]
fn config_default_json_output() {
    let (stdout, stderr, code) = run(&["--output", "json", "config", "default", "--output-format", "json"]);

    assert_eq!(code, 0, "default json should succeed, stderr: {stderr:?}");
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("stdout should be valid JSON");
    assert_eq!(parsed["status"], "ok", "envelope should have status=ok");
    assert!(parsed["data"].is_object(), "data should be an object");
}
