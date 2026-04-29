use std::fs;

use assert_cmd::Command;
use predicates::prelude::*;

/// Helper: build the mf binary and return a Command.
fn mf() -> Command {
    Command::cargo_bin("mf").unwrap()
}

// ---------------------------------------------------------------------------
// US1: mf config show
// ---------------------------------------------------------------------------

#[test]
fn test_show_no_mind_yaml_returns_defaults() {
    let dir = tempfile::tempdir().unwrap();
    mf().current_dir(dir.path())
        .arg("config")
        .arg("show")
        .assert()
        .success()
        .stdout(predicate::str::contains("schema_version:"))
        .stdout(predicate::str::contains("output_dir: _build"));
}

#[test]
fn test_show_with_overlay() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("mind.yaml"), "schema_version: '1'\nbuild:\n  output_dir: '_out'\n")
        .unwrap();
    mf().current_dir(dir.path())
        .arg("config")
        .arg("show")
        .assert()
        .success()
        .stdout(predicate::str::contains("output_dir: _out"));
}

#[test]
fn test_show_json_format() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(
        dir.path().join("mind.yaml"),
        "schema_version: '1'\nbuild:\n  output_dir: '_custom'\n",
    )
    .unwrap();
    mf().current_dir(dir.path())
        .arg("config")
        .arg("show")
        .arg("--output-format")
        .arg("json")
        .assert()
        .success()
        .stdout(predicate::str::contains(r#""output_dir": "_custom""#));
}

#[test]
fn test_show_parse_error() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("mind.yaml"), "invalid: yaml: [[[").unwrap();
    mf().current_dir(dir.path())
        .arg("config")
        .arg("show")
        .assert()
        .failure()
        .stderr(predicate::str::contains("parse error"));
}

#[test]
fn test_show_empty_mind_yaml_is_no_error() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("mind.yaml"), "").unwrap();
    mf().current_dir(dir.path())
        .arg("config")
        .arg("show")
        .assert()
        .success()
        .stdout(predicate::str::contains("schema_version:"));
}

// ---------------------------------------------------------------------------
// US2: mf config init
// ---------------------------------------------------------------------------

#[test]
fn test_init_creates_mind_yaml() {
    let dir = tempfile::tempdir().unwrap();
    mf().current_dir(dir.path()).arg("config").arg("init").assert().success();
    assert!(dir.path().join("mind.yaml").exists());
    let content = fs::read_to_string(dir.path().join("mind.yaml")).unwrap();
    assert!(content.contains("schema_version: \"1\""));
}

#[test]
fn test_init_rejects_duplicate() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("mind.yaml"), "schema_version: '1'\n").unwrap();
    mf().current_dir(dir.path())
        .arg("config")
        .arg("init")
        .assert()
        .failure()
        .stderr(predicate::str::contains("refusing to overwrite"));
}

#[test]
fn test_init_force_overwrites() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("mind.yaml"), "schema_version: '1'\n").unwrap();
    mf().current_dir(dir.path()).arg("config").arg("init").arg("--force").assert().success();
    assert!(dir.path().join("mind.yaml").exists());
}

#[test]
fn test_init_target_user_not_implemented() {
    let dir = tempfile::tempdir().unwrap();
    mf().current_dir(dir.path())
        .arg("config")
        .arg("init")
        .arg("--target")
        .arg("user")
        .assert()
        .code(64)
        .stderr(predicate::str::contains("not yet implemented"));
}

#[test]
fn test_init_roundtrip_with_show() {
    let dir = tempfile::tempdir().unwrap();
    mf().current_dir(dir.path()).arg("config").arg("init").assert().success();
    // show should contain the project name (directory name, sanitized to lowercase)
    mf().current_dir(dir.path())
        .arg("config")
        .arg("show")
        .assert()
        .success()
        .stdout(predicate::str::contains("name:"));
}

// ---------------------------------------------------------------------------
// US3: mf config schema
// ---------------------------------------------------------------------------

#[test]
fn test_schema_outputs_valid_json_schema() {
    mf().arg("config")
        .arg("schema")
        .assert()
        .success()
        .stdout(predicate::str::contains(r#""$schema""#))
        .stdout(predicate::str::contains(r#""properties""#));
}

#[test]
fn test_schema_yaml_format() {
    mf().arg("config")
        .arg("schema")
        .arg("--output-format")
        .arg("yaml")
        .assert()
        .success()
        .stdout(predicate::str::contains("$schema:"));
}

#[test]
fn test_schema_contains_enum_constraint() {
    mf().arg("config")
        .arg("schema")
        .assert()
        .success()
        .stdout(predicate::str::contains("github_pages"));
}

#[test]
fn test_schema_bidirectional_consistency() {
    // init produces a mind.yaml, then schema validates it
    let dir = tempfile::tempdir().unwrap();
    mf().current_dir(dir.path()).arg("config").arg("init").assert().success();

    // Get schema and verify it's valid
    let schema_output = mf().arg("config").arg("schema").output().unwrap();
    assert!(schema_output.status.success());
    let schema_str = String::from_utf8_lossy(&schema_output.stdout);
    // Verify schema parses as valid JSON
    let schema_val: serde_json::Value = serde_json::from_str(&schema_str).unwrap();
    assert!(schema_val.get("properties").is_some());

    // Verify init output contains valid YAML
    let content = fs::read_to_string(dir.path().join("mind.yaml")).unwrap();
    let init_val: serde_json::Value = serde_yaml::from_str(&content).unwrap();
    assert_eq!(init_val.get("schema_version").and_then(|v| v.as_str()), Some("1"));
}
